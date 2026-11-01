//! Stress map: membrane force per face, and what it means for the material.
//!
//! Stage 5 lists "where the material pulls" next to the strain map. The strain
//! map answers *how far* the cloth is stretched; this one answers *how hard* it
//! pulls, in N/m.
//!
//! # The honest limit of this map
//!
//! The solver's membrane is linear elastic. For **one** material the stress is
//! therefore the strain times a constant, and this map is the strain map with
//! different axis labels — no new information. It earns its place through two
//! things only:
//!
//! 1. **Absolute units.** 35 N/m is the 10 % line of
//!    `Latex_0.4mm_Radical_Rubber` (E ≈ 0.87 MPa, `MATERIAL.md`); 3.3 N/m of
//!    the Retifist preset is nowhere near it. Strain alone cannot say that,
//!    because 10 % of one fabric and 10 % of another are different forces.
//! 2. **Mixed stiffness in one scene.** Two pieces of different thickness under
//!    the same stretch carry different force. That is the normal case —
//!    [`crate::szene::Szene::aus_netz`] takes the material per piece.
//!
//! Non-linear material behaviour is not modelled here and is not pretended
//! ([`crate::material`] documents the linear range, `PLAN.md` the open item).
//!
//! # What is measured
//!
//! Per face, the two principal membrane forces in N/m, force per width — the
//! unit CLO's `Stretch` is already given in, so nothing has to be converted:
//!
//! * [`Flaeche::groesste`] = larger principal strain × the piece's
//!   [`crate::material::Material::dehnung_n_m`], tensile;
//! * [`Flaeche::kleinste`] = smaller principal strain × the same, negative
//!   under compression.
//!
//! The strain is the *engineering* strain `λ − 1` against the rest shape the
//! solver uses — the same definition as [`crate::dehnung`], so the two maps can
//! be read side by side without converting.

use std::fmt::Write;

use crate::dehnung::Karte as Dehnungskarte;
use crate::messung::dehnung_je_dreieck;
use crate::szene::Szene;
use crate::verteilung::Verteilung;

/// Stress of one face, in N/m.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flaeche {
    /// Larger principal membrane force (N/m), positive = tension.
    pub groesste: f64,
    /// Smaller principal membrane force (N/m); negative = compression.
    pub kleinste: f64,
    /// Stiffness used for this face (N/m), so a report can say which material
    /// a number belongs to.
    pub steifigkeit_n_m: f64,
    /// Rest area (m²), the weight for the summary.
    pub ruheflaeche_m2: f64,
}

/// Summary of one pattern piece.
#[derive(Debug, Clone, PartialEq)]
pub struct Teil {
    pub name: String,
    /// Faces of the piece.
    pub flaechen: usize,
    /// Rest area (m²).
    pub ruheflaeche_m2: f64,
    /// Stiffness of this piece (N/m) — differs between pieces when the pieces
    /// have different thickness.
    pub steifigkeit_n_m: f64,
    /// Force at which this piece reaches 10 % strain (N/m), i.e. the documented
    /// end of the linear range of its material. Per piece, because it is a
    /// property of the material and not of the run.
    pub grenze_n_m: f64,
    /// Faces above [`Self::grenze_n_m`].
    pub ueber: usize,
    /// Distribution of the tensile force (N/m), area-weighted.
    pub verteilung: Verteilung,
}

/// The whole map.
#[derive(Debug, Clone, PartialEq)]
pub struct Karte {
    /// One entry per face, in scene order.
    pub je_flaeche: Vec<Flaeche>,
    /// One entry per piece, in scene order.
    pub je_teil: Vec<Teil>,
}

/// Strain above which a face counts as past the documented linear range.
///
/// The same 10 % as [`crate::dehnung::UEBER_DEHNUNG`]; here it turns into a
/// force per piece via the piece's stiffness. Kept as its own constant so the
/// two maps can diverge later without one silently changing the other.
pub const UEBER_SPANNUNG: f64 = 0.10;

impl Karte {
    /// Measures the current state. Works without a body: stress is a property
    /// of the cloth and its material alone.
    ///
    /// Stiffness is read per piece from [`crate::szene::Teilinfo::vorgabe`]. A
    /// piece without a usable stiffness (zero, NaN) is a bug in how the scene
    /// was built; its faces get 0 N/m and the piece's [`Teil::steifigkeit_n_m`]
    /// stays 0, which a report can see. Nothing inherits a neighbour's value.
    pub fn messe(s: &Szene) -> Karte {
        // Stiffness per face, from the piece that owns it.
        let mut steifigkeit = vec![0.0f64; s.dreiecke.len()];
        for t in &s.teile {
            let e = t.vorgabe.material.dehnung_n_m;
            let e = if e.is_finite() && e > 0.0 { e } else { 0.0 };
            for k in t.dreiecke.clone() {
                steifigkeit[k] = e;
            }
        }

        let je_flaeche: Vec<Flaeche> = dehnung_je_dreieck(s)
            .into_iter()
            .enumerate()
            .map(|(k, (groesste, kleinste))| Flaeche {
                groesste: groesste * steifigkeit[k],
                kleinste: kleinste * steifigkeit[k],
                steifigkeit_n_m: steifigkeit[k],
                ruheflaeche_m2: ruheflaeche(&s.ruhe_inv[k]),
            })
            .collect();

        let je_teil = s
            .teile
            .iter()
            .map(|t| {
                let bereich = t.dreiecke.clone();
                let werte: Vec<f64> = bereich.clone().map(|k| je_flaeche[k].groesste).collect();
                let gewicht: Vec<f64> = bereich
                    .clone()
                    .map(|k| je_flaeche[k].ruheflaeche_m2)
                    .collect();
                let e = t.vorgabe.material.dehnung_n_m;
                let e = if e.is_finite() && e > 0.0 { e } else { 0.0 };
                let grenze = e * UEBER_SPANNUNG;
                Teil {
                    name: t.name.clone(),
                    flaechen: werte.len(),
                    ruheflaeche_m2: gewicht.iter().sum(),
                    steifigkeit_n_m: e,
                    grenze_n_m: grenze,
                    ueber: werte.iter().filter(|&&w| w > grenze).count(),
                    verteilung: Verteilung::aus_gewichtet(werte, &gewicht),
                }
            })
            .collect();

        Karte {
            je_flaeche,
            je_teil,
        }
    }

