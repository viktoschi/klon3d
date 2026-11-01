//! Distance map garment → skin (stage 5, the first inspection measure).
//!
//! For every cloth point: the signed distance to the body surface, outside
//! positive, inside negative, together with the nearest body point and the
//! outward normal there. Plus a summary per piece.
//!
//! # Why point to surface
//!
//! The measure is the distance from a cloth point to the body **surface**, not
//! to the nearest body vertex. A vertex-to-vertex distance is dominated by how
//! finely the body happens to be meshed; on the reference body (about 31 000
//! points) it reads several millimetres too large wherever the cloth lies
//! between two vertices.
//!
//! # Why the query needs no upper bound
//!
//! The collision query [`Koerpernetz::naechster`](crate::netzkoerper::Koerpernetz::naechster)
//! answers only within a radius and gives up beyond the search depth (10 mm) —
//! right for collision, wrong here: a sleeve cap standing off is 30 mm away and
//! exactly those points are the interesting ones.
//! [`Koerpernetz::abstand_ohne_grenze`](crate::netzkoerper::Koerpernetz::abstand_ohne_grenze)
//! widens the search until a triangle is found.
//!
//! # Sign
//!
//! Taken from the angle-weighted pseudo-normal of the nearest region (see
//! [`crate::netzkoerper`]). A closed, consistently oriented mesh gives the
//! right sign, with one known limit: where the surface folds so tightly that a
//! point a few millimetres off the normal already lies in the neighbouring
//! fold, the sign flips locally. Measured on the reference body inside the
//! clothing band (z 0.5–1.45 m): no deviation against ray parity in 160
//! samples. Outside it — nose, lips, toes — there are such points; clothing
//! does not go there, and the test in `tests/abstand.rs` enforces that this
//! limit stays visible instead of being averaged away.
//!
//! # Fields
//!
//! The normal and the nearest body point are carried along, not just the
//! distance: they tell in which direction a point stands off, which is what
//! turns "the cap stands off 30 mm" into "the cap stands off 30 mm upward".

use crate::koerper::{add, dot, mul, norm, sub, Koerper};
use crate::netzkoerper::Naechster;
use crate::szene::Szene;

/// The distance of one cloth point to the skin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Punkt {
    /// Signed distance in m, outside positive.
    pub abstand: f64,
    /// Nearest point on the body surface (m).
    pub punkt: [f64; 3],
    /// Outward normal of the body there.
    pub normale: [f64; 3],
}

/// Below this depth a point counts as inside the body (m). The collision
/// distance is half the fabric thickness, so a correctly dressed point is
/// never inside; `0` would count rounding noise as penetration.
pub const INNEN: f64 = -1e-4;

/// Distribution of distances in mm.
///
/// Not [`crate::vergleich::Kennzahlen`]: that one starts at zero by nature
/// (two meshes lying on each other) and has no minimum. Here the minimum is
/// the value that matters — it is the one that shows penetration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verteilung {
    pub anzahl: usize,
    pub min: f64,
    pub mittel: f64,
    pub median: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: f64,
}

impl Verteilung {
    /// Summarises single values (mm). Empty input gives all zeros.
    pub fn aus(mut d: Vec<f64>) -> Verteilung {
        if d.is_empty() {
            return Verteilung {
                anzahl: 0,
                min: 0.0,
                mittel: 0.0,
                median: 0.0,
                p90: 0.0,
                p99: 0.0,
                max: 0.0,
            };
        }
        d.sort_by(f64::total_cmp);
        let n = d.len();
        let q = |f: f64| d[((n as f64 * f) as usize).min(n - 1)];
        Verteilung {
            anzahl: n,
            min: d[0],
            mittel: d.iter().sum::<f64>() / n as f64,
            median: q(0.5),
            p90: q(0.9),
            p99: q(0.99),
            max: d[n - 1],
        }
    }
}

/// Summary of one pattern piece.
#[derive(Debug, Clone, PartialEq)]
pub struct Teil {
    pub name: String,
    /// Distances of the piece's points (mm).
    pub verteilung: Verteilung,
    /// Points deeper than [`INNEN`] inside the skin.
    pub innen: usize,
}

/// The whole map.
#[derive(Debug, Clone, PartialEq)]
pub struct Karte {
    /// One entry per cloth point, in scene order.
    pub je_punkt: Vec<Punkt>,
    /// One entry per piece, in scene order.
    pub je_teil: Vec<Teil>,
}

