//! Glue zones: areas where two layers bond firmly.
//!
//! A laminate, a reinforcement, an appliqué or an overlapping seam bonded with
//! glue may no longer slide: the bonded points keep their position relative to
//! each other. The model is CLO's tack spring (finding 59, section O5): a soft
//! spring from a cloth point to a point given in barycentric coordinates on a
//! triangle, with a rest offset and a finite stiffness. Not a rigid coupling -
//! the force goes through a Jacobian, exactly like the proximity springs in
//! [`crate::selbstkollision`].
//!
//! # Data
//!
//! A zone is a list of pairs: cloth point `i` and the triangle it is bonded to,
//! together with the barycentric coordinates of the bond point inside that
//! triangle. The bond point follows the triangle as it deforms.
//!
//! What the spring holds is the **offset vector**, not a distance. That is the
//! difference between glue and a mere tether: a distance spring has no
//! transverse stiffness at rest length, so two layers held by one slide on each
//! other exactly like two loose layers - measured, they did. Holding the offset
//! is what stopping the slide means.
//!
//! The rest offset is the vector between the two points when the zone is built,
//! so a layer may sit at any distance (thickness, air) and stays there.
//!
//! # Stiffness
//!
//! One bond holds a patch of the interface of its own size; its stiffness is
//! the **in-plane** stiffness of the upper fabric ([`Stoffpaar`]). Glue
//! therefore never transmits more load than the layer itself carries - it only
//! stops the layers from sliding. The plate value `Material::biegung_nm` is
//! *not* used: it is in N·m² and is 10⁹ times too soft as a spring constant
//! (measured: bonded layers slid exactly as if loose).
//!
//! # Bending
//!
//! Bonding alone does not make a plate. The bond is a shear spring: it stops
//! the layers from sliding, and that is all a shear spring can do at the
//! interface. Composite bending comes from the layers being held at a distance
//! from the common neutral axis, and the fabric's own bending constraints know
//! nothing about the other layer - so two bonded layers bent as `2 × D` while
//! the laminate of the same thickness bends as `8 × D` (MATERIAL.md).
//!
//! The bending term is therefore put on the constraints that lie **inside** the
//! bonded area: their compliance is divided by [`Stoffpaar::biegefaktor`], so
//! the pair carries the plate stiffness of the total thickness. For two equal
//! layers the factor is 4 - each layer contributes 4 × 2,482 = 9,928 about the
//! common axis, together 19,858, the laminate row of MATERIAL.md. The factor is
//! the same for both layers because the extra stiffness is distributed in
//! proportion to each layer's own `biegung_nm`.
//!
//! Only constraints whose **both** adjacent triangles are bonded are touched, so
//! a zone that covers half a piece does not stiffen the rest: a glued
//! reinforcement must not make the whole panel rigid.
//!
//! # Measured limit
//!
//! On a 200 × 40 mm strip held at both ends all three cases - two loose layers,
//! two bonded layers, one 0.8 mm laminate - sag within 0.2 % of each other, and
//! that stays true with the bending term: the sag there is carried by membrane
//! tension, not by bending, and two loose layers in parallel carry exactly as
//! much as the laminate (`2 × 348 = 696` N/m). The strip result is recorded in
//! `durchbiegung_trennt_die_faelle_nicht_und_das_ist_erklaert`.
//!
//! A cantilever does not help either, although it is the obvious piece for
//! bending: at the mesh sizes that solve here it deforms into a straight line -
//! it shears instead of bending - and making the bending constraints 10×
//! stiffer moves the tip by 0.03 %. The acceptance therefore measures the plate
//! stiffness directly, as energy at a prescribed curvature
//! (`geklebtes_paar_traegt_die_plattensteifigkeit_des_laminats`), where a
//! circular arc carries no membrane strain and the energy is `½·D·κ²·A`.
//!
//! # What this module does not do
//!
//! It does not decide *where* a zone is. The placement file names the zone
//! (`klebt <Teil>`), the geometry comes from the two layers as they are placed.
//! It does not glue against the body (that is CLO's "Tack on Avatar"), it does
//! not skive, and it has no GPU version.

use std::collections::BTreeSet;

