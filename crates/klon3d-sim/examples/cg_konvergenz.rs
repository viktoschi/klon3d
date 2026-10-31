//! Wie viele CG-Iterationen braucht der implizite Schritt? Aufruf:
//! `--example cg_konvergenz <cg_max> <toleranz> <naht_k> <schritte_je_bild>`
use std::time::Instant;
use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{ImplizitCg, ImplizitParameter, RADICAL_RUBBER_04, RETIFIST_NR_040};
fn main() {
    let a: Vec<f64> = std::env::args().skip(1).filter_map(|x| x.parse().ok()).collect();
    let p = ImplizitParameter { cg_max: a[0] as usize, cg_toleranz: a[1], naht_k: a[2], schritte_je_bild: a[3] as usize, normalkraft_aus_last: a.get(4).map_or(false, |&x| x > 0.0), ..Default::default() };
    for (m, red) in [(RADICAL_RUBBER_04, 0.055), (RADICAL_RUBBER_04, 0.10), (RETIFIST_NR_040, 0.055)] {
        let r = Roehre { material: m, reduktion: red, ..Roehre::default() };
        let mut rw = ImplizitCg::neu(p);
        let mut s = r.szene(param_ruhe()).unwrap();
        let t0 = Instant::now();
        let l = bis_zur_ruhe(&mut s, &mut rw, 600, 1e-4, 10);
        let dt = t0.elapsed().as_secs_f64();
        let e = r.auswerten(&s);
        let st = rw.statistik;
        println!("{:<28} {:>4.1} %  ruhig {:<5} Bilder {:>4} {:>7.1} ms/Bild  Δε {:+.3} pp  Median {:+.3} P95 {:+.3} max {:+.3} %  Naht {:.3}  abheben {:+.3}  CG/Lösung {:.0} (nicht konv. {}/{})",
            m.name, red * 100.0, l.ruhig, l.bilder, dt / l.bilder as f64 * 1e3, (e.dehnung - e.dehnung_soll) * 100.0,
            e.dehnung_median * 100.0, e.dehnung_p95 * 100.0, e.dehnung_max * 100.0, e.nahtspalt_m * 1e3, e.abheben_m * 1e3,
            st.cg_iterationen as f64 / st.loesungen.max(1) as f64, st.nicht_konvergiert, st.loesungen);
    }
    for (m, red) in [(RADICAL_RUBBER_04, 0.0), (RADICAL_RUBBER_04, 0.055), (RETIFIST_NR_040, 0.055)] {
        let r = Roehre { material: m, reduktion: red, ..Roehre::default() };
        let mut rw = ImplizitCg::neu(p);
        let pr = r.rutschprobe(&mut rw, 0.5).unwrap();
        let h = 1.0 / 60.0 / p.schritte_je_bild as f64; let nb = (0.5f64 / h).round();
        let fak = h * h * nb * (nb + 1.0) / 2.0 / (0.5 * 0.5 * 0.5);
        println!("Rutsch {:<28} {:>4.1} %  Weg {:>8.2} mm  Soll {:>8.2} (diskret {:>8.2})  q {:.3}  CG/Lösung {:.0} (nicht konv. {}/{}) Kontakte {} haftend {}",
            m.name, red * 100.0, pr.weg_m * 1e3, pr.weg_soll_m * 1e3, pr.weg_soll_m * fak * 1e3, pr.halteverhaeltnis_gemessen,
            rw.statistik.cg_iterationen as f64 / rw.statistik.loesungen.max(1) as f64, rw.statistik.nicht_konvergiert, rw.statistik.loesungen,
            rw.statistik.kontakte, rw.statistik.haftend);
    }
}
