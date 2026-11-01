//! Abnahme der Abstandskarte Stoff → Haut (Stufe 5, Task #15).
//!
//! Die Karte ist das erste Ausgabemaß der Stufe 5: je Stoffpunkt der
//! vorzeichenbehaftete Abstand zur Körperoberfläche, dazu Kennzahlen je Teil.
//! Sie soll zeigen, was die fünf Fehler der Testbluse ausmacht — stehende
//! Ärmelkugel, klaffende Ränder, offener Kragen —, ohne sie zu beheben.
//!
//! Geprüft wird in dieser Reihenfolge:
//!
//! 1. **geschlossen gerechnet** am analytischen Zylinder (Röhre): 5 mm Luft
//!    plus halbe Stoffdicke, auf der Haut die halbe Dicke, innen negativ;
//! 2. **Kennzahlen je Teil** aus bekannten Einzelwerten;
//! 3. **gegen fremden Rechenweg**: der Betrag stimmt mit [`Vergleich`] überein
//!    (Punkt → Fläche, eigenes Gitter), das Vorzeichen und der nächste Punkt
//!    mit [`Koerpernetz::naechster_alle`] (Brute Force).
//!
//! Alle drei laufen ohne CLO-Daten. Der letzte Test braucht den Körper des
//! Referenzprojekts und überspringt sich ohne ihn.

use klon3d_sim::abstand::{abstand, Karte, Verteilung, INNEN};
use klon3d_sim::koerper::Koerper;
use klon3d_sim::netzkoerper::{zylinder, Koerpernetz, SUCHTIEFE};
use klon3d_sim::roehre::Roehre;
use klon3d_sim::vergleich::{Netz, Vergleich};
use klon3d_sim::Szene;

/// Halbe Stoffdicke von Radical Rubber 0,4 mm.
const HALBE_DICKE: f64 = 0.2e-3;

/// Luft, die [`Roehre`] zwischen Haut und Stoff legt (m).
const ANFANGSABSTAND: f64 = 5e-3;

/// Röhre ohne Reduktion auf dem analytischen Zylinder.
fn roehre_auf_zylinder() -> Szene {
    let r = Roehre {
        reduktion: 0.0,
        ..Roehre::default()
    };
    r.szene(klon3d_sim::Simparameter::default())
        .expect("Röhre baubar")
}

/// Alle Punkte der Szene auf den Radius `r` schieben.
fn auf_radius(s: &mut Szene, r: f64) {
    for q in &mut s.x {
        let jetzt = (q[0] * q[0] + q[1] * q[1]).sqrt();
        let f = r / jetzt;
        q[0] *= f;
        q[1] *= f;
    }
}

/// Betrag der Abweichung in m.
fn nah(a: f64, b: f64) -> f64 {
    (a - b).abs()
}

/// 1a. Die Anfangslage der Röhre liegt 5 mm über der Haut, dazu die halbe
/// Dicke: 5,2 mm an jedem Punkt. Damit ist der Maßstab festgelegt.
#[test]
fn anfangslage_ist_fuenf_millimeter_luft_plus_halbe_dicke() {
    let s = roehre_auf_zylinder();
    let k = Karte::messe(&s).expect("Körper vorhanden");
    assert_eq!(k.je_punkt.len(), s.x.len());
    let soll = ANFANGSABSTAND + HALBE_DICKE;
    for (i, p) in k.je_punkt.iter().enumerate() {
        assert!(
            nah(p.abstand, soll) < 1e-12,
            "Punkt {i}: {:.6} mm statt {:.6} mm",
            p.abstand * 1e3,
            soll * 1e3
        );
    }
    // Der nächste Hautpunkt liegt auf dem Zylinder, die Normale zeigt nach
    // außen.
    let radius = Roehre::default().zylinder_radius_m;
    for p in &k.je_punkt {
        let r = (p.punkt[0] * p.punkt[0] + p.punkt[1] * p.punkt[1]).sqrt();
        assert!(nah(r, radius) < 1e-12, "Hautpunkt bei r = {r:.6}");
        assert!(
            p.normale[0] * p.punkt[0] + p.normale[1] * p.punkt[1] > 0.0,
            "Normale zeigt nicht nach außen"
        );
    }
}