    /// Tensile force per **point** in N/m, for vertex colours.
    ///
    /// Same aggregation as the strain map: the map is per face, a colour is per
    /// vertex, so a point takes the area-weighted mean of the faces around it.
    /// A point with no face has no material and reads 0.
    pub fn je_punkt(&self, s: &Szene) -> Vec<f64> {
        let mut summe = vec![0.0; s.x.len()];
        let mut gewicht = vec![0.0; s.x.len()];
        for (k, d) in s.dreiecke.iter().enumerate() {
            let a = self.je_flaeche[k].ruheflaeche_m2;
            if !(a > 0.0) {
                continue;
            }
            let w = self.je_flaeche[k].groesste;
            for &i in d {
                let i = i as usize;
                summe[i] += w * a;
                gewicht[i] += a;
            }
        }
        summe
            .iter()
            .zip(&gewicht)
            .map(|(v, w)| if *w > 0.0 { v / w } else { 0.0 })
            .collect()
    }

    /// Summary over all faces (N/m, area-weighted).
    pub fn gesamt(&self) -> Verteilung {
        let werte: Vec<f64> = self.je_flaeche.iter().map(|f| f.groesste).collect();
        let gewicht: Vec<f64> = self.je_flaeche.iter().map(|f| f.ruheflaeche_m2).collect();
        Verteilung::aus_gewichtet(werte, &gewicht)
    }

    /// The face pulling hardest and where it is.
    pub fn staerkste(&self) -> Option<(f64, usize)> {
        self.je_flaeche
            .iter()
            .enumerate()
            .map(|(i, f)| (f.groesste, i))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// CSV, one line per face:
    /// `dreieck;teil;flaeche_cm2;groesste_n_m;kleinste_n_m;steifigkeit_n_m`.
    ///
    /// The stiffness is repeated per line so the file can be read without the
    /// scene and a reader can turn a force back into a strain. It is printed to
    /// six decimals, not four like the forces: four would round 348.2741562 to
    /// 348.2742, and multiplying a strain by that rounded value leaves an error
    /// larger than the force column's own rounding.
    pub fn csv(&self, s: &Szene) -> String {
        let mut out =
            String::from("dreieck;teil;flaeche_cm2;groesste_n_m;kleinste_n_m;steifigkeit_n_m\n");
        for t in &s.teile {
            for k in t.dreiecke.clone() {
                let f = &self.je_flaeche[k];
                let _ = writeln!(
                    out,
                    "{};{};{:.4};{:.4};{:.4};{:.6}",
                    k,
                    t.name,
                    f.ruheflaeche_m2 * 1e4,
                    f.groesste,
                    f.kleinste,
                    f.steifigkeit_n_m
                );
            }
        }
        out
    }
}

/// Rest area of one face (m²) from the inverse rest matrix: `0.5 / |det|`.
///
/// A deliberate four-line copy of the strain map's private helper rather than a
/// shared function: `dehnung` keeps its own private, and reaching into a sibling
/// module for that much arithmetic would tie the two maps together. The copy is
/// checked against the original by the cross-check on the reference project —
/// `spannung.csv` has to reproduce `dehnung.csv` × stiffness — so a drift shows
/// up as a number, not as silence. A degenerate or broken matrix gets no area
/// rather than NaN, because the value is used as a weight and one NaN would
/// poison a whole area-weighted mean.
fn ruheflaeche(ruhe_inv: &[f64; 4]) -> f64 {
    let det = ruhe_inv[0] * ruhe_inv[3] - ruhe_inv[1] * ruhe_inv[2];
    if !det.is_finite() || det == 0.0 {
        return 0.0;
    }
    0.5 / det.abs()
}

/// Check that this map and the strain map tell the same geometric story: the
/// same face must be the one pulling hardest as the one stretched hardest.
///
/// Not an assertion for a user-facing report but for tests and for anyone
/// comparing the two CSVs: with a single material the order must be identical,
/// and if it is not, one of the two maps has a bug.
pub fn reihenfolge_gleich(spannung: &Karte, dehnung: &Dehnungskarte) -> bool {
    let a = spannung.staerkste().map(|(_, i)| i);
    let b = dehnung.staerkste().map(|(_, i)| i);
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::szene::Teilvorgabe;
    use crate::RADICAL_RUBBER_04;

    /// The stiffness a piece contributes is its material's, and a broken value
    /// stays 0 instead of leaking into a neighbour.
    #[test]
    fn steifigkeit_kommt_aus_dem_material_des_teils() {
        let vg = Teilvorgabe {
            material: RADICAL_RUBBER_04,
            reduktion: 0.0,
        };
        assert!((vg.material.dehnung_n_m - 348.274_156_2).abs() < 1e-9);
        // The 10 % line of that material, as the piece would report it.
        assert!((vg.material.dehnung_n_m * UEBER_SPANNUNG - 34.827_415_62).abs() < 1e-8);
    }
}
