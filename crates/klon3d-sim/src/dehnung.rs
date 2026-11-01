//! Strain map: principal strain per face, per pattern piece (stage 5).
//!
//! The second inspection measure. [`crate::messung::dehnung_je_dreieck`]
//! already answers, per triangle, how much its two principal directions are
//! stretched against the rest shape the solver uses; nothing read that for a
//! dressed garment. This module turns it into a map plus the same kind of
//! per-piece summary the distance map has.
//!
//! # Two reference frames
//!
//! Both are reported, because they answer different questions:
//!
//! * **rest frame** — against the length the solver works with, that is the
//!   pattern scaled by `1 − reduction`. This is what the material feels, and
//!   it is the number `klon3d roehre` prints as `dehnung`.
//! * **pattern frame** — against the cut as drawn. This is what a maker sees
//!   when comparing the dressed garment to the pattern.
//!
//! They differ by exactly the reduction: `e_pattern = (1 + e_rest)·(1 − r) − 1`.
//! A piece cut 5.5 % smaller and stretched back to the pattern size reads 5.82 %
//! in the rest frame and 0 % in the pattern frame. Reporting only one of them
//! silently answers the wrong question, so the table shows the rest frame and
//! adds the pattern frame, because on a fitted latex piece the reduction is
//! where most of the visible strain sits.
//!
//! # Sign
//!
//! Positive is stretch, negative is compression — engineering strain `λ − 1`,
//! not logarithmic. `roehre` uses the same definition, so the numbers are
//! comparable.
//!
//! # Reading the extremes
//!
//! The largest values of a dressed run do not come from a degenerate mesh. On
//! the test blouse every face above 100 % has a rest area of 9–61 mm² and a
//! smallest angle of 32–87°, i.e. ordinary triangles. What happens there is
//! contact: one corner of the face sits at exactly the collision distance
//! (0.2 mm, half the fabric thickness) while its neighbours are 8–12 mm off the
//! skin, because the collision pass pushed a single vertex onto the surface.
//! The face spanning those two states is stretched, correctly. So a high
//! maximum next to a low median names a contact spike, not a material problem —
//! which is why the table shows the median and p90 beside it and why the count
//! above the limit is per face, not an average.
//!
//! # The limit
//!
//! [`UEBER_DEHNUNG`] is an assumption, not a measurement: `MATERIAL.md` gives
//! latex as almost linear up to 5–10 % elongation, and beyond that the profile
//! is extrapolated. It is named in the output and kept in one constant so a
//! real value can replace it.

use crate::messung::dehnung_je_dreieck;
use crate::szene::Szene;
use crate::verteilung::Verteilung;

/// Stretch above which a face counts as over-stretched (engineering strain).
///
/// An assumption from `MATERIAL.md` (latex almost linear to 5–10 %), not a
/// measured limit; see the module documentation.
pub const UEBER_DEHNUNG: f64 = 0.10;

/// Strain of one face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flaeche {
    /// Larger principal strain (engineering strain, positive = stretch), rest
    /// frame.
    pub groesste: f64,
    /// Smaller principal strain; negative means compression, rest frame.
    pub kleinste: f64,
    /// Larger principal strain in the pattern frame, `(1 + e)·(1 − r) − 1`.
    pub muster: f64,
    /// Rest area of the face (m²), the weight for the summary. Rest, not
    /// current area: the weight says how much material the face stands for,
    /// and a stretched face does not represent more material than before.
    pub ruheflaeche_m2: f64,
}

/// Summary of one pattern piece.
#[derive(Debug, Clone, PartialEq)]
pub struct Teil {
    pub name: String,
    /// Faces of the piece.
    pub flaechen: usize,
    /// Rest area of the piece (m²).
    pub ruheflaeche_m2: f64,
    /// Larger principal strain in percent, rest frame; the mean is
    /// area-weighted.
    pub verteilung: Verteilung,
    /// Larger principal strain in percent, pattern frame.
    pub muster: Verteilung,
    /// Faces stretched beyond [`UEBER_DEHNUNG`].
    pub ueber: usize,
    /// Reduction the piece was cut with (0 = pattern size).
    pub reduktion: f64,
}

/// The whole map.
#[derive(Debug, Clone, PartialEq)]
pub struct Karte {
    /// One entry per face, in scene order.
    pub je_flaeche: Vec<Flaeche>,
    /// One entry per piece, in scene order.
    pub je_teil: Vec<Teil>,
}