/// 1b. Auf die Haut gelegt (Mittelradius): genau die halbe Dicke.
#[test]
fn auf_der_haut_ist_es_die_halbe_dicke() {
    let mut s = roehre_auf_zylinder();
    let rm = Roehre::default().mittelradius_m();
    auf_radius(&mut s, rm);
    let k = Karte::messe(&s).expect("Körper vorhanden");
    for (i, p) in k.je_punkt.iter().enumerate() {
        assert!(
            nah(p.abstand, HALBE_DICKE) < 1e-12,
            "Punkt {i}: {:.6} mm",
            p.abstand * 1e3
        );
    }
}

/// 2. Innerhalb der Haut ist der Abstand negativ und die Kennzahl zählt die
/// Punkte als `innen`; genau auf der Haut ist er null.
#[test]
fn innen_ist_negativ_und_wird_gezaehlt() {
    let mut s = roehre_auf_zylinder();
    let radius = Roehre::default().zylinder_radius_m;
    auf_radius(&mut s, radius - 2e-3);
    let k = Karte::messe(&s).expect("Körper vorhanden");
    for (i, p) in k.je_punkt.iter().enumerate() {
        assert!(
            nah(p.abstand, -2e-3) < 1e-12,
            "Punkt {i}: {:.6} mm",
            p.abstand * 1e3
        );
    }
    let t = &k.je_teil[0];
    assert_eq!(t.innen, s.x.len(), "alle Punkte liegen innen");
    assert!(
        nah(t.verteilung.min, -2.0) < 1e-9,
        "min {:.6}",
        t.verteilung.min
    );
    assert!(t.verteilung.max < 0.0, "max {:.6}", t.verteilung.max);

    // Genau auf der Haut: null, und nicht als innen gezählt.
    let mut s = roehre_auf_zylinder();
    auf_radius(&mut s, radius);
    let k = Karte::messe(&s).expect("Körper vorhanden");
    for p in &k.je_punkt {
        assert!(p.abstand.abs() < 1e-12, "{:.6}", p.abstand * 1e3);
    }
    assert_eq!(k.je_teil[0].innen, 0, "auf der Haut ist nichts innen");
    assert!(INNEN < 0.0, "die Schwelle liegt unter null");
}

/// 3. Kennzahlen je Teil aus bekannten Einzelwerten: ein Punkt 2 mm innen,
/// drei bei 5,2 mm — der Median bleibt beim ruhigen Wert, das Minimum zeigt
/// das Eindringen.
#[test]
fn kennzahlen_zeigen_das_eindringen_und_nicht_das_mittel() {
    let v = Verteilung::aus(vec![5.2, 5.2, -2.0, 5.2]);
    assert_eq!(v.anzahl, 4);
    assert!(nah(v.min, -2.0) < 1e-12, "min {:.6}", v.min);
    assert!(nah(v.max, 5.2) < 1e-12, "max {:.6}", v.max);
    assert!(nah(v.median, 5.2) < 1e-12, "median {:.6}", v.median);
    assert!(nah(v.mittel, 3.4) < 1e-12, "mittel {:.6}", v.mittel);

    // Und die Karte bildet dieselbe Zahl aus der Szene: ein Punkt 7,2 mm nach
    // innen, von 5,2 auf −2,0.
    let mut s = roehre_auf_zylinder();
    let radius = Roehre::default().zylinder_radius_m;
    let rm = Roehre::default().mittelradius_m();
    auf_radius(&mut s, rm + ANFANGSABSTAND);
    let q = s.x[0];
    let r = (q[0] * q[0] + q[1] * q[1]).sqrt();
    let f = (r - 7.2e-3) / r;
    s.x[0] = [q[0] * f, q[1] * f, q[2]];
    let k = Karte::messe(&s).expect("Körper vorhanden");
    assert_eq!(k.je_teil.len(), 1, "die Röhre ist ein Teil");
    let t = &k.je_teil[0];
    assert_eq!(t.innen, 1);
    assert!(
        nah(t.verteilung.min, -2.0) < 1e-9,
        "min {:.6}",
        t.verteilung.min
    );
    assert!(
        nah(t.verteilung.max, 5.2) < 1e-9,
        "max {:.6}",
        t.verteilung.max
    );
    assert!(
        nah(t.verteilung.median, 5.2) < 1e-9,
        "median {:.6}",
        t.verteilung.median
    );
    let _ = radius;
}

