//! Körper aus Maßen formen: CLO-Felder, `.smis`-Maße, Newton-Anpassung.
//!
//! Aufruf:
//! ```text
//! cargo run --release -p klon3d-koerper --example forme -- \
//!     [Ordner] [Brust] [Taille] [Hüfte] [Höhe] [HighHip] [Unterbrust] \
//!     [--smis Massdatei.smis] [--out Ziel.obj] [--scan]
//! ```
//!
//! Ohne `--smis` gelten die Maße der persönlichen Datei
//! (160 / 92,67 / 74 / 90 / 83 / 76). Mit `--smis` kommen Höhe, Brust,
//! Taille, Hüfte, HighHip und Unterbrust aus der Datei; Zahlen auf der
//! Kommandozeile überschreiben sie. Fehlt `highhip_circ` oder
//! `lowbust_circ`, wird die jeweilige Linie weggelassen.
//!
//! Die Zuordnung Maßdatei → CLO-Felder und die Messlinien macht
//! [`klon3d_koerper::person`]; hier steht nur die Kommandozeile und die
//! Ausgabe.

use klon3d_koerper::anpassen::{anpassen, messen, HOEHE_REF};
use klon3d_koerper::formung::{namen_vektor, referenz_vektor, Formung};
use klon3d_koerper::person::zielperson;
use klon3d_koerper::{lies_obj, rumpf, schreibe_obj};
use klon3d_pattern::lies_smis;
use std::path::PathBuf;
use std::time::Instant;

const STANDARD_ORDNER: &str =
    r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female";

struct Aufruf {
    ordner: PathBuf,
    smis: Option<PathBuf>,
    aus: Option<PathBuf>,
    zahlen: Vec<f64>,
    scan: bool,
}

fn aufruf() -> Result<Aufruf, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut a = Aufruf {
        ordner: PathBuf::from(STANDARD_ORDNER),
        smis: None,
        aus: None,
        zahlen: Vec::new(),
        scan: false,
    };
    let mut ordner_gesetzt = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--smis" => {
                i += 1;
                let p = args
                    .get(i)
                    .ok_or_else(|| "--smis ohne Pfad".to_string())?;
                a.smis = Some(PathBuf::from(p));
            }
            "--out" => {
                i += 1;
                let p = args.get(i).ok_or_else(|| "--out ohne Pfad".to_string())?;
                a.aus = Some(PathBuf::from(p));
            }
            "--scan" => a.scan = true,
            s if s.starts_with("--") => {
                return Err(format!("unbekannte Option {s}"));
            }
            s => {
                if let Ok(v) = s.parse::<f64>() {
                    a.zahlen.push(v);
                } else if !ordner_gesetzt {
                    a.ordner = PathBuf::from(s);
                    ordner_gesetzt = true;
                } else if s.to_ascii_lowercase().ends_with(".smis") && a.smis.is_none() {
                    a.smis = Some(PathBuf::from(s));
                } else if a.aus.is_none() {
                    a.aus = Some(PathBuf::from(s));
                } else {
                    return Err(format!("unerwartetes Argument {s}"));
                }
            }
        }
        i += 1;
    }
    Ok(a)
}

