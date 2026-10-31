//! Punkttypen und Werkzeuge, die die Hexen-Muster nicht brauchten, wohl aber
//! Bolero, Strumpf und L036 (Befund 99): `cutArc` mit Teilbögen, Schnitt
//! zweier Bögen und zweier Kurven, Operationen (Spiegeln, Drehen,
//! Verschieben) mit Punkt- und Kurvenkopien, Teilkurven geschnittener
//! Splines, `trueDarts`, und die Umrechnung der Maße in die Mustereinheit.
//! Sollwerte von Hand nachgerechnet.

use std::collections::HashMap;

use klon3d_pattern::geom::{lies_messungen_str, Auswertung, Pt};
use klon3d_pattern::read_str;

fn muster(rechnung: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pattern>
  <version>0.7.4</version>
  <unit>cm</unit>
  <draftBlock name="A">
    <calculation>
{rechnung}
    </calculation>
    <modeling/>
    <pieces/>
  </draftBlock>
</pattern>"#
    )
}

fn nah(p: Pt, x: f64, y: f64, tol: f64) {
    assert!((p.x - x).abs() < tol && (p.y - y).abs() < tol, "({:.6}, {:.6}) statt ({x}, {y})", p.x, p.y);
}

#[test]
fn masse_in_die_mustereinheit() {
    let cm = r#"<smis><unit>cm</unit><body-measurements><m name="height" value="160"/></body-measurements></smis>"#;
    let mm = r#"<smis><unit>mm</unit><body-measurements><m name="height" value="1600"/></body-measurements></smis>"#;
    assert_eq!(lies_messungen_str(cm, "mm").unwrap()["height"], 1600.0);
    assert_eq!(lies_messungen_str(cm, "cm").unwrap()["height"], 160.0);
    assert_eq!(lies_messungen_str(mm, "cm").unwrap()["height"], 160.0);
}

/// Bogen um C mit Radius 10 von 0° bis 90° (am Bildschirm gegen den
/// Uhrzeiger, in gespeicherten Werten also nach oben, y < 0), Länge 5π.
/// Bei Länge 5 liegt der Schnitt bei 5/10 rad = 28,6479°; die Teilbögen
/// (Punkt-id + 1, + 2) sind 5 und 5π − 5 lang.
#[test]
fn bogen_schneiden_und_teilboegen() {
    let x = muster(
        r#"<point id="1" name="C" type="single" x="0" y="0" mx="0" my="0"/>
<arc id="2" type="simple" center="1" radius="10" angle1="0" angle2="90"/>
<point id="10" name="K" type="cutArc" arc="2" length="5" mx="0" my="0"/>
<point id="20" name="R" type="cutArc" arc="2" length="5" direction="backward" mx="0" my="0"/>
<point id="30" name="L1" type="endLine" basePoint="1" angle="0" length="Arc_C_11" mx="0" my="0"/>
<point id="31" name="L2" type="endLine" basePoint="1" angle="0" length="Arc_C_12" mx="0" my="0"/>"#,
    );
    let m = read_str(&x).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    let w = 0.5f64;
    nah(ev.punkt("10").unwrap(), 10.0 * w.cos(), -10.0 * w.sin(), 1e-9);
    let w2 = std::f64::consts::FRAC_PI_2 - 0.5;
    nah(ev.punkt("20").unwrap(), 10.0 * w2.cos(), -10.0 * w2.sin(), 1e-9);
    nah(ev.punkt("30").unwrap(), 5.0, 0.0, 1e-9);
    nah(ev.punkt("31").unwrap(), 5.0 * std::f64::consts::PI - 5.0, 0.0, 1e-9);
}

