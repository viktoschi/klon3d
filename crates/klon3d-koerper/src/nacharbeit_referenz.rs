//! Displacement fields, search and post-processing.
//!
//! This is layer 2 of the reference reconstruction (`formung_2026_1.md` 12): the
//! files
//! `displacementData*.txt`, `hand_*.txt`, `head_*.txt` and `foot.bry`, the
//! halving search from 12.1 and the post-processing from 12.2–12.9. The
//! calibration loop itself is in [`crate::kette_referenz`].
//!
//! All lengths in centimetres and in point order of the mesh
//! (47 650 points, `BaseMesh.obj`).

use crate::kette_referenz::{Basis, PUNKTE};
use crate::massband::Messdaten;
use crate::messung_referenz::{Messer, ANZAHL};
use crate::Punkt3;
use std::fs;
use std::io;
use std::path::Path;

fn ungueltig(pfad: &Path, was: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {was}", pfad.display()),
    )
}

/// A displacement field: `PUNKTE` triples in cm.
///
/// CLO rigidly reads `PUNKTE` triples from every text chunk; if the file has fewer
/// rows (`head_*`, `displacementData01…10`: 40 465), the remaining
/// points stay at (0, 0, 0) (`formung_2026_1.md` 2.5).
#[derive(Clone, Debug)]
pub struct Feld(pub Vec<Punkt3>);

impl Feld {
    /// Reads a text file with three numbers per line (whitespace separated).
    pub fn laden(pfad: &Path) -> io::Result<Feld> {
        let text = fs::read_to_string(pfad)?;
        let mut werte = Vec::new();
        for t in text.split_whitespace() {
            werte.push(
                t.parse::<f64>()
                    .map_err(|e| ungueltig(pfad, format!("{t:?}: {e}")))?,
            );
        }
        if werte.len() % 3 != 0 {
            return Err(ungueltig(pfad, format!("{} Zahlen", werte.len())));
        }
        let mut v: Vec<Punkt3> = werte
            .chunks_exact(3)
            .map(|p| [p[0], p[1], p[2]])
            .collect();
        if v.len() > PUNKTE {
            return Err(ungueltig(pfad, format!("{} Punkte", v.len())));
        }
        v.resize(PUNKTE, [0.0, 0.0, 0.0]);
        Ok(Feld(v))
    }
}

/// The fields that the post-processing needs.
pub struct Felder {
    pub crotch: Feld,
    pub bmax: Feld,
    pub bmin: Feld,
    pub hand_min: Feld,
    pub hand_max: Feld,
    pub head_min: Feld,
    pub head_max: Feld,
    pub ball: Feld,
    pub tear: Feld,
    pub narrow: Feld,
    pub wide: Feld,
    pub low: Feld,
    pub high: Feld,
    pub dips_low: Feld,
    pub dips_up: Feld,
    pub vol_flat: Feld,
    pub vol_curved: Feld,
    /// `displacementData01…10`: face shape, index 0 = file 01.
    pub gesicht: Vec<Feld>,
}

impl Felder {
    /// Reads all fields from CLO's resizing folder.
    pub fn laden(ordner: &Path) -> io::Result<Felder> {
        let l = |name: &str| Feld::laden(&ordner.join(name));
        let mut gesicht = Vec::with_capacity(10);
        for i in 1..=10 {
            gesicht.push(l(&format!("displacementData{i:02}.txt"))?);
        }
        Ok(Felder {
            crotch: l("displacementData_crotch.txt")?,
            bmax: l("displacementData_breastMax.txt")?,
            bmin: l("displacementData_breastMin.txt")?,
            hand_min: l("hand_min.txt")?,
            hand_max: l("hand_max.txt")?,
            head_min: l("head_min.txt")?,
            head_max: l("head_max.txt")?,
            ball: l("displacementData_breastBall.txt")?,
            tear: l("displacementData_breastTearDrop.txt")?,
            narrow: l("displacementData_breastNarrow.txt")?,
            wide: l("displacementData_breastWide.txt")?,
            low: l("displacementData_breastLow.txt")?,
            high: l("displacementData_breastHigh.txt")?,
            dips_low: l("displacementData_hipDips_Lower.txt")?,
            dips_up: l("displacementData_hipDips_Upper.txt")?,
            vol_flat: l("displacementData_hipVolume_Flat.txt")?,
            vol_curved: l("displacementData_hipVolume_Curved.txt")?,
            gesicht,
        })
    }
}

