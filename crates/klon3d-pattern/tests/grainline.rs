#![allow(non_snake_case)] // Tests are named in German

//! Reads `<grainline>` per piece: axis from end anchors, from rotation/length
//! around a centre anchor or around `mx`/`my`, plus the refusal cases.
//!
//! The inline cases use a minimal pattern whose rectangle encloses every test
//! axis, so Seamly2D's push into the seam-line box does not fire and the
//! expected positions are exact. The sweep runs over the repository patterns
//! (their grainlines are invisible) and — when present — over the private
//! pattern library (skipped without, CI has none).

use klon3d_pattern::fadenlauf::Fadenlauf;
use klon3d_pattern::geom::{GeomError, Pt};
use klon3d_pattern::Auswertung;
use std::collections::HashMap;
use std::path::PathBuf;

/// Minimal pattern: an enclosing rectangle plus a centre point; 5 construction
/// points, 6 modelled points, one piece. `grainline` is appended verbatim.
fn muster_mit(grainline: &str) -> klon3d_pattern::Pattern {
    let xml = format!(
        r#"<pattern version="0.6.8">
  <unit>mm</unit>
  <measurements/>
  <draftBlock name="A">
    <calculation>
      <point id="1" type="single" x="-60" y="-60"/>
      <point id="2" type="single" x="200" y="-60"/>
      <point id="3" type="single" x="200" y="160"/>
      <point id="4" type="single" x="-60" y="160"/>
      <point id="5" type="single" x="70" y="50"/>
    </calculation>
    <modeling>
      <point id="11" idObject="1" type="modeling"/>
      <point id="12" idObject="2" type="modeling"/>
      <point id="13" idObject="3" type="modeling"/>
      <point id="14" idObject="4" type="modeling"/>
      <point id="15" idObject="5" type="modeling"/>
    </modeling>
  </draftBlock>
  <pieces>
    <piece id="100" name="T" mx="0" my="0">
      {grainline}
      <nodes>
        <node idObject="11" type="NodePoint"/>
        <node idObject="12" type="NodePoint"/>
        <node idObject="13" type="NodePoint"/>
        <node idObject="14" type="NodePoint"/>
      </nodes>
    </piece>
  </pieces>
</pattern>"#
    );
    let muster = klon3d_pattern::read_str(&xml).unwrap_or_else(|e| panic!("Muster liest nicht: {e}"));
    muster
}

fn ev(muster: &klon3d_pattern::Pattern) -> Auswertung<'_> {
    Auswertung::new(muster, HashMap::new())
}

/// Run `fadenlauf` on the single piece of the inline pattern.
fn pfad(grainline: &str) -> Result<Option<Fadenlauf>, GeomError> {
    let muster = muster_mit(grainline);
    ev(&muster).fadenlauf(&muster.pieces()[0])
}

/// Millimetres per screen pixel (the pattern is in mm; 96 px per inch).
fn mm_pro_px() -> f64 {
    25.4 / 96.0
}

