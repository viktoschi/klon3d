//! Das Körpernetz als Kollisionskörper (Schritte 4c/4d).
//!
//! Ein geschlossenes, einheitlich orientiertes Dreiecksnetz — der Körper aus
//! `klon3d koerper`. Dessen Gruppe `body` ist **eine** Komponente mit
//! Normalen nach außen (Volumen 55,7 l bei der Maßdatei der Prüfmuster) und
//! nur an Mund und Augen offen (188 Randkanten in drei Schleifen; am
//! 27.10.2026 nachgemessen). Die 4.170 offenen Kanten aus Befund 25 gehören zu
//! den übrigen Flicken des BaseMesh, nicht zum Körper.
//!
//! Gefragt wird je Stoffpunkt der **vorzeichenbehaftete Abstand** zur
//! Oberfläche:
//!
//! 1. nächster Punkt auf jedem Dreieck in der Nähe mit seiner Region (Fläche,
//!    Kante, Ecke) nach Ericson, „Real-Time Collision Detection“ (2005),
//!    Abschnitt 5.1.5;
//! 2. Vorzeichen aus der **winkelgewichteten Pseudonormale** dieser Region
//!    (Bærentzen & Aanæs, „Signed Distance Computation Using the Angle
//!    Weighted Pseudonormal“, 2005): Fläche → Flächennormale, Kante → Summe
//!    der beiden Flächennormalen, Ecke → Flächennormalen gewichtet mit dem
//!    Innenwinkel. Für ein geschlossenes Netz stimmt das Vorzeichen damit auch
//!    dort, wo mehrere Dreiecke einen Punkt gleich nah haben;
//! 3. Kontaktnormale = Richtung vom nächsten Punkt zum Stoffpunkt, nach außen
//!    gedreht, wenn er innen liegt — der Gradient des Abstandsfelds. Die
//!    Korrektur x + n·Tiefe legt den Punkt also genau `abstand` über den
//!    nächsten Oberflächenpunkt.
//!
//! Gesucht wird nur in der Nähe: ein festes Gitter über dem Körper mit einer
//! Dreiecksliste je Zelle, als Felder (Anfang je Zelle + Liste) wie die
//! GPU-Puffer (ADR-018). Ein Punkt, der tiefer als `suchtiefe` im Körper
//! steckt, wird **nicht** mehr gefunden. Deshalb prüft der implizite Schritt
//! zusätzlich den Weg jedes Punkts im Schritt ([`Koerpernetz::durchstoss`]):
//! durchstößt er die Haut von außen, setzt er am Eintritt auf. Das gilt für
//! einen ruhenden Körper; ein bewegter (Stufe 7) braucht die Prüfung gegen die
//! Bewegung der Dreiecke.
//!
//! An offenen Rändern (Mund, Augen) hat eine Kante nur ein Dreieck; ihre
//! Pseudonormale ist dessen Flächennormale, und nahe am Rand ist das
//! Vorzeichen unsicher. Kleidung kommt dort nicht hin.

use std::collections::HashMap;

use crate::koerper::{add, cross, dot, mul, norm, sub};

/// Vorgabe der Suchtiefe (m): so tief darf ein Stoffpunkt in einem Schritt
/// eindringen, ohne verloren zu gehen.
pub const SUCHTIEFE: f64 = 0.01;

/// Wo der nächste Punkt auf einem Dreieck liegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// an Ecke k (0, 1, 2)
    Ecke(u8),
    /// auf Kante k: 0 = p0p1, 1 = p1p2, 2 = p2p0
    Kante(u8),
    /// im Inneren
    Flaeche,
}

/// Ergebnis einer Abstandsfrage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Naechster {
    /// Abstand zur Oberfläche, außen positiv (m).
    pub abstand: f64,
    /// nächster Punkt auf der Oberfläche
    pub punkt: [f64; 3],
    /// Normale nach außen (Gradient des Abstandsfelds)
    pub normale: [f64; 3],
    pub dreieck: u32,
    pub region: Region,
}

/// Gleichmäßiges Gitter über dem Körper; je Zelle die Einträge (Dreiecke
/// bzw. Punkte), deren Hüllquader sie berührt.
#[derive(Debug, Clone)]
pub struct Gitter {
    pub ursprung: [f64; 3],
    /// Kantenlänge einer Zelle (m).
    pub zelle: f64,
    pub zahl: [usize; 3],
    /// Zelle z belegt `eintraege[anfang[z]..anfang[z + 1]]`.
    pub anfang: Vec<u32>,
    pub eintraege: Vec<u32>,
}

impl Gitter {
    /// Zellindex je Achse, ungekappt.
    fn index(&self, x: [f64; 3]) -> [i64; 3] {
        let mut k = [0i64; 3];
        for a in 0..3 {
            k[a] = ((x[a] - self.ursprung[a]) / self.zelle).floor() as i64;
        }
        k
    }

    fn zellnummer(&self, k: [usize; 3]) -> usize {
        (k[2] * self.zahl[1] + k[1]) * self.zahl[0] + k[0]
    }