impl Karte {
    /// Measures the scene against its bodies; `None` without a body.
    ///
    /// With several bodies the nearest **surface** counts — an arm and a torso
    /// are separate meshes, and a point on the arm is not 20 mm off the arm
    /// because some other body happens to be nearer to a different point.
    pub fn messe(s: &Szene) -> Option<Karte> {
        if s.koerper.is_empty() {
            return None;
        }
        let je_punkt: Vec<Punkt> = s.x.iter().map(|&x| naechste_haut(&s.koerper, x)).collect();
        let je_teil = s
            .teile
            .iter()
            .map(|t| {
                let d: Vec<f64> = t
                    .punkte
                    .clone()
                    .map(|i| je_punkt[i].abstand * 1e3)
                    .collect();
                let innen = t
                    .punkte
                    .clone()
                    .filter(|&i| je_punkt[i].abstand < INNEN)
                    .count();
                Teil {
                    name: t.name.clone(),
                    verteilung: Verteilung::aus(d),
                    innen,
                }
            })
            .collect();
        Some(Karte { je_punkt, je_teil })
    }

    /// Summary over all pieces together (mm).
    pub fn gesamt(&self) -> Verteilung {
        Verteilung::aus(self.je_punkt.iter().map(|p| p.abstand * 1e3).collect())
    }

    /// The deepest point of the whole garment (m, negative inside) and where
    /// it is.
    pub fn tiefster(&self) -> Option<(f64, usize)> {
        self.je_punkt
            .iter()
            .enumerate()
            .map(|(i, p)| (p.abstand, i))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// CSV, one line per cloth point:
    /// `punkt;teil;x_cm;y_cm;z_cm;abstand_mm;nx;ny;nz` — coordinates in cm and
    /// the distance in mm, like the rest of the command line output. The piece
    /// name is repeated on every line so the file can be filtered and read
    /// without the scene.
    pub fn csv(&self, s: &Szene) -> String {
        use std::fmt::Write;
        let mut out = String::from("punkt;teil;x_cm;y_cm;z_cm;abstand_mm;nx;ny;nz\n");
        for t in &s.teile {
            for i in t.punkte.clone() {
                let (q, p) = (s.x[i], self.je_punkt[i]);
                let _ = writeln!(
                    out,
                    "{};{};{:.2};{:.2};{:.2};{:.3};{:.4};{:.4};{:.4}",
                    i,
                    t.name,
                    q[0] * 1e2,
                    q[1] * 1e2,
                    q[2] * 1e2,
                    p.abstand * 1e3,
                    p.normale[0],
                    p.normale[1],
                    p.normale[2]
                );
            }
        }
        out
    }
}

/// Signed distance of `x` to the nearest of the bodies.
fn naechste_haut(koerper: &[Koerper], x: [f64; 3]) -> Punkt {
    let mut best: Option<Punkt> = None;
    for k in koerper {
        let p = abstand(k, x);
        // Nearest surface wins; on a tie the first body keeps it.
        if best.is_none_or(|b| p.abstand.abs() < b.abstand.abs()) {
            best = Some(p);
        }
    }
    best.expect("koerper ist nicht leer")
}

/// Signed distance of `x` to one body.
///
/// The cylinder is answered analytically: the distance to its surface is the
/// distance to the axis minus the radius, so the value is exact and does not
/// depend on a mesh. Everything else goes through the triangle mesh.
pub fn abstand(k: &Koerper, x: [f64; 3]) -> Punkt {
    match k {
        &Koerper::Zylinder {
            punkt,
            achse,
            radius,
            ..
        } => {
            let a = mul(achse, 1.0 / norm(achse));
            let d = sub(x, punkt);
            let quer = sub(d, mul(a, dot(d, a)));
            let r = norm(quer);
            let normale = if r > 1e-12 { mul(quer, 1.0 / r) } else { a };
            Punkt {
                abstand: r - radius,
                punkt: add(punkt, mul(normale, radius)),
                normale,
            }
        }
        Koerper::Netz { netz, versatz, .. } => {
            let n: Naechster = netz.abstand_ohne_grenze(sub(x, *versatz));
            Punkt {
                abstand: n.abstand,
                punkt: add(n.punkt, *versatz),
                normale: n.normale,
            }
        }
    }
}