use crate::koerper::{add, dot, mul, norm, sub};
use crate::selbstkollision::Feder;
use crate::szene::Szene;

/// A bond point: cloth point `punkt`, triangle `dreieck` of the scene, and the
/// barycentric coordinates of the bond point in that triangle.
///
/// `bary` sums to 1. A bond to a corner (`[1, 0, 0]`) is allowed and is what
/// the glue on a single vertex reduces to - two layers of the same pattern bond
/// vertex to vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bindung {
    pub punkt: u32,
    pub dreieck: [u32; 3],
    pub bary: [f64; 3],
    /// Rest offset in m: the vector from the bond point to the cloth point when
    /// the zone was built. A laminate keeps the thickness it was placed with.
    ///
    /// A vector, not a length: glue holds the layers **in place relative to
    /// each other**, so a bond that is offset sideways must not slide back along
    /// the surface. A plain distance spring cannot do that - its transverse
    /// stiffness vanishes at rest length - and two layers held by one would
    /// slide on each other exactly like two loose layers.
    pub versatz: [f64; 3],
}

/// One glue zone: a name for the report, the bonds it holds and the stiffness
/// of the bonded unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub name: String,
    pub bindungen: Vec<Bindung>,
    /// Spring stiffness of the whole zone in N/m, spread over its bonds
    /// ([`Stoffpaar::steifigkeit`]).
    pub steifigkeit: f64,
    /// Factor on the bending stiffness of the constraints inside the bonded
    /// area ([`Stoffpaar::biegefaktor`]). `1.0` leaves them untouched.
    pub biegefaktor: f64,
}

impl Zone {
    /// Bonds every point of `oben` to the nearest point of the layer below,
    /// over the whole area. Both layers are given as point ranges of the scene
    /// (`teile[n].punkte`).
    ///
    /// This is the "Glue Fill" case: laminate and reinforcement, where the
    /// whole face is bonded. The bond point is the projection of the cloth point
    /// onto the lower layer; a point that projects onto no triangle is left
    /// unglued and counted in the returned number, so a zone that silently
    /// misses half its area cannot pass as complete.
    ///
    /// `dreiecke` are the scene's triangles, `x` its positions. Only triangles
    /// whose three corners lie in `unten` are considered. `paar` gives the
    /// materials and with them the stiffness of the zone; it depends on the
    /// number of bonds found, so it is evaluated after the bonds are built.
    pub fn flaeche(
        name: &str,
        x: &[[f64; 3]],
        dreiecke: &[[u32; 3]],
        oben: std::ops::Range<usize>,
        unten: std::ops::Range<usize>,
        suchradius: f64,
        paar: Stoffpaar,
    ) -> (Zone, usize) {
        let mut bindungen = Vec::new();
        let mut ohne = 0;
        let unten_dreiecke: Vec<[u32; 3]> = dreiecke
            .iter()
            .copied()
            .filter(|d| d.iter().all(|&i| unten.contains(&(i as usize))))
            .collect();
        for p in oben {
            let q = x[p];
            let mut best: Option<(f64, [u32; 3], [f64; 3])> = None;
            for d in &unten_dreiecke {
                let (a, b, c) = (x[d[0] as usize], x[d[1] as usize], x[d[2] as usize]);
                let (naechster, bary) = naechster_mit_bary(q, a, b, c);
                let abstand = norm(sub(q, naechster));
                if abstand <= suchradius && best.map_or(true, |(s, _, _)| abstand < s) {
                    best = Some((abstand, *d, bary));
                }
            }
            match best {
                Some((_, dreieck, bary)) => {
                    let pp = x[p];
                    let q = add(
                        add(mul(x[dreieck[0] as usize], bary[0]), mul(x[dreieck[1] as usize], bary[1])),
                        mul(x[dreieck[2] as usize], bary[2]),
                    );
                    bindungen.push(Bindung { punkt: p as u32, dreieck, bary, versatz: sub(pp, q) });
                }
                None => ohne += 1,
            }
        }
        let steifigkeit = paar.steifigkeit(bindungen.len());
        (Zone { name: name.to_string(), bindungen, steifigkeit, biegefaktor: paar.biegefaktor() }, ohne)
    }

