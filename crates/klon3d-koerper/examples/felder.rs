//! Wirkung der CLO-Formungsfelder, gemessen mit dem Verfahren des Projekts.
//!
//! Die Anwendung rechnet `P = Grundnetz + Summe Feld_k * c_k + Feld_letztes`.
//! Ein Maß `c_k = 1` muss dabei genau eine Einheit des zugehoerigen Masses
//! bewegen (dafuer sorgt CLOs Regler). Dieses Beispiel misst nach: wie stark
//! aendert jedes Feld den Rumpfumfang je Hoehe und die Koerperhoehe?
//!
//! Aufruf: `cargo run -p klon3d-koerper --example felder -- <ordner> [bd]`

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

/// Liest eine `ConvertingMat_*.bd`: `(Felder, Punkte, Werte[f][i][c])`.
fn lies_bd(pfad: &Path, punkte: usize) -> (usize, Vec<[f64; 3]>) {
    let b = fs::read(pfad).expect("BD lesbar");
    let felder = i32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
    let anzahl = i32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
    assert_eq!(anzahl, punkte * 3, "BD passt nicht zum Grundnetz");
    let mut v = Vec::with_capacity(felder * punkte);
    for i in 0..felder * punkte {
        let o = 8 + i * 12;
        let x = f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64;
        let y = f32::from_le_bytes(b[o + 4..o + 8].try_into().unwrap()) as f64;
        let z = f32::from_le_bytes(b[o + 8..o + 12].try_into().unwrap()) as f64;
        v.push([x, y, z]);
    }
    (felder, v)
}

fn mit_feld(netz: &Netz, feld: &[[f64; 3]], c: f64) -> Netz {
    let mut q = netz.clone();
    for (i, p) in q.punkte.iter_mut().enumerate() {
        for a in 0..3 {
            p[a] += feld[i][a] * 10.0 * c;
        }
    }
    q
}

fn spanne(netz: &Netz) -> f64 {
    let (lo, hi) = netz.hoehe().unwrap();
    (hi - lo) / 10.0
}

fn main() {
    let ordner: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female".to_string()
        })
        .into();
    let name = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "ConvertingMat_BASE_Bust_TotalHeight.bd".to_string());

    let netz = lies_obj(&ordner.join("BaseMesh.obj"));
    let punkte = netz.punkte.len();
    let (felder, vektoren) = lies_bd(&ordner.join(&name), punkte);
    let feld = |k: usize, i: usize| vektoren[k * punkte + i];

    println!("{name}: {felder} Felder, {punkte} Punkte");
    println!(
        "Grundnetz: Hoehe {:.2} cm",
        spanne(&netz)
    );

    // Hoehenwirkung: d(Spanne)/dc, zentral, Schritt 1.0
    print!("d(Hoehe)/dc:");
    for k in 0..felder {
        let f: Vec<[f64; 3]> = (0..punkte).map(|i| feld(k, i)).collect();
        let hoch = spanne(&mit_feld(&netz, &f, 1.0));
        let tief = spanne(&mit_feld(&netz, &f, -1.0));
        print!("  F{k}={:+.3}", (hoch - tief) / 2.0);
    }
    println!();

    // Umfangswirkung je Hoehe
    let mut kopf = String::from("Hoehe  Umfang");
    for k in 0..felder {
        kopf.push_str(&format!("   dU/dc F{k}"));
    }
    println!("\n{kopf}");
    for y in (78..=130).step_by(2) {
        let y_mm = y as f64 * 10.0;
        let u0 = match rumpf(&netz, y_mm) {
            Some(q) => q.laenge / 10.0,
            None => {
                println!("{y:5}  -- kein Ring --");
                continue;
            }
        };
        let mut zeile = format!("{y:5}  {u0:6.2}");
        for k in 0..felder {
            let f: Vec<[f64; 3]> = (0..punkte).map(|i| feld(k, i)).collect();
            let plus = rumpf(&mit_feld(&netz, &f, 1.0), y_mm).map(|q| q.laenge / 10.0);
            let minus = rumpf(&mit_feld(&netz, &f, -1.0), y_mm).map(|q| q.laenge / 10.0);
            match (plus, minus) {
                (Some(p), Some(m)) => zeile.push_str(&format!(" {:+10.3}", (p - m) / 2.0)),
                _ => zeile.push_str("        --"),
            }
        }
        println!("{zeile}");
    }
}
