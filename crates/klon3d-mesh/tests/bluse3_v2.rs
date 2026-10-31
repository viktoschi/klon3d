//! Abnahme Stufe 3, Teil 2 an der Bluse3 v2: Netz je Teil und Punktpaare je
//! Naht (Kriterien in `klon3d_mesh::pruefung`, abgestimmt am 20.10.2026).

use std::collections::HashMap;
use std::path::PathBuf;

use klon3d_mesh::{pruefung, vernetze, Musternetz, Parameter, Stellenart};
use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::naehte::{lies_nahtliste, pruefe_naehte};
use klon3d_pattern::read_path;

fn wurzel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../klon3d-pattern/tests/muster")
}

fn abstand(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Netz der Bluse und die Randkanten aus `klon3d-pattern` zum Gegenprüfen.
fn netz() -> (Musternetz, HashMap<String, Vec<[f64; 2]>>) {
    let m = read_path(&wurzel().join("bluse3_v2.sm2d")).unwrap();
    let ev = Auswertung::lade(&m, &wurzel().join("measurements.smis")).unwrap();
    let defs = lies_nahtliste(&std::fs::read_to_string(wurzel().join("bluse3_v2.naehte")).unwrap()).unwrap();
    let b = pruefe_naehte(&ev, &defs, Kurvenquelle::Seamly).unwrap();
    let netz = vernetze(&ev, Kurvenquelle::Seamly, &b.naehte, &Parameter::default()).unwrap();
    // Punktknoten je Teil in mm, unabhängig vom Netz gelesen.
    let mut knoten = HashMap::new();
    for t in m.pieces() {
        let r = ev.teilrand(t, Kurvenquelle::Seamly).unwrap();
        let mm = r.mm_pro_einheit;
        knoten.insert(r.teil.clone(), r.punktknoten.iter().map(|p| [r.umriss[p.stelle].x * mm, r.umriss[p.stelle].y * mm]).collect());
    }
    (netz, knoten)
}

#[test]
fn abnahme_netz_bluse3_v2() {
    let (netz, knoten) = netz();
    assert_eq!(netz.teile.len(), 28);
    assert!(netz.fehler.is_empty(), "{:?}", netz.fehler);
    assert_eq!(netz.offene_teilungen, 0);
    let p = netz.parameter;
    let b = pruefung::pruefe(&netz);
    let v = b.verstoesse(&p);
    assert!(v.is_empty(), "{v:#?}");

    for (t, tp) in netz.teile.iter().zip(&b.teile) {
        // Netzrand auf 0,01 cm, Mindestwinkel ohne Ausnahme (keine Ecke unter 53°).
        assert!(tp.struktur.is_empty(), "{}: {:?}", t.teil, tp.struktur);
        assert!(tp.min_winkel >= 25.0 && tp.spitze_ecken == 0, "{}: {:.2}°", t.teil, tp.min_winkel);
        assert!(tp.sehne_max <= 0.1 && tp.laenge_max <= 0.1, "{}: {} / {}", t.teil, tp.sehne_max, tp.laenge_max);
        // Fläche: Netz = Randpolygon; gegen die Nahtlinie fehlen nur die Sehnen.
        assert!(tp.flaeche_rand_diff.abs() < 1e-6 * tp.flaeche_netz);
        let u: f64 = t.kanten.iter().map(|k| k.laenge).sum();
        assert!((tp.flaeche_netz - tp.flaeche_kontur).abs() <= 2.0 / 3.0 * p.sehne_mm * u, "{}", t.teil);
        // Jeder Punktknoten ist ein Netzpunkt an seiner Stelle.
        let soll = &knoten[&t.teil];
        assert_eq!(t.punktknoten.len(), soll.len(), "{}", t.teil);
        for ((name, v), q) in t.punktknoten.iter().zip(soll) {
            assert!(abstand(t.punkte[*v as usize], *q) < 1e-9, "{} {name}", t.teil);
        }
        // Kantenlänge etwa 10 mm an allen Teilen, die größer als die Scheiben sind.
        if tp.flaeche_netz > 2000.0 {
            assert!((9.0..=11.0).contains(&tp.kante_median), "{}: Median {:.2}", t.teil, tp.kante_median);
            assert!(tp.kante_max <= 18.0, "{}: {:.2}", t.teil, tp.kante_max);
        }
    }
    // Links und rechts gleich viele Randpunkte.
    for t in netz.teile.iter().filter(|t| t.teil.ends_with(" Links")) {
        let r = netz.teil(&t.teil.replace(" Links", " Rechts")).unwrap();
        assert_eq!(t.nrand, r.nrand, "{}", t.teil);
    }
}

#[test]
fn abnahme_nahtaufteilung_bluse3_v2() {
    let (netz, _) = netz();
    assert_eq!(netz.naehte.len(), 24);
    let b = pruefung::pruefe(&netz);
    let teilnaehte: HashMap<&str, usize> = netz.naehte.iter().map(|n| (n.kennung.as_str(), n.teilnaehte.len())).collect();
    // 1:1 eine Teilnaht; N03 2:2 (gemeinsame Grenze), N04 1:3, N05 2:2, N13 4:1.
    for (k, n) in [("N01", 1), ("N03L", 2), ("N04L", 3), ("N05L", 2), ("N07L", 1), ("N13", 4)] {
        assert_eq!(teilnaehte[k], n, "{k}");
    }
    for (n, np) in netz.naehte.iter().zip(&b.naehte) {
        assert!(np.struktur.is_empty(), "{}: {:?}", n.kennung, np.struktur);
        assert!(np.punkte.iter().all(|(a, b)| a == b), "{}: {:?}", n.kennung, np.punkte);
        // Zusammengelegte Kerben: höchstens 0,26 mm von der Anteilsregel (N02 RTM2/RTS2).
        assert!(np.versatz_max < 0.26, "{}: {}", n.kennung, np.versatz_max);
        // Gepaarte Randstücke gleich lang bis 0,1 mm (N07: 0,74 mm auf 30 cm verteilt).
        assert!(np.stueck_diff_max < 0.1, "{}: {}", n.kennung, np.stueck_diff_max);
        // Längenanteile unabhängig nachgemessen: Sehnen entlang der Paare.
        for tn in &n.teilnaehte {
            for (seite, ti, l) in [('A', tn.teil_a, n.laenge_a_mm), ('B', tn.teil_b, n.laenge_b_mm)] {
                let t = &netz.teile[ti];
                let punkt = |q: &klon3d_mesh::Punktpaar| t.punkte[if seite == 'A' { q.a } else { q.b } as usize];
                let rho = |q: &klon3d_mesh::Punktpaar| if seite == 'A' { q.rho_a } else { q.rho_b };
                let mut weg = 0.0;
                for w in tn.paare.windows(2) {
                    weg += abstand(punkt(&w[0]), punkt(&w[1]));
                    let soll = (rho(&w[1]) - rho(&tn.paare[0])) * l;
                    assert!((weg - soll).abs() < 0.2, "{} {seite}: {weg:.3} gegen {soll:.3}", n.kennung);
                }
            }
            for q in &tn.paare {
                assert!((q.rho_a - q.rho_b).abs() * n.laenge_a_mm < 0.26, "{}", n.kennung);
            }
        }
    }
    // Die Taillenkerbe hinten (RW) hat vorn kein Gegenstück: B bekommt einen eigenen Punkt.
    let n04 = netz.naehte.iter().find(|n| n.kennung == "N04L").unwrap();
    let rw = n04.stellen.iter().find(|s| s.name_a == "RW_L").unwrap();
    assert_eq!(rw.art, Stellenart::NurA);
    assert!(n04.stellen.iter().any(|s| s.art == Stellenart::Gemeinsam && s.name_a == "RSN1_L"));
}
