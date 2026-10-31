//! The entry point of `klon3d.exe`: body from `.smis`, chain or fallback.
//!
//! Mesh and data: CLO's resizing folder (`CLO_FEMALE` selects another one; the
//! default is 2026.1). Without it the tests are skipped.
//!
//! Three paths are checked: the 2026.1 chain (default folder), the 5.2 chain
//! (only when the 5.2 folder exists – backup) and the old `anpassen` path from a
//! folder holding only the 5.2 `Advanced` files.

use std::path::PathBuf;

use klon3d_koerper::kette::KOERPER;
use klon3d_koerper::koerper::{aus_smis, ordner, Weg};

/// CLO's 5.2 folder of the default installation.
const ORDNER_52: &str = r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female";

fn daten() -> Option<(PathBuf, klon3d_pattern::Smis)> {
    let o = ordner();
    if !o.join("BaseMesh.obj").is_file() {
        eprintln!("übersprungen: CLO-Resizing-Daten fehlen in {}", o.display());
        return None;
    }
    let pfad = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../klon3d-pattern/tests/muster/measurements.smis"
    ));
    let Ok(smis) = klon3d_pattern::lies_smis(&pfad) else {
        eprintln!("übersprungen: {} fehlt", pfad.display());
        return None;
    };
    Some((o, smis))
}

fn pruefe_netz(k: &klon3d_koerper::koerper::Koerper) {
    assert_eq!(k.netz.punkte.len(), KOERPER);
    assert!(!k.netz.dreiecke.is_empty());
    assert!(k.netz.dreiecke.iter().all(|d| d.iter().all(|&i| i < KOERPER)));
    // mm: rund 160 cm hoch.
    let (lo, hi) = k.netz.hoehe().unwrap();
    assert!((1550.0..1650.0).contains(&(hi - lo)), "Höhe {} mm", hi - lo);
}

#[test]
fn kette_formt_den_koerper() {
    let Some((o, smis)) = daten() else { return };
    let k = aus_smis(&o, &|n| smis.wert(n)).expect("Körper");
    assert_eq!(k.weg, Weg::Kette, "{:?}", k.meldungen);
    pruefe_netz(&k);
    let e = k.kette.as_ref().expect("Werte der Kette");
    for (j, name) in [(2, "bust_circ"), (6, "waist_circ"), (8, "hip_circ")] {
        assert!(
            (e.werte[j] - smis.wert(name).unwrap()).abs() < 1.0,
            "{name}: {}",
            e.werte[j]
        );
    }
}

/// No 2026.1 data, but 5.2 data: the 5.2 chain shapes (backup).
#[test]
fn sicherung_52_formt_den_koerper() {
    let Some((_, smis)) = daten() else { return };
    let alt = PathBuf::from(ORDNER_52);
    if !alt.join("ConvertingMat_DETAIL_Human_Bust_TotalHeight.bd").is_file() {
        eprintln!("übersprungen: 5.2-Daten fehlen in {}", alt.display());
        return;
    }
    let k = aus_smis(&alt, &|n| smis.wert(n)).expect("Körper");
    assert_eq!(k.weg, Weg::Kette52, "{:?}", k.meldungen);
    assert!(
        k.meldungen[0].starts_with("2026.1 nicht ladbar"),
        "{:?}",
        k.meldungen
    );
    pruefe_netz(&k);
    assert!(k.kette.is_some());
}

/// Only the files of the old path: neither chain is loadable, `anpassen` shapes.
#[test]
fn fallback_ohne_kettendaten() {
    let Some((_, smis)) = daten() else { return };
    let alt52 = PathBuf::from(ORDNER_52);
    let alt = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("koerper_nur_alt");
    std::fs::create_dir_all(&alt).expect("Ordner");
    for f in [
        "BaseMesh.obj",
        "ConvertingMat_DETAIL_Advanced_TotalHeight.bd",
        "_DetailFeature_Advanced_Bust_TotalHeight.txt",
    ] {
        std::fs::copy(alt52.join(f), alt.join(f)).expect(f);
    }
    let k = aus_smis(&alt, &|n| smis.wert(n)).expect("Körper");
    assert_eq!(k.weg, Weg::Anpassen);
    assert!(
        k.meldungen[0].starts_with("2026.1 nicht ladbar"),
        "{:?}",
        k.meldungen
    );
    assert!(k.kette.is_none());
    pruefe_netz(&k);
}
