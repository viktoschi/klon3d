//! `klon3d netz` an der Bluse3 v2: Abnahme, Ausgabedateien, Fehlerfälle.
//!
//! Die Netze selbst prüft `klon3d-mesh/tests/bluse3_v2.rs`; hier geht es um
//! den Befehl.

use std::path::PathBuf;
use std::process::{Command, Output};

fn muster(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../klon3d-pattern/tests/muster").join(name)
}

fn klon3d(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_klon3d")).arg("netz").args(args).output().expect("klon3d startet")
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn bluse_mit_nahtliste_und_ausgabe() {
    let ordner = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("netz_cli");
    std::fs::create_dir_all(&ordner).unwrap();
    let (svg, obj, paare) = (ordner.join("bluse.svg"), ordner.join("bluse.obj"), ordner.join("bluse.paare"));
    let o = klon3d(&[
        muster("bluse3_v2.sm2d").to_str().unwrap(),
        "--svg",
        svg.to_str().unwrap(),
        "--nebeneinander",
        "--obj",
        obj.to_str().unwrap(),
        "--paare",
        paare.to_str().unwrap(),
    ]);
    let t = text(&o);
    assert!(o.status.success(), "{t}");
    assert!(t.contains("(24 Nähte)"), "{t}");
    assert!(t.contains("N04L   12+6+13"), "{t}");
    assert!(t.contains("Abnahme: bestanden"), "{t}");
    let obj = std::fs::read_to_string(obj).unwrap();
    assert_eq!(obj.lines().filter(|z| z.starts_with("o ")).count(), 28);
    let paare = std::fs::read_to_string(paare).unwrap();
    let zeilen: Vec<&str> = paare.lines().filter(|z| !z.starts_with('#')).collect();
    assert!(zeilen.iter().all(|z| z.split('\t').count() == 8));
    assert_eq!(zeilen.iter().filter(|z| z.starts_with("N13\t")).count(), 62, "N13: 19+12+12+19 Paare");
    let svg = std::fs::read_to_string(svg).unwrap();
    assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"));
    assert_eq!(svg.matches(r#"<g id="naht-"#).count(), 24);
}

#[test]
fn ohne_naehte_nur_die_netze() {
    let o = klon3d(&[muster("bluse3_v2.sm2d").to_str().unwrap(), "--ohne-naehte", "--kante", "20"]);
    let t = text(&o);
    assert!(o.status.success(), "{t}");
    assert!(t.contains("ohne Nahtliste") && t.contains("Kante 20 mm"), "{t}");
    assert!(!t.contains("Punkte je Teilnaht"), "{t}");
}

/// `--fang 0` rechnet nach CLOs reiner Anteilsregel: Das läuft, aber an der
/// Bluse entstehen Randstücke im Bruchteil eines Millimeters (Befund 33).
#[test]
fn fang_null_ist_clos_regel() {
    let o = klon3d(&[muster("bluse3_v2.sm2d").to_str().unwrap(), "--fang", "0"]);
    let t = text(&o);
    assert!(!o.status.success(), "{t}");
    assert!(t.contains("Fang 0 mm") && t.contains("Abnahme: NICHT bestanden"), "{t}");
    assert!(t.contains("Dreiecke unter 25°"), "{t}");
}

#[test]
fn falsche_angaben() {
    let m = muster("bluse3_v2.sm2d");
    let m = m.to_str().unwrap();
    for (args, teil) in [
        (vec![m, "--winkel", "40"], "über 34°"),
        (vec![m, "--kante", "0"], "keine Zahl > 0"),
        (vec![m, "--fang"], "braucht einen Wert"),
        (vec![m, "--liste", "fehlt.naehte"], "fehlt.naehte"),
        (vec![m, "--gibtsnicht"], "unbekannte Option"),
    ] {
        let o = klon3d(&args);
        assert!(!o.status.success(), "{args:?}");
        assert!(text(&o).contains(teil), "{args:?}: {}", text(&o));
    }
}
