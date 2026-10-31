//! Der Eingang von `klon3d.exe`: Körper aus der `.smis`, Kette oder Fallback.
//!
//! Netz und Daten: CLOs Resizing-Ordner (`CLO_FEMALE` setzt einen anderen).
//! Fehlt er, werden die Tests übersprungen.

use std::path::PathBuf;

use klon3d_koerper::kette::KOERPER;
use klon3d_koerper::koerper::{aus_smis, ordner, Weg};

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
    let m = k.kette.as_ref().expect("Ergebnis der Kette").werte();
    for (j, name) in [(2, "bust_circ"), (6, "waist_circ"), (8, "hip_circ")] {
        assert!((m[j] - smis.wert(name).unwrap()).abs() < 1.0, "{name}: {}", m[j]);
    }
}

/// Nur die Dateien des alten Wegs: die Kette ist nicht ladbar, `anpassen`
/// formt.
#[test]
fn fallback_ohne_kettendaten() {
    let Some((o, smis)) = daten() else { return };
    let alt = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("koerper_nur_alt");
    std::fs::create_dir_all(&alt).expect("Ordner");
    for f in [
        "BaseMesh.obj",
        "ConvertingMat_DETAIL_Advanced_TotalHeight.bd",
        "_DetailFeature_Advanced_Bust_TotalHeight.txt",
    ] {
        std::fs::copy(o.join(f), alt.join(f)).expect(f);
    }
    let k = aus_smis(&alt, &|n| smis.wert(n)).expect("Körper");
    assert_eq!(k.weg, Weg::Anpassen);
    assert!(k.meldungen[0].starts_with("Formungskette nicht ladbar"));
    assert!(k.kette.is_none());
    pruefe_netz(&k);
}
