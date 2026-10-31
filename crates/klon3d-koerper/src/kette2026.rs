//! CLO 2026.1: the shaping matrix via PCA regression.
//!
//! 5.2 ([`crate::kette`]) shapes with finished fields from
//! `ConvertingMat_*.bd`. 2026.1 no longer has these files; it computes the
//! form step itself (specification `work/spez/formung_2026_1.md`,
//! sections 6–8).
//!
//! ## The regression (section 6)
//!
//! For a step list `j_0 < … < j_{K−1}`:
//!
//! 1. `X` is the `(K+1) × 2161` matrix from rows `j_k` of
//!    `FeatureData.bd` and a last row of all ones.
//! 2. `X⁺` is its pseudoinverse, `2161 × (K+1)`.
//! 3. `A = ReducedVertexData · X⁺`, `100 × (K+1)`.
//! 4. `M = PCA_weight · A`, `121 395 × (K+1)`. Column `k` belongs to feature
//!    `j_k`, the last column is the intercept.
//!
//! ## Why Cholesky instead of SVD
//!
//! The specification only demands "a numerically clean pseudoinverse"; CLO
//! computes it via a complete orthogonal decomposition. `X` here is
//! **wide** (19 × 2161), so the normal equations `X Xᵀ` are small and
//! positive definite: smallest singular value of `X` 0,735, condition 1,4 · 10⁴,
//! its square 2 · 10⁸ — uncritical in `f64`. Checked against
//! `numpy.linalg.pinv`: `M` agrees to 2 · 10⁻⁹. So klon3d needs no
//! linear-algebra crate.
//!
//! All lengths in centimetres, like `BaseMesh.obj` and the measurement.

use crate::{lies_obj, Punkt3};
use std::fs;
use std::io;
use std::path::Path;

/// Points of the whole mesh (`BaseMesh.obj`, including hair).
pub const PUNKTE: usize = 47_650;
/// Points the regression shapes: rows of `PCA_weight.bd` divided by 3.
pub const NP: usize = 40_465;
/// Points of the `body` group: 0 … 31 421.
pub const KOERPER: usize = 31_422;
/// Mirror partner of the hair points: last number in `symmetry_data.sym`.
const HAAR_PARTNER: usize = 39_635;
/// Bodies of the sample (`FeatureData.bd`, columns).
const STICHPROBE: usize = 2_161;
/// Principal components (`ReducedVertexData.bd`, rows).
const KOMPONENTEN: usize = 100;
/// Features of the regression (`FeatureData.bd`, rows).
const MERKMALE: usize = 58;

fn ungueltig(pfad: &Path, was: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {was}", pfad.display()),
    )
}

/// A `.bd` table: `int32 a, int32 b`, then `a·b` `f32` row by row.
///
/// All three files of the regression have this format
/// (`formung_2026_1.md` 2.3).
fn lies_bd(pfad: &Path) -> io::Result<(usize, usize, Vec<f32>)> {
    let b = fs::read(pfad)?;
    if b.len() < 8 {
        return Err(ungueltig(pfad, "kürzer als der Kopf"));
    }
    let zeilen = i32::from_le_bytes(b[0..4].try_into().unwrap());
    let spalten = i32::from_le_bytes(b[4..8].try_into().unwrap());
    if zeilen <= 0 || spalten <= 0 {
        return Err(ungueltig(pfad, format!("Kopf ({zeilen}, {spalten})")));
    }
    let (zeilen, spalten) = (zeilen as usize, spalten as usize);
    let n = zeilen
        .checked_mul(spalten)
        .ok_or_else(|| ungueltig(pfad, "Kopf zu gross"))?;
    if b.len() != 8 + 4 * n {
        return Err(ungueltig(
            pfad,
            format!("{n} Werte erwartet, {} Byte", b.len()),
        ));
    }
    let werte = b[8..]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    Ok((zeilen, spalten, werte))
}

/// The three tables of the regression.
pub struct Pca {
    /// 58 × 2161, feature space (square root for circumferences and weight).
    feature: Vec<f32>,
    /// 100 × 2161, coefficient of the principal component per body.
    reduziert: Vec<f32>,
    /// 121 395 × 100; row `3i+k` = point `i`, axis `k`.
    gewicht: Vec<f32>,
}

