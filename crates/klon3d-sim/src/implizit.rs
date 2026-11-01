//! Impliziter Schritt mit konjugierten Gradienten — das Verfahren von CLO, zum
//! Vergleich mit der XPBD-Referenz (Befund 35).
//!
//! CLO löst je Zeitschritt ein dünn besetztes lineares System mit Kraft und
//! Jacobi-Matrix je Constraint und konjugierten Gradienten (Befund 09:
//! `Compute…ForceAndJacobian`, `ComputeDeltaX`; Befund 43: Block-CG mit
//! symmetrischen 3×3-Blöcken). Voreinstellung (Befund 49): Zeitschritt 1/30 s,
//! 50 CG-Iterationen, Nahtfedern `fSeamSpringK` 500 000 (= 500 N/m in der
//! Umrechnung von `MATERIAL.md`). Gebaut ist hier das Lehrbuchverfahren dazu,
//! nicht CLOs Code: **Rückwärts-Euler mit einem Newton-Schritt** (Baraff &
//! Witkin, „Large Steps in Cloth Simulation“, 1998):
//!
//! ```text
//! (M·(1 + h·c) + h²·H) · Δv = h · (f(x, v) − h·H·v),     x ← x + h·(v + Δv)
//! ```
//!
//! mit H = −∂f/∂x (positiv semidefinit gemacht) und Luftdämpfung c.
//!
//! * **Dehnung:** dieselben Kantenfedern wie die Referenz, k = 1/α. Für H gilt
//!   je Kante k·[n·nᵀ + max(0, 1 − L₀/l)·(I − n·nᵀ)] — die Querrichtung nur
//!   unter Zug, sonst wäre H nicht positiv (Choi & Ko 2002).
//! * **Naht:** Feder mit `naht_k` (Vorgabe CLO: 500 N/m) und der Ruhelänge der
//!   Szene, sonst 0 (dann H = k·I); H wie bei den Kanten. Mit einer Ruhelänge,
//!   die je Bild kürzer wird ([`crate::Szene::naht_kuerzen`]), schließt sich
//!   eine Naht mit begrenzter Geschwindigkeit statt in einem Bild (Befund 96).
//! * **Biegung:** Kraft −k·θ·∇θ aus dem Diederwinkel, in H die Gauß-Newton-
//!   Näherung k·∇θ·∇θᵀ (positiv).
//! * **Kontakt:** Punkte an oder in einem Körper werden im CG in Normalrichtung
//!   festgelegt (Filter, „modified PCG“ nach Baraff & Witkin): die neue
//!   Normalgeschwindigkeit bringt sie genau auf die Oberfläche. Die
//!   **Normalkraft fällt aus dem System heraus** (Rest A·Δv − b in
//!   Normalrichtung); zieht sie am Körper, wird der Punkt freigegeben und noch
//!   einmal gelöst.
//! * **Durchstoß:** Nach dem Schritt wird der Weg jedes Punkts gegen ein
//!   Körpernetz geprüft; hat er die Haut von außen durchstoßen, setzt er am
//!   Eintritt auf ([`crate::netzkoerper`]). Ohne das ginge ein Punkt, der
//!   schneller als Suchtiefe/h läuft (10 mm bei 1/60 s: 0,6 m/s), verloren.
//! * **Körperpunkt gegen Stoffdreieck** (wahlweise, Vorgabe aus): danach
//!   heben bis zu `koerperpunkt_durchgaenge` Durchgänge jedes Stoffdreieck,
//!   durch das ein Körperpunkt sticht
//!   ([`crate::Koerpernetz::punkte_gegen_dreiecke`], Befund 96). Beim Anziehen
//!   läuft das während des Nähens ([`crate::platzierung::anziehen`]); in Ruhe
//!   zieht der Schritt gespannte Dreiecke jedes Bild wieder ein wenig durch,
//!   und die Korrektur hielte die Szene in Bewegung.
//! * **Reibung:** Coulomb auf die Geschwindigkeit relativ zum Körper: steht
//!   der Punkt, hält ihn μ·F_N·h/m, sonst bremst sie um so viel.
//! * **CG:** matrixfrei, Vorkonditionierer mit den 3×3-Diagonalblöcken von A,
//!   Abbruch bei ‖r‖² ≤ tol²·‖b‖² oder nach `cg_max` Iterationen.
//!
//! Ein Schritt verschiebt um g·h² ≈ 10⁻² m — weit über der f32-Auflösung; die
//! Verschiebungsfelder der Referenz braucht dieser Weg nicht.

use crate::cpu::{dieder, Rechenweg};
use crate::koerper::{add, dot, mul, norm, sub};
use crate::selbstkollision::{self, Feder, Stoffnetz, Stoffwerte};
use crate::szene::Szene;