/// Zwei volle Kreise, Radius 10, Mitten (0, 0) und (10, 0): Schnitt bei
/// (5, ±8,6603). Seamly2Ds Kreisformel nennt zuerst den unteren (y > 0 in
/// gespeicherten Werten); `crossPoint` 2 nimmt den oberen. Halbe Bögen von
/// 0° bis 180° (oben) lassen nur den oberen gelten.
#[test]
fn schnitt_zweier_boegen() {
    let h = 75f64.sqrt();
    let x = muster(
        r#"<point id="1" name="C1" type="single" x="0" y="0" mx="0" my="0"/>
<point id="2" name="C2" type="single" x="10" y="0" mx="0" my="0"/>
<arc id="3" type="simple" center="1" radius="10" angle1="0" angle2="360"/>
<arc id="4" type="simple" center="2" radius="10" angle1="0" angle2="360"/>
<arc id="5" type="simple" center="1" radius="10" angle1="0" angle2="180"/>
<arc id="6" type="simple" center="2" radius="10" angle1="0" angle2="180"/>
<point id="10" name="S1" type="pointOfIntersectionArcs" firstArc="3" secondArc="4" crossPoint="1" mx="0" my="0"/>
<point id="11" name="S2" type="pointOfIntersectionArcs" firstArc="3" secondArc="4" crossPoint="2" mx="0" my="0"/>
<point id="12" name="S3" type="pointOfIntersectionArcs" firstArc="5" secondArc="6" crossPoint="1" mx="0" my="0"/>"#,
    );
    let m = read_str(&x).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    nah(ev.punkt("10").unwrap(), 5.0, h, 1e-9);
    nah(ev.punkt("11").unwrap(), 5.0, -h, 1e-9);
    nah(ev.punkt("12").unwrap(), 5.0, -h, 1e-9);
}

/// Eine waagerechte Spline bei y = 5 schneidet einen Zickzack-Pfad
/// (0,0)–(5,10)–(10,0)–(15,10)–(20,0) viermal auf gleicher Höhe:
/// `hCrossPoint` 1 nimmt den linken (x = 2,5), 2 den rechten (x = 17,5).
#[test]
fn schnitt_zweier_kurven() {
    let x = muster(
        r#"<point id="1" name="A" type="single" x="0" y="5" mx="0" my="0"/>
<point id="2" name="B" type="single" x="20" y="5" mx="0" my="0"/>
<point id="3" name="Z0" type="single" x="0" y="0" mx="0" my="0"/>
<point id="4" name="Z1" type="single" x="5" y="10" mx="0" my="0"/>
<point id="5" name="Z2" type="single" x="10" y="0" mx="0" my="0"/>
<point id="6" name="Z3" type="single" x="15" y="10" mx="0" my="0"/>
<point id="7" name="Z4" type="single" x="20" y="0" mx="0" my="0"/>
<spline id="8" type="simpleInteractive" point1="1" point4="2" angle1="0" angle2="180" length1="0" length2="0"/>
<spline id="9" type="pathInteractive">
  <pathPoint pSpline="3" angle1="180" angle2="0" length1="0" length2="0"/>
  <pathPoint pSpline="4" angle1="180" angle2="0" length1="0" length2="0"/>
  <pathPoint pSpline="5" angle1="180" angle2="0" length1="0" length2="0"/>
  <pathPoint pSpline="6" angle1="180" angle2="0" length1="0" length2="0"/>
  <pathPoint pSpline="7" angle1="180" angle2="0" length1="0" length2="0"/>
</spline>
<point id="10" name="L" type="pointOfIntersectionCurves" curve1="8" curve2="9" vCrossPoint="1" hCrossPoint="1" mx="0" my="0"/>
<point id="11" name="R" type="pointOfIntersectionCurves" curve1="8" curve2="9" vCrossPoint="1" hCrossPoint="2" mx="0" my="0"/>"#,
    );
    let m = read_str(&x).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    nah(ev.punkt("10").unwrap(), 2.5, 5.0, 1e-6);
    nah(ev.punkt("11").unwrap(), 17.5, 5.0, 1e-6);
}