impl Pca {
    /// Reads `FeatureData.bd`, `ReducedVertexData.bd`, `PCA_weight.bd`.
    pub fn laden(ordner: &Path) -> io::Result<Pca> {
        let (fz, fs_, feature) = lies_bd(&ordner.join("FeatureData.bd"))?;
        if (fz, fs_) != (MERKMALE, STICHPROBE) {
            return Err(ungueltig(
                &ordner.join("FeatureData.bd"),
                format!("{fz} × {fs_} statt {MERKMALE} × {STICHPROBE}"),
            ));
        }
        let (rz, rs, reduziert) = lies_bd(&ordner.join("ReducedVertexData.bd"))?;
        if (rz, rs) != (KOMPONENTEN, STICHPROBE) {
            return Err(ungueltig(
                &ordner.join("ReducedVertexData.bd"),
                format!("{rz} × {rs} statt {KOMPONENTEN} × {STICHPROBE}"),
            ));
        }
        let (wz, ws, gewicht) = lies_bd(&ordner.join("PCA_weight.bd"))?;
        if (wz, ws) != (3 * NP, KOMPONENTEN) {
            return Err(ungueltig(
                &ordner.join("PCA_weight.bd"),
                format!("{wz} × {ws} statt {} × {KOMPONENTEN}", 3 * NP),
            ));
        }
        Ok(Pca {
            feature,
            reduziert,
            gewicht,
        })
    }

    /// Shaping matrix of a step list: `K+1` columns, `3 · NP` rows.
    ///
    /// Column `k` belongs to feature `liste[k]`, the last is the
    /// intercept (section 6).
    pub fn matrix(&self, liste: &[usize]) -> Vec<f64> {
        let k = liste.len() + 1;
        // X: k × 2161, row k−1 = ones.
        let mut x = vec![0.0f64; k * STICHPROBE];
        for (z, &j) in liste.iter().enumerate() {
            for s in 0..STICHPROBE {
                x[z * STICHPROBE + s] = self.feature[j * STICHPROBE + s] as f64;
            }
        }
        for s in 0..STICHPROBE {
            x[(k - 1) * STICHPROBE + s] = 1.0;
        }
        // Normal equations G = X Xᵀ (k × k), then Cholesky.
        let mut g = vec![0.0f64; k * k];
        for a in 0..k {
            for b in a..k {
                let mut s = 0.0;
                for t in 0..STICHPROBE {
                    s += x[a * STICHPROBE + t] * x[b * STICHPROBE + t];
                }
                g[a * k + b] = s;
                g[b * k + a] = s;
            }
        }
        let l = cholesky(&g, k);
        // X⁺ = G⁻¹ X  (2161 × k): first solve L y = I, then X⁺ = Xᵀ G⁻¹.
        // In practice: for every sample s the solution z of G z = x(:, s),
        // then X⁺[s][a] = z[a]. That is O(2161 · k²).
        let mut xp = vec![0.0f64; STICHPROBE * k];
        let mut z = vec![0.0f64; k];
        for s in 0..STICHPROBE {
            for a in 0..k {
                z[a] = x[a * STICHPROBE + s];
            }
            let y = vorwaerts(&l, &z, k);
            let w = rueckwaerts(&l, &y, k);
            for a in 0..k {
                xp[s * k + a] = w[a];
            }
        }
        // A = ReducedVertexData · X⁺ (100 × k)
        let mut a_ = vec![0.0f64; KOMPONENTEN * k];
        for c in 0..KOMPONENTEN {
            for s in 0..STICHPROBE {
                let r = self.reduziert[c * STICHPROBE + s] as f64;
                if r == 0.0 {
                    continue;
                }
                for b in 0..k {
                    a_[c * k + b] += r * xp[s * k + b];
                }
            }
        }
        // M = PCA_weight · A (3 NP × k)
        let mut m = vec![0.0f64; 3 * NP * k];
        for i in 0..3 * NP {
            for c in 0..KOMPONENTEN {
                let w = self.gewicht[i * KOMPONENTEN + c] as f64;
                if w == 0.0 {
                    continue;
                }
                for b in 0..k {
                    m[i * k + b] += w * a_[c * k + b];
                }
            }
        }
        m
    }
}