/// Die Paare (Punkt der ersten Seite, Punkt der zweiten Seite, Gewicht
/// |gₖ|·|gₗ|), über die eine Feder in die Systemmatrix geht — bei CLO je
/// Punkt–Dreieck 3, je Kante–Kante 4, je Durchdringung 6 Nebendiagonal-Blöcke,
/// die `UpdateCOOValue` in Laplace-Form bringt (−B daneben, +B auf beide
/// Diagonalen; Befund 116).
fn feder_paare(fe: &Feder) -> impl Iterator<Item = (usize, usize, f64)> + '_ {
    (0..5).flat_map(move |q| {
        (0..5).filter_map(move |r| {
            let (gq, gr) = (fe.gewichte[q], fe.gewichte[r]);
            (gq > 0.0 && gr < 0.0).then(|| (fe.punkte[q] as usize, fe.punkte[r] as usize, -gq * gr))
        })
    })
}

/// 3×3-Matrix, zeilenweise.
type M3 = [[f64; 3]; 3];

fn m3_add_skal(m: &mut M3, s: f64) {
    for (k, z) in m.iter_mut().enumerate() {
        z[k] += s;
    }
}
fn m3_add_aus(m: &mut M3, a: [f64; 3], b: [f64; 3], s: f64) {
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] += s * a[i] * b[j];
        }
    }
}
fn m3_mal(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}
fn m3_inv(m: &M3) -> M3 {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    let s = 1.0 / det;
    [
        [(e * i - f * h) * s, (c * h - b * i) * s, (b * f - c * e) * s],
        [(f * g - d * i) * s, (a * i - c * g) * s, (c * d - a * f) * s],
        [(d * h - e * g) * s, (b * g - a * h) * s, (a * e - b * d) * s],
    ]
}

/// Einstellungen des impliziten Wegs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImplizitParameter {
    /// Substeps per frame (CLO: frame 1/30 s; preset `Nonlinear` five
    /// substeps, `Normal` one).
    pub schritte_je_bild: usize,
    /// Maximum number of CG iterations per solve (CLO: 50).
    pub cg_max: usize,
    /// Stop at ‖r‖ ≤ tol·‖b‖ (CLO "residual"; `Nonlinear` 1e-5, `Normal`
    /// 1e-4).
    pub cg_toleranz: f64,
    /// Nahtfeder (N/m), CLO `fSeamSpringK` 500 000 g/s².
    pub naht_k: f64,
    /// Kontakt schon ab diesem Abstand zur Oberfläche (m).
    pub kontakt_rand: f64,
    /// Normalkraft der Reibung aus der physikalischen Last (Kantenzug +
    /// Gewicht gegen die Normale, wie die XPBD-Referenz) statt aus dem Rest
    /// des Gleichungssystems — robust, wenn das CG nicht konvergiert.
    pub normalkraft_aus_last: bool,
    /// Den Weg je Schritt gegen Körpernetze prüfen und am Eintritt aufsetzen
    /// ([`crate::Koerper::durchstoss`]). Nur zum Vergleich abschaltbar.
    pub durchstoss_pruefen: bool,
    /// Durchgänge Körperpunkt gegen Stoffdreieck je Schritt
    /// ([`crate::Koerpernetz::punkte_gegen_dreiecke`]); 0 schaltet ab.
    pub koerperpunkt_durchgaenge: usize,
    /// So tief (m) unter einem Körperpunkt wird ein Stoffdreieck noch gehoben.
    pub koerperpunkt_tiefe: f64,
    /// Selbstkollision Stoff gegen Stoff wie CLO ([`crate::selbstkollision`],
    /// Befund 116): Nähe-Federn im System, Stoßprüfung danach.
    pub selbstkollision: bool,
    /// Zusatzdicke „Kollision“ je Teil (m), CLO-Vorgabe 2,5 mm: Radius je
    /// Punkt = (Stoffdicke + Zusatzdicke)/2.
    pub selbst_zusatzdicke: f64,
    /// Steifigkeitsfaktor der Nähe (CLO `AvoidanceStiffness`, 0,001).
    pub selbst_steifigkeit: f64,
    /// Durchgänge der Stoßantwort (CLO `SelfCollisionIterationCount`, 1).
    pub selbst_stoss_iterationen: usize,
    /// Durchdringungen auch zwischen Nahtpartnern suchen wie CLO (Vorgabe aus,
    /// [`selbstkollision::finde_schnitte`]).
    pub selbst_schnitte_an_naehten: bool,
    /// Faktor auf die Steifigkeit jeder Klebezone (Vorgabe 1). Die Steifigkeit
    /// selbst steht in der Zone und kommt aus der **Dehnsteifigkeit** der oberen
    /// Lage, nicht aus ihrer Biegesteifigkeit ([`crate::klebung::Stoffpaar`]);
    /// der Faktor ist zum Prüfen da. Die Biegung des geklebten Paares steht
    /// getrennt in der Zone (`Zone::biegefaktor`) und wirkt auf die
    /// Biege-Constraints, nicht auf die Federn. CLO rechnet die Klebe als Feder
    /// mit Jacobian, nicht als feste Kopplung (Befund 59).
    pub klebe_faktor: f64,
}