    pub fn len(&self) -> usize {
        self.bindungen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindungen.is_empty()
    }

    /// Largest current deviation from the rest offset; the measure for whether a
    /// glued edge has come open or a layer has slid.
    pub fn groesster_abstand(&self, x: &[[f64; 3]]) -> f64 {
        self.bindungen
            .iter()
            .map(|b| norm(sub(sub(x[b.punkt as usize], bond_punkt(b, x)), b.versatz)))
            .fold(0.0, f64::max)
    }

    /// Triangles of the scene that lie fully inside the bonded area.
    ///
    /// **Which points count as bonded.** Both sides of the interface: the cloth
    /// points the bonds hang on (the upper layer's points) and the corners of
    /// the triangles the bonds project into (the lower layer's points). The
    /// second set is needed because the lower layer is meshed on its own - it
    /// has triangles that no bond point falls into, simply because two upper
    /// points shared a triangle. Without them the lower layer keeps its own
    /// bending stiffness and the pair ends up at 3.25× instead of 4× the loose
    /// layers (measured).
    ///
    /// **Why the closure stops here.** A triangle is covered when *all* three
    /// corners are bonded. That is one local step - no flood fill, no radius -
    /// and it keeps a glued patch from stiffening the piece around it: a
    /// triangle on the border of the zone touches unbonded points and stays out.
    fn gebundene_dreiecke(&self, s: &Szene) -> BTreeSet<[u32; 3]> {
        let schluessel = |d: [u32; 3]| {
            let mut k = d;
            k.sort_unstable();
            k
        };
        let mut punkte: BTreeSet<u32> = self.bindungen.iter().map(|b| b.punkt).collect();
        for b in &self.bindungen {
            punkte.extend(b.dreieck.iter().copied());
        }
        let mut gebunden = BTreeSet::new();
        for d in &s.dreiecke {
            if d.iter().all(|i| punkte.contains(i)) {
                gebunden.insert(schluessel(*d));
            }
        }
        gebunden
    }

    /// Indices of the scene's bending constraints that lie inside the bonded
    /// area: both triangles of the constraint are bonded. Those are the ones
    /// whose compliance has to shrink by [`Self::biegefaktor`].
    pub fn biege_constraints(&self, s: &Szene) -> Vec<usize> {
        if (self.biegefaktor - 1.0).abs() < 1e-12 || self.is_empty() {
            return Vec::new();
        }
        let gebunden = self.gebundene_dreiecke(s);
        let schluessel = |d: [u32; 3]| {
            let mut k = d;
            k.sort_unstable();
            k
        };
        let mut idx = Vec::new();
        for (c, p) in s.biegung.punkte.iter().enumerate() {
            let [i, j, o_a, o_b] = *p;
            if gebunden.contains(&schluessel([i, j, o_a])) && gebunden.contains(&schluessel([j, i, o_b])) {
                idx.push(c);
            }
        }
        idx
    }
}

/// Position of the bond point in the scene: the barycentric combination of the
/// triangle's current corners.
pub fn bond_punkt(b: &Bindung, x: &[[f64; 3]]) -> [f64; 3] {
    let (a, bb, c) = (x[b.dreieck[0] as usize], x[b.dreieck[1] as usize], x[b.dreieck[2] as usize]);
    add(add(mul(a, b.bary[0]), mul(bb, b.bary[1])), mul(c, b.bary[2]))
}

/// Nächster Punkt auf dem Dreieck (a, b, c) zu p, mit baryzentrischen
/// Koordinaten.
///
/// Regionen wie in [`crate::netzkoerper::naechster_auf_dreieck`] (Ericson 2005,
/// 5.1.5); hier zusätzlich die Gewichte, weil die Klebe den Punkt *im* Dreieck
/// braucht und nicht nur seine Lage.
pub fn naechster_mit_bary(p: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let quot = |z: f64, n: f64| if n > 0.0 { z / n } else { 0.0 };
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, [1.0, 0.0, 0.0]);
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (b, [0.0, 1.0, 0.0]);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let w = quot(d1, d1 - d3);
        return (add(a, mul(ab, w)), [1.0 - w, w, 0.0]);
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (c, [0.0, 0.0, 1.0]);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = quot(d2, d2 - d6);
        return (add(a, mul(ac, w)), [1.0 - w, 0.0, w]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = quot(d4 - d3, (d4 - d3) + (d5 - d6));
        return (add(b, mul(sub(c, b), w)), [0.0, 1.0 - w, w]);
    }
    let s = va + vb + vc;
    let (v, w) = (quot(vb, s), quot(vc, s));
    let an = add(a, add(mul(ab, v), mul(ac, w)));
    (an, [1.0 - v - w, v, w])
}

