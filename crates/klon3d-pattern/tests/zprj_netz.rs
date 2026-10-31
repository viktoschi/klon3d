#![allow(non_snake_case)] // Tests sind deutsch benannt

//! Prüft das 3D-Netz aus `baPosition3D` gegen das gespeicherte CLO-Projekt.
//!
//! Die Pfade zeigen auf die Prüfprojekte der Anwenderin und auf die fremden
//! `.zprj` in `downloads/clo3d-patterns`. Fehlen sie, werden die Tests
//! übersprungen statt fehlzuschlagen - beides ist nicht Teil des Repos.
//!
//! Die fremden Projekte sind ein Klon von `github.com/tabjy/clo3d-patterns`
//! (CC BY-SA 4.0), Stand `22d0c8a`; gegen diesen Stand gelten die Zählungen.
//! `KLON3D_FREMD` setzt einen anderen Ordner (so holt sie die CI).
//!
//! **Warum diese Tests zählen:** `baPosition3D` ist CLOs fertig simuliertes
//! Ergebnis. Es ist die einzige Referenz im Projekt, gegen die klon3ds eigene
//! Anprobe gemessen werden kann. Stimmen Knoten- und Dreieckszahlen nicht mit
//! `uiVertexCount2D`/`uiTriangleCount` überein, ist der Puffer falsch gedeutet.

use klon3d_pattern::zprj::Projekt;
use std::path::{Path, PathBuf};

const PRUEFPROJEKT: &str =
    r"D:\ai\rea\work\messung\japanese_school_uniform\japanese_school_uniform.zprj";

const FREMD: &str = r"D:\ai\rea\downloads\clo3d-patterns";

fn pruefprojekt() -> Option<PathBuf> {
    let p = PathBuf::from(PRUEFPROJEKT);
    p.is_file().then_some(p)
}

fn fremd_ordner() -> PathBuf {
    std::env::var_os("KLON3D_FREMD").map_or_else(|| PathBuf::from(FREMD), PathBuf::from)
}

fn fremde() -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "zprj") {
                out.push(p);
            }
        }
    }
    walk(&fremd_ordner(), &mut out);
    out.sort();
    out
}

#[test]
fn liest_3D_Netz_des_Pruefprojekts() {
    let Some(p) = pruefprojekt() else {
        eprintln!("übersprungen: {PRUEFPROJEKT} fehlt");
        return;
    };
    let projekt = Projekt::lies(&p).expect("Projekt lesbar");

    // Die Summen stehen so in CLOs eigener 2D-Netzangabe; stimmen sie nicht
    // überein, ist baPosition3D nicht Knoten für Knoten dasselbe Netz.
    let knoten3d: usize = projekt.teile.iter().map(|t| t.position3d.len()).sum();
    let dreiecke3d: usize = projekt.teile.iter().map(|t| t.dreiecke.len()).sum();
    let knoten2d: usize = projekt.teile.iter().map(|t| t.netzknoten).sum();
    let dreiecke2d: usize = projekt.teile.iter().map(|t| t.netzdreiecke).sum();

    assert_eq!(knoten3d, 88_718, "Knoten im 3D-Netz");
    assert_eq!(dreiecke3d, 169_181, "Dreiecke im 3D-Netz");
    assert_eq!(knoten3d, knoten2d, "3D-Knoten müssen zur 2D-Netzangabe passen");
    assert_eq!(dreiecke3d, dreiecke2d, "3D-Dreiecke müssen zur 2D-Netzangabe passen");
}

