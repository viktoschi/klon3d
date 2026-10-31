//! Abnahme Stufe 3, Teil 2: Netz und Nahtaufteilung.
//!
//! Abgestimmt mit der Anwenderin am 20.10.2026:
//!
//! | Prüfung | Regel |
//! |---|---|
//! | Netz | je Teil eine Scheibe: Dreiecke positiv, jede innere Kante in zwei Dreiecken, die Randkanten genau der Randumlauf, Eulerzahl 1 |
//! | Netzrand | jeder Punktknoten ist ein Netzpunkt, jeder Randpunkt liegt auf seiner Randkante; Sehnenabstand ≤ 0,01 cm, Längenabweichung je Randkante ≤ 0,01 cm |
//! | Fläche | Netzfläche = Fläche des Randpolygons; Abstand zur Konturfläche wird gemeldet (Sehnen schneiden Kurven ab) |
//! | Winkel | kleinster Dreieckswinkel ≥ 25°; ausgenommen das Dreieck in einer Teilecke, die selbst spitzer ist (gezählt als `spitze_ecken`) |
//! | Kantenlänge | etwa 10 mm: gemeldet werden Median, Mittel, kleinste und größte Kante |
//! | Nähte | jede Teilnaht hat auf beiden Seiten gleich viele Randpunkte, die auf beiden Teilen Randnachbarn sind und jede genähte Randkante ganz abdecken; Versatz der Stellen ≤ Fangweite (2 mm) |

use std::collections::{HashMap, HashSet};

use crate::{Musternetz, Nahtnetz, Parameter, Stellenart, Teilnetz};

