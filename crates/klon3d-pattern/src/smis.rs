//! SeamlyMe-Maßdateien (`.smis`).
//!
//! Aufbau:
//!
//! ```text
//! <smis>
//!   <version>0.3.4</version>
//!   <unit>cm</unit>
//!   <personal>…</personal>
//!   <body-measurements>
//!     <m name="height" value="160"/>
//!     <m name="@armhole_width_f" value="6" description="…"/>
//!   </body-measurements>
//! </smis>
//! ```
//!
//! Für die Körperformung zählen nur die `<m>`-Einträge in
//! `<body-measurements>`. Namen werden **unverändert** übernommen, also
//! einschließlich eines führenden `@` (SeamlyMe kennzeichnet damit eigene
//! Maße). Doppelte Namen sind ein Fehler: stilles Überschreiben würde eine
//! beschädigte Datei verschleiern.
//!
//! Alle Werte werden beim Einlesen in Zentimeter umgerechnet. `einheit`
//! bleibt die in der Datei angegebene Einheit — nützlich für Meldungen und
//! für den Fall, dass ein Muster in Millimetern oder Zoll rechnet.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::PatternError;

/// In der Datei angegebene Maßeinheit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Einheit {
    Cm,
    Mm,
    Inch,
}

impl Einheit {
    /// Rechnet einen Wert dieser Einheit in Zentimeter um.
    pub fn in_cm(self, wert: f64) -> f64 {
        match self {
            Einheit::Cm => wert,
            Einheit::Mm => wert / 10.0,
            Einheit::Inch => wert * 2.54,
        }
    }
}

/// Eine gelesene Maßdatei. `masse` steht bereits in Zentimetern.
#[derive(Clone, Debug)]
pub struct Smis {
    /// Einheit, in der die Datei geschrieben war.
    pub einheit: Einheit,
    /// Maße in Dateireihenfolge, Werte in Zentimetern.
    pub masse: Vec<(String, f64)>,
}

impl Smis {
    /// Wert eines Maßes in Zentimetern, `None` wenn es fehlt.
    pub fn wert(&self, name: &str) -> Option<f64> {
        self.masse
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, wert)| *wert)
    }

    /// Enthält die Datei dieses Maß?
    pub fn hat(&self, name: &str) -> bool {
        self.masse.iter().any(|(n, _)| n == name)
    }
}

/// Liest eine `.smis`-Datei vom Pfad.
pub fn lies_smis(pfad: &Path) -> Result<Smis, PatternError> {
    let text = fs::read_to_string(pfad)
        .map_err(|e| PatternError::Xml(format!("{}: {e}", pfad.display())))?;
    lies_smis_str(&text)
}