impl Default for ImplizitParameter {
    fn default() -> Self {
        ImplizitParameter {
            // CLO preset `Nonlinear.smp`: five substeps, residual 1e-5.
            // With one substep a 5.5 % Radical band on the reference body does not
            // settle and sits 2…4 % off the tape-measure circumference; the
            // error falls monotonically with the number of substeps
            // (0.06 % at five).
            schritte_je_bild: 5,
            cg_max: 50,
            cg_toleranz: 1e-5,
            naht_k: 500.0,
            kontakt_rand: 1e-4,
            normalkraft_aus_last: false,
            durchstoss_pruefen: true,
            koerperpunkt_durchgaenge: 0,
            koerperpunkt_tiefe: 0.01,
            selbstkollision: false,
            selbst_zusatzdicke: 2.5e-3,
            selbst_steifigkeit: 1e-3,
            selbst_stoss_iterationen: 1,
            selbst_schnitte_an_naehten: false,
            // 1 = die Steifigkeit der Zone selbst (aus ihrem Stoffpaar).
            klebe_faktor: 1.0,
        }
    }
}

/// Zahlen des letzten Bildes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ImplizitStatistik {
    /// CG-Iterationen insgesamt und Lösungen (mit Freigabe-Durchgängen).
    pub cg_iterationen: usize,
    pub loesungen: usize,
    /// Lösungen, die `cg_max` erreichten, ohne die Toleranz zu treffen.
    pub nicht_konvergiert: usize,
    /// Kontakte (Punkt × Schritt), davon haftend.
    pub kontakte: usize,
    pub haftend: usize,
    /// Punkte, die wegen ziehender Normalkraft freigegeben wurden.
    pub freigegeben: usize,
    /// Wege, die die Haut eines Körpernetzes durchstießen (am Eintritt
    /// aufgesetzt).
    pub durchstoesse: usize,
    /// Stoffdreiecke, durch die ein Körperpunkt stach (gehoben).
    pub koerperpunkte: usize,
    /// Größter relativer Rest ‖r‖/‖b‖ einer Lösung.
    pub rest_max: f64,
    /// Nähe-Paare Punkt–Dreieck und Kante–Kante (Summe über die Schritte).
    pub naehe_punkt_dreieck: usize,
    pub naehe_kante_kante: usize,
    /// Stöße Stoff gegen Stoff, die die Stoßantwort zurücksetzte.
    pub selbst_stoesse: usize,
    /// Schnitte Kante durch Dreieck und ihre Gebiete (Summe über die Schritte).
    pub durchdringungen: usize,
    pub durchdringungsgebiete: usize,
}

/// Der implizite Rechenweg.
#[derive(Debug, Clone, Default)]
pub struct ImplizitCg {
    pub param: ImplizitParameter,
    pub statistik: ImplizitStatistik,
    /// Je Nahtfeder Richtung und Querfaktor des laufenden Schritts.
    naht_n: Vec<([f64; 3], f64)>,
    /// Δv des vorigen Schritts — Startwert des CG (Warmstart). Ohne ihn findet
    /// ein CG mit lokalem Vorkonditionierer die langsame Gesamtbewegung (die
    /// ganze Röhre fällt) erst nach Hunderten Iterationen: mit 50 fiel sie
    /// 170 mm statt 1.255 mm (20.10.2026).
    dv_vorher: Vec<[f64; 3]>,
    /// Stoffnetz und Stoffwerte der Selbstkollision (beim ersten Schritt gebaut).
    selbst: Option<(Stoffnetz, Stoffwerte)>,
    /// Nähe- und Schubfedern des laufenden Schritts.
    federn: Vec<Feder>,
    /// Schnitte Kante durch Dreieck des laufenden Schritts.
    schnitte: Vec<selbstkollision::Schnitt>,
    /// je Dreieck die Schritte in Folge mit Durchdringung
    folge: Vec<u32>,
}

/// Kontakt eines Punkts im Schritt.
#[derive(Debug, Clone, Copy)]
struct Fest {
    normale: [f64; 3],
    /// verlangte Normalgeschwindigkeit nach dem Schritt
    vn_soll: f64,
    v_koerper: [f64; 3],
}

impl ImplizitCg {
    pub fn neu(param: ImplizitParameter) -> Self {
        ImplizitCg { param, statistik: ImplizitStatistik::default(), naht_n: Vec::new(), dv_vorher: Vec::new(), selbst: None, federn: Vec::new(), schnitte: Vec::new(), folge: Vec::new() }
    }