/// 4. Gegen den fremden Rechenweg. Der Betrag kommt aus [`Vergleich`]
/// (Punkt → Fläche über ein eigenes Gitter, Millimeter), Vorzeichen und
/// nächster Punkt aus der Brute-Force-Suche. Gerechnet am Zylindernetz, damit
/// beide Wege wirklich verschieden sind.
#[test]
fn betrag_wie_vergleich_und_vorzeichen_wie_brute_force() {
    let (punkte_m, dreiecke) = zylinder(0.09, -0.3, 0.1, 48, 12);
    let netz =
        Koerpernetz::neu(punkte_m.clone(), dreiecke.clone(), SUCHTIEFE).expect("Zylindernetz");
    let koerper = Koerper::netz(netz.clone());

    let mut s = 0x2545_f491_4f6c_dd1du64;
    let mut zufall = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let pruef: Vec<[f64; 3]> = (0..2000)
        .map(|_| {
            let r = 0.06 + 0.06 * zufall();
            let w = 2.0 * std::f64::consts::PI * zufall();
            let z = -0.32 + 0.46 * zufall();
            [r * w.cos(), r * w.sin(), z]
        })
        .collect();

    // `Vergleich` rechnet in mm und braucht auf beiden Seiten Dreiecke. Für
    // den Hinweg zählen nur die Knoten von `a` und die Fläche von `b`; das
    // eine Dreieck in `a` hält die Indizes gültig und verdirbt den Hinweg
    // nicht.
    let mm = |p: &[f64; 3]| [p[0] * 1e3, p[1] * 1e3, p[2] * 1e3];
    let a = Netz {
        punkte: pruef.iter().map(mm).collect(),
        dreiecke: vec![[0, 1, 2]],
        gruppe: vec![0],
        namen: vec!["pruef".to_string()],
    };
    let b = Netz {
        punkte: punkte_m.iter().map(mm).collect(),
        dreiecke: dreiecke.iter().map(|d| [d[0], d[1], d[2]]).collect(),
        gruppe: vec![0; dreiecke.len()],
        namen: vec!["koerper".to_string()],
    };
    let v = Vergleich::rechne(&a, &b, 20.0).expect("Vergleich rechenbar");
    assert_eq!(v.a_zu_b.len(), pruef.len());

    for (i, &x) in pruef.iter().enumerate() {
        let a = abstand(&koerper, x);
        let b_m = v.a_zu_b[i] * 1e-3;
        assert!(
            nah(a.abstand.abs(), b_m) < 1e-12,
            "Punkt {i}: {:.9} gegen {:.9} m",
            a.abstand.abs(),
            b_m
        );
        let brute = netz.naechster_alle(x);
        assert!(
            nah(a.abstand, brute.abstand) < 1e-12,
            "Punkt {i}: Betrag/Vorzeichen {:.9} gegen {:.9} m",
            a.abstand,
            brute.abstand
        );
        let dd = (a.punkt[0] - brute.punkt[0]).powi(2)
            + (a.punkt[1] - brute.punkt[1]).powi(2)
            + (a.punkt[2] - brute.punkt[2]).powi(2);
        assert!(
            dd.sqrt() < 1e-9,
            "Punkt {i}: nächster Punkt {:.9} m daneben",
            dd.sqrt()
        );
    }
}

/// 5. Ohne Körper gibt es keine Karte — nicht eine Karte voller Nullen.
#[test]
fn ohne_koerper_keine_karte() {
    let mut s = roehre_auf_zylinder();
    s.koerper.clear();
    assert!(Karte::messe(&s).is_none());
}

