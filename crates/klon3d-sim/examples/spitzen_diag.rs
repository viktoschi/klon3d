//! Wo liegen die Dreiecke mit der größten Dehnung? (Radical, 5,5 %)
use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{messung, CpuReferenz};
fn main() {
    let r = Roehre::default();
    let mut s = r.szene(param_ruhe()).unwrap();
    let mut rw = CpuReferenz::neu();
    bis_zur_ruhe(&mut s, &mut rw, 1200, 1e-4, 10);
    let d = messung::dehnung_je_dreieck(&s);
    let mut idx: Vec<usize> = (0..d.len()).collect();
    idx.sort_by(|a, b| d[*b].0.total_cmp(&d[*a].0));
    let h = r.hoehe_m;
    let rand = |k: usize| { let z = s.dreiecke[k].iter().map(|&i| s.x[i as usize][2]).sum::<f64>() / 3.0; (-z).min(h + z) };
    for &k in idx.iter().take(12) {
        let tri = s.dreiecke[k];
        let w = tri.iter().map(|&i| s.x[i as usize][1].atan2(s.x[i as usize][0]).to_degrees()).sum::<f64>() / 3.0;
        println!("ε {:+.2} %  Abstand zum freien Rand {:.1} mm  Winkel {:+.0}°", d[k].0 * 100.0, rand(k) * 1e3, w);
    }
    let innen: Vec<f64> = (0..d.len()).filter(|&k| rand(k) > 0.015).map(|k| d[k].0).collect();
    let mut i2 = innen.clone(); i2.sort_by(f64::total_cmp);
    println!("Innen (> 15 mm vom Rand): {} Dreiecke, Median {:+.3} %, P95 {:+.3} %, max {:+.3} %", i2.len(), i2[i2.len()/2]*100.0, i2[(i2.len() as f64*0.95) as usize]*100.0, i2[i2.len()-1]*100.0);
}