/// Cholesky `G = L Lᵀ` (lower triangular matrix, row by row `k × k`).
fn cholesky(g: &[f64], k: usize) -> Vec<f64> {
    let mut l = vec![0.0f64; k * k];
    for i in 0..k {
        for j in 0..=i {
            let mut s = g[i * k + j];
            for t in 0..j {
                s -= l[i * k + t] * l[j * k + t];
            }
            if i == j {
                // G is positive definite as long as the sample separates the
                // features; a zero would mean two list rows are identical.
                l[i * k + j] = if s > 0.0 { s.sqrt() } else { 0.0 };
            } else {
                l[i * k + j] = if l[j * k + j] > 0.0 {
                    s / l[j * k + j]
                } else {
                    0.0
                };
            }
        }
    }
    l
}

/// `L y = b`.
fn vorwaerts(l: &[f64], b: &[f64], k: usize) -> Vec<f64> {
    let mut y = vec![0.0f64; k];
    for i in 0..k {
        let mut s = b[i];
        for t in 0..i {
            s -= l[i * k + t] * y[t];
        }
        y[i] = if l[i * k + i] > 0.0 {
            s / l[i * k + i]
        } else {
            0.0
        };
    }
    y
}

/// `Lᵀ x = y`.
fn rueckwaerts(l: &[f64], y: &[f64], k: usize) -> Vec<f64> {
    let mut x = vec![0.0f64; k];
    for i in (0..k).rev() {
        let mut s = y[i];
        for t in i + 1..k {
            s -= l[t * k + i] * x[t];
        }
        x[i] = if l[i * k + i] > 0.0 {
            s / l[i * k + i]
        } else {
            0.0
        };
    }
    x
}

/// Base mesh and symmetry mapping: the immutable part of the shaping.
pub struct Basis {
    /// `BaseMesh.obj` in centimetres, 47 650 points.
    pub punkte: Vec<Punkt3>,
    /// Triangles of the `body` group (all points < [`KOERPER`]).
    pub dreiecke: Vec<[usize; 3]>,
    /// Mirror partner per point; hair points get [`HAAR_PARTNER`].
    partner: Vec<usize>,
}

impl Basis {
    /// Reads `BaseMesh.obj` from CLO's resizing folder.
    pub fn laden(ordner: &Path) -> io::Result<Basis> {
        let obj = ordner.join("BaseMesh.obj");
        let netz = lies_obj(&obj, 1.0)?;
        if netz.punkte.len() != PUNKTE {
            return Err(ungueltig(&obj, format!("{} Punkte", netz.punkte.len())));
        }
        let dreiecke = netz
            .dreiecke
            .iter()
            .copied()
            .filter(|d| d.iter().all(|&i| i < KOERPER))
            .collect();
        Ok(Basis {
            punkte: netz.punkte,
            dreiecke,
            partner: lies_partner(&ordner.join("symmetry_data.sym"))?,
        })
    }

    /// Linear shaping, hair points, symmetrising, centring.
    ///
    /// `merkmal` is the feature vector of the step (in the order of
    /// `liste`); `m` is the matrix from [`Pca::matrix`] for the same list.
    pub fn formen(&self, m: &[f64], liste: &[usize], merkmal: &[f64]) -> Vec<Punkt3> {
        let k = liste.len() + 1;
        debug_assert_eq!(m.len(), 3 * NP * k);
        let roh: Vec<Punkt3> = (0..PUNKTE)
            .map(|i| {
                let mut p = self.punkte[i];
                if i < NP {
                    for c in 0..3 {
                        let zeile = 3 * i + c;
                        let mut s = 0.0;
                        for z in 0..k - 1 {
                            s += merkmal[z] * m[zeile * k + z];
                        }
                        p[c] += s + m[zeile * k + k - 1];
                    }
                }
                p
            })
            .collect();

        // 7.2: hair points sit on the mirror image of point 39 635.
        let s = roh[HAAR_PARTNER];
        let mut v = roh;
        for p in v.iter_mut().skip(NP) {
            *p = [-s[0], s[1], s[2]];
        }
        self.symmetrisieren(&v)
    }

