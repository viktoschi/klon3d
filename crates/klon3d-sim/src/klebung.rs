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
//! # What the stiffness does not model
//!
//! Bending of the bonded plate. Measured on a 200 × 40 mm strip held at both
//! ends, all three cases - two loose layers, two bonded layers, one 0.8 mm
//! laminate - sag within 0.2 % of each other, because the sag is carried by
//! membrane tension, and two loose layers in parallel carry exactly as much as
//! the laminate (`2 × 348 = 696` N/m). Bending only matters for short, thick or
//! pre-curved pieces, and there the zone reproduces it only as far as the
//! fabric's own bending constraint does. The laminate's plate stiffness
//! (19,858 against 2 × 2,482) is therefore *not* reproduced - a known limit,
//! recorded in the test `durchbiegung_trennt_die_faelle_nicht_und_das_ist_erklaert`.
//!
//! # What this module does not do
//!
//! It does not decide *where* a zone is. The placement file names the zone
//! (`klebt <Teil>`), the geometry comes from the two layers as they are placed.
//! It does not glue against the body (that is CLO's "Tack on Avatar"), it does
//! not skive, and it has no GPU version.

use crate::koerper::{add, dot, mul, norm, sub};
use crate::selbstkollision::Feder;

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
        (Zone { name: name.to_string(), bindungen, steifigkeit }, ohne)
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

/// The pair of layers that a zone bonds, and the spring stiffness that follows
/// from them.
///
/// **Why not the bending stiffness.** `Material::biegung_nm` is a plate
/// stiffness in N·m² (the table value 2,482 is 2,482·10⁻⁹ N·m²). A bond spring
/// needs N/m, and using the plate value directly makes it 10⁹ times too soft -
/// measured: two layers bonded with it slide exactly as if loose. The quantity
/// with the right dimension is the **in-plane** stiffness
/// `Material::dehnung_n_m` (N/m).
///
/// **What the number means.** One bond holds a square patch of the upper
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
        let paar = Stoffpaar { dehnung_oben: 348.274, dicke_oben: 0.4e-3, dicke_unten: 0.4e-3 };
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
        let z = Zone { name: "t".into(), bindungen: vec![b], steifigkeit: 1.0 };
        assert!(z.groesster_abstand(&x) < 1e-12);
        // 2 mm seitlich verschoben: die Feder muss ziehen, nicht nur den Abstand halten.
        x[3][0] += 2e-3;
        assert!((z.groesster_abstand(&x) - 2e-3).abs() < 1e-9);
        let f = federn(&z, &x, 1.0);
        assert!(f[0].kraft[0] < 0.0, "Kraft zieht zurueck: {:?}", f[0].kraft);
    }
}
