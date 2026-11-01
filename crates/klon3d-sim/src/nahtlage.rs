//! Seam position: how far every seam runs from the named body points
//! (stage 5).
//!
//! `PLAN.md` asks for "seams against body points (armpit, shoulder point, bust
//! point)". This is the last open line of stage 5, and it is the only one whose
//! answer is a length: for every seam of the pattern, the smallest distance to
//! each named point and the place on the seam where that distance occurs.
//!
//! # What is a "seam position"
//!
//! Two things have to be fixed before a millimetre can be checked, and both are
//! decisions rather than facts:
//!
//! 1. **Which body point.** The three names are three separate landmarks of the
//!    body data, not one point each: `Armhole` exists four times per side (front,
//!    back, torso side, shoulder), `HPS` and `Shoulder` sit next to each other,
//!    and `Bust` is a ring of points around `APEX`. [`Koerperpunkt`] carries the
//!    name it was given, so a report can never say "the armpit" without saying
//!    which one; the crate that builds them documents the choice.
//! 2. **What the seam is.** The sewn seam, seen after sewing: the midpoint of
//!    each paired point pair. The two sides lie on each other when the seam is
//!    closed, so their midpoint is the seam line and neither side is preferred.
//!
//! # Distance to the seam, not to the seam's points
//!
//! The seam is a polyline through those midpoints, and the distance is to the
//! polyline. A search over the sampled points alone overestimates by up to half
//! a mesh edge wherever the seam passes the body point between two vertices —
//! on the reference mesh 10 mm, which is the same order as the numbers this map
//! reports, so it would not be a rounding detail but the result.
//!
//! Between two consecutive pairs the seam is a chord. Where a seam is sewn from
//! several runs (one seam, several piece pairs) the runs are **not** connected
//! by an invented segment: each run is its own polyline, and the share along
//! the seam counts their lengths one after another.
//!
//! # The honest limit
//!
//! The numbers are distances between two named things: a seam line and one
//! vertex of the body mesh. A seam that runs straight across the body point
//! reads the cloth's thickness plus the standoff, not 0, and the value moves
//! with how the body happens to be meshed rather than with the anatomy. That is
//! intended — it is what makes the number checkable — but it is not "the seam
//! sits 0 mm from the armpit".
//!
//! The map says nothing about which pattern side belongs to the left or the
//! right side of the body. Left and right are separate points and are printed
//! separately; whether the two columns agree is something a reader sees, not
//! something this module decides.

use std::fmt::Write;

use klon3d_mesh::{Musternetz, Nahtnetz};

use crate::koerper::{add, dot, mul, norm, sub};
use crate::szene::Szene;

/// A named body point in scene coordinates (m).
///
/// The name travels with the value: the same landmark exists several times on a
/// body, and a number without its name is not checkable.
#[derive(Debug, Clone, PartialEq)]
pub struct Koerperpunkt {
    /// What the point is called in the report, e.g. `Achsel links`.
    pub name: String,
    /// Position in scene coordinates (m).
    pub punkt: [f64; 3],
}

/// The closest place on one seam for one body point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stelle {
    /// Distance (m).
    pub abstand: f64,
    /// The closest point on the seam (m) — on the polyline, not necessarily on
    /// a sampled pair.
    pub punkt: [f64; 3],
    /// Pair index along the seam; for a point between two pairs, the pair that
    /// starts the segment.
    pub paar: usize,
    /// Share along the seam, 0 at its first point and 1 at its last.
    pub anteil: f64,
}

/// One body point against one seam.
#[derive(Debug, Clone, PartialEq)]
pub struct Punktabstand {
    pub koerperpunkt: String,
    pub stelle: Stelle,
}

/// One seam against all body points.
#[derive(Debug, Clone, PartialEq)]
pub struct Naht {
    pub kennung: String,
    /// Length of the sewn seam (m): the polyline through the pair midpoints.
    pub laenge_m: f64,
    /// Paired points that make up the seam.
    pub paare: usize,
    /// One entry per body point, in the order the points were given.
    pub je_punkt: Vec<Punktabstand>,
}