/// The pair of layers that a zone bonds, the spring stiffness that follows from
/// them, and the bending the bonded unit gains over two loose layers.
///
/// **Why not the bending stiffness for the bond.** `Material::biegung_nm` is a
/// plate stiffness in N·m² (the table value 2,482 is 2,482·10⁻⁹ N·m²). A bond
/// spring needs N/m, and using the plate value directly makes it 10⁹ times too
/// soft - measured: two layers bonded with it slide exactly as if loose. The
/// quantity with the right dimension for the bond is the **in-plane** stiffness
/// `Material::dehnung_n_m` (N/m).
///
/// **What the bond number means.** One bond holds a square patch of the upper
/// fabric of its own size. Stretching a square patch by `u` costs
/// `dehnung · u` (width and length are equal, so they cancel), so a single bond
/// is as stiff as `dehnung_n_m` in N/m. The zone as a whole is that times its
/// number of bonds. The interface can therefore never transmit more load than
/// the layer itself carries - glue does not create stiffness the fabric does
/// not have, it only stops the layers from sliding.
#[derive(Debug, Clone, Copy)]
pub struct Stoffpaar {
    /// In-plane stiffness of the upper layer in N/m (`Material::dehnung_n_m`).
    pub dehnung_oben: f64,
    /// Thickness of the upper layer in m - kept for the report and for the
    /// rest offset, which is the gap the layers were placed with.
    pub dicke_oben: f64,
    /// Thickness of the lower layer in m.
    pub dicke_unten: f64,
    /// Plate stiffness of the upper layer in N·m² (`Material::biegung_nm`).
    pub biegung_oben: f64,
    /// Plate stiffness of the lower layer in N·m².
    pub biegung_unten: f64,
}

impl Stoffpaar {
    /// Stiffness of one bond in N/m.
    pub fn steifigkeit_einzeln(&self) -> f64 {
        self.dehnung_oben
    }

    /// Stiffness of the whole zone in N/m.
    ///
    /// Grows with the number of bonds: a finer bond samples the same interface
    /// more often, and each sample holds its own patch. That is a property of
    /// the discretisation, not of the glue - see the limitation in the module
    /// docs.
    pub fn steifigkeit(&self, bindungen: usize) -> f64 {
        self.dehnung_oben * bindungen.max(1) as f64
    }

