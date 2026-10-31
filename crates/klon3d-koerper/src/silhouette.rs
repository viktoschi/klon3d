//! Silhouette check for a body mesh: self-intersection and dents (stage 2).
//!
//! Closes the point that `PLAN.md` stage 2 still listed as open (finding 24)
//! with measurements that are defined once, in the code, and not fitted to any
//! body.
//!
//! ## Self-intersection
//!
//! Two triangles that share no position may not cross. The broad phase is a
//! uniform grid over the triangle boxes; the narrow phase tests every edge of
//! one triangle against the other and, for coplanar pairs, clips the faces
//! against each other for their overlap area. Triangles that share a vertex
//! **position** are skipped: the base mesh is open at its seams and repeats the
//! seam vertices, so triangles meeting across a seam are neighbours, not a
//! defect.
//!
//! Every remaining contact is classified, because they mean different things:
//!
//! * [`Art::Kreuzung`] — one face passes through the other. A real
//!   self-intersection: the surface folds through itself. Reported with its
//!   [`Kontakt::breite_mm`], the penetration width, because on a real body mesh
//!   existence alone says nothing: a graze of a sliver triangle at a seam is
//!   not a fold.
//! * [`Art::Ueberdeckung`] — the two faces are coplanar and their overlap area
//!   is non-zero. Two layers at the same place: a duplicated or superimposed
//!   patch.
//! * [`Art::Beruehrung`] — they touch along an edge or at a vertex without
//!   passing through. Common where a mesh has a T-junction.
//!
//! ## Dent
//!
//! A vertex counts as a dent when it sits below the plane through the centroid
//! of its one-ring, measured along the area-weighted vertex normal, by at least
//! [`Parameter::delle_anteil`] of its mean ring radius **and** at least
//! [`Parameter::delle_mm`]. Both are relative to the mesh, so the rule carries
//! over to any body.
//!
//! Why a genuine concavity does not trip it: across a ring of diameter `d` and
//! local radius `R`, a smooth concave patch sags by `d²/(2R)`. With 7 mm edges
//! and `R = 50 mm` that is 0.5 mm — below a quarter ring radius. A fold or a
//! wrong shaping step produces a notch several millimetres deep, and that is
//! what the rule catches.
//!
//! Vertices on an open edge are skipped: their ring is one-sided, so its
//! centroid lies behind them by construction.
//!
//! ## What the numbers mean
//!
//! The base mesh a body is shaped from is not clean: its seams are open, it
//! repeats the seam vertices, and it carries thin triangles and genuine
//! overlaps. A check that demanded "no contact" would fail on the untouched
//! body. [`neue_kontakte`] therefore compares two checks of the same topology
//! and reports only what grew — in width, not in count.
//!
//! Not detected: two exactly coplanar faces where one lies completely inside
//! the other and they share no boundary point.

use std::collections::{HashMap, HashSet};

use crate::{Netz, Punkt3};

/// Settings of the silhouette check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parameter {
    /// Edge length of the broad-phase grid (mm). In the order of the mesh
    /// edges: smaller cells cost memory, larger cells cost pair tests.
    pub zelle_mm: f64,
    /// A vertex counts as a dent when it sinks by at least this fraction of
    /// its mean ring radius.
    pub delle_anteil: f64,
    /// …and by at least this many millimetres.
    pub delle_mm: f64,
    /// Cap on the collected contact list; above it the list is truncated and
    /// [`Befund::gekuerzt`] says so.
    pub hoechstzahl: usize,
}

impl Default for Parameter {
    fn default() -> Self {
        Parameter {
            zelle_mm: 20.0,
            delle_anteil: 0.25,
            delle_mm: 0.5,
            hoechstzahl: 20_000,
        }
    }
}

/// What kind of contact two triangles have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Art {
    /// Surfaces touch along an edge or at a vertex, without passing through.
    Beruehrung,
    /// Coplanar faces whose projections overlap.
    Ueberdeckung,
    /// One face passes through the other.
    Kreuzung,
}

impl Art {
    pub fn name(self) -> &'static str {
        match self {
            Art::Beruehrung => "Berührung",
            Art::Ueberdeckung => "Überdeckung",
            Art::Kreuzung => "Kreuzung",
        }
    }
}

/// One reported contact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kontakt {
    pub a: usize,
    pub b: usize,
    pub art: Art,
    /// How far a crossing point stays from the border of the face it passes
    /// through (mm); 0 for a graze along an edge or a vertex.
    pub breite_mm: f64,
}

