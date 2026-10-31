//! CLO 2026.1: the 67 measurements, with fixed points instead of candidate search.
//!
//! The basis are the methods from [`crate::messung`] (5.2) and
//! [`crate::massband`]: edge intersection, 0,5-cm filter, 20-cm chain,
//! direction of traversal, convex hull, paths, heights, straight lines, difference, sum,
//! averaging. They apply unchanged. This module implements the changes from
//! `work/spez/messung_2026_1.md`:
//!
//! 1. **No candidate search.** Bust [L18, L19], underbust [L22] in the new
//!    segment 7, waist [L24], HighHip [L26], LowHip [L29], calf [L37]/[L38]
//!    are fixed in `IndexSet.txt`.
//! 2. **Bust bridge.** At the bust the hull is relieved before closing around
//!    the points at the front between the bust points (the band
//!    bridges the décolleté).
//! 3. **New intersection rule** ([`kreuzung26`]): closed ring, after a
//!    hit the following segment is skipped, the first hit counts,
//!    with two of them the front or back one.
//! 4. **Projected points.** L18/L19 (and in paths L24/L26) are placed onto
//!    the raw chain of their circumference before the path.
//! 5. **New measurements** 58 `LENGTH_Hand` (polyline L104…L113) and
//!    59 `CIRCUMFERENCE_Head` (plane through L114, L115, L116); 60…66 are
//!    sliders without a measurement method.
//!
//! All lengths in centimetres, measured on the whole mesh with the edges from
//! `BaseMesh.be`. Checked on three meshes from CLO's memory: ≤ 0,04 mm.

use crate::massband::{abstand, huelle, segment, umfangsschnitt_f32, Messdaten, WAAGERECHT};
use crate::messung::{weg, Eintrag, Lm};
use crate::Punkt3;

/// Number of measurements in 2026.1 (58 like 5.2 plus hand, head and seven sliders).
pub const ANZAHL: usize = 67;

/// Names of the measurements in slot order (`formung_2026_1.md` 3).
#[rustfmt::skip]
pub const NAMEN: [&str; ANZAHL] = [
    "HEIGHT_Total", "WEIGHT_Total", "CIRCUMFERENCE_Bust", "CIRCUMFERENCE_MidNeck",
    "CIRCUMFERENCE_NeckBase", "CIRCUMFERENCE_UnderBust", "CIRCUMFERENCE_Waist",
    "CIRCUMFERENCE_HighHip", "CIRCUMFERENCE_LowHip", "CIRCUMFERENCE_Thigh",
    "CIRCUMFERENCE_MidThigh", "CIRCUMFERENCE_Knee", "CIRCUMFERENCE_Calf",
    "CIRCUMFERENCE_Ankle", "CIRCUMFERENCE_Armhole", "CIRCUMFERENCE_Bicep",
    "CIRCUMFERENCE_Elbow", "CIRCUMFERENCE_Wrist", "HEIGHT_HPS", "HEIGHT_Shoulder",
    "HEIGHT_ShoulderDrop", "HEIGHT_APEX", "HEIGHT_UnderBust", "HEIGHT_Waist",
    "HEIGHT_HighHip", "HEIGHT_LowHip", "HEIGHT_Crotch", "HEIGHT_Thigh",
    "HEIGHT_MidThigh", "HEIGHT_Knee", "HEIGHT_Calf", "HEIGHT_Ankle",
    "LENGTH_Bust_Width", "LENGTH_Bust_Depth", "LENGTH_AcrossFront", "LENGTH_AcrossBack",
    "LENGTH_AcrossShoulder", "LENGTH_CBNeck_Shoulder", "LENGTH_CFNeck_AcrossFront",
    "LENGTH_CBNeck_AcrossBack", "LENGTH_CFNeck_Waist", "LENGTH_CBNeck_Waist",
    "LENGTH_Waist_HighHip", "LENGTH_Waist_LowHip", "LENGTH_APEX_APEX", "LENGTH_HPS_APEX",
    "LENGTH_Halter", "LENGTH_Arm", "LENGTH_CBNeck_Wrist", "LENGTH_Total_Rise",
    "LENGTH_Front_Rise", "LENGTH_Back_Rise", "LENGTH_CrotchDepth", "LENGTH_CrotchWidth",
    "LENGTH_Crothch_MidThigh", "LENGTH_VerticalTrunk_Front", "LENGTH_VerticalTrunk_Back",
    "LENGTH_VerticalTrunk", "LENGTH_Hand", "CIRCUMFERENCE_Head", "SCALE_CrotchWidth",
    "SCALE_CrotchVolume", "SCALE_Breast_Shape", "SCALE_Breast_Space", "SCALE_Breast_Height",
    "SCALE_Hip_Depth", "SCALE_Hip_Width",
];