    /// Ruft `f` für jeden Eintrag in den Zellen, die der Quader [lo, hi]
    /// berührt (ein Eintrag kann mehrfach kommen).
    pub fn je_eintrag(&self, lo: [f64; 3], hi: [f64; 3], mut f: impl FnMut(u32)) {
        let Some(r) = self.bereich(lo, hi) else { return };
        for kz in r[2].0..=r[2].1 {
            for ky in r[1].0..=r[1].1 {
                for kx in r[0].0..=r[0].1 {
                    let z = self.zellnummer([kx, ky, kz]);
                    for &e in &self.eintraege[self.anfang[z] as usize..self.anfang[z + 1] as usize] {
                        f(e);
                    }
                }
            }
        }
    }

    /// Zellbereich je Achse für den Quader [lo, hi], gekappt; `None`, wenn er
    /// das Gitter nicht berührt.
    fn bereich(&self, lo: [f64; 3], hi: [f64; 3]) -> Option<[(usize, usize); 3]> {
        let (a, b) = (self.index(lo), self.index(hi));
        let mut r = [(0, 0); 3];
        for d in 0..3 {
            let n = self.zahl[d] as i64;
            if b[d] < 0 || a[d] >= n {
                return None;
            }
            r[d] = (a[d].max(0) as usize, b[d].min(n - 1) as usize);
        }
        Some(r)
    }
}

/// Das Körpernetz mit allem, was die Abstandsfrage braucht (SI-Einheiten,
/// Szenenrahmen: z oben).
#[derive(Debug, Clone)]
pub struct Koerpernetz {
    pub punkte: Vec<[f64; 3]>,
    pub dreiecke: Vec<[u32; 3]>,
    pub flaechennormale: Vec<[f64; 3]>,
    /// Je Dreieck die Pseudonormalen seiner Kanten p0p1, p1p2, p2p0 (normiert).
    pub kantennormale: Vec<[[f64; 3]; 3]>,
    /// Winkelgewichtete Pseudonormale je Punkt (normiert).
    pub punktnormale: Vec<[f64; 3]>,
    /// Hüllkugel je Dreieck: Schwerpunkt und größter Eckabstand davon.
    pub kugel: Vec<([f64; 3], f64)>,
    pub suchtiefe: f64,
    /// Gitter der Dreiecke (Abstandsfrage, Durchstoß) und der Punkte
    /// (Körperpunkt gegen Stoffdreieck), gleich aufgeteilt.
    pub gitter: Gitter,
    pub punktgitter: Gitter,
    /// Kanten mit nur einem Dreieck (offene Ränder).
    pub randkanten: usize,
    /// Umschlossenes Volumen (m³), positiv bei Normalen nach außen.
    pub volumen: f64,
}

/// Ein Punkt aus CLOs Rahmen (mm, y oben, vorn +z) im Szenenrahmen (m, z oben,
/// vorn −y): Drehung um +90° um x.
pub fn clo_nach_szene(q: [f64; 3]) -> [f64; 3] {
    [q[0] * 1e-3, -q[2] * 1e-3, q[1] * 1e-3]
}

/// Nächster Punkt auf dem Dreieck (a, b, c) zu p und seine Region (Ericson
/// 2005, 5.1.5).
pub fn naechster_auf_dreieck(p: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> ([f64; 3], Region) {
    let quot = |z: f64, n: f64| if n > 0.0 { z / n } else { 0.0 };
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, Region::Ecke(0));
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (b, Region::Ecke(1));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (add(a, mul(ab, quot(d1, d1 - d3))), Region::Kante(0));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (c, Region::Ecke(2));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (add(a, mul(ac, quot(d2, d2 - d6))), Region::Kante(2));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = quot(d4 - d3, (d4 - d3) + (d5 - d6));
        return (add(b, mul(sub(c, b), w)), Region::Kante(1));
    }
    let s = va + vb + vc;
    let (v, w) = (quot(vb, s), quot(vc, s));
    (add(a, add(mul(ab, v), mul(ac, w))), Region::Flaeche)
}

fn normiert(a: [f64; 3]) -> [f64; 3] {
    let l = norm(a);
    if l > 0.0 {
        mul(a, 1.0 / l)
    } else {
        [0.0; 3]
    }
}

/// Winkel zwischen u und v (rad).
fn winkel(u: [f64; 3], v: [f64; 3]) -> f64 {
    norm(cross(u, v)).atan2(dot(u, v))
}