    /// Symmetrising and centring (section 8).
    ///
    /// 5.2 also shifted x; 2026.1 does not — the centre line lies at x = 0,
    /// because the centre-line points are set to x = 0. y starts from the
    /// lowest point, z around the mean over **all** points (with hair).
    pub fn symmetrisieren(&self, v: &[Punkt3]) -> Vec<Punkt3> {
        let mut w: Vec<Punkt3> = (0..v.len())
            .map(|i| {
                let t = self.partner[i];
                if t == i {
                    [0.0, v[i][1], v[i][2]]
                } else {
                    let q = v[t];
                    [
                        (v[i][0] - q[0]) / 2.0,
                        (v[i][1] + q[1]) / 2.0,
                        (v[i][2] + q[2]) / 2.0,
                    ]
                }
            })
            .collect();
        let n = w.len() as f64;
        let my = w.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let mz = w.iter().map(|p| p[2]).sum::<f64>() / n;
        for p in &mut w {
            p[1] -= my;
            p[2] -= mz;
        }
        w
    }
}

/// `symmetry_data.sym`: partner per point, remainder set to 39 635.
fn lies_partner(pfad: &Path) -> io::Result<Vec<usize>> {
    let text = fs::read_to_string(pfad)?;
    let mut s = Vec::with_capacity(PUNKTE);
    for t in text.split_whitespace() {
        let z: usize = t
            .parse()
            .map_err(|e| ungueltig(pfad, format!("{t:?}: {e}")))?;
        if z >= PUNKTE {
            return Err(ungueltig(pfad, format!("Partner {z}")));
        }
        s.push(z);
    }
    if s.len() > PUNKTE {
        return Err(ungueltig(pfad, format!("{} Einträge", s.len())));
    }
    s.resize(PUNKTE, HAAR_PARTNER);
    Ok(s)
}

/// Points of the `body` group in millimetres (section 14).
pub fn koerper_mm(punkte: &[Punkt3]) -> Vec<Punkt3> {
    punkte[..KOERPER]
        .iter()
        .map(|p| [p[0] * 10.0, p[1] * 10.0, p[2] * 10.0])
        .collect()
}

// --- Calibrate -----------------------------------------------------------

use crate::massband::Messdaten;

use crate::messung2026::{gemessen, Messer, ANZAHL, TYP};
use crate::nacharbeit2026::{Felder, Fuss};

/// CIRCUMFERENCE_HighHip and LENGTH_Waist_HighHip (`massdefinitionen.md`).
const HUEFTE_OBEN: usize = 7;
const LAENGE_TAILLE_HUEFTE_OBEN: usize = 42;

/// Features for which the feature space takes the square root
/// (`formung_2026_1.md` 5): circumferences and weight.
fn g(j: usize, v: f64) -> f64 {
    if matches!(TYP[j], 0 | 1 | 3) {
        v.sqrt()
    } else {
        v
    }
}

/// Inverse of [`g`].
fn fz(j: usize, f: f64) -> f64 {
    if matches!(TYP[j], 0 | 1 | 3) {
        f * f
    } else {
        f
    }
}

/// Does a measurement count in step `s` (section 10)?
pub fn zaehlt(code: u8, stufe: usize) -> bool {
    if code == 0 || code == 5 {
        return false;
    }
    match stufe {
        1 => code != 2 && code != 4,
        3 => code != 4,
        _ => true,
    }
}

/// The state of the chain: feature vector, target values, step lists.
#[derive(Clone, Debug)]
pub struct Zustand {
    /// Codes per measurement from the configuration (`formung_2026_1.md` 4).
    pub codes: Vec<u8>,
    /// Step lists, index = step (0…4).
    pub listen: Vec<Vec<usize>>,
    /// Feature vector, 67 values in feature space.
    pub f: [f64; crate::messung2026::ANZAHL],
    /// Target values of the same step, feature space.
    pub soll: [f64; crate::messung2026::ANZAHL],
    /// Face variant 1…10 (klon3d: 8).
    pub gesicht: usize,
    /// BASE switch (step 1).
    pub f20: bool,
    /// DETAIL switch (step 3).
    pub f21: bool,
    /// Bust memory b*, −1 = empty.
    pub bust: f64,
}

