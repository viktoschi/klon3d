//! Ausgangswerte für Schritt 4b: Szene der Bluse3 v2 (flach, ohne Körper),
//! Unterschritte nach der Weichheitsbedingung und Zeit je Unterschritt.
use std::time::Instant;
use klon3d_mesh::{vernetze, Parameter};
use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::naehte::{lies_nahtliste, pruefe_naehte};
use klon3d_sim::{CpuReferenz, Rechenweg, Simparameter, Szene, Teilvorgabe, RADICAL_RUBBER_04, RETIFIST_NR_040, WEICH_FAKTOR};
fn main() {
    let wurzel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../klon3d-pattern/tests/muster");
    let m = klon3d_pattern::read_path(&wurzel.join("bluse3_v2.sm2d")).unwrap();
    let ev = Auswertung::lade(&m, &wurzel.join("measurements.smis")).unwrap();
    let defs = lies_nahtliste(&std::fs::read_to_string(wurzel.join("bluse3_v2.naehte")).unwrap()).unwrap();
    let b = pruefe_naehte(&ev, &defs, Kurvenquelle::Seamly).unwrap();
    let netz = vernetze(&ev, Kurvenquelle::Seamly, &b.naehte, &Parameter::default()).unwrap();
    for mat in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        let vg = Teilvorgabe { material: mat, reduktion: 0.055 };
        let mut s = Szene::aus_netz(&netz, &|_| vg, &|ti, p| [p[0] * 1e-3, p[1] * 1e-3, ti as f64 * 0.01], vec![],
            Simparameter { schwerkraft: [0.0; 3], ..Simparameter::default() });
        let n = s.unterschritte_weich(WEICH_FAKTOR);
        // Verteilung: nötige Unterschritte je Kante, und wo die schlimmsten liegen.
        let mut je: Vec<(f64, usize, f64, f64)> = (0..s.abstand.len()).map(|c| {
            let [i, j] = s.abstand.punkte[c].map(|p| p as usize);
            let hmax = (s.abstand.nachgiebigkeit[c] / (WEICH_FAKTOR * (s.w[i] + s.w[j]))).sqrt();
            (s.param.bild_dt / hmax, c, s.masse[i].min(s.masse[j]), s.abstand.ruhe[c])
        }).collect();
        je.sort_by(|a, b| b.0.total_cmp(&a.0));
        let q = |a: f64| je[((je.len() - 1) as f64 * a) as usize].0;
        println!("  nötige Unterschritte je Kante: max {:.0}, 99,9 % {:.0}, 99 % {:.0}, 95 % {:.0}, Median {:.0}", je[0].0, q(0.001), q(0.01), q(0.05), q(0.5));
        for &(n, c, m, l) in je.iter().take(5) {
            let [i, j] = s.abstand.punkte[c];
            let teil = s.teile.iter().find(|t| t.punkte.contains(&(i as usize))).map(|t| t.name.clone()).unwrap_or_default();
            let rand = |k: u32| { let t = s.teile.iter().find(|t| t.punkte.contains(&(k as usize))).unwrap(); (k as usize - t.punkte.start) < netz.teile.iter().find(|n| n.teil == t.name).unwrap().nrand };
            println!("    {:.0} Unterschritte: Kante {:.2} mm, kleinere Masse {:.3} mg, Randpunkte {}/{} ({})", n, l * 1e3, m * 1e6, rand(i), rand(j), teil);
        }
        let mut massen = s.masse.clone(); massen.sort_by(f64::total_cmp);
        println!("  Punktmassen: min {:.3} mg, 1 % {:.3} mg, Median {:.3} mg", massen[0]*1e6, massen[massen.len()/100]*1e6, massen[massen.len()/2]*1e6);
        let mut rw = CpuReferenz::neu();
        // 200 Unterschritte der Länge h als ein Bild.
        let h = s.param.bild_dt / n as f64;
        s.param.unterschritte = 200;
        s.param.bild_dt = 200.0 * h;
        let t0 = Instant::now();
        rw.bild(&mut s);
        let us = t0.elapsed().as_secs_f64() / 200.0 * 1e6;
        println!("{:<28} {} Punkte, {} Kanten, {} Nahtpaare, {} Biegekanten; Farben {}/{}/{}; Unterschritte je Bild (60/s) {}; CPU {:.0} µs je Unterschritt → {:.1} Bilder/s",
            mat.name, s.x.len(), s.abstand.len(), s.naht.len(), s.biegung.len(),
            s.abstand.farben.len(), s.naht.farben.len(), s.biegung.farben.len(), n, us, 1e6 / (us * n as f64));
    }
}
