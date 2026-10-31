//! Diagnose der Reibung in der Rutschprobe (Radical, 5,5 %).
use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{messung, CpuReferenz, Rechenweg};
fn main() {
    let r = Roehre::default();
    let mut s = r.szene(param_ruhe()).unwrap();
    let mut rw = CpuReferenz::neu();
    bis_zur_ruhe(&mut s, &mut rw, 1200, 1e-4, 10);
    let st = rw.statistik;
    println!("Ruhe ohne g: Kontakte {} haftend {} mittl. Tiefe {:.4} mm mittl. tangential {:.4} mm",
        st.kontakte, st.haftend, st.tiefe / st.kontakte as f64 * 1e3, st.tangential / st.kontakte as f64 * 1e3);
    s.param.schwerkraft = [0.0, 0.0, -9.81]; s.param.daempfung = 0.0;
    let z0 = messung::schwerpunkt(&s)[2];
    for b in 0..10 {
        rw.bild(&mut s);
        let st = rw.statistik;
        println!("Bild {b}: Kontakte {} haftend {} Tiefe {:.4} mm tangential {:.4} mm  Weg {:.3} mm",
            st.kontakte, st.haftend, st.tiefe / st.kontakte.max(1) as f64 * 1e3, st.tangential / st.kontakte.max(1) as f64 * 1e3,
            (z0 - messung::schwerpunkt(&s)[2]) * 1e3);
    }
}