/// Method per slot; the square root in feature space applies to types 0, 1 and 3
/// (`formung_2026_1.md` 5).
#[rustfmt::skip]
pub const TYP: [u8; ANZAHL] = [
    2, 3, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5,
    7, 5, 7, 5, 5, 6, 6, 4, 6, 5, 5, 7, 4, 0, 8, 8, 8, 8, 8, 8, 8,
];

/// Slot of a measurement by name.
pub fn platz(name: &str) -> Option<usize> {
    NAMEN.iter().position(|n| *n == name)
}

/// Is the measurement measured? Only the weight (1) and the sliders (60…66) are not
/// (`formung_2026_1.md` 3); the weight carries the constant of the configuration.
pub fn gemessen(j: usize) -> bool {
    j < 60 && j != 1
}

/// A measurement slot: points, segment, direction, result.
#[derive(Clone, Debug, Default)]
pub struct Slot {
    /// Row values from `IndexSet.txt`; negative numbers are computed points.
    pub idx: Vec<i64>,
    pub seg: i32,
    pub hinten: bool,
    /// Value of the slot, cm.
    pub wert: f64,
    /// Result line (for paths and circumferences).
    pub pts: Vec<Punkt3>,
    /// Raw line from which the computed points come.
    pub roh: Vec<Punkt3>,
}

/// A measurement with one or two slots.
#[derive(Clone, Debug, Default)]
pub struct Mass {
    pub typ: u8,
    pub beid: bool,
    pub mittel: bool,
    pub slots: [Slot; 2],
    /// Value of the measurement, cm.
    pub wert: f64,
}

/// Result of a measurement: the 67 values and the slots per measurement.
#[derive(Clone, Debug)]
pub struct Messung {
    /// The 67 values, cm (weight 0, sliders 0).
    pub werte: [f64; ANZAHL],
    /// Values of slots 0 and 1 per measurement; `NaN` where there is no slot.
    pub slots: [[f64; 2]; ANZAHL],
}

/// Intersection of a **closed** ring with the plane `x = q.x`
/// (`messung_2026_1.md` 4).
///
/// After a hit the following segment is skipped, so that a
/// centre-line point with `x = q.x` does not count twice. The result is the
/// first hit; with at least two hits the front (larger z) resp.
/// back (smaller z) one of the first two. Without a hit `(0, 0, 0)`.
pub fn kreuzung26(ring: &[Punkt3], q: Punkt3, hinten: bool) -> Punkt3 {
    let n = ring.len();
    if n == 0 {
        return [0.0; 3];
    }
    let mut treffer: Vec<Punkt3> = Vec::new();
    let mut i = 0;
    while i < n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let d0 = a[0] - q[0];
        let d1 = b[0] - q[0];
        if d1 * d0 <= 0.0 {
            let (w0, w1) = (d0.abs(), d1.abs());
            let s = w0 + w1;
            treffer.push(if s != 0.0 {
                [
                    (b[0] * w0 + a[0] * w1) / s,
                    (b[1] * w0 + a[1] * w1) / s,
                    (b[2] * w0 + a[2] * w1) / s,
                ]
            } else {
                a
            });
            i += 1;
        }
        i += 1;
    }
    let Some(&r) = treffer.first() else {
        return [0.0; 3];
    };
    if treffer.len() > 1 {
        let (z0, z1) = (treffer[0][2], treffer[1][2]);
        if (!hinten && z1 > z0) || (hinten && z0 > z1) {
            return treffer[1];
        }
    }
    r
}

/// Normal of the plane through three mesh points (type 0 with three points), computed in `f32`
/// as in CLO.
pub fn normale_drei(v: &[Punkt3], a: usize, b: usize, c: usize) -> Punkt3 {
    normale_f32(v[a], v[b], v[c]).unwrap_or(WAAGERECHT)
}