/// Operationen: Spiegeln an einer senkrechten Achse und an einer Geraden,
/// Drehen (am Bildschirm gegen den Uhrzeiger), Verschieben; Namen mit
/// Suffix, Kurvenkopie mit `Spl_<Anfang><Suffix>_<Ende><Suffix>`, verkettete
/// Operation, ein Ziel als Bezug eines späteren Punktes.
#[test]
fn operationen() {
    let x = muster(
        r#"<point id="1" name="A" type="single" x="0" y="0" mx="0" my="0"/>
<point id="2" name="B" type="single" x="10" y="0" mx="0" my="0"/>
<point id="3" name="X" type="single" x="20" y="0" mx="0" my="0"/>
<point id="4" name="P" type="single" x="4" y="3" mx="0" my="0"/>
<spline id="5" type="simpleInteractive" point1="1" point4="2" angle1="0" angle2="180" length1="0" length2="0"/>
<operation id="6" type="flippingByAxis" center="3" axisType="1" suffix="m">
  <source><item idObject="1"/><item idObject="2"/><item idObject="5"/><item idObject="4"/></source>
  <destination><item idObject="7"/><item idObject="8"/><item idObject="9"/><item idObject="10"/></destination>
</operation>
<operation id="11" type="rotation" center="1" angle="90" suffix="r">
  <source><item idObject="2"/></source>
  <destination><item idObject="12"/></destination>
</operation>
<operation id="13" type="moving" angle="90" length="5" suffix="v">
  <source><item idObject="10"/></source>
  <destination><item idObject="14"/></destination>
</operation>
<operation id="15" type="flippingByLine" p1Line="1" p2Line="2" suffix="g">
  <source><item idObject="4"/></source>
  <destination><item idObject="16"/></destination>
</operation>
<point id="20" name="Q" type="endLine" basePoint="14" angle="0" length="Spl_Am_Bm" mx="0" my="0"/>
<point id="21" name="W" type="endLine" basePoint="1" angle="0" length="Line_Am_Bm" mx="0" my="0"/>"#,
    );
    let m = read_str(&x).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    nah(ev.punkt("7").unwrap(), 40.0, 0.0, 1e-12);
    nah(ev.punkt("8").unwrap(), 30.0, 0.0, 1e-12);
    nah(ev.punkt("10").unwrap(), 36.0, 3.0, 1e-12);
    assert_eq!(ev.id_von_name("Pm"), Some("10"));
    assert_eq!(ev.punkt_name("10"), Some("Pm"));
    // gedreht um A um 90°: am Bildschirm nach oben
    nah(ev.punkt("12").unwrap(), 0.0, -10.0, 1e-12);
    // verkettet: die Spiegelkopie Pm um 5 nach oben verschoben, heißt Pmv
    nah(ev.punkt("14").unwrap(), 36.0, -2.0, 1e-12);
    assert_eq!(ev.id_von_name("Pmv"), Some("14"));
    // an der Geraden A–B (y = 0) gespiegelt
    nah(ev.punkt("16").unwrap(), 4.0, -3.0, 1e-12);
    // Kurvenkopie: Länge 10, Kontrollpunkte gespiegelt
    assert!((ev.geo("Spl_Am_Bm").unwrap() - 10.0).abs() < 1e-9);
    let k = ev.kontrollen("9").unwrap();
    nah(k[0][0], 40.0, 0.0, 1e-12);
    nah(k[0][3], 30.0, 0.0, 1e-12);
    nah(ev.punkt("20").unwrap(), 46.0, -2.0, 1e-9);
    nah(ev.punkt("21").unwrap(), 10.0, 0.0, 1e-9);
}