/// 6. Die CSV trägt je Stoffpunkt eine Zeile mit Teil, Lage, Abstand und
/// Normale — und lässt sich ohne die Szene lesen.
#[test]
fn csv_hat_je_punkt_eine_zeile_mit_teil_und_abstand() {
    let s = roehre_auf_zylinder();
    let k = Karte::messe(&s).expect("Körper vorhanden");
    let t = k.csv(&s);
    let mut zeilen = t.lines();
    assert_eq!(
        zeilen.next(),
        Some("punkt;teil;x_cm;y_cm;z_cm;abstand_mm;nx;ny;nz"),
        "Kopfzeile"
    );
    let r: Vec<&str> = zeilen.collect();
    assert_eq!(r.len(), s.x.len(), "eine Zeile je Punkt");
    for (i, z) in r.iter().enumerate() {
        let f: Vec<&str> = z.split(';').collect();
        assert_eq!(f.len(), 9, "Zeile {i}: {}", f.len());
        assert_eq!(f[0].parse::<usize>().unwrap(), i, "Punktnummer");
        assert_eq!(f[1], s.teile[0].name, "Teilname");
        let d: f64 = f[5].parse().unwrap();
        assert!(
            nah(d, (ANFANGSABSTAND + HALBE_DICKE) * 1e3) < 1e-3,
            "Zeile {i}: {d} mm"
        );
    }
}

/// 7. Am Körper des Referenzprojekts (Hauptmaschine) — im **Kleidungsband**:
/// das Vorzeichen stimmt mit der Strahlparität überein, einem Orakel, das die
/// Pseudonormalen gar nicht kennt. Ohne die Daten übersprungen.
///
/// Geprüft wird nur zwischen 500 und 1450 mm Höhe, weil dort die Kleidung
/// liegt (das Kleid reicht bis 1440 mm; Befund 119). Außerhalb weicht das
/// Vorzeichen ab, und zwar an Stellen, an denen keine Kleidung hinkommt:
/// gemessen sind es drei Punkte am Kopf (z ≈ 1584–1589 mm, Nasenflügel,
/// Mundwinkel, Augenhöhle) und an den Zehen (z = −100 mm). Dort führt eine
/// Verschiebung um 5 mm entlang der Außennormale in eine Falte oder Höhle —
/// erst lokal außen, global innen. Die Parität selbst ist an solchen Stellen
/// nicht stabil (bei 1 mm Verschiebung kippt sie), deshalb ist das keine
/// Toleranzfrage, sondern eine Grenze des Verfahrens und der Grund für das
/// Band.
///
/// Gemessen wird in drei Strahlrichtungen; wo sie sich uneinig sind (Streifer
/// an einer Kante), wird der Punkt übersprungen und gezählt.
#[test]
fn vorzeichen_stimmt_am_koerper_im_kleidungsband() {
    let pfad = std::env::var("KLON3D_PROJEKT").unwrap_or_else(|_| {
        r"D:\ai\rea\work\messung\japanese_school_uniform\japanese_school_uniform.zprj".to_string()
    });
    let p = std::path::Path::new(&pfad);
    if !p.exists() {
        eprintln!("übersprungen: {} fehlt", p.display());
        return;
    }
    let projekt = match klon3d_pattern::zprj::Projekt::lies(p) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("übersprungen: {} nicht lesbar ({e})", p.display());
            return;
        }
    };
    let Some(k) = projekt.koerper else {
        eprintln!("übersprungen: {} ohne .avt-Körper", p.display());
        return;
    };
    let punkte: Vec<[f64; 3]> = k.punkte.iter().map(|q| [q.x, q.y, q.z]).collect();
    let dreiecke: Vec<[usize; 3]> = k
        .dreiecke
        .iter()
        .map(|d| [d[0] as usize, d[1] as usize, d[2] as usize])
        .collect();
    let netz = match Koerpernetz::aus_clo_mm(&punkte, &dreiecke, SUCHTIEFE) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("übersprungen: Körpernetz nicht baubar ({e})");
            return;
        }
    };
    let koerper = Koerper::netz(netz.clone());

    let mut geprueft = 0usize;
    let mut streifer = 0usize;
    // Außen und innen: Punkte 5 mm bzw. 5 mm gegen die Normale versetzt.
    for s in [5e-3, -5e-3] {
        for i in (0..netz.punkte.len()).step_by(211) {
            let n = netz.punktnormale[i];
            let q = netz.punkte[i];
            // Kleidungsband (z oben, m): darunter Schuhe, darüber Kopf.
            if !(0.5..=1.45).contains(&q[2]) {
                continue;
            }
            let x = [q[0] + n[0] * s, q[1] + n[1] * s, q[2] + n[2] * s];
            let Some(drin) = paritaet(&netz.punkte, &netz.dreiecke, x) else {
                streifer += 1;
                continue;
            };
            let gelesen = abstand(&koerper, x).abstand;
            assert!(
                (gelesen > 0.0) == !drin,
                "Punkt {i} bei z = {:.0} mm ({s:+.0e} m): gelesen {:+.3} mm, Parität sagt {}",
                q[2] * 1e3,
                gelesen * 1e3,
                if drin { "innen" } else { "außen" }
            );
            geprueft += 1;
        }
    }
    assert!(
        geprueft > 100,
        "nur {geprueft} Punkte geprüft, {streifer} Streifer"
    );

    // Und das Gegenstück, damit die Grenze nicht stillschweigend wandert:
    // außerhalb des Bands weicht das Vorzeichen bekannt ab. Findet dieser
    // Abschnitt **keine** Abweichung, ist entweder der Körper anders oder die
    // Abfrage wurde geändert — beides gehört in den PR, nicht in ein stilles
    // Weglassen.
    let mut abweichend = 0usize;
    for s in [5e-3, -5e-3] {
        for i in (0..netz.punkte.len()).step_by(211) {
            let n = netz.punktnormale[i];
            let q = netz.punkte[i];
            if (0.5..=1.45).contains(&q[2]) {
                continue;
            }
            let x = [q[0] + n[0] * s, q[1] + n[1] * s, q[2] + n[2] * s];
            let Some(drin) = paritaet(&netz.punkte, &netz.dreiecke, x) else {
                continue;
            };
            let gelesen = abstand(&koerper, x).abstand;
            if (gelesen > 0.0) == drin {
                abweichend += 1;
                eprintln!(
                    "  bekannt abweichend: Punkt {i} bei z = {:.0} mm, {s:+.0e} m",
                    q[2] * 1e3
                );
            }
        }
    }
    assert!(
        abweichend > 0,
        "außerhalb des Kleidungsbands weicht kein Punkt ab — dann stimmt die Begründung oben nicht mehr"
    );
}