impl Koerpernetz {
    /// Baut das Netz aus Punkten (m, z oben) und Dreiecken. Verlangt ein
    /// mannigfaltiges, einheitlich orientiertes Netz mit Normalen nach außen;
    /// offene Ränder sind erlaubt und werden gezählt.
    pub fn neu(punkte: Vec<[f64; 3]>, dreiecke: Vec<[u32; 3]>, suchtiefe: f64) -> Result<Koerpernetz, String> {
        if dreiecke.is_empty() {
            return Err("Körpernetz ohne Dreiecke".into());
        }
        if !(suchtiefe > 0.0) {
            return Err(format!("Suchtiefe {suchtiefe} m muss positiv sein"));
        }
        let n = punkte.len();
        if let Some(d) = dreiecke.iter().find(|d| d.iter().any(|&i| i as usize >= n)) {
            return Err(format!("Dreieck {d:?} verweist auf einen Punkt ≥ {n}"));
        }

        // Kanten: je ungerichteter Kante die Dreiecke mit Kantennummer und
        // Laufrichtung.
        let mut kanten: HashMap<(u32, u32), Vec<(u32, u8, bool)>> = HashMap::new();
        for (t, d) in dreiecke.iter().enumerate() {
            for k in 0..3u8 {
                let (i, j) = (d[k as usize], d[(k as usize + 1) % 3]);
                kanten.entry((i.min(j), i.max(j))).or_default().push((t as u32, k, i < j));
            }
        }

        let flaechennormale: Vec<[f64; 3]> = dreiecke
            .iter()
            .map(|d| {
                let [a, b, c] = d.map(|i| punkte[i as usize]);
                normiert(cross(sub(b, a), sub(c, a)))
            })
            .collect();

        let mut kantennormale = vec![[[0.0; 3]; 3]; dreiecke.len()];
        let mut randkanten = 0;
        for (&(i, j), liste) in &kanten {
            match liste[..] {
                [(t, k, _)] => {
                    randkanten += 1;
                    kantennormale[t as usize][k as usize] = flaechennormale[t as usize];
                }
                [(t1, k1, r1), (t2, k2, r2)] => {
                    if r1 == r2 {
                        return Err(format!("Netz uneinheitlich orientiert an der Kante {i}–{j}"));
                    }
                    let nk = normiert(add(flaechennormale[t1 as usize], flaechennormale[t2 as usize]));
                    kantennormale[t1 as usize][k1 as usize] = nk;
                    kantennormale[t2 as usize][k2 as usize] = nk;
                }
                _ => return Err(format!("Kante {i}–{j} hat {} Dreiecke", liste.len())),
            }
        }

        let mut punktnormale = vec![[0.0; 3]; n];
        let mut volumen = 0.0;
        let mut kugel = Vec::with_capacity(dreiecke.len());
        for (t, d) in dreiecke.iter().enumerate() {
            let q = d.map(|i| punkte[i as usize]);
            for k in 0..3 {
                let w = winkel(sub(q[(k + 1) % 3], q[k]), sub(q[(k + 2) % 3], q[k]));
                let p = d[k] as usize;
                punktnormale[p] = add(punktnormale[p], mul(flaechennormale[t], w));
            }
            volumen += dot(q[0], cross(q[1], q[2])) / 6.0;
            let m = mul(add(add(q[0], q[1]), q[2]), 1.0 / 3.0);
            let r = q.iter().map(|&e| norm(sub(e, m))).fold(0.0, f64::max);
            kugel.push((m, r));
        }
        if volumen <= 0.0 {
            return Err(format!("Volumen {volumen:.4e} m³ ≤ 0: die Normalen zeigen nach innen"));
        }
        for p in punktnormale.iter_mut() {
            *p = normiert(*p);
        }

        let huelle = |d: &[u32; 3]| {
            let q = d.map(|i| punkte[i as usize]);
            let (mut a, mut b) = (q[0], q[0]);
            for e in &q[1..] {
                for k in 0..3 {
                    a[k] = a[k].min(e[k]);
                    b[k] = b[k].max(e[k]);
                }
            }
            (a, b)
        };
        let gitter = baue_gitter(&punkte, suchtiefe, &dreiecke.iter().map(huelle).collect::<Vec<_>>());
        let punktgitter = baue_gitter(&punkte, suchtiefe, &punkte.iter().map(|&p| (p, p)).collect::<Vec<_>>());
        Ok(Koerpernetz {
            punkte,
            dreiecke,
            flaechennormale,
            kantennormale,
            punktnormale,
            kugel,
            suchtiefe,
            gitter,
            punktgitter,
            randkanten,
            volumen,
        })
    }

    /// Aus einem Netz im Rahmen von CLO und `klon3d koerper`: mm, y oben, vorn
    /// +z. Gedreht um +90° um x in den Szenenrahmen (z oben, vorn −y) und in m
    /// umgerechnet: (x, y, z) → (x, −z, y)·10⁻³. Die Drehung erhält die
    /// Orientierung der Dreiecke.
    pub fn aus_clo_mm(punkte_mm: &[[f64; 3]], dreiecke: &[[usize; 3]], suchtiefe: f64) -> Result<Koerpernetz, String> {
        let p = punkte_mm.iter().map(|&q| clo_nach_szene(q)).collect();
        let d = dreiecke.iter().map(|d| d.map(|i| i as u32)).collect();
        Koerpernetz::neu(p, d, suchtiefe)
    }

    /// Die Pseudonormale der Region.
    fn pseudonormale(&self, t: usize, r: Region) -> [f64; 3] {
        match r {
            Region::Flaeche => self.flaechennormale[t],
            Region::Kante(k) => self.kantennormale[t][k as usize],
            Region::Ecke(k) => self.punktnormale[self.dreiecke[t][k as usize] as usize],
        }
    }

    /// Ergebnis für Dreieck t mit nächstem Punkt c in Region r.
    fn ergebnis(&self, x: [f64; 3], t: usize, c: [f64; 3], r: Region) -> Naechster {
        let pn = self.pseudonormale(t, r);
        let d = sub(x, c);
        let l = norm(d);
        let aussen = dot(d, pn) >= 0.0;
        let normale = if l > 1e-12 { mul(d, if aussen { 1.0 / l } else { -1.0 / l }) } else { pn };
        Naechster { abstand: if aussen { l } else { -l }, punkt: c, normale, dreieck: t as u32, region: r }
    }