    /// Bending stiffness of the bonded unit over the sum of the two layers'
    /// own plate stiffness, i.e. the factor each layer's bending constraints
    /// get inside the zone.
    ///
    /// **Why not simply 2.** Two loose layers bend about their own mid-planes:
    /// `D₁ + D₂`. Bonded, the pair bends about one common neutral axis, and a
    /// layer whose mid-plane sits `c` away from it carries
    /// `D + E·t·c²` (parallel axis). With `D = E·t³/12` the term is
    /// `12·D/t² · c²`, so for two equal layers with `c = t/2` each:
    /// `D + 3·D = 4·D`, together `8·D` - the `Latex_Laminat_2x0.4mm` row of
    /// MATERIAL.md (19,858 against 8 × 2,482).
    ///
    /// The modulus is taken from the **bending** stiffness (`12·D/t²`), not
    /// from the stretch stiffness: the measured latex bends stiffer than its
    /// stretch predicts, and the laminate rows in MATERIAL.md follow `D ∝ t³`
    /// of the bending values. Using the stretch stiffness gives 3.27 instead of
    /// 4 - measured, not assumed.
    ///
    /// The one factor is applied to **both** layers' bending constraints, so
    /// the pair as a whole ends up at `factor · (D₁ + D₂)`. That reproduces the
    /// equal-layer case exactly; for layers of different thickness the parallel
    /// term is distributed in proportion to `D`, not to `D + E·t·c²`, which is
    /// the known limit of this task (see the PR).
    ///
    /// `1.0` when the layers cannot bond (zero stiffness) or a plate value is
    /// missing: a pair that carries no shear gets no plate stiffness either.
    pub fn biegefaktor(&self) -> f64 {
        let (d_1, d_2) = (self.biegung_oben, self.biegung_unten);
        let (t_1, t_2) = (self.dicke_oben, self.dicke_unten);
        if d_1 <= 0.0 || d_2 <= 0.0 || t_1 <= 0.0 || t_2 <= 0.0 {
            return 1.0;
        }
        // Axial stiffness per layer consistent with its own plate stiffness.
        let (ea_1, ea_2) = (12.0 * d_1 / (t_1 * t_1), 12.0 * d_2 / (t_2 * t_2));
        // Mid-planes relative to the interface: the upper layer sits half its
        // thickness above, the lower half its thickness below.
        let (z_1, z_2) = (0.5 * t_1, -0.5 * t_2);
        let z_s = (ea_1 * z_1 + ea_2 * z_2) / (ea_1 + ea_2);
        let d_parallel = ea_1 * (z_1 - z_s).powi(2) + ea_2 * (z_2 - z_s).powi(2);
        (d_1 + d_2 + d_parallel) / (d_1 + d_2)
    }
}

