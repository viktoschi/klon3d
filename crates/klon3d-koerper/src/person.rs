//! Zielperson aus Maßen: Namen der Maßdatei auf CLO-Felder abbilden.
//!
//! Die Maßdatei gehört zu SeamlyMe (`.smis`, gelesen von `klon3d-pattern`),
//! die Felder zu CLOs Resizing (Befund 22). Diese Brücke ist bewusst
//! **ohne** Abhängigkeit zum Parser gebaut: [`zielperson`] fragt Maße über
//! eine Funktion ab (`name -> Option<f64>`), damit die Bibliothek auch mit
//! einer Tabelle, einem Messbuch-Export oder Testwerten arbeiten kann.

use crate::anpassen::{messlinien_gemischt, Messlinien, Messort, Ziel};
use crate::formung::referenzwert;
use crate::Netz;

/// Alles, was die Anpassung über die Zielperson wissen muss.
#[derive(Clone, Debug)]
pub struct Zielperson {
    /// Höhe und die Umfänge der gewählten Linien.
    pub ziel: Ziel,
    /// Referenzvektor mit den gesetzten `HEIGHT_*`-Feldern.
    pub c_start: Vec<f64>,
    /// Messlinien: feste Schnitte, sobald die Höhe bekannt ist.
    pub linien: Messlinien,
    /// Feldindex in `c` je Linie, für [`crate::anpassen::anpassen`].
    pub aktiv: Vec<usize>,
    /// Geänderte Höhenfelder `(Name, alt, neu)`, für Meldungen.
    pub hoehen_geaendert: Vec<(String, f64, f64)>,
}

fn fehlt(name: &str) -> String {
    format!("Mass {name:?} fehlt in der Maßdatei")
}

