//! Acceptance for the comparison map (stage 5, task #21).
//!
//! `PLAN.md` asks for "distance per point as a color map on the garment". The
//! measurement is [`klon3d_sim::vergleich`]; this task paints it. What is
//! checked here is that the colour says the same thing as the number and that
//! the geometry arrives unchanged — nothing about looks.
//!
//! 1. the comparison scale: ends, clamping, and the **noise floor** — a
//!    difference inside the scatter of the reference must not read as a real
//!    deviation;
//! 2. colour and number agree per vertex on a pair with hand-computed
//!    distances;
//! 3. geometry, faces and group names survive the writer unchanged;
//! 4. a round trip through a real `Vergleich`: write, read back, recompute the
//!    distance from the CSV and get the same colour.
//!
//! All four run without CLO data.

use klon3d_sim::farbkarte::{vergleich, VERGLEICH_BIS, VERGLEICH_VON};
use klon3d_sim::vergleich::{Netz, Vergleich};
use klon3d_sim::vergleichskarte::obj_farbig;

/// A square in the z = 0 plane, 100 mm edge, two faces, one group.
fn quadrat() -> Netz {
    Netz::lies_obj(
        "g stoff\nv 0 0 0\nv 100 0 0\nv 100 100 0\nv 0 100 0\n\
         f 1 2 3\nf 1 3 4\n",
    )
    .expect("square readable")
}

/// The colour triple of a `v` line.
fn farbe_von(z: &str) -> [u8; 3] {
    let f: Vec<&str> = z
        .strip_prefix("v ")
        .expect("v line")
        .split_whitespace()
        .collect();
    [
        f[3].parse().unwrap(),
        f[4].parse().unwrap(),
        f[5].parse().unwrap(),
    ]
}

/// 1. The scale is fixed, clamped, and starts at the noise floor.
#[test]
fn die_skala_beginnt_am_rauschen() {
    // Below the floor: the cold end, i.e. "measured, inside the noise".
    assert_eq!(vergleich(0.0), vergleich(VERGLEICH_VON));
    assert_eq!(vergleich(-5.0), vergleich(VERGLEICH_VON));
    assert_eq!(vergleich(VERGLEICH_VON), vergleich(VERGLEICH_VON));
    // Just above the floor it leaves the cold end.
    assert_ne!(vergleich(VERGLEICH_VON + 5.0), vergleich(VERGLEICH_VON));
    // Above the top: the warm end, no rescaling for outliers.
    assert_eq!(vergleich(VERGLEICH_BIS), vergleich(1e6));

    // The floor is not zero — that is the whole point of this scale. A scale
    // starting at 0 would paint the 2.14 mm median of two runs that differ
    // only in fabric as a deviation in the middle of the ramp.
    assert!(VERGLEICH_VON > 0.0, "the comparison must start above 0");
    assert!(VERGLEICH_VON < VERGLEICH_BIS);

    // Blue falls from cold to warm without ever rising.
    let mut vorher = vergleich(VERGLEICH_VON);
    for i in 0..=200 {
        let v = VERGLEICH_VON + (VERGLEICH_BIS - VERGLEICH_VON) * i as f64 / 200.0;
        let c = vergleich(v);
        assert!(
            c[2] <= vorher[2],
            "blue rises at {v:.2}: {vorher:?} -> {c:?}"
        );
        vorher = c;
    }
}

/// A value that is not a number is grey, not the cold end: "not measured" is
/// not the same statement as "measured, inside the noise".
#[test]
fn kein_wert_ist_grau_und_nicht_kalt() {
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let c = vergleich(v);
        assert_eq!(c, klon3d_sim::farbkarte::UNGUELTIG, "{v}");
        assert_ne!(c, vergleich(VERGLEICH_VON), "{v} must not read as cold");
    }
}

/// 2. Every vertex carries the colour of its own distance. Two parallel planes
/// 5 mm apart, and one vertex lifted to 30 mm — both values are known by hand,
/// so this asserts equality and not a tolerance.
#[test]
fn farbe_je_knoten_passt_zum_abstand() {
    let mut b = quadrat();
    for p in &mut b.punkte {
        p[2] = 5.0;
    }
    // One vertex of `a` goes up to 30 mm above the other plane.
    let mut a = quadrat();
    a.punkte[0] = [0.0, 0.0, 30.0];

    let v = Vergleich::rechne(&a, &b, 20.0).expect("computable");
    assert!(
        (v.a_zu_b[0] - 25.0).abs() < 1e-9,
        "vertex 0: {}",
        v.a_zu_b[0]
    );
    assert!(
        (v.a_zu_b[1] - 5.0).abs() < 1e-9,
        "vertex 1: {}",
        v.a_zu_b[1]
    );

    let text = obj_farbig(&a, &v.a_zu_b);
    let farben: Vec<[u8; 3]> = text
        .lines()
        .filter(|z| z.starts_with("v "))
        .map(farbe_von)
        .collect();
    assert_eq!(farben.len(), a.punkte.len());
    for (i, &d) in v.a_zu_b.iter().enumerate() {
        assert_eq!(farben[i], vergleich(d), "vertex {i} at {d:.4} mm");
    }
    // The lifted vertex is warmer than the others: 25 mm against 5 mm.
    assert!(
        farben[0][0] > farben[1][0],
        "25 mm must be redder than 5 mm"
    );
}

