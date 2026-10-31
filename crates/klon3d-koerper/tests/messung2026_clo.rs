//! The 2026.1 measurement against CLO's stored meshes.
//!
//! Reference values: `pruefwerte_2026_1.json`, section `clo_netze` — per mesh the
//! 60 measured values from CLO's store and the points of the mesh
//! (`netze_2026_1/*.bin`, 47 650 × 3 `f32` cm). Tolerance per
//! `formung_2026_1.md` 15: 0.05 mm per measure; 0.0004 mm is reached because the
//! measurement computes in `f32` like CLO itself — the test demands 0.01 mm so that
//! a fallback to `f64` (0.08 mm at neck and upper arm) shows up.
//!
//! Without the CLO folder or without the reference file, the test is skipped.

mod common;

use common::json_mini::{ordner, pruef};
use klon3d_koerper::kette2026::PUNKTE;
use klon3d_koerper::massband::Messdaten;
use klon3d_koerper::messung2026::{gemessen, Messer, ANZAHL, NAMEN};
use klon3d_koerper::Punkt3;

/// A CLO mesh from the reference file: `47 650 × 3` float32 little endian, cm.
fn netz(datei: &str) -> std::io::Result<Vec<Punkt3>> {
    let b = std::fs::read(datei)?;
    let erwartet = PUNKTE * 3 * 4;
    assert_eq!(b.len(), erwartet, "{datei}: {} Byte", b.len());
    Ok(b.chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()) as f64)
        .collect::<Vec<f64>>()
        .chunks_exact(3)
        .map(|p| [p[0], p[1], p[2]])
        .collect())
}

/// All three meshes match CLO's measured values to 0.05 mm.
#[test]
fn measurement_matches_clo_meshes() {
    let (Some(o), Some(p)) = (ordner(), pruef()) else {
        eprintln!("übersprungen: CLO-Ordner oder Prüfdatei fehlt");
        return;
    };
    let d = Messdaten::laden(&o).expect("Messdaten");
    let spez = common::json_mini::spez();

    let mut gesamt: f64 = 0.0;
    for name in ["clo_gui_basic", "clo_avs_base", "clo_avs_detail"] {
        let eintrag = p.feld("clo_netze").feld(name);
        let datei = match eintrag.feld("datei") {
            common::json_mini::J::Text(t) => t.clone(),
            j => panic!("datei ist kein Text: {j:?}"),
        };
        let v = netz(&spez.join(&datei).display().to_string()).expect("Netz");
        assert_eq!(v.len(), PUNKTE, "{name}");

        let mut ms = Messer::neu(&v, &d).expect("Messer");
        let m = ms.alle();

        // CLO's values are stored as a list of 67 numbers (weight 50, sliders empty).
        let clo = eintrag.feld("clo").liste();
        let mut maxd: f64 = 0.0;
        let mut wo = String::new();
        let mut n = 0;
        for j in 0..ANZAHL {
            if !gemessen(j) {
                continue;
            }
            let soll = clo[j].zahl();
            let ist = m.werte[j];
            let dmm = (ist - soll).abs() * 10.0;
            n += 1;
            if dmm > maxd {
                maxd = dmm;
                wo = format!("{} ({j}): {ist:.6} statt {soll:.6}", NAMEN[j]);
            }
        }
        println!("{name}: {n} Maße, grösste Abweichung {maxd:.5} mm — {wo}");
        if std::env::var_os("KLON3D_ALLE").is_some() {
            for j in 0..ANZAHL {
                if gemessen(j) {
                    let d = (m.werte[j] - clo[j].zahl()) * 10.0;
                    if d.abs() > 0.001 {
                        println!("   {j:2} {:28} {d:+.5} mm", NAMEN[j]);
                    }
                }
            }
        }
        assert!(maxd <= 0.01, "{name}: {maxd:.5} mm bei {wo}");
        gesamt = gesamt.max(maxd);
    }
    println!("grösste Abweichung über alle Netze: {gesamt:.5} mm");
}
