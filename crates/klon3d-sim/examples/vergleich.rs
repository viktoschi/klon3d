//! Vergleich XPBD-Referenz gegen impliziten Schritt mit CG (wie CLO) an der
//! Röhre. Aufruf: `cargo run --release -p klon3d-sim --example vergleich [kante_mm]`
use std::time::Instant;
use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{CpuReferenz, ImplizitCg, ImplizitParameter, Rechenweg, RADICAL_RUBBER_04, RETIFIST_NR_040};

fn zeile(name: &str, r: &Roehre, rw: &mut dyn Rechenweg, stat: &dyn Fn(&dyn std::any::Any) -> String, rw_any: bool) {
    let _ = (stat, rw_any);
    let mut s = r.szene(param_ruhe()).unwrap();
    let t0 = Instant::now();
    let l = bis_zur_ruhe(&mut s, rw, 1200, 1e-4, 10);
    let dt = t0.elapsed().as_secs_f64();
    let e = r.auswerten(&s);
    println!("  {:<26} ruhig {:<5} Bilder {:>4}  {:>7.2} ms/Bild  ε {:+.4} % (Δ {:+.3} pp)  Median {:+.3} P95 {:+.3} max {:+.3} %  Naht {:.3} mm  abheben {:+.3} mm",
        name, l.ruhig, l.bilder, dt / l.bilder as f64 * 1e3, e.dehnung * 100.0, (e.dehnung - e.dehnung_soll) * 100.0,
        e.dehnung_median * 100.0, e.dehnung_p95 * 100.0, e.dehnung_max * 100.0, e.nahtspalt_m * 1e3, e.abheben_m * 1e3);
}

fn main() {
    let kante: f64 = std::env::args().skip(1).find_map(|a| a.parse().ok()).unwrap_or(10.0);
    let varianten = [
        ("implizit wie CLO (50 CG)", ImplizitParameter::default()),
        ("implizit 50 CG, Naht 3e4", ImplizitParameter { naht_k: 3e4, ..Default::default() }),
        ("implizit konvergiert", ImplizitParameter { cg_max: 3000, cg_toleranz: 1e-6, ..Default::default() }),
    ];
    for m in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        for red in [0.0, 0.055, 0.10] {
            let r = Roehre { material: m, reduktion: red, kante_mm: kante, ..Roehre::default() };
            println!("{} r {:.1} %", m.name, red * 100.0);
            zeile("XPBD weich", &r, &mut CpuReferenz::neu(), &|_| String::new(), false);
            for (n, p) in varianten {
                let mut rw = ImplizitCg::neu(p);
                let mut s = r.szene(param_ruhe()).unwrap();
                let t0 = Instant::now();
                let l = bis_zur_ruhe(&mut s, &mut rw, 1200, 1e-4, 10);
                let dt = t0.elapsed().as_secs_f64();
                let e = r.auswerten(&s);
                let st = rw.statistik;
                println!("  {:<26} ruhig {:<5} Bilder {:>4}  {:>7.2} ms/Bild  ε {:+.4} % (Δ {:+.3} pp)  Median {:+.3} P95 {:+.3} max {:+.3} %  Naht {:.3} mm  abheben {:+.3} mm  CG {}/{} (nicht konv. {})",
                    n, l.ruhig, l.bilder, dt / l.bilder as f64 * 1e3, e.dehnung * 100.0, (e.dehnung - e.dehnung_soll) * 100.0,
                    e.dehnung_median * 100.0, e.dehnung_p95 * 100.0, e.dehnung_max * 100.0, e.nahtspalt_m * 1e3, e.abheben_m * 1e3,
                    st.cg_iterationen, st.loesungen, st.nicht_konvergiert);
            }
        }
    }
    println!("\nRutschprobe (0,5 s):");
    for m in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        for red in [0.0, 0.055, 0.10] {
            let r = Roehre { material: m, reduktion: red, kante_mm: kante, ..Roehre::default() };
            let px = r.rutschprobe(&mut CpuReferenz::neu(), 0.5).unwrap();
            let mut ri = ImplizitCg::neu(ImplizitParameter::default());
            let pi = r.rutschprobe(&mut ri, 0.5).unwrap();
            // diskret: Rückwärts-Euler legt Σ k·a·h² = a·h²·N(N+1)/2 zurück
            let h = 1.0 / 60.0; let nb = (0.5f64 / h).round();
            let fak = h * h * nb * (nb + 1.0) / 2.0 / (0.5 * 0.5 * 0.5);
            println!("  {:<28} r {:>4.1} %  q {:.3}/{:.3}  XPBD {:>8.2} mm (Soll {:>8.2})  implizit {:>8.2} mm (Soll {:>8.2}, diskret {:>8.2}; q {:.3})",
                m.name, red * 100.0, px.halteverhaeltnis, px.halteverhaeltnis_gemessen, px.weg_m * 1e3, px.weg_soll_m * 1e3,
                pi.weg_m * 1e3, pi.weg_soll_m * 1e3, pi.weg_soll_m * fak * 1e3, pi.halteverhaeltnis_gemessen);
        }
    }
}
