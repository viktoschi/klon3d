//! `klon3d dxf` gegen Seamlys AAMA-Export des Rocks v34.
//!
//! Die Sollwerte sind Seamlys Nahtlinien: Ebene 8 in
//! `Rock_v34_AAMA_pieces.dxf` (Export vom 19.10.2026,
//! `latex_schnitte/werkstatt/rock/2026-10-19_clo_import/dxf/`), je Teil die
//! längste geschlossene Polylinie — Länge und Hülle in mm, dazu die Zahl der
//! Passzeichen (Seamly schreibt jedes zweimal auf Ebene 4).
//!
//! Der Test lässt das Programm schreiben und liest die DXF mit einem eigenen
//! kleinen Leser zurück, unabhängig vom Schreiber.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Blockname, Nahtlänge, Breite, Höhe (mm), Passzeichen.
const SEAMLY: [(&str, f64, f64, f64, usize); 22] = [
    ("2_VORNE_RECHTS_-_OBERE_LAGE_AN_CF", 2546.62, 572.98, 936.41, 16),
    ("3_VORNE_LINKS_-_UNTERE_LAGE_AN_CF", 2546.62, 572.98, 936.41, 16),
    ("4_HINTEN_RECHTS_-_OBERE_LAGE_AN_CB", 2836.79, 980.67, 937.67, 16),
    ("5_HINTEN_LINKS_-_UNTERE_LAGE_AN_CB", 2836.79, 980.67, 937.67, 16),
    ("BL1_HL_SCHLITZBLENDE_HINTEN_LINKS", 1860.00, 10.00, 920.00, 7),
    ("BL1_HR_SCHLITZBLENDE_HINTEN_RECHTS", 1860.00, 10.00, 920.00, 7),
    ("BL1_VL_SCHLITZBLENDE_VORNE_LINKS", 1860.00, 10.00, 920.00, 7),
    ("BL1_VR_SCHLITZBLENDE_VORNE_RECHTS", 1860.00, 10.00, 920.00, 7),
    ("DN_L_DONUT_SCHLITZSPITZE_LINKS", 94.25, 30.00, 30.00, 0),
    ("DN_R_DONUT_SCHLITZSPITZE_RECHTS", 94.25, 30.00, 30.00, 0),
    ("ST_HL_SAUMTRIM_HINTEN_LINKS", 1629.60, 10.00, 804.80, 1),
    ("ST_HR_SAUMTRIM_HINTEN_RECHTS", 1729.60, 10.00, 854.80, 2),
    ("ST_VL_SAUMTRIM_VORNE_LINKS", 1194.59, 10.00, 587.30, 1),
    ("ST_VR_SAUMTRIM_VORNE_RECHTS", 1294.59, 10.00, 637.30, 2),
    ("VS1_HL_LAENGSVERSTAERKUNG_SCHLITZSPITZE_HINTEN_LINKS", 280.00, 20.00, 120.00, 0),
    ("VS1_HR_LAENGSVERSTAERKUNG_SCHLITZSPITZE_HINTEN_RECHTS", 280.00, 20.00, 120.00, 0),
    ("VS1_VL_LAENGSVERSTAERKUNG_SCHLITZSPITZE_VORNE_LINKS", 280.00, 20.00, 120.00, 0),
    ("VS1_VR_LAENGSVERSTAERKUNG_SCHLITZSPITZE_VORNE_RECHTS", 280.00, 20.00, 120.00, 0),
    ("VS2_HL_QUERRIEGEL_SCHLITZSPITZE_HINTEN_LINKS", 100.00, 30.00, 20.00, 0),
    ("VS2_HR_QUERRIEGEL_SCHLITZSPITZE_HINTEN_RECHTS", 100.00, 30.00, 20.00, 0),
    ("VS2_VL_QUERRIEGEL_SCHLITZSPITZE_VORNE_LINKS", 100.00, 30.00, 20.00, 0),
    ("VS2_VR_QUERRIEGEL_SCHLITZSPITZE_VORNE_RECHTS", 100.00, 30.00, 20.00, 0),
];

/// Der Bund. Bis 20.10.2026 hatte er 22,3 m Naht statt 1,5 m: Der Leser hing
/// die Knoten der Klebezonen-Innenpfade aus dem Block „02 Rock“ an ihn an.
const BUND: (&str, f64, f64, f64, usize) = ("1_BUND_-_UMSCHLAG,_GANZER_UMFANG,_NAHT_HINTEN", 1513.06, 701.26, 96.44, 17);

const TOL_MM: f64 = 0.1;

type P = [f64; 2];

#[derive(Debug, Default)]
struct Polylinie {
    ebene: String,
    zu: bool,
    punkte: Vec<P>,
}

#[derive(Debug, Default)]
struct Block {
    name: String,
    polylinien: Vec<Polylinie>,
    linien: Vec<(String, P, P)>,
    texte: Vec<(String, String)>,
}

