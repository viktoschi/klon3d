//! Woher kommt die Instabilität des impliziten Wegs ohne Reibung?
use klon3d_sim::roehre::{param_ruhe, Roehre};
use klon3d_sim::{messung, ImplizitCg, ImplizitParameter, Rechenweg};
fn main() {
    let mu: f64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(0.0);
    let r = Roehre::default();
    let mut s = r.szene(param_ruhe()).unwrap();
    for x in s.reibung.iter_mut() { *x = mu; }
    let mut rw = ImplizitCg::neu(ImplizitParameter { cg_max: 2000, cg_toleranz: 1e-8, ..Default::default() });
    for b in 0..60 {
        rw.bild(&mut s);
        let vmax = s.v.iter().map(|v| (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt()).fold(0.0, f64::max);
        let e = r.auswerten(&s);
        let st = rw.statistik;
        // Drehung um die Achse: mittlere tangentiale Geschwindigkeit
        let rot: f64 = s.x.iter().zip(&s.v).map(|(x, v)| { let rr = (x[0]*x[0]+x[1]*x[1]).sqrt(); (x[0]*v[1]-x[1]*v[0]) / rr }).sum::<f64>() / s.x.len() as f64;
        println!("Bild {b:>2}: v_max {:.4} m/s  v_mittel {:.5}  Drehung {:+.5} m/s  ε {:+.3} %  P95 {:+.3} %  Kontakte {} frei {} CG {} nicht konv. {} Rest {:.1e}",
            vmax, messung::mittlere_geschwindigkeit(&s), rot, e.dehnung*100.0, e.dehnung_p95*100.0, st.kontakte, st.freigegeben, st.cg_iterationen, st.nicht_konvergiert, st.rest_max);
        if !vmax.is_finite() { break; }
    }
}