    /// Nächster Oberflächenpunkt näher als `radius` (ohne Vorzeichen
    /// gemessen); `None`, wenn keiner so nah liegt.
    pub fn naechster(&self, x: [f64; 3], radius: f64) -> Option<Naechster> {
        let g = &self.gitter;
        let r = g.bereich(sub(x, [radius; 3]), add(x, [radius; 3]))?;
        let mut best = radius;
        let mut treffer: Option<(usize, [f64; 3], Region)> = None;
        for kz in r[2].0..=r[2].1 {
            for ky in r[1].0..=r[1].1 {
                for kx in r[0].0..=r[0].1 {
                    let z = g.zellnummer([kx, ky, kz]);
                    for &t in &g.eintraege[g.anfang[z] as usize..g.anfang[z + 1] as usize] {
                        let t = t as usize;
                        let (m, rk) = self.kugel[t];
                        if norm(sub(x, m)) - rk >= best {
                            continue;
                        }
                        let [a, b, c] = self.dreiecke[t].map(|i| self.punkte[i as usize]);
                        let (q, reg) = naechster_auf_dreieck(x, a, b, c);
                        let l = norm(sub(x, q));
                        if l < best {
                            best = l;
                            treffer = Some((t, q, reg));
                        }
                    }
                }
            }
        }
        treffer.map(|(t, q, reg)| self.ergebnis(x, t, q, reg))
    }

    /// Erster Durchstoß der Strecke a → b durch die Oberfläche **von außen nach
    /// innen** (Möller & Trumbore, „Fast, Minimum Storage Ray/Triangle
    /// Intersection“, 1997): Punkt und Flächennormale. Fängt Stoffpunkte, die
    /// in einem Schritt tiefer als die Suchtiefe oder ganz durch ein dünnes Teil
    /// (Arm, Finger) laufen würden; Austritte von innen zählen nicht.
    pub fn durchstoss(&self, a: [f64; 3], b: [f64; 3]) -> Option<([f64; 3], [f64; 3])> {
        let d = sub(b, a);
        if dot(d, d) < 1e-24 {
            return None;
        }
        let mut lo = a;
        let mut hi = a;
        for k in 0..3 {
            lo[k] = lo[k].min(b[k]);
            hi[k] = hi[k].max(b[k]);
        }
        let g = &self.gitter;
        let r = g.bereich(lo, hi)?;
        let mut best: Option<(f64, usize)> = None;
        // Ränder leicht einschließen, damit ein Strahl durch eine gemeinsame
        // Kante nicht beiden Dreiecken entgeht.
        let eps = 1e-12;
        for kz in r[2].0..=r[2].1 {
            for ky in r[1].0..=r[1].1 {
                for kx in r[0].0..=r[0].1 {
                    let z = g.zellnummer([kx, ky, kz]);
                    for &t in &g.eintraege[g.anfang[z] as usize..g.anfang[z + 1] as usize] {
                        let t = t as usize;
                        if dot(d, self.flaechennormale[t]) >= 0.0 {
                            continue;
                        }
                        let [p0, p1, p2] = self.dreiecke[t].map(|i| self.punkte[i as usize]);
                        let (e1, e2) = (sub(p1, p0), sub(p2, p0));
                        let pv = cross(d, e2);
                        let det = dot(e1, pv);
                        if det.abs() < 1e-30 {
                            continue;
                        }
                        let tv = sub(a, p0);
                        let u = dot(tv, pv) / det;
                        if u < -eps || u > 1.0 + eps {
                            continue;
                        }
                        let qv = cross(tv, e1);
                        let v = dot(d, qv) / det;
                        if v < -eps || u + v > 1.0 + eps {
                            continue;
                        }
                        let s = dot(e2, qv) / det;
                        if (-eps..=1.0 + eps).contains(&s) && best.is_none_or(|(sb, _)| s < sb) {
                            best = Some((s, t));
                        }
                    }
                }
            }
        }
        best.map(|(s, t)| (add(a, mul(d, s.clamp(0.0, 1.0))), self.flaechennormale[t]))
    }

