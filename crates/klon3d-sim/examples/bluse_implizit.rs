//! Impliziter Schritt an der Bluse3 v2 (flach ausgelegt, Nähte ziehen die
//! Teile zusammen, Schwerkraft): CG-Iterationen je Lösung und Zeit je Bild —
//! die Hochrechnung für die GPU. Zum Vergleich die XPBD-Unterschritte.
use std::time::Instant;
use klon3d_mesh::{vernetze, Parameter};
use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::naehte::{lies_nahtliste, pruefe_naehte};
use klon3d_sim::{ImplizitCg, ImplizitParameter, Rechenweg, Simparameter, Szene, Teilvorgabe, RADICAL_RUBBER_04, RETIFIST_NR_040, WEICH_FAKTOR};
fn main() {
    let wurzel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../klon3d-pattern/tests/muster");
    let m = klon3d_pattern::read_path(&wurzel.join("bluse3_v2.sm2d")).unwrap();
    let ev = Auswertung::lade(&m, &wurzel.join("measurements.smis")).unwrap();
    let defs = lies_nahtliste(&std::fs::read_to_string(wurzel.join("bluse3_v2.naehte")).unwrap()).unwrap();
    let b = pruefe_naehte(&ev, &defs, Kurvenquelle::Seamly).unwrap();
    let netz = vernetze(&ev, Kurvenquelle::Seamly, &b.naehte, &Parameter::default()).unwrap();
    for mat in [RADICAL_RUBBER_04, RETIFIST_NR_040] {
        for (cgmax, tol) in [(50usize, 1e-5), (200, 1e-5), (3000, 1e-6)] {
            let vg = Teilvorgabe { material: mat, reduktion: 0.0 };
            let mut s = Szene::aus_netz(&netz, &|_| vg, &|ti, p| [p[0] * 1e-3, p[1] * 1e-3, ti as f64 * 0.01], vec![], Simparameter::default());
            let n_xpbd = s.unterschritte_weich(WEICH_FAKTOR);
            let mut rw = ImplizitCg::neu(ImplizitParameter { cg_max: cgmax, cg_toleranz: tol, ..Default::default() });
            let (mut it, mut los, mut nk, mut t) = (0usize, 0usize, 0usize, 0.0f64);
            let mut vmax = 0.0f64;
            for _ in 0..20 {
                let t0 = Instant::now();
                rw.bild(&mut s);
                t += t0.elapsed().as_secs_f64();
                it += rw.statistik.cg_iterationen; los += rw.statistik.loesungen; nk += rw.statistik.nicht_konvergiert;
                vmax = s.v.iter().map(|v| (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt()).fold(vmax, f64::max);
            }
            println!("{:<28} CG max {:>4} tol {:.0e}: {} Punkte, CG/Lösung {:.0}, nicht konvergiert {}/{}, CPU {:.1} ms/Bild, v_max {:.2} m/s  | XPBD bräuchte {} Unterschritte je Bild",
                mat.name, cgmax, tol, s.x.len(), it as f64 / los as f64, nk, los, t / 20.0 * 1e3, vmax, n_xpbd);
        }
    }
}
