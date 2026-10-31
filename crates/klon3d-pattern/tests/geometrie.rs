//! Geometrieprüfung gegen die Sollwerte.
//!
//! Die Sollwerte stammen aus den Abwärfen von `schnitt.py`
//! (`tests/muster/sollwerte.rs`), einer dokumentierten Nachbildung von
//! Seamly2Ds C++-Geometrie. Sie stehen in der Konvention der `.sm2d`
//! (y nach unten), dieselbe, in der [`Auswertung`] rechnet.
//!
//! Toleranz 1 µm Mustereinheit. Das ist 0,001 mm — deutlich unter dem, was
//! `schnitt.py` gegen Seamly2Ds eigenen Export erreicht (0,51 mm an den
//! Umrissen, 0,0000003 cm an den Schnittlängen).

mod muster;

use klon3d_pattern::geom::{lies_messungen, Auswertung};
use klon3d_pattern::read_path;

use muster::*;

/// Höchste zulässige Abweichung in Mustereinheiten (cm).
///
/// **0,001 cm = 0,01 mm** — die Auflösung der Referenz. Die Dumps von
/// `schnitt.py` schreiben drei Nachkommastellen; strenger zu messen wäre
/// sinnlos. Belegt: `J_R` und `K_R` des Bustiers weichen exakt um 0,0005 ab,
/// das ist die halbe letzte Stelle.
const TOL: f64 = 1e-3;

struct Bericht {
    geprueft: usize,
    fehlend: Vec<String>,
    unauflösbar: Vec<String>,
    schlechtester: (String, f64),
    ueber_tol: Vec<(String, f64, f64, f64)>,
}

fn pruefe(muster: &str, soll: &[(&str, f64, f64)]) -> Bericht {
    let pfad = muster_pfad(muster);
    let m = read_path(&pfad)
        .unwrap_or_else(|e| panic!("{} nicht lesbar: {e}", pfad.display()));
    let mess = lies_messungen(&mess_pfad(), &m.unit).expect("Massdatei");
    let ev = Auswertung::new(&m, mess);

    let mut b = Bericht {
        geprueft: 0,
        fehlend: Vec::new(),
        unauflösbar: Vec::new(),
        schlechtester: (String::new(), 0.0),
        ueber_tol: Vec::new(),
    };
    for (name, sx, sy) in soll {
        let id = match ev.id_von_name(name) {
            Some(i) => i.to_string(),
            None => {
                b.fehlend.push((*name).to_string());
                continue;
            }
        };
        let Ok(p) = ev.punkt(&id) else {
            b.unauflösbar.push((*name).to_string());
            continue;
        };
        b.geprueft += 1;
        let d = ((p.x - sx).powi(2) + (p.y - sy).powi(2)).sqrt();
        if d > b.schlechtester.1 {
            b.schlechtester = ((*name).to_string(), d);
        }
        if d > TOL {
            b.ueber_tol.push(((*name).to_string(), d, p.x - sx, p.y - sy));
        }
    }
    b.ueber_tol.sort_by(|a, c| c.1.total_cmp(&a.1));
    b
}

#[test]
fn bustier_stimmt_genau() {
    let b = pruefe("bustier_v1", BUSTIER_V1);
    println!(
        "bustier_v1: {} verglichen, schlechtester {} {:.2e}, ueber Toleranz {}",
        b.geprueft,
        b.schlechtester.0,
        b.schlechtester.1,
        b.ueber_tol.len()
    );
    assert_eq!(b.unauflösbar.len(), 0, "nicht aufloesbar: {:?}", b.unauflösbar);
    assert_eq!(
        b.ueber_tol, Vec::<(String, f64, f64, f64)>::new(),
        "Abweichungen ueber {TOL}"
    );
}

#[test]
fn bluse2_v26_stimmt_innerhalb_toleranz() {
    let b = pruefe("bluse2_v26", BLUSE2_V26);
    bericht_ausgeben("bluse2_v26", &b);
    assert!(
        b.unauflösbar.is_empty(),
        "{} Punkte nicht aufloesbar, z. B. {:?}",
        b.unauflösbar.len(),
        &b.unauflösbar[..b.unauflösbar.len().min(8)]
    );
    assert!(
        b.ueber_tol.is_empty(),
        "{} Punkte ueber {TOL}, schlimmste: {:?}",
        b.ueber_tol.len(),
        &b.ueber_tol[..b.ueber_tol.len().min(8)]
    );
}

#[test]
fn bluse2_v29_stimmt_innerhalb_toleranz() {
    let b = pruefe("bluse2_v29", BLUSE2_V29);
    bericht_ausgeben("bluse2_v29", &b);
    assert!(
        b.unauflösbar.is_empty(),
        "{} Punkte nicht aufloesbar",
        b.unauflösbar.len()
    );
    assert!(
        b.ueber_tol.is_empty(),
        "{} Punkte ueber {TOL}, schlimmste: {:?}",
        b.ueber_tol.len(),
        &b.ueber_tol[..b.ueber_tol.len().min(8)]
    );
}

#[test]
fn bluse3_v1_stimmt_innerhalb_toleranz() {
    let b = pruefe("bluse3_v1", BLUSE3_V1);
    bericht_ausgeben("bluse3_v1", &b);
    assert!(
        b.unauflösbar.is_empty(),
        "{} Punkte nicht aufloesbar",
        b.unauflösbar.len()
    );
    assert!(
        b.ueber_tol.is_empty(),
        "{} Punkte ueber {TOL}, schlimmste: {:?}",
        b.ueber_tol.len(),
        &b.ueber_tol[..b.ueber_tol.len().min(8)]
    );
}

fn bericht_ausgeben(name: &str, b: &Bericht) {
    println!(
        "{name}: {} verglichen | fehlend {} | unauflösbar {} | ueber Toleranz {} | \
         schlechtester {} = {:.3e}",
        b.geprueft,
        b.fehlend.len(),
        b.unauflösbar.len(),
        b.ueber_tol.len(),
        b.schlechtester.0,
        b.schlechtester.1
    );
    for (n, d, dx, dy) in b.ueber_tol.iter().take(10) {
        println!("    {n:<24} {d:.3e}  dx={dx:+.6} dy={dy:+.6}");
    }
    for n in b.unauflösbar.iter().take(10) {
        println!("    unauflösbar: {n}");
    }
}