impl Kontakt {
    pub fn neu(a: usize, b: usize, art: Art, breite_mm: f64) -> Self {
        Kontakt {
            a,
            b,
            art,
            breite_mm,
        }
    }
}

/// Result of the check.
#[derive(Debug, Clone)]
pub struct Befund {
    pub dreiecke: usize,
    /// Triangle pairs the broad phase handed to the narrow phase.
    pub paare: usize,
    /// Pairs skipped because they share a vertex position (neighbours across an
    /// open seam of the base mesh).
    pub ueber_naht: usize,
    /// Every contact, sorted by [`Art`] and then by index.
    pub kontakte: Vec<Kontakt>,
    /// The contact list hit [`Parameter::hoechstzahl`].
    pub gekuerzt: bool,
    /// Vertices that sink below their one-ring plane by more than the
    /// threshold.
    pub dellen: usize,
    /// Deepest sink: vertex index and depth in mm (positive = below).
    pub tiefste_delle: Option<(usize, f64)>,
    /// Median mean ring radius over all interior vertices (mm), so that the
    /// dent threshold can be read against the mesh.
    pub ring_median_mm: f64,
    /// Vertices without a ring (isolated points).
    pub ohne_ring: usize,
    /// Vertices on an open edge, skipped in the dent test.
    pub am_rand: usize,
}

impl Befund {
    pub fn zahl(&self, art: Art) -> usize {
        self.kontakte.iter().filter(|k| k.art == art).count()
    }

    /// All contacts of one kind, as index pairs.
    pub fn paare_von(&self, art: Art) -> Vec<(usize, usize)> {
        self.kontakte
            .iter()
            .filter(|k| k.art == art)
            .map(|k| (k.a, k.b))
            .collect()
    }

    /// Line for a report; says what was found, not what should be.
    pub fn als_text(&self) -> String {
        let mut teile = Vec::new();
        for art in [Art::Kreuzung, Art::Ueberdeckung, Art::Beruehrung] {
            let n = self.zahl(art);
            if n > 0 {
                let eigene: Vec<&Kontakt> = self.kontakte.iter().filter(|k| k.art == art).collect();
                let b: Vec<String> = eigene
                    .iter()
                    .take(3)
                    .map(|k| format!("{}/{}", k.a, k.b))
                    .collect();
                let breit = eigene.iter().map(|k| k.breite_mm).fold(0.0, f64::max);
                teile.push(format!(
                    "{n} {} (z. B. {}), breiteste {breit:.2} mm",
                    art.name(),
                    b.join(", ")
                ));
            }
        }
        if teile.is_empty() {
            teile.push("keine Kontakte".to_string());
        }
        let delle = match self.tiefste_delle {
            Some((i, t)) => format!("{}, tiefste {t:.2} mm bei Punkt {i}", self.dellen),
            None => self.dellen.to_string(),
        };
        format!(
            "{} Dreiecke, {} Paare, {}; {delle} Dellen; Ringradius {:.1} mm{}",
            self.dreiecke,
            self.paare,
            teile.join(", "),
            self.ring_median_mm,
            if self.gekuerzt {
                ", Liste gekürzt"
            } else {
                ""
            }
        )
    }
}

fn sub(a: Punkt3, b: Punkt3) -> Punkt3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: Punkt3, b: Punkt3) -> Punkt3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mal(a: Punkt3, s: f64) -> Punkt3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: Punkt3, b: Punkt3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn kreuz(a: Punkt3, b: Punkt3) -> Punkt3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn laenge(a: Punkt3) -> f64 {
    dot(a, a).sqrt()
}

/// Tolerance for "on the plane" (mm). The meshes are given to 0.01 mm.
const EPS_MM: f64 = 1.0e-4;

/// Tolerance for "inside the triangle" (barycentric).
const EPS_BARY: f64 = 1.0e-9;

/// Barycentric coordinates of `x` with respect to the triangle `t`, if `x`
/// lies in the plane of `t`. `None` when the triangle is degenerate.
fn baryzentrisch(x: Punkt3, t: [Punkt3; 3]) -> Option<[f64; 3]> {
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let d = sub(x, t[0]);
    let (d00, d01, d11) = (dot(e1, e1), dot(e1, e2), dot(e2, e2));
    let den = d00 * d11 - d01 * d01;
    if den <= 0.0 {
        return None;
    }
    let (d20, d21) = (dot(d, e1), dot(d, e2));
    let v = (d11 * d20 - d01 * d21) / den;
    let w = (d00 * d21 - d01 * d20) / den;
    Some([1.0 - v - w, v, w])
}

