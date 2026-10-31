//! The reference form step against CLO: regression and first form step.
//!
//! Reference values: `work/spez/pruefwerte_2026_1.json` - `regression`
//! (sample points of the matrix `M`) and `erste_formung_base` (`f_0`, `f_2`,
//! points raw and after symmetrising/centring). Tolerances per
//! `formung_2026_1.md` 15: 10⁻⁵ cm for the first form step, 2 · 10⁻⁶ in
//! feature space.
//!
//! Without CLO's reference folder or without the reference file, the test is skipped.

mod common;

use common::json_mini::{listen, ordner, pruef};
use klon3d_koerper::kette_referenz::{Basis, Pca, NP};

/// The form matrix matches CLO's samples (regression, section 6).
#[test]
fn regression_matches_the_samples() {
    let (Some(o), Some(p)) = (ordner(), pruef()) else {
        eprintln!("übersprungen: CLO-Ordner oder Prüfdatei fehlt");
        return;
    };
    let pca = Pca::laden(&o).expect("PCA-Tabellen");
    let (liste1, liste0) = listen(&p);

    for (stufe, liste) in [(1, &liste1), (0, &liste0)] {
        let m = pca.matrix(liste);
        let k = liste.len() + 1;
        let stichprobe = p.feld("regression").feld(&stufe.to_string()).feld("stichprobe");
        let mut maxd: f64 = 0.0;
        let mut n = 0;
        for (punkt, soll) in stichprobe.objekt() {
            let i: usize = punkt.parse().expect("Punktnummer");
            for c in 0..3 {
                for spalte in 0..k {
                    let ist = m[(3 * i + c) * k + spalte];
                    let s = soll.liste()[c].liste()[spalte].zahl();
                    maxd = maxd.max((ist - s).abs());
                    n += 1;
                }
            }
        }
        println!("Stufe {stufe}: {n} Werte, grösste Abweichung {maxd:.3e}");
        assert!(
            maxd < 2.0e-6,
            "Stufe {stufe}: {maxd:.3e} statt < 2e-6 ({} Stichproben)",
            stichprobe.objekt().len()
        );
    }
}

/// First form step of the BASE stage: raw and after symmetrising/centring.
#[test]
fn first_form_step_base_matches_clo() {
    let (Some(o), Some(p)) = (ordner(), pruef()) else {
        eprintln!("übersprungen");
        return;
    };
    let pca = Pca::laden(&o).expect("PCA");
    let basis = Basis::laden(&o).expect("BaseMesh");
    let (liste1, _) = listen(&p);
    let m = pca.matrix(&liste1);

    let ef = p.feld("erste_formung_base");
    let merkmal = [ef.z("f_0"), ef.z("f_2")];
    let geformt = basis.formen(&m, &liste1, &merkmal);

    // 7.1 without hair: the same computation, but without the hair rule, so the
    // raw points can be compared individually. `formen` applies the
    // hair points already before symmetrising; for the reference points (< NP)
    // that changes nothing.
    let k = liste1.len() + 1;
    let roh: [f64; 3] = {
        let i = 1949;
        let mut q = basis.punkte[i];
        for c in 0..3 {
            let mut s = 0.0;
            for z in 0..k - 1 {
                s += merkmal[z] * m[(3 * i + c) * k + z];
            }
            q[c] += s + m[(3 * i + c) * k + k - 1];
        }
        q
    };
    let soll_roh = ef.feld("punkte_roh").feld("1949").punkt();
    for c in 0..3 {
        assert!(
            (roh[c] - soll_roh[c]).abs() < 1.0e-5,
            "roh[{c}]: {} statt {}",
            roh[c],
            soll_roh[c]
        );
    }

    let mut maxd: f64 = 0.0;
    for (punkt, soll) in ef.feld("punkte_sym_zentriert").objekt() {
        let i: usize = punkt.parse().expect("Punktnummer");
        let s = soll.punkt();
        for c in 0..3 {
            maxd = maxd.max((geformt[i][c] - s[c]).abs());
        }
    }
    println!("symmetrisiert + zentriert: grösste Abweichung {maxd:.3e} cm");
    assert!(maxd < 1.0e-5, "{maxd:.3e} statt < 1e-5 cm");
    assert_eq!(NP, 40_465);
}
