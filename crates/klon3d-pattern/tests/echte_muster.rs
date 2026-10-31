#![allow(non_snake_case)] // Tests sind deutsch benannt

//! Prüft den Parser gegen echte Muster.
//!
//! Die Pfade zeigen auf die Musterbibliothek des Nutzers. Fehlen sie,
//! werden die Tests übersprungen statt fehlzuschlagen — die Bibliothek ist
//! nicht Teil des Repositories.

use klon3d_pattern::read_path;
use std::path::PathBuf;

fn bibliothek() -> Option<PathBuf> {
    let p = PathBuf::from(r"D:\ai\claude\latex_schnitte");
    p.is_dir().then_some(p)
}

fn alle_muster(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "sm2d") {
                out.push(p);
            }
        }
    }
    walk(root, &mut out);
    out.sort();
    out
}

/// Das Prüfmodell: Bluse mit Plunge und Cup, parametrisch.
const PRUEFMODELL: &str =
    r"Bluse3_Plunge_Cups_parametrisch_v2.sm2d";

#[test]
fn liest_Pruefmodell_vollstaendig() {
    let Some(root) = bibliothek() else {
        eprintln!("Musterbibliothek fehlt — uebersprungen");
        return;
    };
    let datei = alle_muster(&root)
        .into_iter()
        .find(|p| p.file_name().is_some_and(|n| n == PRUEFMODELL))
        .unwrap_or_else(|| panic!("{PRUEFMODELL} nicht in der Bibliothek"));

    let p = read_path(&datei).unwrap_or_else(|e| panic!("{}: {e}", datei.display()));

    // Grundstruktur
    assert!(!p.version.is_empty());
    assert_eq!(p.unit_to_mm().unwrap(), 10.0, "Bluse ist in cm");

    // Zählwerte gegen die Vorab-Analyse dieser Datei
    assert_eq!(p.draft_blocks.len(), 6, "6 Konstruktionsbloecke");
    // 480 Konstruktionspunkte ueber 6 Bloecke, gezaehlt ueber die ganze
    // Datei. `point_count` addiert calculation + modeling je Block.
    assert_eq!(p.point_count(), 480);

    let arten = p.point_kinds();
    let von = |t: &str| arten.iter().find(|(k, _)| k == t).map(|(_, n)| *n).unwrap_or(0);
    assert_eq!(von("endLine"), 175);
    assert_eq!(von("modeling"), 168);
    assert_eq!(von("alongLine"), 49);
    assert_eq!(von("curveIntersectAxis"), 16);
    assert_eq!(von("single"), 12);
    assert_eq!(von("lineIntersectAxis"), 4);
    assert_eq!(von("cutSpline"), 4);
    assert_eq!(von("pointOfContact"), 2);
    assert_eq!(von("cutSplinePath"), 50);

    // Jeder Punkttyp muss eine id tragen, sonst ist die Kontur nicht
    // auflösbar.
    for b in &p.draft_blocks {
        if let Some(c) = &b.calculation {
            for pt in &c.points {
                assert!(!pt.id.is_empty(), "Punkt ohne id in Block {:?}", b.name);
            }
        }
    }

    // Formeln sind Text, keine Zahlen — genau das muss später die Engine
    // auswerten.
    let mit_formel = p
        .draft_blocks
        .iter()
        .filter_map(|b| b.calculation.as_ref())
        .flat_map(|c| c.points.iter())
        .filter(|pt| pt.length.as_deref().is_some_and(|f| f.contains('#')))
        .count();
    assert!(mit_formel > 100, "nur {mit_formel} Punkte mit Messbezug");
}

#[test]
fn liest_alle_muster_der_bibliothek() {
    let Some(root) = bibliothek() else {
        eprintln!("Musterbibliothek fehlt — uebersprungen");
        return;
    };
    let muster = alle_muster(&root);
    assert!(
        muster.len() > 100,
        "erwartet >100 Muster, gefunden {}",
        muster.len()
    );

    let (mut ok, mut fehler) = (0usize, Vec::new());
    for m in &muster {
        match read_path(m) {
            Ok(p) => {
                // Grundannahmen, die für die Geometrie gelten müssen.
                assert!(!p.version.is_empty(), "{}: keine Version", m.display());
                ok += 1;
            }
            Err(e) => fehler.push(format!("{}: {e}", m.file_name().unwrap().to_string_lossy())),
        }
    }

    println!("  gelesen: {ok}/{}", muster.len());
    if !fehler.is_empty() {
        println!("  fehlgeschlagen:");
        for f in fehler.iter().take(10) {
            println!("    {f}");
        }
    }
    assert!(
        fehler.is_empty(),
        "{} von {} Mustern nicht lesbar",
        fehler.len(),
        muster.len()
    );
}

#[test]
fn teilstuecke_haben_geschlossene_Kontur() {
    let Some(root) = bibliothek() else {
        eprintln!("Musterbibliothek fehlt — uebersprungen");
        return;
    };
    let datei = alle_muster(&root)
        .into_iter()
        .find(|p| p.file_name().is_some_and(|n| n == PRUEFMODELL))
        .unwrap();

    let p = read_path(&datei).unwrap();
    let teile = p.pieces();
    assert!(teile.len() >= 10, "erwartet >=10 Teile, gefunden {}", teile.len());

    for t in teile {
        let name = t.name.as_deref().unwrap_or("<ohne Namen>");
        assert!(!t.id.is_empty(), "Teil ohne id: {name}");
        let knoten = &t.nodes.as_ref().expect("Teil ohne Knoten").items;
        assert!(knoten.len() >= 3, "Teil {name} hat nur {} Knoten", knoten.len());

        // Die Kette muss geschlossen sein: aufeinanderfolgende Knoten müssen
        // End- und Startpunkt teilen. Geprüft wird das in geom.rs; hier nur
        // die strukturelle Voraussetzung.
        for k in knoten {
            assert!(!k.id_object.is_empty(), "Knoten ohne idObject in {name}");
            assert!(
                // Vier Typen kommen in der Bibliothek vor.
                matches!(
                    k.kind.as_str(),
                    "NodePoint" | "NodeSpline" | "NodeSplinePath" | "NodeArc"
                ),
                "unbekannter Knotentyp {} in {name}",
                k.kind
            );
        }
    }
}