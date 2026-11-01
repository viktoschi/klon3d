//! Two meshes with a colour per vertex: the comparison map of stage 5.
//!
//! Stage 5 says "distance per point as a color map on the garment". The
//! measurement for it already exists — [`crate::vergleich`] gives the distance
//! of every vertex to the **surface** of the other mesh, in both directions.
//! This module only turns those numbers into a file a viewer can show:
//!
//! ```text
//! v x y z r g b
//! ```
//!
//! # Why not reuse `ausgabe::obj_farbig`
//!
//! That writer works on a [`crate::szene::Szene`] and knows piece indices. Here
//! the input is a [`crate::vergleich::Netz`] that came from some OBJ file and
//! carries its own group names; those have to come out unchanged, or the
//! coloured file no longer lines up with the piece tables. The two writers do
//! the same arithmetic on vertices and differ in what they know about the
//! model, so they stay separate rather than grow a shared abstraction for
//! three lines of formatting.
//!
//! # What is asserted about the colour
//!
//! Nothing here decides what a colour means: the scale lives in
//! [`crate::farbkarte`]. A point that is not a number — the search failing, a
//! mesh with no triangles — reads as the neutral grey of that module, not as
//! the cold end. The difference matters: the cold end means "measured, and
//! below the noise floor", grey means "not measured".
//!
//! # What this writer does not preserve
//!
//! See [`obj_farbig`]: coordinates are printed to four decimals, and `o`/`g`
//! both become `g`. Group names and faces survive.

use std::fmt::Write;

use crate::farbkarte;
use crate::vergleich::Netz;