/// Liest eine `.smis`-Datei aus dem Speicher.
pub fn lies_smis_str(xml: &str) -> Result<Smis, PatternError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut einheit: Option<Einheit> = None;
    let mut masse: Vec<(String, f64)> = Vec::new();
    let mut namen: HashMap<String, ()> = HashMap::new();

    // Nur innerhalb von <body-measurements> sammeln. Andere <m>-Elemente
    // (falls eine künftige Version sie einführt) gehören nicht zu den Maßen.
    let mut im_koerper = false;
    let mut im_einheit = false;
    let mut einheit_text = String::new();

    // Tiefe unabhängig vom Kontext: abgeschnittenes XML fällt am Ende auf.
    let mut tiefe: i32 = 0;
    let mut wurzel_gesehen = false;

    loop {
        let ev = reader
            .read_event()
            .map_err(|e| PatternError::Xml(e.to_string()))?;
        match ev {
            Event::Start(e) => {
                tiefe += 1;
                let name = e.local_name();
                match name.as_ref() {
                    "smis" => wurzel_gesehen = true,
                    "body-measurements" => im_koerper = true,
                    "unit" => {
                        im_einheit = true;
                        einheit_text.clear();
                    }
                    "m" if im_koerper => {
                        lies_mass(&e, &mut masse, &mut namen)?;
                    }
                    _ => {}
                }
            }
            Event::Empty(e) => {
                // <m .../> ist der Normalfall; <body-measurements/> wäre leer.
                if e.local_name().as_ref() == "m" && im_koerper {
                    lies_mass(&e, &mut masse, &mut namen)?;
                }
            }
            Event::Text(t) if im_einheit => {
                einheit_text.push_str(&quick_xml::escape::unescape(t.as_ref()).map_err(|e| {
                    PatternError::Xml(e.to_string())
                })?);
            }
            Event::End(e) => {
                tiefe -= 1;
                match e.local_name().as_ref() {
                    "body-measurements" => im_koerper = false,
                    "unit" => {
                        im_einheit = false;
                        let text = einheit_text.trim();
                        if text.is_empty() {
                            return Err(PatternError::MissingUnit);
                        }
                        let gelesen = einheit_aus(text)?;
                        // Mehrfache <unit>-Elemente: die letzte Angabe gilt.
                        einheit = Some(gelesen);
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if tiefe != 0 {
        return Err(PatternError::Xml("unvollständiges XML".to_string()));
    }
    if !wurzel_gesehen {
        return Err(PatternError::UnexpectedElement(
            "kein <smis>-Wurzelelement".to_string(),
        ));
    }
    let einheit = einheit.ok_or(PatternError::MissingUnit)?;
    let masse = masse
        .into_iter()
        .map(|(n, wert)| (n, einheit.in_cm(wert)))
        .collect();
    Ok(Smis { einheit, masse })
}

fn lies_mass(
    e: &BytesStart<'_>,
    masse: &mut Vec<(String, f64)>,
    namen: &mut HashMap<String, ()>,
) -> Result<(), PatternError> {
    let a = attrs(e);
    let name = a
        .get("name")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or(PatternError::MissingMeasurementName)?;
    if namen.insert(name.clone(), ()).is_some() {
        return Err(PatternError::DuplicateMeasurement(name));
    }
    let roh = a
        .get("value")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| PatternError::MissingMeasurementValue(name.clone()))?;
    let wert = roh
        .parse::<f64>()
        // Das Messbuch schreibt Komma; SeamlyMe schreibt Punkt.
        .or_else(|_| roh.replace(',', ".").parse::<f64>())
        .map_err(|_| PatternError::InvalidMeasurementValue {
            name: name.clone(),
            wert: roh.to_string(),
        })?;
    masse.push((name, wert));
    Ok(())
}

fn einheit_aus(text: &str) -> Result<Einheit, PatternError> {
    match text.to_ascii_lowercase().as_str() {
        "cm" => Ok(Einheit::Cm),
        "mm" => Ok(Einheit::Mm),
        "inch" | "in" => Ok(Einheit::Inch),
        _ => Err(PatternError::UnsupportedUnit(text.to_string())),
    }
}

/// Attributliste mit aufgelösten XML-Entitäten (`&amp;` usw.).
fn attrs(e: &BytesStart<'_>) -> HashMap<String, String> {
    e.attributes()
        .flatten()
        .map(|a| {
            let k = String::from_utf8_lossy(a.key.as_ref().as_bytes()).into_owned();
            let roh = String::from_utf8_lossy(a.value.as_ref().as_bytes());
            let v = quick_xml::escape::unescape(&roh)
                .map(|s| s.into_owned())
                .unwrap_or_else(|_| roh.into_owned());
            (k, v)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KOPF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<smis>
  <version>0.3.4</version>
  <unit>cm</unit>
  <personal><given-name>Test</given-name></personal>
"#;

    fn mit_koerper(inhalt: &str) -> String {
        format!("{KOPF}<body-measurements>{inhalt}</body-measurements></smis>")
    }

    #[test]
    fn liest_masse_und_ignoriert_kommentare() {
        let xml = mit_koerper(
            r#"
            <!-- Grundmaße -->
            <m name="height" value="160"/>
            <m name="bust_circ" value="92.67"/>
            <m name="waist_circ" value=" 74 "/>
            "#,
        );
        let s = lies_smis_str(&xml).expect("Maßdatei");
        assert_eq!(s.einheit, Einheit::Cm);
        assert_eq!(s.masse.len(), 3);
        assert_eq!(s.wert("height"), Some(160.0));
        assert_eq!(s.wert("bust_circ"), Some(92.67));
        assert_eq!(s.wert("waist_circ"), Some(74.0));
        assert_eq!(s.wert("gibt_es_nicht"), None);
    }

    #[test]
    fn extra_attribute_und_eigene_namen_bleiben_erhalten() {
        let xml = mit_koerper(
            r#"<m name="@armhole_width_f" value="6" description="vorn &amp; hinten" full_name="Armloch"/>"#,
        );
        let s = lies_smis_str(&xml).expect("Maßdatei");
        assert!(s.hat("@armhole_width_f"));
        assert_eq!(s.wert("@armhole_width_f"), Some(6.0));
        // Das &amp; steht nur im Zusatzattribut, darf den Parser nicht stören.
        assert_eq!(s.masse[0].0, "@armhole_width_f");
    }

    #[test]
    fn komma_als_dezimaltrennzeichen_wird_akzeptiert() {
        let xml = mit_koerper(r#"<m name="bust_circ" value="92,67"/>"#);
        let s = lies_smis_str(&xml).expect("Maßdatei");
        assert_eq!(s.wert("bust_circ"), Some(92.67));
    }

    #[test]
    fn millimeter_werden_in_zentimeter_umgerechnet() {
        let xml = KOPF
            .replace("<unit>cm</unit>", "<unit>mm</unit>")
            + r#"<body-measurements><m name="height" value="1600"/></body-measurements></smis>"#;
        let s = lies_smis_str(&xml).expect("Maßdatei");
        assert_eq!(s.einheit, Einheit::Mm);
        assert_eq!(s.wert("height"), Some(160.0));
    }

    #[test]
    fn zoll_werden_in_zentimeter_umgerechnet() {
        let xml = KOPF
            .replace("<unit>cm</unit>", "<unit>inch</unit>")
            + r#"<body-measurements><m name="height" value="10"/></body-measurements></smis>"#;
        let s = lies_smis_str(&xml).expect("Maßdatei");
        assert_eq!(s.einheit, Einheit::Inch);
        assert_eq!(s.wert("height"), Some(25.4));
    }

    #[test]
    fn unbekannte_einheit_ist_ein_fehler() {
        let xml = KOPF.replace("<unit>cm</unit>", "<unit>elle</unit>")
            + r#"<body-measurements/></smis>"#;
        match lies_smis_str(&xml) {
            Err(PatternError::UnsupportedUnit(u)) => assert_eq!(u, "elle"),
            sonst => panic!("erwartet UnsupportedUnit, war {sonst:?}"),
        }
    }

    #[test]
    fn fehlende_einheit_ist_ein_fehler() {
        let xml = KOPF.replace("<unit>cm</unit>", "")
            + r#"<body-measurements><m name="height" value="160"/></body-measurements></smis>"#;
        assert!(matches!(
            lies_smis_str(&xml),
            Err(PatternError::MissingUnit)
        ));
    }

    #[test]
    fn doppelter_name_ist_ein_fehler() {
        let xml = mit_koerper(
            r#"<m name="height" value="160"/><m name="height" value="161"/>"#,
        );
        match lies_smis_str(&xml) {
            Err(PatternError::DuplicateMeasurement(n)) => assert_eq!(n, "height"),
            sonst => panic!("erwartet DuplicateMeasurement, war {sonst:?}"),
        }
    }

    #[test]
    fn fehlender_wert_ist_ein_fehler() {
        let xml = mit_koerper(r#"<m name="height"/>"#);
        match lies_smis_str(&xml) {
            Err(PatternError::MissingMeasurementValue(n)) => assert_eq!(n, "height"),
            sonst => panic!("erwartet MissingMeasurementValue, war {sonst:?}"),
        }
    }

    #[test]
    fn unlesbarer_wert_ist_ein_fehler() {
        let xml = mit_koerper(r#"<m name="height" value="gross"/>"#);
        match lies_smis_str(&xml) {
            Err(PatternError::InvalidMeasurementValue { name, wert }) => {
                assert_eq!(name, "height");
                assert_eq!(wert, "gross");
            }
            sonst => panic!("erwartet InvalidMeasurementValue, war {sonst:?}"),
        }
    }

    #[test]
    fn masse_ausserhalb_von_body_measurements_zaehlen_nicht() {
        let xml = KOPF.replace(
            "<personal>",
            r#"<m name="falle" value="1"/><personal>"#,
        ) + r#"<body-measurements><m name="height" value="160"/></body-measurements></smis>"#;
        let s = lies_smis_str(&xml).expect("Maßdatei");
        assert_eq!(s.masse.len(), 1);
        assert_eq!(s.wert("falle"), None);
    }

    /// Prüft eine echte persönliche Maßdatei, sofern `KLON3D_SMIS` gesetzt ist.
    #[test]
    #[ignore = "braucht eine echte .smis über KLON3D_SMIS"]
    fn echte_datei() {
        let pfad = std::env::var("KLON3D_SMIS").expect("KLON3D_SMIS");
        let s = lies_smis(&Path::new(&pfad)).expect("Maßdatei");
        assert!(s.wert("height").is_some(), "height fehlt");
        assert!(s.wert("bust_circ").is_some(), "bust_circ fehlt");
        assert!(s.wert("waist_circ").is_some(), "waist_circ fehlt");
        println!("{} Maße aus {pfad}", s.masse.len());
    }
}
