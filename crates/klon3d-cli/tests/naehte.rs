//! `klon3d naehte` an der Bluse3 v2: Abnahme, Rückgabewert, Fehlerfälle.
//!
//! Die Längen selbst prüft `klon3d-pattern/tests/naehte.rs`; hier geht es um
//! den Befehl.

use std::path::PathBuf;
use std::process::{Command, Output};

fn muster(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../klon3d-pattern/tests/muster").join(name)
}

fn klon3d(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_klon3d"))
        .arg("naehte")
        .args(args)
        .output()
        .expect("klon3d startet")
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn nahtliste_neben_dem_muster_wird_gefunden() {
    let o = klon3d(&[muster("bluse3_v2.sm2d").to_str().unwrap()]);
    let t = text(&o);
    assert!(o.status.success(), "{t}");
    assert!(t.contains("(24 Nähte)"), "{t}");
    assert!(t.contains("größte Abweichung: N07L Δ -0.0737"), "{t}");
    assert!(t.contains("Abnahme |Δ| ≤ 0.08 cm: bestanden (24 von 24"), "{t}");
    assert!(t.contains("N04L Seite A: RW_L"), "{t}");
}

#[test]
fn enge_toleranz_faellt_durch() {
    let o = klon3d(&[muster("bluse3_v2.sm2d").to_str().unwrap(), "--toleranz", "0,05"]);
    let t = text(&o);
    assert!(!o.status.success(), "{t}");
    assert!(t.contains("NICHT bestanden (22 von 24"), "{t}");
    assert_eq!(t.matches("  ÜBER").count(), 2, "{t}");
}

#[test]
fn ohne_nahtliste_nur_kanten() {
    let o = klon3d(&[muster("bluse3_v1.sm2d").to_str().unwrap(), "--kanten"]);
    let t = text(&o);
    assert!(o.status.success(), "{t}");
    assert!(t.contains("keine Nahtliste"), "{t}");
    assert!(t.contains("RT_L         → RTM1_L*"), "{t}");
    // Ohne --kanten ist eine fehlende Liste ein Fehler.
    assert!(!klon3d(&[muster("bluse3_v1.sm2d").to_str().unwrap()]).status.success());
}

#[test]
fn fehler_in_der_liste_werden_gemeldet() {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let liste = tmp.join("falsch.naehte");
    std::fs::write(&liste, "X1 | kaputt | Untercup Links: VX_L NIRGENDS v | Band Mitte: VC8_L VCX_L r\n").unwrap();
    let o = klon3d(&[muster("bluse3_v2.sm2d").to_str().unwrap(), "--liste", liste.to_str().unwrap()]);
    let t = text(&o);
    assert!(!o.status.success(), "{t}");
    assert!(t.contains("Naht X1") && t.contains("kein Punktknoten NIRGENDS"), "{t}");

    let o = klon3d(&[muster("bluse3_v2.sm2d").to_str().unwrap(), "--liste", tmp.join("fehlt.naehte").to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(text(&o).contains("Nahtliste fehlt"));
}