impl Zustand {
    /// Builds the state from codes and switches (section 4).
    pub fn neu(codes: &[u8], gesicht: usize, f20: bool, f21: bool, bust: f64) -> Zustand {
        let hat = |c: u8| codes.iter().any(|x| *x == c);
        let mut listen = vec![
            (0..67).filter(|&j| !matches!(codes[j], 0 | 5)).collect::<Vec<_>>(),
            (0..67).filter(|&j| matches!(codes[j], 1 | 3)).collect::<Vec<_>>(),
            (0..67)
                .filter(|&j| !matches!(codes[j], 0 | 5 | 2))
                .collect::<Vec<_>>(),
            (0..67)
                .filter(|&j| !matches!(codes[j], 0 | 5 | 4))
                .collect::<Vec<_>>(),
        ];
        for l in &mut listen {
            if l.is_empty() {
                *l = vec![0, 2];
            }
        }
        if !hat(2) {
            // Without a bust measurement there is no special rule; the list stays.
        }
        Zustand {
            codes: codes.to_vec(),
            listen,
            f: [0.0; crate::messung2026::ANZAHL],
            soll: [0.0; crate::messung2026::ANZAHL],
            gesicht,
            f20,
            f21,
            bust,
        }
    }

    /// Sets a feature value in the value range.
    ///
    /// Bounds: sliders 0…100, everything else 0,1…300; afterwards into the
    /// feature space (`g`).
    pub fn setze(&mut self, j: usize, v: f64) {
        let (lo, hi) = if TYP[j] == 8 { (0.0, 100.0) } else { (0.1, 300.0) };
        self.f[j] = g(j, v.clamp(lo, hi));
    }
}

/// What the chain needs: base mesh, PCA, measurement data, post-processing fields.
pub struct Kette2026 {
    pub pca: Pca,
    pub basis: Basis,
    pub messdaten: Messdaten,
    pub felder: Felder,
    pub fuss: Fuss,
}

impl Kette2026 {
    /// Loads everything from CLO's resizing folder.
    pub fn laden(ordner: &Path) -> io::Result<Kette2026> {
        Ok(Kette2026 {
            pca: Pca::laden(ordner)?,
            basis: Basis::laden(ordner)?,
            messdaten: Messdaten::laden(ordner)?,
            felder: Felder::laden(ordner)?,
            fuss: Fuss::laden(&ordner.join("foot.bry"))?,
        })
    }

    /// One shaping pass of a step (`formung_2026_1.md` 7).
    ///
    /// `p3` is the marker: 1 means "BASE switch on", then the
    /// bust adjustment is omitted.
    pub fn formen(&self, z: &Zustand, stufe: usize, p3: usize) -> Vec<Punkt3> {
        let liste = &z.listen[stufe];
        let m = self.pca.matrix(liste);
        let k = liste.len() + 1;
        let roh: Vec<Punkt3> = (0..PUNKTE)
            .map(|i| {
                let mut p = self.basis.punkte[i];
                if i < NP {
                    for c in 0..3 {
                        let zeile = 3 * i + c;
                        let mut s = 0.0;
                        for (zz, &j) in liste.iter().enumerate() {
                            let merkmal = if j == 2 {
                                let x = if stufe != 1 && z.bust > 0.0 {
                                    z.bust
                                } else {
                                    z.f[j]
                                };
                                x
                            } else {
                                z.f[j]
                            };
                            s += merkmal * m[zeile * k + zz];
                        }
                        p[c] += s + m[zeile * k + k - 1];
                    }
                }
                p
            })
            .collect();
        let s = roh[HAAR_PARTNER];
        let mut v = roh;
        for p in v.iter_mut().skip(NP) {
            *p = [-s[0], s[1], s[2]];
        }
        if z.codes[2] != 0 && (p3 != 1 || !z.f20) {
            // 7.3 Bust adjustment: Netz(t) = Sym+Zentr(S + 2(t−0,5)·D_breastMax)
            // resp. 2(0,5−t)·D_breastMin; the target is the target bust circumference.
            let ziel = fz(2, z.soll[2]);
            let start = v;
            let bmax = &self.felder.bmax;
            let bmin = &self.felder.bmin;
            let (v, _, _) = crate::nacharbeit2026::suche(
                |t| {
                    let (d, feld) = if t >= 0.5 {
                        (2.0 * (t - 0.5), bmax)
                    } else {
                        (2.0 * (0.5 - t), bmin)
                    };
                    self.basis
                        .symmetrisieren(&crate::nacharbeit2026::add(&start, d, feld))
                },
                |v| crate::nacharbeit2026::miss(v, &self.messdaten, 2),
                ziel,
                0.2,
                0.01,
                50,
            );
            return v;
        }
        self.basis.symmetrisieren(&v)
    }

