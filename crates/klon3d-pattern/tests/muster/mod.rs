//! Woche Muster und Sollwerte liegen.
//!
//! Musterdateien stammen aus dem Musterbestand des Nutzers, die Sollwerte aus
//! den Abwärfen von `schnitt.py`. Beides wird nur gelesen.

// Jede Testdatei bindet das Modul ein und braucht nur einen Teil davon.
#![allow(dead_code, unused_imports)]

use std::path::PathBuf;

/// Verzeichnis der Prüfmuster.
pub fn wurzel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("muster")
}

pub fn muster_pfad(name: &str) -> PathBuf {
    wurzel().join(format!("{name}.sm2d"))
}

/// Die Massdatei liegt bei allen Prüfmustern daneben.
pub fn mess_pfad() -> PathBuf {
    wurzel().join("measurements.smis")
}

// Sollwerte — erzeugt, nicht von Hand gepflegt.
#[path = "sollwerte.rs"]
mod werte;
pub use werte::{BLUSE2_V26, BLUSE2_V29, BLUSE3_V1, BUSTIER_V1};