/// Normal to three points: `normiert((B − A) × (C − A))`, all in `f32`.
///
/// CLO computes the measurement in single precision; only then the three
/// definition points demonstrably lie next to their own plane (about 2,4·10⁻⁷ cm)
/// instead of computationally exactly on it. That is the prerequisite for the
/// edge intersection not depending on the summation order — see
/// [`crate::massband::kantenschnitt`].
pub fn normale_f32(a: Punkt3, b: Punkt3, c: Punkt3) -> Option<Punkt3> {
    let f = |p: Punkt3| [p[0] as f32, p[1] as f32, p[2] as f32];
    let (a, b, c) = (f(a), f(b), f(c));
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let k = [
        u[1] * w[2] - u[2] * w[1],
        u[2] * w[0] - u[0] * w[2],
        u[0] * w[1] - u[1] * w[0],
    ];
    let l = (k[0] * k[0] + k[1] * k[1] + k[2] * k[2]).sqrt();
    if l == 0.0 || !l.is_finite() {
        return None;
    }
    Some([(k[0] / l) as f64, (k[1] / l) as f64, (k[2] / l) as f64])
}

/// Closed length of a point sequence.
fn geschlossen(p: &[Punkt3]) -> f64 {
    let n = p.len();
    if n == 0 {
        return 0.0;
    }
    (0..n).map(|i| abstand(p[i], p[(i + 1) % n])).sum()
}

/// The measurement of a mesh.
pub struct Messer {
    v: Vec<Punkt3>,
    d: Messdaten,
    /// `IndexSet.txt`, raw (including the placeholders −1…−9).
    l: Vec<i64>,
    lm: Lm,
    m: Vec<Mass>,
}

impl Messer {
    /// Builds the measurement table from `IndexSet.txt` and measures nothing yet.
    pub fn neu(v: &[Punkt3], d: &Messdaten) -> Result<Messer, String> {
        d.pruefe(v.len())?;
        let l = d.landmarken.clone();
        let lm = Lm {
            l5: zeile(&l, 5)? as usize,
            l8: zeile(&l, 8)? as usize,
            l9: zeile(&l, 9)? as usize,
            l18: us(&l, 18),
            l19: us(&l, 19),
        };
        let m = definiere(&l)?;
        Ok(Messer {
            v: v.to_vec(),
            d: d.clone(),
            l,
            lm,
            m,
        })
    }

    /// The measurement table of a pass (for checks and the chain).
    pub fn masse(&self) -> &[Mass] {
        &self.m
    }

    /// A slot of a measurement (for checks and the post-processing).
    pub fn slot(&self, j: usize, k: usize) -> &Slot {
        &self.m[j].slots[k]
    }

    pub fn punkte(&self) -> &[Punkt3] {
        &self.v
    }

    /// Row `z` of the `IndexSet`, if it is a mesh point.
    fn netzpunkt(&self, z: usize) -> Option<usize> {
        us(&self.l, z)
    }

    /// Point of a placeholder (`i < 0`) by the 2026 rule; otherwise the zero point.
    fn berechnet(&self, i: i64) -> Punkt3 {
        let gleich = |z: usize| self.l.get(z).copied() == Some(i);
        let punkt = |z: usize| -> Option<Punkt3> {
            let j = self.netzpunkt(z)?;
            self.v.get(j).copied()
        };
        if gleich(25) {
            // −2: waist, back
            if let (Some(q), Some(roh)) = (punkt(24), Some(&self.m[6].slots[0].roh)) {
                if !roh.is_empty() {
                    return kreuzung26(roh, q, true);
                }
            }
        }
        if gleich(27) {
            // −3: HighHip, back
            if let (Some(q), Some(roh)) = (punkt(26), Some(&self.m[7].slots[0].roh)) {
                if !roh.is_empty() {
                    return kreuzung26(roh, q, true);
                }
            }
        }
        if gleich(28) {
            // −4: LowHip, front
            if let (Some(q), Some(roh)) = (punkt(29), Some(&self.m[8].slots[0].roh)) {
                if !roh.is_empty() {
                    return kreuzung26(roh, q, false);
                }
            }
        }
        if gleich(10) {
            // −5: AcrossFront, front, at the height of the shoulder points
            if let (Some(a), Some(b), Some(c)) = (punkt(4), punkt(12), punkt(13)) {
                let q = [a[0], (b[1] + c[1]) * 0.5, a[2]];
                if !self.m[34].slots[0].roh.is_empty() {
                    return kreuzung26(&self.m[34].slots[0].roh, q, false);
                }
            }
        }
        if gleich(11) {
            // −6: AcrossBack, back
            if let (Some(a), Some(b), Some(c)) = (punkt(5), punkt(14), punkt(15)) {
                let q = [a[0], (b[1] + c[1]) * 0.5, a[2]];
                if !self.m[35].slots[0].roh.is_empty() {
                    return kreuzung26(&self.m[35].slots[0].roh, q, true);
                }
            }
        }
        [0.0; 3]
    }