    /// Error sum and individual values of a step (`formung_2026_1.md` 10, step 3/4).
    ///
    /// First the measurements that others build on (5, 37/47, 55/56, 8), then
    /// every counting measurement in ascending order.
    pub fn messe_fehler(&self, z: &Zustand, v: &[Punkt3], stufe: usize) -> (f64, [f64; ANZAHL]) {
        let mut ms = match Messer::neu(v, &self.messdaten) {
            Ok(m) => m,
            Err(_) => return (f64::NAN, [f64::NAN; ANZAHL]),
        };
        let mut vor = vec![5usize];
        if z.codes[48] != 0 {
            vor.extend([37, 47]);
        }
        if z.codes[57] != 0 {
            vor.extend([55, 56]);
        }
        if z.codes[7] != 0 {
            vor.push(8);
        }
        for j in vor {
            ms.messe(j);
        }
        let mut e = 0.0;
        let mut m = [f64::NAN; ANZAHL];
        for j in 0..ANZAHL {
            if gemessen(j) && zaehlt(z.codes[j], stufe) {
                m[j] = ms.messe(j);
                e += (m[j] - fz(j, z.soll[j])).abs();
            }
        }
        (e, m)
    }

    /// The calibration loop of a step (`formung_2026_1.md` 10).
    ///
    /// Damping 0,5; fallback already on the first rise to the **best**
    /// vector; at most 30 form steps; bound `0,01 · K`. Afterwards always
    /// `f ← soll`, post-processing (section 12), measure all measurements,
    /// final assignment (section 11).
    pub fn kalibrieren(
        &self,
        z: &mut Zustand,
        stufe: usize,
        p3: usize,
    ) -> Ergebnis {
        z.soll = z.f;
        let mut best = z.f;
        let mut e_vor = 1000.0;
        let mut e_best = 1000.0;
        let mut n = 0usize;
        let k = z.listen[stufe].len() as f64;
        let mut e_folge = Vec::new();
        let mut f_folge: Vec<[f64; ANZAHL]> = Vec::new();
        let grund;
        let mut v;
        loop {
            v = self.formen(z, stufe, p3);
            let (e, m) = self.messe_fehler(z, &v, stufe);
            e_folge.push(e);
            f_folge.push(z.f);
            if e < 0.0 {
                grund = Ende::Fehler;
                break;
            }
            if e < k * 0.01 {
                grund = Ende::Konvergenz;
                break;
            }
            if n + 1 > 29 {
                grund = Ende::ZuViele;
                break;
            }
            n += 1;
            if e > e_vor {
                z.f = best;
                v = self.formen(z, stufe, p3);
                grund = Ende::Rueckfall;
                break;
            }
            if e < e_best {
                best = z.f;
                e_best = e;
            }
            let mut stop = false;
            for j in 0..ANZAHL {
                if gemessen(j) && zaehlt(z.codes[j], stufe) {
                    let x = fz(j, z.f[j]) + 0.5 * (fz(j, z.soll[j]) - m[j]);
                    if x < 0.0 {
                        stop = true;
                        break;
                    }
                    z.f[j] = g(j, x);
                }
            }
            if stop {
                grund = Ende::Negativ;
                break;
            }
            e_vor = e;
        }

        // Always: f ← soll, post-processing with these target values.
        z.f = z.soll;
        v = crate::nacharbeit2026::nacharbeit(
            &v,
            &z.codes,
            &z.soll,
            z.gesicht,
            z.f21,
            stufe,
            &self.felder,
            Some(&self.fuss),
            &self.basis,
            &self.messdaten,
        );
        let werte = match Messer::neu(&v, &self.messdaten) {
            Ok(mut m) => m.alle().werte,
            Err(_) => [0.0; ANZAHL],
        };
        // Final assignment (section 11).
        let mut mf = [0.0; ANZAHL];
        for j in 0..ANZAHL {
            mf[j] = if j < 60 { g(j, werte[j]) } else { z.f[j] };
        }
        for j in 0..ANZAHL {
            let c = z.codes[j];
            if matches!(stufe, 0 | 2 | 4) && c != 0 {
                continue;
            }
            if stufe == 1 && ((z.f20 && (c == 1 || c == 3)) || (!z.f20 && c != 0)) {
                continue;
            }
            if stufe == 3 && ((z.f21 && matches!(c, 1 | 2 | 3)) || (!z.f21 && c != 0)) {
                continue;
            }
            if j == 2 && stufe == 1 && p3 == 0 && z.bust > 0.0 {
                continue;
            }
            if gemessen(j) {
                z.f[j] = mf[j];
            }
        }
        // Bust memory b*.
        if stufe == 1 && (p3 == 1 || z.bust < 0.0) {
            z.bust = z.f[2];
        }
        Ergebnis {
            punkte: v,
            werte,
            e_folge,
            f_folge,
            grund,
        }
    }
}