/// The springs of one zone, in the shape the implicit step already sums.
///
/// Five places with weights, exactly like a proximity spring: the cloth point
/// with `+1`, the three triangle corners with their negative barycentric
/// weights, the fifth at zero. Because the weights sum to zero the spring is
/// translation-invariant, and `Σ bary_i·corner_i` is the bond point - the step
/// therefore sees a force between the cloth point and the bond point and
/// assembles the matching block itself.
///
/// The spring holds the **offset vector**, not a distance: the force is
/// `k·(versatz − (x_i − bond))` and the block is the full `k·I`. That is what
/// makes it glue rather than merely tether: at rest length a distance spring has
/// no transverse stiffness and the two layers slide on each other, whereas a
/// bond that holds the offset keeps every point of the upper layer where it was
/// placed on the lower one. It is still a spring with a Jacobian, not a rigid
/// constraint - CLO's tack spring is soft for the same reason (finding 59).
///
/// Each bond carries its own patch of the interface: `zone.steifigkeit` is the
/// zone's total, spread evenly over its bonds, so one bond gets the in-plane
/// stiffness of one patch ([`Stoffpaar`]).
pub fn federn(zone: &Zone, x: &[[f64; 3]], faktor: f64) -> Vec<Feder> {
    let n = zone.len().max(1) as f64;
    let steifigkeit = zone.steifigkeit * faktor;
    zone.bindungen
        .iter()
        .map(|b| {
            let (i, a, bb, c) = (b.punkt, b.dreieck[0], b.dreieck[1], b.dreieck[2]);
            let gewichte = [1.0, -b.bary[0], -b.bary[1], -b.bary[2], 0.0];
            let k = steifigkeit / n;
            // Kraft auf den Stoffpunkt: k·(Ruheversatz − Istversatz).
            let abw = sub(b.versatz, sub(x[i as usize], bond_punkt(b, x)));
            let kraft = mul(abw, k);
            let mut block = [[0.0; 3]; 3];
            for (m, z) in block.iter_mut().enumerate() {
                z[m] = k;
            }
            Feder { punkte: [i, a, bb, c, 0], gewichte, kraft, block }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    /// A right triangle in the z = 0 plane, 1 m on each leg.
    fn dreieck() -> ([f64; 3], [f64; 3], [f64; 3]) {
        ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    }

    #[test]
    fn bary_im_innenraum_und_auf_den_ecken() {
        let (a, b, c) = dreieck();
        // Der nächste Punkt liegt in der Dreiecksebene, also bei z = 0.
        let (p, w) = naechster_mit_bary([0.25, 0.25, 0.5], a, b, c);
        assert!(p[2].abs() < 1e-12, "Projektion: {p:?}");
        assert!((p[0] - 0.25).abs() < 1e-12 && (p[1] - 0.25).abs() < 1e-12);
        assert!((w[0] - 0.5).abs() < 1e-12);
        assert!((w[1] - 0.25).abs() < 1e-12);
        assert!((w[2] - 0.25).abs() < 1e-12);

        let (_, w) = naechster_mit_bary([-0.5, -0.5, 0.0], a, b, c);
        assert!((w[0] - 1.0).abs() < 1e-12, "Ecke a: {w:?}");
        let (_, w) = naechster_mit_bary([2.0, -0.5, 0.0], a, b, c);
        assert!(w[1] > 0.99, "Ecke b: {w:?}");
    }

    #[test]
    fn gewichte_summe_ist_eins() {
        let (a, b, c) = dreieck();
        for p in [[0.1, 0.1, 0.0], [0.9, 0.05, 0.3], [-1.0, 0.5, 0.0], [0.5, 0.9, -2.0]] {
            let (_, w) = naechster_mit_bary(p, a, b, c);
            let s: f64 = w.iter().sum();
            assert!((s - 1.0).abs() < 1e-12, "{p:?} → {w:?}");
        }
    }

    /// The zone's stiffness is the in-plane stiffness of one patch per bond; a
    /// single bond therefore carries exactly what the fabric carries, and the
    /// interface can never be stiffer than the layer itself.
    #[test]
    fn steifigkeit_ist_die_der_lage() {
        let paar = Stoffpaar {
            dehnung_oben: 348.274,
            dicke_oben: 0.4e-3,
            dicke_unten: 0.4e-3,
            biegung_oben: 2482.2424e-9,
            biegung_unten: 2482.2424e-9,
        };
        assert!((paar.steifigkeit_einzeln() - 348.274).abs() < 1e-9);
        assert!((paar.steifigkeit(1) - 348.274).abs() < 1e-9);
        assert!((paar.steifigkeit(10) - 3482.74).abs() < 1e-9);
        // Nie steifer als die Lage: eine Zone mit k Bindungen entspricht einer
        // Fläche mit k Patches, also genau dem Stoff darüber.
        let zone_k = paar.steifigkeit(100) / 100.0;
        assert!((zone_k - paar.steifigkeit_einzeln()).abs() < 1e-9);
    }

    #[test]
    fn bond_punkt_folgt_dem_dreieck() {
        let b = Bindung { punkt: 3, dreieck: [0, 1, 2], bary: [0.0, 0.5, 0.5], versatz: [0.0; 3] };
        let x = vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [9.0, 9.0, 9.0]];
        let p = bond_punkt(&b, &x);
        assert!((p[0] - 1.0).abs() < 1e-12 && (p[1] - 1.0).abs() < 1e-12);
        // Verschiebt sich das Dreieck, wandert der Punkt mit.
        let x2 = vec![[0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [0.0, 2.0, 1.0], [9.0, 9.0, 9.0]];
        assert!((bond_punkt(&b, &x2)[2] - 1.0).abs() < 1e-12);
    }

    /// Der Ruheversatz wird gehalten: nur wenn die Lage wirklich rutscht,
    /// meldet die Zone einen Abstand.
    #[test]
    fn versatz_wird_gehalten() {
        let mut x = vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [1.0, 1.0, 0.4e-3]];
        let b = Bindung { punkt: 3, dreieck: [0, 1, 2], bary: [0.0, 0.5, 0.5], versatz: [0.0, 0.0, 0.4e-3] };
        let z = Zone { name: "t".into(), bindungen: vec![b], steifigkeit: 1.0, biegefaktor: 1.0 };
        assert!(z.groesster_abstand(&x) < 1e-12);
        // 2 mm seitlich verschoben: die Feder muss ziehen, nicht nur den Abstand halten.
        x[3][0] += 2e-3;
        assert!((z.groesster_abstand(&x) - 2e-3).abs() < 1e-9);
        let f = federn(&z, &x, 1.0);
        assert!(f[0].kraft[0] < 0.0, "Kraft zieht zurueck: {:?}", f[0].kraft);
    }
}