fn lauf() -> Result<(), String> {
    let a = aufruf()?;
    let txt = a.ordner.join("_DetailFeature_Advanced_Bust_TotalHeight.txt");
    let bd = a
        .ordner
        .join("ConvertingMat_DETAIL_Advanced_TotalHeight.bd");

    let grund = lies_obj(&a.ordner.join("BaseMesh.obj"), 10.0)
        .map_err(|e| format!("BaseMesh.obj: {e}"))?;
    let formung =
        Formung::laden(&bd, grund.punkte.len()).map_err(|e| format!("Formungsdatei: {e}"))?;
    let c_ref = referenz_vektor(&txt).map_err(|e| format!("Referenzvektor: {e}"))?;
    let namen = namen_vektor(&txt).map_err(|e| format!("Maßnamen: {e}"))?;
    if c_ref.len() + 1 != formung.felder() {
        return Err(format!(
            "{} Koeffizienten für {} Felder",
            c_ref.len(),
            formung.felder()
        ));
    }

    // Maße: Standard = persönliche Datei; --smis ersetzt sie.
    let smis = match &a.smis {
        Some(p) => Some(lies_smis(p).map_err(|e| format!("{}: {e}", p.display()))?),
        None => None,
    };
    let mut hoehe = 160.0;
    let mut brust = 92.67;
    let mut taille = 74.0;
    let mut huefte = 90.0;
    let mut highhip = Some(83.0);
    let mut lowbust = Some(76.0);
    if let Some(s) = &smis {
        hoehe = s.wert("height").ok_or_else(|| "height fehlt".to_string())?;
        brust = s
            .wert("bust_circ")
            .ok_or_else(|| "bust_circ fehlt".to_string())?;
        taille = s
            .wert("waist_circ")
            .ok_or_else(|| "waist_circ fehlt".to_string())?;
        huefte = s
            .wert("hip_circ")
            .ok_or_else(|| "hip_circ fehlt".to_string())?;
        highhip = s.wert("highhip_circ");
        lowbust = s.wert("lowbust_circ");
    }
    if let Some(v) = a.zahlen.first() {
        brust = *v;
    }
    if let Some(v) = a.zahlen.get(1) {
        taille = *v;
    }
    if let Some(v) = a.zahlen.get(2) {
        huefte = *v;
    }
    if let Some(v) = a.zahlen.get(3) {
        hoehe = *v;
    }
    if let Some(v) = a.zahlen.get(4) {
        highhip = Some(*v);
    }
    if let Some(v) = a.zahlen.get(5) {
        lowbust = Some(*v);
    }

    // Die effektiven Maße gehen vor die Datei; alles andere (Höhen,
    // Armlochbreiten, …) kommt unverändert aus der .smis.
    let wert = |name: &str| -> Option<f64> {
        match name {
            "height" => Some(hoehe),
            "bust_circ" => Some(brust),
            "waist_circ" => Some(taille),
            "hip_circ" => Some(huefte),
            "highhip_circ" => highhip,
            "lowbust_circ" => lowbust,
            _ => smis.as_ref().and_then(|s| s.wert(name)),
        }
    };
    let p = zielperson(&grund, &namen, &c_ref, &wert)?;

    let (lo, hi) = grund.hoehe().unwrap();
    println!(
        "Grundnetz: {} Punkte, {:.2} cm hoch",
        grund.punkte.len(),
        (hi - lo) / 10.0
    );
    println!(
        "Formung: {} Felder ({} Maße + Nullpunkt), Referenzvektor {} Werte",
        formung.felder(),
        c_ref.len(),
        namen.len()
    );
    if !p.hoehen_geaendert.is_empty() {
        println!("\nHöhenfelder aus der Maßdatei:");
        for (feld, alt, neu) in &p.hoehen_geaendert {
            println!("  {feld:22} {alt:8.3} -> {neu:8.3} cm");
        }
    }

    let linien = &p.linien;
    println!("\nAktive Felder:");
    for (i, k) in p.aktiv.iter().enumerate() {
        println!("  {:12} -> {}", linien.namen()[i], namen[*k]);
    }

    // Probe: der unveränderte Referenzvektor bei denselben Linien.
    let referenz = formung.anwenden(&grund, &c_ref);
    let ist_ref = messen(&referenz, linien);
    println!("\nReferenzkörper ({HOEHE_REF:.2} cm), gleiche Linien:");
    for (i, name) in linien.namen().iter().enumerate() {
        let art = if linien.linien[i].spur.is_some() {
            "Materiallinie"
        } else {
            "Schnitt"
        };
        println!(
            "  {name:12} {:7.2} cm ({art} bei {:.2} cm)",
            ist_ref.masse[i], linien.linien[i].hoehe
        );
    }

    println!("\nZiel: Höhe {:.2} cm", p.ziel.hoehe);
    for (i, name) in linien.namen().iter().enumerate() {
        println!(
            "  {name:12} {:7.2} cm bei {:.2} cm",
            p.ziel.masse[i], linien.linien[i].hoehe
        );
    }

    let t0 = Instant::now();
    let e = anpassen(&grund, &formung, &p.c_start, linien, &p.ziel, &p.aktiv)?;
    let dauer = t0.elapsed();

    print!("\nRunde");
    for name in linien.namen() {
        print!(" {name:>11}");
    }
    println!();
    for (i, f) in e.verlauf.iter().enumerate() {
        print!("{i:5}");
        for wert in f {
            print!(" {wert:11.4}");
        }
        println!();
    }
    println!(
        "\n{} Runden in {:.1} s{}",
        e.runden,
        dauer.as_secs_f64(),
        if e.divergiert {
            " (Rücknahme, divergiert)"
        } else {
            ""
        }
    );
    println!("\nIst:");
    for (i, name) in linien.namen().iter().enumerate() {
        println!(
            "  {name:12} {:7.3} cm (Fehler {:+.4})",
            e.ist.masse[i], e.fehler[i]
        );
    }

    let geformt = formung.anwenden(&grund, &e.c);
    println!("\nLage der Linien am geformten Körper:");
    for linie in &linien.linien {
        println!(
            "  {:12} {:.2} cm (angesetzt bei {:.2})",
            linie.ort.name(),
            linie.ist_hoehe(&geformt),
            linie.hoehe
        );
    }

    println!("\nGeänderte Koeffizienten (gegen die Referenz):");
    let mut geaendert = 0;
    for (k, name) in namen.iter().enumerate() {
        let d = e.c[k] - c_ref[k];
        if d.abs() > 0.05 {
            println!("  {name:30} {d:+9.3}");
            geaendert += 1;
        }
    }
    if geaendert == 0 {
        println!("  keine");
    }

    if let Some(pfad) = &a.aus {
        schreibe_obj(pfad, &geformt, 10.0).map_err(|e| format!("{}: {e}", pfad.display()))?;
        println!("\nKörper geschrieben: {}", pfad.display());
    }

    let (lo, hi) = geformt.hoehe().unwrap();
    let (glo, ghi) = grund.hoehe().unwrap();
    println!(
        "\nGeformter Körper: {:.2} cm hoch (Grundnetz {:.2})",
        (hi - lo) / 10.0,
        (ghi - glo) / 10.0
    );

    let (mut min_ta, mut min_y) = (f64::MAX, 0.0);
    let (mut max_hi, mut max_y) = (f64::MIN, 0.0);
    if a.scan {
        println!("\nHöhe  Umfang");
    }
    for y in 68..=132 {
        let y = y as f64;
        if let Some(q) = rumpf(&geformt, y * 10.0) {
            let u = q.laenge / 10.0;
            if a.scan {
                println!("{y:4.0}  {u:7.2}");
            }
            if u < min_ta {
                min_ta = u;
                min_y = y;
            }
            if u > max_hi {
                max_hi = u;
                max_y = y;
            }
        }
    }
    println!(
        "\nSchmalste Stelle {:.2} cm bei {:.0} cm, weiteste {:.2} cm bei {:.0} cm",
        min_ta, min_y, max_hi, max_y
    );

    Ok(())
}

fn main() {
    if let Err(e) = lauf() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