/// Why the calibration loop ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ende {
    /// `E < 0,01 · K`.
    Konvergenz,
    /// `E > E_vor`: jump back to the best vector.
    Rueckfall,
    /// `n + 1 > 29`.
    ZuViele,
    /// `X < 0` in the feature update.
    Negativ,
    /// `E < 0` (does not occur).
    Fehler,
}

/// Result of a step.
#[derive(Clone, Debug)]
pub struct Ergebnis {
    /// The finished mesh after calibration and post-processing, cm.
    pub punkte: Vec<Punkt3>,
    /// The 60 measured values, cm.
    pub werte: [f64; ANZAHL],
    /// Error sum per form step.
    pub e_folge: Vec<f64>,
    /// Feature vector per form step, 67 values.
    pub f_folge: Vec<[f64; ANZAHL]>,
    /// Reason for the end.
    pub grund: Ende,
}

/// Result of the whole chain (section 13).
#[derive(Clone, Debug)]
pub struct Ganze {
    pub base: Ergebnis,
    /// After the bust adjustment, with post-processing (hand, head, foot).
    pub detail: Ergebnis,
    /// The input of the DETAIL step, 67 slots (with derived HighHip).
    pub eingabe: [f64; ANZAHL],
}

impl Ganze {
    /// The 60 values of the DETAIL step, cm (weight kg).
    pub fn werte(&self) -> &[f64; ANZAHL] {
        &self.detail.werte
    }
}

/// The configuration with which CLO and klon3d shape (section 4).
///
/// klon3d uses **anchor TotalHeight, width Bust, detail Intermediate**;
/// CLO's UI offers other detail lists besides these, which are not
/// built here.
#[derive(Clone, Debug)]
pub struct Konfiguration {
    /// Codes per measurement (`formung_2026_1.md` 4).
    pub codes: Vec<u8>,
    /// Defaults of the post-processing 58…66 (hand 16,1 cm, head 50 cm, slider in
    /// percent).
    pub vorgaben: Vec<(usize, f64)>,
}

impl Konfiguration {
    /// TotalHeight / Bust / Intermediate with the observed defaults.
    pub fn klon3d() -> Konfiguration {
        let mut codes = vec![0u8; ANZAHL];
        codes[0] = 3;
        codes[2] = 1;
        for j in [4, 5, 6, 7, 8, 9, 11, 12, 15, 17] {
            codes[j] = 2;
        }
        for j in [26, 36, 40, 41, 48, 49] {
            codes[j] = 4;
        }
        for j in 58..67 {
            codes[j] = 5;
        }
        Konfiguration {
            codes,
            vorgaben: vec![
                (58, 16.1),
                (59, 50.0),
                (60, 30.0),
                (61, 30.0),
                (62, 50.0),
                (63, 50.0),
                (64, 70.0),
                (65, 50.0),
                (66, 50.0),
            ],
        }
    }

