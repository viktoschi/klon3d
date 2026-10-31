//! Stufe 3: Randkanten und Nahtpaare an der Bluse3 v2.
//!
//! Abnahme (PLAN.md, Stufe 3): alle Nahtpaare gleich lang, |Δ| ≤ 0,08 cm.
//! Die Nahtliste `tests/muster/bluse3_v2.naehte` stammt aus der Soll-Liste
//! `work/naehte/bluse3_v2_soll.json` (20.10.2026, `soll_naehte.py` über
//! `schnitt.py`); die Sollängen unten sind deren `laenge_zug` — gemessen auf
//! Seamly2Ds eigener Kurvenzerlegung, auf vier Stellen gerundet.

mod muster;

use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::naehte::{lies_nahtliste, pruefe_naehte, Nahtbericht, TOLERANZ_CM};
use klon3d_pattern::read_path;

/// (Kennung, L_A, L_B) in cm, Seamly-Zerlegung.
const SOLL: &[(&str, f64, f64)] = &[
    ("N01", 42.8571, 42.8571),
    ("N02L", 43.6019, 43.6358),
    ("N03L", 10.5380, 10.5380),
    ("N04L", 23.8754, 23.8747),
    ("N05L", 40.0186, 40.0187),
    ("N06L", 8.6449, 8.6449),
    ("N07L", 29.6060, 29.6796),
    ("N08L", 15.1800, 15.1800),
    ("N09L", 10.8617, 10.8617),
    ("N10L", 8.2656, 8.2663),
    ("N11L", 16.2145, 16.2145),
    ("N12L", 6.3602, 6.3668),
    ("N02R", 43.6019, 43.6358),
    ("N03R", 10.5380, 10.5380),
    ("N04R", 23.8754, 23.8747),
    ("N05R", 40.0186, 40.0187),
    ("N06R", 8.6449, 8.6449),
    ("N07R", 29.6060, 29.6796),
    ("N08R", 15.1800, 15.1800),
    ("N09R", 10.8617, 10.8617),
    ("N10R", 8.2656, 8.2663),
    ("N11R", 16.2145, 16.2145),
    ("N12R", 6.3602, 6.3668),
    ("N13", 37.6326, 37.6326),
];

fn bericht(quelle: Kurvenquelle) -> Nahtbericht {
    let m = read_path(&muster::muster_pfad("bluse3_v2")).unwrap();
    let ev = Auswertung::lade(&m, &muster::mess_pfad()).unwrap();
    let text = std::fs::read_to_string(muster::wurzel().join("bluse3_v2.naehte")).unwrap();
    let defs = lies_nahtliste(&text).unwrap();
    pruefe_naehte(&ev, &defs, quelle).unwrap()
}

#[test]
fn abnahme_stufe3_bluse3_v2() {
    let b = bericht(Kurvenquelle::Seamly);
    assert_eq!(b.naehte.len(), SOLL.len());
    for (n, (kennung, la, lb)) in b.naehte.iter().zip(SOLL) {
        assert_eq!(n.kennung, *kennung);
        // Sollwerte sind auf 4 Stellen gerundet.
        assert!(
            (n.a.laenge_cm - la).abs() < 1.5e-4 && (n.b.laenge_cm - lb).abs() < 1.5e-4,
            "{kennung}: {:.5} / {:.5} gegen Soll {la} / {lb}",
            n.a.laenge_cm,
            n.b.laenge_cm
        );
    }
    let g = b.groesste_abweichung().unwrap();
    assert!(g.kennung.starts_with("N07"), "größte Abweichung bei {}", g.kennung);
    assert!(b.bestanden(TOLERANZ_CM), "über Toleranz: {:?}", b.ueber(TOLERANZ_CM).iter().map(|n| &n.kennung).collect::<Vec<_>>());
}