/// `p + a · f` point by point.
pub fn add(p: &[Punkt3], a: f64, f: &Feld) -> Vec<Punkt3> {
    p.iter()
        .zip(&f.0)
        .map(|(p, d)| [p[0] + a * d[0], p[1] + a * d[1], p[2] + a * d[2]])
        .collect()
}

/// The halving search from `formung_2026_1.md` 12.1.
///
/// Initial step `schritt`, direction `+1`, on a sign change the
/// step is halved. Abort at `|d| < tol` (success), after `maxn` steps
/// or if `n > maxn` — then the **last applied** mesh remains.
///
/// Returns mesh, `t` and the number of steps.
pub fn suche(
    anwenden: impl Fn(f64) -> Vec<Punkt3>,
    messen: impl Fn(&[Punkt3]) -> f64,
    ziel: f64,
    mut schritt: f64,
    tol: f64,
    maxn: usize,
) -> (Vec<Punkt3>, f64, usize) {
    let mut t = 0.5;
    let mut v = anwenden(t);
    let mut d = ziel - messen(&v);
    let mut richt = 1.0;
    let mut n = 0;
    while d.abs() >= tol {
        if d <= 0.0 {
            if richt > 0.0 {
                schritt *= 0.5;
            }
            richt = -1.0;
        } else {
            if richt < 0.0 {
                schritt *= 0.5;
            }
            richt = 1.0;
        }
        n += 1;
        t += richt * schritt;
        if n > maxn {
            return (v, t, n);
        }
        v = anwenden(t);
        d = ziel - messen(&v);
    }
    (v, t, n)
}