/// The whole map.
#[derive(Debug, Clone, PartialEq)]
pub struct Karte {
    /// The body points, in the order they were given.
    pub punkte: Vec<Koerperpunkt>,
    /// One entry per seam, in pattern order.
    pub je_naht: Vec<Naht>,
}

impl Karte {
    /// Measures the current state of `s` against `punkte`.
    ///
    /// `netz` is the pattern mesh the scene was built from: the seams are its
    /// seams, the positions are the scene's. The map therefore reads the state
    /// after the run, not the rest shape of the pattern.
    pub fn messe(s: &Szene, netz: &Musternetz, punkte: &[Koerperpunkt]) -> Karte {
        let je_naht = netz
            .naehte
            .iter()
            .map(|n| {
                let z = Zug::aus(s, n);
                let je_punkt = punkte
                    .iter()
                    .filter_map(|k| {
                        z.naechste(&k.punkt).map(|stelle| Punktabstand {
                            koerperpunkt: k.name.clone(),
                            stelle,
                        })
                    })
                    .collect();
                Naht {
                    kennung: n.kennung.clone(),
                    laenge_m: z.gesamt_m,
                    paare: z.paare,
                    je_punkt,
                }
            })
            .collect();
        Karte {
            punkte: punkte.to_vec(),
            je_naht,
        }
    }

    /// One seam by its identifier.
    pub fn naht(&self, kennung: &str) -> Option<&Naht> {
        self.je_naht.iter().find(|n| n.kennung == kennung)
    }