#[test]
fn jeder_Teil_hat_ein_vollstaendiges_Netz() {
    let Some(p) = pruefprojekt() else {
        eprintln!("übersprungen: {PRUEFPROJEKT} fehlt");
        return;
    };
    let projekt = Projekt::lies(&p).expect("Projekt lesbar");
    for (i, t) in projekt.teile.iter().enumerate() {
        assert_eq!(
            t.position3d.len(),
            t.netzknoten,
            "Teil {i} {}: Knoten 3D gegen uiVertexCount2D",
            t.name
        );
        assert_eq!(
            t.dreiecke.len(),
            t.netzdreiecke,
            "Teil {i} {}: Dreiecke gegen uiTriangleCount",
            t.name
        );
        assert_eq!(
            t.ruheform.len(),
            t.netzknoten,
            "Teil {i} {}: baRest muss je Knoten einen Punkt haben",
            t.name
        );
        // Jeder Dreiecksindex muss in beide Knotenlisten zeigen.
        let n = t.position3d.len();
        for d in &t.dreiecke {
            for &k in d {
                assert!((k as usize) < n, "Teil {i} {}: Index {k} >= {n}", t.name);
            }
            assert!(
                d[0] != d[1] && d[1] != d[2] && d[0] != d[2],
                "Teil {i} {}: entartetes Dreieck {d:?}",
                t.name
            );
        }
    }
}

#[test]
fn das_3D_Netz_ist_vernaeht() {
    // Der eigentliche Beleg: in CLOs gespeichertem Ergebnis treffen sich die
    // beiden Seiten jeder Naht. Läge hier ein flaches oder ungenähtes Netz,
    // wäre der Abstand beliebig groß.
    let Some(p) = pruefprojekt() else {
        eprintln!("übersprungen: {PRUEFPROJEKT} fehlt");
        return;
    };
    let projekt = Projekt::lies(&p).expect("Projekt lesbar");

    let mut abstaende: Vec<f64> = Vec::new();
    for naht in &projekt.naehte {
        // Nur der vernähte Zustand: eine Gruppe, die CLO simuliert hat, mit
        // eingeschaltetem Nahtpaar. Ohne diesen Filter zählt man Nähte mit,
        // die CLO selbst nicht geschlossen hat (siehe Befund 118).
        if !naht.simuliert || !naht.aktiv {
            continue;
        }
        let (Some(ta), Some(tb)) = (
            projekt.teile.get(naht.seite_a.teil),
            projekt.teile.get(naht.seite_b.teil),
        ) else {
            continue;
        };
        let (a, b) = (&naht.seite_a.netzpunkte, &naht.seite_b.netzpunkte);
        if a.len() != b.len() || a.is_empty() {
            continue;
        }
        for (ka, kb) in a.iter().zip(b) {
            let (Some(pa), Some(pb)) = (
                ta.position3d.get(*ka as usize),
                tb.position3d.get(*kb as usize),
            ) else {
                continue;
            };
            abstaende.push(
                ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2) + (pa.z - pb.z).powi(2)).sqrt(),
            );
        }
    }

    assert!(abstaende.len() > 5_000, "nur {} Nahtpunkte", abstaende.len());
    abstaende.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = abstaende[abstaende.len() / 2];
    let max = *abstaende.last().unwrap();
    assert!(
        median < 1.0,
        "Median-Abstand der Nahtpunkte {median:.3} mm - Netz ist nicht vernäht"
    );
    assert!(
        max < 5.0,
        "größter Nahtabstand {max:.3} mm - einzelne Naht sitzt nicht"
    );
}

