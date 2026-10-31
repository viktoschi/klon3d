//! Ausgabe: Endzustand als OBJ (mm) und Dehnung je Dreieck als CSV.

use std::fmt::Write;

use crate::messung::dehnung_je_dreieck;
use crate::szene::Szene;

/// Wavefront-OBJ in mm, je Schnittteil eine Gruppe.
pub fn obj(s: &Szene) -> String {
    let mut out = String::from("# klon3d-sim, Einheit mm\n");
    for p in &s.x {
        let _ = writeln!(out, "v {:.4} {:.4} {:.4}", p[0] * 1e3, p[1] * 1e3, p[2] * 1e3);
    }
    for t in &s.teile {
        let _ = writeln!(out, "g {}", t.name.replace(' ', "_"));
        for d in &s.dreiecke[t.dreiecke.clone()] {
            let _ = writeln!(out, "f {} {} {}", d[0] + 1, d[1] + 1, d[2] + 1);
        }
    }
    out
}

/// Dehnung je Dreieck: `dreieck;teil;groesste;kleinste` (Anteile, nicht %).
pub fn dehnung_csv(s: &Szene) -> String {
    let d = dehnung_je_dreieck(s);
    let mut out = String::from("dreieck;teil;groesste;kleinste\n");
    for t in &s.teile {
        for k in t.dreiecke.clone() {
            let _ = writeln!(out, "{};{};{:.6};{:.6}", k, t.name, d[k].0, d[k].1);
        }
    }
    out
}