#[test]
fn abdeckung_und_passzeichen() {
    let b = bericht(Kurvenquelle::Seamly);
    assert!(b.doppelt.is_empty());
    assert!(b.teile_fehler.is_empty(), "{:?}", b.teile_fehler);
    assert!(b.genaeht_ohne_zugabe.is_empty());
    // Nur die Reserve am Innenkragen hat Zugabe ohne Gegenstück (Soll-Liste,
    // Unklarheit 3).
    let ohne: Vec<String> =
        b.zugabe_ohne_naht.iter().map(|h| format!("{} {}", h.von, h.bis)).collect();
    assert_eq!(ohne, ["KUR KUR0", "KUL0 KUL"]);
    assert!(b.zugabe_ohne_naht.iter().all(|h| h.teil.ends_with("Stehkragen Innen")));
    // Die Taillenkerbe hinten hat vorn kein Gegenstück (Unklarheit 2).
    let allein: Vec<(String, String)> = b
        .naehte
        .iter()
        .flat_map(|n| n.kerben_allein.iter().map(move |(_, m)| (n.kennung.clone(), m.name.clone())))
        .collect();
    assert_eq!(allein, [("N04L".into(), "RW_L".into()), ("N04R".into(), "RW_R".into())]);
    // Größte Passzeichenabweichung: VC6/VC5 an der Teilungsnaht vorn.
    let n07 = b.naehte.iter().find(|n| n.kennung == "N07L").unwrap();
    let k = n07.groesste_kerbabweichung().unwrap();
    assert_eq!((k.a.name.as_str(), k.b.name.as_str()), ("VC6_L", "VC5_L"));
    assert!((k.diff_cm + 0.0747).abs() < 2e-4, "{}", k.diff_cm);
    // Halsnaht: die drei Grenzen des Halslochs treffen die drei Kragenkerben.
    let n13 = b.naehte.iter().find(|n| n.kennung == "N13").unwrap();
    assert_eq!(n13.kerbpaare.len(), 3);
    assert!(n13.kerbpaare.iter().all(|p| p.diff_cm.abs() < 1e-3));
}

/// Mit der gezeichneten Polylinie kommen die Zahlen von `pruef_bluse3.py`
/// (v2, vier Stellen) heraus. (`schnitt.py piece_outline` schneidet an der
/// letzten Strecke und läuft an VJMN 0,0026 cm über den Punkt hinaus und
/// zurück: N12L A 6,3618; Seamly2D entfernt solche Rückläufe.)
#[test]
fn zeichnung_wie_pruef_bluse3() {
    let b = bericht(Kurvenquelle::Zeichnung);
    let laenge = |k: &str| {
        let n = b.naehte.iter().find(|n| n.kennung == k).unwrap();
        (n.a.laenge_cm, n.b.laenge_cm)
    };
    for (k, la, lb) in [
        ("N07L", 29.6036, 29.6794),
        ("N08L", 15.1800, 15.1800),
        ("N09L", 10.8617, 10.8617),
        ("N10L", 8.2656, 8.2663),
        ("N12L", 6.3567, 6.3642),
    ] {
        let (a, b) = laenge(k);
        assert!((a - la).abs() < 1.5e-4 && (b - lb).abs() < 1.5e-4, "{k}: {a:.5} / {b:.5}");
    }
    // Vorderseite der Seitennaht (Oberteil seitlich + Untercup + Band).
    assert!((laenge("N04L").1 - 23.8743).abs() < 1.5e-4);
}

/// Die Zeichnung ist je Kante höchstens wenig kürzer als Seamlys Zerlegung,
/// länger nur um Rundung. Ein verfehlter Schnitt an einer Kerbe (Zickzack, bis 38 cm
/// am Bischofärmel vor dem 20.10.2026) fiele hier auf.
#[test]
fn zeichnung_ohne_zickzack() {
    for name in ["bluse2_v26", "bluse2_v29", "bluse3_v1", "bluse3_v2", "bustier_v1", "rock_2schlitze_v34"] {
        let m = read_path(&muster::muster_pfad(name)).unwrap();
        let ev = Auswertung::lade(&m, &muster::mess_pfad()).unwrap();
        for t in m.pieces() {
            let (Ok(s), Ok(z)) = (ev.teilrand(t, Kurvenquelle::Seamly), ev.teilrand(t, Kurvenquelle::Zeichnung)) else {
                continue;
            };
            assert_eq!(s.kanten.len(), z.kanten.len(), "{name} {}", s.teil);
            for (ks, kz) in s.kanten.iter().zip(&z.kanten) {
                let d = s.in_cm(kz.laenge - ks.laenge);
                assert!((-0.05..=1e-3).contains(&d), "{name} {}: Zeichnung − Seamly {d:+.4} cm", s.teil);
            }
        }
    }
}

/// Randkanten: die Summe der Kanten ist der Umfang der Nahtlinie.
#[test]
fn kanten_ergeben_den_umriss() {
    let m = read_path(&muster::muster_pfad("bluse3_v2")).unwrap();
    let ev = Auswertung::lade(&m, &muster::mess_pfad()).unwrap();
    for t in m.pieces() {
        let r = ev.teilrand(t, Kurvenquelle::Zeichnung).unwrap();
        if r.kanten.is_empty() {
            continue;
        }
        let mut ring = r.umriss.clone();
        ring.push(ring[0]);
        let umfang = ev.umriss_laenge(&ring);
        assert!((r.umfang() - umfang).abs() < 1e-9, "{}: {} gegen {}", r.teil, r.umfang(), umfang);
    }
}
