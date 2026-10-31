//! Dehnung je Dreieck in Umfangs- und Längsrichtung (Rohrachse).
use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{CpuReferenz, RADICAL_RUBBER_04, RETIFIST_NR_040};
fn main() {
    for m in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        let r = Roehre { material: m, ..Roehre::default() };
        let mut s = r.szene(param_ruhe()).unwrap();
        let mut rw = CpuReferenz::neu();
        bis_zur_ruhe(&mut s, &mut rw, 1200, 1e-4, 10);
        let (mut hoop, mut axial, mut scher, mut n) = (Vec::new(), Vec::new(), Vec::new(), 0usize);
        for (d, ri) in s.dreiecke.iter().zip(&s.ruhe_inv) {
            let p = d.map(|i| s.x[i as usize]);
            let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
            let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
            let f1: Vec<f64> = (0..3).map(|k| e1[k]*ri[0] + e2[k]*ri[2]).collect(); // F·e_x (Umfang)
            let f2: Vec<f64> = (0..3).map(|k| e1[k]*ri[1] + e2[k]*ri[3]).collect(); // F·e_y (Achse)
            let l1 = f1.iter().map(|v| v*v).sum::<f64>().sqrt();
            let l2 = f2.iter().map(|v| v*v).sum::<f64>().sqrt();
            let c = f1.iter().zip(&f2).map(|(a,b)| a*b).sum::<f64>() / (l1*l2);
            hoop.push(l1-1.0); axial.push(l2-1.0); scher.push(c.acos().to_degrees()-90.0); n += 1;
        }
        let mittel = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
        let mut hs = hoop.clone(); hs.sort_by(|a,b| a.partial_cmp(b).unwrap());
        println!("{:<28} n {n}  Umfang Mittel {:+.3} %  Median {:+.3} %  min {:+.3} max {:+.3}  | Achse Mittel {:+.3} %  | Scherwinkel Mittel {:+.2}° |Mittel| {:.2}°",
            m.name, mittel(&hoop)*100.0, hs[hs.len()/2]*100.0, hs[0]*100.0, hs[hs.len()-1]*100.0, mittel(&axial)*100.0,
            mittel(&scher), scher.iter().map(|x| x.abs()).sum::<f64>()/scher.len() as f64);
    }
}