#[test]
fn nur_der_vernaehte_Zustand_traegt_die_Nahtaussage() {
    // Grenze der Aussage "die Nähte treffen sich": das gilt für Gruppen mit
    // `bDraped = 1`, deren Paar `bActive` hat. Alle anderen sind in CLO
    // entweder nicht simuliert oder abgeschaltet - dort klaffen die Seiten
    // weit auseinander. Über 48 Projekte gemessen:
    //
    // | Gruppe `bDraped` | Paar `bActive` | Paare | Median | über 20 mm |
    // |---|---|---:|---:|---:|
    // | 1 | 1 | 2 739 | 0,00 mm | 1 |
    // | 1 | 0 | 1 305 | 0,01 mm | 54 |
    // | 0 | 1 | 132 | 0,23 mm | 25 |
    // | 0 | 0 | 195 | 2,68 mm | 72 |
    //
    // Der Unterschied zwischen den beiden Zeilen mit `bDraped = 0` ist genau
    // die Aussage: ohne `aktiv` wird der Abstand beliebig.
    let fremd = fremde();
    if fremd.is_empty() {
        eprintln!("übersprungen: {} fehlt", fremd_ordner().display());
        return;
    }
    let mut korb = [[0usize; 2]; 2];
    let (mut summe, mut anzahl) = ([0.0f64; 4], [0usize; 4]);
    for p in std::iter::once(pruefprojekt()).flatten().chain(fremd) {
        let Ok(projekt) = Projekt::lies(&p) else { continue };
        for naht in &projekt.naehte {
            let i = (naht.simuliert as usize) * 2 + naht.aktiv as usize;
            let (Some(ta), Some(tb)) = (
                projekt.teile.get(naht.seite_a.teil),
                projekt.teile.get(naht.seite_b.teil),
            ) else {
                continue;
            };
            let (a, b) = (&naht.seite_a.netzpunkte, &naht.seite_b.netzpunkte);
            if a.len() != b.len() || a.is_empty() {
                continue;
            }
            let mut mitte = 0.0;
            let mut n = 0;
            for (ka, kb) in a.iter().zip(b) {
                let (Some(pa), Some(pb)) = (
                    ta.position3d.get(*ka as usize),
                    tb.position3d.get(*kb as usize),
                ) else {
                    continue;
                };
                mitte += ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2) + (pa.z - pb.z).powi(2))
                    .sqrt();
                n += 1;
            }
            if n > 0 {
                korb[i / 2][i % 2] += 1;
                summe[i] += mitte / n as f64;
                anzahl[i] += 1;
            }
        }
    }

    // Die vier Körbe müssen alle besetzt sein, sonst prüft der Test nichts.
    for i in 0..2 {
        for j in 0..2 {
            assert!(
                korb[i][j] > 0,
                "Korb bDraped={i} bActive={j} ist leer - Prüfmenge zu klein"
            );
        }
    }
    let m = |i: usize| summe[i] / anzahl[i] as f64;
    // Der vernähte Zustand sitzt deutlich enger als jeder andere.
    assert!(
        m(3) < 0.5,
        "bDraped=1 + aktiv: Mittel {:.3} mm, erwartet unter 0,5",
        m(3)
    );
    for (i, was) in [(0, "bDraped=0 + inaktiv"), (2, "bDraped=0 + aktiv")] {
        assert!(
            m(i) > 3.0 * m(3),
            "{was}: Mittel {:.3} mm liegt nicht deutlich über dem vernähten Zustand",
            m(i)
        );
    }
    eprintln!(
        "Nahtabstand im Mittel: draped+aktiv {:.3} mm, draped+inaktiv {:.3} mm, \
         undraped+aktiv {:.3} mm, undraped+inaktiv {:.3} mm",
        m(3),
        m(1),
        m(2),
        m(0)
    );
}

#[test]
fn flache_Teile_liegen_in_einer_Ebene() {
    // **Nicht** über `bDraped` prüfen: das Feld sagt die Flachheit nicht
    // voraus. Über alle 47 fremden Projekte gezählt (2 109 Teile):
    //
    // | `bDraped` | räumlich | flach |
    // |---|---|---:|---:|
    // | 1 | 1 831 | **139** |
    // | 0 | **78** | 61 |
    //
    // "Flach" heißt hier: Spanne des z unter 0,01 mm. Über alle 47 fremden
    // Projekte sind das **198** von 2 109 Teilen.
    // Die belastbare Aussage ist die Umkehrung: liegt das Netz in einer
    // Ebene, steht dort ein praktisch konstanter z-Wert (`gloves`/`mask`:
    // 200,0 mm). "Praktisch" heißt unter 0,01 mm Spanne - die gespeicherten
    // Werte streuen um Bruchteile eines Tausendstelmillimeters (gemessen:
    // 0,00095 mm bei `2022_racing_miku_v2`), also messbar, aber ohne jede
    // geometrische Bedeutung. Ein flaches Netz ist damit erkennbar, ohne
    // `bDraped` zu deuten.
    let fremd = fremde();
    if fremd.is_empty() {
        eprintln!("übersprungen: {} fehlt", fremd_ordner().display());
        return;
    }
    let mut flach = 0;
    for p in fremd {
        let Ok(projekt) = Projekt::lies(&p) else { continue };
        for t in &projekt.teile {
            if t.position3d.len() < 10 {
                continue;
            }
            let z: Vec<f64> = t.position3d.iter().map(|q| q.z).collect();
            let tiefe = z.iter().cloned().fold(f64::MIN, f64::max)
                - z.iter().cloned().fold(f64::MAX, f64::min);
            if tiefe < 0.01 {
                flach += 1;
            }
        }
    }
    assert_eq!(flach, 198, "flache Teile über alle fremden Projekte");
}

