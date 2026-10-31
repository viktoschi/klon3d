//! Der implizite Schritt mit CG (wie CLO) gegen die XPBD-Referenz an der
//! Röhre (Befund 35). Nur mit `cargo test --release`.

use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{messung, CpuReferenz, ImplizitCg, ImplizitParameter, Rechenweg, RADICAL_RUBBER_04, RETIFIST_NR_040};

/// Rückwärts-Euler legt in N Schritten der Länge h unter konstanter
/// Beschleunigung a den Weg a·h²·N(N+1)/2 zurück (½·a·t² + ½·a·h·t).
fn weg_diskret(a: f64, h: f64, t: f64) -> f64 {
    let n = (t / h).round();
    a * h * h * n * (n + 1.0) / 2.0
}

/// Freier Fall (ohne Reduktion liegt die Röhre lose an) und Rutschen nach
/// Coulomb — mit CLOs 50 CG-Iterationen, dank Warmstart.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn rutschprobe_wie_clo_mit_50_cg() {
    let h = 1.0 / 60.0;
    let frei = Roehre { reduktion: 0.0, ..Roehre::default() };
    let p = frei.rutschprobe(&mut ImplizitCg::neu(ImplizitParameter::default()), 0.5).unwrap();
    let soll = weg_diskret(9.81 * (1.0 - p.halteverhaeltnis_gemessen), h, p.sekunden);
    assert!((p.weg_m / soll - 1.0).abs() < 0.005, "freier Fall {:.1} mm gegen {:.1} mm", p.weg_m * 1e3, soll * 1e3);

    let halten = Roehre::default();
    let p = halten.rutschprobe(&mut ImplizitCg::neu(ImplizitParameter::default()), 0.5).unwrap();
    assert!(p.halteverhaeltnis_gemessen > 1.5);
    assert!(p.weg_m < 1e-3, "Radical 5,5 % rutscht {:.2} mm", p.weg_m * 1e3);

    let rutschen = Roehre { material: RETIFIST_NR_040, ..Roehre::default() };
    let p = rutschen.rutschprobe(&mut ImplizitCg::neu(ImplizitParameter::default()), 0.5).unwrap();
    let soll = weg_diskret(9.81 * (1.0 - p.halteverhaeltnis_gemessen), h, p.sekunden);
    assert!((p.weg_m / soll - 1.0).abs() < 0.02, "Retifist {:.1} mm gegen {:.1} mm", p.weg_m * 1e3, soll * 1e3);
}

/// Die Röhre im Gleichgewicht: dieselbe Abnahme wie die Referenz (Ruhe,
/// Umfangsdehnung ±0,5 %-Punkte, liegt an), Naht mit CLOs Nahtfeder.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn roehre_im_gleichgewicht() {
    for m in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        for red in [0.055, 0.10] {
            let r = Roehre { material: m, reduktion: red, ..Roehre::default() };
            let mut s = r.szene(param_ruhe()).unwrap();
            let l = bis_zur_ruhe(&mut s, &mut ImplizitCg::neu(ImplizitParameter::default()), 600, 1e-4, 10);
            let e = r.auswerten(&s);
            let was = format!("{} {:.1} %", m.name, red * 100.0);
            assert!(l.ruhig, "{was}: {l:?}");
            assert!((e.dehnung - e.dehnung_soll).abs() <= 0.005, "{was}: ε {:.4}", e.dehnung);
            assert!(e.abheben_m < 0.5e-3, "{was}: abheben {:.3} mm", e.abheben_m * 1e3);
            // Nahtfeder 500 N/m (CLO): der Spalt wächst mit der Ringspannung.
            assert!(e.nahtspalt_m < 1.0e-3, "{was}: Naht {:.3} mm", e.nahtspalt_m * 1e3);
        }
    }
}

/// Ohne Reibung finden beide Rechenwege dasselbe Gleichgewicht des
/// Federnetzes; die unterschiedliche Verteilung mit Reibung kommt vom Weg
/// (die Haftreibung friert verschiedene Zwischenzustände ein).
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn ohne_reibung_dasselbe_gleichgewicht() {
    let r = Roehre { material: RETIFIST_NR_040, ..Roehre::default() };
    let mut ergebnis = Vec::new();
    let wege: Vec<Box<dyn Rechenweg>> = vec![
        Box::new(CpuReferenz::neu()),
        Box::new(ImplizitCg::neu(ImplizitParameter { cg_max: 3000, cg_toleranz: 1e-8, naht_k: 3e4, ..Default::default() })),
    ];
    for mut rw in wege {
        let mut s = r.szene(param_ruhe()).unwrap();
        for x in s.reibung.iter_mut() {
            *x = 0.0;
        }
        let l = bis_zur_ruhe(&mut s, rw.as_mut(), 1200, 1e-5, 10);
        assert!(l.ruhig);
        ergebnis.push(r.auswerten(&s));
    }
    let (x, i) = (ergebnis[0], ergebnis[1]);
    assert!((x.dehnung_median - i.dehnung_median).abs() < 2e-4, "Median {} gegen {}", x.dehnung_median, i.dehnung_median);
    assert!((x.dehnung_p95 - i.dehnung_p95).abs() < 1e-3, "P95 {} gegen {}", x.dehnung_p95, i.dehnung_p95);
}

/// In Ruhe ist die rechte Seite fast null; das CG darf dann nicht im
/// Rundungsrauschen weiterlaufen (vor dem Boden im Abbruchkriterium explodierte
/// die reibungsfreie Röhre nach 10 ruhigen Bildern).
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn cg_bleibt_in_ruhe_stabil() {
    let r = Roehre::default();
    let mut s = r.szene(param_ruhe()).unwrap();
    for x in s.reibung.iter_mut() {
        *x = 0.0;
    }
    let mut rw = ImplizitCg::neu(ImplizitParameter { cg_max: 2000, cg_toleranz: 1e-8, ..Default::default() });
    for _ in 0..60 {
        rw.bild(&mut s);
        let v = messung::mittlere_geschwindigkeit(&s);
        assert!(v.is_finite() && v < 1.0, "v {v}");
    }
}
