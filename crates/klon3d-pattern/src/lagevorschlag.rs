//! Lagevorschlag: die Lagedatei (`<muster>.lage`, gelesen von
//! `klon3d_sim::platzierung::lies_lage`) aus dem Muster und seiner Nahtliste,
//! ohne Handarbeit.
//!
//! Die Platzierung legt jedes Teil längentreu auf einen Hüllkörper:
//! Muster-x wird Bogenlänge um Rumpf, Hals oder Arm, Muster-y die Höhe. Der
//! Vorschlag rechnet deshalb in einer **Abwicklung** (cm): x die Bogenlänge ab
//! einem Ursprung — vordere oder hintere Mitte am Rumpf, hintere Mitte am
//! Hals, „oben“ am Arm —, y die Höhe nach oben. Je Teil eine Lage
//! `V = R(φ)·(s·x, −y) + t` (s = −1 für von innen gezeichnet, φ Drehung).
//!
//! 1. **Zone** nach dem Teilnamen: Ärmel und Manschette am Arm (Seite nach
//!    dem Namen, sonst nach dem Armloch), Kragen am Hals, alles andere am
//!    Rumpf. Teile ohne Naht (Auflagen, Hilfsteile) bleiben weg.
//! 2. **Spiegelung** aus dem Nahtgraphen (siehe
//!    [`crate::vorschlag::verdrehte_naehte`]).
//! 3. **Rahmen:** Rumpfteile mit gemeinsamen Rechenpunkten und gleicher
//!    Spiegelung sind im selben Rahmen gezeichnet und liegen zueinander wie
//!    im Muster.
//! 4. **Verankerte Rahmen** tragen „vorn“ oder „hinten“ im Namen und
//!    reichen über die Mitte (beide Seiten oder ein Mittelteil) oder haben eine
//!    Mittelnaht (linkes an rechtes Teil derselben Zone). Ihre Mitte kommt an
//!    die vordere bzw. hintere Mitte: die Mittelnaht, sonst die Mitte ihrer
//!    Breite; die Mittelnaht wird senkrecht gestellt (die Hinterbahnen des
//!    Rocks sind um 67° gedreht gezeichnet). Nicht verankert wird ein Rahmen,
//!    der waagerecht an Teile vorn **und** hinten genäht ist (Bund).
//!    Halbe Vorder- oder Rückenteile ohne Mittelnaht (offene Front des
//!    Bolero) hängen an ihrer Schulternaht: das Halsende so weit von der
//!    eigenen Mitte wie das des Gegenstücks von dessen Mitte.
//! 5. **Höhe:** Das höchste Ende der Schulternähte (waagerecht zwischen vorn
//!    und hinten, oben im Teil) je Körperseite liegt am seitlichen Halspunkt,
//!    das tiefste ist der Schulterpunkt. Ohne Schulternaht die Taille aus den
//!    Maßen des Musters (`height_neck_side − height_waist_side`): bei Röcken,
//!    Hosen und Bünden am oberen Ende der Seitennähte, sonst am unteren
//!    (Bustier). Gruppen ohne eigene Höhe folgen über die Seitennaht.
//! 6. **Angelegte Rahmen:** Die übrigen folgen über ihre Nähte an schon
//!    liegende Teile, so gedreht und verschoben, dass die Nähte decken
//!    (kleinste Quadrate über alle Nähte zu einer Gruppe), höchstens 90°
//!    gedreht, mit den Teilen beidseits der Naht und der kleinsten
//!    Abweichung je Spanne zuerst (Obercup an Untercup, Bund über dem Rock,
//!    Manschette unter dem Ärmel).
//! 7. **Arm:** Der Ärmel hängt mit dem Punkt gegenüber dem Schulterpunkt am
//!    Schulterpunkt, „oben“, Höhe 0 (ohne Schulternaht: gegenüber der Stelle,
//!    an der Vorder- und Rückenteil im Armloch aneinanderstoßen);
//!    Manschetten folgen über ihre Naht.
//! 8. **Hals:** Der Kragen hängt mit dem Punkt gegenüber der hinteren Mitte
//!    des Halsausschnitts an der hinteren Mitte, in deren Höhe.
//! 9. **Dicke**, wenn der Teilname genau eine in mm nennt (Rockbund „1 mm“).
//! 10. **Abstand:** am Arm und an Rumpfteilen, die an ein Armteil genäht sind
//!    (Armloch, Seitennaht mit der Ärmelnaht), 0,5 cm — die Arme hängen eng am
//!    Rumpf (Befund 95; Bluse3 98,6 statt 97,8 % der Nahtpaare); am Hals 3 cm,
//!    sonst der der Datei (2 cm).

use std::collections::HashMap;

use crate::geom::{Auswertung, Pt};
use crate::kanten::{Kurvenquelle, Teilrand};
use crate::naehte::{pruefe_naehte, NahtFehler, Nahtdef, Nahtseite};
use crate::vorschlag::{seitenwort, zonenwort};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    Rumpf,
    Hals,
    Arm,
}

fn zone_nach_name(name: &str) -> Zone {
    let n = name.to_lowercase();
    if ["ärmel", "aermel", "armel", "sleeve", "manschette", "cuff"].iter().any(|w| n.contains(w)) {
        Zone::Arm
    } else if ["kragen", "collar"].iter().any(|w| n.contains(w)) {
        Zone::Hals
    } else {
        Zone::Rumpf
    }
}