#[derive(Debug, Default)]
struct Dxf {
    abschnitte: Vec<String>,
    bloecke: Vec<Block>,
    inserts: Vec<String>,
}

impl Dxf {
    fn block(&self, name: &str) -> &Block {
        self.bloecke
            .iter()
            .find(|b| b.name == name)
            .unwrap_or_else(|| panic!("Block {name} fehlt"))
    }

    fn teile(&self) -> Vec<&str> {
        self.bloecke
            .iter()
            .map(|b| b.name.as_str())
            .filter(|n| !n.starts_with('$'))
            .collect()
    }
}

/// Liest eine R12-DXF: Gruppencode und Wert je Zeilenpaar, Windows-1252.
fn lies(pfad: &Path) -> Dxf {
    let roh = std::fs::read(pfad).unwrap_or_else(|e| panic!("{}: {e}", pfad.display()));
    // Für die Prüfung genügt Latin-1; die Namen des Rocks sind ASCII.
    let text: String = roh.iter().map(|&b| b as char).collect();
    assert!(text.ends_with("  0\r\nEOF\r\n"), "Datei endet nicht mit EOF und CRLF");
    let zeilen: Vec<&str> = text.trim_end_matches("\r\n").split("\r\n").collect();
    assert_eq!(zeilen.len() % 2, 0, "ungerade Zeilenzahl");

    // In Elemente gliedern: jedes beginnt mit Code 0.
    let mut elemente: Vec<(&str, Vec<(i32, &str)>)> = Vec::new();
    for z in zeilen.chunks_exact(2) {
        let code: i32 = z[0].trim().parse().unwrap_or_else(|_| panic!("Gruppencode {:?}", z[0]));
        match (code, elemente.last_mut()) {
            (0, _) => elemente.push((z[1], Vec::new())),
            (_, Some(e)) => e.1.push((code, z[1])),
            (_, None) => {} // Kommentar vor dem ersten Abschnitt
        }
    }

    let mut dxf = Dxf::default();
    let mut abschnitt = String::new();
    for (typ, codes) in &elemente {
        let wert = |c: i32| codes.iter().find(|(k, _)| *k == c).map(|(_, v)| *v).unwrap_or("");
        let zahl = |c: i32| -> f64 {
            wert(c).trim().parse().unwrap_or_else(|_| panic!("{typ}: Code {c} keine Zahl"))
        };
        let in_block = abschnitt == "BLOCKS";
        match *typ {
            "SECTION" => {
                abschnitt = wert(2).to_string();
                dxf.abschnitte.push(abschnitt.clone());
            }
            "ENDSEC" => abschnitt.clear(),
            "BLOCK" if in_block => dxf.bloecke.push(Block { name: wert(2).into(), ..Default::default() }),
            "POLYLINE" if in_block => dxf.bloecke.last_mut().unwrap().polylinien.push(Polylinie {
                ebene: wert(8).into(),
                zu: wert(70).trim().parse::<i32>().unwrap_or(0) & 1 == 1,
                punkte: Vec::new(),
            }),
            "VERTEX" if in_block => {
                let b = dxf.bloecke.last_mut().unwrap();
                b.polylinien.last_mut().unwrap().punkte.push([zahl(10), zahl(20)]);
            }
            "LINE" if in_block => {
                let l = (wert(8).to_string(), [zahl(10), zahl(20)], [zahl(11), zahl(21)]);
                dxf.bloecke.last_mut().unwrap().linien.push(l);
            }
            "TEXT" if in_block => {
                let t = (wert(8).to_string(), wert(1).to_string());
                dxf.bloecke.last_mut().unwrap().texte.push(t);
            }
            "INSERT" if abschnitt == "ENTITIES" => dxf.inserts.push(wert(2).into()),
            _ => {}
        }
    }
    dxf
}

fn rock() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../klon3d-pattern/tests/muster/rock_2schlitze_v34.sm2d")
}

/// Ruft `klon3d dxf` auf und liest das Ergebnis.
fn klon3d_dxf(datei: &str, mehr: &[&str]) -> Dxf {
    let aus = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(datei);
    let _ = std::fs::remove_file(&aus);
    let r = Command::new(env!("CARGO_BIN_EXE_klon3d"))
        .arg("dxf")
        .arg(rock())
        .arg(&aus)
        .args(mehr)
        .output()
        .expect("klon3d startet");
    assert!(
        r.status.success(),
        "klon3d dxf: {}\n{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    lies(&aus)
}

fn abstand(a: P, b: P) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn ring_laenge(p: &[P]) -> f64 {
    (0..p.len()).map(|i| abstand(p[i], p[(i + 1) % p.len()])).sum()
}

fn abstand_zum_ring(q: P, p: &[P]) -> f64 {
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = dx * dx + dy * dy;
            let t = if l2 == 0.0 { 0.0 } else { (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
            abstand(q, [a[0] + t * dx, a[1] + t * dy])
        })
        .fold(f64::INFINITY, f64::min)
}

/// Gerade-ungerade-Regel mit waagerechtem Strahl.
fn innen(q: P, p: &[P]) -> bool {
    let mut drin = false;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            drin = !drin;
        }
    }
    drin
}