    /// Ein Zeitschritt der Länge `h`.
    fn schritt(&mut self, s: &mut Szene, h: f64) {
        let n = s.x.len();
        let g = s.param.schwerkraft;
        let c = s.param.daempfung;
        let h2 = h * h;

        // Selbstkollision: Nähe-Paare suchen, Federn bilden; die Dämpfung der
        // Annäherung wirkt auf v vor allen anderen Kräften (wie bei CLO).
        let x_anfang = if self.param.selbstkollision { s.x.clone() } else { Vec::new() };
        self.federn.clear();
        // Klebezonen stehen fest und wirken in jedem Schritt, unabhängig von
        // der Selbstkollision: Eine Zone ist keine Nähe, sondern eine Bindung.
        for z in &s.klebungen {
            if z.is_empty() {
                continue;
            }
            self.federn.extend(crate::klebung::federn(z, &s.x, self.param.klebe_faktor));
        }
        if self.param.selbstkollision {
            if self.selbst.as_ref().is_none_or(|(netz, _)| netz.r_punkt.len() != n) {
                self.selbst = Some(selbstkollision::aus_szene(s, self.param.selbst_zusatzdicke));
            }
            let (netz, werte) = self.selbst.as_ref().expect("eben gebaut");
            let naehe = selbstkollision::finde(netz, &s.x, &s.w, true, true);
            for p in &naehe {
                match p.art {
                    selbstkollision::Art::PunktDreieck => self.statistik.naehe_punkt_dreieck += 1,
                    selbstkollision::Art::KanteKante => self.statistik.naehe_kante_kante += 1,
                }
            }
            self.federn = selbstkollision::federn(netz, werte, &naehe, self.param.selbst_steifigkeit, &s.masse, &s.w, &s.naht.ruhe, &mut s.v);
            // Durchdringung: Schnitte, Gebiete, Schubfedern. Eingeklemmt ist
            // ein Schnitt, wenn der Körper Kanten- und Dreiecksseite aus
            // entgegengesetzten Richtungen hält (Achsel, Schritt).
            let mut schnitte = selbstkollision::finde_schnitte(netz, &s.x, self.param.selbst_schnitte_an_naehten);
            let wurzeln = selbstkollision::gebiete(netz, &mut schnitte);
            self.statistik.durchdringungen += schnitte.len();
            self.statistik.durchdringungsgebiete += wurzeln.iter().enumerate().filter(|&(i, &r)| i == r).count();
            let rand = self.param.kontakt_rand;
            let normale = |i: usize| s.koerper.iter().find_map(|k| k.kontakt(s.x[i], s.kollisionsabstand[i] + rand).map(|kt| kt.normale));
            let eingeklemmt = |kp: [usize; 2], dp: [usize; 3]| {
                let summe = dp.iter().filter_map(|&i| normale(i)).fold([0.0; 3], add);
                norm(summe) > 0.0 && kp.iter().filter_map(|&i| normale(i)).any(|nk| dot(nk, summe) < 0.0)
            };
            let schub = selbstkollision::schubfedern(netz, werte, &schnitte, &wurzeln, self.param.selbst_steifigkeit, &mut self.folge, eingeklemmt);
            self.federn.extend(schub);
            self.schnitte = schnitte;
        }

        // Kanten: Steifigkeit, Richtung, Querfaktor; Kräfte. Daneben die
        // physikalische Last (nur Zug, plus Gewicht) für die Reibung.
        let mut f: Vec<[f64; 3]> = (0..n).map(|i| sub(mul(g, s.masse[i]), mul(s.v[i], c * s.masse[i]))).collect();
        let mut last: Vec<[f64; 3]> = (0..n).map(|i| mul(g, s.masse[i])).collect();
        let mut kk = Vec::with_capacity(s.abstand.len());
        for (e, &[i, j]) in s.abstand.punkte.iter().enumerate() {
            let (i, j) = (i as usize, j as usize);
            let k = 1.0 / s.abstand.nachgiebigkeit[e];
            let d = sub(s.x[j], s.x[i]);
            let l = norm(d).max(1e-12);
            let nr = mul(d, 1.0 / l);
            let l0 = s.abstand.ruhe[e];
            let fi = mul(nr, k * (l - l0));
            f[i] = add(f[i], fi);
            f[j] = sub(f[j], fi);
            if l > l0 {
                last[i] = add(last[i], fi);
                last[j] = sub(last[j], fi);
            }
            kk.push((k, nr, (1.0 - l0 / l).max(0.0)));
        }
        let mut naht_n = Vec::with_capacity(s.naht.len());
        for (c, &[i, j]) in s.naht.punkte.iter().enumerate() {
            let (i, j) = (i as usize, j as usize);
            let d = sub(s.x[j], s.x[i]);
            let l = norm(d);
            // Offen (unendlich, vor dem ersten Kürzen): Ruhelänge = Abstand.
            let l0 = if s.naht.ruhe[c].is_finite() { s.naht.ruhe[c] } else { l };
            let fi = if l0 > 0.0 && l > 1e-12 { mul(d, self.param.naht_k * (l - l0) / l) } else { mul(d, self.param.naht_k) };
            f[i] = add(f[i], fi);
            f[j] = sub(f[j], fi);
            // Richtung und Querfaktor wie bei den Kanten; Länge 0: k·I.
            naht_n.push(if l0 > 0.0 && l > 1e-12 { (mul(d, 1.0 / l), (1.0 - l0 / l).max(0.0)) } else { ([0.0; 3], 1.0) });
        }
        self.naht_n = naht_n;
        // Biegung: Kraft und Gauß-Newton-Glied (Gradienten merken).
        let mut biege: Vec<([usize; 4], [[f64; 3]; 4], f64)> = Vec::with_capacity(s.biegung.len());
        for (b, p) in s.biegung.punkte.iter().enumerate() {
            let idx = p.map(|i| i as usize);
            let Some((theta, gr)) = dieder(idx.map(|i| s.x[i])) else { continue };
            let k = 1.0 / s.biegung.nachgiebigkeit[b];
            for m in 0..4 {
                f[idx[m]] = sub(f[idx[m]], mul(gr[m], k * (theta - s.biegung.ruhe[b])));
            }
            biege.push((idx, gr, k));
        }
        for fe in &self.federn {
            for q in 0..5 {
                let i = fe.punkte[q] as usize;
                f[i] = add(f[i], mul(fe.kraft, fe.gewichte[q]));
            }
        }
        // b = h·(f − h·H·v)
        let mut hv = vec![[0.0; 3]; n];
        self.mal_h(s, &kk, &biege, &s.v, &mut hv);
        let rhs: Vec<[f64; 3]> = (0..n).map(|i| mul(sub(f[i], mul(hv[i], h)), h)).collect();

        // Diagonalblöcke von A für den Vorkonditionierer.
        let mut diag: Vec<M3> = (0..n)
            .map(|i| {
                let mut m = [[0.0; 3]; 3];
                m3_add_skal(&mut m, s.masse[i] * (1.0 + h * c));
                m
            })
            .collect();
        for (e, &[i, j]) in s.abstand.punkte.iter().enumerate() {
            let (k, nr, quer) = kk[e];
            for q in [i as usize, j as usize] {
                m3_add_skal(&mut diag[q], k * h2 * quer);
                m3_add_aus(&mut diag[q], nr, nr, k * h2 * (1.0 - quer));
            }
        }
        for (c, &[i, j]) in s.naht.punkte.iter().enumerate() {
            let (nr, quer) = self.naht_n[c];
            for q in [i as usize, j as usize] {
                m3_add_skal(&mut diag[q], self.param.naht_k * h2 * quer);
                m3_add_aus(&mut diag[q], nr, nr, self.param.naht_k * h2 * (1.0 - quer));
            }
        }
        for (idx, gr, k) in &biege {
            for m in 0..4 {
                m3_add_aus(&mut diag[idx[m]], gr[m], gr[m], k * h2);
            }
        }
        for fe in &self.federn {
            for (q, r, om) in feder_paare(fe) {
                for p in [q, r] {
                    for (a, z) in diag[p].iter_mut().enumerate() {
                        for (b, d) in z.iter_mut().enumerate() {
                            *d += h2 * om * fe.block[a][b];
                        }
                    }
                }
            }
        }
        let pinv: Vec<M3> = diag.iter().map(m3_inv).collect();

        // Kontakte: festlegen, lösen, ziehende freigeben, noch einmal lösen.
        let mut fest: Vec<Option<Fest>> = vec![None; n];
        for i in 0..n {
            if s.w[i] <= 0.0 {
                continue;
            }
            for k in &s.koerper {
                if let Some(kt) = k.kontakt(s.x[i], s.kollisionsabstand[i] + self.param.kontakt_rand) {
                    let tiefe = kt.tiefe - self.param.kontakt_rand;
                    fest[i] = Some(Fest { normale: kt.normale, vn_soll: tiefe / h + dot(kt.geschwindigkeit, kt.normale), v_koerper: kt.geschwindigkeit });
                }
            }
        }
        // Warmstart: Δv des vorigen Schritts, beim ersten Schritt h·g.
        let mut dv = if self.dv_vorher.len() == n { self.dv_vorher.clone() } else { vec![mul(g, h); n] };
        let mut fn_kraft = vec![0.0; n];
        for _durchgang in 0..2 {
            self.loese(s, h, &kk, &biege, &rhs, &pinv, &fest, &mut dv);
            // Normalkraft aus dem Rest r = A·Δv − b (in Normalrichtung) / h.
            let mut adv = vec![[0.0; 3]; n];
            self.mal_a_voll(s, h, &kk, &biege, &dv, &mut adv);
            let mut frei = false;
            for i in 0..n {
                if let Some(fe) = fest[i] {
                    let r = sub(adv[i], rhs[i]);
                    let fn_i = dot(r, fe.normale) / h;
                    if fn_i < 0.0 {
                        fest[i] = None;
                        frei = true;
                        self.statistik.freigegeben += 1;
                    } else {
                        fn_kraft[i] = fn_i;
                    }
                }
            }
            if !frei {
                break;
            }
        }

        self.dv_vorher.clone_from(&dv);
        // Neue Geschwindigkeit, Reibung, Lage.
        for i in 0..n {
            if s.w[i] <= 0.0 {
                continue;
            }
            let mut v = add(s.v[i], dv[i]);
            if let Some(fe) = fest[i] {
                self.statistik.kontakte += 1;
                let rel = sub(v, fe.v_koerper);
                let t = sub(rel, mul(fe.normale, dot(rel, fe.normale)));
                let lt = norm(t);
                let normal = if self.param.normalkraft_aus_last {
                    (-dot(last[i], fe.normale)).max(0.0)
                } else {
                    fn_kraft[i]
                };
                let grenze = s.reibung[i] * normal * h * s.w[i];
                if lt <= grenze {
                    self.statistik.haftend += 1;
                    v = sub(v, t);
                } else if lt > 0.0 {
                    v = sub(v, mul(t, grenze / lt));
                }
            }
            s.v[i] = v;
            let alt = s.x[i];
            s.x[i] = add(alt, mul(v, h));
            // Hat der Weg die Haut durchstoßen, setzt der Punkt am Eintritt auf
            // (sonst ginge er tiefer als die Suchtiefe verloren oder käme an
            // einem dünnen Teil auf der Gegenseite heraus). Der Stoß nimmt die
            // Normalgeschwindigkeit relativ zum Körper weg; Coulomb auf den
            // Stoß nimmt von der tangentialen bis zu μ·|vₙ|.
            for k in s.koerper.iter().filter(|_| self.param.durchstoss_pruefen) {
                if let Some((p, nk, vk)) = k.durchstoss(alt, s.x[i]) {
                    self.statistik.durchstoesse += 1;
                    s.x[i] = add(p, mul(nk, s.kollisionsabstand[i]));
                    let rel = sub(s.v[i], vk);
                    let vn = dot(rel, nk);
                    if vn < 0.0 {
                        let t = sub(rel, mul(nk, vn));
                        let lt = norm(t);
                        let grenze = s.reibung[i] * -vn;
                        let t = if lt <= grenze { [0.0; 3] } else { mul(t, 1.0 - grenze / lt) };
                        s.v[i] = add(vk, t);
                    }
                }
            }
            // Rest der Eindringung (Linearisierung) wegschieben und die
            // Geschwindigkeit in den Körper hinein entfernen.
            for k in &s.koerper {
                if let Some(kt) = k.kontakt(s.x[i], s.kollisionsabstand[i]) {
                    s.x[i] = add(s.x[i], mul(kt.normale, kt.tiefe));
                    let vn = dot(sub(s.v[i], kt.geschwindigkeit), kt.normale);
                    if vn < 0.0 {
                        s.v[i] = sub(s.v[i], mul(kt.normale, vn));
                    }
                }
            }
        }
        // Körperpunkte gegen Stoffdreiecke: kein Körperpunkt sticht durch den
        // Stoff (Kanten zwischen zwei Stoffpunkten durch Schulter oder Arm).
        for _ in 0..self.param.koerperpunkt_durchgaenge {
            let mut n = 0;
            for k in &s.koerper {
                n += k.punkte_gegen_dreiecke(self.param.koerperpunkt_tiefe, &s.dreiecke, &s.w, &mut s.x, &mut s.v);
            }
            self.statistik.koerperpunkte += n;
            if n == 0 {
                break;
            }
        }
        // Stoff gegen Stoff: den Weg prüfen und Durchgänge zurücksetzen (reine
        // Lagekorrektur, v bleibt wie bei CLO). Punkte am Körper zählen wie
        // gebundene Punkte bei CLO als schwer (× 10⁹).
        if let Some((netz, _)) = self.selbst.as_ref().filter(|_| self.param.selbstkollision) {
            let stoesse = selbstkollision::finde_stoesse(netz, &x_anfang, &s.x, &s.w, true, true);
            let stoesse = selbstkollision::ohne_durchdrungene(netz, stoesse, &self.schnitte);
            if !stoesse.is_empty() {
                let m_eff: Vec<f64> = (0..n).map(|i| if fest[i].is_some() { s.masse[i] * 1e9 } else { s.masse[i] }).collect();
                self.statistik.selbst_stoesse += selbstkollision::stossantwort(&stoesse, &mut s.x, &m_eff, &s.w, self.param.selbst_stoss_iterationen);
            }
        }
    }

