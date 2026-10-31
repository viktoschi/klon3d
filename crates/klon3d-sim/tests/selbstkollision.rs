//! Selbstkollision Stoff gegen Stoff nach CLO (Befund 116): zwei Lagen
//! übereinander. Nur mit `cargo test --release`.

use std::collections::HashMap;

use klon3d_mesh::{vernetze, Parameter};
use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::read_str;
use klon3d_sim::roehre::rechteck_muster;
use klon3d_sim::{ImplizitCg, ImplizitParameter, Rechenweg, Simparameter, Szene, Teilvorgabe, RADICAL_RUBBER_04};

/// Zwei Quadrate 20 × 20 cm (Kanten etwa 10 mm), das untere bei z = 0
/// festgehalten, das obere 10 mm darüber frei.
fn zwei_lagen() -> Szene {
    let m = read_str(&rechteck_muster(0.2, 0.2)).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    let p = Parameter { kante_mm: 10.0, ..Parameter::default() };
    let mut netz = vernetze(&ev, Kurvenquelle::Seamly, &[], &p).unwrap();
    assert!(netz.fehler.is_empty(), "{:?}", netz.fehler);
    let oben = netz.teile[0].clone();
    netz.teile.push(oben);
    let vg = Teilvorgabe { material: RADICAL_RUBBER_04, reduktion: 0.0 };
    let lage = |ti: usize, q: [f64; 2]| [q[0] * 1e-3, q[1] * 1e-3, if ti == 0 { 0.0 } else { 0.01 }];
    let param = Simparameter { bild_dt: 1.0 / 30.0, daempfung: 1.0, ..Simparameter::default() };
    let mut s = Szene::aus_netz(&netz, &|_| vg, &lage, Vec::new(), param);
    for i in s.teile[0].punkte.clone() {
        s.w[i] = 0.0;
    }
    s
}

fn tiefster_oben(s: &Szene) -> f64 {
    s.teile[1].punkte.clone().map(|i| s.x[i][2]).fold(f64::INFINITY, f64::min)
}

/// Ohne Selbstkollision fällt die obere Lage durch, mit liegt sie auf
/// (Ruheabstand r₁ + r₂ = 0,4 mm + 2,5 mm Zusatzdicke = 2,9 mm), ohne einen
/// Punkt unter der unteren Lage. Gemessen 30.10.2026: 2,66…2,90 mm über der
/// unteren Lage (durchhängend zwischen den Paaren), 0 Stöße.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn obere_lage_bleibt_liegen() {
    let mut s = zwei_lagen();
    let mut rw = ImplizitCg::neu(ImplizitParameter::default());
    for _ in 0..30 {
        rw.bild(&mut s);
    }
    assert!(tiefster_oben(&s) < -0.05, "ohne Selbstkollision {:.1} mm", tiefster_oben(&s) * 1e3);

    let mut s = zwei_lagen();
    let mut rw = ImplizitCg::neu(ImplizitParameter { selbstkollision: true, ..ImplizitParameter::default() });
    let mut paare = 0;
    for _ in 0..60 {
        rw.bild(&mut s);
        paare = rw.statistik.naehe_punkt_dreieck + rw.statistik.naehe_kante_kante;
    }
    let z = tiefster_oben(&s);
    let hoechster = s.teile[1].punkte.clone().map(|i| s.x[i][2]).fold(f64::NEG_INFINITY, f64::max);
    assert!(paare > 0);
    assert!(z > 2.0e-3 && hoechster < 3.2e-3, "liegt bei {:.2}…{:.2} mm", z * 1e3, hoechster * 1e3);
}

/// Die obere Lage startet schräg durch die untere (z von −5 mm auf +15 mm
/// über 20 cm). Die Durchdringungs-Stufe schiebt das Schnittgebiet als
/// Ganzes zurück: nach 3 s sticht keine Kante mehr durch die andere Lage.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn schraege_lage_loest_sich() {
    let m = read_str(&rechteck_muster(0.2, 0.2)).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    let p = Parameter { kante_mm: 10.0, ..Parameter::default() };
    let mut netz = vernetze(&ev, Kurvenquelle::Seamly, &[], &p).unwrap();
    let oben = netz.teile[0].clone();
    netz.teile.push(oben);
    let vg = Teilvorgabe { material: RADICAL_RUBBER_04, reduktion: 0.0 };
    let lage = |ti: usize, q: [f64; 2]| {
        let (x, y) = (q[0] * 1e-3, q[1] * 1e-3);
        [x, y, if ti == 0 { 0.0 } else { -0.005 + 0.1 * x }]
    };
    let param = Simparameter { bild_dt: 1.0 / 30.0, daempfung: 1.0, ..Simparameter::default() };
    let mut s = Szene::aus_netz(&netz, &|_| vg, &lage, Vec::new(), param);
    for i in s.teile[0].punkte.clone() {
        s.w[i] = 0.0;
    }
    let (sn, _) = klon3d_sim::selbstkollision::aus_szene(&s, 2.5e-3);
    let anfang = klon3d_sim::selbstkollision::finde_schnitte(&sn, &s.x, true).len();
    assert!(anfang > 10, "{anfang} Schnitte am Anfang");
    let mut rw = ImplizitCg::neu(ImplizitParameter { selbstkollision: true, ..ImplizitParameter::default() });
    for _ in 0..90 {
        rw.bild(&mut s);
    }
    let ende = klon3d_sim::selbstkollision::finde_schnitte(&sn, &s.x, true).len();
    assert_eq!(ende, 0, "{anfang} Schnitte am Anfang, {ende} am Ende");
}
