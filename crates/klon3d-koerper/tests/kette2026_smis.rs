//! The full 2026.1 chain from a measurement file (`.smis`) — the entry point of
//! `klon3d koerper`.
//!
//! Reference values: `pruefwerte_2026_1.json` → `anwenderin_smis` (assignment
//! "klon3d sets" from `massdefinitionen.md`, HighHip derived from the own
//! BASE mesh). Checked are mesh (min/max/mean), final values and
//! point sample; tolerances per `formung_2026_1.md` 15.
//!
//! The measurement file is that of the reference sample; its values are the user's.
//! Without CLO's 2026.1 folder or without the reference file, the test is skipped.

mod common;

use common::json_mini::{ordner, pruef, J};
use klon3d_koerper::kette2026::{Ende, Kette2026, Konfiguration, KOERPER};
use klon3d_koerper::messung2026::NAMEN;
use klon3d_koerper::Punkt3;
use std::path::PathBuf;

/// The measurement file of the reference sample (the user's values).
fn smis() -> Option<klon3d_pattern::Smis> {
    let p = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../klon3d-pattern/tests/muster/measurements.smis"
    ));
    klon3d_pattern::lies_smis(&p).ok()
}

fn zahl_liste(j: &J) -> Vec<f64> {
    j.liste()
        .iter()
        .map(|x| match x {
            J::Zahl(z) => *z,
            _ => f64::NAN,
        })
        .collect()
}

fn abstand(a: Punkt3, b: Punkt3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn chain_smis_matches_the_reference_values() {
    let (Some(o), Some(p), Some(smis)) = (ordner(), pruef(), smis()) else {
        eprintln!("übersprungen: CLO-Ordner, Prüfdatei oder Maßdatei fehlt");
        return;
    };
    let k = Kette2026::laden(&o).expect("CLO-Daten");
    let konf = Konfiguration::klon3d();
    let g = k
        .aus_smis(&konf, &|n| smis.wert(n))
        .expect("Kette aus der Maßdatei");
    let a = p.feld("anwenderin_smis");

    println!(
        "BASE {} Formungen ({:?}), DETAIL {} Formungen ({:?})",
        g.base.e_folge.len(),
        g.base.grund,
        g.detail.e_folge.len(),
        g.detail.grund
    );
    assert_eq!(g.base.grund, Ende::Konvergenz, "BASE-Ende");
    assert_eq!(g.detail.grund, Ende::Konvergenz, "DETAIL-Ende");
    assert_eq!(g.base.e_folge.len(), 5, "BASE-Formungen");
    assert_eq!(g.detail.e_folge.len(), 21, "DETAIL-Formungen");

    // --- Final values -----------------------------------------------------
    let end = zahl_liste(a.feld("endwerte"));
    let mut maxd: f64 = 0.0;
    let mut wo = 0;
    for j in 0..60 {
        if j == 1 {
            continue;
        }
        let d = (g.werte()[j] - end[j]).abs() * 10.0;
        if d > maxd {
            maxd = d;
            wo = j;
        }
    }
    println!(
        "Endwerte: grösste Abweichung {maxd:.5} mm — Platz {wo} ({})",
        NAMEN[wo]
    );
    assert!(maxd <= 0.1, "{maxd:.5} mm bei Platz {wo}");

    // --- Point sample -----------------------------------------------------
    let stich = a.feld("punktstichprobe").objekt();
    let mut maxp: f64 = 0.0;
    let mut wp = 0;
    for (nr, soll) in stich {
        let i: usize = nr.parse().expect("Punktnummer");
        if i >= KOERPER {
            continue;
        }
        let s: Punkt3 = [
            soll.liste()[0].zahl(),
            soll.liste()[1].zahl(),
            soll.liste()[2].zahl(),
        ];
        let d = abstand(g.detail.punkte[i], s) * 10.0;
        if d > maxp {
            maxp = d;
            wp = i;
        }
    }
    println!("Punkte: grösste Abweichung {maxp:.5} mm (Punkt {wp})");
    assert!(maxp <= 0.05, "{maxp:.5} mm bei Punkt {wp}");

    // --- Mesh: min, max, mean of the group body --------------------------
    let koerper = &g.detail.punkte[..KOERPER];
    let feld = |f: &dyn Fn(&[Punkt3]) -> [f64; 3]| f(koerper);
    let min = feld(&|v| {
        [0, 1, 2].map(|c| v.iter().map(|p| p[c]).fold(f64::INFINITY, f64::min))
    });
    let max = feld(&|v| {
        [0, 1, 2].map(|c| v.iter().map(|p| p[c]).fold(f64::NEG_INFINITY, f64::max))
    });
    let mittel = feld(&|v| {
        let n = v.len() as f64;
        [0, 1, 2].map(|c| v.iter().map(|p| p[c]).sum::<f64>() / n)
    });
    let netz = a.feld("netz_body");
    let mut maxn: f64 = 0.0;
    for (name, ist) in [("min", min), ("max", max), ("mittel", mittel)] {
        let soll: Vec<f64> = zahl_liste(netz.feld(name));
        for c in 0..3 {
            let d = (ist[c] - soll[c]).abs() * 10.0;
            maxn = maxn.max(d);
            assert!(
                d <= 0.05,
                "Netz {name}[{c}]: {:.6} statt {:.6} ({d:.5} mm)",
                ist[c],
                soll[c]
            );
        }
    }
    println!("Netz min/max/Mittel: grösste Abweichung {maxn:.5} mm");

    // The lowest point does not sit exactly at 0 after the foot (section 12.7).
    let tief = koerper.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    assert!(tief > -0.02 && tief < 0.01, "tiefster Punkt {tief:.6} cm");
}