/// `cutSpline` und `cutSplinePath` legen zwei Teilkurven an, die Formeln als
/// `Spl_<Anfang>_<Schnitt>` und `Spl_<Schnitt>_<Ende>` nennen — auch wenn die
/// Datei die ids + 1 und + 2 an andere Objekte vergibt (hier Punkt 11).
#[test]
fn teilkurven() {
    let x = muster(
        r#"<point id="1" name="A" type="single" x="0" y="0" mx="0" my="0"/>
<point id="2" name="B" type="single" x="10" y="0" mx="0" my="0"/>
<point id="3" name="C" type="single" x="20" y="0" mx="0" my="0"/>
<spline id="4" type="simpleInteractive" point1="1" point4="2" angle1="0" angle2="180" length1="0" length2="0"/>
<spline id="5" type="pathInteractive">
  <pathPoint pSpline="1" angle1="180" angle2="0" length1="0" length2="0"/>
  <pathPoint pSpline="2" angle1="180" angle2="0" length1="0" length2="0"/>
  <pathPoint pSpline="3" angle1="180" angle2="0" length1="0" length2="0"/>
</spline>
<point id="10" name="K" type="cutSpline" spline="4" length="4" mx="0" my="0"/>
<point id="11" name="belegt" type="single" x="1" y="1" mx="0" my="0"/>
<point id="20" name="M" type="cutSplinePath" splinePath="5" length="13" mx="0" my="0"/>"#,
    );
    let m = read_str(&x).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    // GetParmT trifft die Länge auf 0,1 %.
    let a = ev.geo("Spl_A_K").unwrap();
    let b = ev.geo("Spl_K_B").unwrap();
    assert!((a - 4.0).abs() < 0.005 && (a + b - 10.0).abs() < 1e-6, "{a} + {b}");
    let a = ev.geo("SplPath_A_M").unwrap();
    let b = ev.geo("SplPath_M_C").unwrap();
    assert!((a - 13.0).abs() < 0.005 && (a + b - 20.0).abs() < 1e-6, "{a} + {b}");
    // id 11 bleibt der Punkt, keine Teilkurve
    nah(ev.punkt("11").unwrap(), 1.0, 1.0, 1e-12);
}

/// `trueDarts`: symmetrischer Abnäher (Spitze (10, 10), Schenkel bei x = 9
/// und 11) auf der Basislinie (0, 0)–(20, 0). Die beiden Punkte liegen auf
/// den Schenkelgeraden, gleich weit von der Spitze und spiegelbildlich.
#[test]
fn echte_abnaeher() {
    let x = muster(
        r#"<point id="1" name="B1" type="single" x="0" y="0" mx="0" my="0"/>
<point id="2" name="B2" type="single" x="20" y="0" mx="0" my="0"/>
<point id="3" name="D1" type="single" x="9" y="0" mx="0" my="0"/>
<point id="4" name="D2" type="single" x="10" y="10" mx="0" my="0"/>
<point id="5" name="D3" type="single" x="11" y="0" mx="0" my="0"/>
<point id="6" type="trueDarts" baseLineP1="1" baseLineP2="2" dartP1="3" dartP2="4" dartP3="5" point1="7" point2="8" name1="T1" name2="T2" mx1="0" my1="0" mx2="0" my2="0"/>"#,
    );
    let m = read_str(&x).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    let (p1, p2) = (ev.punkt("7").unwrap(), ev.punkt("8").unwrap());
    assert_eq!(ev.id_von_name("T2"), Some("8"));
    // auf der Geraden Spitze–D1: x = 10 − (10 − y)/10
    assert!((p1.x - (10.0 - (10.0 - p1.y) / 10.0)).abs() < 1e-9, "{p1:?}");
    nah(p2, 20.0 - p1.x, p1.y, 1e-9);
    let d2 = Pt::new(10.0, 10.0);
    assert!((d2.dist(p1) - d2.dist(p2)).abs() < 1e-9);
    // Die Punkte liegen über der Basislinie (y < 0): der Abnäher schließt sich nach außen.
    assert!(p1.y < 0.0, "{p1:?}");
}