/// Tolerance for px round trips: `k = 96/25.4` is not exactly representable,
/// so an anchored point returns within a digital ULP of itself — numerically
/// identical behaviour to the Python reference reader.
fn nahe(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn kein_Element_oder_unsichtbar_ist_kein_Fadenlauf() {
    let muster = muster_mit("");
    assert!(muster.pieces()[0].grainline.is_none(), "no element — no field");
    assert_eq!(pfad("").unwrap(), None);

    let g = r#"<grainline arrowLength="" arrows="0" length="" mx="0" my="0" rotation="" visible="false"/>"#;
    let m = muster_mit(g);
    assert!(m.pieces()[0].grainline.is_some(), "the element is kept raw");
    assert_eq!(ev(&m).fadenlauf(&m.pieces()[0]).unwrap(), None);

    assert_eq!(pfad(r#"<grainline arrows="0" visible="0"/>"#).unwrap(), None);
}

#[test]
fn Anker_ultimativ_ueber_rotation_und_laenge() {
    // Bottom anchor = point 1 (-60,-60), top = point 3 (200,160);
    // rotation/length are garbage on purpose and must not be evaluated.
    let g = r#"<grainline arrows="0" length="9999" rotation="27" bottomAnchor="1" topAnchor="3" visible="true"/>"#;
    let f = pfad(g).unwrap().unwrap();
    assert!(nahe(f.p1.x, -60.0) && nahe(f.p1.y, -60.0), "p1 = bottom anchor");
    assert!(nahe(f.p2.x, 200.0) && nahe(f.p2.y, 160.0), "p2 = top anchor");
    assert_eq!(f.pfeile, 0);
}

#[test]
fn Anker_hinter_Modellierpunkt() {
    let g = r#"<grainline arrows="0" bottomAnchor="11" topAnchor="13" visible="true"/>"#;
    let f = pfad(g).unwrap().unwrap();
    assert!(nahe(f.p1.x, -60.0) && nahe(f.p1.y, -60.0));
    assert!(nahe(f.p2.x, 200.0) && nahe(f.p2.y, 160.0));
}

#[test]
fn Standardangaben_der_leeren_Elemente() {
    // Nothing filled in: length 1, rotation 90 — near (0,0) with mx/my 0.
    let g = r#"<grainline arrows="0" visible="true"/>"#;
    let f = pfad(g).unwrap().unwrap();
    assert!((f.p1.dist(f.p2) - 1.0).abs() < 1e-9, "length in the unit");
    assert_eq!(f.p1, Pt::new(0.0, 0.0));
}

#[test]
fn Centeranker_ist_die_Achsenmitte() {
    // Centre anchor = point 5 (70,50); rotation 90, length 12 (mm unit).
    let g = r#"<grainline arrows="0" length="12" rotation="90" centerAnchor="15" visible="true"/>"#;
    let f = pfad(g).unwrap().unwrap();
    let mitte = Pt::new((f.p1.x + f.p2.x) / 2.0, (f.p1.y + f.p2.y) / 2.0);
    assert!(mitte.dist(Pt::new(70.0, 50.0)) < 1e-9, "the anchor is the middle");
    assert!((f.p1.dist(f.p2) - 12.0).abs() < 1e-9);
}

#[test]
fn mx_my_sind_Pixel_gelesen() {
    let g = r#"<grainline arrows="0" length="24" rotation="0" mx="20" my="10" visible="true"/>"#;
    let f = pfad(g).unwrap().unwrap();
    let pix = mm_pro_px();
    // mx/my are screen pixels: both raw, straight into the canvas (y down,
    // like the stored frame) — no mirroring happens around them.
    assert!(nahe(f.p1.x, 20.0 * pix), "mx in mm: {}", f.p1.x);
    assert!(nahe(f.p1.y, 10.0 * pix), "my is canvas px too: {}", f.p1.y);
    // rotation 0 runs to the right, the length is a formula in the unit
    // (`length=24` in a mm pattern = 24 mm), not raw pixels like mx/my.
    assert!(nahe(f.p2.x - f.p1.x, 24.0), "length in the unit");
    assert_eq!(f.p2.y, f.p1.y);
}

#[test]
fn Pfeilzahl_wird_ubertragen() {
    for (roh, soll) in [("0", 0u8), ("1", 1), ("2", 2)] {
        let g = format!(r#"<grainline arrows="{roh}" visible="true"/>"#);
        let f = pfad(&g).unwrap().unwrap();
        assert_eq!(f.pfeile, soll, "arrows={roh}");
    }
}

#[test]
fn Unbekannter_Anker_ist_kein_Anker() {
    // Point 777 does not exist — the anchor drops out (Seamly2D catches the
    // bad-id error), the axis starts at mx/my instead.
    let g = r#"<grainline arrows="0" length="10" rotation="0" bottomAnchor="777" mx="0" my="0" visible="true"/>"#;
    let f = pfad(g).unwrap().unwrap();
    assert_eq!(f.p1, Pt::new(0.0, 0.0), "mx/my take over");
    assert!((f.p1.dist(f.p2) - 10.0).abs() < 1e-9);
}

#[test]
fn Fehlerfaelle() {
    // `visible` outside the boolean set
    let e = pfad(r#"<grainline arrows="0" visible="yes"/>"#).unwrap_err();
    assert!(format!("{e}").contains("ist kein Wahrheitswert"), "{e}");

    // `arrows` outside 0/1/2
    let e = pfad(r#"<grainline arrows="3" visible="true"/>"#).unwrap_err();
    assert!(format!("{e}").contains("kennt 0, 1, 2"), "{e}");

    // `mx` not a number
    let e = pfad(r#"<grainline arrows="0" mx="nur_keins" visible="true"/>"#).unwrap_err();
    assert!(format!("{e}").contains("ist keine Zahl"), "{e}");

    // a formula that does not resolve
    let e = pfad(r##"<grainline arrows="0" rotation="#GibtsNicht" visible="true"/>"##).unwrap_err();
    assert!(format!("{e}").contains("rotation="), "{e}");
}

// ─────────────────────────────── Musterbestand ──

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

fn anzahl_grainline(quelltext: &str) -> usize {
    quelltext.matches("<grainline").count()
}

#[test]
fn Repos_Muster_lesen_und_zaehlen() {
    let verwurzelung = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("muster");
    let liste = alle_muster(&verwurzelung);
    assert!(liste.len() >= 8, "Prüfmuster fehlen: {}", verwurzelung.display());
    for datei in &liste {
        let muster =
            klon3d_pattern::read_path(datei).unwrap_or_else(|e| panic!("{}: {e}", datei.display()));
        let quelle = std::fs::read_to_string(datei).unwrap();
        let gesamt = anzahl_grainline(&quelle);
        let teile = muster.pieces();
        let mit = teile.iter().filter(|t| t.grainline.is_some()).count();
        assert_eq!(mit, gesamt, "{}: Element- und Modellzahl", datei.display());
        for (i, t) in teile.iter().enumerate() {
            if t.grainline.is_some() {
                let f = ev(&muster).fadenlauf(t).unwrap();
                assert!(
                    f.is_none(),
                    "{}: Stück {i} doch sichtbar",
                    datei.display()
                );
            }
        }
    }
}

fn bibliothek() -> Option<PathBuf> {
    std::env::var_os("KLON3D_MUSTER")
        .map(PathBuf::from)
        .or_else(|| {
            let p = PathBuf::from(r"D:\ai\claude\latex_schnitte");
            p.is_dir().then_some(p)
        })
}

#[test]
fn Bibliothek_liest_alle_Fadenlaeufe() {
    let Some(root) = bibliothek() else {
        eprintln!("Musterbibliothek fehlt — übersprungen (KLON3D_MUSTER)");
        return;
    };
    let liste = alle_muster(&root);
    assert!(!liste.is_empty(), "Bibliothek leer: {}", root.display());
    let (mut gesamt, mut sichtbare, mut achsen) = (0usize, 0usize, 0usize);
    let mut fehler: Vec<String> = Vec::new();
    for datei in &liste {
        let quelle = std::fs::read_to_string(datei)
            .unwrap_or_else(|e| panic!("{} nicht lesbar: {e}", datei.display()));
        let muster =
            klon3d_pattern::read_path(datei).unwrap_or_else(|e| panic!("{}: {e}", datei.display()));
        let mit = muster
            .pieces()
            .iter()
            .filter(|t| t.grainline.is_some())
            .count();
        assert_eq!(
            mit,
            anzahl_grainline(&quelle),
            "{}: Element- und Modellzahl",
            datei.display()
        );
        gesamt += mit;
        let vergleich = ev(&muster);
        for t in muster.pieces() {
            let Some(g) = &t.grainline else { continue };
            if !matches!(g.visible.as_deref(), Some("true") | Some("1")) {
                continue;
            }
            sichtbare += 1;
            match vergleich.fadenlauf(t) {
                Ok(Some(f)) => {
                    assert!(
                        f.p1.x.is_finite()
                            && f.p1.y.is_finite()
                            && f.p2.x.is_finite()
                            && f.p2.y.is_finite(),
                        "{}: unendliche Achse",
                        datei.display()
                    );
                    achsen += 1;
                }
                Ok(None) => panic!("{}: sichtbar ohne Achse", datei.display()),
                Err(e) => fehler.push(format!("{}: {e}", datei.display())),
            }
        }
    }
    eprintln!(
        "Bibliothek: {gesamt} Fadenläufe, {sichtbare} sichtbar, {achsen} Achsen, {} Fehler",
        fehler.len()
    );
    for f in fehler.iter().take(10) {
        eprintln!("  {f}");
    }
}