    /// H·p (Steifigkeit ohne Masse und Faktor h²), für die rechte Seite.
    fn mal_h(&self, s: &Szene, kk: &[(f64, [f64; 3], f64)], biege: &[([usize; 4], [[f64; 3]; 4], f64)], p: &[[f64; 3]], out: &mut [[f64; 3]]) {
        for o in out.iter_mut() {
            *o = [0.0; 3];
        }
        for (e, &[i, j]) in s.abstand.punkte.iter().enumerate() {
            let (i, j) = (i as usize, j as usize);
            let (k, nr, quer) = kk[e];
            let d = sub(p[i], p[j]);
            let bd = mul(add(mul(nr, dot(nr, d) * (1.0 - quer)), mul(d, quer)), k);
            out[i] = add(out[i], bd);
            out[j] = sub(out[j], bd);
        }
        for (c, &[i, j]) in s.naht.punkte.iter().enumerate() {
            let (i, j) = (i as usize, j as usize);
            let (nr, quer) = self.naht_n[c];
            let d = sub(p[i], p[j]);
            let bd = mul(add(mul(nr, dot(nr, d) * (1.0 - quer)), mul(d, quer)), self.param.naht_k);
            out[i] = add(out[i], bd);
            out[j] = sub(out[j], bd);
        }
        for (idx, gr, k) in biege {
            let gp: f64 = (0..4).map(|m| dot(gr[m], p[idx[m]])).sum();
            for m in 0..4 {
                out[idx[m]] = add(out[idx[m]], mul(gr[m], k * gp));
            }
        }
        for fe in &self.federn {
            for (q, r, om) in feder_paare(fe) {
                let t = mul(m3_mal(&fe.block, sub(p[q], p[r])), om);
                out[q] = add(out[q], t);
                out[r] = sub(out[r], t);
            }
        }
    }