/// Signed distance of `x` from the plane of `t`, positive on the side of
/// `(t1−t0) × (t2−t0)`. `None` for a degenerate triangle.
fn ebenenabstand(x: Punkt3, t: [Punkt3; 3]) -> Option<f64> {
    let n = kreuz(sub(t[1], t[0]), sub(t[2], t[0]));
    let l = laenge(n);
    if l <= 0.0 {
        return None;
    }
    Some(dot(n, sub(x, t[0])) / l)
}

/// Where the segment `p`–`q` meets the triangle `t`: (distance from `t`'s
/// plane at `p`, at `q`, parameter `s` along the segment, barycentric
/// coordinates of the contact point). `None` when the segment is parallel or
/// misses the face.
fn treffer(p: Punkt3, q: Punkt3, t: [Punkt3; 3]) -> Option<(f64, f64, f64, [f64; 3])> {
    let (dp, dq) = (ebenenabstand(p, t)?, ebenenabstand(q, t)?);
    let nenner = dp - dq;
    if nenner.abs() <= f64::MIN_POSITIVE {
        return None;
    }
    let s = dp / nenner;
    if !(-1.0e-12..=1.0 + 1.0e-12).contains(&s) {
        return None;
    }
    let x = add(p, mal(sub(q, p), s));
    let b = baryzentrisch(x, t)?;
    b.iter().all(|&w| w >= -EPS_BARY).then_some((dp, dq, s, b))
}

/// Area of the triangle.
fn flaeche(t: [Punkt3; 3]) -> f64 {
    0.5 * laenge(kreuz(sub(t[1], t[0]), sub(t[2], t[0])))
}

/// Distance of `x` to the border of the triangle `t`; 0 when `x` lies outside.
/// Measured in the plane of `t`.
fn randabstand(x: Punkt3, t: [Punkt3; 3]) -> f64 {
    let Some(w) = baryzentrisch(x, t) else {
        return 0.0;
    };
    if w.iter().any(|&c| c < 0.0) {
        return 0.0;
    }
    let fl = flaeche(t);
    let mut m = f64::INFINITY;
    for i in 0..3 {
        let gegen = laenge(sub(t[(i + 1) % 3], t[(i + 2) % 3]));
        if gegen > 0.0 {
            // Height over that edge, times the barycentric weight of the
            // opposite vertex.
            m = m.min(w[i] * 2.0 * fl / gegen);
        }
    }
    m
}

/// Penetration width of a pair: the largest distance a crossing point keeps to
/// the border of the face it passes through. A graze along an edge reads near
/// zero; a fold that goes through the middle of the faces reads in the order of
/// the edge length.
fn eindringbreite(a: [Punkt3; 3], b: [Punkt3; 3]) -> f64 {
    let mut m: f64 = 0.0;
    for (aus, ziel) in [(a, b), (b, a)] {
        for i in 0..3 {
            let (p, q) = (aus[i], aus[(i + 1) % 3]);
            let (Some(dp), Some(dq)) = (ebenenabstand(p, ziel), ebenenabstand(q, ziel)) else {
                continue;
            };
            if dp * dq >= 0.0 {
                continue;
            }
            let s = dp / (dp - dq);
            let x = add(p, mal(sub(q, p), s));
            m = m.max(randabstand(x, ziel));
        }
    }
    m
}

