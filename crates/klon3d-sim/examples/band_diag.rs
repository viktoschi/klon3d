//! Diagnose des Bands um die Taille (`tests/koerpernetz.rs`): je Bild mittlere
//! und größte Geschwindigkeit, Kontakte, CG-Statistik und der tiefste Punkt
//! (über alle Dreiecke gemessen); am Ende das Band als OBJ.
//!
//! `cargo run --release --example band_diag -- [radical|retifist] [Reduktion %] [Bilder] [Kante mm] [CG max] [band.obj]`

#[path = "../tests/band/mod.rs"]
mod band;

use klon3d_sim::{ausgabe, ImplizitCg, ImplizitParameter, Material, Rechenweg, RADICAL_RUBBER_04};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let m = a.first().and_then(|n| Material::nach_namen(n)).unwrap_or(RADICAL_RUBBER_04);
    let red = a.get(1).and_then(|v| v.parse::<f64>().ok()).unwrap_or(5.5) / 100.0;
    let bilder = a.get(2).and_then(|v| v.parse::<usize>().ok()).unwrap_or(300);
    let k = band::koerper().expect("Körper");
    let tz = band::taille(&k);
    let kante = a.get(3).and_then(|v| v.parse::<f64>().ok()).unwrap_or(band::KANTE_MM);
    let cg_max = a.get(4).and_then(|v| v.parse::<usize>().ok()).unwrap_or(50);
    let (mut s, muster) = band::band_szene(&k, tz, m, red, kante);
    let t = m.dicke_m;
    println!("{} {:.1} %: Taille {:.1} cm, {} Punkte, Kante {kante} mm, CG max {cg_max}", m.name, red * 100.0, tz * 100.0, s.x.len());
    let mut rw = ImplizitCg::neu(ImplizitParameter { cg_max, ..Default::default() });
    for b in 1..=bilder {
        rw.bild(&mut s);
        if b <= 12 || b % 25 == 0 {
            let v: Vec<f64> = s.v.iter().map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()).collect();
            let (imax, vmax) = v.iter().enumerate().fold((0, 0.0), |m, (i, &x)| if x > m.1 { (i, x) } else { m });
            let tief: Vec<f64> = s.x.iter().map(|&p| k.naechster_alle(p).abstand - t / 2.0).collect();
            let (itief, dtief) = tief.iter().enumerate().fold((0, f64::INFINITY), |m, (i, &x)| if x < m.1 { (i, x) } else { m });
            let verloren = tief.iter().filter(|&&d| d < -k.suchtiefe).count();
            let st = rw.statistik;
            println!(
                "Bild {b:4}: v mittel {:.4} max {:.3} m/s (Punkt {imax}, z {:.1} cm, Abstand {:+.2} mm); tiefster {:+.3} mm (Punkt {itief}), unter Suchtiefe {verloren}; Kontakte {} haftend {} frei {}; CG {}/{} nicht konv. {} Rest {:.1e}",
                v.iter().sum::<f64>() / v.len() as f64,
                vmax,
                s.x[imax][2] * 100.0,
                tief[imax] * 1e3,
                dtief * 1e3,
                st.kontakte,
                st.haftend,
                st.freigegeben,
                st.cg_iterationen,
                st.loesungen,
                st.nicht_konvergiert,
                st.rest_max
            );
        }
    }
    let e = band::band_auswerten(&k, &s, &muster, tz);
    println!("{e:?}");
    if let Some(p) = a.get(5) {
        std::fs::write(p, ausgabe::obj(&s)).expect("OBJ");
        println!("geschrieben {p}");
    }
}
