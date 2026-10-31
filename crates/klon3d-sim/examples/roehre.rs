//! Alle Röhren des Prüffalls (2 Stoffe × 3 Reduktionen) mit Messwerten,
//! dazu die Rutschprobe. Aufruf: `cargo run --release -p klon3d-sim --example roehre [kante_mm]`

use std::time::Instant;

use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{CpuReferenz, RADICAL_RUBBER_04, RETIFIST_NR_040};

fn main() {
    let kante: f64 = std::env::args().skip(1).find_map(|a| a.parse().ok()).unwrap_or(10.0);
    for material in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        for reduktion in [0.0, 0.055, 0.10] {
            let spalt = std::env::args().any(|a| a == "--spalt");
            let r = Roehre { material, reduktion, kante_mm: kante, anfang_mit_spalt: spalt, ..Roehre::default() };
            // --steif: 20 Unterschritte wie vor der Weichheitsbedingung.
            let mut s = if std::env::args().any(|a| a == "--steif") {
                r.szene_roh(param_ruhe()).unwrap()
            } else {
                r.szene(param_ruhe()).unwrap()
            };
            let mut rw = CpuReferenz::neu();
            let t0 = Instant::now();
            let lauf = bis_zur_ruhe(&mut s, &mut rw, 1200, 1e-4, 10);
            let dt = t0.elapsed().as_secs_f64();
            let e = r.auswerten(&s);
            let us = dt / (lauf.bilder * s.param.unterschritte) as f64 * 1e6;
            println!(
                "{:<28} r {:>4.1} %  {:>5} Pkt  n {:>4}  Bilder {:>4} ruhig {}  Umfang {:.5} m  ε {:+.4} % (Soll {:+.4} %, Δ {:+.3} pp)  ε Dreiecke Median {:+.3} P95 {:+.3} max {:+.3} %  Naht {:.4} mm  abheben {:+.3} mm  {:.0} µs/Unterschritt",
                material.name,
                reduktion * 100.0,
                e.punkte,
                s.param.unterschritte,
                lauf.bilder,
                lauf.ruhig,
                e.umfang_m,
                e.dehnung * 100.0,
                e.dehnung_soll * 100.0,
                (e.dehnung - e.dehnung_soll) * 100.0,
                e.dehnung_median * 100.0,
                e.dehnung_p95 * 100.0,
                e.dehnung_max * 100.0,
                e.nahtspalt_m * 1e3,
                e.abheben_m * 1e3,
                us
            );
        }
    }
    if std::env::args().any(|a| a == "--ohne-rutsch") {
        return;
    }
    println!();
    for material in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        for reduktion in [0.0, 0.055, 0.10] {
            let r = Roehre { material, reduktion, kante_mm: kante, ..Roehre::default() };
            let mut rw = CpuReferenz::neu();
            let p = r.rutschprobe(&mut rw, 0.5).unwrap();
            println!(
                "Rutschprobe {:<28} r {:>4.1} %  Halteverhältnis {:>6.3} (gemessen {:>6.3})  Weg {:>8.2} mm in {:.2} s (Soll {:>8.2} mm)",
                material.name,
                reduktion * 100.0,
                p.halteverhaeltnis,
                p.halteverhaeltnis_gemessen,
                p.weg_m * 1e3,
                p.sekunden,
                p.weg_soll_m * 1e3
            );
        }
    }
}