    /// A fresh state: step lists from the codes, `b*` = −1.
    pub fn zustand(&self) -> Zustand {
        Zustand::neu(&self.codes, 8, true, true, -1.0)
    }

    /// Sets the given values (step 2 and 4 from section 13).
    ///
    /// Only slots with code ≠ 0 and a value > 0 are set; 0 means
    /// "not given" and stays with the BASE prediction. The sliders
    /// 58…66 otherwise get their default.
    pub fn setze_gegeben(&self, z: &mut Zustand, v: &[f64; ANZAHL]) {
        for j in 0..ANZAHL {
            if self.codes[j] == 5 {
                continue;
            }
            if self.codes[j] != 0 && v[j] > 0.0 {
                z.setze(j, v[j]);
            }
        }
        for &(j, wert) in &self.vorgaben {
            if v[j] > 0.0 {
                z.setze(j, v[j]);
            } else {
                z.setze(j, wert);
            }
        }
    }
}

impl Kette2026 {
    /// The whole chain as CLO does on load (section 13).
    ///
    /// `nach_base` may change the input after the BASE step (klon3d derives
    /// HighHip there); `f` and `b*` are preserved.
    pub fn ganz(
        &self,
        k: &Konfiguration,
        wert: &[f64; ANZAHL],
        nach_base: &dyn Fn(&Ergebnis, &mut [f64; ANZAHL]),
    ) -> Ganze {
        let mut z = k.zustand();
        let mut v = *wert;
        k.setze_gegeben(&mut z, &v);
        let base = self.kalibrieren(&mut z, 1, 1);
        nach_base(&base, &mut v);
        k.setze_gegeben(&mut z, &v);
        let detail = self.kalibrieren(&mut z, 0, 0);
        Ganze {
            base,
            detail,
            eingabe: v,
        }
    }

    /// The chain from a measurement file (`kette::SMIS`, `massdefinitionen.md`).
    ///
    /// Requires the values that CLO's BASE step needs (code 1 or 3).
    /// Slots without their own measurement stay 0 and thus with the BASE
    /// prediction; HighHip (7) is interpolated from `LENGTH_Waist_HighHip`
    /// of the BASE mesh between `waist_to_highhip_side` and
    /// `waist_to_hip_side` (`massdefinitionen.md`, line 7).
    pub fn aus_smis(
        &self,
        k: &Konfiguration,
        wert: &dyn Fn(&str) -> Option<f64>,
    ) -> Result<Ganze, String> {
        let mut v = [0.0; ANZAHL];
        for &(j, name) in &crate::kette::SMIS {
            v[j] = wert(name).unwrap_or(0.0);
        }
        for j in 0..ANZAHL {
            if matches!(k.codes[j], 1 | 3) && !(v[j] > 0.0) {
                let name = crate::kette::SMIS
                    .iter()
                    .find(|s| s.0 == j)
                    .map_or("?", |s| s.1);
                return Err(format!("Maß {name:?} fehlt in der Maßdatei"));
            }
        }
        let oben = (
            wert("highhip_circ"),
            wert("hip_circ"),
            wert("waist_to_highhip_side"),
            wert("waist_to_hip_side"),
        );
        let mut fehler = None;
        let ganze = self.ganz(k, &v, &|base, v| {
            if let (Some(u0), Some(u1), Some(a0), Some(a1)) = oben {
                if a1 <= a0 {
                    return;
                }
                let d = base.werte[LAENGE_TAILLE_HUEFTE_OBEN];
                v[HUEFTE_OBEN] = u0 + (u1 - u0) * (d - a0) / (a1 - a0);
            }
        });
        if let (Some(_), Some(_), Some(a0), Some(a1)) = oben {
            if a1 <= a0 {
                fehler = Some(format!(
                    "waist_to_highhip_side {a0} ≥ waist_to_hip_side {a1}"
                ));
            }
        }
        match fehler {
            Some(e) => Err(e),
            None => Ok(ganze),
        }
    }
}