/// Baut die Zielperson aus abgefragten Maßen.
///
/// Pflicht sind `height`, `bust_circ`, `waist_circ` und `hip_circ`;
/// `highhip_circ` und `lowbust_circ` ergänzen die entsprechenden Linien,
/// wenn vorhanden. Höhen (`height_bustpoint`, `height_waist_side`,
/// `height_highhip`, `height_hip`, `leg_crotch_to_floor`, `height_knee`)
/// setzen die `HEIGHT_*`-Felder und machen aus der jeweiligen Linie einen
/// festen Schnitt bei der Zielhöhe; fehlt eine Höhe, bleibt es bei der
/// mitwandernden Materiallinie der Referenz.
pub fn zielperson(
    grund: &Netz,
    namen: &[String],
    c_ref: &[f64],
    wert: &dyn Fn(&str) -> Option<f64>,
) -> Result<Zielperson, String> {
    if namen.len() != c_ref.len() {
        return Err(format!(
            "{} Maßnamen, aber {} Koeffizienten",
            namen.len(),
            c_ref.len()
        ));
    }
    let hoehe = wert("height").ok_or_else(|| fehlt("height"))?;
    let brust = wert("bust_circ").ok_or_else(|| fehlt("bust_circ"))?;
    let taille = wert("waist_circ").ok_or_else(|| fehlt("waist_circ"))?;
    let huefte = wert("hip_circ").ok_or_else(|| fehlt("hip_circ"))?;
    let highhip = wert("highhip_circ");
    let lowbust = wert("lowbust_circ");

    let mut ziele = vec![
        (Messort::Brust, brust),
        (Messort::Taille, taille),
        (Messort::Huefte, huefte),
    ];
    if let Some(v) = lowbust {
        ziele.push((Messort::Unterbrust, v));
    }
    if let Some(v) = highhip {
        ziele.push((Messort::HighHip, v));
    }
    ziele.sort_by_key(|(ort, _)| *ort);

    // CLOs Höhenfelder sind absolute Zentimeter (Befund 22). Zwei Höhen
    // stehen nicht in der Maßdatei; sie werden mit dem Referenzabstand
    // abgeleitet: Unterbrust über der Taille, Oberschenkel unter dem Schritt.
    let unterbrust_offset =
        referenzwert("HEIGHT_UnderBust").unwrap() - referenzwert("HEIGHT_Waist").unwrap();
    let thigh_offset =
        referenzwert("HEIGHT_Crotch").unwrap() - referenzwert("HEIGHT_Thigh").unwrap();

    let mut c_start = c_ref.to_vec();
    let mut hoehen_geaendert = Vec::new();
    {
        let mut setze = |feld: &str, neu: f64| {
            if let Some(i) = namen.iter().position(|n| n == feld) {
                if (c_start[i] - neu).abs() > 1.0e-9 {
                    hoehen_geaendert.push((feld.to_string(), c_start[i], neu));
                    c_start[i] = neu;
                }
            }
        };
        if let Some(v) = wert("height_bustpoint") {
            setze("HEIGHT_APEX", v);
        }
        if let Some(v) = wert("height_waist_side") {
            setze("HEIGHT_Waist", v);
            setze("HEIGHT_UnderBust", v + unterbrust_offset);
        }
        if let Some(v) = wert("height_highhip") {
            setze("HEIGHT_HighHip", v);
        }
        if let Some(v) = wert("height_hip") {
            setze("HEIGHT_LowHip", v);
        }
        if let Some(v) = wert("leg_crotch_to_floor") {
            setze("HEIGHT_Crotch", v);
            setze("HEIGHT_Thigh", v - thigh_offset);
        }
        if let Some(v) = wert("height_knee") {
            setze("HEIGHT_Knee", v);
        }
    }

    let mut wahl = Vec::with_capacity(ziele.len());
    for (ort, _) in &ziele {
        let hoehe = match ort {
            Messort::Brust => wert("height_bustpoint"),
            Messort::Unterbrust => wert("height_waist_side").map(|v| v + unterbrust_offset),
            Messort::Taille => wert("height_waist_side"),
            Messort::HighHip => wert("height_highhip"),
            Messort::Huefte => wert("height_hip"),
        };
        wahl.push((*ort, hoehe));
    }
    let linien = messlinien_gemischt(grund, &wahl)?;
    let aktiv = linien.aktiv(namen)?;

    Ok(Zielperson {
        ziel: Ziel {
            hoehe,
            masse: ziele.iter().map(|(_, v)| *v).collect(),
        },
        c_start,
        linien,
        aktiv,
        hoehen_geaendert,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nur die Felder, die die Zielperson braucht, in ID-Reihenfolge.
    fn modell() -> (Vec<String>, Vec<f64>) {
        let namen = [
            "HEIGHT_Total",
            "CIRCUMFERENCE_Bust",
            "CIRCUMFERENCE_UnderBust",
            "CIRCUMFERENCE_Waist",
            "CIRCUMFERENCE_HighHip",
            "CIRCUMFERENCE_LowHip",
            "HEIGHT_APEX",
            "HEIGHT_Waist",
            "HEIGHT_UnderBust",
            "HEIGHT_HighHip",
            "HEIGHT_LowHip",
            "HEIGHT_Crotch",
            "HEIGHT_Thigh",
            "HEIGHT_Knee",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let werte = vec![
            163.95, 9.8856, 9.1710, 8.9211, 9.8387, 10.1972, 119.4119, 105.7056, 113.1081, 93.8775,
            82.2780, 76.8747, 74.2305, 45.1646,
        ];
        (namen, werte)
    }

    #[test]
    fn alle_masse_aus_der_tabelle_ergeben_feste_schnitte() {
        let (namen, c_ref) = modell();
        let wert = |name: &str| -> Option<f64> {
            Some(match name {
                "height" => 160.0,
                "bust_circ" => 92.67,
                "waist_circ" => 74.0,
                "hip_circ" => 90.0,
                "highhip_circ" => 83.0,
                "lowbust_circ" => 76.0,
                "height_bustpoint" => 111.0,
                "height_waist_side" => 98.0,
                "height_highhip" => 93.0,
                "height_hip" => 77.0,
                "leg_crotch_to_floor" => 72.0,
                "height_knee" => 42.0,
                _ => return None,
            })
        };
        let p = zielperson(&Netz::default(), &namen, &c_ref, &wert).expect("Zielperson");
        assert_eq!(p.ziel.hoehe, 160.0);
        // Reihenfolge: Brust, Unterbrust, Taille, HighHip, Hüfte.
        assert_eq!(p.ziel.masse, vec![92.67, 76.0, 74.0, 83.0, 90.0]);
        assert_eq!(p.aktiv, vec![1, 2, 3, 4, 5]);
        assert!(p.linien.linien.iter().all(|l| l.spur.is_none()));
        let soll = [111.0, 105.4025, 98.0, 93.0, 77.0];
        for (h, s) in p.linien.hoehen().iter().zip(&soll) {
            assert!((h - s).abs() < 1.0e-9, "{h} != {s}");
        }
        // HEIGHT_Waist steht an Position 7, HEIGHT_UnderBust auf 98 + 7,4025.
        assert!((p.c_start[7] - 98.0).abs() < 1.0e-9);
        assert!((p.c_start[8] - 105.4025).abs() < 1.0e-9);
        assert!((p.c_start[12] - (72.0 - 2.6442)).abs() < 1.0e-9);
        assert!(!p.hoehen_geaendert.is_empty());
    }

    #[test]
    fn ohne_hoehen_bleiben_die_linien_materiallinien() {
        let (namen, c_ref) = modell();
        // Für Materiallinien braucht der Test einen echten Körper; ein
        // Zylinder genügt, weil nur die Schnitte existieren müssen.
        let grund = crate::Netz::zylinder(100.0, 3000.0, 720, 20);
        let wert = |name: &str| -> Option<f64> {
            match name {
                "height" => Some(160.0),
                "bust_circ" => Some(92.0),
                "waist_circ" => Some(74.0),
                "hip_circ" => Some(90.0),
                _ => None,
            }
        };
        let p = zielperson(&grund, &namen, &c_ref, &wert).expect("Zielperson");
        assert_eq!(p.ziel.masse, vec![92.0, 74.0, 90.0]);
        assert!(p.linien.linien.iter().all(|l| l.spur.is_some()));
        assert!(p.hoehen_geaendert.is_empty());
    }

    #[test]
    fn fehlendes_pflichtmass_ist_ein_fehler() {
        let (namen, c_ref) = modell();
        let wert = |name: &str| -> Option<f64> {
            match name {
                "height" => Some(160.0),
                "bust_circ" => Some(92.0),
                "hip_circ" => Some(90.0),
                _ => None,
            }
        };
        match zielperson(&Netz::default(), &namen, &c_ref, &wert) {
            Err(e) => assert!(e.contains("waist_circ"), "{e}"),
            Ok(_) => panic!("waist_circ fehlt, darf nicht durchgehen"),
        }
    }
}
