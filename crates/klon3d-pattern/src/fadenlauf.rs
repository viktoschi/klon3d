//! The grainline of a Seamly2D piece.
//!
//! Seamly2D draws a `<grainline>` per piece; klon3d needs its axis as the warp
//! direction for fabric garments (stage "Fabric alongside latex", ADR-021).
//! The semantics model Seamly2D's own behaviour as verified against its SVG
//! export through our Python reference reader, measured on 96 layout pieces:
//!
//! - Only a visible grainline counts: `visible` must be `true`/`1`; a value
//!   outside `true`/`1`/`false`/`0` is not a boolean and Seamly2D refuses the
//!   pattern — the axis is refused here too.
//! - Both end anchors (`topAnchor`, `bottomAnchor`) present: the axis runs
//!   from the bottom to the top anchor as the crow flies; `length` and
//!   `rotation` are not used. `arrowLength` is evaluated anyway — a broken
//!   formula raises, as in Seamly2D, which draws nothing then.
//! - Otherwise the axis is built from `rotation` (degrees) and `length`
//!   (pattern unit). With `centerAnchor` the axis centre sits on that anchor;
//!   without it the axis starts at `mx`/`my` — screen pixels (96 per inch, y
//!   down, plain `xs:double`, not a formula).
//! - An anchor id names a construction point. Modelled points resolve through
//!   their `idObject`; an unknown id is no anchor at all (Seamly2D catches the
//!   bad-id error there); an id of a line, spline or arc is an error.
//! - One drawing detail *does* move the axis: Seamly2D pushes the whole path
//!   into the bounding box of the seam line, so that an anchored grainline
//!   cannot lie outside its piece. The box is the one of a `QGraphicsPathItem`
//!   with Qt's default pen (1 px, bevel join): every edge of the drawn seam
//!   polyline overhangs 0.5 px on both sides. Seamly2D builds it from the
//!   analytic seam edge; this port uses [`Auswertung::stueck_umriss`], whose
//!   curves are sampled (12 steps per segment). On a curved extreme the
//!   sampled outline can sit slightly inside the analytic edge, so box and
//!   pushed axis can differ by a fraction of a pixel: measured against
//!   Seamly2D's SVG export on the test library, `kurve` is off by 0.0959 px
//!   (about 0.025 mm at 96 dpi) while every other piece matches within
//!   0.01 px.
//! - Arrowheads (`arrows` 0 both ends, 1 top only, 2 bottom only, 20 degrees
//!   each side) are a drawing property; their tips take part in the push,
//!   because they can overhang the box, but are not returned.
//! - Missing formulas fall back to Seamly2D's values (vpattern.cpp):
//!   `length` 1, `rotation` 90, `arrowLength` 0.5.
//!
//! Returned points live in the stored pattern frame, the same frame as
//! [`Auswertung::punkt`] and [`Auswertung::stueck_umriss`].

use crate::formula;
use crate::geom::Pt;
use crate::model::{Grainline, Piece};
use crate::geom::GeomError;
use crate::Auswertung;

/// Kind of every `GeomError::Geometrie` raised here.
const KIND: &str = "Fadenlauf";

/// Half the opening of an arrowhead tip: Seamly2D draws the arrows 20 degrees
/// to the left and right of the axis.
const PI_20: f64 = std::f64::consts::PI / 9.0;

/// The grainline axis of a piece plus Seamly2D's arrow count.
///
/// `p1` is the start of the axis (the bottom end when built from anchors),
/// `p2` its end. `pfeile` carries Seamly2D's `arrows` value: 0 arrowheads at
/// both ends, 1 only at `p2`, 2 only at `p1`. The arrowheads themselves are a
/// drawing property and are not represented.
#[derive(Debug, Clone, PartialEq)]
pub struct Fadenlauf {
    pub p1: Pt,
    pub p2: Pt,
    pub pfeile: u8,
}