    /// Projection of a mesh point onto the raw chain of its circumference
    /// (`messung_2026_1.md` 4), otherwise `None`.
    fn projiziert(&self, i: usize, weg_modus: bool) -> Option<Punkt3> {
        let q = self.v[i];
        let brust = self.netzpunkt(18) == Some(i) || self.netzpunkt(19) == Some(i);
        if brust {
            return Some(kreuzung26(&self.m[2].slots[0].roh, q, false));
        }
        if weg_modus && self.netzpunkt(24) == Some(i) {
            return Some(kreuzung26(&self.m[6].slots[0].roh, q, false));
        }
        if weg_modus && self.netzpunkt(26) == Some(i) {
            return Some(kreuzung26(&self.m[7].slots[0].roh, q, false));
        }
        None
    }

    /// Point of a slot entry.
    fn punkt(&self, i: i64, weg_modus: bool) -> Punkt3 {
        if i < 0 {
            return self.berechnet(i);
        }
        let i = i as usize;
        if i >= self.v.len() {
            return [0.0; 3];
        }
        self.projiziert(i, weg_modus).unwrap_or(self.v[i])
    }

    /// Entries of a slot; the mesh point number is kept for the rules of the
    /// plane.
    fn eintraege(&self, idx: &[i64], weg_modus: bool) -> Vec<Eintrag> {
        idx.iter()
            .map(|&i| {
                let p = self.punkt(i, weg_modus);
                (p, if i >= 0 { Some(i as usize) } else { None })
            })
            .collect()
    }

    /// Extreme point of the raw chain in x (smallest or largest), for the
    /// bust width (−7/−8).
    fn rand(&self, roh: &[Punkt3], kleinster: bool) -> Punkt3 {
        let mut best: Option<Punkt3> = None;
        let mut schranke = if kleinster { f64::INFINITY } else { f64::NEG_INFINITY };
        for p in roh {
            if (kleinster && p[0] < schranke) || (!kleinster && p[0] > schranke) {
                schranke = p[0];
                best = Some(*p);
            }
        }
        best.unwrap_or([0.0; 3])
    }

    /// Type 0: circumference through a horizontal or inclined plane
    /// (`messung_2026_1.md` 3).
    fn umfang(&mut self, j: usize, k: usize) {
        let idx = self.m[j].slots[k].idx.clone();
        let seg = self.m[j].slots[k].seg;
        let mut p0 = match idx.first().and_then(|&i| usize::try_from(i).ok()) {
            Some(i) => self.v[i],
            None => [0.0; 3],
        };
        let mut n = WAAGERECHT;
        match idx.len() {
            2 => {
                // Bust: horizontal at the mean height of the two points.
                if let (Some(a), Some(b)) = (
                    usize::try_from(idx[0]).ok(),
                    usize::try_from(idx[1]).ok(),
                ) {
                    p0[1] = (self.v[a][1] + self.v[b][1]) * 0.5;
                }
            }
            3 => {
                // Plane through three points, in `f32` as in CLO (see
                // `massband::kantenschnitt`): only then the three
                // definition points lie unambiguously next to the plane.
                if let (Some(a), Some(b), Some(c)) = (
                    usize::try_from(idx[0]).ok(),
                    usize::try_from(idx[1]).ok(),
                    usize::try_from(idx[2]).ok(),
                ) {
                    if let Some(nn) = normale_f32(self.v[a], self.v[b], self.v[c]) {
                        n = nn;
                    }
                }
            }
            _ => {}
        }
        let s = umfangsschnitt_f32(&self.v, &self.d, p0, n, seg);
        let (mut h, _) = huelle(&s.kette, n);
        // Bust bridge: at the front between the bust points the band bridges over.
        if idx.len() >= 2
            && self.netzpunkt(18) == usize::try_from(idx[0]).ok()
            && self.netzpunkt(19) == usize::try_from(idx[1]).ok()
        {
            if let (Some(a), Some(b)) = (self.netzpunkt(18), self.netzpunkt(19)) {
                let (xl, xr) = (self.v[a][0], self.v[b][0]);
                h.retain(|p| p[2] <= 0.0 || p[0] >= xl - 2.0 || p[0] <= xr + 2.0);
            }
        }
        self.m[j].slots[k].wert = geschlossen(&h);
        self.m[j].slots[k].roh = s.kette;
        self.m[j].slots[k].pts = h;
    }