    /// **Körperpunkt gegen Stoffdreieck** (CLO:
    /// `bSolidVertexToClothTriangleCollisionHandling`, Befund 49): Ein
    /// Körperpunkt darf nicht durch den Stoff stechen. Für jeden Körperpunkt p
    /// mit Normale n und jedes Stoffdreieck in der Nähe wird die Gerade
    /// p + t·n mit dem Dreieck geschnitten (Möller–Trumbore); liegt der
    /// Schnitt bei −`tief` < t < 0 — das Dreieck ist unter dem Körperpunkt,
    /// der Körper sticht durch die Mittelfläche des Stoffs —, wird das
    /// Dreieck an dieser Stelle auf t = 0 gehoben: Verschiebung −t·n, verteilt nach
    /// Schwerpunktgewichten und inverser Masse wie ein Punkt-Dreieck-Kontakt
    /// bei PBD; die Geschwindigkeit des Schnittpunkts relativ zum Körper in
    /// den Körper hinein entfällt ebenso. Das fängt, was die Stoffpunkte allein
    /// nicht sehen: Kanten, die zwischen zwei Stoffpunkten durch Schulter,
    /// Armansatz oder Fingerspitze schneiden.
    ///
    /// Nur auf t = 0, nicht auf den Kollisionsabstand: Ein ebenes Dreieck aus
    /// 10-mm-Kanten hängt auf einem Arm von 4,5 cm Radius in der Mitte 0,28 mm
    /// durch — mehr als die halbe Dicke. Auf volle Dicke gehoben, verloren
    /// seine Ecken den Kontakt, die Spannung zog sie durch die Haut zurück, und
    /// das wiederholte sich jedes Bild (Bluse3 v2: 390 Hebungen und 500
    /// Durchstöße je Bild in Ruhe).
    ///
    /// `x`, `v`, `w` sind Lage, Geschwindigkeit und inverse Masse der
    /// Stoffpunkte; `versatz` und `geschwindigkeit` die des Körpers. Liefert
    /// die Zahl der Korrekturen.
    #[allow(clippy::too_many_arguments)]
    pub fn punkte_gegen_dreiecke(
        &self,
        tief: f64,
        versatz: [f64; 3],
        geschwindigkeit: [f64; 3],
        dreiecke: &[[u32; 3]],
        w: &[f64],
        x: &mut [[f64; 3]],
        v: &mut [[f64; 3]],
    ) -> usize {
        // Je Dreieck der tiefste Stich; danach ist das Dreieck schon gehoben.
        let mut tiefster: Vec<Option<(f64, [f64; 3], [f64; 3])>> = vec![None; dreiecke.len()];
        // Rundung: was schon gehoben ist, liegt bei t ≈ 0.
        self.stiche(tief, -1e-9, versatz, dreiecke, x, |di, bary, n, t| {
            if tiefster[di].is_none_or(|(d, _, _)| -t > d) {
                tiefster[di] = Some((-t, n, bary));
            }
        });
        let mut zahl = 0;
        for (d, stich) in dreiecke.iter().zip(tiefster) {
            let Some((delta, n, bary)) = stich else { continue };
            let idx = d.map(|i| i as usize);
            let gew = idx.map(|i| w[i]);
            let nenner: f64 = (0..3).map(|j| bary[j] * bary[j] * gew[j]).sum();
            if nenner <= 0.0 {
                continue;
            }
            let vq = sub((0..3).fold([0.0; 3], |s, j| add(s, mul(v[idx[j]], bary[j]))), geschwindigkeit);
            let vn = dot(vq, n).min(0.0);
            for j in 0..3 {
                let f = bary[j] * gew[j] / nenner;
                x[idx[j]] = add(x[idx[j]], mul(n, f * delta));
                v[idx[j]] = sub(v[idx[j]], mul(n, f * vn));
            }
            zahl += 1;
        }
        zahl
    }

    /// Alle Stiche: für jedes Stoffdreieck (Index in `dreiecke`) jeder
    /// Körperpunkt, dessen Gerade p + t·n das Dreieck bei −`tief` < t <
    /// `band` trifft; gemeldet mit Schwerpunktgewichten des Treffpunkts,
    /// Normale und t.
    pub fn stiche(
        &self,
        tief: f64,
        band: f64,
        versatz: [f64; 3],
        dreiecke: &[[u32; 3]],
        x: &[[f64; 3]],
        mut f: impl FnMut(usize, [f64; 3], [f64; 3], f64),
    ) {
        let rand = tief.max(band);
        for (di, d) in dreiecke.iter().enumerate() {
            let idx = d.map(|i| i as usize);
            let [a, b, c] = idx.map(|i| x[i]);
            let mut lo = a;
            let mut hi = a;
            for q in [b, c] {
                for k in 0..3 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
            }
            let lo = sub(sub(lo, versatz), [rand; 3]);
            let hi = add(sub(hi, versatz), [rand; 3]);
            let (e1, e2) = (sub(b, a), sub(c, a));
            self.punktgitter.je_eintrag(lo, hi, |k| {
                let k = k as usize;
                let p = add(self.punkte[k], versatz);
                let n = self.punktnormale[k];
                let pv = cross(n, e2);
                let det = dot(e1, pv);
                if det.abs() < 1e-18 {
                    return;
                }
                let tv = sub(p, a);
                let bu = dot(tv, pv) / det;
                if !(0.0..=1.0).contains(&bu) {
                    return;
                }
                let qv = cross(tv, e1);
                let bv = dot(n, qv) / det;
                if bv < 0.0 || bu + bv > 1.0 {
                    return;
                }
                let t = dot(e2, qv) / det;
                if t > -tief && t < band {
                    f(di, [1.0 - bu - bv, bu, bv], n, t);
                }
            });
        }
    }

    /// Nächster Oberflächenpunkt über alle Dreiecke, ohne Gitter — der
    /// Vergleich für die Tests.
    pub fn naechster_alle(&self, x: [f64; 3]) -> Naechster {
        let mut best = (f64::INFINITY, 0usize, [0.0; 3], Region::Flaeche);
        for (t, d) in self.dreiecke.iter().enumerate() {
            let [a, b, c] = d.map(|i| self.punkte[i as usize]);
            let (q, reg) = naechster_auf_dreieck(x, a, b, c);
            let l = norm(sub(x, q));
            if l < best.0 {
                best = (l, t, q, reg);
            }
        }
        self.ergebnis(x, best.1, best.2, best.3)
    }
}