    /// The smallest distance in the whole map: (m, seam, body point).
    ///
    /// One line for a report, and the same number the table is built from, so
    /// the summary cannot drift away from the rows.
    pub fn kleinster(&self) -> Option<(f64, &str, &str)> {
        self.je_naht
            .iter()
            .flat_map(|n| n.je_punkt.iter().map(move |p| (n, p)))
            .map(|(n, p)| {
                (
                    p.stelle.abstand,
                    n.kennung.as_str(),
                    p.koerperpunkt.as_str(),
                )
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// How many seams are within `grenze` (m) of the body point `name`.
    ///
    /// The counterpart of the "over the line" counts of the other maps: the
    /// number that names a problem, not the label.
    pub fn innerhalb(&self, name: &str, grenze: f64) -> usize {
        self.je_naht
            .iter()
            .filter(|n| {
                n.je_punkt
                    .iter()
                    .any(|p| p.koerperpunkt == name && p.stelle.abstand <= grenze)
            })
            .count()
    }

    /// CSV, one line per seam and body point:
    /// `naht;laenge_mm;paare;koerperpunkt;abstand_mm;paar;anteil;x_cm;y_cm;z_cm`.
    ///
    /// Length and distance in mm, positions in cm, like the rest of the command
    /// line output. The seam length is repeated on every line so a filtered file
    /// still says which seam a line belongs to and how long it is; `anteil` then
    /// turns into a position along it without the scene.
    pub fn csv(&self) -> String {
        let mut out = String::from(
            "naht;laenge_mm;paare;koerperpunkt;abstand_mm;paar;anteil;x_cm;y_cm;z_cm\n",
        );
        for n in &self.je_naht {
            for p in &n.je_punkt {
                let q = p.stelle.punkt;
                let _ = writeln!(
                    out,
                    "{};{:.3};{};{};{:.3};{};{:.4};{:.2};{:.2};{:.2}",
                    n.kennung,
                    n.laenge_m * 1e3,
                    n.paare,
                    p.koerperpunkt,
                    p.stelle.abstand * 1e3,
                    p.stelle.paar,
                    p.stelle.anteil,
                    q[0] * 1e2,
                    q[1] * 1e2,
                    q[2] * 1e2
                );
            }
        }
        out
    }
}

/// The run of one seam as polylines, with their lengths.
struct Zug {
    /// Midpoints of the pair pairs per sewn run, each with its pair index.
    runs: Vec<Vec<([f64; 3], usize)>>,
    /// Total length of all runs (m).
    gesamt_m: f64,
    /// Paired points over all runs.
    paare: usize,
}

impl Zug {
    fn aus(s: &Szene, naht: &Nahtnetz) -> Zug {
        let mut runs = Vec::new();
        let mut paare = 0usize;
        for tn in &naht.teilnaehte {
            let a0 = s.teile[tn.teil_a].punkte.start;
            let b0 = s.teile[tn.teil_b].punkte.start;
            let mut r = Vec::with_capacity(tn.paare.len());
            for (k, pp) in tn.paare.iter().enumerate() {
                let i = a0 + pp.a as usize;
                let j = b0 + pp.b as usize;
                // The sewn seam: both sides lie on each other, so the midpoint
                // is the seam line and neither side is preferred.
                r.push((mul(add(s.x[i], s.x[j]), 0.5), k));
            }
            paare += r.len();
            if !r.is_empty() {
                runs.push(r);
            }
        }
        let gesamt_m = runs.iter().map(|r| laenge(r)).sum();
        Zug {
            runs,
            gesamt_m,
            paare,
        }
    }

    /// Closest place of the whole seam to `p`.
    fn naechste(&self, p: &[f64; 3]) -> Option<Stelle> {
        let mut best: Option<Stelle> = None;
        let mut vorher = 0.0;
        for r in &self.runs {
            let mut kum = vorher;
            if let [(q, paar)] = r[..] {
                // A run of one pair is a point; the seam has no direction there.
                best = besser(
                    best,
                    Stelle {
                        abstand: norm(sub(*p, q)),
                        punkt: q,
                        paar,
                        anteil: self.anteil(kum),
                    },
                );
            }
            for w in r.windows(2) {
                let ((a, paar), (b, _)) = (w[0], w[1]);
                let ab = sub(b, a);
                let l = norm(ab);
                let t = if l > 0.0 {
                    (dot(sub(*p, a), ab) / (l * l)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let q = add(a, mul(ab, t));
                best = besser(
                    best,
                    Stelle {
                        abstand: norm(sub(*p, q)),
                        punkt: q,
                        paar,
                        anteil: self.anteil(kum + t * l),
                    },
                );
                kum += l;
            }
            vorher += laenge(r);
        }
        best
    }

    /// Share of `laenge` (m) in the seam's whole length, kept inside 0…1.
    fn anteil(&self, laenge: f64) -> f64 {
        if self.gesamt_m > 0.0 {
            (laenge / self.gesamt_m).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Length of one polyline (m).
fn laenge(r: &[([f64; 3], usize)]) -> f64 {
    r.windows(2).map(|w| norm(sub(w[1].0, w[0].0))).sum()
}

/// The nearer of two candidates; on a tie the earlier one keeps it, so the
/// result does not depend on the order the runs happen to be walked in.
fn besser(best: Option<Stelle>, neu: Stelle) -> Option<Stelle> {
    match best {
        Some(b) if b.abstand <= neu.abstand => Some(b),
        _ => Some(neu),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seam built by hand: a straight line along z from 0 to 1, three pairs.
    fn gerade() -> Karte {
        // The map reads a scene, so the seam is exercised through `messe` in
        // `tests/nahtlage.rs`. Here only the polyline arithmetic is checked,
        // because that is what the millimetre has to survive.
        let runs = vec![vec![
            ([0.0, 0.0, 0.0], 0usize),
            ([0.0, 0.0, 0.5], 1usize),
            ([0.0, 0.0, 1.0], 2usize),
        ]];
        let gesamt_m = runs.iter().map(|r| laenge(r)).sum();
        let z = Zug {
            runs,
            gesamt_m,
            paare: 3,
        };
        let punkte = vec![
            Koerperpunkt {
                name: "Mitte".into(),
                punkt: [0.25, 0.0, 0.5],
            },
            Koerperpunkt {
                name: "Oben".into(),
                punkt: [0.0, 0.0, 1.0],
            },
        ];
        let je_punkt = punkte
            .iter()
            .map(|k| Punktabstand {
                koerperpunkt: k.name.clone(),
                stelle: z.naechste(&k.punkt).expect("a seam is not empty"),
            })
            .collect();
        Karte {
            punkte,
            je_naht: vec![Naht {
                kennung: "T1".into(),
                laenge_m: gesamt_m,
                paare: 3,
                je_punkt,
            }],
        }
    }

    /// A point beside the middle of a segment reads the perpendicular distance,
    /// and its share is measured along the polyline — not the vertex index.
    #[test]
    fn der_abstand_geht_auf_die_strecke() {
        // z = 0.75 is the middle of the segment between pair 1 (z = 0.5) and
        // pair 2 (z = 1.0); the point beside it is 0.25 m away.
        let z = Zug {
            runs: vec![vec![
                ([0.0, 0.0, 0.0], 0usize),
                ([0.0, 0.0, 0.5], 1usize),
                ([0.0, 0.0, 1.0], 2usize),
            ]],
            gesamt_m: 1.0,
            paare: 3,
        };
        let m = z.naechste(&[0.25, 0.0, 0.75]).expect("seam");
        assert!((m.abstand - 0.25).abs() < 1e-12, "{}", m.abstand);
        assert!((m.anteil - 0.75).abs() < 1e-12, "{}", m.anteil);
        // The segment starts at pair 1 (0.5 … 1.0), so that is the pair named.
        assert_eq!(m.paar, 1);
        assert!((m.punkt[2] - 0.75).abs() < 1e-12, "{}", m.punkt[2]);

        // The same point halfway between two pairs is a tie; the earlier pair
        // keeps it, so the answer does not depend on the order of the runs.
        let k = gerade();
        let m = k.naht("T1").expect("seam").je_punkt[0].stelle;
        assert!((m.abstand - 0.25).abs() < 1e-12, "{}", m.abstand);
        assert!((m.anteil - 0.5).abs() < 1e-12, "{}", m.anteil);
        assert_eq!(m.paar, 0, "on a tie the earlier pair keeps it");
        assert!((m.punkt[2] - 0.5).abs() < 1e-12);
    }

    /// The endpoints carry share 0 and 1, and a point beyond the end is
    /// measured to the end, not to the infinite line.
    #[test]
    fn die_enden_tragen_anteil_null_und_eins() {
        let k = gerade();
        let oben = k.naht("T1").expect("seam").je_punkt[1].stelle;
        assert!((oben.abstand).abs() < 1e-12);
        assert!((oben.anteil - 1.0).abs() < 1e-12, "{}", oben.anteil);

        let runs = vec![vec![([0.0, 0.0, 0.0], 0usize), ([0.0, 0.0, 1.0], 1usize)]];
        let gesamt_m = runs.iter().map(|r| laenge(r)).sum();
        let z = Zug {
            runs,
            gesamt_m,
            paare: 2,
        };
        let unten = z.naechste(&[0.0, 0.0, -0.5]).expect("seam");
        assert!((unten.abstand - 0.5).abs() < 1e-12, "{}", unten.abstand);
        assert!((unten.anteil).abs() < 1e-12, "{}", unten.anteil);
        let darueber = z.naechste(&[0.0, 0.0, 1.5]).expect("seam");
        assert!((darueber.abstand - 0.5).abs() < 1e-12);
        assert!((darueber.anteil - 1.0).abs() < 1e-12);
    }

    /// `innerhalb` counts seams, not rows, and it counts by the point's name.
    #[test]
    fn innerhalb_zaehlt_naehte_und_fragt_nach_dem_namen() {
        let k = gerade();
        assert_eq!(k.innerhalb("Mitte", 0.3), 1, "0.25 is within 30 mm");
        assert_eq!(k.innerhalb("Mitte", 0.2), 0, "0.25 is not within 20 mm");
        assert_eq!(k.innerhalb("Oben", 1e-9), 1, "the top point is on the seam");
        assert_eq!(
            k.innerhalb("Nirgends", 10.0),
            0,
            "an unknown name counts nothing"
        );
    }
}