/// Liegt `x` im geschlossenen Netz? Drei Strahlen in verschiedene Richtungen
/// gegen alle Dreiecke (Möller–Trumbore); `None`, wenn sie sich uneinig sind.
fn paritaet(punkte: &[[f64; 3]], dreiecke: &[[u32; 3]], x: [f64; 3]) -> Option<bool> {
    let richtungen = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut stimmen = [0usize; 2];
    for d in richtungen {
        match schnitte(punkte, dreiecke, x, d) {
            Some(n) => stimmen[n % 2] += 1,
            None => return None,
        }
    }
    // Alle drei gleich: das Ergebnis. Sonst Streifer.
    if stimmen[0] == 3 {
        Some(false)
    } else if stimmen[1] == 3 {
        Some(true)
    } else {
        None
    }
}

/// Zahl der Treffer des Strahls `x + t·d` (t > 0) mit dem Netz; `None` bei
/// einem Streifer (Strahl fast parallel zur Fläche).
fn schnitte(punkte: &[[f64; 3]], dreiecke: &[[u32; 3]], x: [f64; 3], d: [f64; 3]) -> Option<usize> {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let mut n = 0usize;
    for t in dreiecke {
        let (a, b, c) = (
            punkte[t[0] as usize],
            punkte[t[1] as usize],
            punkte[t[2] as usize],
        );
        let (e1, e2) = (sub(b, a), sub(c, a));
        let pv = cross(d, e2);
        let det = dot(e1, pv);
        if det.abs() < 1e-12 {
            return None; // parallel zur Fläche
        }
        let inv = 1.0 / det;
        let tv = sub(x, a);
        let u = dot(tv, pv) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let qv = cross(tv, e1);
        let v = dot(d, qv) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let tt = dot(e2, qv) * inv;
        // Ein Treffer genau auf der Fläche (|t| ≈ 0) wäre kein sauberer
        // Strahl; an einer Kante liegen zwei Treffer derselben Stelle.
        if tt.abs() < 1e-9 {
            return None;
        }
        if tt > 0.0 {
            n += 1;
        }
    }
    Some(n)
}
