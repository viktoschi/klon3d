//! Diagnose der Unruhe in der Ruhe (Befund 102): ein Prüfmuster wie
//! `klon3d anziehen` anziehen (Körper aus der Maßdatei neben dem Muster,
//! Dicke je Teil aus der Lage) und je 30 Bilder Bewegungsenergie, die
//! schnellsten Punkte (Ort, Teil, Hautabstand) und die Kontaktzahlen des
//! Rechenwegs ausgeben.
//!
//! `cargo run --release --example unruhe_diag`
//!
//! Umgebung: `MUSTER=<Pfad unter tests/muster ohne .sm2d>` (Vorgabe
//! `hexen_2026_10_27/rock_v36`), `NAEHTE`, `LAGE` (Dateien statt der
//! Handdateien), `RUHEBILDER=<n>` (höchstens so viele Bilder Einschwingen), `ABSTAND=<Bilder>` (Ausgabe alle n Bilder),
//! `CG_MAX=<n>`, `CG_TOL=<rel. Rest>`, `SCHRITTE=<je Bild>`, `NORMAL_LAST` (Normalkraft der Reibung aus der Last) (Rechenweg).

#[path = "../tests/band/mod.rs"]
mod band;

use std::path::PathBuf;

use klon3d_mesh::{vernetze, Parameter};
use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::naehte::{lies_nahtliste, pruefe_naehte};
use klon3d_sim::platzierung::{anziehen, lies_lage, platziere, Anziehen};
use klon3d_sim::{messung, ImplizitCg, ImplizitParameter, Koerper, Simparameter, Szene, Teilvorgabe, RADICAL_RUBBER_04};