impl Karte {
    /// Measures the current state. Works without a body: strain is a property
    /// of the cloth alone.
    pub fn messe(s: &Szene) -> Karte {
        // Reduction per face, so the pattern-frame value is carried along with
        // the face instead of being recomputed in every report.
        let mut reduktion = vec![0.0; s.dreiecke.len()];
        for t in &s.teile {
            for k in t.dreiecke.clone() {
                reduktion[k] = t.vorgabe.reduktion;
            }
        }
        let je_flaeche: Vec<Flaeche> = dehnung_je_dreieck(s)
            .into_iter()
            .enumerate()
            .map(|(k, (groesste, kleinste))| Flaeche {
                groesste,
                kleinste,
                muster: muster_dehnung(groesste, reduktion[k]),
                ruheflaeche_m2: ruheflaeche(&s.ruhe_inv[k]),
            })
            .collect();
        let je_teil = s
            .teile
            .iter()
            .map(|t| {
                let bereich = t.dreiecke.clone();
                let rest: Vec<f64> = bereich
                    .clone()
                    .map(|k| je_flaeche[k].groesste * 100.0)
                    .collect();
                let muster: Vec<f64> = bereich
                    .clone()
                    .map(|k| je_flaeche[k].muster * 100.0)
                    .collect();
                let gewicht: Vec<f64> = bereich
                    .clone()
                    .map(|k| je_flaeche[k].ruheflaeche_m2)
                    .collect();
                let verteilung = Verteilung::aus_gewichtet(rest, &gewicht);
                Teil {
                    name: t.name.clone(),
                    flaechen: verteilung.anzahl,
                    ruheflaeche_m2: gewicht.iter().sum(),
                    ueber: bereich
                        .clone()
                        .filter(|&k| je_flaeche[k].groesste > UEBER_DEHNUNG)
                        .count(),
                    reduktion: t.vorgabe.reduktion,
                    muster: Verteilung::aus(muster),
                    verteilung,
                }
            })
            .collect();
        Karte {
            je_flaeche,
            je_teil,
        }
    }

    /// Summary over all faces (percent, rest frame).
    pub fn gesamt(&self) -> Verteilung {
        Verteilung::aus(self.je_flaeche.iter().map(|f| f.groesste * 100.0).collect())
    }

    /// Summary over all faces (percent, pattern frame).
    pub fn gesamt_muster(&self) -> Verteilung {
        Verteilung::aus(self.je_flaeche.iter().map(|f| f.muster * 100.0).collect())
    }

    /// The most stretched face (rest frame) and where it is.
    pub fn staerkste(&self) -> Option<(f64, usize)> {
        self.je_flaeche
            .iter()
            .enumerate()
            .map(|(i, f)| (f.groesste, i))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// CSV, one line per face:
    /// `dreieck;teil;flaeche_cm2;groesste_prozent;kleinste_prozent;muster_prozent`.
    /// Rest frame for the two principal strains, pattern frame in the last
    /// column. The piece name is repeated so the file can be read without the
    /// scene.
    pub fn csv(&self, s: &Szene) -> String {
        use std::fmt::Write;
        let mut out = String::from(
            "dreieck;teil;flaeche_cm2;groesste_prozent;kleinste_prozent;muster_prozent\n",
        );
        for t in &s.teile {
            for k in t.dreiecke.clone() {
                let f = self.je_flaeche[k];
                let _ = writeln!(
                    out,
                    "{};{};{:.4};{:+.4};{:+.4};{:+.4}",
                    k,
                    t.name,
                    f.ruheflaeche_m2 * 1e4,
                    f.groesste * 100.0,
                    f.kleinste * 100.0,
                    f.muster * 100.0
                );
            }
        }
        out
    }
}

/// Converts rest-frame strain to pattern-frame strain:
/// `(1 + e)·(1 − r) − 1`.
pub fn muster_dehnung(e_ruhe: f64, reduktion: f64) -> f64 {
    (1.0 + e_ruhe) * (1.0 - reduktion) - 1.0
}

/// Rest area of one face (m²) from the inverse rest matrix the scene keeps for
/// every triangle: `det(M) = 1 / det(M⁻¹)`, and the triangle's area is half
/// that determinant. This is the area the 2D pattern gives, after the
/// reduction — not the current, deformed area.
fn ruheflaeche(ruhe_inv: &[f64; 4]) -> f64 {
    let det = ruhe_inv[0] * ruhe_inv[3] - ruhe_inv[1] * ruhe_inv[2];
    if det == 0.0 {
        return 0.0;
    }
    0.5 / det.abs()
}