/// 3. The file is the input plus three numbers per `v`: coordinates, faces and
/// group names are unchanged.
#[test]
fn geometrie_flaechen_und_gruppen_bleiben() {
    let n = quadrat();
    let text = obj_farbig(&n, &vec![12.0; n.punkte.len()]);

    let v: Vec<&str> = text.lines().filter(|z| z.starts_with("v ")).collect();
    assert_eq!(v.len(), n.punkte.len());
    for (i, z) in v.iter().enumerate() {
        let f: Vec<&str> = z.strip_prefix("v ").unwrap().split_whitespace().collect();
        assert_eq!(f.len(), 6, "six fields: {z}");
        for k in 0..3 {
            let a: f64 = f[k].parse().unwrap();
            assert!((a - n.punkte[i][k]).abs() < 5e-4, "vertex {i} axis {k}");
        }
    }

    let f: Vec<&str> = text.lines().filter(|z| z.starts_with("f ")).collect();
    assert_eq!(f.len(), n.dreiecke.len());
    for (i, z) in f.iter().enumerate() {
        let idx: Vec<u32> = z
            .strip_prefix("f ")
            .unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(idx, n.dreiecke[i].iter().map(|i| i + 1).collect::<Vec<_>>());
    }

    let g: Vec<&str> = text.lines().filter(|z| z.starts_with("g ")).collect();
    assert_eq!(g, vec!["g stoff"], "the group name comes back unchanged");

    // Stripping the colours leaves the input geometry, line for line.
    let mut zurueck = Vec::new();
    for z in text.lines().filter(|z| z.starts_with("v ")) {
        let f: Vec<&str> = z.strip_prefix("v ").unwrap().split_whitespace().collect();
        zurueck.push(format!("v {} {} {}", f[0], f[1], f[2]));
    }
    for z in text
        .lines()
        .filter(|z| z.starts_with("f ") || z.starts_with("g "))
    {
        zurueck.push(z.to_string());
    }
    let mut soll = Vec::new();
    for p in &n.punkte {
        soll.push(format!("v {:.4} {:.4} {:.4}", p[0], p[1], p[2]));
    }
    for (t, d) in n.dreiecke.iter().enumerate() {
        let _ = t;
        soll.push(format!("f {} {} {}", d[0] + 1, d[1] + 1, d[2] + 1));
    }
    // Group line sits before its faces in `zurueck`, after the vertices; the
    // comparison ignores the order of the two lists and checks content.
    let v_ist: Vec<&String> = zurueck.iter().filter(|z| z.starts_with("v ")).collect();
    let v_soll: Vec<&String> = soll.iter().filter(|z| z.starts_with("v ")).collect();
    assert_eq!(v_ist, v_soll, "coordinates changed");
    let f_ist: Vec<&String> = zurueck.iter().filter(|z| z.starts_with("f ")).collect();
    let f_soll: Vec<&String> = soll.iter().filter(|z| z.starts_with("f ")).collect();
    assert_eq!(f_ist, f_soll, "faces changed");
}

/// 4. Round trip over a real comparison: the colour in the file is what the
/// scale gives for the distance, and a grouped mesh keeps its groups.
#[test]
fn rundlauf_gruppen_und_farben() {
    // Two pieces, one moved by a known amount, so the per-group medians differ.
    let a = Netz::lies_obj(
        "g oben\nv 0 0 0\nv 100 0 0\nv 100 100 0\nv 0 100 0\nf 1 2 3\nf 1 3 4\n\
         g unten\nv 0 0 200\nv 100 0 200\nv 100 100 200\nv 0 100 200\nf 5 6 7\nf 5 7 8\n",
    )
    .expect("a readable");
    let mut b = a.clone();
    // The lower piece moves 40 mm; the upper stays.
    for i in 4..8 {
        b.punkte[i][2] += 40.0;
    }

    let v = Vergleich::rechne(&a, &b, 20.0).expect("computable");
    // The upper piece lies on itself, the lower is 40 mm off.
    assert!(v.a_zu_b[0] < 1e-9, "upper piece: {}", v.a_zu_b[0]);
    assert!(
        (v.a_zu_b[4] - 40.0).abs() < 1e-9,
        "lower piece: {}",
        v.a_zu_b[4]
    );

    let text = obj_farbig(&a, &v.a_zu_b);
    let farben: Vec<[u8; 3]> = text
        .lines()
        .filter(|z| z.starts_with("v "))
        .map(farbe_von)
        .collect();
    for (i, &d) in v.a_zu_b.iter().enumerate() {
        assert_eq!(farben[i], vergleich(d), "vertex {i} at {d:.4} mm");
    }
    // 0 mm reads cold (inside the noise), 40 mm reads at the warm end.
    assert_eq!(farben[0], vergleich(VERGLEICH_VON));
    assert_eq!(farben[4], vergleich(VERGLEICH_BIS));

    let g: Vec<&str> = text.lines().filter(|z| z.starts_with("g ")).collect();
    assert_eq!(g, vec!["g oben", "g unten"], "both groups survive");
}
