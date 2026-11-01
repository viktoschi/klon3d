//! Acceptance for the seam map (stage 5, task #25).
//!
//! `PLAN.md` asks for "seams against body points (armpit, shoulder point, bust
//! point)". This is a length per seam and body point, and the five tests below
//! are built so that each one fails for a different wrong implementation:
//!
//! 1. **the number itself** — a point 50 mm off the seam's end reads 50.000 mm,
//!    not approximately;
//! 2. **polyline, not vertices** — a point beside the middle of a 20 mm segment
//!    reads exactly its perpendicular distance, while the distance to the
//!    nearest seam *vertex* is 1.6 mm larger. A vertex-only search fails here;
//! 3. **the current state, not the rest shape** — moving the whole seam 10 mm
//!    towards a body point lowers that distance by exactly 10 mm;
//! 4. **the share along the seam** — 0 at the first point, 1 at the last, and
//!    non-decreasing over the seam;
//! 5. **per point, in the order given** — the near point reads nearer, and the
//!    entries keep the order of the points they were asked about.
//!
//! All five run without CLO data. The body points are placed by hand: what an
//! armpit is belongs to `klon3d-koerper`, not here.

use klon3d_sim::nahtlage::{Karte, Koerperpunkt};
use klon3d_sim::roehre::rohr_netz;
use klon3d_sim::szene::Teilvorgabe;
use klon3d_sim::{Simparameter, Szene, RADICAL_RUBBER_04};

/// Rectangle 560 × 280 mm, meshed at 20 mm, laid flat in the z = 0 plane.
///
/// Its one seam (left edge against right edge) is the straight line x = 0.28 m
/// running along y from 0 to 0.28 m — every pair midpoint lies on it. That is
/// the shape the arithmetic has to survive: one straight run, known ends.
fn flach(dx: f64) -> (Szene, klon3d_mesh::Musternetz) {
    let netz = rohr_netz(0.56, 0.28, 20.0).expect("rectangle meshable");
    let vg = Teilvorgabe {
        material: RADICAL_RUBBER_04,
        reduktion: 0.0,
    };
    let lage = move |_: usize, q: [f64; 2]| [q[0] * 1e-3 + dx, q[1] * 1e-3, 0.0];
    let s = Szene::aus_netz(&netz, &|_| vg, &lage, Vec::new(), Simparameter::default());
    (s, netz)
}

/// The pair midpoints of the first seam, in seam order — the seam line the map
/// is supposed to measure against.
fn nahtpunkte(s: &Szene, netz: &klon3d_mesh::Musternetz) -> Vec<[f64; 3]> {
    let n = &netz.naehte[0];
    let mut out = Vec::new();
    for tn in &n.teilnaehte {
        let (a0, b0) = (
            s.teile[tn.teil_a].punkte.start,
            s.teile[tn.teil_b].punkte.start,
        );
        for pp in &tn.paare {
            let (a, b) = (s.x[a0 + pp.a as usize], s.x[b0 + pp.b as usize]);
            out.push([
                0.5 * (a[0] + b[0]),
                0.5 * (a[1] + b[1]),
                0.5 * (a[2] + b[2]),
            ]);
        }
    }
    out
}

fn einer(name: &str, punkt: [f64; 3]) -> Vec<Koerperpunkt> {
    vec![Koerperpunkt {
        name: name.into(),
        punkt,
    }]
}