/// Prüft ein Teil gegen Seamlys Nahtlinie.
fn pruefe_teil(dxf: &Dxf, (name, laenge, breite, hoehe, kerben): (&str, f64, f64, f64, usize)) {
    let b = dxf.block(name);

    // Genau eine Kante: geschlossen, Ebene 1. Keine Innenlinien (Ebene 8).
    assert_eq!(b.polylinien.len(), 1, "{name}: Polylinien");
    let k = &b.polylinien[0];
    assert_eq!(k.ebene, "1", "{name}: Ebene der Kante");
    assert!(k.zu, "{name}: Kante nicht geschlossen");
    assert!(k.punkte.len() >= 3, "{name}: {} Punkte", k.punkte.len());
    let p = &k.punkte;

    let l = ring_laenge(p);
    let xs = || p.iter().map(|q| q[0]);
    let ys = || p.iter().map(|q| q[1]);
    let b_ist = xs().fold(f64::MIN, f64::max) - xs().fold(f64::MAX, f64::min);
    let h_ist = ys().fold(f64::MIN, f64::max) - ys().fold(f64::MAX, f64::min);
    assert!((l - laenge).abs() <= TOL_MM, "{name}: Naht {l:.3} mm, Seamly {laenge}");
    assert!((b_ist - breite).abs() <= TOL_MM, "{name}: Breite {b_ist:.3} mm, Seamly {breite}");
    assert!((h_ist - hoehe).abs() <= TOL_MM, "{name}: Höhe {h_ist:.3} mm, Seamly {hoehe}");

    // Passzeichen: Ebene 4, 4 mm, Fuß auf der Kante, Ende im Teil.
    assert_eq!(b.linien.len(), kerben, "{name}: Passzeichen");
    for (ebene, fuss, ende) in &b.linien {
        assert_eq!(ebene, "4", "{name}: Ebene eines Passzeichens");
        assert!((abstand(*fuss, *ende) - 4.0).abs() < 1e-5, "{name}: Passzeichen {} mm", abstand(*fuss, *ende));
        assert!(abstand_zum_ring(*fuss, p) < 1e-5, "{name}: Fuß {fuss:?} nicht auf der Kante");
        assert!(innen(*ende, p), "{name}: Passzeichen bei {fuss:?} zeigt nach außen");
    }

    // Teilename als Text.
    assert!(
        b.texte.iter().any(|(e, t)| e == "1" && t.replace(' ', "_").to_uppercase() == name),
        "{name}: Name fehlt, Texte {:?}",
        b.texte
    );
}

#[test]
fn rock_wie_seamly() {
    let dxf = klon3d_dxf("rock_alle.dxf", &[]);
    assert_eq!(dxf.abschnitte, ["HEADER", "TABLES", "BLOCKS", "ENTITIES"]);

    let teile = dxf.teile();
    assert_eq!(teile.len(), 23, "{teile:?}");
    assert!(teile.contains(&BUND.0));
    // Je Teil genau ein INSERT, in derselben Reihenfolge.
    assert_eq!(dxf.inserts, teile);

    for soll in SEAMLY {
        pruefe_teil(&dxf, soll);
    }
}

#[test]
fn bund_wie_seamly() {
    pruefe_teil(&klon3d_dxf("rock_bund.dxf", &["--teile", "1_"]), BUND);
}

#[test]
fn auswahl_nach_namensanfang() {
    // Leerzeichen und `_` gelten gleich, Groß/Klein egal.
    let dxf = klon3d_dxf("rock_auswahl.dxf", &["--teile", "2,3 vorne,bl1"]);
    assert_eq!(
        dxf.teile(),
        [
            "2_VORNE_RECHTS_-_OBERE_LAGE_AN_CF",
            "3_VORNE_LINKS_-_UNTERE_LAGE_AN_CF",
            "BL1_VR_SCHLITZBLENDE_VORNE_RECHTS",
            "BL1_HR_SCHLITZBLENDE_HINTEN_RECHTS",
            "BL1_VL_SCHLITZBLENDE_VORNE_LINKS",
            "BL1_HL_SCHLITZBLENDE_HINTEN_LINKS",
        ]
    );
    assert_eq!(dxf.inserts, dxf.teile());
    for soll in SEAMLY.iter().filter(|s| dxf.teile().contains(&s.0)) {
        pruefe_teil(&dxf, *soll);
    }
}