/// Checks `visible` and `arrows` — the two plain-text attributes.
///
/// `Ok(None)` is an invisible grainline (Seamly2D does not draw it);
/// `Ok(Some(n))` is the arrow count of a visible one. A boolean or arrow
/// value outside the known set is a load error in Seamly2D, so an error here.
fn sichtbar_pfeile(g: &Grainline) -> Result<Option<u8>, GeomError> {
    let sichtbar = g.visible.as_deref().unwrap_or("false");
    let ja = sichtbar == "true" || sichtbar == "1";
    let nein = sichtbar == "false" || sichtbar == "0";
    if !ja && !nein {
        return Err(GeomError::Geometrie {
            kind: KIND,
            grund: format!(
                "visible={sichtbar:?} ist kein Wahrheitswert -- Seamly2D laedt so ein Muster nicht"
            ),
        });
    }
    if !ja {
        return Ok(None);
    }
    let pfeile = match g.arrows.as_deref().unwrap_or("0") {
        "0" => 0u8,
        "1" => 1,
        "2" => 2,
        s => {
            return Err(GeomError::Geometrie {
                kind: KIND,
                grund: format!("arrows={s:?} -- Seamly2D kennt 0, 1, 2"),
            })
        }
    };
    Ok(Some(pfeile))
}

