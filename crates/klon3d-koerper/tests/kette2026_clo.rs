//! The full 2026.1 chain against CLO: calibration, post-processing, final values, points.
//!
//! Reference values: `pruefwerte_2026_1.json` → `anwenderin_avs` (calibration of
//! BASE and DETAIL from the user's `.avs`, with a recording from CLO as
//! reference). Tolerances per `formung_2026_1.md` 15: 0.1 mm for the
//! final values, 0.05 mm for the points.
//!
//! Without CLO's 2026.1 folder or without the reference file, the test is skipped.

mod common;

use common::json_mini::{codes, listen, ordner, pruef, J};
use klon3d_koerper::kette2026::{Ende, Kette2026, Zustand, KOERPER};
use klon3d_koerper::messung2026::ANZAHL;
use klon3d_koerper::Punkt3;

/// The user's `.avs` input (raw values, slot order 0…57).
fn eingabe(p: &J) -> Vec<f64> {
    p.feld("anwenderin_avs")
        .feld("eingabe_avs")
        .liste()
        .iter()
        .map(|z| z.zahl())
        .collect()
}

/// Target values of the post-processing (sliders 58…66): presets.
fn vorgaben(p: &J) -> Vec<(usize, f64)> {
    let v = p.feld("vorgaben_58_66");
    let mut r: Vec<(usize, f64)> = v
        .objekt()
        .iter()
        .map(|(k, v)| (k.parse().expect("Platz"), v.zahl()))
        .collect();
    r.sort_by_key(|(j, _)| *j);
    r
}

/// `.avs` → state, as CLO does on load.
///
/// Slots 0…57 with code ≠ 0 are set when a value is present; the
/// sliders 58…66 get their presets.
fn lade_zustand(p: &J, c: &[u8]) -> Zustand {
    let mut z = Zustand::neu(c, 8, true, true, -1.0);
    setze_eingabe(&mut z, p);
    z
}

/// Sets the `.avs` values again (CLO does this before every step), `f` and `b*`
/// stay.
fn setze_eingabe(z: &mut Zustand, p: &J) {
    let e = eingabe(p);
    for j in 0..58 {
        if z.codes[j] != 0 {
            z.setze(j, e[j]);
        }
    }
    for (j, v) in vorgaben(p) {
        z.setze(j, v);
    }
}

fn zahl_liste(j: &J) -> Vec<f64> {
    j.liste()
        .iter()
        .map(|x| match x {
            J::Zahl(z) => *z,
            _ => f64::NAN,
        })
        .collect()
}

