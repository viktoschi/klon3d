//! Exakte Vorzeichentests auf einem Ganzzahlgitter.
//!
//! Die Triangulierung entscheidet über zwei Vorzeichen: auf welcher Seite
//! einer Geraden ein Punkt liegt ([`orient`]) und ob er im Umkreis eines
//! Dreiecks liegt ([`umkreis`]). In Gleitkomma fallen beide bei fast
//! kollinearen oder fast kozirkularen Punkten zufällig aus, und dann zerfällt
//! das Netz. Gleichmäßig unterteilte gerade Kanten sind genau dieser Fall.
//!
//! Deshalb liegen alle Punkte eines Teils auf einem Gitter mit höchstens
//! 2²⁸ Schritten über das Teil (Teilung 10⁻⁵ mm bis 2,5 m Ausdehnung), und
//! beide Tests rechnen ohne Rundung in `i128`: Differenzen < 2²⁹, Produkte
//! im Umkreistest < 2¹¹⁶. Die Lage der Randpunkte selbst bleibt unverändert
//! (`Triangulierung::pf`), gerundet wird nur für die Entscheidungen.

/// Gitterpunkt.
pub(crate) type Ip = [i64; 2];

/// Größte Gitterkoordinate eines Teils.
const SCHRITTE: f64 = (1u64 << 28) as f64;

/// Feinste Gitterteilung in mm.
const Q_MIN: f64 = 1e-5;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Gitter {
    ursprung: [f64; 2],
    /// Gitterteilung in mm.
    pub q: f64,
}

impl Gitter {
    /// Gitter über die Punkte eines Teils, mit 5 % Rand.
    pub fn fuer(punkte: &[[f64; 2]]) -> Self {
        let (mut min, mut max) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in punkte {
            for k in 0..2 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let ausdehnung = (max[0] - min[0]).max(max[1] - min[1]).max(1.0);
        let rand = 0.05 * ausdehnung;
        let q = Q_MIN.max((ausdehnung + 2.0 * rand) / SCHRITTE);
        Gitter { ursprung: [min[0] - rand, min[1] - rand], q }
    }

    pub fn ein(&self, p: [f64; 2]) -> Ip {
        [
            ((p[0] - self.ursprung[0]) / self.q).round() as i64,
            ((p[1] - self.ursprung[1]) / self.q).round() as i64,
        ]
    }

    pub fn aus(&self, p: Ip) -> [f64; 2] {
        [self.ursprung[0] + p[0] as f64 * self.q, self.ursprung[1] + p[1] as f64 * self.q]
    }

    /// Kürzt den Weg von `von` nach `nach` (mm) so, dass sein Ende im
    /// Gitterbereich bleibt. Die Richtung bleibt erhalten.
    pub fn im_bereich(&self, von: [f64; 2], nach: [f64; 2]) -> [f64; 2] {
        let lo = self.ursprung;
        let hi = [lo[0] + SCHRITTE * self.q, lo[1] + SCHRITTE * self.q];
        let mut t: f64 = 1.0;
        for k in 0..2 {
            let d = nach[k] - von[k];
            if nach[k] > hi[k] && d > 0.0 {
                t = t.min((hi[k] - von[k]) / d);
            }
            if nach[k] < lo[k] && d < 0.0 {
                t = t.min((lo[k] - von[k]) / d);
            }
        }
        let t = t.max(0.0);
        [von[0] + t * (nach[0] - von[0]), von[1] + t * (nach[1] - von[1])]
    }
}

fn vorzeichen(x: i128) -> i32 {
    match x {
        0 => 0,
        x if x > 0 => 1,
        _ => -1,
    }
}

/// +1, wenn `c` links der Geraden `a → b` liegt (positiver Umlauf), −1 rechts,
/// 0 auf ihr.
pub(crate) fn orient(a: Ip, b: Ip, c: Ip) -> i32 {
    let (abx, aby) = (b[0] as i128 - a[0] as i128, b[1] as i128 - a[1] as i128);
    let (acx, acy) = (c[0] as i128 - a[0] as i128, c[1] as i128 - a[1] as i128);
    vorzeichen(abx * acy - aby * acx)
}

/// [`orient`] für Punkte mit dreifachen Koordinaten (Schwerpunkt eines
/// Dreiecks ganzzahlig): Beträge bis 2³¹, Produkte bis 2⁶³.
pub(crate) fn orient3(a: [i128; 2], b: [i128; 2], c: [i128; 2]) -> i32 {
    vorzeichen((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
}

/// +1, wenn `d` im Umkreis des positiv umlaufenden Dreiecks `a b c` liegt,
/// −1 außerhalb, 0 auf dem Kreis.
pub(crate) fn umkreis(a: Ip, b: Ip, c: Ip, d: Ip) -> i32 {
    let f = |p: Ip| (p[0] as i128 - d[0] as i128, p[1] as i128 - d[1] as i128);
    let ((adx, ady), (bdx, bdy), (cdx, cdy)) = (f(a), f(b), f(c));
    let al = adx * adx + ady * ady;
    let bl = bdx * bdx + bdy * bdy;
    let cl = cdx * cdx + cdy * cdy;
    vorzeichen(
        al * (bdx * cdy - bdy * cdx) + bl * (cdx * ady - cdy * adx) + cl * (adx * bdy - ady * bdx),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vorzeichen_genau() {
        assert_eq!(orient([0, 0], [10, 0], [5, 1]), 1);
        assert_eq!(orient([0, 0], [10, 0], [5, -1]), -1);
        assert_eq!(orient([0, 0], [10, 0], [20, 0]), 0);
        // Fast kollinear am Rand des Gitters: g(g−2) − (g−1)² = −1. In f64
        // gingen die Produkte (2⁵⁶) unter.
        let g = 1i64 << 28;
        assert_eq!(orient([0, 0], [g, g - 1], [g - 1, g - 2]), -1);
        assert_eq!(umkreis([0, 0], [2, 0], [0, 2], [1, 1]), 1);
        assert_eq!(umkreis([0, 0], [2, 0], [0, 2], [2, 2]), 0);
        assert_eq!(umkreis([0, 0], [2, 0], [0, 2], [3, 3]), -1);
        assert_eq!(umkreis([0, 0], [g, 0], [0, g], [g, g]), 0);
        assert_eq!(umkreis([0, 0], [g, 0], [0, g], [g, g - 1]), 1);
    }

    #[test]
    fn gitter_hin_und_zurueck() {
        let g = Gitter::fuer(&[[100.0, 200.0], [900.0, 650.0]]);
        assert_eq!(g.q, Q_MIN);
        let p = [123.456789, 345.678901];
        let z = g.aus(g.ein(p));
        assert!((z[0] - p[0]).abs() <= g.q && (z[1] - p[1]).abs() <= g.q);
        let w = g.im_bereich([500.0, 400.0], [1e9, 400.0]);
        assert!(w[0] < 1e9 && (w[1] - 400.0).abs() < 1e-9);
        let gross = Gitter::fuer(&[[0.0, 0.0], [5000.0, 10.0]]);
        assert!(gross.q > Q_MIN && gross.ein([5000.0, 10.0])[0] < 1 << 28);
    }
}