    /// A·p = M(1 + hc)·p + h²·H·p.
    fn mal_a_voll(&self, s: &Szene, h: f64, kk: &[(f64, [f64; 3], f64)], biege: &[([usize; 4], [[f64; 3]; 4], f64)], p: &[[f64; 3]], out: &mut [[f64; 3]]) {
        self.mal_h(s, kk, biege, p, out);
        let c = s.param.daempfung;
        for i in 0..s.x.len() {
            out[i] = add(mul(out[i], h * h), mul(p[i], s.masse[i] * (1.0 + h * c)));
        }
    }

    /// Modifiziertes PCG (Baraff & Witkin 1998): festgelegte Punkte bekommen
    /// in Normalrichtung die verlangte Geschwindigkeitsänderung, der Filter
    /// hält sie dort.
    #[allow(clippy::too_many_arguments)]
    fn loese(
        &mut self,
        s: &Szene,
        h: f64,
        kk: &[(f64, [f64; 3], f64)],
        biege: &[([usize; 4], [[f64; 3]; 4], f64)],
        b: &[[f64; 3]],
        pinv: &[M3],
        fest: &[Option<Fest>],
        dv: &mut [[f64; 3]],
    ) {
        let n = s.x.len();
        let filter = |i: usize, v: [f64; 3]| -> [f64; 3] {
            match fest[i] {
                Some(fe) => sub(v, mul(fe.normale, dot(v, fe.normale))),
                None => v,
            }
        };
        // Start: frei wie zuvor, festgelegt auf den Sollwert in Normalrichtung.
        for i in 0..n {
            if s.w[i] <= 0.0 {
                dv[i] = [0.0; 3];
                continue;
            }
            if let Some(fe) = fest[i] {
                let soll = fe.vn_soll - dot(s.v[i], fe.normale);
                dv[i] = add(filter(i, dv[i]), mul(fe.normale, soll));
            }
        }
        let mut ap = vec![[0.0; 3]; n];
        self.mal_a_voll(s, h, kk, biege, dv, &mut ap);
        let mut r: Vec<[f64; 3]> = (0..n).map(|i| if s.w[i] > 0.0 { filter(i, sub(b[i], ap[i])) } else { [0.0; 3] }).collect();
        let bb: f64 = (0..n).map(|i| if s.w[i] > 0.0 { let fb = filter(i, b[i]); dot(fb, m3_mal(&pinv[i], fb)) } else { 0.0 }).sum();
        let mut z: Vec<[f64; 3]> = (0..n).map(|i| m3_mal(&pinv[i], r[i])).collect();
        let mut cdir: Vec<[f64; 3]> = (0..n).map(|i| filter(i, z[i])).collect();
        let mut delta: f64 = (0..n).map(|i| dot(r[i], cdir[i])).sum();
        // Ziel relativ zu ‖b‖ und zum Anfangsrest, aber nie unter einer
        // Geschwindigkeitsgenauigkeit von 10⁻⁹ m/s (r·P⁻¹·r ≈ Σ m·Δv²): in
        // Ruhe ist b ≈ 0, und ein rein relatives Ziel ließe das CG im
        // Rundungsrauschen weiterlaufen, bis ein winziger Nenner die
        // Schrittweite sprengt (am 20.10.2026 gesehen: Rest 10¹⁰⁴).
        let boden: f64 = s.masse.iter().sum::<f64>() * 1e-18;
        let ziel = (self.param.cg_toleranz * self.param.cg_toleranz * bb.max(delta)).max(boden);
        let mut it = 0;
        self.statistik.loesungen += 1;
        while delta > ziel && it < self.param.cg_max {
            self.mal_a_voll(s, h, kk, biege, &cdir, &mut ap);
            for i in 0..n {
                ap[i] = if s.w[i] > 0.0 { filter(i, ap[i]) } else { [0.0; 3] };
            }
            let ca: f64 = (0..n).map(|i| dot(cdir[i], ap[i])).sum();
            if ca <= f64::MIN_POSITIVE || !ca.is_finite() {
                break;
            }
            let alpha = delta / ca;
            for i in 0..n {
                dv[i] = add(dv[i], mul(cdir[i], alpha));
                r[i] = sub(r[i], mul(ap[i], alpha));
            }
            for i in 0..n {
                z[i] = m3_mal(&pinv[i], r[i]);
            }
            let neu: f64 = (0..n).map(|i| dot(r[i], z[i])).sum();
            let beta = neu / delta;
            delta = neu;
            for i in 0..n {
                cdir[i] = if s.w[i] > 0.0 { filter(i, add(z[i], mul(cdir[i], beta))) } else { [0.0; 3] };
            }
            it += 1;
        }
        self.statistik.cg_iterationen += it;
        self.statistik.rest_max = self.statistik.rest_max.max((delta / bb.max(boden)).sqrt());
        if delta > ziel {
            self.statistik.nicht_konvergiert += 1;
        }
    }
}

