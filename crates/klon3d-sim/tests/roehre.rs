//! Abnahme von Schritt 4a (PLAN.md, Stufe 4): die Röhre.
//!
//! Mit der Anwenderin abgestimmt am 20.10.2026: beide Stoffe (Radical Rubber
//! 0,4 mm gemessen, Retifist NR-sheet-0.40 gesetzt), Reduktion 0 / 5,5 / 10 %,
//! Umfangsdehnung ±0,5 %-Punkte gegen Zylinderumfang ÷ reduzierten Umfang,
//! Naht bündig. Dazu: kommt zur Ruhe, liegt an, Naht klafft nicht (< 0,5 mm),
//! zwei Netzfeinheiten geben dasselbe, und die Rutschprobe folgt Coulomb.
//!
//! Die Läufe sind im Debug-Bau zu langsam (bis 800 Unterschritte je Bild);
//! sie laufen mit `cargo test --release`.

use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Ergebnis, Lauf, Roehre};
use klon3d_sim::{CpuReferenz, Material, RADICAL_RUBBER_04, RETIFIST_NR_040};

/// Umfangsdehnung gegen die Rechnung (Anteil, ±0,5 %-Punkte).
const TOL_DEHNUNG: f64 = 0.005;
/// Nahtspalt (m).
const TOL_NAHT: f64 = 0.5e-3;
/// Abheben von der Sollfläche, wenn Spannung anliegt (m).
const TOL_ABHEBEN: f64 = 0.5e-3;

fn lauf(material: Material, reduktion: f64, kante_mm: f64, spalt: bool) -> (Lauf, Ergebnis) {
    let r = Roehre { material, reduktion, kante_mm, anfang_mit_spalt: spalt, ..Roehre::default() };
    let mut s = r.szene(param_ruhe()).unwrap();
    let mut rw = CpuReferenz::neu();
    let l = bis_zur_ruhe(&mut s, &mut rw, 1200, 1e-4, 10);
    (l, r.auswerten(&s))
}

#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn abnahme_beide_stoffe_drei_reduktionen() {
    for m in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        for red in [0.0, 0.055, 0.10] {
            let (l, e) = lauf(m, red, 10.0, false);
            let was = format!("{} {:.1} %", m.name, red * 100.0);
            assert!(l.ruhig, "{was}: nicht zur Ruhe ({l:?})");
            assert!(
                (e.dehnung - e.dehnung_soll).abs() <= TOL_DEHNUNG,
                "{was}: ε {:.4} % gegen Soll {:.4} %",
                e.dehnung * 100.0,
                e.dehnung_soll * 100.0
            );
            assert!(e.nahtspalt_m < TOL_NAHT, "{was}: Naht {:.3} mm", e.nahtspalt_m * 1e3);
            // Ohne Reduktion liegt die Röhre lose an: nur der Sehnenüberschuss
            // der 2D-Kanten hebt sie ab (unter 0,5 mm, seit die Naht Biegung hat).
            assert!(e.abheben_m < TOL_ABHEBEN, "{was}: hebt {:.3} mm ab", e.abheben_m * 1e3);
            if red > 0.0 {
                assert!(e.abheben_m < TOL_ABHEBEN, "{was}: hebt {:.3} mm ab", e.abheben_m * 1e3);
                // Die Hälfte der Dreiecke liegt nahe der Rechnung.
                assert!((e.dehnung_median - e.dehnung_soll).abs() < 0.002, "{was}: Median {:.4}", e.dehnung_median);
            }
        }
    }
}

/// 10 mm und 5 mm Netz: Umfangsdehnung und Median auf 0,1 %-Punkte gleich.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn netzfeinheit_aendert_nichts() {
    let (l10, e10) = lauf(RADICAL_RUBBER_04, 0.055, 10.0, false);
    let (l5, e5) = lauf(RADICAL_RUBBER_04, 0.055, 5.0, false);
    assert!(l10.ruhig && l5.ruhig);
    assert!(e5.punkte > 3 * e10.punkte, "{} gegen {} Punkte", e5.punkte, e10.punkte);
    assert!((e10.dehnung - e5.dehnung).abs() < 0.001, "{} gegen {}", e10.dehnung, e5.dehnung);
    assert!((e10.dehnung_median - e5.dehnung_median).abs() < 0.001);
    // Das feinere Netz liegt näher an der Rechnung (Sehnenfehler des Rings).
    assert!((e5.dehnung - e5.dehnung_soll).abs() < (e10.dehnung - e10.dehnung_soll).abs());
}