fn abstand(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn flaeche(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>() * 0.5
}

/// Kleinster Winkel eines Dreiecks in Grad.
pub fn kleinster_winkel(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    let (sin_min, _, _) = crate::dreieck::form(a, b, c);
    sin_min.clamp(0.0, 1.0).asin().to_degrees()
}

/// Winkel an `a` zwischen `a→b` und `a→c` in Grad.
fn winkel_an(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
    (u[0] * v[1] - u[1] * v[0]).abs().atan2(u[0] * v[0] + u[1] * v[1]).to_degrees()
}

/// Kennzahlen eines Teilnetzes.
#[derive(Debug, Clone)]
pub struct Teilpruefung {
    pub teil: String,
    pub punkte: usize,
    pub randpunkte: usize,
    pub dreiecke: usize,
    /// Fehler der Netzstruktur, leer wenn in Ordnung.
    pub struktur: Vec<String>,
    pub min_winkel: f64,
    /// Dreiecke unter dem Zielwinkel, außer an spitzen Teilecken.
    pub unter_winkel: usize,
    /// Dreiecke an einer Teilecke, die selbst spitzer als der Zielwinkel ist:
    /// beide Schenkel des kleinsten Winkels sind Randstücke, die übrigen
    /// Winkel erreichen das Ziel. Kein Netz kann das verbessern.
    pub spitze_ecken: usize,
    pub kante_min: f64,
    pub kante_median: f64,
    pub kante_mittel: f64,
    pub kante_max: f64,
    /// Netzfläche und Fläche der Nahtlinie (mm²).
    pub flaeche_netz: f64,
    pub flaeche_kontur: f64,
    /// Netzfläche − Fläche des Randpolygons (mm², nur Rundung).
    pub flaeche_rand_diff: f64,
    /// Größter Abstand eines Kantenpunkts vom Netzrand (mm).
    pub sehne_max: f64,
    /// Größte |Länge Netzrand − Länge Randkante| einer Randkante (mm).
    pub laenge_max: f64,
    /// Größter Abstand eines Randpunkts von seinem Ort auf der Randkante
    /// bzw. eines Punktknotens von seinem Kantenanfang (mm).
    pub lage_max: f64,
}

/// Kennzahlen einer Naht.
#[derive(Debug, Clone)]
pub struct Nahtpruefung {
    pub kennung: String,
    /// Punkte je Teilnaht auf Seite A und B.
    pub punkte: Vec<(usize, usize)>,
    pub paare: usize,
    /// Fehler der Zuordnung, leer wenn in Ordnung.
    pub struktur: Vec<String>,
    /// Größter |Versatz| einer Stelle (mm) und ihre Namen.
    pub versatz_max: f64,
    pub versatz_stelle: String,
    /// Größter Unterschied zweier gepaarter Randstücke (mm).
    pub stueck_diff_max: f64,
    /// Kürzestes und längstes Randstück der Naht (mm).
    pub stueck_min: f64,
    pub stueck_max: f64,
}

#[derive(Debug, Clone)]
pub struct Netzbericht {
    pub teile: Vec<Teilpruefung>,
    pub naehte: Vec<Nahtpruefung>,
}

/// Prüft ein Teilnetz.
pub fn pruefe_teil(t: &Teilnetz, p: &Parameter) -> Teilpruefung {
    let mut struktur = Vec::new();
    let n = t.punkte.len();
    let nr = t.nrand;
    let randstueck = |a: u32, b: u32| {
        let (a, b, n) = (a as usize, b as usize, nr);
        a < n && b < n && (b == (a + 1) % n || a == (b + 1) % n)
    };
    let mut kanten: HashMap<(u32, u32), usize> = HashMap::new();
    let mut benutzt = vec![false; n];
    let (mut min_winkel, mut unter_winkel, mut spitze_ecken, mut flaeche_netz) = (180.0f64, 0, 0, 0.0);
    for d in &t.dreiecke {
        let q = |k: usize| t.punkte[d[k] as usize];
        let f2 = (q(1)[0] - q(0)[0]) * (q(2)[1] - q(0)[1]) - (q(1)[1] - q(0)[1]) * (q(2)[0] - q(0)[0]);
        if f2 <= 0.0 {
            struktur.push(format!("Dreieck {d:?} nicht positiv"));
        }
        flaeche_netz += 0.5 * f2;
        let w = kleinster_winkel(q(0), q(1), q(2));
        min_winkel = min_winkel.min(w);
        if w < p.winkel_grad {
            let winkel: Vec<f64> = (0..3).map(|k| winkel_an(q(k), q((k + 1) % 3), q((k + 2) % 3))).collect();
            let k = (0..3).min_by(|&i, &j| winkel[i].total_cmp(&winkel[j])).unwrap();
            let ecke = randstueck(d[k], d[(k + 1) % 3]) && randstueck(d[k], d[(k + 2) % 3]);
            let rest_gut = (0..3).filter(|&i| i != k).all(|i| winkel[i] >= p.winkel_grad);
            if ecke && rest_gut {
                spitze_ecken += 1;
            } else {
                unter_winkel += 1;
            }
        }
        for i in 0..3 {
            *kanten.entry((d[i], d[(i + 1) % 3])).or_default() += 1;
            benutzt[d[i] as usize] = true;
        }
    }
    let mut rand: HashSet<(u32, u32)> = HashSet::new();
    let mut laengen = Vec::with_capacity(kanten.len());
    for (&(a, b), &k) in &kanten {
        if k != 1 {
            struktur.push(format!("Kante {a}→{b} {k}-mal"));
        }
        if !kanten.contains_key(&(b, a)) {
            rand.insert((a.min(b), a.max(b)));
        }
        if a < b || !kanten.contains_key(&(b, a)) {
            laengen.push(abstand(t.punkte[a as usize], t.punkte[b as usize]));
        }
    }
    let umlauf: HashSet<(u32, u32)> =
        (0..nr).map(|i| (i as u32, ((i + 1) % nr) as u32)).map(|(a, b)| (a.min(b), a.max(b))).collect();
    if rand != umlauf {
        struktur.push(format!("Netzrand ({} Kanten) ist nicht der Randumlauf ({nr})", rand.len()));
    }
    if let Some(v) = benutzt.iter().position(|b| !b) {
        struktur.push(format!("Punkt {v} in keinem Dreieck"));
    }
    let euler = n as i64 - laengen.len() as i64 + t.dreiecke.len() as i64;
    if euler != 1 {
        struktur.push(format!("Eulerzahl {euler} statt 1"));
    }
    laengen.sort_by(f64::total_cmp);
    let median = if laengen.is_empty() { 0.0 } else { laengen[laengen.len() / 2] };

    // Netzrand gegen Randkanten.
    let mut lage_max: f64 = 0.0;
    for (i, &(k, s)) in t.randort.iter().enumerate() {
        lage_max = lage_max.max(abstand(t.punkte[i], t.kanten[k].ort(s)));
    }
    for &(_, v) in t.punktknoten.iter() {
        let k = t.randort[v as usize].0;
        lage_max = lage_max.max(abstand(t.punkte[v as usize], t.kanten[k].punkte[0]));
    }
    let (mut sehne_max, mut laenge_max): (f64, f64) = (0.0, 0.0);
    for (k, kz) in t.kanten.iter().enumerate() {
        let mut s: Vec<f64> = t.randort.iter().filter(|o| o.0 == k).map(|o| o.1).collect();
        s.push(kz.laenge);
        let mut summe = 0.0;
        for w in s.windows(2) {
            let (h, _) = kz.sehne(w[0], w[1]);
            sehne_max = sehne_max.max(h);
            summe += abstand(kz.ort(w[0]), kz.ort(w[1]));
        }
        laenge_max = laenge_max.max((summe - kz.laenge).abs());
    }
    let randpolygon: Vec<[f64; 2]> = t.punkte[..nr].to_vec();
    Teilpruefung {
        teil: t.teil.clone(),
        punkte: n,
        randpunkte: nr,
        dreiecke: t.dreiecke.len(),
        struktur,
        min_winkel,
        unter_winkel,
        spitze_ecken,
        kante_min: laengen.first().copied().unwrap_or(0.0),
        kante_median: median,
        kante_mittel: laengen.iter().sum::<f64>() / laengen.len().max(1) as f64,
        kante_max: laengen.last().copied().unwrap_or(0.0),
        flaeche_netz,
        flaeche_kontur: flaeche(&t.kontur).abs(),
        flaeche_rand_diff: flaeche_netz - flaeche(&randpolygon).abs(),
        sehne_max,
        laenge_max,
        lage_max,
    }
}

/// Prüft die Punktpaare einer Naht.
pub fn pruefe_naht(netz: &Musternetz, n: &Nahtnetz) -> Nahtpruefung {
    let mut struktur = Vec::new();
    let mut punkte = Vec::new();
    let (mut diff_max, mut st_min, mut st_max): (f64, f64, f64) = (0.0, f64::INFINITY, 0.0);
    // Abgedeckte Randstücke (Index des Anfangs) je Teil.
    let mut gedeckt: HashMap<usize, HashSet<usize>> = HashMap::new();
    for tn in &n.teilnaehte {
        let (ta, tb) = (&netz.teile[tn.teil_a], &netz.teile[tn.teil_b]);
        let pa: Vec<u32> = tn.paare.iter().map(|q| q.a).collect();
        let pb: Vec<u32> = tn.paare.iter().map(|q| q.b).collect();
        punkte.push((pa.len(), pb.len()));
        for (seite, t, p) in [("A", ta, &pa), ("B", tb, &pb)] {
            let nr = t.nrand as u32;
            if p.iter().any(|&v| v >= nr) {
                struktur.push(format!("{seite}: Punkt nicht auf dem Rand ({})", t.teil));
                continue;
            }
            let schritt = |x: u32, y: u32| if y == (x + 1) % nr { 1 } else if x == (y + 1) % nr { -1 } else { 0 };
            let richtungen: HashSet<i32> = p.windows(2).map(|w| schritt(w[0], w[1])).collect();
            if richtungen.contains(&0) || richtungen.len() > 1 {
                struktur.push(format!("{seite}: Punkte nicht fortlaufend am Rand von {}", t.teil));
                continue;
            }
            let ziel = if seite == "A" { tn.teil_a } else { tn.teil_b };
            let stuecke = p.windows(2).map(|w| if schritt(w[0], w[1]) == 1 { w[0] } else { w[1] } as usize);
            gedeckt.entry(ziel).or_default().extend(stuecke);
        }
        for (w, v) in pa.windows(2).zip(pb.windows(2)) {
            let la = abstand(ta.punkte[w[0] as usize], ta.punkte[w[1] as usize]);
            let lb = abstand(tb.punkte[v[0] as usize], tb.punkte[v[1] as usize]);
            diff_max = diff_max.max((la - lb).abs());
            st_min = st_min.min(la.min(lb));
            st_max = st_max.max(la.max(lb));
        }
        for w in tn.paare.windows(2) {
            if !(w[1].rho_a > w[0].rho_a && w[1].rho_b > w[0].rho_b) {
                struktur.push("Längenanteile nicht steigend".into());
                break;
            }
        }
    }
    // Nähte bestehen aus ganzen Randkanten: Ist ein Randstück einer Kante
    // genäht, dann alle.
    for (&ti, menge) in &gedeckt {
        let t = &netz.teile[ti];
        let kanten: HashSet<usize> = menge.iter().map(|&i| t.randort[i].0).collect();
        for k in kanten {
            if (0..t.nrand).any(|i| t.randort[i].0 == k && !menge.contains(&i)) {
                struktur.push(format!("{}: Randkante {k} nur teilweise in der Naht", t.teil));
            }
        }
    }
    let (versatz_max, versatz_stelle) = n
        .stellen
        .iter()
        .filter(|s| s.art != Stellenart::Ende)
        .map(|s| (s.versatz_mm.abs(), format!("{} ↔ {}", s.name_a, s.name_b)))
        .max_by(|x, y| x.0.total_cmp(&y.0))
        .unwrap_or((0.0, String::new()));
    if punkte.iter().any(|(a, b)| a != b) {
        struktur.push(format!("ungleiche Punktzahl {punkte:?}"));
    }
    Nahtpruefung {
        kennung: n.kennung.clone(),
        paare: n.teilnaehte.iter().map(|t| t.paare.len()).sum(),
        punkte,
        struktur,
        versatz_max,
        versatz_stelle,
        stueck_diff_max: diff_max,
        stueck_min: if st_min.is_finite() { st_min } else { 0.0 },
        stueck_max: st_max,
    }
}

pub fn pruefe(netz: &Musternetz) -> Netzbericht {
    Netzbericht {
        teile: netz.teile.iter().map(|t| pruefe_teil(t, &netz.parameter)).collect(),
        naehte: netz.naehte.iter().map(|n| pruefe_naht(netz, n)).collect(),
    }
}

impl Netzbericht {
    /// Verstöße gegen die Abnahme; leer heißt bestanden.
    pub fn verstoesse(&self, p: &Parameter) -> Vec<String> {
        let mut v = Vec::new();
        for t in &self.teile {
            for s in &t.struktur {
                v.push(format!("{}: {s}", t.teil));
            }
            if t.unter_winkel > 0 {
                v.push(format!("{}: {} Dreiecke unter {}° (kleinster {:.2}°)", t.teil, t.unter_winkel, p.winkel_grad, t.min_winkel));
            }
            if t.sehne_max > p.sehne_mm + 1e-9 {
                v.push(format!("{}: Sehnenabstand {:.4} mm", t.teil, t.sehne_max));
            }
            if t.laenge_max > p.laenge_mm + 1e-9 {
                v.push(format!("{}: Randkante {:.4} mm zu kurz/lang", t.teil, t.laenge_max));
            }
            if t.lage_max > 1e-6 {
                v.push(format!("{}: Randpunkt {:.6} mm neben der Kante", t.teil, t.lage_max));
            }
            if t.flaeche_rand_diff.abs() > 1e-6 * t.flaeche_netz.abs().max(1.0) {
                v.push(format!("{}: Netzfläche weicht vom Randpolygon ab ({:.6} mm²)", t.teil, t.flaeche_rand_diff));
            }
        }
        for n in &self.naehte {
            for s in &n.struktur {
                v.push(format!("{}: {s}", n.kennung));
            }
            // Stellen unter 1 µm legt der Randplan immer zusammen.
            if n.versatz_max > p.fang_mm.max(1e-3) + 1e-9 {
                v.push(format!("{}: Versatz {:.3} mm an {}", n.kennung, n.versatz_max, n.versatz_stelle));
            }
        }
        v
    }
}