/// Bolero v37 (`cutArc`, Teilkurven, Faltenpunkte auf einer mehrfach
/// genannten Spline) und Strumpf v26 (acht Spiegelungen mit Punkt- und
/// Kurvenkopien) aus dem Hexenensemble vom 27.10.2026 mit der Maßdatei dieses
/// Tages: alle benannten Punkte (auch Kopien) lösbar, der Umfang jedes Teils
/// auf Seamlys Zerlegung wie in Seamly2Ds eigenem Export
/// (`seamly2d.exe -f 14 --exportOnlyDetails`, geschlossene Polylinien, mm).
#[test]
fn seamly2d_export_bolero_und_strumpf() {
    use klon3d_pattern::kanten::Kurvenquelle;
    let w = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/muster/hexen_2026_10_27"));
    let faelle: [(&str, &[(&str, f64)]); 2] = [
        (
            "bolero_v37",
            &[
                ("Rueckenteil 1x", 1342.79),
                ("Vorderteil links 1x", 707.79),
                ("Bischofaermel links 1x", 1418.58),
                ("Manschette links 1x", 596.76),
                ("Stehkragen gestuft 1x", 953.36),
                ("Stehkragen Innenlage 1x", 948.36),
                ("Vorderteil rechts 1x", 707.79),
                ("Bischofaermel rechts 1x", 1418.58),
                ("Manschette rechts 1x", 596.76),
                ("Bolero - Scheibe Saum links", 62.84),
                ("Bolero - Donut Keyhole links", 94.25),
                ("Bolero - Frog-Plaettchen", 80.00),
            ],
        ),
        (
            "strumpf_v26",
            &[
                ("A Abschluss gerade 0,40mm - Strumpf 1", 1089.47),
                ("Ferse 0,40mm (Verstaerkung) - Strumpf 1", 298.48),
                ("Bein 0,25mm - Strumpf 1", 1974.91),
                ("Sohle 0,25mm - Fuss 1", 556.65),
                ("Spitze 0,40mm (Verstaerkung) - Fuss 1", 237.69),
                ("B Abschluss Umschlag + Keyhole 0,25mm - Strumpf 1", 1289.35),
                ("A Abschluss gerade 0,40mm - Strumpf 2", 1089.47),
                ("B Abschluss Umschlag + Keyhole 0,25mm - Strumpf 2", 1289.35),
                ("Bein 0,25mm - Strumpf 2", 1974.91),
                ("Ferse 0,40mm (Verstaerkung) - Strumpf 2", 298.48),
                ("Sohle 0,25mm - Fuss 2 (gespiegelt)", 556.65),
                ("Spitze 0,40mm (Verstaerkung) - Fuss 2 (gespiegelt)", 237.69),
            ],
        ),
    ];
    for (name, soll) in faelle {
        let m = klon3d_pattern::read_path(&w.join(format!("{name}.sm2d"))).unwrap();
        let ev = Auswertung::lade(&m, &w.join("measurements.smis")).unwrap();
        let mut ids: Vec<String> = m
            .draft_blocks
            .iter()
            .filter_map(|b| b.calculation.as_ref())
            .flat_map(|c| c.points.iter().filter(|p| p.name.is_some()).map(|p| p.id.clone()))
            .collect();
        let kopien = ev.abgeleitete_punkte();
        ids.extend(kopien.iter().map(|(_, id)| id.clone()));
        let offen: Vec<String> = ids.iter().filter_map(|id| ev.punkt(id).err().map(|e| format!("{id}: {e}"))).collect();
        assert!(offen.is_empty(), "{name}: {} Punkte nicht lösbar, z. B. {:?}", offen.len(), &offen[..offen.len().min(3)]);
        if name == "strumpf_v26" {
            assert!(kopien.len() > 100, "{name}: nur {} Punktkopien", kopien.len());
        }
        let mut geprueft = 0;
        for t in m.pieces() {
            let rand = ev.teilrand(t, Kurvenquelle::Seamly).unwrap();
            let Some(&(_, mm)) = soll.iter().find(|(n, _)| *n == rand.teil) else { continue };
            let ist = rand.in_cm(rand.umfang()) * 10.0;
            assert!((ist - mm).abs() < 0.05, "{name} {}: {ist:.2} mm, Seamly2D {mm:.2} mm", rand.teil);
            geprueft += 1;
        }
        assert!(geprueft >= soll.len(), "{name}: nur {geprueft} von {} Teilen gefunden", soll.len());
    }
}