/// The two dominant axes of a plane normal: projecting along the third makes
/// the largest possible area.
fn projektion(n: Punkt3) -> (usize, usize) {
    let a = [n[0].abs(), n[1].abs(), n[2].abs()];
    // Drop the axis the normal points along most strongly; what remains spans
    // the largest area.
    let weg = if a[0] >= a[1] && a[0] >= a[2] {
        0
    } else if a[1] >= a[2] {
        1
    } else {
        2
    };
    match weg {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

/// Overlap area of two coplanar triangles, projected onto the plane they span.
/// Computed by clipping one triangle against the other (Sutherland–Hodgman), so
/// two identical triangles read as a full overlap and edge neighbours as zero.
fn ueberdeckungsflaeche(a: [Punkt3; 3], b: [Punkt3; 3]) -> f64 {
    let n = kreuz(sub(a[1], a[0]), sub(a[2], a[0]));
    let (u, v) = projektion(n);
    let p2 = |q: Punkt3| (q[u], q[v]);
    // Positive orientation of `b` in the projection, so that "inside" is
    // consistent no matter how the triangle is wound.
    let b2: Vec<(f64, f64)> = b.iter().map(|&q| p2(q)).collect();
    let fl = |t: &[(f64, f64)]| -> f64 {
        let m = t.len();
        (0..m)
            .map(|i| t[i].0 * t[(i + 1) % m].1 - t[(i + 1) % m].0 * t[i].1)
            .sum::<f64>()
            * 0.5
    };
    let mut vorzeichen = 1.0;
    if fl(&b2) < 0.0 {
        vorzeichen = -1.0;
    }
    // Clip `a` against the three half-planes of `b`.
    let mut poly: Vec<(f64, f64)> = a.iter().map(|&q| p2(q)).collect();
    for i in 0..3 {
        if poly.is_empty() {
            break;
        }
        let (p, q) = (b2[i], b2[(i + 1) % 3]);
        let seite =
            |x: (f64, f64)| vorzeichen * ((q.0 - p.0) * (x.1 - p.1) - (q.1 - p.1) * (x.0 - p.0));
        let mut neu = Vec::with_capacity(poly.len() + 2);
        for j in 0..poly.len() {
            let (x, y) = (poly[j], poly[(j + 1) % poly.len()]);
            let (sx, sy) = (seite(x), seite(y));
            if sx >= 0.0 {
                neu.push(x);
            }
            if (sx > 0.0 && sy < 0.0) || (sx < 0.0 && sy > 0.0) {
                let t = sx / (sx - sy);
                neu.push((x.0 + t * (y.0 - x.0), x.1 + t * (y.1 - x.1)));
            }
        }
        poly = neu;
    }
    fl(&poly).abs()
}

/// Classification of a coplanar pair.
fn koplanar_art(a: [Punkt3; 3], b: [Punkt3; 3]) -> Option<Art> {
    let ueber = ueberdeckungsflaeche(a, b);
    let klein = flaeche(a).min(flaeche(b));
    if klein <= 0.0 {
        return None;
    }
    // A shared edge or vertex has zero area; a real superimposition does not.
    if ueber > 1.0e-6 * klein {
        return Some(Art::Ueberdeckung);
    }
    let beruehrt = (0..3).any(|i| {
        baryzentrisch(a[i], b).map_or(false, |w| w.iter().all(|&v| v >= -EPS_BARY))
            || baryzentrisch(b[i], a).map_or(false, |w| w.iter().all(|&v| v >= -EPS_BARY))
    });
    beruehrt.then_some(Art::Beruehrung)
}

/// Classifies the contact of two triangles that share no position. `None` when
/// they do not touch at all.
fn art(a: [Punkt3; 3], b: [Punkt3; 3]) -> Option<Art> {
    // Coplanar? All six vertices on one plane.
    let e1 = sub(a[1], a[0]);
    let e2 = sub(a[2], a[0]);
    let n = kreuz(e1, e2);
    let l = laenge(n);
    if l > 0.0 {
        let flach = a
            .iter()
            .chain(b.iter())
            .all(|&q| (dot(n, sub(q, a[0])) / l).abs() <= EPS_MM);
        if flach {
            return koplanar_art(a, b);
        }
    }

    // Not coplanar: an edge that passes through the interior of the other face
    // is a crossing; one that only grazes its boundary is a contact.
    let mut beruehrt = false;
    for (aus, ziel) in [(a, b), (b, a)] {
        for i in 0..3 {
            let (p, q) = (aus[i], aus[(i + 1) % 3]);
            let Some((dp, dq, _, bary)) = treffer(p, q, ziel) else {
                continue;
            };
            beruehrt = true;
            // Both ends strictly outside the plane on opposite sides, and the
            // contact strictly inside the face: a transversal crossing.
            let beidseits = dp * dq < 0.0 && dp.abs() > EPS_MM && dq.abs() > EPS_MM;
            let innen = bary.iter().all(|&w| w > EPS_BARY);
            if beidseits && innen {
                return Some(Art::Kreuzung);
            }
        }
    }
    beruehrt.then_some(Art::Beruehrung)
}

/// Cell index of a coordinate on the grid.
fn zelle(x: f64, k: f64) -> i64 {
    (x / k).floor() as i64
}

/// A key for a vertex position: coincident points of the base mesh's open
/// seams land in the same group, so triangles across such a seam count as
/// neighbours instead of as a contact.
fn ortsschluessel(q: Punkt3) -> [i64; 3] {
    // 1 µm; the meshes are given to about 0.01 mm.
    let f = |x: f64| (x * 1000.0).round() as i64;
    [f(q[0]), f(q[1]), f(q[2])]
}

/// Neighbours of every vertex and the triangles around it.
struct Nachbarschaft {
    ring: Vec<Vec<u32>>,
    dreiecke: Vec<Vec<u32>>,
    /// The vertex lies on an open edge (a seam of the base mesh).
    offen: Vec<bool>,
}

fn nachbarschaft(netz: &Netz) -> Nachbarschaft {
    let mut ring: Vec<Vec<u32>> = vec![Vec::new(); netz.punkte.len()];
    let mut dreiecke: Vec<Vec<u32>> = vec![Vec::new(); netz.punkte.len()];
    let mut offen = vec![false; netz.punkte.len()];
    let mut gesehen: HashSet<(u32, u32)> = HashSet::new();
    let mut zahl: HashMap<(u32, u32), usize> = HashMap::new();
    for (ti, d) in netz.dreiecke.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (d[k] as u32, d[(k + 1) % 3] as u32);
            dreiecke[a as usize].push(ti as u32);
            let (x, y) = if a < b { (a, b) } else { (b, a) };
            *zahl.entry((x, y)).or_default() += 1;
            if a != b && gesehen.insert((x, y)) {
                ring[a as usize].push(b);
                ring[b as usize].push(a);
            }
        }
    }
    for ((a, b), c) in zahl {
        if c == 1 {
            offen[a as usize] = true;
            offen[b as usize] = true;
        }
    }
    Nachbarschaft {
        ring,
        dreiecke,
        offen,
    }
}

