//! Acceptance for glue zones: a bonded layer carries the other one with it.
//!
//! Two identical layers deform identically under the same load, so comparing
//! their deflection cannot show what glue does - both cases look the same. What
//! glue does is **transfer load**: displace one layer and the other follows,
//! because the bond holds the offset instead of letting the layers slide.
//!
//! The case: two 0.4 mm layers of latex, 20 × 4 cm, no gravity. The lower layer
//! is clamped along one short edge. The upper layer's free end is pushed 5 mm
//! sideways and held there. Then the lower layer is bent along - with a glue
//! zone the lower layer's free end is pulled along, without one it stays.
//!
//! The second test records why the deflection comparison from the task
//! description does **not** work here, with numbers instead of an assertion of
//! faith. At these dimensions the sag of a strip held at both ends is carried
//! by membrane tension, not by bending: two loose layers in parallel carry
//! 2 × 348 N/m and the laminate carries 696 N/m, so all three cases sag alike.
//! The transverse (= sliding) stiffness is what separates them.
//!
//! Only with `cargo test --release`.

use std::collections::HashMap;

use klon3d_mesh::{vernetze, Parameter};
use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::read_str;
use klon3d_sim::klebung::{Stoffpaar, Zone};
use klon3d_sim::roehre::rechteck_muster;
use klon3d_sim::{ImplizitCg, ImplizitParameter, Rechenweg, Simparameter, Szene, Teilvorgabe, Material, RADICAL_RUBBER_04};

/// The laminate fabric of two 0.4 mm layers, from MATERIAL.md: 0.8 mm,
/// 698 g/m², stretch 696,548, shear 348,318, bending 19,858 (in N·m²:
/// 19,858·10⁻⁹).
const LAMINAT_08: Material = Material {
    name: "Latex_Laminat_2x0.4mm",
    dehnung_n_m: 696.548,
    schub_n_m: 348.318,
    biegung_nm: 2482.2424e-9 * 8.0,
    flaechengewicht_kg_m2: 0.698,
    dicke_m: 0.8e-3,
    reibung: 0.03,
};

/// Two layers 20 × 4 cm on top of each other, 0.4 mm apart, no gravity.
/// The lower one is held along the short edge x = 0.
fn zwei_lagen(geklebt: bool) -> Szene {
    let m = read_str(&rechteck_muster(0.2, 0.04)).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    let p = Parameter { kante_mm: 10.0, ..Parameter::default() };
    let mut netz = vernetze(&ev, Kurvenquelle::Seamly, &[], &p).unwrap();
    assert!(netz.fehler.is_empty(), "{:?}", netz.fehler);
    let oben = netz.teile[0].clone();
    netz.teile.push(oben);
    let vg = Teilvorgabe { material: RADICAL_RUBBER_04, reduktion: 0.0 };
    // Die obere Lage liegt als Folie direkt auf: ihr Ruheversatz ist die Dicke.
    let lage = |ti: usize, q: [f64; 2]| [q[0] * 1e-3, q[1] * 1e-3, ti as f64 * 0.4e-3];
    let param = Simparameter {
        bild_dt: 1.0 / 30.0,
        daempfung: 1.0,
        schwerkraft: [0.0; 3],
        ..Simparameter::default()
    };
    let mut s = Szene::aus_netz(&netz, &|_| vg, &lage, Vec::new(), param);
    // Untere Lage am kurzen Rand halten.
    for i in s.teile[0].punkte.clone() {
        if s.x[i][0].abs() < 1e-6 {
            s.w[i] = 0.0;
        }
    }
    let oben_r = s.teile[1].punkte.clone();
    let unten_r = s.teile[0].punkte.clone();
    if geklebt {
        let paar = Stoffpaar {
            dehnung_oben: RADICAL_RUBBER_04.dehnung_n_m,
            dicke_oben: RADICAL_RUBBER_04.dicke_m,
            dicke_unten: RADICAL_RUBBER_04.dicke_m,
        };
        let (zone, ohne) = Zone::flaeche("laminat", &s.x, &s.dreiecke, oben_r, unten_r, 0.01, paar);
        assert_eq!(ohne, 0, "{ohne} Punkte ohne Gegenstueck");
        assert!(!zone.is_empty(), "keine Bindungen");
        s.klebungen.push(zone);
    }
    s
}

/// Verschiebt das freie Ende (`frei`: Punktauswahl) um `weg` in y und hält es
/// dort. Gibt die Zahl der festgelegten Punkte zurück.
fn ende_verschieben(s: &mut Szene, teil: usize, weg: f64) -> usize {
    let mut n = 0;
    for i in s.teile[teil].punkte.clone() {
        if s.x[i][0] > 0.2 - 1e-6 {
            s.x[i][1] += weg;
            s.v[i] = [0.0; 3];
            s.w[i] = 0.0;
            n += 1;
        }
    }
    n
}

/// Größte Verschiebung in y am freien Ende eines Teils gegenüber dem Start, in
/// mm.
fn auslenkung(s: &Szene, teil: usize, x0: &[[f64; 3]]) -> f64 {
    s.teile[teil]
        .punkte
        .clone()
        .filter(|&i| s.x[i][0] > 0.2 - 1e-6)
        .map(|i| (s.x[i][1] - x0[i][1]).abs())
        .fold(0.0, f64::max)
        * 1e3
}