#[test]
fn liest_alle_fremden_projekte_mit_Netz() {
    let dateien = fremde();
    if dateien.is_empty() {
        eprintln!("übersprungen: {} fehlt", fremd_ordner().display());
        return;
    }
    let (mut gelesen, mut mit_netz) = (0, 0);
    for p in &dateien {
        let projekt = Projekt::lies(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        gelesen += 1;
        let knoten: usize = projekt.teile.iter().map(|t| t.position3d.len()).sum();
        if knoten > 0 {
            mit_netz += 1;
        }
        // Auch in fremden Builds muss die Knotenzahl zur 2D-Angabe passen.
        for t in &projekt.teile {
            assert_eq!(
                t.position3d.len(),
                t.netzknoten,
                "{}: Teil {} Knoten",
                p.display(),
                t.name
            );
            assert_eq!(
                t.dreiecke.len(),
                t.netzdreiecke,
                "{}: Teil {} Dreiecke",
                p.display(),
                t.name
            );
        }
    }
    assert_eq!(gelesen, 47, "fremde Projekte");
    assert_eq!(mit_netz, 47, "Projekte mit 3D-Netz");
}

// ── Der Körper aus der .avt ──

/// Der Körper steckt als `.avt` im Projekt: `body` mit 31 422 Knoten und
/// 62 658 Dreiecken - genau die Zahlen des Standardkörpers aus Befund 74.
#[test]
fn liest_den_Koerper_aus_der_avt() {
    let Some(p) = pruefprojekt() else {
        eprintln!("übersprungen: {PRUEFPROJEKT} fehlt");
        return;
    };
    let projekt = Projekt::lies(&p).expect("Projekt lesbar");
    let k = projekt.koerper.as_ref().expect("Körper aus der .avt");
    assert_eq!(k.name, "body");
    assert_eq!(k.punkte.len(), 31_422, "Knoten");
    assert_eq!(k.dreiecke.len(), 62_658, "Dreiecke");

    // Alle Dreiecke zeigen in die Knotenliste - sonst ist baIndex falsch
    // gedeutet (in der .dan ohne Array-Kopf, reine Rohdaten).
    let n = k.punkte.len() as u32;
    assert!(
        !k.dreiecke.iter().flatten().any(|&i| i >= n),
        "Dreiecksindex außerhalb der Knoten"
    );

    // Die .dan führt alle Körperteile; body ist eines davon.
    assert_eq!(k.teile.len(), 9, "listSkinController");
    let namen: Vec<&str> = k.teile.iter().map(|t| t.name.as_str()).collect();
    assert!(namen.contains(&"body"), "body fehlt in {namen:?}");
    assert!(namen.contains(&"hair_Shape"), "hair_Shape fehlt in {namen:?}");
    let body = k.teile.iter().find(|t| t.name == "body").expect("body");
    assert_eq!(body.knoten, 31_422);
    assert_eq!(body.dreiecke, 62_658);
    assert!(body.collidable, "body muss kollidierbar sein");
}

/// Der Körper steht in **Millimetern im Avatarraum** (y = Höhe) - dasselbe
/// Bezugssystem wie CLOs Kleidungsnetz. Die Spannen belegen das: 1,80 m hoch,
/// A-Pose mit **22° von der Senkrechten** abgespreizten Armen (x bis 491 mm;
/// die Fingerspitze bei y = 736 ist **nicht** die Schulter, siehe Befund 119,
/// Abschnitt 4.1).
#[test]
fn der_Koerper_steht_im_Avatarraum() {
    let Some(p) = pruefprojekt() else {
        eprintln!("übersprungen: {PRUEFPROJEKT} fehlt");
        return;
    };
    let projekt = Projekt::lies(&p).expect("Projekt lesbar");
    let k = projekt.koerper.as_ref().expect("Körper");
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in &k.punkte {
        for a in 0..3 {
            lo[a] = lo[a].min([p.x, p.y, p.z][a]);
            hi[a] = hi[a].max([p.x, p.y, p.z][a]);
        }
    }
    // y ist die Höhe: 1,8 m, nicht 0,18 m und nicht 1800.
    assert!(
        (hi[1] - lo[1]) > 1700.0 && (hi[1] - lo[1]) < 1900.0,
        "Höhe {} mm - nicht in Millimetern?",
        hi[1] - lo[1]
    );
    // A-Pose: die Arme sind waagerecht, die Spannweite ist größer als die
    // Körperbreite einer entspannten Pose.
    assert!(
        (hi[0] - lo[0]) > 800.0,
        "Spannweite {} mm - keine A-Pose?",
        hi[0] - lo[0]
    );
    // Der Körper liegt dort, wo das Kleidungsnetz liegt (gleicher Raum).
    let kleid: Vec<&klon3d_pattern::zprj::Punkt3> =
        projekt.teile.iter().filter(|t| !t.position3d.is_empty()).flat_map(|t| &t.position3d).collect();
    assert!(!kleid.is_empty(), "kein Kleidungsnetz");
    let ky: Vec<f64> = kleid.iter().map(|p| p.y).collect();
    let (klo, khi) = (
        ky.iter().cloned().fold(f64::MAX, f64::min),
        ky.iter().cloned().fold(f64::MIN, f64::max),
    );
    assert!(
        klo > lo[1] && khi < hi[1],
        "Kleidung (y {klo:.0}..{khi:.0}) liegt nicht innerhalb des Körpers (y {:.0}..{:.0})",
        lo[1],
        hi[1]
    );
}

/// 40 von 47 fremden Projekten führen eine `.avt`; die übrigen 7 nicht.
/// Beide Fälle sind in Ordnung - ein Projekt ohne Körper ist kein Lesefehler.
/// Geprüft wird, dass der Körper, wenn er da ist, **dieselbe** Topologie hat:
/// CLO benutzt für alle diese Projekte denselben Standardkörper.
#[test]
fn fremde_Projekte_mit_und_ohne_Koerper() {
    let liste = fremde();
    if liste.len() < 47 {
        eprintln!("übersprungen: nur {} fremde Projekte", liste.len());
        return;
    }
    let (mut mit, mut ohne) = (0, 0);
    for p in liste.iter().take(47) {
        let projekt = Projekt::lies(p).expect("fremdes Projekt lesbar");
        match &projekt.koerper {
            None => ohne += 1,
            Some(k) => {
                mit += 1;
                assert_eq!(k.name, "body", "{}", p.display());
                assert_eq!(k.punkte.len(), 31_422, "{}: Knoten", p.display());
                assert_eq!(k.dreiecke.len(), 62_658, "{}: Dreiecke", p.display());
                let n = k.punkte.len() as u32;
                assert!(
                    !k.dreiecke.iter().flatten().any(|&i| i >= n),
                    "{}: Dreiecksindex außerhalb",
                    p.display()
                );
            }
        }
    }
    assert_eq!(mit, 40, "Projekte mit .avt");
    assert_eq!(ohne, 7, "Projekte ohne .avt");
}