/// Dicke der Folie, wenn der Teilname genau eine in mm nennt („1 BUND - 1 mm
/// einlagig“ → 1,0).
fn dicke_nach_name(name: &str) -> Option<f64> {
    let n = name.to_lowercase();
    let b = n.as_bytes();
    let mut treffer = Vec::new();
    for (i, _) in n.match_indices("mm") {
        // Zahl davor, höchstens ein Leerzeichen dazwischen.
        let mut e = i;
        if e > 0 && b[e - 1] == b' ' {
            e -= 1;
        }
        let mut a = e;
        while a > 0 && (b[a - 1].is_ascii_digit() || b[a - 1] == b',' || b[a - 1] == b'.') {
            a -= 1;
        }
        let davor_frei = a == 0 || !b[a - 1].is_ascii_alphanumeric();
        let danach_frei = i + 2 >= b.len() || !b[i + 2].is_ascii_alphanumeric();
        if a < e && davor_frei && danach_frei {
            if let Ok(x) = n[a..e].replace(',', ".").parse::<f64>() {
                treffer.push(x);
            }
        }
    }
    match treffer[..] {
        [x] if x > 0.0 && x < 10.0 => Some(x),
        _ => None,
    }
}

/// Unterteil (Taille oben an den Seitennähten) nach den Teilnamen.
fn unterteil(namen: &[&str]) -> bool {
    let w = ["rock", "skirt", "hose", "trouser", "pant", "short", "legging", "slip", "bund", "waistband", "culotte"];
    namen.iter().any(|n| {
        let n = n.to_lowercase();
        w.iter().any(|x| n.contains(x))
    })
}

/// Lage eines Teils in der Abwicklung (cm): `V = R(phi)·(s·x, −y) + t`.
#[derive(Debug, Clone, Copy)]
struct Pose {
    s: f64,
    phi: f64,
    t: Pt,
}

impl Pose {
    fn dreh(&self, a: Pt) -> Pt {
        let (sn, c) = self.phi.sin_cos();
        Pt::new(c * a.x - sn * a.y, sn * a.x + c * a.y)
    }
    /// Musterpunkt (cm) in die Abwicklung.
    fn an(&self, q: Pt) -> Pt {
        let a = self.dreh(Pt::new(self.s * q.x, -q.y));
        Pt::new(a.x + self.t.x, a.y + self.t.y)
    }
    /// Richtung im Muster in die Abwicklung.
    fn richtung(&self, v: Pt) -> Pt {
        self.dreh(Pt::new(self.s * v.x, -v.y))
    }
}

/// Drehung φ und Verschiebung t mit `R(φ)·a + t ≈ b` (kleinste Quadrate) und
/// die mittlere Abweichung.
fn procrustes(a: &[Pt], b: &[Pt]) -> (f64, Pt, f64) {
    let n = a.len().max(1) as f64;
    let ma = Pt::new(a.iter().map(|p| p.x).sum::<f64>() / n, a.iter().map(|p| p.y).sum::<f64>() / n);
    let mb = Pt::new(b.iter().map(|p| p.x).sum::<f64>() / n, b.iter().map(|p| p.y).sum::<f64>() / n);
    let (mut sc, mut ss) = (0.0, 0.0);
    for (p, q) in a.iter().zip(b) {
        let (u, v) = (Pt::new(p.x - ma.x, p.y - ma.y), Pt::new(q.x - mb.x, q.y - mb.y));
        sc += u.x * v.x + u.y * v.y;
        ss += u.x * v.y - u.y * v.x;
    }
    let phi = ss.atan2(sc);
    let (sn, c) = phi.sin_cos();
    let t = Pt::new(mb.x - (c * ma.x - sn * ma.y), mb.y - (sn * ma.x + c * ma.y));
    let rms = (a
        .iter()
        .zip(b)
        .map(|(p, q)| {
            let r = Pt::new(c * p.x - sn * p.y + t.x, sn * p.x + c * p.y + t.y);
            r.dist(*q).powi(2)
        })
        .sum::<f64>()
        / n)
        .sqrt();
    (phi, t, rms)
}

/// Eine Nahtseite als Streckenzug in cm: je Strecke Teil, Anfang, Ende,
/// Innennormale (im Muster) und Bogenlänge am Anfang.
struct Zug {
    strecken: Vec<(usize, Pt, Pt, Pt, f64)>,
    laenge: f64,
    /// Bogenlänge an den Grenzen der Teilstücke (innen) mit den beiden Teilen.
    fugen: Vec<(f64, usize, usize)>,
}

/// Probe auf einem Zug: Teil, Punkt (cm, Muster), Innennormale (Muster).
#[derive(Debug, Clone, Copy)]
struct Probe {
    teil: usize,
    p: Pt,
    n: Pt,
}

impl Zug {
    fn neu(seite: &Nahtseite, idx: &HashMap<String, usize>, raender: &[Teilrand], sinn: &[f64]) -> Zug {
        let mut strecken = Vec::new();
        let mut fugen = Vec::new();
        let mut s = 0.0;
        let mut vorher: Option<usize> = None;
        for t in &seite.teilstuecke {
            let ti = idx[&t.teil];
            if let Some(v) = vorher {
                fugen.push((s, v, ti));
            }
            vorher = Some(ti);
            let r = &raender[ti];
            let k = r.in_cm(1.0);
            for &e in &t.kanten {
                let mut pkt: Vec<Pt> = r.kanten[e].punkte.iter().map(|p| Pt::new(p.x * k, p.y * k)).collect();
                if t.rueckwaerts {
                    pkt.reverse();
                }
                for w in pkt.windows(2) {
                    let l = w[0].dist(w[1]);
                    if l < 1e-9 {
                        continue;
                    }
                    // Innen liegt links der Vorwärtsrichtung bei positiver Fläche.
                    let v = if t.rueckwaerts { Pt::new(w[0].x - w[1].x, w[0].y - w[1].y) } else { Pt::new(w[1].x - w[0].x, w[1].y - w[0].y) };
                    let n = Pt::new(-v.y / l * sinn[ti], v.x / l * sinn[ti]);
                    strecken.push((ti, w[0], w[1], n, s));
                    s += l;
                }
            }
        }
        Zug { strecken, laenge: s, fugen }
    }