    /// Type 1 and 5: length on the surface (paths).
    fn oberflaeche(&mut self, j: usize, k: usize) {
        let idx = self.m[j].slots[k].idx.clone();
        let seg = self.m[j].slots[k].seg;
        let hinten = self.m[j].slots[k].hinten;
        // In paths, bust, waist and hip points are projected.
        let e = self.eintraege(&idx, true);
        let (wert, erg, roh) = weg(&self.v, &self.d, &e, seg, hinten, &self.lm);
        self.m[j].slots[k].wert = wert;
        self.m[j].slots[k].pts = erg;
        self.m[j].slots[k].roh = roh;
    }

    /// Type 2: height (absolute y distance).
    fn hoehe(&mut self, j: usize, k: usize) {
        let idx = self.m[j].slots[k].idx.clone();
        let y = |i: i64| -> f64 {
            usize::try_from(i)
                .ok()
                .and_then(|i| self.v.get(i))
                .map_or(0.0, |p| p[1].abs())
        };
        self.m[j].slots[k].wert = match idx.len() {
            1 => y(idx[0]),
            2 => (y(idx[0]) - y(idx[1])).abs(),
            _ => 0.0,
        };
    }

    /// Type 4: straight line through fixed and computed points.
    fn gerade(&mut self, j: usize, k: usize) {
        let idx = self.m[j].slots[k].idx.clone();
        let mut pts = Vec::with_capacity(idx.len());
        let mut breite = false;
        for &i in &idx {
            if i >= 0 && (i as usize) < self.v.len() {
                pts.push(self.punkt(i, false));
                continue;
            }
            let l = |z: usize| self.l.get(z).copied();
            let p = if l(23) == Some(i) {
                // −9: underbust, back
                let q = self
                    .netzpunkt(22)
                    .map_or([0.0; 3], |a| self.v[a]);
                kreuzung26(&self.m[5].slots[0].roh, q, true)
            } else if l(20) == Some(i) {
                breite = true;
                let roh = self.m[5].slots[0].roh.clone();
                self.rand(&roh, true)
            } else if l(21) == Some(i) {
                breite = true;
                let roh = self.m[5].slots[0].roh.clone();
                self.rand(&roh, false)
            } else if l(28) == Some(i) {
                // −4: LowHip, front
                let q = self.netzpunkt(29).map_or([0.0; 3], |a| self.v[a]);
                kreuzung26(&self.m[8].slots[0].roh, q, false)
            } else {
                [0.0; 3]
            };
            pts.push(p);
        }
        // Bust width: move the two edge points to the same height.
        if breite && pts.len() >= 2 {
            let z = (pts[0][2] + pts[1][2]) * 0.5;
            pts[0][2] = z;
            pts[1][2] = z;
        }
        let wert = crate::messung::polylinie(&pts);
        self.m[j].slots[k].wert = wert;
        self.m[j].slots[k].pts = pts;
    }