fn main() {
    let w = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../klon3d-pattern/tests/muster"));
    let name = std::env::var("MUSTER").unwrap_or_else(|_| "hexen_2026_10_27/rock_v36".into());
    let pfad = w.join(format!("{name}.sm2d"));
    let mess = pfad.with_file_name("measurements.smis");
    let (k, h) = band::pruefkoerper_aus(&mess).expect("Körper");
    let m = klon3d_pattern::read_path(&pfad).unwrap();
    let ev = Auswertung::lade(&m, &mess).unwrap();
    let datei = |var: &str, endung: &str| std::env::var(var).map(PathBuf::from).unwrap_or_else(|_| pfad.with_extension(endung));
    let defs = lies_nahtliste(&std::fs::read_to_string(datei("NAEHTE", "naehte")).unwrap()).unwrap();
    let b = pruefe_naehte(&ev, &defs, Kurvenquelle::Seamly).unwrap();
    let netz = vernetze(&ev, Kurvenquelle::Seamly, &b.naehte, &Parameter::default()).unwrap();
    let liste = lies_lage(&std::fs::read_to_string(datei("LAGE", "lage")).unwrap()).unwrap();
    let p = platziere(&netz, &liste, &h).unwrap();
    let vg = |teil: &str| {
        let dicke = p.netz.teile.iter().position(|t| t.teil == teil).and_then(|ti| p.dicke(ti));
        Teilvorgabe { material: dicke.map_or(RADICAL_RUBBER_04, |d| RADICAL_RUBBER_04.mit_dicke(d)), reduktion: 0.0 }
    };
    let param = Simparameter { schwerkraft: [0.0, 0.0, -9.81], daempfung: 1.0, ..Simparameter::default() };
    let mut s = Szene::aus_netz(&p.netz, &vg, &|ti, q| p.lage(ti, q), vec![Koerper::netz(k.clone())], param);
    let mut ablauf = Anziehen::default();
    let zahl = |n: &str| std::env::var(n).ok().and_then(|v| v.parse::<usize>().ok());
    if let Some(r) = zahl("RUHEBILDER") {
        ablauf.einschwingen_max = r;
    }
    let alle = zahl("ABSTAND").unwrap_or(30);
    let teil_von = |s: &Szene, i: usize| s.teile.iter().find(|t| t.punkte.contains(&i)).map_or("?".to_string(), |t| t.name.rsplit(" - ").next().unwrap_or(&t.name).to_string());
    println!("{name}: {} Punkte, {} Nähte; Ablauf {ablauf:?}", s.x.len(), p.netz.naehte.len());
    let mut ip = ImplizitParameter::default();
    if let Some(c) = zahl("CG_MAX") {
        ip.cg_max = c;
    }
    if let Some(c) = std::env::var("CG_TOL").ok().and_then(|v| v.parse::<f64>().ok()) {
        ip.cg_toleranz = c;
    }
    if std::env::var_os("NORMAL_LAST").is_some() {
        ip.normalkraft_aus_last = true;
    }
    if let Some(c) = zahl("SCHRITTE") {
        ip.schritte_je_bild = c;
    }
    println!("Rechenweg: {ip:?}");
    let mut rw = ImplizitCg::neu(ip);
    let uhr = std::time::Instant::now();
    let mut iterationen = (0usize, 0usize, 0usize);
    anziehen(&mut s, &mut rw, &ablauf, &mut |bild, s, rw| {
        iterationen.0 += rw.statistik.cg_iterationen;
        iterationen.1 += rw.statistik.loesungen;
        iterationen.2 += rw.statistik.nicht_konvergiert;
        // Zahlen des Rechenwegs gelten je Bild.
        let st = rw.statistik;
        if bild % alle != 0 && bild != ablauf.naehbilder + 1 {
            return;
        }
        let e: f64 = (0..s.x.len()).map(|i| 0.5 * s.masse[i] * (s.v[i][0].powi(2) + s.v[i][1].powi(2) + s.v[i][2].powi(2))).sum();
        let mut schnell: Vec<(f64, usize)> = s.v.iter().enumerate().map(|(i, v)| ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(), i)).collect();
        schnell.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!(
            "Bild {bild:4} ({}): E {:.3e} J, v Mittel {:.4} m/s; Kontakte {} (haftend {}), freigegeben {}, Durchstöße {}, CG nicht konv. {}/{}, {} Iterationen, Rest max {:.1e}",
            if bild <= ablauf.naehbilder { "nähen" } else { "Ruhe" },
            e,
            messung::mittlere_geschwindigkeit(s),
            st.kontakte,
            st.haftend,
            st.freigegeben,
            st.durchstoesse,
            st.nicht_konvergiert,
            st.loesungen,
            st.cg_iterationen,
            st.rest_max
        );
        for &(v, i) in schnell.iter().take(5) {
            let q = s.x[i];
            let n = k.naechster_alle(q);
            println!(
                "    {:.3} m/s {:<28} ({:6.1}, {:6.1}, {:6.1}) cm, Haut {:+6.1} mm, Normale ({:+.2}, {:+.2}, {:+.2}), v ({:+.2}, {:+.2}, {:+.2})",
                v,
                teil_von(s, i),
                q[0] * 100.0,
                q[1] * 100.0,
                q[2] * 100.0,
                n.abstand * 1e3,
                n.normale[0],
                n.normale[1],
                n.normale[2],
                s.v[i][0],
                s.v[i][1],
                s.v[i][2]
            );
        }
    });
    let a: Vec<f64> = messung::nahtabstaende(&s, &p.netz).into_iter().flat_map(|(_, d)| d).collect();
    println!("Ende: {} von {} Nahtpaaren bis 10 mm, v {:.4} m/s", a.iter().filter(|&&x| x <= 0.01).count(), a.len(), messung::mittlere_geschwindigkeit(&s));
    println!(
        "CG: {} Lösungen, im Mittel {:.0} Iterationen, {} nicht konvergiert; {:.1} s",
        iterationen.1,
        iterationen.0 as f64 / iterationen.1.max(1) as f64,
        iterationen.2,
        uhr.elapsed().as_secs_f64()
    );
}