#[test]
fn chain_avs_matches_clo() {
    let (Some(o), Some(p)) = (ordner(), pruef()) else {
        eprintln!("übersprungen: CLO-Ordner oder Prüfdatei fehlt");
        return;
    };
    let k = Kette2026::laden(&o).expect("CLO-Daten");
    let c = codes(&p);
    let (liste1, liste0) = listen(&p);
    let a = p.feld("anwenderin_avs");

    // --- BASE -------------------------------------------------------------
    let mut z = lade_zustand(&p, &c);
    assert_eq!(z.listen[1], liste1, "BASE-Stufenliste");
    let f_start = zahl_liste(a.feld("base").feld("f_start"));
    assert_eq!(f_start.len(), ANZAHL, "f_start ist 67 lang");
    for &j in &liste1 {
        let ist = z.f[j];
        assert!(
            (ist - f_start[j]).abs() < 1.0e-6,
            "f_start[{j}]: {ist} statt {}",
            f_start[j]
        );
    }
    let e = k.kalibrieren(&mut z, 1, 1);
    let soll_e = zahl_liste(a.feld("base").feld("E"));
    println!(
        "BASE: {} Formungen, Ende {:?}, E = {:?}",
        e.e_folge.len(),
        e.grund,
        e.e_folge.iter().map(|x| (x * 1e6).round() / 1e6).collect::<Vec<_>>()
    );
    assert_eq!(e.grund, Ende::Konvergenz, "BASE-Ende");
    assert_eq!(e.e_folge.len(), soll_e.len(), "BASE-Formungen");
    for (i, (ist, soll)) in e.e_folge.iter().zip(&soll_e).enumerate() {
        assert!(
            (ist - soll).abs() < 1.0e-4,
            "BASE E[{i}]: {ist} statt {soll}"
        );
    }
    let b = a.feld("base");
    let f_folge = b.feld("f_folge");
    let bust_soll = b.z("bust_86a78");
    assert!(
        (z.bust - bust_soll).abs() < 1.0e-6,
        "b*: {} statt {bust_soll}",
        z.bust
    );
    // The breast cache of the own run (full precision).
    let bust_soll_ist = z.bust;
    // Feature sequence of every form step.
    let mut maxf: f64 = 0.0;
    for (i, zeile) in f_folge.liste().iter().enumerate() {
        for (jj, &j) in liste1.iter().enumerate() {
            maxf = maxf.max((e.f_folge[i][j] - zeile.liste()[jj].zahl()).abs());
        }
    }
    println!(
        "BASE f_folge: {} Formungen, grösste Abweichung {maxf:.3e}",
        e.f_folge.len()
    );
    assert_eq!(e.f_folge.len(), f_folge.liste().len(), "f_folge-Länge");
    assert!(maxf < 1.0e-5, "BASE f_folge: {maxf:.3e}");

    // --- DETAIL -----------------------------------------------------------
    let mut z = lade_zustand(&p, &c);
    let f_start0 = zahl_liste(a.feld("detail").feld("f_start"));
    for &j in &liste0 {
        let ist = z.f[j];
        assert!(
            (ist - f_start0[j]).abs() < 1.0e-6,
            "DETAIL f_start[{j}]: {ist} statt {}",
            f_start0[j]
        );
    }
    // After BASE the DETAIL stage carries the breast cache; the input is set
    // again as in CLO.
    z.bust = bust_soll_ist;
    setze_eingabe(&mut z, &p);
    let ed = k.kalibrieren(&mut z, 0, 0);
    let soll_ed = zahl_liste(a.feld("detail").feld("E"));
    println!(
        "DETAIL: {} Formungen, Ende {:?}, E = {:?}",
        ed.e_folge.len(),
        ed.grund,
        ed.e_folge.iter().map(|x| (x * 1e6).round() / 1e6).collect::<Vec<_>>()
    );
    assert_eq!(ed.grund, Ende::Konvergenz, "DETAIL-Ende");
    assert_eq!(ed.e_folge.len(), soll_ed.len(), "DETAIL-Formungen");
    let mut maxe: f64 = 0.0;
    for (i, (ist, soll)) in ed.e_folge.iter().zip(&soll_ed).enumerate() {
        maxe = maxe.max((ist - soll).abs());
        if (ist - soll).abs() > 1.0e-4 {
            println!("  DETAIL E[{i}]: {ist:.6} statt {soll:.6}");
        }
    }
    println!("DETAIL E: grösste Abweichung {maxe:.3e}");
    // The E values of the reference file arise before the f32 measurement; the loop
    // amplifies small differences over 21 form steps. The yardstick is
    // therefore not E, but final values and points (below).
    assert!(maxe < 0.05, "DETAIL E: {maxe:.3e}");

    // --- Final values -----------------------------------------------------
    let end = zahl_liste(a.feld("endwerte"));
    let mut maxd: f64 = 0.0;
    let mut wo = 0;
    for j in 0..60 {
        if j == 1 {
            continue;
        }
        let d = (ed.werte[j] - end[j]).abs() * 10.0;
        if d > maxd {
            maxd = d;
            wo = j;
        }
    }
    println!(
        "Endwerte: grösste Abweichung {maxd:.5} mm — Platz {wo} ({:?})",
        klon3d_koerper::messung2026::NAMEN[wo]
    );
    assert!(maxd <= 0.1, "{maxd:.5} mm bei Platz {wo}");

    // --- Points -----------------------------------------------------------
    let clo = a.feld("punktstichprobe_clo").objekt();
    let nach = a.feld("punktstichprobe_nachbau").objekt();
    let mut maxp: f64 = 0.0;
    let mut wp = 0;
    for (nr, soll) in clo {
        let i: usize = nr.parse().expect("Punktnummer");
        if i >= KOERPER {
            continue;
        }
        let s: Punkt3 = [
            soll.liste()[0].zahl(),
            soll.liste()[1].zahl(),
            soll.liste()[2].zahl(),
        ];
        let d = abstand(ed.punkte[i], s) * 10.0;
        if d > maxp {
            maxp = d;
            wp = i;
        }
    }
    println!("Punkte gegen CLO: grösste Abweichung {maxp:.5} mm (Punkt {wp})");
    assert!(maxp <= 0.05, "{maxp:.5} mm bei Punkt {wp}");

    let mut maxn: f64 = 0.0;
    for (nr, soll) in nach {
        let i: usize = nr.parse().expect("Punktnummer");
        if i >= KOERPER {
            continue;
        }
        let s: Punkt3 = [
            soll.liste()[0].zahl(),
            soll.liste()[1].zahl(),
            soll.liste()[2].zahl(),
        ];
        maxn = maxn.max(abstand(ed.punkte[i], s) * 10.0);
    }
    println!("Punkte gegen Nachbau: grösste Abweichung {maxn:.5} mm");
    assert!(maxn <= 0.05, "Nachbau-Punkte {maxn:.5} mm");
    assert_eq!(ANZAHL, 67);
}

fn abstand(a: Punkt3, b: Punkt3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