    /// Measures a measurement (and its second slot, if two-sided).
    pub fn messe(&mut self, j: usize) -> f64 {
        match self.m[j].typ {
            0 => {
                self.umfang(j, 0);
                if self.m[j].beid {
                    self.umfang(j, 1);
                }
            }
            1 | 5 => {
                self.oberflaeche(j, 0);
                if self.m[j].beid {
                    self.oberflaeche(j, 1);
                }
            }
            2 => {
                self.hoehe(j, 0);
                if self.m[j].beid {
                    self.hoehe(j, 1);
                }
            }
            4 => {
                self.gerade(j, 0);
                if self.m[j].beid {
                    self.gerade(j, 1);
                }
            }
            6 | 7 => {
                let (a, b) = (self.m[j].slots[0].idx[0], self.m[j].slots[0].idx[1]);
                let wert = if self.m[j].typ == 6 {
                    self.m[a as usize].wert - self.m[b as usize].wert
                } else if a == 4 {
                    // Halter: 2 · path + back neck arc.
                    2.0 * self.m[b as usize].wert + self.m[4].slots[1].wert
                } else {
                    self.m[a as usize].wert + self.m[b as usize].wert
                };
                self.m[j].slots[0].wert = wert;
            }
            _ => {}
        }
        let mut wert = self.m[j].slots[0].wert;
        if self.m[j].beid {
            wert += self.m[j].slots[1].wert;
            if self.m[j].mittel {
                wert *= 0.5;
            }
        }
        self.m[j].wert = wert;
        wert
    }

    /// Measures all measurements in CLO's order (7 and 8 swapped).
    pub fn alle(&mut self) -> Messung {
        for i in 0..60 {
            let j = match i {
                7 => 8,
                8 => 7,
                _ => i,
            };
            self.messe(j);
        }
        let mut werte = [0.0f64; ANZAHL];
        let mut slots = [[f64::NAN; 2]; ANZAHL];
        for (j, wert) in werte.iter_mut().enumerate() {
            *wert = self.m[j].wert;
            slots[j][0] = self.m[j].slots[0].wert;
            if self.m[j].beid {
                slots[j][1] = self.m[j].slots[1].wert;
            }
        }
        Messung { werte, slots }
    }
}

/// `IndexSet` row as a point number.
fn us(l: &[i64], z: usize) -> Option<usize> {
    l.get(z).copied().filter(|&v| v >= 0).map(|v| v as usize)
}

/// `IndexSet` row that must be a mandatory point.
fn zeile(l: &[i64], z: usize) -> Result<i64, String> {
    match l.get(z) {
        Some(&v) if v >= 0 => Ok(v),
        _ => Err(format!("Landmarke L{z} fehlt in IndexSet")),
    }
}

/// Measurement table according to `messung_2026_1.md` 1, 2 and 5.

/// Builds the measurement table row by row (`messung_2026_1.md` 1, 2, 5).
struct Tabelle {
    m: Vec<Mass>,
    l: Vec<i64>,
}

impl Tabelle {
    fn neu(l: &[i64]) -> Tabelle {
        let m = (0..ANZAHL)
            .map(|j| Mass {
                typ: TYP[j],
                mittel: true,
                slots: [Slot::default(), Slot::default()],
                ..Mass::default()
            })
            .collect();
        Tabelle { m, l: l.to_vec() }
    }

    /// Raw value of an `IndexSet` row (also −1…−9).
    fn zeile(&self, z: usize) -> Result<i64, String> {
        match self.l.get(z) {
            Some(&v) => Ok(v),
            None => Err(format!("IndexSet hat keine Zeile {z}")),
        }
    }

    fn mk(&mut self, mid: usize, typ: u8, beid: bool, mittel: bool) {
        if mid >= ANZAHL {
            return;
        }
        self.m[mid].typ = typ;
        self.m[mid].beid = beid;
        self.m[mid].mittel = mittel;
    }

    /// Slot with raw `IndexSet` values (including placeholders).
    fn setze(&mut self, mid: usize, k: usize, idx: Vec<i64>, seg: i32, hinten: bool) {
        self.m[mid].slots[k] = Slot {
            idx,
            seg,
            hinten,
            ..Slot::default()
        };
    }

    /// Slot from `IndexSet` rows.
    fn setze_l(
        &mut self,
        mid: usize,
        k: usize,
        zeilen: &[usize],
        seg: i32,
        hinten: bool,
    ) -> Result<(), String> {
        let idx = zeilen
            .iter()
            .map(|&z| self.zeile(z))
            .collect::<Result<Vec<_>, _>>()?;
        self.setze(mid, k, idx, seg, hinten);
        Ok(())
    }