/// 1. The number is the number: 50 mm off the end of the seam reads 50.000 mm.
///
/// The point sits perpendicular to the seam at its top end, so the nearest
/// place is the end point itself and the value is not a projection accident.
#[test]
fn fuenfzig_millimeter_lesen_sich_als_fuenfzig() {
    let (s, netz) = flach(0.0);
    let p = nahtpunkte(&s, &netz);
    assert!(p.len() > 10, "a meshed edge has many pairs: {}", p.len());
    let oben = p
        .iter()
        .copied()
        .max_by(|a, b| a[1].total_cmp(&b[1]))
        .expect("seam");
    let k = Karte::messe(&s, &netz, &einer("Oben", [oben[0] + 0.05, oben[1], 0.0]));
    let n = k.naht("R1").expect("the tube's seam is called R1");
    assert_eq!(n.je_punkt.len(), 1);
    let m = n.je_punkt[0].stelle;
    assert!(
        (m.abstand - 0.05).abs() < 1e-12,
        "50 mm expected, read {:.9} m",
        m.abstand
    );
    // The nearest place is the seam's own top end, and that is its last pair.
    assert!((m.anteil - 1.0).abs() < 1e-12, "share {}", m.anteil);
    assert!((m.punkt[0] - oben[0]).abs() < 1e-12);
    assert!((m.punkt[1] - oben[1]).abs() < 1e-12);
}

/// 2. The test that separates this map from a nearest-vertex search: a point
/// beside the **middle** of one 20 mm segment.
///
/// The perpendicular distance is exactly 30 mm; the distance to the nearest of
/// the two vertices is `sqrt(30² + 10²) = 31.62 mm`, 1.6 mm more. A vertex-only
/// implementation reads the larger number.
#[test]
fn der_abstand_geht_auf_die_strecke_nicht_auf_die_punkte() {
    let (s, netz) = flach(0.0);
    let p = nahtpunkte(&s, &netz);
    // A segment in the middle of the seam, clear of both ends.
    let (a, b) = (p[p.len() / 2], p[p.len() / 2 + 1]);
    let mitte = [
        0.5 * (a[0] + b[0]),
        0.5 * (a[1] + b[1]),
        0.5 * (a[2] + b[2]),
    ];
    let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
    assert!(
        l > 0.015 && l < 0.025,
        "20 mm mesh, segment is {:.1} mm",
        l * 1e3
    );

    let daneben = [mitte[0] + 0.03, mitte[1], mitte[2]];
    let k = Karte::messe(&s, &netz, &einer("Mitte", daneben));
    let m = k.naht("R1").expect("seam").je_punkt[0].stelle;
    assert!(
        (m.abstand - 0.03).abs() < 1e-12,
        "perpendicular distance 30 mm expected, read {:.9} m",
        m.abstand
    );

    // What a vertex-only search would have said, from the same scene.
    let zum_punkt = |q: &[f64; 3]| {
        ((q[0] - daneben[0]).powi(2) + (q[1] - daneben[1]).powi(2) + (q[2] - daneben[2]).powi(2))
            .sqrt()
    };
    let naechster = p.iter().map(|q| zum_punkt(q)).fold(f64::INFINITY, f64::min);
    assert!(
        naechster - m.abstand > 0.5e-3,
        "the vertex answer ({:.3} mm) must be visibly larger than the segment answer ({:.3} mm)",
        naechster * 1e3,
        m.abstand * 1e3
    );
}

/// 3. The map reads the current state: moving the whole seam 10 mm towards a
/// body point lowers that point's distance by exactly 10 mm, for every seam.
#[test]
fn die_karte_liest_den_jetzigen_zustand() {
    let ziel = [0.30, 0.14, 0.0];
    let (vorher, netz) = flach(0.0);
    let (nachher, _) = flach(0.01);

    let a = Karte::messe(&vorher, &netz, &einer("Ziel", ziel));
    let b = Karte::messe(&nachher, &netz, &einer("Ziel", ziel));
    assert_eq!(a.je_naht.len(), b.je_naht.len());
    assert!(!a.je_naht.is_empty());
    for (x, y) in a.je_naht.iter().zip(&b.je_naht) {
        assert_eq!(x.kennung, y.kennung);
        for (p, q) in x.je_punkt.iter().zip(&y.je_punkt) {
            assert_eq!(p.koerperpunkt, q.koerperpunkt);
            assert!(
                (p.stelle.abstand - q.stelle.abstand - 0.01).abs() < 1e-12,
                "{}: {:.9} → {:.9}, 10 mm expected",
                p.koerperpunkt,
                p.stelle.abstand,
                q.stelle.abstand
            );
        }
    }
}

