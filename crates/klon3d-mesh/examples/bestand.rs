//! Vernetzt jedes Teil des Musterbestands (ohne Nähte) und zählt, was die
//! Abnahme verletzt.
//!
//! ```text
//! cargo run --release -p klon3d-mesh --example bestand -- [D:\ai\claude\latex_schnitte]
//! ```
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use klon3d_mesh::{pruefung, vernetze, Parameter};
use klon3d_pattern::geom::{lies_messungen, Auswertung};
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::read_path;

fn alle(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            alle(&p, out);
        } else if p.extension().is_some_and(|x| x == "sm2d") {
            out.push(p);
        }
    }
}

fn main() {
    let wurzel = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| r"D:\ai\claude\latex_schnitte".into()));
    let mut muster = Vec::new();
    alle(&wurzel, &mut muster);
    muster.sort();
    let p = Parameter::default();
    let (mut n_muster, mut n_teile, mut n_fehler, mut n_teilfehler) = (0, 0, 0, 0);
    let mut verstoesse: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut min_winkel = f64::INFINITY;
    let (mut spitze, mut teile_spitz, mut kante_min) = (0, 0, f64::INFINITY);
    let mut langsam = (0.0, String::new());
    let start = std::time::Instant::now();
    for pfad in &muster {
        let Ok(m) = read_path(pfad) else { continue };
        let Some(mess) = m.measurements.as_ref().map(|x| pfad.parent().unwrap().join(x)) else { continue };
        let Ok(mess) = lies_messungen(&mess, &m.unit) else { continue };
        let ev = Auswertung::new(&m, mess);
        n_muster += 1;
        let t0 = std::time::Instant::now();
        let erg = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| vernetze(&ev, Kurvenquelle::Seamly, &[], &p)));
        let dauer = t0.elapsed().as_secs_f64();
        if dauer > langsam.0 {
            langsam = (dauer, pfad.display().to_string());
        }
        let netz = match erg {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
                n_fehler += 1;
                verstoesse.entry("Fehler".into()).or_default().push(format!("{}: {e}", pfad.display()));
                continue;
            }
            Err(_) => {
                n_fehler += 1;
                verstoesse.entry("Absturz".into()).or_default().push(pfad.display().to_string());
                continue;
            }
        };
        n_teile += netz.teile.len();
        n_teilfehler += netz.fehler.len();
        let b = pruefung::pruefe(&netz);
        for t in &b.teile {
            min_winkel = min_winkel.min(t.min_winkel);
            spitze += t.spitze_ecken;
            teile_spitz += (t.spitze_ecken > 0) as usize;
            kante_min = kante_min.min(t.kante_min);
        }
        if netz.offene_teilungen > 0 {
            verstoesse.entry("offene Teilungen".into()).or_default().push(pfad.display().to_string());
        }
        for v in b.verstoesse(&p) {
            let art = if v.contains("Dreiecke unter") {
                "Winkel"
            } else if v.contains("Sehnenabstand") {
                "Sehne"
            } else if v.contains("zu kurz/lang") {
                "Länge"
            } else {
                "Struktur"
            };
            verstoesse.entry(art.into()).or_default().push(format!("{}: {v}", pfad.file_name().unwrap().to_string_lossy()));
        }
    }
    println!(
        "{n_muster} Muster, {n_teile} Teile vernetzt, {n_teilfehler} Teile nicht auflösbar, {n_fehler} Muster mit Fehler; {:.1} s",
        start.elapsed().as_secs_f64()
    );
    println!("kleinster Winkel {min_winkel:.2}°, langsamstes Muster {:.2} s {}", langsam.0, langsam.1);
    println!("{spitze} Dreiecke in spitzen Teilecken ({teile_spitz} Teile), kürzeste Kante {kante_min:.3} mm");
    for (art, liste) in &verstoesse {
        println!("{art}: {}", liste.len());
        for x in liste.iter().take(12) {
            println!("  {x}");
        }
    }
}