impl Auswertung<'_> {
    /// Screen pixels per pattern unit (96 px per inch, `def.cpp: UnitConvertor`).
    fn px_pro_einheit(&self) -> f64 {
        self.mm_pro_einheit() / 25.4 * 96.0
    }

    /// Stored pattern frame -> canvas pixels, y down (Seamly2D's drawing
    /// frame). The stored frame *is* the canvas orientation: the `.sm2d`
    /// carries the drawing coordinates raw, so neither axis gets mirrored;
    /// schnitt.py's Python counterpart works in a mirrored internal frame and
    /// returns its axis reversed — same physical points either way.
    fn nach_px(p: Pt, k: f64) -> (f64, f64) {
        (p.x * k, p.y * k)
    }

    /// Canvas pixels -> stored pattern frame (no mirroring, see `nach_px`).
    fn aus_px(x: f64, y: f64, k: f64) -> Pt {
        Pt::new(x / k, y / k)
    }

    /// A grainline formula: a number or a formula in the pattern unit; a
    /// failed evaluation raises, because Seamly2D refuses to draw then.
    fn formel(&self, roh: Option<&str>, vorgabe: f64, name: &str) -> Result<f64, GeomError> {
        let Some(t) = roh.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(vorgabe);
        };
        if let Ok(z) = t.parse::<f64>() {
            return Ok(z);
        }
        formula::eval_str(t, self).map_err(|e| GeomError::Geometrie {
            kind: KIND,
            grund: format!("{name}={t:?}: {e}"),
        })
    }

    /// Position of an anchor id, in the stored pattern frame.
    ///
    /// Empty or `"0"` is no anchor. A modelled point resolves through its
    /// `idObject`; an unknown id is no anchor (Seamly2D catches the bad-id
    /// error and leaves the anchor unset); the id of a line, spline or arc is
    /// an error. A point whose formula does not resolve raises as everywhere
    /// else.
    fn anker(&self, roh: Option<&str>) -> Result<Option<Pt>, GeomError> {
        let Some(roh) = roh.map(str::trim).filter(|s| !s.is_empty() && *s != "0") else {
            return Ok(None);
        };
        if let Some((ist_punkt, ziel)) = self.modeling_index().get(roh) {
            if !*ist_punkt {
                return Err(GeomError::Geometrie {
                    kind: KIND,
                    grund: format!("Anker {roh} ist kein Punkt"),
                });
            }
            return self.punkt(ziel).map(Some);
        }
        if self.punkte.contains_key(roh) {
            return self.punkt(roh).map(Some);
        }
        if self.linie(roh).is_some() || self.spline(roh).is_some() || self.bogen(roh).is_some() {
            return Err(GeomError::Geometrie {
                kind: KIND,
                grund: format!("Anker {roh} ist kein Punkt"),
            });
        }
        Ok(None)
    }

    /// A raw `mx`/`my`: screen pixels, a plain `xs:double`, not a formula.
    /// Missing counts as 0.
    fn px_zahl(roh: Option<&str>, key: &str) -> Result<f64, GeomError> {
        let Some(t) = roh else {
            return Ok(0.0);
        };
        t.parse::<f64>().map_err(|_| GeomError::Geometrie {
            kind: KIND,
            grund: format!(
                "{key}={t:?} ist keine Zahl -- Seamly2D laedt so ein Muster nicht (xs:double)"
            ),
        })
    }

    /// Bounding box of a drawn seam polyline as `QGraphicsPathItem` with Qt's
    /// default pen: every edge overhangs 0.5 px on both sides (bevel join).
    /// Doubled neighbours are dropped and a repeated closing point, so that
    /// the case "only one point" counts as no seam line.
    fn qt_huelle(naht: &[(f64, f64)]) -> Result<((f64, f64), (f64, f64)), GeomError> {
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for &p in naht {
            if pts.last().map_or(true, |q| *q != p) {
                pts.push(p);
            }
        }
        if pts.len() > 1 && pts.last() == pts.first() {
            pts.pop();
        }
        if pts.is_empty() {
            return Err(GeomError::Geometrie {
                kind: KIND,
                grund: "das Teil hat keine Nahtlinie".into(),
            });
        }
        let mut alle_xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
        let mut alle_ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
        let n = pts.len();
        for i in 0..n {
            let ((x1, y1), (x2, y2)) = (pts[i], pts[(i + 1) % n]);
            let laenge = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
            if laenge == 0.0 {
                continue;
            }
            // `QLineF::normalVector`, length 0.5
            let (nx, ny) = ((y2 - y1) / laenge * 0.5, -(x2 - x1) / laenge * 0.5);
            for (x, y) in [(x1, y1), (x2, y2)] {
                alle_xs.extend([x + nx, x - nx]);
                alle_ys.extend([y + ny, y - ny]);
            }
        }
        let links = alle_xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let rechts = alle_xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let oben = alle_ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let unten = alle_ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Ok(((links, oben), (rechts, unten)))
    }

    /// The grainline axis of a piece: `None` without a visible grainline.
    ///
    /// Positions come back in the stored pattern frame of
    /// [`Auswertung::punkt`]; errors name the attribute that could not be
    /// evaluated.
    pub fn fadenlauf(&self, stueck: &Piece) -> Result<Option<Fadenlauf>, GeomError> {
        let Some(g) = stueck.grainline.as_ref() else {
            return Ok(None);
        };
        let Some(pfeile) = sichtbar_pfeile(g)? else {
            return Ok(None);
        };
        // All arithmetic runs in screen pixels (y down), exactly as in the
        // drawing, and comes back mirrored. `k` converts units <-> px.
        let k = self.px_pro_einheit();

        // Seamly2D resolves both end anchors first, in either case.
        let oben = self.anker(g.top_anchor.as_deref())?;
        let unten = self.anker(g.bottom_anchor.as_deref())?;
        let (mut p1, theta, laenge_px, pfeil_px);
        if let (Some(u), Some(o)) = (unten, oben) {
            let (ux, uy) = Self::nach_px(u, k);
            let (ox, oy) = Self::nach_px(o, k);
            let (dx, dy) = (ox - ux, oy - uy);
            // `QLineF::length` and `angle`; angle from 0 to below 360.
            let mut grad = (-dy).atan2(dx).to_degrees();
            if grad < 0.0 {
                grad += 360.0;
            }
            if (grad - 360.0).abs() * 1e12 <= grad.abs().min(360.0) {
                grad = 0.0;
            }
            theta = grad.to_radians();
            laenge_px = (dx * dx + dy * dy).sqrt();
            p1 = (ux, uy);
            pfeil_px = self.formel(g.arrow_length.as_deref(), 0.5, "arrowLength")? * k;
        } else {
            theta = self
                .formel(g.rotation.as_deref(), 90.0, "rotation")?
                .to_radians();
            laenge_px = self.formel(g.length.as_deref(), 1.0, "length")? * k;
            pfeil_px = self.formel(g.arrow_length.as_deref(), 0.5, "arrowLength")? * k;
            p1 = match self.anker(g.center_anchor.as_deref())? {
                Some(c) => {
                    // QLineF(c, c + (l / 2, 0)); setAngle; swap the ends;
                    // setLength(l) — the anchor ends up in the axis middle.
                    let (cx, cy) = Self::nach_px(c, k);
                    let halb = laenge_px / 2.0;
                    let mut a = (cx, cy);
                    let mut b = (cx + halb, cy);
                    let strecke = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                    b = (a.0 + theta.cos() * strecke, a.1 - theta.sin() * strecke);
                    std::mem::swap(&mut a, &mut b);
                    let alt = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                    if alt > 0.0 {
                        b = (a.0 + laenge_px * ((b.0 - a.0) / alt), a.1 + laenge_px * ((b.1 - a.1) / alt));
                    }
                    b
                }
                None => (
                    Self::px_zahl(g.mx.as_deref(), "mx")?,
                    Self::px_zahl(g.my.as_deref(), "my")?,
                ),
            };
        }
        let (cos, sin) = (theta.cos(), theta.sin());
        let mut p2 = (p1.0 + laenge_px * cos, p1.1 - laenge_px * sin);

        // The drawn path with arrowhead tips — they take part in the push.
        let zopf = |basis: (f64, f64), basis_winkel: f64| {
            [PI_20, -(PI_20)].map(|d| {
                (
                    basis.0 + pfeil_px * (basis_winkel + d).cos(),
                    basis.1 - pfeil_px * (basis_winkel + d).sin(),
                )
            })
        };
        let mut pfad: Vec<(f64, f64)> = vec![p1];
        if pfeile != 1 {
            pfad.extend(zopf(p1, theta));
        }
        pfad.push(p1);
        pfad.push(p2);
        if pfeile != 2 {
            pfad.extend(zopf(p2, std::f64::consts::PI + theta));
        }
        pfad.push(p2);

        // Seamly2D pushes the whole path into the seam-line box.
        let naht_px: Vec<(f64, f64)> = self
            .stueck_umriss(stueck)?
            .iter()
            .map(|p| Self::nach_px(*p, k))
            .collect();
        let ((huelle_l, huelle_o), (huelle_r, huelle_u)) = Self::qt_huelle(&naht_px)?;
        let (mut dx, mut dy, mut drin) = (0.0f64, 0.0f64, true);
        for &(x, y) in &pfad {
            // QRectF::contains — a rect without extent contains nothing.
            if huelle_l != huelle_r
                && huelle_o != huelle_u
                && huelle_l <= x
                && x <= huelle_r
                && huelle_o <= y
                && y <= huelle_u
            {
                continue;
            }
            let ueber_x = if x < huelle_l {
                huelle_l - x
            } else if x > huelle_r {
                huelle_r - x
            } else {
                0.0
            };
            let ueber_y = if y < huelle_o {
                huelle_o - y
            } else if y > huelle_u {
                huelle_u - y
            } else {
                0.0
            };
            if ueber_x.abs() > dx.abs() {
                dx = ueber_x;
            }
            if ueber_y.abs() > dy.abs() {
                dy = ueber_y;
            }
            drin = false;
        }
        if !drin {
            p1.0 += dx;
            p1.1 += dy;
            p2.0 += dx;
            p2.1 += dy;
        }
        Ok(Some(Fadenlauf {
            p1: Self::aus_px(p1.0, p1.1, k),
            p2: Self::aus_px(p2.0, p2.1, k),
            pfeile,
        }))
    }
}