/// Gitter mit Zellen der Kantenlänge 2·Suchtiefe über dem Hüllquader der
/// Punkte, um die Suchtiefe erweitert; jeder Quader steht in allen Zellen, die
/// er berührt. Eine Abfrage mit Radius ≤ Suchtiefe berührt so höchstens
/// 2×2×2 Zellen.
fn baue_gitter(punkte: &[[f64; 3]], suchtiefe: f64, quader: &[([f64; 3], [f64; 3])]) -> Gitter {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in punkte {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let zelle = 2.0 * suchtiefe;
    let ursprung = sub(lo, [suchtiefe; 3]);
    let mut zahl = [1usize; 3];
    for a in 0..3 {
        zahl[a] = (((hi[a] + suchtiefe - ursprung[a]) / zelle).floor() as usize + 1).max(1);
    }
    let mut g = Gitter { ursprung, zelle, zahl, anfang: Vec::new(), eintraege: Vec::new() };
    let bereiche: Vec<[(usize, usize); 3]> = quader.iter().map(|&(a, b)| g.bereich(a, b).expect("Quader liegt im Gitter")).collect();
    let nz = zahl[0] * zahl[1] * zahl[2];
    let mut zaehler = vec![0u32; nz + 1];
    for r in &bereiche {
        for kz in r[2].0..=r[2].1 {
            for ky in r[1].0..=r[1].1 {
                for kx in r[0].0..=r[0].1 {
                    zaehler[g.zellnummer([kx, ky, kz]) + 1] += 1;
                }
            }
        }
    }
    for z in 0..nz {
        zaehler[z + 1] += zaehler[z];
    }
    let mut frei = zaehler.clone();
    let mut liste = vec![0u32; zaehler[nz] as usize];
    for (t, r) in bereiche.iter().enumerate() {
        for kz in r[2].0..=r[2].1 {
            for ky in r[1].0..=r[1].1 {
                for kx in r[0].0..=r[0].1 {
                    let z = g.zellnummer([kx, ky, kz]);
                    liste[frei[z] as usize] = t as u32;
                    frei[z] += 1;
                }
            }
        }
    }
    g.anfang = zaehler;
    g.eintraege = liste;
    g
}

/// Geschlossener Zylinder (Achse z) als Netz, Normalen nach außen:
/// `segmente` Punkte je Ring, `ringe` Abschnitte zwischen `z_unten` und
/// `z_oben`, Deckel als Fächer. Der Prüffall Röhre rechnet damit gegen den
/// analytischen Zylinder.
pub fn zylinder(radius: f64, z_unten: f64, z_oben: f64, segmente: usize, ringe: usize) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
    let (s, r) = (segmente.max(3), ringe.max(1));
    let mut p = Vec::with_capacity((r + 1) * s + 2);
    for i in 0..=r {
        let z = z_unten + (z_oben - z_unten) * i as f64 / r as f64;
        for j in 0..s {
            let w = 2.0 * std::f64::consts::PI * j as f64 / s as f64;
            p.push([radius * w.cos(), radius * w.sin(), z]);
        }
    }
    let unten = p.len() as u32;
    p.push([0.0, 0.0, z_unten]);
    let oben = p.len() as u32;
    p.push([0.0, 0.0, z_oben]);
    let id = |i: usize, j: usize| (i * s + j % s) as u32;
    let mut d = Vec::with_capacity(2 * r * s + 2 * s);
    for i in 0..r {
        for j in 0..s {
            let (a, b, c, e) = (id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j));
            d.push([a, b, c]);
            d.push([a, c, e]);
        }
    }
    for j in 0..s {
        d.push([unten, id(0, j + 1), id(0, j)]);
        d.push([oben, id(r, j), id(r, j + 1)]);
    }
    (p, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Würfel [−h, h]³, Normalen nach außen.
    fn wuerfel(h: f64) -> Koerpernetz {
        let p: Vec<[f64; 3]> = (0..8)
            .map(|i| [if i & 1 == 0 { -h } else { h }, if i & 2 == 0 { -h } else { h }, if i & 4 == 0 { -h } else { h }])
            .collect();
        // je Seite zwei Dreiecke, gegen den Uhrzeigersinn von außen
        let d = vec![
            [0, 2, 3], [0, 3, 1], // z = −h
            [4, 5, 7], [4, 7, 6], // z = +h
            [0, 1, 5], [0, 5, 4], // y = −h
            [2, 6, 7], [2, 7, 3], // y = +h
            [0, 4, 6], [0, 6, 2], // x = −h
            [1, 3, 7], [1, 7, 5], // x = +h
        ];
        Koerpernetz::neu(p, d, 0.5).unwrap()
    }

    /// Vorzeichenbehafteter Abstand zum Würfel, analytisch.
    fn abstand_wuerfel(x: [f64; 3], h: f64) -> f64 {
        let q = x.map(|c| c.abs() - h);
        let aussen = norm(q.map(|c| c.max(0.0)));
        aussen + q[0].max(q[1]).max(q[2]).min(0.0)
    }

    /// Zufallszahlen ohne Abhängigkeit (xorshift).
    fn zufall(s: &mut u64) -> f64 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        (*s >> 11) as f64 / (1u64 << 53) as f64
    }

    #[test]
    fn wuerfel_abstand_exakt_auch_an_kanten_und_ecken() {
        let w = wuerfel(1.0);
        assert_eq!(w.randkanten, 0);
        assert!((w.volumen - 8.0).abs() < 1e-12);
        let mut s = 0x1234_5678_9abc_def1u64;
        for _ in 0..20_000 {
            let x = [0, 1, 2].map(|_| 3.0 * zufall(&mut s) - 1.5);
            let n = w.naechster_alle(x);
            let soll = abstand_wuerfel(x, 1.0);
            assert!((n.abstand - soll).abs() < 1e-12, "{x:?}: {} gegen {soll}", n.abstand);
            // Normale zeigt nach außen und führt auf den nächsten Punkt.
            let zurueck = sub(x, mul(n.normale, n.abstand));
            assert!(norm(sub(zurueck, n.punkt)) < 1e-9, "{x:?}");
        }
        // vor einer Ecke und vor einer Kante: Ecken- bzw. Kantenregion
        let n = w.naechster_alle([2.0, 2.0, 2.0]);
        assert!(matches!(n.region, Region::Ecke(_)), "{n:?}");
        assert!((n.abstand - 3f64.sqrt()).abs() < 1e-12);
        let n = w.naechster_alle([1.5, 0.3, 1.5]);
        assert!(matches!(n.region, Region::Kante(_)), "{n:?}");
        assert!((n.abstand - 0.5f64.sqrt()).abs() < 1e-12);
        // innen nahe einer Seite
        let n = w.naechster_alle([0.9, 0.95, 0.0]);
        assert!((n.abstand + 0.05).abs() < 1e-12);
    }

    /// Mit Gitter dasselbe wie über alle Dreiecke, für Punkte innerhalb der
    /// Suchtiefe; weiter weg nichts.
    #[test]
    fn gitter_wie_alle_dreiecke() {
        let (p, d) = zylinder(0.09, -0.3, 0.1, 96, 20);
        let k = Koerpernetz::neu(p, d, 0.01).unwrap();
        assert_eq!(k.randkanten, 0);
        let mut s = 0xfeed_beef_dead_cafeu64;
        let (mut innen, mut aussen, mut weit) = (0, 0, 0);
        for _ in 0..20_000 {
            let r = 0.07 + 0.04 * zufall(&mut s);
            let w = 2.0 * std::f64::consts::PI * zufall(&mut s);
            let x = [r * w.cos(), r * w.sin(), -0.32 + 0.44 * zufall(&mut s)];
            let alle = k.naechster_alle(x);
            match k.naechster(x, k.suchtiefe) {
                Some(n) => {
                    assert!((n.abstand - alle.abstand).abs() < 1e-12, "{x:?}");
                    assert!(norm(sub(n.punkt, alle.punkt)) < 1e-12, "{x:?}");
                    if n.abstand < 0.0 { innen += 1 } else { aussen += 1 }
                }
                None => {
                    assert!(alle.abstand.abs() >= k.suchtiefe, "{x:?}: {} übersehen", alle.abstand);
                    weit += 1;
                }
            }
        }
        assert!(innen > 1000 && aussen > 1000 && weit > 1000, "{innen} {aussen} {weit}");
    }

    /// Durchstoß von außen am nächsten Eintritt; von innen nach außen nichts;
    /// ganz durch eine dünne Platte: der Eintritt oben.
    #[test]
    fn durchstoss_von_aussen() {
        let w = wuerfel(1.0);
        let (p, n) = w.durchstoss([0.2, 0.3, 3.0], [0.2, 0.3, -3.0]).unwrap();
        assert!(norm(sub(p, [0.2, 0.3, 1.0])) < 1e-12 && (n[2] - 1.0).abs() < 1e-12, "{p:?} {n:?}");
        assert!(w.durchstoss([0.2, 0.3, 0.0], [0.2, 0.3, -3.0]).is_none());
        assert!(w.durchstoss([0.2, 0.3, 3.0], [0.2, 0.3, 1.5]).is_none());
        // genau durch die Diagonale der Oberseite, die Kante ihrer zwei Dreiecke
        let (p, _) = w.durchstoss([0.5, 0.5, 2.0], [0.5, 0.5, 0.0]).unwrap();
        assert!((p[2] - 1.0).abs() < 1e-12);
        // dünne Platte 2 mm: ganz hindurch, Eintritt oben
        let (q, d) = zylinder(0.05, -0.001, 0.001, 64, 1);
        let platte = Koerpernetz::neu(q, d, 0.01).unwrap();
        let (p, n) = platte.durchstoss([0.01, 0.0, 0.05], [0.01, 0.0, -0.05]).unwrap();
        assert!((p[2] - 0.001).abs() < 1e-12 && n[2] > 0.999);
    }

    /// Eine Spitze (Tetraeder, Spitze 1 cm über der Stoffebene) sticht durch
    /// ein großes Stoffdreieck, dessen Ecken weit außerhalb liegen: Die
    /// Stoffpunkte allein sehen nichts, Körperpunkt gegen Stoffdreieck hebt das
    /// Dreieck über die Spitze.
    #[test]
    fn spitze_sticht_nicht_durch() {
        let p = vec![[-0.02, -0.02, -0.03], [0.02, -0.02, -0.03], [0.0, 0.03, -0.03], [0.0, 0.0, 0.01]];
        let d = vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]];
        let k = Koerpernetz::neu(p, d, 0.02).unwrap();
        let mut x = vec![[-0.3, -0.3, 0.0], [0.3, -0.3, 0.0], [0.0, 0.4, 0.0]];
        let mut v = vec![[0.0, 0.0, -1.0]; 3];
        for &q in &x {
            assert!(k.naechster_alle(q).abstand > 0.0, "Stoffpunkt {q:?} liegt außen");
        }
        let n = k.punkte_gegen_dreiecke(0.02, [0.0; 3], [0.0; 3], &[[0, 1, 2]], &[1.0; 3], &mut x, &mut v);
        assert_eq!(n, 1);
        // Die Mittelfläche des Stoffs geht jetzt durch die Spitze, nicht mehr
        // unter ihr; die Ecken heben sich nach ihren Schwerpunktgewichten.
        let (a, b, c) = (x[0], x[1], x[2]);
        let nrm = normiert(cross(sub(b, a), sub(c, a)));
        let hoehe = dot(sub(a, [0.0, 0.0, 0.01]), nrm);
        assert!(hoehe.abs() < 1e-12, "{hoehe} {x:?}");
        assert!(x.iter().all(|q| q[2] > 0.0), "{x:?}");
        // Keine Geschwindigkeit mehr in die Spitze hinein, am Schnittpunkt der
        // Geraden durch die Spitze längs ihrer Normale.
        let ns = k.punktnormale[3];
        let (e1, e2) = (sub(b, a), sub(c, a));
        let pv = cross(ns, e2);
        let det = dot(e1, pv);
        let tv = sub([0.0, 0.0, 0.01], a);
        let bu = dot(tv, pv) / det;
        let bv = dot(ns, cross(tv, e1)) / det;
        let vq = add(add(mul(v[0], 1.0 - bu - bv), mul(v[1], bu)), mul(v[2], bv));
        assert!(dot(vq, ns) >= -1e-12, "{vq:?}");
        // Ein zweiter Durchgang findet nichts mehr.
        let n2 = k.punkte_gegen_dreiecke(0.02, [0.0; 3], [0.0; 3], &[[0, 1, 2]], &[1.0; 3], &mut x, &mut v);
        assert_eq!(n2, 0);
    }

    /// Falsch orientiert, nicht mannigfaltig oder nach innen gekehrt: Fehler.
    #[test]
    fn fehlerhafte_netze_abgelehnt() {
        let w = wuerfel(1.0);
        let mut d = w.dreiecke.clone();
        d[0] = [d[0][0], d[0][2], d[0][1]];
        assert!(Koerpernetz::neu(w.punkte.clone(), d, 0.1).unwrap_err().contains("orientiert"));
        let innen: Vec<[u32; 3]> = w.dreiecke.iter().map(|t| [t[0], t[2], t[1]]).collect();
        assert!(Koerpernetz::neu(w.punkte.clone(), innen, 0.1).unwrap_err().contains("nach innen"));
        let mut drei = w.dreiecke.clone();
        drei.push(w.dreiecke[0]);
        assert!(Koerpernetz::neu(w.punkte.clone(), drei, 0.1).is_err());
    }

    /// Der Rahmen von CLO wird gedreht, nicht gespiegelt: Volumen bleibt
    /// positiv, y oben wird z oben, vorn +z wird −y.
    #[test]
    fn clo_rahmen() {
        let (p, d) = zylinder(90.0, 0.0, 400.0, 64, 4);
        // Zylinder mit Achse y statt z (mm): (x, y, z) → (x, z, −y) dreht −90° um x
        let p_clo: Vec<[f64; 3]> = p.iter().map(|q| [q[0], q[2], -q[1]]).collect();
        let d_clo: Vec<[usize; 3]> = d.iter().map(|t| t.map(|i| i as usize)).collect();
        let k = Koerpernetz::aus_clo_mm(&p_clo, &d_clo, 0.01).unwrap();
        assert!(k.volumen > 0.0);
        let zmax = k.punkte.iter().map(|q| q[2]).fold(f64::MIN, f64::max);
        assert!((zmax - 0.4).abs() < 1e-12);
        // 5 mm vor dem Mantel in CLOs Rahmen (vorn +z, Höhe y = 200 mm) liegt
        // im Szenenrahmen bei −y, auf Höhe z = 0,2 m.
        let vorn = [0.0, 200.0, 95.0];
        let x = [vorn[0] * 1e-3, -vorn[2] * 1e-3, vorn[1] * 1e-3];
        let n = k.naechster(x, 0.01).unwrap();
        assert!((n.abstand - 0.005).abs() < 1e-12 && n.normale[1] < -0.999, "{n:?}");
    }
}
