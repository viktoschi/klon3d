//! Beide Rechenwege ohne Reibung: dasselbe Gleichgewicht des Federnetzes?
use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{CpuReferenz, ImplizitCg, ImplizitParameter, Rechenweg, RADICAL_RUBBER_04, RETIFIST_NR_040};
fn main() {
    for mu in [0.0, 0.03] {
        for m in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
            let r = Roehre { material: m, ..Roehre::default() };
            let wege: Vec<(&str, Box<dyn Rechenweg>)> = vec![
                ("XPBD", Box::new(CpuReferenz::neu())),
                ("implizit", Box::new(ImplizitCg::neu(ImplizitParameter { cg_max: 2000, cg_toleranz: 1e-8, ..Default::default() }))),
            ];
            for (n, mut rw) in wege {
                let mut s = r.szene(param_ruhe()).unwrap();
                for x in s.reibung.iter_mut() { *x = mu; }
                let l = bis_zur_ruhe(&mut s, rw.as_mut(), 1200, 1e-5, 10);
                let e = r.auswerten(&s);
                println!("μ {:.2} {:<28} {:<9} ruhig {} Bilder {:>4}  Δε {:+.3} pp  Median {:+.3} P95 {:+.3} max {:+.3} %  Naht {:.3} mm",
                    mu, m.name, n, l.ruhig, l.bilder, (e.dehnung - e.dehnung_soll) * 100.0, e.dehnung_median * 100.0, e.dehnung_p95 * 100.0, e.dehnung_max * 100.0, e.nahtspalt_m * 1e3);
            }
        }
    }
}