#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn geklebte_lage_traegt_die_andere_mit() {
    let mut werte = Vec::new();
    for geklebt in [false, true] {
        let mut s = zwei_lagen(geklebt);
        let mut rw = ImplizitCg::neu(ImplizitParameter::default());
        // Erst zur Ruhe, dann das obere Ende 5 mm zur Seite und halten.
        for _ in 0..20 {
            rw.bild(&mut s);
        }
        let x0 = s.x.clone();
        let fest = ende_verschieben(&mut s, 1, 5e-3);
        assert!(fest > 0, "kein Punkt am freien Ende");
        for _ in 0..60 {
            rw.bild(&mut s);
        }
        werte.push((geklebt, auslenkung(&s, 0, &x0)));
        println!("  geklebt={geklebt}: untere Lage am freien Ende {:.3} mm", werte.last().unwrap().1);
    }
    let (lose, geklebt) = (werte[0].1, werte[1].1);
    println!("Auslenkung der unteren Lage: lose {lose:.3} mm, geklebt {geklebt:.3} mm");
    assert!(lose < 1.0, "ohne Klebung darf die untere Lage nicht folgen, sie folgt {lose:.3} mm");
    assert!(
        geklebt > 2.0,
        "mit Klebung muss die untere Lage folgen, sie folgt nur {geklebt:.3} mm"
    );
}

/// Sag of a strip held at both short ends, under its own weight, in mm: the
/// three cases of the task description, measured.
fn streifen_durchbiegung(lagen: usize, material: Material, kleben: bool) -> f64 {
    let m = read_str(&rechteck_muster(0.2, 0.04)).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    let p = Parameter { kante_mm: 10.0, ..Parameter::default() };
    let mut netz = vernetze(&ev, Kurvenquelle::Seamly, &[], &p).unwrap();
    let g = netz.teile[0].clone();
    for _ in 1..lagen {
        netz.teile.push(g.clone());
    }
    let vg = Teilvorgabe { material, reduktion: 0.0 };
    let dicke = material.dicke_m;
    let lage = |ti: usize, q: [f64; 2]| [q[0] * 1e-3, q[1] * 1e-3, ti as f64 * dicke];
    let param = Simparameter { bild_dt: 1.0 / 30.0, daempfung: 1.0, ..Simparameter::default() };
    let mut s = Szene::aus_netz(&netz, &|_| vg, &lage, Vec::new(), param);
    // Beide kurzen Ränder halten: der Streifen hängt in der Mitte durch.
    for ti in 0..s.teile.len() {
        for i in s.teile[ti].punkte.clone() {
            if s.x[i][0].abs() < 1e-6 || s.x[i][0] > 0.2 - 1e-6 {
                s.w[i] = 0.0;
            }
        }
    }
    if kleben {
        let oben = s.teile[1].punkte.clone();
        let unten = s.teile[0].punkte.clone();
        let paar = Stoffpaar {
            dehnung_oben: RADICAL_RUBBER_04.dehnung_n_m,
            dicke_oben: RADICAL_RUBBER_04.dicke_m,
            dicke_unten: RADICAL_RUBBER_04.dicke_m,
        };
        let (zone, ohne) = Zone::flaeche("laminat", &s.x, &s.dreiecke, oben, unten, 0.01, paar);
        assert_eq!(ohne, 0);
        s.klebungen.push(zone);
    }
    let mut rw = ImplizitCg::neu(ImplizitParameter::default());
    for _ in 0..120 {
        rw.bild(&mut s);
    }
    let ti = s.teile.len() - 1;
    let z0 = s.teile[ti].punkte.clone().map(|i| s.x[i][2]).fold(f64::NEG_INFINITY, f64::max);
    s.teile[ti]
        .punkte
        .clone()
        .filter(|&i| (s.x[i][0] - 0.1).abs() < 0.02)
        .map(|i| (z0 - s.x[i][2]) * 1e3)
        .fold(0.0, f64::max)
}

/// The deflection comparison of the task description, measured - and it does
/// not separate the cases. This is a property of the test piece, not a defect
/// of the glue, and the numbers say why:
///
/// * the sag needs the strip to lengthen; the tension that does it is
///   `stretch · strain · width`,
/// * two loose layers stretch in parallel, so the pair carries 2 × 348 N/m,
/// * the 0.8 mm laminate carries 696 N/m - the same,
/// * bending is irrelevant at 200 × 40 mm (its share of the sag is below the
///   print precision).
///
/// So all three sag the same and the load-transfer test above is the
/// discriminator. If this test ever fails, the deflection has started to
/// depend on bending, and the laminate comparison becomes meaningful again.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn durchbiegung_trennt_die_faelle_nicht_und_das_ist_erklaert() {
    let lose = streifen_durchbiegung(2, RADICAL_RUBBER_04, false);
    let geklebt = streifen_durchbiegung(2, RADICAL_RUBBER_04, true);
    let laminat = streifen_durchbiegung(1, LAMINAT_08, false);
    println!("Durchbiegung (mm): lose {lose:.3}  geklebt {geklebt:.3}  laminat {laminat:.3}");
    let max = lose.max(geklebt).max(laminat);
    for (name, w) in [("lose", lose), ("geklebt", geklebt), ("laminat", laminat)] {
        assert!(
            (w - max).abs() / max < 0.05,
            "{name} weicht von der Membranwirkung ab ({w:.3} gegen {max:.3} mm) - die Erklaerung oben gilt nicht mehr"
        );
    }
    // Und die Membranrechnung, die das erklaert: Zug aus der Dehnung.
    let zug_lose = 2.0 * RADICAL_RUBBER_04.dehnung_n_m;
    let zug_laminat = LAMINAT_08.dehnung_n_m;
    println!("Membranzug des Streifens (N/m): lose {zug_lose:.0}  laminat {zug_laminat:.0}");
    assert!((zug_lose - zug_laminat).abs() / zug_laminat < 0.01, "parallele Lagen tragen nicht dasselbe wie das Laminat");
}