impl Rechenweg for ImplizitCg {
    fn bild(&mut self, s: &mut Szene) {
        let n = self.param.schritte_je_bild.max(1);
        let h = s.param.bild_dt / n as f64;
        self.statistik = ImplizitStatistik::default();
        let koerper_anfang = s.koerper.clone();
        for k in 0..n {
            let weg = (k + 1) as f64 * h;
            for (kb, ka) in s.koerper.iter_mut().zip(&koerper_anfang) {
                *kb = ka.verschoben(weg);
            }
            self.schritt(s, h);
        }
        s.zeit += s.param.bild_dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zwei freie Punkte, 10 cm auseinander, an einer Nahtfeder: Mit
    /// Ruhelänge 4 cm stellen sie sich auf 4 cm ein, mit dem Kürzen je Bild
    /// ([`Szene::naht_kuerzen`]) laufen sie mit begrenzter Geschwindigkeit
    /// zusammen bis auf 0.
    #[test]
    fn naht_mit_ruhelaenge_und_kuerzen() {
        use crate::szene::{Gruppe, Simparameter};
        let szene = |ruhe: f64| Szene {
            x: vec![[0.0; 3], [0.1, 0.0, 0.0]],
            v: vec![[0.0; 3]; 2],
            masse: vec![1e-5; 2],
            w: vec![1e5; 2],
            kollisionsabstand: vec![2e-4; 2],
            reibung: vec![0.0; 2],
            abstand: Gruppe::default(),
            naht: Gruppe { punkte: vec![[0, 1]], ruhe: vec![ruhe], nachgiebigkeit: vec![0.0], farben: vec![0..1] },
            biegung: Gruppe::default(),
            biegung_grund: Vec::new(),
            dreiecke: Vec::new(),
            ruhe_inv: Vec::new(),
            teile: Vec::new(),
            naht_biegung_ohne: 0,
            klebungen: Vec::new(),
            koerper: Vec::new(),
            param: Simparameter { schwerkraft: [0.0; 3], daempfung: 5.0, ..Simparameter::default() },
            zeit: 0.0,
        };
        let abstand = |s: &Szene| norm(sub(s.x[1], s.x[0]));
        let mut s = szene(0.04);
        let mut rw = ImplizitCg::neu(ImplizitParameter::default());
        for _ in 0..60 {
            rw.bild(&mut s);
        }
        assert!((abstand(&s) - 0.04).abs() < 1e-6, "{}", abstand(&s));
        // Kürzen um 1 mm je Bild: nach 30 Bildern 3 cm, nach 120 Bildern zu.
        let mut s = szene(0.0);
        s.naht_offen();
        let mut rw = ImplizitCg::neu(ImplizitParameter::default());
        let mut groesster_schritt = 0.0f64;
        for bild in 1..=120 {
            let vorher = abstand(&s);
            s.naht_kuerzen(1e-3);
            rw.bild(&mut s);
            groesster_schritt = groesster_schritt.max(vorher - abstand(&s));
            if bild == 30 {
                assert!((abstand(&s) - 0.07).abs() < 1e-4, "nach 30 Bildern {}", abstand(&s));
            }
        }
        assert!(abstand(&s) < 1e-6, "{}", abstand(&s));
        assert!(groesster_schritt < 1.1e-3, "Schritt {groesster_schritt}");
    }

    #[test]
    fn inverse_3x3() {
        let m = [[4.0, 1.0, 0.5], [1.0, 3.0, 0.2], [0.5, 0.2, 2.0]];
        let i = m3_inv(&m);
        for r in 0..3 {
            for c in 0..3 {
                let v: f64 = (0..3).map(|k| m[r][k] * i[k][c]).sum();
                assert!((v - if r == c { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
    }
}
