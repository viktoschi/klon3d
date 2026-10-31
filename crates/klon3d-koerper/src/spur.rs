//! Mitwandernde Messlinien.
//!
//! `rumpf()` liefert einen waagerechten Schnitt an einer **festen Höhe**.
//! Für die Anpassung ist das falsch: verschiebt ein Feld den Körper nach
//! oben oder unten, misst dieselbe Höhe plötzlich eine andere Stelle des
//! Körpers. CLO misst stattdessen eine **Landmarkenlinie**, die auf dem
//! Körper mitwandert.
//!
//! Eine [`Spur`] ist genau das: für jeden Punkt des Referenzschnitts die
//! Kante `(a, b)` und der Anteil `t`, also `p = p_a + t · (p_b − p_a)`.
//! Auf einem geformten Netz werden dieselben Kanten und Anteile neu
//! ausgewertet — die Linie bleibt dieselbe Materiallinie.

use crate::{Netz, Querschnitt};
use std::collections::HashMap;

/// Raster zum Wiederfinden der Schnittpunkte, Millimeter (wie in `rumpf`).
const RASTER: f64 = 1.0e-3;

fn raster(p: (f64, f64)) -> (i64, i64) {
    (
        (p.0 / RASTER).round() as i64,
        (p.1 / RASTER).round() as i64,
    )
}

/// Ein Punkt der Messlinie als Ort auf einer Netzkante.
#[derive(Clone, Copy, Debug)]
pub struct Kante {
    pub a: usize,
    pub b: usize,
    /// 0 = bei `a`, 1 = bei `b`.
    pub t: f64,
}

impl Kante {
    pub fn punkt(&self, netz: &Netz) -> (f64, f64) {
        let p = self.punkt3(netz);
        (p[0], p[2])
    }

    /// Der Punkt im Raum.
    pub fn punkt3(&self, netz: &Netz) -> [f64; 3] {
        let (pa, pb) = (netz.punkte[self.a], netz.punkte[self.b]);
        [
            pa[0] + self.t * (pb[0] - pa[0]),
            pa[1] + self.t * (pb[1] - pa[1]),
            pa[2] + self.t * (pb[2] - pa[2]),
        ]
    }
}

/// Eine mitwandernde Schnittlinie, in Ringreihenfolge.
#[derive(Clone, Debug)]
pub struct Spur {
    pub kanten: Vec<Kante>,
}

impl Spur {
    /// Die Punkte auf einem (beliebig geformten) Netz, in `(x, z)`.
    pub fn punkte(&self, netz: &Netz) -> Vec<(f64, f64)> {
        self.kanten.iter().map(|k| k.punkt(netz)).collect()
    }

    /// Länge der Linie in der Einheit des Netzes (Millimeter).
    pub fn laenge(&self, netz: &Netz) -> f64 {
        let p = self.punkte(netz);
        p.iter()
            .zip(p.iter().cycle().skip(1))
            .map(|(a, b)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt())
            .sum()
    }

    /// Mittlere Höhe der Linie auf einem Netz, in der Einheit des Netzes.
    pub fn mittlere_hoehe(&self, netz: &Netz) -> f64 {
        let summe: f64 = self.kanten.iter().map(|k| k.punkt3(netz)[1]).sum();
        summe / self.kanten.len() as f64
    }
}

/// Sucht zu einem Referenzschnitt die Herkunft jedes Punktes.
///
/// Alle Kanten, die die Ebene kreuzen, werden einmal indiziert; die
/// Ringpunkte werden darüber wiedergefunden. Der Ring ist entweder
/// geschlossen oder ein offener Rest, dessen Enden höchstens 2 cm
/// auseinanderliegen (Nahtschluss, Befunde 22 und 25) — in beiden Fällen bleibt
/// die Reihenfolge der Punkte dieselbe und das Schlussstück wird wie in
/// `rumpf` als Sehne gerechnet.
pub fn spur(netz: &Netz, q: &Querschnitt) -> Option<Spur> {
    let y = q.hoehe;
    let mut index: HashMap<(i64, i64), Kante> = HashMap::new();
    for d in &netz.dreiecke {
        for e in 0..3 {
            let (a, b) = (d[e], d[(e + 1) % 3]);
            let (pa, pb) = (netz.punkte[a], netz.punkte[b]);
            // Genau wie in `rumpf`: Eckpunkte auf der Ebene und Kanten,
            // die sie nicht echt kreuzen, zählen nicht.
            if pa[1] == y || pb[1] == y || (pa[1] - y) * (pb[1] - y) > 0.0 {
                continue;
            }
            let t = (y - pa[1]) / (pb[1] - pa[1]);
            let x = pa[0] + t * (pb[0] - pa[0]);
            let z = pa[2] + t * (pb[2] - pa[2]);
            index.entry(raster((x, z))).or_insert(Kante { a, b, t });
        }
    }
    let mut kanten = Vec::with_capacity(q.ring.len());
    for &(x, z) in &q.ring {
        kanten.push(*index.get(&raster((x, z)))?);
    }
    Some(Spur { kanten })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rumpf;

    /// Auf einem Zylinder ist die Spur genauso lang wie der Schnitt, und
    /// nach einer Verschiebung in x bleibt sie es (starr mitgewandert).
    #[test]
    fn spur_folgt_dem_netz() {
        let n = Netz::zylinder(100.0, 400.0, 720, 8);
        let q = rumpf(&n, 200.0).expect("Ring");
        let s = spur(&n, &q).expect("Spur");
        assert!((s.laenge(&n) - q.laenge).abs() < 1.0e-6);

        let mut m = n.clone();
        for p in &mut m.punkte {
            p[0] += 37.0;
        }
        assert!((s.laenge(&m) - q.laenge).abs() < 1.0e-6);
    }

    /// Ein expandierter Zylinder ist länger — die Spur misst die neue Form,
    /// nicht die alte.
    #[test]
    fn spur_misst_die_neue_form() {
        let n = Netz::zylinder(100.0, 400.0, 720, 8);
        let q = rumpf(&n, 200.0).expect("Ring");
        let s = spur(&n, &q).expect("Spur");
        let mut m = n.clone();
        for p in &mut m.punkte {
            p[0] *= 1.1;
            p[2] *= 1.1;
        }
        assert!((s.laenge(&m) - q.laenge * 1.1).abs() < 1.0e-6);
    }
}