    /// Probe beim Anteil `f` der Länge.
    fn bei(&self, f: f64) -> Option<Probe> {
        let ziel = f.clamp(0.0, 1.0) * self.laenge;
        let i = self.strecken.iter().rposition(|st| st.4 <= ziel + 1e-12).unwrap_or(0);
        let &(teil, a, b, n, s0) = self.strecken.get(i)?;
        let l = a.dist(b).max(1e-12);
        let u = ((ziel - s0) / l).clamp(0.0, 1.0);
        Some(Probe { teil, p: Pt::new(a.x + u * (b.x - a.x), a.y + u * (b.y - a.y)), n })
    }
}

fn wurzel(e: &mut [usize], x: usize) -> usize {
    let mut r = x;
    while e[r] != r {
        r = e[r];
    }
    let mut y = x;
    while e[y] != r {
        let z = e[y];
        e[y] = r;
        y = z;
    }
    r
}

fn zahl(x: f64) -> String {
    format!("{x:+.2}").replace('.', ",")
}

/// Der Vorschlag: Text der Lagedatei und Hinweise, was unsicher blieb.
#[derive(Debug, Clone)]
pub struct Lagevorschlag {
    pub text: String,
    pub hinweise: Vec<String>,
}

/// Schlägt die Lagedatei vor (siehe Moduldoku). `gespiegelt`: Teile, die von
/// innen gezeichnet sind (volle Namen oder der Teil nach dem letzten „ - “),
/// etwa [`crate::vorschlag::Vorschlag::gespiegelt`].
pub fn schlage_lage_vor(
    ev: &Auswertung<'_>,
    quelle: Kurvenquelle,
    defs: &[Nahtdef],
    gespiegelt: &[String],
    kopf: &str,
) -> Result<Lagevorschlag, NahtFehler> {
    let bericht = pruefe_naehte(ev, defs, quelle)?;
    let raender: Vec<Teilrand> = ev.muster().pieces().into_iter().filter_map(|t| ev.teilrand(t, quelle).ok()).collect();
    let n = raender.len();
    let idx: HashMap<String, usize> = raender.iter().enumerate().map(|(i, r)| (r.teil.clone(), i)).collect();
    let sinn: Vec<f64> = raender
        .iter()
        .map(|r| {
            let u = &r.umriss;
            (0..u.len()).map(|k| u[k].x * u[(k + 1) % u.len()].y - u[(k + 1) % u.len()].x * u[k].y).sum::<f64>().signum()
        })
        .collect();
    let gleich = |voll: &str, kurz: &str| voll == kurz || voll.ends_with(&format!(" - {kurz}"));
    let s: Vec<f64> = raender.iter().map(|r| if gespiegelt.iter().any(|g| gleich(&r.teil, g)) { -1.0 } else { 1.0 }).collect();
    let zuege: Vec<(Zug, Zug)> = bericht.naehte.iter().map(|e| (Zug::neu(&e.a, &idx, &raender, &sinn), Zug::neu(&e.b, &idx, &raender, &sinn))).collect();
    let mut in_naht = vec![false; n];
    for (a, b) in &zuege {
        for st in a.strecken.iter().chain(&b.strecken) {
            in_naht[st.0] = true;
        }
    }
    let zone: Vec<Zone> = raender.iter().map(|r| zone_nach_name(&r.teil)).collect();
    let zw: Vec<Option<bool>> = raender.iter().map(|r| zonenwort(&r.teil)).collect();
    let mut sw: Vec<Option<bool>> = raender.iter().map(|r| seitenwort(&r.teil)).collect();
    let punkte_cm = |t: usize| -> Vec<Pt> {
        let k = raender[t].in_cm(1.0);
        raender[t].umriss.iter().map(|p| Pt::new(p.x * k, p.y * k)).collect()
    };
    let knoten_cm = |t: usize, i: usize| -> Pt {
        let r = &raender[t];
        let k = r.in_cm(1.0);
        let p = r.umriss[r.punktknoten[i].stelle];
        Pt::new(p.x * k, p.y * k)
    };
    // Paare gleicher Anteile beider Seiten einer Naht.
    let paare = |j: usize| -> Vec<(Probe, Probe)> {
        let (a, b) = &zuege[j];
        let m = (a.laenge as usize).clamp(8, 200);
        (0..m).filter_map(|i| Some((a.bei((i as f64 + 0.5) / m as f64)?, b.bei((i as f64 + 0.5) / m as f64)?))).collect()
    };
    let mut hinweise = Vec::new();


    // 3. Rahmen: Teile mit gemeinsamen Rechenpunkten, gleicher Spiegelung und
    // verträglicher Zone (nie vorn mit hinten — die Rockbahnen sind als
    // Fächer um die Seitennähte gezeichnet und teilen dort Punkte). Die
    // engsten Paare zuerst.
    let mut eltern: Vec<usize> = (0..n).collect();
    let mut rzone: Vec<Option<bool>> = zw.clone();
    let mut paarliste: Vec<(usize, usize, usize)> = Vec::new();
    for p in 0..n {
        for q in p + 1..n {
            if zone[p] != Zone::Rumpf || zone[q] != Zone::Rumpf || !in_naht[p] || !in_naht[q] || s[p] != s[q] {
                continue;
            }
            let gemein = raender[p].punktknoten.iter().filter(|a| raender[q].punktknoten.iter().any(|b| a.id == b.id)).count();
            if gemein > 0 {
                paarliste.push((gemein, p, q));
            }
        }
    }
    paarliste.sort_by(|a, b| b.0.cmp(&a.0));
    for &(_, p, q) in &paarliste {
        let (rp, rq) = (wurzel(&mut eltern, p), wurzel(&mut eltern, q));
        if rp == rq || matches!((rzone[rp], rzone[rq]), (Some(x), Some(y)) if x != y) {
            continue;
        }
        eltern[rq] = rp;
        rzone[rp] = rzone[rp].or(rzone[rq]);
    }
    let rahmen: Vec<usize> = (0..n).map(|t| wurzel(&mut eltern, t)).collect();
    let glieder = |f: usize| -> Vec<usize> { (0..n).filter(|&t| rahmen[t] == f && in_naht[t] && zone[t] == Zone::Rumpf).collect() };
    let mut alle_rahmen: Vec<usize> = (0..n).filter(|&t| in_naht[t] && zone[t] == Zone::Rumpf).map(|t| rahmen[t]).collect();
    alle_rahmen.sort();
    alle_rahmen.dedup();
    let rahmen_zone: HashMap<usize, Option<bool>> = alle_rahmen.iter().map(|&f| (f, rzone[f])).collect();
    // Zone einer Nahtseite am Rumpf nach den Rahmen ihrer Teile.
    let seitenzone = |z: &Zug| -> Option<bool> {
        if !z.strecken.iter().all(|st| zone[st.0] == Zone::Rumpf) {
            return None;
        }
        let v: Vec<bool> = z.strecken.iter().filter_map(|st| rahmen_zone.get(&rahmen[st.0]).copied().flatten()).collect();
        (!v.is_empty() && v.iter().all(|&x| x == v[0])).then(|| v[0])
    };

    // Bund (waagerecht an Teile vorn und hinten genäht), Mittelnähte.
    let mut ring: HashMap<usize, bool> = HashMap::new();
    let mut mittelnaehte: Vec<(usize, usize, usize)> = Vec::new(); // (Naht, Rahmen a, Rahmen b)
    for (j, (a, b)) in zuege.iter().enumerate() {
        for (x, y) in [(a, b), (b, a)] {
            let zonen_y: Vec<bool> = y.strecken.iter().filter_map(|st| zw[st.0]).collect();
            if zonen_y.contains(&true) && zonen_y.contains(&false) {
                let (p0, p1) = (x.strecken.first().map(|st| st.1), x.strecken.last().map(|st| st.2));
                if let (Some(p0), Some(p1)) = (p0, p1) {
                    if (p1.y - p0.y).abs() < (p1.x - p0.x).abs() {
                        for st in &x.strecken {
                            if zone[st.0] == Zone::Rumpf {
                                ring.insert(rahmen[st.0], true);
                            }
                        }
                    }
                }
            }
        }
        let einheitlich = |z: &Zug| -> Option<(bool, bool, usize)> {
            let t0 = z.strecken.first()?.0;
            let ok = z.strecken.iter().all(|st| rahmen[st.0] == rahmen[t0] && zw[st.0] == zw[t0] && sw[st.0] == sw[t0]);
            (ok && zone[t0] == Zone::Rumpf).then_some(())?;
            Some((zw[t0]?, sw[t0]?, rahmen[t0]))
        };
        if let (Some((za, sa, fa)), Some((zb, sb, fb))) = (einheitlich(a), einheitlich(b)) {
            if za == zb && sa != sb {
                mittelnaehte.push((j, fa, fb));
            }
        }
    }

    // 4. Verankerte Rahmen. Gruppe je Teil: (Ursprung vorn = true / hinten =
    // false, Kennung des Ankerrahmens).
    let mut pose: Vec<Option<Pose>> = vec![None; n];
    let mut gruppe: Vec<Option<(bool, usize)>> = vec![None; n];
    let breite_mitte = |g: &[usize], ps: &Pose| -> f64 {
        let xs: Vec<f64> = g.iter().flat_map(|&t| punkte_cm(t)).map(|p| ps.an(p).x).collect();
        (xs.iter().cloned().fold(f64::INFINITY, f64::min) + xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) / 2.0
    };
    for &f in &alle_rahmen {
        let g = glieder(f);
        let Some(z) = rahmen_zone[&f] else { continue };
        if ring.get(&f).copied().unwrap_or(false) {
            continue;
        }
        let beidseits = g.iter().any(|&t| sw[t].is_none()) || (g.iter().any(|&t| sw[t] == Some(true)) && g.iter().any(|&t| sw[t] == Some(false)));
        let mn: Vec<&(usize, usize, usize)> = mittelnaehte.iter().filter(|m| m.1 == f || m.2 == f).collect();
        if !beidseits && mn.is_empty() {
            continue;
        }
        let mut ps = Pose { s: s[g[0]], phi: 0.0, t: Pt::new(0.0, 0.0) };
        // Längste Mittelnaht: ihre Seite in diesem Rahmen senkrecht.
        let laengste = mn.iter().max_by(|x, y| zuege[x.0].0.laenge.total_cmp(&zuege[y.0].0.laenge));
        let mut mitte_x: Option<Vec<Pt>> = None;
        if let Some(m) = laengste {
            let zug = if m.1 == f { &zuege[m.0].0 } else { &zuege[m.0].1 };
            if let (Some(a), Some(b)) = (zug.bei(0.0), zug.bei(1.0)) {
                let c = ps.richtung(Pt::new(b.p.x - a.p.x, b.p.y - a.p.y));
                let w = c.y.atan2(c.x);
                let norm = |x: f64| (x + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI;
                let (u, o) = (norm(-std::f64::consts::FRAC_PI_2 - w), norm(std::f64::consts::FRAC_PI_2 - w));
                ps.phi = if u.abs() <= o.abs() { u } else { o };
                mitte_x = Some((0..=20).filter_map(|i| zug.bei(i as f64 / 20.0)).map(|p| p.p).collect());
            }
        }
        let x0 = match mitte_x {
            Some(v) => v.iter().map(|&p| ps.an(p).x).sum::<f64>() / v.len().max(1) as f64,
            None => breite_mitte(&g, &ps),
        };
        ps.t = Pt::new(-x0, 0.0);
        for &t in &g {
            pose[t] = Some(ps);
            gruppe[t] = Some((z, f));
        }
    }
    // Halbe Vorder- oder Rückenteile ohne Mittelnaht (offene Front des
    // Bolero) an ihrer Schulternaht: Das Halsende liegt so weit von der
    // eigenen Mitte wie das des Gegenstücks von dessen Mitte, in Höhe des
    // seitlichen Halspunkts.
    for &f in &alle_rahmen {
        let g = glieder(f);
        let Some(z) = rahmen_zone[&f] else { continue };
        if pose[g[0]].is_some() || ring.get(&f).copied().unwrap_or(false) {
            continue;
        }
        let mut wahl: Option<(f64, Pt, f64)> = None; // (Länge, Halsende im Muster, x des Gegenstücks)
        for (a, b) in &zuege {
            for (x, y) in [(a, b), (b, a)] {
                if !x.strecken.iter().all(|st| rahmen[st.0] == f) || seitenzone(y) != Some(!z) {
                    continue;
                }
                let (Some(x0), Some(x1), Some(y0), Some(y1)) = (x.bei(0.0), x.bei(1.0), y.bei(0.0), y.bei(1.0)) else { continue };
                let (Some(p0), Some(p1)) = (pose[y0.teil], pose[y1.teil]) else { continue };
                if (x1.p.y - x0.p.y).abs() >= (x1.p.x - x0.p.x).abs() {
                    continue;
                }
                let ys: Vec<f64> = g.iter().flat_map(|&t| punkte_cm(t)).map(|p| p.y).collect();
                let mitte = (ys.iter().cloned().fold(f64::INFINITY, f64::min) + ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) / 2.0;
                if (x0.p.y + x1.p.y) / 2.0 > mitte {
                    continue;
                }
                let (hx, hy) = if x0.p.y <= x1.p.y { (x0.p, p0.an(y0.p).x) } else { (x1.p, p1.an(y1.p).x) };
                if wahl.is_none_or(|w| x.laenge > w.0) {
                    wahl = Some((x.laenge, hx, hy));
                }
            }
        }
        let Some((_, hals, gegen_x)) = wahl else { continue };
        let mut ps = Pose { s: s[g[0]], phi: 0.0, t: Pt::new(0.0, 0.0) };
        let v = ps.an(hals);
        ps.t = Pt::new(-gegen_x - v.x, -v.y);
        for &t in &g {
            pose[t] = Some(ps);
            gruppe[t] = Some((z, f));
        }
    }

    // 5. Höhe je Gruppe (vor dem Anlegen, damit die Ziele absolut sind).
    let mut hoehe: HashMap<usize, Vec<f64>> = HashMap::new();
    let mut bereich: HashMap<usize, (f64, f64)> = HashMap::new();
    for t in 0..n {
        if let (Some(p), Some(g)) = (pose[t], gruppe[t]) {
            for q in punkte_cm(t) {
                let y = p.an(q).y;
                let e = bereich.entry(g.1).or_insert((f64::INFINITY, f64::NEG_INFINITY));
                e.0 = e.0.min(y);
                e.1 = e.1.max(y);
            }
        }
    }
    // Seite einer Naht ganz in einer Gruppe: (Gruppe, Anfang, Ende) in der
    // Abwicklung und die Proben an den Enden.
    let seite_in = |z: &Zug| -> Option<(usize, Pt, Pt, Probe, Probe)> {
        let g = gruppe[z.strecken.first()?.0]?.1;
        if !z.strecken.iter().all(|st| zone[st.0] == Zone::Rumpf && gruppe[st.0].is_some_and(|h| h.1 == g)) {
            return None;
        }
        let (a, b) = (z.bei(0.0)?, z.bei(1.0)?);
        Some((g, pose[a.teil]?.an(a.p), pose[b.teil]?.an(b.p), a, b))
    };
    let mut seitennaehte = Vec::new();
    // Enden der Schulternähte je Gruppe: (Gruppe, Lage, Probe). Eine
    // Schulter kann aus mehreren Nähten bestehen (innen die Mittelteile,
    // außen die Seitenteile); je Körperseite ist ihr höchstes Ende der
    // Halspunkt, ihr tiefstes der Schulterpunkt.
    let mut schulterenden: Vec<(usize, Pt, Probe)> = Vec::new();
    for (a, b) in &zuege {
        let (Some(za), Some(zb)) = (seitenzone(a), seitenzone(b)) else { continue };
        if za == zb {
            continue;
        }
        let (Some((ga, a0, a1, pa0, pa1)), Some((gb, b0, b1, pb0, pb1))) = (seite_in(a), seite_in(b)) else { continue };
        if (a1.y - a0.y).abs() < (a1.x - a0.x).abs() {
            // Schulternaht: oben in beiden Gruppen; Halsende = das höhere Ende.
            let oben = |g: usize, y: f64| bereich.get(&g).is_some_and(|&(u, o)| y > (u + o) / 2.0);
            if oben(ga, (a0.y + a1.y) / 2.0) && oben(gb, (b0.y + b1.y) / 2.0) {
                schulterenden.extend([(ga, a0, pa0), (ga, a1, pa1), (gb, b0, pb0), (gb, b1, pb1)]);
            }
        } else {
            seitennaehte.push((ga, a0, a1, gb, b0, b1));
        }
    }
    let mut schulterpunkte: Vec<(usize, Pt)> = Vec::new();
    for &g in schulterenden.iter().map(|(g, _, _)| g).collect::<std::collections::BTreeSet<_>>() {
        for minus in [false, true] {
            let e: Vec<&(usize, Pt, Probe)> = schulterenden.iter().filter(|(h, v, _)| *h == g && (v.x < 0.0) == minus).collect();
            let hoch = e.iter().max_by(|a, b| a.1.y.total_cmp(&b.1.y));
            let tief = e.iter().min_by(|a, b| a.1.y.total_cmp(&b.1.y));
            if let (Some(h), Some(t)) = (hoch, tief) {
                hoehe.entry(g).or_default().push(-h.1.y);
                schulterpunkte.push((t.2.teil, t.2.p));
            }
        }
    }
    let mut dy: HashMap<usize, f64> = hoehe.iter().map(|(&g, v)| (g, v.iter().sum::<f64>() / v.len() as f64)).collect();
    if dy.is_empty() {
        let namen: Vec<&str> = (0..n).filter(|&t| in_naht[t]).map(|t| raender[t].teil.as_str()).collect();
        let unten = unterteil(&namen);
        let k = raender.first().map_or(1.0, |r| r.in_cm(1.0));
        match (ev.variable("height_neck_side"), ev.variable("height_waist_side")) {
            (Ok(h), Ok(w)) => {
                let taille = -(h - w) * k;
                for &(ga, a0, a1, gb, b0, b1) in &seitennaehte {
                    let (ya, yb) = if (a0.y >= a1.y) == unten { (a0.y, b0.y) } else { (a1.y, b1.y) };
                    dy.entry(ga).or_insert(taille - ya);
                    dy.entry(gb).or_insert(taille - yb);
                }
                hinweise.push(format!(
                    "keine Schulternaht: Taille {} cm unter dem seitlichen Halspunkt (height_neck_side − height_waist_side) am {} Ende der Seitennähte",
                    zahl((h - w) * k).trim_start_matches('+'),
                    if unten { "oberen" } else { "unteren" }
                ));
            }
            _ => hinweise.push("keine Schulternaht und keine Maße height_neck_side, height_waist_side: Höhe offen (0)".to_string()),
        }
    }
    // Über Seitennähte weiterreichen: obere Enden gleich hoch.
    loop {
        let mut neu = false;
        for &(ga, a0, a1, gb, b0, b1) in &seitennaehte {
            let (ya, yb) = if a0.y >= a1.y { (a0.y, b0.y) } else { (a1.y, b1.y) };
            match (dy.get(&ga).copied(), dy.get(&gb).copied()) {
                (Some(d), None) => {
                    dy.insert(gb, ya + d - yb);
                    neu = true;
                }
                (None, Some(d)) => {
                    dy.insert(ga, yb + d - ya);
                    neu = true;
                }
                _ => {}
            }
        }
        if !neu {
            break;
        }
    }
    // Höhe in die Lagen der Gruppen.
    let mut ohne_hoehe = Vec::new();
    for t in 0..n {
        if let (Some(p), Some(g)) = (pose[t].as_mut(), gruppe[t]) {
            match dy.get(&g.1) {
                Some(d) => p.t.y += d,
                None => ohne_hoehe.push(raender[t].teil.clone()),
            }
        }
    }
    if !ohne_hoehe.is_empty() {
        hinweise.push(format!("Höhe offen (0): {}", ohne_hoehe.join(", ")));
    }

    // 6. Angelegte Teile: über ihre Nähte an schon liegende Teile derselben
    // Zone (am Rumpf je Ursprung vorn/hinten, am Arm je Seite).
    let anlegen = |pose: &mut Vec<Option<Pose>>,
                   gruppe: &mut Vec<Option<(bool, usize)>>,
                   frei: &dyn Fn(usize) -> bool,
                   ziel: &dyn Fn(usize, usize) -> bool,
                   glieder_von: &dyn Fn(usize) -> Vec<usize>| loop {
        // Bester Kandidat: (Abweichung / Spanne, Teil, Pose, Gruppe).
        let mut best: Option<(f64, usize, Pose, (bool, usize))> = None;
        // x: das freie Teil, y: liegende Teile; Paare aller Nähte je freiem
        // Teil und Ursprung (die Manschette des Bolero hängt an acht Stücken).
        let mut je: HashMap<(usize, bool), Vec<(Probe, Probe)>> = HashMap::new();
        for j in 0..zuege.len() {
            for (pa, pb) in paare(j) {
                for (x, y) in [(pa, pb), (pb, pa)] {
                    if !frei(x.teil) || pose[x.teil].is_some() || !ziel(x.teil, y.teil) {
                        continue;
                    }
                    if let (Some(_), Some(gy)) = (pose[y.teil], gruppe[y.teil]) {
                        je.entry((x.teil, gy.0)).or_default().push((x, y));
                    }
                }
            }
        }
        {
            {
                for ((tx, _), v) in je {
                    if v.len() < 3 {
                        continue;
                    }
                    let a: Vec<Pt> = v.iter().map(|(x, _)| Pt::new(s[tx] * x.p.x, -x.p.y)).collect();
                    let b: Vec<Pt> = v.iter().map(|(_, y)| pose[y.teil].unwrap().an(y.p)).collect();
                    let (phi, t, rms) = procrustes(&a, &b);
                    let spanne = v.iter().map(|(x, _)| x.p.dist(v[0].0.p)).fold(0.0, f64::max);
                    // Bis 30 % der Spanne: Falten und Kräusel zwischen den Stücken
                    // (Manschette an den Bolero-Ärmel: 6 cm auf 24 cm).
                    let rel = rms / spanne.max(1.0);
                    if phi.abs() > std::f64::consts::FRAC_PI_2 + 1e-6 || rms > (0.3 * spanne).max(1.0) || best.as_ref().is_some_and(|b| b.0 <= rel) {
                        continue;
                    }
                    let ps = Pose { s: s[tx], phi, t };
                    let (x, y) = v[v.len() / 2];
                    let (nx, ny) = (ps.richtung(x.n), pose[y.teil].unwrap().richtung(y.n));
                    if nx.x * ny.x + nx.y * ny.y >= 0.0 {
                        continue;
                    }
                    best = Some((rel, tx, ps, gruppe[y.teil].unwrap()));
                }
            }
        }
        let Some((_, tx, ps, gy)) = best else { break };
        for t in glieder_von(tx) {
            pose[t] = Some(ps);
            gruppe[t] = Some(gy);
        }
    };
    anlegen(&mut pose, &mut gruppe, &|t| zone[t] == Zone::Rumpf && in_naht[t], &|_, y| zone[y] == Zone::Rumpf, &|t| glieder(rahmen[t]));
    for &f in &alle_rahmen {
        let g = glieder(f);
        if pose[g[0]].is_some() {
            continue;
        }
        let mut ps = Pose { s: s[g[0]], phi: 0.0, t: Pt::new(0.0, 0.0) };
        ps.t = Pt::new(-breite_mitte(&g, &ps), 0.0);
        let vorn = rahmen_zone[&f].unwrap_or(true);
        for &t in &g {
            pose[t] = Some(ps);
            gruppe[t] = Some((vorn, f));
        }
        hinweise.push(format!(
            "{}: an keiner Naht anzulegen, mittig {} gelegt, Höhe offen",
            g.iter().map(|&t| raender[t].teil.as_str()).collect::<Vec<_>>().join(", "),
            if vorn { "vorn" } else { "hinten" }
        ));
    }

    // 7./8. Arm und Hals: je Gruppe ein Wurzelteil an seiner Naht zum Rumpf.
    // Arm: Seite nach dem Namen, sonst nach dem Armloch.
    let (rp, rg) = (pose.clone(), gruppe.clone());
    let liegt = |t: usize| rp[t].map(|p| (p, rg[t].unwrap()));
    let mut an_rumpf: Vec<Vec<(usize, bool)>> = vec![Vec::new(); n]; // (Naht, Teil auf Seite a)
    for (j, (a, b)) in zuege.iter().enumerate() {
        for (x, y, auf_a) in [(a, b, true), (b, a, false)] {
            let tx = x.strecken.first().map(|st| st.0);
            if let Some(tx) = tx {
                if zone[tx] != Zone::Rumpf && x.strecken.iter().all(|st| st.0 == tx) && y.strecken.iter().all(|st| zone[st.0] == Zone::Rumpf) {
                    an_rumpf[tx].push((j, auf_a));
                }
            }
        }
    }
    for t in 0..n {
        if zone[t] == Zone::Arm && sw[t].is_none() {
            if let Some(&(j, auf_a)) = an_rumpf[t].first() {
                let y = if auf_a { &zuege[j].1 } else { &zuege[j].0 };
                sw[t] = y.strecken.iter().find_map(|st| sw[st.0]);
            }
        }
    }
    let mut anker: Vec<Option<usize>> = vec![None; n];
    let mut hals_hoehe: Option<f64> = None;
    for t in 0..n {
        if zone[t] == Zone::Rumpf || !in_naht[t] || an_rumpf[t].is_empty() || pose[t].is_some() {
            continue;
        }
        // Arm: die Probe am Schulterpunkt (äußeres Ende einer Schulternaht)
        // über alle Nähte zum Rumpf, auch an den Enden (das Armloch des
        // Bolero ist in Stücke zwischen den Falten geteilt).
        let mut px: Option<Probe> = None;
        let mut py: Option<Probe> = None;
        if zone[t] == Zone::Arm {
            let mut best = f64::INFINITY;
            for &(j, auf_a) in &an_rumpf[t] {
                let (x, y) = if auf_a { (&zuege[j].0, &zuege[j].1) } else { (&zuege[j].1, &zuege[j].0) };
                let m = (x.laenge as usize).clamp(8, 200);
                for i in 0..=m {
                    let f = i as f64 / m as f64;
                    let (Some(a), Some(b)) = (x.bei(f), y.bei(f)) else { continue };
                    for &(st, sp) in &schulterpunkte {
                        let d = if st == b.teil { b.p.dist(sp) } else { f64::INFINITY };
                        if d < best {
                            best = d;
                            px = Some(a);
                            py = Some(b);
                        }
                    }
                }
            }
            if best > 2.0 {
                px = None;
            }
        }
        if px.is_none() {
            // Längste Naht zum Rumpf; Fuge auf der Rumpfseite: Arm — zwischen
            // vorn und hinten; Hals — hintere Mitte (zwischen linkem und
            // rechtem Rückenteil oder zwei Stücken desselben Rückenteils).
            let &(j, auf_a) = an_rumpf[t].iter().max_by(|x, y| zuege[x.0].0.laenge.total_cmp(&zuege[y.0].0.laenge)).unwrap();
            let (x, y) = if auf_a { (&zuege[j].0, &zuege[j].1) } else { (&zuege[j].1, &zuege[j].0) };
            let hinten = |u: usize| liegt(u).is_some_and(|(_, g)| !g.0);
            let fuge = y.fugen.iter().find(|&&(_, p, q)| match zone[t] {
                Zone::Arm => liegt(p).is_some() && liegt(q).is_some() && liegt(p).unwrap().1 .0 != liegt(q).unwrap().1 .0,
                _ => hinten(p) && hinten(q) && (p == q || (sw[p].is_some() && sw[q].is_some() && sw[p] != sw[q])),
            });
            let f = match fuge {
                Some(&(sf, _, _)) => sf / y.laenge.max(1e-12),
                None if zone[t] == Zone::Hals => {
                    // Mitte des hinteren Teils der Seite.
                    let h: Vec<f64> = y.strecken.iter().filter(|st| hinten(st.0)).map(|st| st.4).collect();
                    if h.is_empty() {
                        0.5
                    } else {
                        (h.iter().cloned().fold(f64::INFINITY, f64::min) + h.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) / 2.0 / y.laenge.max(1e-12)
                    }
                }
                None => {
                    hinweise.push(format!("{}: kein Schulterpunkt im Armloch, Kugelmitte angenommen", raender[t].teil));
                    0.5
                }
            };
            px = x.bei(f);
            py = y.bei(f);
        }
        let (Some(px), Some(py)) = (px, py) else { continue };
        let r = &raender[t];
        let i = (0..r.punktknoten.len()).min_by(|&a, &b| knoten_cm(t, a).dist(px.p).total_cmp(&knoten_cm(t, b).dist(px.p))).unwrap_or(0);
        let q = knoten_cm(t, i);
        let mut ps = Pose { s: s[t], phi: 0.0, t: Pt::new(0.0, 0.0) };
        let v = ps.an(q);
        ps.t = Pt::new(-v.x, -v.y);
        pose[t] = Some(ps);
        anker[t] = Some(i);
        // Gruppe: Arm je Seite (links = true), Hals eine.
        gruppe[t] = Some(match zone[t] {
            Zone::Arm => (sw[t].unwrap_or(true), usize::MAX - 1 - usize::from(sw[t] == Some(false))),
            _ => (false, usize::MAX),
        });
        if zone[t] == Zone::Hals {
            if let Some(p) = rp[py.teil] {
                hals_hoehe = Some(p.an(py.p).y);
            }
        }
    }
    let gr = gruppe.clone();
    anlegen(
        &mut pose,
        &mut gruppe,
        &|t| zone[t] != Zone::Rumpf && in_naht[t],
        &|x, y| zone[y] == zone[x] && (zone[x] != Zone::Arm || gr[y].is_some_and(|g| Some(g.0) == sw[x].or(Some(g.0)))),
        &|t| vec![t],
    );

    // Ausgabe.
    let mut text = String::new();
    text.push_str(&format!("# {}\n", kopf.replace('\n', "\n# ")));
    text.push_str("#\n# Teil | Zone | Bezugspunkt | Umfang | Hoehe | Optionen\n");
    for h in &hinweise {
        text.push_str(&format!("# Hinweis: {h}\n"));
    }
    let weg: Vec<&str> = (0..n).filter(|&t| !in_naht[t]).map(|t| raender[t].teil.as_str()).collect();
    if !weg.is_empty() {
        text.push_str(&format!("# Nicht platziert (ohne Naht): {}\n", weg.join(", ")));
    }
    text.push_str("\nabstand 2\n\n");
    // Rumpf: ein gemeinsamer Bezugspunkt je Rahmen — sein höchster Punktknoten.
    let mut rahmen_anker: HashMap<usize, (usize, usize)> = HashMap::new();
    for t in 0..n {
        let Some(p) = pose[t] else { continue };
        if zone[t] != Zone::Rumpf {
            continue;
        }
        for i in 0..raender[t].punktknoten.len() {
            let y = p.an(knoten_cm(t, i)).y;
            let e = rahmen_anker.entry(rahmen[t]).or_insert((t, i));
            if y > pose[e.0].unwrap().an(knoten_cm(e.0, e.1)).y + 1e-9 {
                *e = (t, i);
            }
        }
    }
    // Rumpfteile an einem Armteil (Armloch, Seitennaht mit der Ärmelnaht):
    // Abstand 0,5 cm wie am Arm, sonst liegen sie im Arm.
    let mut am_arm = vec![false; n];
    for (a, b) in &zuege {
        for (x, y) in [(a, b), (b, a)] {
            if y.strecken.iter().any(|st| zone[st.0] == Zone::Arm) {
                for st in &x.strecken {
                    am_arm[st.0] |= zone[st.0] == Zone::Rumpf;
                }
            }
        }
    }
    let mut fehlt = Vec::new();
    for t in 0..n {
        if !in_naht[t] {
            continue;
        }
        let (Some(p), Some(g)) = (pose[t], gruppe[t]) else {
            fehlt.push(raender[t].teil.clone());
            continue;
        };
        let (at, ai) = match zone[t] {
            Zone::Rumpf => rahmen_anker[&rahmen[t]],
            _ => (
                t,
                anker[t].unwrap_or_else(|| {
                    (0..raender[t].punktknoten.len()).max_by(|&a, &b| p.an(knoten_cm(t, a)).y.total_cmp(&p.an(knoten_cm(t, b)).y)).unwrap_or(0)
                }),
            ),
        };
        let v = p.an(knoten_cm(at, ai));
        let name = &raender[at].punktknoten[ai].name;
        let mut opt = Vec::new();
        if p.s < 0.0 {
            opt.push("gespiegelt".to_string());
        }
        let dreh = (p.s * p.phi).to_degrees();
        if dreh.abs() > 0.05 {
            opt.push(format!("drehung {}", zahl(dreh)));
        }
        if zone[t] == Zone::Rumpf && am_arm[t] {
            opt.push("abstand 0,5".to_string());
        }
        if let Some(d) = dicke_nach_name(&raender[t].teil) {
            opt.push(format!("dicke {}", zahl(d).trim_start_matches('+')));
        }
        let zeile = match zone[t] {
            Zone::Rumpf => format!(
                "{} | rumpf | {name} | {} {}cm | hals seitlich {} |",
                raender[t].teil,
                if g.0 { "vorn" } else { "hinten" },
                zahl(v.x),
                zahl(v.y)
            ),
            Zone::Hals => {
                opt.push("abstand 3".to_string());
                format!("{} | hals | {name} | hinten {}cm | hals seitlich {} |", raender[t].teil, zahl(v.x), zahl(v.y + hals_hoehe.unwrap_or(0.0)))
            }
            Zone::Arm => {
                opt.push("abstand 0,5".to_string());
                // Der Versatz zählt in Richtung wachsender Winkel (nach vorn);
                // „von außen rechts“ zeigt oben auf dem linken Arm nach hinten.
                let links = sw[t] != Some(false);
                let seite = if links { "links" } else { "rechts" };
                format!("{} | arm {seite} | {name} | oben {}cm | {} |", raender[t].teil, zahl(if links { -v.x } else { v.x }), zahl(-v.y))
            }
        };
        text.push_str(&format!("{zeile} {}\n", opt.join(" ")).replace("| \n", "|\n"));
    }
    if !fehlt.is_empty() {
        hinweise.push(format!("ohne Lage: {}", fehlt.join(", ")));
        text.push_str(&format!("# ohne Lage (an nichts anzulegen): {}\n", fehlt.join(", ")));
    }
    Ok(Lagevorschlag { text, hinweise })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dicke_aus_dem_teilnamen() {
        assert_eq!(dicke_nach_name("1 BUND - 1 mm einlagig, ganzer Umfang, Naht hinten"), Some(1.0));
        assert_eq!(dicke_nach_name("Rock - Bahn 0,4mm"), Some(0.4));
        assert_eq!(dicke_nach_name("Bluse 3 - Oberteil vorn Links"), None);
        assert_eq!(dicke_nach_name("Saum 30 mm, Lage 1 mm"), None);
        assert_eq!(dicke_nach_name("Teil 12mmx"), None);
    }
}