/// 4. The share along the seam is 0 at the first pair, 1 at the last, and never
/// falls — the property a reader uses to find the place in the pattern.
#[test]
fn der_anteil_laeuft_von_null_bis_eins() {
    let (s, netz) = flach(0.0);
    let p = nahtpunkte(&s, &netz);
    // Ask about the seam's own points: each one must come back at its own place.
    let punkte: Vec<Koerperpunkt> = p
        .iter()
        .enumerate()
        .map(|(i, q)| Koerperpunkt {
            name: format!("P{i}"),
            punkt: *q,
        })
        .collect();
    let k = Karte::messe(&s, &netz, &punkte);
    let n = k.naht("R1").expect("seam");
    assert_eq!(n.je_punkt.len(), p.len());

    let anteile: Vec<f64> = n.je_punkt.iter().map(|q| q.stelle.anteil).collect();
    assert!(anteile[0].abs() < 1e-12, "first share {}", anteile[0]);
    assert!((anteile[anteile.len() - 1] - 1.0).abs() < 1e-12);
    for w in anteile.windows(2) {
        assert!(w[1] >= w[0], "share falls: {} → {}", w[0], w[1]);
    }

    // The share is a **length** share, not a pair index: recomputed here from
    // the midpoints alone. With an evenly meshed seam the two differ only in
    // the way `i/(n−1)` differs from the metric, so a share that was really a
    // pair index would still be close — the numbers below are the metric.
    let mut kum = vec![0.0f64; p.len()];
    for i in 1..p.len() {
        let d = ((p[i][0] - p[i - 1][0]).powi(2)
            + (p[i][1] - p[i - 1][1]).powi(2)
            + (p[i][2] - p[i - 1][2]).powi(2))
        .sqrt();
        kum[i] = kum[i - 1] + d;
    }
    let ganz = kum[p.len() - 1];
    assert!(
        ganz > 0.2,
        "the seam is about 280 mm long, read {:.1} mm",
        ganz * 1e3
    );
    for (i, a) in anteile.iter().enumerate() {
        let soll = kum[i] / ganz;
        assert!(
            (a - soll).abs() < 1e-9,
            "share at pair {i}: {a:.9} against the length share {soll:.9}"
        );
    }
}

/// 5. Two body points keep their own numbers and their order.
#[test]
fn zwei_punkte_behalten_ihre_zahlen_und_ihre_reihenfolge() {
    let (s, netz) = flach(0.0);
    let p = nahtpunkte(&s, &netz);
    let mitte = p[p.len() / 2];
    let nah = Koerperpunkt {
        name: "nah".into(),
        punkt: [mitte[0] + 0.01, mitte[1], 0.0],
    };
    let fern = Koerperpunkt {
        name: "fern".into(),
        punkt: [mitte[0] + 0.20, mitte[1], 0.0],
    };
    // Deliberately far first, so the order of the output cannot come from the
    // order of the distances.
    let k = Karte::messe(&s, &netz, &[fern.clone(), nah.clone()]);
    let n = k.naht("R1").expect("seam");
    assert_eq!(n.je_punkt[0].koerperpunkt, "fern");
    assert_eq!(n.je_punkt[1].koerperpunkt, "nah");
    assert!(
        n.je_punkt[0].stelle.abstand > n.je_punkt[1].stelle.abstand,
        "the near point must read nearer"
    );
    assert!((n.je_punkt[1].stelle.abstand - 0.01).abs() < 1e-12);
    assert!((n.je_punkt[0].stelle.abstand - 0.20).abs() < 1e-12);

    // The summary names the same point the rows do.
    let (d, naht, punkt) = k.kleinster().expect("a map has an entry");
    assert!((d - 0.01).abs() < 1e-12);
    assert_eq!(naht, "R1");
    assert_eq!(punkt, "nah");
}