/// Area-weighted normal of the triangles around `i`.
///
/// The sum of the unnormalised face normals `(b−a) × (c−a)` points outwards for
/// a consistently wound mesh and weights each face by its area.
fn normal(netz: &Netz, umgebung: &[u32]) -> Option<Punkt3> {
    let mut n = [0.0; 3];
    for &ti in umgebung {
        let d = netz.dreiecke[ti as usize];
        let (a, b, c) = (
            netz.punkte[d[0] as usize],
            netz.punkte[d[1] as usize],
            netz.punkte[d[2] as usize],
        );
        n = add(n, kreuz(sub(b, a), sub(c, a)));
    }
    let l = laenge(n);
    (l > 0.0).then(|| mal(n, 1.0 / l))
}

/// Checks one mesh.
pub fn pruefe(netz: &Netz, p: &Parameter) -> Befund {
    let n = netz.punkte.len();
    let mut befund = Befund {
        dreiecke: netz.dreiecke.len(),
        paare: 0,
        ueber_naht: 0,
        kontakte: Vec::new(),
        gekuerzt: false,
        dellen: 0,
        tiefste_delle: None,
        ring_median_mm: 0.0,
        ohne_ring: 0,
        am_rand: 0,
    };
    if netz.punkte.is_empty() {
        return befund;
    }

    // --- Self-intersection: broad phase over a grid of the triangle boxes ---
    let k = p.zelle_mm.max(1.0e-6);
    let mut gitter: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
    let mut quader: Vec<([f64; 3], [f64; 3])> = Vec::with_capacity(netz.dreiecke.len());
    for (ti, d) in netz.dreiecke.iter().enumerate() {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for &i in d {
            let q = netz.punkte[i as usize];
            for a in 0..3 {
                lo[a] = lo[a].min(q[a]);
                hi[a] = hi[a].max(q[a]);
            }
        }
        quader.push((lo, hi));
        let (i0, i1) = (zelle(lo[0], k), zelle(hi[0], k));
        let (j0, j1) = (zelle(lo[1], k), zelle(hi[1], k));
        let (m0, m1) = (zelle(lo[2], k), zelle(hi[2], k));
        for i in i0..=i1 {
            for j in j0..=j1 {
                for m in m0..=m1 {
                    gitter.entry((i, j, m)).or_default().push(ti as u32);
                }
            }
        }
    }
    let mut geprueft: HashSet<(u32, u32)> = HashSet::new();
    let dreieck = |ti: u32| -> [Punkt3; 3] {
        let d = netz.dreiecke[ti as usize];
        [
            netz.punkte[d[0] as usize],
            netz.punkte[d[1] as usize],
            netz.punkte[d[2] as usize],
        ]
    };
    for zellen in gitter.values() {
        for (u, &a) in zellen.iter().enumerate() {
            for &b in &zellen[u + 1..] {
                let (x, y) = if a < b { (a, b) } else { (b, a) };
                if !geprueft.insert((x, y)) {
                    continue;
                }
                let (da, db) = (netz.dreiecke[x as usize], netz.dreiecke[y as usize]);
                // Neighbours share a position; the base mesh is open at its
                // seams and repeats the seam vertices.
                let gleich = |i: usize, j: usize| {
                    i as u32 == j as u32
                        || ortsschluessel(netz.punkte[i]) == ortsschluessel(netz.punkte[j])
                };
                if da
                    .iter()
                    .any(|&i| db.iter().any(|&j| gleich(i as usize, j as usize)))
                {
                    befund.ueber_naht += 1;
                    continue;
                }
                let (qa, qb) = (quader[x as usize], quader[y as usize]);
                if (0..3).any(|z| qa.0[z] > qb.1[z] || qb.0[z] > qa.1[z]) {
                    continue;
                }
                befund.paare += 1;
                let Some(kind) = art(dreieck(x), dreieck(y)) else {
                    continue;
                };
                let breite = if kind == Art::Kreuzung {
                    eindringbreite(dreieck(x), dreieck(y))
                } else {
                    0.0
                };
                if befund.kontakte.len() < p.hoechstzahl {
                    befund
                        .kontakte
                        .push(Kontakt::neu(x as usize, y as usize, kind, breite));
                } else {
                    befund.gekuerzt = true;
                }
            }
        }
    }
    // Strongest kind first, then by index: the report reads top-down.
    befund.kontakte.sort_by(|x, y| {
        y.art
            .cmp(&x.art)
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });

    // --- Dents: sink below the plane of the own ring ----------------------
    let nb = nachbarschaft(netz);
    let mut ringradien: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let ring = &nb.ring[i];
        if ring.is_empty() || nb.dreiecke[i].is_empty() {
            befund.ohne_ring += 1;
            continue;
        }
        // A vertex on an open edge has a one-sided ring: its centroid lies
        // behind it by construction, so it is a border, not a dent.
        if nb.offen[i] {
            befund.am_rand += 1;
            continue;
        }
        let pi = netz.punkte[i];
        let mut c = [0.0; 3];
        let mut r = 0.0;
        for &j in ring {
            let q = netz.punkte[j as usize];
            c = add(c, q);
            r += laenge(sub(q, pi));
        }
        let m = ring.len() as f64;
        c = mal(c, 1.0 / m);
        r /= m;
        ringradien.push(r);
        let Some(nrm) = normal(netz, &nb.dreiecke[i]) else {
            continue;
        };
        // Signed distance of the vertex from the plane through the ring
        // centroid, along the outward normal: positive when the vertex pokes
        // out (convex), negative when it sits below the ring (a dent).
        let senkung = dot(sub(c, pi), nrm);
        let grenze = (p.delle_anteil * r).max(p.delle_mm);
        if senkung > grenze {
            befund.dellen += 1;
            if befund.tiefste_delle.map_or(true, |(_, t)| senkung > t) {
                befund.tiefste_delle = Some((i, senkung));
            }
        }
    }
    ringradien.sort_by(f64::total_cmp);
    befund.ring_median_mm = if ringradien.is_empty() {
        0.0
    } else {
        ringradien[ringradien.len() / 2]
    };
    befund
}