    /// Slot with measurement slots as computed points (difference and sum).
    fn setze_mass(&mut self, mid: usize, a: usize, b: usize) {
        self.setze(mid, 0, vec![a as i64, b as i64], -1, false);
    }
}

/// Measurement table according to `messung_2026_1.md` 1, 2 and 5.
pub fn definiere(l: &[i64]) -> Result<Vec<Mass>, String> {
    let mut t = Tabelle::neu(l);
    let rumpf = segment::RUMPF;

    // 0 height, 1 weight (type 3, no method).
    t.mk(0, 2, false, true);
    t.setze_l(0, 0, &[0], -1, false)?;
    t.mk(1, 3, false, true);

    // 32 bust width, 33 bust depth (straight lines with computed points).
    t.mk(32, 4, false, true);
    t.setze_l(32, 0, &[20, 21], -1, false)?;
    t.mk(33, 4, false, true);
    t.setze_l(33, 0, &[22, 23], -1, false)?;

    // 3 neck circumference (plane through three points), 4 neck base (two arcs).
    t.mk(3, 0, false, true);
    t.setze_l(3, 0, &[1, 2, 3], segment::KOPF, false)?;
    t.mk(4, 1, true, false);
    t.setze_l(4, 0, &[6, 64, 65, 7], segment::KOPF, true)?;
    t.setze_l(4, 1, &[7, 5, 6], segment::KOPF, true)?;

    // 2, 5, 6, 7, 8: fixed points instead of candidate search.
    t.mk(2, 0, false, true);
    t.setze_l(2, 0, &[18, 19], rumpf, false)?;
    t.mk(5, 0, false, true);
    t.setze_l(5, 0, &[22], 7, false)?;
    t.mk(6, 0, false, true);
    t.setze_l(6, 0, &[24], rumpf, false)?;
    t.mk(7, 0, false, true);
    t.setze_l(7, 0, &[26], rumpf, false)?;
    t.mk(8, 0, false, true);
    t.setze_l(8, 0, &[29], rumpf, false)?;

    // 9–11, 13: horizontal through one leg point per side.
    for (mid, a, b) in [
        (9usize, 31usize, 32usize),
        (10, 33, 34),
        (11, 35, 36),
        (13, 39, 40),
    ] {
        t.mk(mid, 0, true, true);
        t.setze_l(mid, 0, &[a], segment::BEIN_LINKS, false)?;
        t.setze_l(mid, 1, &[b], segment::BEIN_RECHTS, false)?;
    }
    // 12 calf: fixed point per side.
    t.mk(12, 0, true, true);
    t.setze_l(12, 0, &[37], segment::BEIN_LINKS, false)?;
    t.setze_l(12, 1, &[38], segment::BEIN_RECHTS, false)?;

    // 14 armhole: two partial paths per side, all edges, back side.
    t.mk(14, 1, true, true);
    t.setze_l(14, 0, &[8, 12, 16, 14, 8], segment::ALLE, true)?;
    t.setze_l(14, 1, &[9, 13, 17, 15, 9], segment::ALLE, true)?;

    // 15–17: inclined planes at the arm.
    for (mid, a, b) in [
        (15usize, [41usize, 42, 43], [44usize, 45, 46]),
        (16, [49, 50, 51], [52, 53, 54]),
        (17, [57, 58, 59], [60, 61, 62]),
    ] {
        t.mk(mid, 0, true, true);
        t.setze_l(mid, 0, &a, segment::ARM_LINKS, false)?;
        t.setze_l(mid, 1, &b, segment::ARM_RECHTS, false)?;
    }

    // 18–31: heights.
    const HOEHE: [(usize, &[usize], &[usize], bool); 14] = [
        (18, &[6], &[7], true),
        (19, &[8], &[9], true),
        (20, &[6, 8], &[7, 9], true),
        (21, &[18], &[19], true),
        (22, &[22], &[], false),
        (23, &[24], &[], false),
        (24, &[26], &[], false),
        (25, &[29], &[], false),
        (26, &[30], &[], false),
        (27, &[31], &[32], true),
        (28, &[33], &[34], true),
        (29, &[35], &[36], true),
        (30, &[37], &[38], true),
        (31, &[39], &[40], true),
    ];
    for (mid, a, b, beid) in HOEHE {
        t.mk(mid, 2, beid, true);
        t.setze_l(mid, 0, a, -1, false)?;
        if !b.is_empty() {
            t.setze_l(mid, 1, b, -1, false)?;
        }
    }

    // 34–43: paths in the centre plane.
    t.mk(34, 5, false, true);
    t.setze_l(34, 0, &[12, 13], segment::ALLE, false)?;
    t.mk(35, 5, false, true);
    t.setze_l(35, 0, &[14, 15], segment::ALLE, true)?;
    t.mk(36, 5, false, true);
    t.setze_l(36, 0, &[8, 63, 9], segment::ALLE, true)?;
    t.mk(37, 5, true, true);
    t.setze_l(37, 0, &[5, 8], segment::ALLE, false)?;
    t.setze_l(37, 1, &[5, 9], segment::ALLE, false)?;
    t.mk(38, 5, false, true);
    t.setze_l(38, 0, &[4, 10], segment::ALLE, false)?;
    t.mk(39, 5, false, true);
    t.setze_l(39, 0, &[5, 11], segment::ALLE, true)?;
    t.mk(40, 5, false, true);
    t.setze_l(40, 0, &[4, 24], segment::ALLE, false)?;
    t.mk(41, 5, false, true);
    t.setze_l(41, 0, &[5, 25], segment::ALLE, true)?;
    t.mk(42, 5, false, true);
    t.setze_l(42, 0, &[24, 26], segment::ALLE, false)?;
    t.mk(43, 5, false, true);
    t.setze_l(43, 0, &[24, 28], segment::ALLE, false)?;

    // 44 bust point distance, 45 side neck point to the bust.
    t.mk(44, 4, false, true);
    t.setze_l(44, 0, &[18, 19], -1, false)?;
    t.mk(45, 5, true, true);
    t.setze_l(45, 0, &[6, 18], segment::ALLE, false)?;
    t.setze_l(45, 1, &[7, 19], segment::ALLE, false)?;

    // 46 halter, 47 arm length, 48 nape–wrist.
    t.mk(46, 7, false, true);
    t.setze_mass(46, 4, 45);
    t.mk(47, 5, true, true);
    t.setze_l(47, 0, &[8, 47, 49, 57], segment::ARM_LINKS, true)?;
    t.setze_l(47, 1, &[9, 48, 52, 60], segment::ARM_RECHTS, true)?;
    t.mk(48, 7, false, true);
    t.setze_mass(48, 37, 47);

    // 49–54: crotch, crotch width, crotch lengths.
    t.mk(49, 5, false, true);
    t.setze_l(49, 0, &[24, 30, 25], rumpf, true)?;
    t.mk(50, 5, false, true);
    t.setze_l(50, 0, &[24, 30], rumpf, false)?;
    t.mk(51, 6, false, true);
    t.setze_mass(51, 49, 50);
    t.mk(52, 6, false, true);
    t.setze_mass(52, 23, 26);
    t.mk(53, 4, false, true);
    t.setze_l(53, 0, &[28, 29], -1, false)?;
    t.mk(54, 6, false, true);
    t.setze_mass(54, 26, 28);

    // 55–57: side neck point to the crotch, front and back.
    t.mk(55, 5, true, true);
    t.setze_l(55, 0, &[6, 30], segment::ALLE, false)?;
    t.setze_l(55, 1, &[7, 30], segment::ALLE, false)?;
    t.mk(56, 5, true, true);
    t.setze_l(56, 0, &[6, 30], segment::ALLE, true)?;
    t.setze_l(56, 1, &[7, 30], segment::ALLE, true)?;
    t.mk(57, 7, false, true);
    t.setze_mass(57, 55, 56);

    // 58 hand: open polyline per side, averaged.
    t.mk(58, 4, true, true);
    t.setze_l(58, 0, &[104, 105, 106, 107, 108], 1, false)?;
    t.setze_l(58, 1, &[109, 110, 111, 112, 113], 0, false)?;

    // 59 head circumference: plane through three points in segment 6.
    t.mk(59, 0, false, true);
    t.setze_l(59, 0, &[114, 115, 116], segment::KOPF, false)?;

    // 60–66 sliders: type 8, no measurement method.
    for mid in 60..ANZAHL {
        t.mk(mid, 8, false, true);
    }
    Ok(t.m)
}
