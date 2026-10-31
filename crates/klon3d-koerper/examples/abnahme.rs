//! Prüfung der CLO-Formungsdaten gegen das eigene Messverfahren.
//!
//! Kein Teil des Produkts, sondern die Abnahme: CLOs `ConvertingMat_*.bd`
//! enthalten je Maß ein Verschiebungsfeld. Wenn das stimmt, dann muss eine
//! Änderung von `CIRCUMFERENCE_Bust` um 1 cm den gemessenen Brustumfang um
//! genau 1 cm ändern — gemessen mit demselben Verfahren, das später die
//! Körperformung abnimmt.
//!
//! Aufruf: `cargo run -p klon3d-koerper --example abnahme -- <ordner>`

use klon3d_koerper::{rumpf, Netz};
use std::fs;
use std::path::{Path, PathBuf};

/// Liest ein OBJ als Dreiecksnetz. CLO speichert in Zentimetern.
fn lies_obj(pfad: &Path) -> Netz {
    let text = fs::read_to_string(pfad).expect("OBJ lesbar");
    let mut netz = Netz::default();
    for zeile in text.lines() {
        let mut teile = zeile.split_whitespace();
        match teile.next() {
            Some("v") => {
                let x: f64 = teile.next().unwrap().parse().unwrap();
                let y: f64 = teile.next().unwrap().parse().unwrap();
                let z: f64 = teile.next().unwrap().parse().unwrap();
                netz.punkte.push([x * 10.0, y * 10.0, z * 10.0]);
            }
            Some("f") => {
                let mut idx = Vec::new();
                for t in teile {
                    let i: usize = t.split('/').next().unwrap().parse::<usize>().unwrap() - 1;
                    idx.push(i);
                }
                for k in 1..idx.len() - 1 {
                    netz.dreiecke.push([idx[0], idx[k], idx[k + 1]]);
                }
            }
            _ => {}
        }
    }
    netz
}

/// Liest eine `ConvertingMat_*.bd`.
fn lies_bd(pfad: &Path) -> (i32, Vec<[f64; 3]>) {
    let b = fs::read(pfad).expect("BD lesbar");
    let felder = i32::from_le_bytes(b[0..4].try_into().unwrap());
    let gesamt = i32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
    let mut v = Vec::with_capacity(gesamt);
    for i in 0..gesamt {
        let o = 8 + i * 12;
        let x = f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64;
        let y = f32::from_le_bytes(b[o + 4..o + 8].try_into().unwrap()) as f64;
        let z = f32::from_le_bytes(b[o + 8..o + 12].try_into().unwrap()) as f64;
        v.push([x, y, z]);
    }
    (felder, v)
}

fn main() {
    let ordner: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female".to_string()
        })
        .into();

    let netz = lies_obj(&ordner.join("BaseMesh.obj"));
    let (felder, vektoren) = lies_bd(&ordner.join("ConvertingMat_BASE_Bust_TotalHeight.bd"));
    let punkte = netz.punkte.len();
    println!(
        "Basisnetz: {} Punkte, {} Dreiecke",
        punkte,
        netz.dreiecke.len()
    );
    println!("Felder: {felder}, Vektoren: {}", vektoren.len());

    let feld = |k: usize, i: usize| vektoren[k * punkte + i];

    // Feldstatistik: Wie gross ist die Verschiebung je Einheit? Ohne diese
    // Zahl ist dU/dFeld nicht deutbar.
    println!("\n=== Feldstatistik ===");
    for k in 0..felder as usize {
        let (mut max, mut sum, mut radial, mut n_rad) = (0.0f64, 0.0f64, 0.0f64, 0usize);
        for i in 0..punkte {
            let v = feld(k, i);
            let m = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            max = max.max(m);
            sum += m;
            let p = netz.punkte[i];
            let r = (p[0] * p[0] + p[2] * p[2]).sqrt();
            if r > 1.0 && p[1] > 700.0 && p[1] < 1300.0 {
                radial += (v[0] * p[0] + v[2] * p[2]) / r;
                n_rad += 1;
            }
        }
        println!(
            "Feld {k}: max |v| = {max:.3} cm, mittel = {:.3} cm, mittlere Radialkomponente am Rumpf = {:.3} cm",
            sum / punkte as f64,
            if n_rad > 0 { radial / n_rad as f64 } else { 0.0 }
        );
    }

    // Höhenschnitt: Umfang und Wirkung jedes Feldes, alle 2 cm.
    println!("\nHöhe  Ringpunkte  Breite    Umfang     dU/dFeld0   dU/dFeld1");
    for y in (60..=150).step_by(2) {
        let y_mm = y as f64 * 10.0;
        let q0 = match rumpf(&netz, y_mm) {
            Some(q) => q,
            None => {
                println!("{y:5}  -- kein Ring --");
                continue;
            }
        };
        let u0 = q0.laenge / 10.0;
        let n_ring = q0.ring.len();
        let (mut x0, mut x1) = (f64::MAX, f64::MIN);
        for (x, _) in &q0.ring {
            x0 = x0.min(*x);
            x1 = x1.max(*x);
        }
        let breite = (x1 - x0) / 10.0;

        let mut d = [0.0f64; 2];
        for k in 0..2 {
            let mut q = netz.clone();
            for i in 0..punkte {
                let v = feld(k, i);
                for c in 0..3 {
                    q.punkte[i][c] += v[c] * 10.0;
                }
            }
            if let Some(s) = rumpf(&q, y_mm) {
                d[k] = s.laenge / 10.0 - u0;
            }
        }
        println!(
            "{y:5}  {n_ring:10}  {breite:7.1}  {u0:8.3}  {:10.3}  {:10.3}",
            d[0], d[1]
        );
    }
}