/// `netz` as Wavefront OBJ with a colour on every vertex, taken from
/// `abstand[i]` for vertex `i`.
///
/// Faces, group names and the order of vertices are those of the input, so the
/// file is the input plus three numbers per `v` line. Two things are normalised
/// on the way out and both are stated rather than glossed over:
///
/// * **Coordinates are printed to four decimals** ([`obj`](crate::ausgabe::obj)
///   does the same); on a mesh in millimetres that is 1e-4 mm and therefore
///   below any measurement. The input's own text is not reproduced byte for
///   byte.
/// * **`o` and `g` both come out as `g`.** [`Netz`]
///   records the piece name but not which of the two keywords introduced it,
///   and the reader treats them as the same thing. The names survive, the
///   keyword does not.
///
/// `abstand` must have one entry per vertex; a shorter slice would silently
/// mis-colour the tail, which is why a mismatch is a panic and not a guess —
/// the caller has the distances from [`crate::vergleich::Vergleich`] and both
/// lengths are known.
pub fn obj_farbig(netz: &Netz, abstand: &[f64]) -> String {
    assert_eq!(
        abstand.len(),
        netz.punkte.len(),
        "one distance per vertex: {} distances, {} vertices",
        abstand.len(),
        netz.punkte.len()
    );

    let mut out =
        String::from("# klon3d-sim, Einheit mm, Farbe = Abstand zur Flaeche des anderen Netzes\n");
    for (i, p) in netz.punkte.iter().enumerate() {
        let c = farbkarte::vergleich(abstand[i]);
        let _ = writeln!(
            out,
            "v {:.4} {:.4} {:.4} {} {} {}",
            p[0], p[1], p[2], c[0], c[1], c[2]
        );
    }

    // Groups exactly as they came in: one `g` line when the name changes, then
    // the faces of that group. A mesh without names gets no `g` line at all.
    let mut letzte: Option<u32> = None;
    for (t, d) in netz.dreiecke.iter().enumerate() {
        let g = netz.gruppe.get(t).copied();
        if g != letzte {
            if let Some(name) = g.and_then(|g| netz.namen.get(g as usize)) {
                let _ = writeln!(out, "g {}", name.replace(' ', "_"));
            }
            letzte = g;
        }
        let _ = writeln!(out, "f {} {} {}", d[0] + 1, d[1] + 1, d[2] + 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square, 100 mm edge, two faces, one group name.
    fn quadrat() -> Netz {
        Netz::lies_obj("g stoff\nv 0 0 0\nv 100 0 0\nv 100 100 0\nv 0 100 0\nf 1 2 3\nf 1 3 4\n")
            .expect("square readable")
    }

    /// The fields of a `v` line.
    fn felder(z: &str) -> Vec<&str> {
        z.strip_prefix("v ")
            .expect("v line")
            .split_whitespace()
            .collect()
    }

    #[test]
    fn each_vertex_carries_the_colour_of_its_distance() {
        let n = quadrat();
        let d = vec![0.0, 2.0, 10.0, 100.0];
        let text = obj_farbig(&n, &d);
        let farben: Vec<[u8; 3]> = text
            .lines()
            .filter(|z| z.starts_with("v "))
            .map(|z| {
                let f = felder(z);
                [
                    f[3].parse().unwrap(),
                    f[4].parse().unwrap(),
                    f[5].parse().unwrap(),
                ]
            })
            .collect();
        assert_eq!(farben.len(), n.punkte.len());
        for (i, &wert) in d.iter().enumerate() {
            assert_eq!(
                farben[i],
                farbkarte::vergleich(wert),
                "vertex {i}, {wert} mm"
            );
        }
        // Below the noise floor reads as the cold end, far above as the warm end.
        assert_eq!(farben[0], farbkarte::vergleich(farbkarte::VERGLEICH_VON));
        assert_eq!(farben[3], farbkarte::vergleich(farbkarte::VERGLEICH_BIS));
    }

    #[test]
    fn geometry_faces_and_groups_are_unchanged() {
        let n = quadrat();
        let text = obj_farbig(&n, &vec![5.0; n.punkte.len()]);

        let v: Vec<&str> = text.lines().filter(|z| z.starts_with("v ")).collect();
        assert_eq!(v.len(), n.punkte.len());
        for (i, z) in v.iter().enumerate() {
            let f = felder(z);
            assert_eq!(f.len(), 6, "six fields: {z}");
            // Coordinates are those of the input, to the printed precision.
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

        // The group name arrives once, unchanged except for the space.
        let g: Vec<&str> = text.lines().filter(|z| z.starts_with("g ")).collect();
        assert_eq!(g.len(), 1, "one group: {g:?}");
        assert_eq!(g[0], "g stoff");
    }

    #[test]
    fn two_groups_stay_two_groups() {
        let n = Netz::lies_obj(
            "g oben\nv 0 0 0\nv 10 0 0\nv 0 10 0\nf 1 2 3\n\
             g unten\nv 0 0 100\nv 10 0 100\nv 0 10 100\nf 4 5 6\n",
        )
        .expect("readable");
        let text = obj_farbig(&n, &vec![1.0; n.punkte.len()]);
        let g: Vec<&str> = text.lines().filter(|z| z.starts_with("g ")).collect();
        assert_eq!(g, vec!["g oben", "g unten"]);
    }

    /// A mesh without group names gets no invented `g` line.
    #[test]
    fn without_names_no_group_line() {
        let n = Netz::lies_obj("v 0 0 0\nv 10 0 0\nv 0 10 0\nf 1 2 3\n").expect("readable");
        let text = obj_farbig(&n, &vec![1.0; n.punkte.len()]);
        assert!(
            !text.lines().any(|z| z.starts_with("g ")),
            "no group line expected:\n{text}"
        );
        assert!(text.lines().filter(|z| z.starts_with("v ")).count() == 3);
    }

    /// `o` in the input comes out as `g`; the name survives, the keyword does
    /// not. Documented in [`obj_farbig`] — this test keeps it from drifting.
    #[test]
    fn objekt_wird_zur_gruppe_namen_bleiben() {
        let n =
            Netz::lies_obj("o teil_a\nv 0 0 0\nv 10 0 0\nv 0 10 0\nf 1 2 3\n").expect("readable");
        let text = obj_farbig(&n, &vec![1.0; n.punkte.len()]);
        assert!(text.lines().any(|z| z == "g teil_a"), "name as g line");
        assert!(!text.lines().any(|z| z.starts_with("o ")), "no o line");
    }
}