/// Does the shaping add contacts? Compares two checks of meshes with the same
/// topology. What counts is not existence but width: a pair may already graze
/// in the base mesh and go deeper after shaping. Reported is every pair whose
/// penetration width grew by more than `toleranz_mm`, plus the deepest dent if
/// it grew by more than that.
pub fn neue_kontakte(
    vorher: &Befund,
    nachher: &Befund,
    toleranz_mm: f64,
) -> (Vec<Kontakt>, Option<(usize, f64)>) {
    let alt: HashMap<(usize, usize), f64> = vorher
        .kontakte
        .iter()
        .map(|k| ((k.a, k.b), k.breite_mm))
        .collect();
    let neu: Vec<Kontakt> = nachher
        .kontakte
        .iter()
        .copied()
        .filter(|k| k.breite_mm > alt.get(&(k.a, k.b)).copied().unwrap_or(0.0) + toleranz_mm)
        .collect();
    let delle = match (vorher.tiefste_delle, nachher.tiefste_delle) {
        (Some((_, a)), Some((i, b))) if b > a + toleranz_mm => Some((i, b)),
        (None, Some((i, b))) if b > toleranz_mm => Some((i, b)),
        _ => None,
    };
    (neu, delle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sphere as a mesh: one pole at the top and at the bottom, rings
    /// between. Closed and convex, so without a contact or a dent.
    fn kugel(radius: f64, ringe: usize, segmente: usize) -> Netz {
        assert!(ringe >= 3 && segmente >= 4);
        let mut netz = Netz::default();
        for r in 1..ringe {
            let phi = std::f64::consts::PI * r as f64 / ringe as f64;
            for s in 0..segmente {
                let th = 2.0 * std::f64::consts::PI * s as f64 / segmente as f64;
                netz.punkte.push([
                    radius * phi.sin() * th.cos(),
                    radius * phi.cos(),
                    radius * phi.sin() * th.sin(),
                ]);
            }
        }
        let oben = netz.punkte.len();
        netz.punkte.push([0.0, radius, 0.0]);
        let unten = netz.punkte.len();
        netz.punkte.push([0.0, -radius, 0.0]);
        let ring = |r: usize, s: usize| (r - 1) * segmente + s % segmente;
        for s in 0..segmente {
            netz.dreiecke.push([oben, ring(1, s + 1), ring(1, s)]);
        }
        for r in 1..ringe - 1 {
            for s in 0..segmente {
                let (a, b) = (ring(r, s), ring(r, s + 1));
                let (c, d) = (ring(r + 1, s + 1), ring(r + 1, s));
                netz.dreiecke.push([a, b, c]);
                netz.dreiecke.push([a, c, d]);
            }
        }
        for s in 0..segmente {
            netz.dreiecke
                .push([unten, ring(ringe - 1, s), ring(ringe - 1, s + 1)]);
        }
        netz
    }

    /// A closed, convex sphere has neither contact nor dent.
    #[test]
    fn sphere_is_clean() {
        let b = pruefe(&kugel(100.0, 24, 48), &Parameter::default());
        assert_eq!(b.kontakte.len(), 0, "{}", b.als_text());
        assert_eq!(b.dellen, 0, "{}", b.als_text());
        assert!(b.paare > 0, "Grobphase hat nichts geprüft");
        assert!(
            b.ring_median_mm > 5.0 && b.ring_median_mm < 20.0,
            "{}",
            b.ring_median_mm
        );
    }

    /// Two non-adjacent triangles crossing each other are found and classified
    /// as a crossing; lifted by 30 mm they are gone.
    #[test]
    fn crossing_triangles_are_found() {
        let a = [[-10.0, 0.0, -10.0], [10.0, 0.0, -10.0], [0.0, 0.0, 10.0]];
        let b = [[0.0, -10.0, 0.0], [0.0, 10.0, 0.0], [0.0, 0.0, 5.0]];
        assert_eq!(art(a, b), Some(Art::Kreuzung));
        let hoch = b.map(|q| [q[0], q[1], q[2] + 30.0]);
        assert_eq!(art(a, hoch), None, "falscher Treffer");
    }

    /// Two coplanar, overlapping faces read as an overlap, not as a crossing.
    #[test]
    fn coplanar_overlap_is_an_overlap() {
        let a = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];
        let b = [[2.0, 2.0, 0.0], [12.0, 2.0, 0.0], [2.0, 12.0, 0.0]];
        assert_eq!(art(a, b), Some(Art::Ueberdeckung));
        // Neben einander, nur die Kante gemeinsam: Berührung.
        let c = [[10.0, 0.0, 0.0], [10.0, 10.0, 0.0], [0.0, 10.0, 0.0]];
        assert_eq!(art(a, c), Some(Art::Beruehrung));
    }

    /// Twin triangles at the same place are a full overlap, not a border touch:
    /// the mirrored halves of a body meet that way at the centre seam.
    #[test]
    fn identical_triangles_are_a_full_overlap() {
        let a = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];
        assert_eq!(art(a, a), Some(Art::Ueberdeckung));
        // Ein halb so großes Dreieck ganz innen deckt die halbe Fläche.
        let innen = [[1.0, 1.0, 0.0], [5.0, 1.0, 0.0], [1.0, 5.0, 0.0]];
        let ueber = ueberdeckungsflaeche(a, innen);
        let eigen = flaeche(innen);
        assert!(
            (ueber - eigen).abs() < 1.0e-9,
            "Überdeckung {ueber:.4} gegen Fläche {eigen:.4} mm²"
        );
    }

    /// The penetration width separates a graze from a real fold: a crossing
    /// through the middle of the faces is wide, one at their border is not.
    #[test]
    fn penetration_width_tells_a_graze_from_a_fold() {
        // `a` liegt in der Ebene z = 0, `b` steht senkrecht darauf.
        let a = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];
        // Mitten durch: die Schnittlinie liegt weit vom Rand entfernt.
        let mitte = [[2.0, 2.0, -5.0], [2.0, 2.0, 5.0], [8.0, 2.0, 0.0]];
        // Nur am Rand: die Schnittlinie läuft durch den Eckpunkt.
        let rand = [[0.0, 0.0, -5.0], [0.0, 0.0, 5.0], [0.0, 8.0, 0.0]];
        let (w_mitte, w_rand) = (eindringbreite(a, mitte), eindringbreite(a, rand));
        assert!(w_mitte > 0.5, "Mitte {w_mitte:.3} mm");
        assert!(w_mitte < 5.0, "Mitte {w_mitte:.3} mm");
        assert!(w_rand < 1.0e-9, "Rand {w_rand:.3} mm");
        assert!(w_mitte > w_rand, "Unterscheidung greift nicht");
    }

    /// A pair that only grazes wider after shaping is reported as grown; one
    /// that keeps its width is not.
    #[test]
    fn only_a_wider_contact_counts_as_new() {
        let mut bv = pruefe(&Netz::default(), &Parameter::default());
        bv.kontakte = vec![Kontakt::neu(1, 2, Art::Kreuzung, 0.1)];
        let mut bn = bv.clone();
        bn.kontakte = vec![Kontakt::neu(1, 2, Art::Kreuzung, 0.3)];
        assert!(neue_kontakte(&bv, &bn, 0.5).0.is_empty(), "gleiche Breite");
        assert_eq!(neue_kontakte(&bv, &bn, 0.1).0.len(), 1, "Zunahme 0,2 mm");
        // Ein neues Paar ohne Vorgeschichte zählt ab seiner eigenen Breite.
        bn.kontakte.push(Kontakt::neu(3, 4, Art::Kreuzung, 0.8));
        assert_eq!(neue_kontakte(&bv, &bn, 0.5).0.len(), 1, "neues Paar 0,8 mm");
    }

    /// A pushed-in point reads as a dent; the sphere does not.
    #[test]
    fn dented_sphere_has_a_dent() {
        let mut netz = kugel(100.0, 24, 48);
        let oben = netz
            .punkte
            .iter()
            .position(|q| q[1] > 99.0)
            .expect("oberster Punkt");
        let tief = netz.punkte[oben][1] - 20.0;
        netz.punkte[oben][1] = tief;
        let b = pruefe(&netz, &Parameter::default());
        assert!(b.dellen >= 1, "{}", b.als_text());
        let (i, d) = b.tiefste_delle.expect("Tiefe");
        assert!(d > 1.0, "Tiefe {d:.2} mm");
        assert!(
            netz.punkte[i][1] < tief + 30.0,
            "Punkt {i} y {}",
            netz.punkte[i][1]
        );
    }

    /// A seam that repeats its vertices is not a contact: the two halves of the
    /// base mesh meet there, they do not cross.
    #[test]
    fn doubled_seam_vertices_are_neighbours() {
        // Zwei Hälften eines Quadrats, an der Naht x = 0 doppelt.
        let mut netz = Netz::default();
        netz.punkte = vec![
            [0.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [-10.0, 0.0, 0.0],
            [-10.0, 10.0, 0.0],
            // dieselben beiden Nahtpunkte, eigene Indizes
            [0.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
        ];
        netz.dreiecke = vec![[0, 2, 3], [0, 3, 1], [4, 5, 7], [4, 7, 6]];
        let b = pruefe(&netz, &Parameter::default());
        assert_eq!(b.kontakte.len(), 0, "{}", b.als_text());
        assert!(b.ueber_naht > 0, "Naht nicht erkannt");
    }

    /// A flat square is clean.
    #[test]
    fn plane_is_clean() {
        let mut netz = Netz::default();
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)] {
            netz.punkte.push([x, y, 0.0]);
        }
        netz.dreiecke = vec![[0, 1, 2], [0, 2, 3]];
        let b = pruefe(&netz, &Parameter::default());
        assert_eq!(b.kontakte.len(), 0, "{}", b.als_text());
        assert_eq!(b.dellen, 0);
    }
}