/// Nahtprobe: mit 31 mm offenem Spalt beginnen; die Naht schließt sich.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn nahtprobe_spalt_schliesst_sich() {
    for m in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        let (l, e) = lauf(m, 0.055, 10.0, true);
        assert!(l.ruhig, "{}", m.name);
        assert!(e.nahtspalt_m < TOL_NAHT, "{}: Naht {:.3} mm", m.name, e.nahtspalt_m * 1e3);
        assert!((e.dehnung - e.dehnung_soll).abs() <= TOL_DEHNUNG);
        assert!(e.dehnung_p95 < 0.08, "{}: P95 {:.4} — Dehnung an der Naht gestaut?", m.name, e.dehnung_p95);
    }
}

/// Rutschprobe: Radical hält bei 5,5 % (Coulomb-Reserve 2,08), Retifist
/// rutscht (0,18 bzw. 0,37) mit a = g − μN/M; ohne Reduktion freier Fall.
/// Verglichen wird mit der Normallast, die im Ruhezustand anliegt.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn rutschprobe_folgt_coulomb() {
    let halten = Roehre::default();
    let p = halten.rutschprobe(&mut CpuReferenz::neu(), 0.5).unwrap();
    assert!(p.halteverhaeltnis > 1.5 && p.halteverhaeltnis_gemessen > 1.5);
    assert!(p.weg_m < 1e-3, "Radical 5,5 % rutscht {:.2} mm", p.weg_m * 1e3);

    for red in [0.055, 0.10] {
        let r = Roehre { material: RETIFIST_NR_040, reduktion: red, ..Roehre::default() };
        let p = r.rutschprobe(&mut CpuReferenz::neu(), 0.5).unwrap();
        assert!(p.halteverhaeltnis_gemessen < 0.5);
        // analytisch und gemessen auf wenige Prozent gleich
        assert!((p.halteverhaeltnis_gemessen / p.halteverhaeltnis - 1.0).abs() < 0.05, "{p:?}");
        let rel = p.weg_m / p.weg_soll_m - 1.0;
        assert!(rel.abs() < 0.01, "Retifist {:.1} %: {:.1} mm gegen Coulomb {:.1} mm", red * 100.0, p.weg_m * 1e3, p.weg_soll_m * 1e3);
    }

    let frei = Roehre { reduktion: 0.0, ..Roehre::default() };
    let p = frei.rutschprobe(&mut CpuReferenz::neu(), 0.5).unwrap();
    assert!((p.weg_m / p.weg_soll_m - 1.0).abs() < 0.005, "freier Fall {:.1} mm gegen {:.1} mm", p.weg_m * 1e3, p.weg_soll_m * 1e3);
}

/// Die Weichheitsbedingung: Radical braucht mehr Unterschritte als Retifist
/// (rund √10 so viele bei ähnlicher Masse). Mit nur 20 Unterschritten (steifer
/// Bereich) wirken die Federn mit einem Bruchteil ihrer Kraft gegen die
/// physikalische Haftreibung, und die Dehnung friert ungleich ein — der Grund
/// für die Bedingung (Befund 34).
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn weichheitsbedingung() {
    let r = Roehre::default();
    let rad = r.szene(param_ruhe()).unwrap();
    let ret = Roehre { material: RETIFIST_NR_040, ..Roehre::default() }.szene(param_ruhe()).unwrap();
    let q = rad.param.unterschritte as f64 / ret.param.unterschritte as f64;
    assert!((2.5..4.0).contains(&q), "{} gegen {}", rad.param.unterschritte, ret.param.unterschritte);
    assert!(rad.param.unterschritte > 100);

    let mut steif = r.szene_roh(param_ruhe()).unwrap();
    assert_eq!(steif.param.unterschritte, 20);
    bis_zur_ruhe(&mut steif, &mut CpuReferenz::neu(), 1200, 1e-4, 10);
    let e = r.auswerten(&steif);
    assert!(e.dehnung_p95 > 0.10, "steif: P95 {:.4} — erwartet gestaute Dehnung", e.dehnung_p95);
}
