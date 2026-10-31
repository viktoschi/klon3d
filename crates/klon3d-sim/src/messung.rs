//! Messungen an der Szene: Umfang in einer Schnittebene, Dehnung je Dreieck,
//! Nahtspalt und Nahtabstände je Naht, Bewegungsenergie.

use crate::koerper::{add, dot, mul, norm, sub};
use crate::szene::Szene;

/// Länge des Schnitts der Netzfläche mit der Ebene {x : (x − punkt)·n = 0}.
/// Je Dreieck, das die Ebene kreuzt, zählt die Strecke zwischen den beiden
/// Schnittpunkten seiner Kanten.
pub fn umfang_in_ebene(s: &Szene, punkt: [f64; 3], normale: [f64; 3]) -> f64 {
    let n = mul(normale, 1.0 / norm(normale));
    let hoehe = |p: [f64; 3]| dot(sub(p, punkt), n);
    let mut summe = 0.0;
    for d in &s.dreiecke {
        let p = d.map(|i| s.x[i as usize]);
        let h = p.map(hoehe);
        let mut schnitt = Vec::with_capacity(2);
        for k in 0..3 {
            let (a, b) = (k, (k + 1) % 3);
            if (h[a] < 0.0) != (h[b] < 0.0) {
                let t = h[a] / (h[a] - h[b]);
                schnitt.push(add(p[a], mul(sub(p[b], p[a]), t)));
            }
        }
        if let [a, b] = schnitt[..] {
            summe += norm(sub(b, a));
        }
    }
    summe
}

/// Hauptdehnungen je Dreieck gegen die reduzierte Ruhelage: (größte,
/// kleinste) Streckung − 1. Aus F = [x₁−x₀, x₂−x₀]·D⁻¹ und den Eigenwerten
/// von FᵀF.
pub fn dehnung_je_dreieck(s: &Szene) -> Vec<(f64, f64)> {
    s.dreiecke
        .iter()
        .zip(&s.ruhe_inv)
        .map(|(d, r)| {
            let p = d.map(|i| s.x[i as usize]);
            let (e1, e2) = (sub(p[1], p[0]), sub(p[2], p[0]));
            // F = [e1 e2] · [[r0 r1],[r2 r3]]
            let f1 = add(mul(e1, r[0]), mul(e2, r[2]));
            let f2 = add(mul(e1, r[1]), mul(e2, r[3]));
            let (a, b, c) = (dot(f1, f1), dot(f1, f2), dot(f2, f2));
            let mitte = 0.5 * (a + c);
            let rad = (0.25 * (a - c).powi(2) + b * b).sqrt();
            ((mitte + rad).sqrt() - 1.0, (mitte - rad).max(0.0).sqrt() - 1.0)
        })
        .collect()
}

/// Größter Abstand eines Nahtpaars (m).
pub fn nahtspalt(s: &Szene) -> f64 {
    s.naht
        .punkte
        .iter()
        .map(|p| norm(sub(s.x[p[0] as usize], s.x[p[1] as usize])))
        .fold(0.0, f64::max)
}

/// Abstand der Punktpaare je Naht des Schnittnetzes `netz` (aus dem die
/// Szene gebaut ist), in Nährichtung (m).
pub fn nahtabstaende(s: &Szene, netz: &klon3d_mesh::Musternetz) -> Vec<(String, Vec<f64>)> {
    netz.naehte
        .iter()
        .map(|n| {
            let d = n
                .paare()
                .map(|(tn, pp)| {
                    let i = s.teile[tn.teil_a].punkte.start + pp.a as usize;
                    let j = s.teile[tn.teil_b].punkte.start + pp.b as usize;
                    norm(sub(s.x[i], s.x[j]))
                })
                .collect();
            (n.kennung.clone(), d)
        })
        .collect()
}

/// Bewegungsenergie (J).
pub fn bewegungsenergie(s: &Szene) -> f64 {
    s.v.iter().zip(&s.masse).map(|(v, m)| 0.5 * m * dot(*v, *v)).sum()
}

/// Massenmittelpunkt (m).
pub fn schwerpunkt(s: &Szene) -> [f64; 3] {
    let m = s.gesamtmasse();
    let mut q = [0.0; 3];
    for (x, mi) in s.x.iter().zip(&s.masse) {
        q = add(q, mul(*x, *mi));
    }
    mul(q, 1.0 / m)
}

/// Wie weit jedes Teil gegenüber der Lage `vorher` gesunken ist: Mittel der
/// Höhenänderung seiner Punkte (m, nach unten positiv), das stärkste zuerst.
/// Prüft „hält" nach ADR-023.
pub fn sinken_je_teil(s: &Szene, vorher: &[[f64; 3]]) -> Vec<(String, f64)> {
    let mut v: Vec<(String, f64)> = s
        .teile
        .iter()
        .map(|t| {
            let r = t.punkte.clone();
            let n = r.len().max(1) as f64;
            (t.name.clone(), r.map(|i| vorher[i][2] - s.x[i][2]).sum::<f64>() / n)
        })
        .collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

/// Mittlere Geschwindigkeit (Wurzel aus 2·E/m, m/s) — das Ruhekriterium.
pub fn mittlere_geschwindigkeit(s: &Szene) -> f64 {
    (2.0 * bewegungsenergie(s) / s.gesamtmasse()).sqrt()
}