/// Clamping to `[0, 1]`.
pub fn klemm(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

/// `foot.bry`: ankle triangles and the mapping of the points onto them
/// (`formung_2026_1.md` 2.6).
pub struct Fuss {
    /// Triangle of the left ankle (Female: 9316 10630 10609).
    pub dreieck_a: [usize; 3],
    /// Triangle of the right ankle (Female: 24948 26253 26234).
    pub dreieck_b: [usize; 3],
    /// Per point: `(triangle 1|2, u, v, w, h)` — or `None`.
    pub zuordnung: Vec<Option<(u8, f64, f64, f64, f64)>>,
    /// Two point numbers; the ankle height is the mean of their y.
    pub knoechel: (usize, usize),
    /// Per point: `(1|2, q)` or `(0, 0)`.
    pub regel: Vec<(u8, f64)>,
}

impl Fuss {
    /// Reads `foot.bry` (whitespace separated).
    pub fn laden(pfad: &Path) -> io::Result<Fuss> {
        let text = fs::read_to_string(pfad)?;
        let w: Vec<&str> = text.split_whitespace().collect();
        let mut p = 0usize;
        let mut nimm = |k: usize| -> io::Result<Vec<&str>> {
            if p + k > w.len() {
                return Err(ungueltig(pfad, "Datei endet zu früh"));
            }
            let r = w[p..p + k].to_vec();
            p += k;
            Ok(r)
        };
        let zahl = |t: &str, pfad: &Path| -> io::Result<f64> {
            t.parse::<f64>()
                .map_err(|e| ungueltig(pfad, format!("{t:?}: {e}")))
        };
        let ganz = |t: &str, pfad: &Path| -> io::Result<usize> {
            t.parse::<usize>()
                .map_err(|e| ungueltig(pfad, format!("{t:?}: {e}")))
        };

        let a = nimm(3)?;
        let b = nimm(3)?;
        let dreieck_a = [
            ganz(a[0], pfad)?,
            ganz(a[1], pfad)?,
            ganz(a[2], pfad)?,
        ];
        let dreieck_b = [
            ganz(b[0], pfad)?,
            ganz(b[1], pfad)?,
            ganz(b[2], pfad)?,
        ];
        let vorz = |t: &str, pfad: &Path| -> io::Result<i64> {
            t.parse::<i64>()
                .map_err(|e| ungueltig(pfad, format!("{t:?}: {e}")))
        };
        let mut zuordnung = Vec::with_capacity(PUNKTE);
        for _ in 0..PUNKTE {
            let k = vorz(nimm(1)?[0], pfad)?;
            if k < 1 {
                zuordnung.push(None);
            } else {
                let v = nimm(4)?;
                zuordnung.push(Some((
                    k as u8,
                    zahl(v[0], pfad)?,
                    zahl(v[1], pfad)?,
                    zahl(v[2], pfad)?,
                    zahl(v[3], pfad)?,
                )));
            }
        }
        let k = nimm(2)?;
        let knoechel = (ganz(k[0], pfad)?, ganz(k[1], pfad)?);
        let mut regel = Vec::with_capacity(PUNKTE);
        for _ in 0..PUNKTE {
            // r = 1 and r = 2 carry a number along; every other value does not.
            let f = vorz(nimm(1)?[0], pfad)?;
            if f == 1 || f == 2 {
                let q = zahl(nimm(1)?[0], pfad)?;
                regel.push((f as u8, q));
            } else {
                regel.push((0, 0.0));
            }
        }
        Ok(Fuss {
            dreieck_a,
            dreieck_b,
            zuordnung,
            knoechel,
            regel,
        })
    }

    /// Places the feet (`formung_2026_1.md` 12.9).
    ///
    /// The points with an entry are placed onto their ankle triangle: barycentric
    /// plus normal distance `h`. Below the ankle height it is blended over to the
    /// body surface (square root), afterwards rules 1 and 2 overwrite the height.
    pub fn anwenden(&self, p: &[Punkt3]) -> Vec<Punkt3> {
        let tri = |k: u8| -> [Punkt3; 3] {
            let e = if k == 1 {
                self.dreieck_a
            } else {
                self.dreieck_b
            };
            [p[e[0]], p[e[1]], p[e[2]]]
        };
        let yk = (p[self.knoechel.0][1] + p[self.knoechel.1][1]) * 0.5;
        let mut q = p.to_vec();
        for (i, r) in self.zuordnung.iter().enumerate() {
            let Some((k, u, v, w, h)) = *r else { continue };
            let a = tri(k);
            let n = crate::massband::kreuz(
                crate::massband::sub(a[0], a[1]),
                crate::massband::sub(a[2], a[1]),
            );
            let ln = crate::massband::betrag(n);
            let n = if ln > 0.0 {
                [n[0] / ln, n[1] / ln, n[2] / ln]
            } else {
                [0.0, 0.0, 0.0]
            };
            let ziel = [
                u * a[0][0] + v * a[1][0] + w * a[2][0] + h * n[0],
                u * a[0][1] + v * a[1][1] + w * a[2][1] + h * n[1],
                u * a[0][2] + v * a[1][2] + w * a[2][2] + h * n[2],
            ];
            if self.regel[i].0 == 1 && p[i][1] <= yk {
                let s = ((p[i][1] - 4.0) / (yk - 4.0)).max(0.0).sqrt();
                q[i] = [
                    (1.0 - s) * ziel[0] + s * p[i][0],
                    (1.0 - s) * ziel[1] + s * p[i][1],
                    (1.0 - s) * ziel[2] + s * p[i][2],
                ];
            } else {
                q[i] = ziel;
            }
        }
        for (i, &(f, v)) in self.regel.iter().enumerate() {
            match f {
                1 => q[i][1] = (yk - 4.0) * v + 4.0,
                2 => q[i][1] = v,
                _ => {}
            }
        }
        q
    }
}

/// Measures a single measurement, cm (`formung_2026_1.md` 12: "Messen").
pub fn miss(v: &[Punkt3], d: &Messdaten, j: usize) -> f64 {
    match Messer::neu(v, d) {
        Ok(mut m) => m.messe(j),
        Err(_) => f64::NAN,
    }
}

/// The post-processing after the calibration (`formung_2026_1.md` 12).
///
/// `soll` are the target values of the step (after `f ← soll`), `codes` the
/// configuration. Order and conditions as in the specification; every
/// displacement is symmetrised and centred, except the foot.
#[allow(clippy::too_many_arguments)]
pub fn nacharbeit(
    p: &[Punkt3],
    codes: &[u8],
    soll: &[f64],
    gesicht: usize,
    f21: bool,
    stufe: usize,
    f: &Felder,
    fuss: Option<&Fuss>,
    basis: &Basis,
    d: &Messdaten,
) -> Vec<Punkt3> {
    assert_eq!(codes.len(), ANZAHL);
    assert_eq!(soll.len(), ANZAHL);
    let mut p = p.to_vec();
    let sym = |v: Vec<Punkt3>| basis.symmetrisieren(&v);

    // 12.2 stride width.
    if codes[60] != 0 {
        p = sym(add(&p, soll[60] / 100.0, &f.crotch));
    }

    // 12.3 breast shape.
    if codes[62] != 0 && codes[63] != 0 && codes[64] != 0 {
        let (a, b, g) = (soll[62] / 100.0, soll[63] / 100.0, soll[64] / 100.0);
        let gewichte = [
            (klemm(2.0 * (0.5 - a)), &f.ball),
            (klemm(2.0 * (a - 0.5)), &f.tear),
            (klemm(2.0 * (0.5 - b)), &f.narrow),
            (klemm(2.0 * (b - 0.5)), &f.wide),
            (klemm((0.7 - g) / 0.7), &f.low),
            (klemm((g - 0.7) / 0.3), &f.high),
        ];
        p = sym(summe(&p, &gewichte));
    }

    // 12.4 hip.
    if codes[65] != 0 && codes[66] != 0 {
        let (a, b) = (soll[65] / 100.0, soll[66] / 100.0);
        let gewichte = [
            (klemm(2.0 * (0.5 - a)), &f.dips_low),
            (klemm(2.0 * (a - 0.5)), &f.dips_up),
            (klemm(2.0 * (0.5 - b)), &f.vol_flat),
            (klemm(2.0 * (b - 0.5)), &f.vol_curved),
        ];
        p = sym(summe(&p, &gewichte));
    }

    // 12.5 hand.
    if stufe == 3 && f21 {
        // Hand correction by body height as in the older version; klon3d does
        // not need the branch (the specification names a = 140, b = 250, in the
        // reference version not read out).
        let h = miss(&p, d, 0);
        let t = 0.5 * klemm((h - 140.0) / (250.0 - 140.0));
        let q = add(&p, 1.0, &f.hand_min);
        let spanne = differenz(&f.hand_max, &f.hand_min);
        p = sym(summe(&q, &[(t, &spanne)]));
    } else if codes[58] != 0 {
        let start = p.clone();
        let hmin = f.hand_min.clone();
        let spanne = differenz(&f.hand_max, &f.hand_min);
        let (v, _, _) = suche(
            |t| {
                let q = add(&start, 1.0, &hmin);
                basis.symmetrisieren(&summe(&q, &[(t, &spanne)]))
            },
            |v| miss(v, d, 58),
            soll[58],
            0.1,
            1.0e-4,
            100,
        );
        p = v;
    }

    // 12.6 head and face.
    if codes[59] != 0 {
        let start = p.clone();
        let kmin = f.head_min.clone();
        let spanne = differenz(&f.head_max, &f.head_min);
        let gesichtsfeld = &f.gesicht[gesicht.saturating_sub(1).min(9)];
        let (v, _, _) = suche(
            |t| {
                let q = add(&start, 1.0, &kmin);
                let mut r = basis.symmetrisieren(&summe(&q, &[(t, &spanne)]));
                // The face is asymmetric and is added after the
                // symmetrising, not centred.
                for (p, a) in r.iter_mut().zip(&gesichtsfeld.0) {
                    p[0] += a[0];
                    p[1] += a[1];
                    p[2] += a[2];
                }
                r
            },
            |v| miss(v, d, 59),
            soll[59] * soll[59],
            0.1,
            1.0e-4,
            100,
        );
        p = v;
    }

    // 12.7 foot, without subsequent centring.
    match fuss {
        Some(ft) => ft.anwenden(&p),
        None => p,
    }
}

/// `a − b` point by point.
pub fn differenz(a: &Feld, b: &Feld) -> Feld {
    Feld(
        a.0.iter()
            .zip(&b.0)
            .map(|(a, b)| [a[0] - b[0], a[1] - b[1], a[2] - b[2]])
            .collect(),
    )
}

/// `p + Σ w_k · Feld_k`.
fn summe(p: &[Punkt3], gewichte: &[(f64, &Feld)]) -> Vec<Punkt3> {
    let mut q = p.to_vec();
    for (w, f) in gewichte {
        if *w == 0.0 {
            continue;
        }
        for (p, d) in q.iter_mut().zip(&f.0) {
            p[0] += w * d[0];
            p[1] += w * d[1];
            p[2] += w * d[2];
        }
    }
    q
}
