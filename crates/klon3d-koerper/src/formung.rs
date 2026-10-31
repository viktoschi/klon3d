//! CLOs Formungsfelder: `P = Grundnetz + F_letzt + Σ F_k · c_k`.
//!
//! Die Rechnung ist in Befund 22 belegt. Kurz:
//!
//! * Eine `.bd`-Datei enthält `felder` Vektoren je Punkt, merkmalweise
//!   gespeichert (`int32 felder, int32 vektoren`, dann `f32`).
//! * Die Feldreihenfolge ist die **ID-Reihenfolge der Masse**, nicht die
//!   Reihenfolge der `_*Feature_*.txt`. `referenz_vektor` sortiert die
//!   Namen der txt deshalb über [`id`].
//! * Das **letzte** Feld hat fest den Koeffizienten 1,0 und ist der
//!   dateieigene Nullpunkt. Ein Koeffizientenvektor hat daher
//!   `felder − 1` Einträge.
//! * Höhen und Längen sind Zentimeter, Umfänge werden intern halbiert
//!   geführt. Für die Anpassung ist das gleichgültig, weil der
//!   Referenzvektor aus derselben Quelle kommt (Befund 22).
//!
//! Die Felder stehen in Zentimetern, [`Netz`] rechnet in Millimetern —
//! [`Formung::laden`] rechnet beim Einlesen um.

use crate::{Netz, Punkt3};
use std::fs;
use std::io;
use std::path::Path;

/// Maßnamen und ihre ID (Befund 22).
///
/// Die ID ist die Stelle im 58-Einträge-Feld des Programms und zugleich
/// die Reihenfolge, in der die `.bd`-Felder angewendet werden.
pub const IDS: [(&str, u8); 58] = [
    ("HEIGHT_Total", 0),
    ("WEIGHT_Total", 1),
    ("CIRCUMFERENCE_Bust", 2),
    ("CIRCUMFERENCE_MidNeck", 3),
    ("CIRCUMFERENCE_NeckBase", 4),
    ("CIRCUMFERENCE_UnderBust", 5),
    ("CIRCUMFERENCE_Waist", 6),
    ("CIRCUMFERENCE_HighHip", 7),
    ("CIRCUMFERENCE_LowHip", 8),
    ("CIRCUMFERENCE_Thigh", 9),
    ("CIRCUMFERENCE_MidThigh", 10),
    ("CIRCUMFERENCE_Knee", 11),
    ("CIRCUMFERENCE_Calf", 12),
    ("CIRCUMFERENCE_Ankle", 13),
    ("CIRCUMFERENCE_Armhole", 14),
    ("CIRCUMFERENCE_Bicep", 15),
    ("CIRCUMFERENCE_Elbow", 16),
    ("CIRCUMFERENCE_Wrist", 17),
    ("HEIGHT_HPS", 18),
    ("HEIGHT_Shoulder", 19),
    ("HEIGHT_ShoulderDrop", 20),
    ("HEIGHT_APEX", 21),
    ("HEIGHT_UnderBust", 22),
    ("HEIGHT_Waist", 23),
    ("HEIGHT_HighHip", 24),
    ("HEIGHT_LowHip", 25),
    ("HEIGHT_Crotch", 26),
    ("HEIGHT_Thigh", 27),
    ("HEIGHT_MidThigh", 28),
    ("HEIGHT_Knee", 29),
    ("HEIGHT_Calf", 30),
    ("HEIGHT_Ankle", 31),
    ("LENGTH_Bust_Width", 32),
    ("LENGTH_Bust_Depth", 33),
    ("LENGTH_AcrossFront", 34),
    ("LENGTH_AcrossBack", 35),
    ("LENGTH_AcrossShoulder", 36),
    ("LENGTH_CBNeck_Shoulder", 37),
    ("LENGTH_CFNeck_AcrossFront", 38),
    ("LENGTH_CBNeck_AcrossBack", 39),
    ("LENGTH_CFNeck_Waist", 40),
    ("LENGTH_CBNeck_Waist", 41),
    ("LENGTH_Waist_HighHip", 42),
    ("LENGTH_Waist_LowHip", 43),
    ("LENGTH_APEX_APEX", 44),
    ("LENGTH_HPS_APEX", 45),
    ("LENGTH_Halter", 46),
    ("LENGTH_Arm", 47),
    ("LENGTH_CBNeck_Wrist", 48),
    ("LENGTH_Total_Rise", 49),
    ("LENGTH_Front_Rise", 50),
    ("LENGTH_Back_Rise", 51),
    ("LENGTH_CrotchDepth", 52),
    ("LENGTH_CrotchWidth", 53),
    ("LENGTH_Crothch_MidThigh", 54),
    ("LENGTH_VerticalTrunk_Front", 55),
    ("LENGTH_VerticalTrunk_Back", 56),
    ("LENGTH_VerticalTrunk", 57),
];

/// ID eines Maßnamens, `None` bei unbekanntem Namen.
pub fn id(name: &str) -> Option<u8> {
    IDS.iter().find(|(n, _)| *n == name).map(|(_, i)| *i)
}

/// Referenzmaße der weiblichen CLO-Avatarin in der Einheit der `.bd`-Datei.
///
/// Quelle: `DETAIL_Advanced_TotalHeight` (weiblich), Least Squares gegen
/// `BaseMesh.obj`, RMS 0,00014 cm. Höhen und Längen sind Zentimeter,
/// Umfänge die internen Halbwerte (Befund 22).
pub const REFERENZ: [(&str, f64); 32] = [
    ("HEIGHT_Total", 163.9500),
    ("CIRCUMFERENCE_Bust", 9.8856),
    ("CIRCUMFERENCE_NeckBase", 6.2161),
    ("CIRCUMFERENCE_UnderBust", 9.1710),
    ("CIRCUMFERENCE_Waist", 8.9211),
    ("CIRCUMFERENCE_HighHip", 9.8387),
    ("CIRCUMFERENCE_LowHip", 10.1972),
    ("CIRCUMFERENCE_Thigh", 7.8189),
    ("CIRCUMFERENCE_Knee", 6.0528),
    ("CIRCUMFERENCE_Calf", 6.0056),
    ("CIRCUMFERENCE_Ankle", 4.7758),
    ("CIRCUMFERENCE_Bicep", 5.5056),
    ("CIRCUMFERENCE_Elbow", 4.8836),
    ("CIRCUMFERENCE_Wrist", 3.9049),
    ("HEIGHT_ShoulderDrop", 4.0609),
    ("HEIGHT_APEX", 119.4119),
    ("HEIGHT_UnderBust", 113.1081),
    ("HEIGHT_Waist", 105.7056),
    ("HEIGHT_HighHip", 93.8775),
    ("HEIGHT_LowHip", 82.2780),
    ("HEIGHT_Crotch", 76.8747),
    ("HEIGHT_Thigh", 74.2305),
    ("HEIGHT_Knee", 45.1646),
    ("LENGTH_AcrossShoulder", 38.8418),
    ("LENGTH_CFNeck_Waist", 33.5280),
    ("LENGTH_CBNeck_Waist", 36.9380),
    ("LENGTH_APEX_APEX", 18.3350),
    ("LENGTH_HPS_APEX", 26.5276),
    ("LENGTH_Arm", 55.8476),
    ("LENGTH_CBNeck_Wrist", 75.1367),
    ("LENGTH_Total_Rise", 76.7299),
    ("LENGTH_VerticalTrunk", 156.3254),
];

/// Referenzwert eines Maßes, `None` wenn es nicht zur weiblichen
/// Advanced-Stufe gehört.
pub fn referenzwert(name: &str) -> Option<f64> {
    REFERENZ.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

/// Die Formungsfelder einer `.bd`-Datei, in Millimetern je Einheit `c`.
#[derive(Clone, Debug)]
pub struct Formung {
    felder: usize,
    punkte: usize,
    /// `feld k, punkt i`; Länge `felder · punkte`.
    vektoren: Vec<Punkt3>,
}

impl Formung {
    /// Liest eine `ConvertingMat_*.bd` für ein Netz mit `punkte` Punkten.
    pub fn laden(pfad: &Path, punkte: usize) -> io::Result<Formung> {
        let b = fs::read(pfad)?;
        if b.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: kürzer als der Kopf", pfad.display()),
            ));
        }
        let felder = i32::from_le_bytes(b[0..4].try_into().unwrap());
        let anzahl = i32::from_le_bytes(b[4..8].try_into().unwrap());
        if felder < 2 || anzahl <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: unsinniger Kopf ({felder}, {anzahl})", pfad.display()),
            ));
        }
        let felder = felder as usize;
        if anzahl as usize != punkte * 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {} Vektoren, das Netz hat {} Punkte",
                    pfad.display(),
                    anzahl,
                    punkte
                ),
            ));
        }
        let erwartet = 8 + felder * punkte * 12;
        if b.len() != erwartet {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {} Bytes, erwartet {erwartet}",
                    pfad.display(),
                    b.len()
                ),
            ));
        }
        let mut vektoren = Vec::with_capacity(felder * punkte);
        for s in b[8..].chunks_exact(12) {
            let x = f32::from_le_bytes(s[0..4].try_into().unwrap()) as f64;
            let y = f32::from_le_bytes(s[4..8].try_into().unwrap()) as f64;
            let z = f32::from_le_bytes(s[8..12].try_into().unwrap()) as f64;
            // Zentimeter -> Millimeter, wie in `Netz`.
            vektoren.push([x * 10.0, y * 10.0, z * 10.0]);
        }
        Ok(Formung {
            felder,
            punkte,
            vektoren,
        })
    }

    /// Anzahl der Felder einschliesslich des Nullpunktfelds.
    pub fn felder(&self) -> usize {
        self.felder
    }

    /// Anzahl der Punkte, zu denen die Felder gehören.
    pub fn punkte(&self) -> usize {
        self.punkte
    }

    /// Verschiebung von Punkt `i` durch Feld `k`, in Millimetern je Einheit.
    pub fn feld(&self, k: usize, i: usize) -> Punkt3 {
        self.vektoren[k * self.punkte + i]
    }

    /// Wendet die Formung an: `P = Grundnetz + F_letzt + Σ F_k · c_k`.
    ///
    /// `c` muss `felder − 1` Einträge haben. Ein zu kurzer Vektor wird
    /// als Fehler behandelt, ein zu langer ebenfalls — stilles Abschneiden
    /// würde eine falsche Feldreihenfolge verschleiern.
    pub fn anwenden(&self, grund: &Netz, c: &[f64]) -> Netz {
        assert_eq!(
            c.len(),
            self.felder - 1,
            "{} Koeffizienten für {} Felder",
            c.len(),
            self.felder
        );
        assert_eq!(
            grund.punkte.len(),
            self.punkte,
            "Netz und Felder gehören nicht zusammen"
        );
        let mut netz = grund.clone();
        for (i, p) in netz.punkte.iter_mut().enumerate() {
            let mut q = [0.0f64; 3];
            for k in 0..self.felder - 1 {
                let f = self.feld(k, i);
                for a in 0..3 {
                    q[a] += f[a] * c[k];
                }
            }
            let letzt = self.feld(self.felder - 1, i);
            for a in 0..3 {
                p[a] += q[a] + letzt[a];
            }
        }
        netz
    }

    /// Nur für Prüfungen: Formung aus rohen Vektoren (Millimeter).
    #[cfg(test)]
    pub(crate) fn aus_vektoren(punkte: usize, vektoren: Vec<Punkt3>) -> Formung {
        assert_eq!(vektoren.len() % punkte, 0);
        Formung {
            felder: vektoren.len() / punkte,
            punkte,
            vektoren,
        }
    }
}

/// Die Maßnamen einer `_DetailFeature_*.txt` in Feldreihenfolge (ID-Reihenfolge).
pub fn namen_vektor(txt: &Path) -> io::Result<Vec<String>> {
    let text = fs::read_to_string(txt)?;
    let mut zeilen = text
        .lines()
        .map(|z| z.trim().trim_start_matches('\u{feff}'))
        .filter(|z| !z.is_empty());
    let kopf = zeilen.next().unwrap_or("");
    if !kopf.starts_with("DETAIL_") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: erste Zeile ist kein Detail-Schlüssel: {kopf}", txt.display()),
        ));
    }
    let mut paare = Vec::new();
    for name in zeilen {
        let i = id(name).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: unbekannter Maßname {name}", txt.display()),
            )
        })?;
        paare.push((i, name.to_string()));
    }
    paare.sort_by_key(|(i, _)| *i);
    for fenster in paare.windows(2) {
        if fenster[0].0 == fenster[1].0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: Maßname doppelt", txt.display()),
            ));
        }
    }
    Ok(paare.into_iter().map(|(_, name)| name).collect())
}

/// Baut den Referenzvektor aus einer `_DetailFeature_*.txt`.
///
/// Die Datei hat in Zeile 0 den Konfigurationsschlüssel, danach einen
/// Maßnamen je Zeile. Die Reihenfolge der Namen ist **nicht** die
/// Feldreihenfolge; sortiert wird über [`id`]. Namen ohne Referenzwert
/// (z. B. die männliche Stufe) sind ein Fehler.
pub fn referenz_vektor(txt: &Path) -> io::Result<Vec<f64>> {
    namen_vektor(txt)?
        .into_iter()
        .map(|name| {
            referenzwert(&name).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: kein Referenzwert für {name}", txt.display()),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn netz_aus(punkte: Vec<Punkt3>) -> Netz {
        Netz {
            punkte,
            dreiecke: Vec::new(),
        }
    }

    /// Zwei Felder: das erste verschiebt in x, das letzte ist der Nullpunkt.
    /// `c = 2` muss exakt das Doppelte des ersten Feldes ergeben.
    #[test]
    fn letztes_feld_hat_koeffizient_eins() {
        let f = Formung::aus_vektoren(
            2,
            vec![
                [10.0, 0.0, 0.0],
                [0.0, 20.0, 0.0],
                [1.0, 2.0, 3.0],
                [4.0, 5.0, 6.0],
            ],
        );
        let g = netz_aus(vec![[100.0, 100.0, 100.0], [200.0, 200.0, 200.0]]);
        let p = f.anwenden(&g, &[2.0]);
        assert_eq!(p.punkte[0], [100.0 + 20.0 + 1.0, 100.0 + 2.0, 100.0 + 3.0]);
        // Punkt 1: Feld 0 ist [0, 20, 0], das letzte Feld [4, 5, 6].
        assert_eq!(p.punkte[1], [200.0 + 4.0, 200.0 + 40.0 + 5.0, 200.0 + 6.0]);
    }

    #[test]
    fn falsche_koeffizientenzahl_panikt() {
        let f = Formung::aus_vektoren(1, vec![[1.0, 0.0, 0.0], [0.0, 0.0, 0.0]]);
        let g = netz_aus(vec![[0.0, 0.0, 0.0]]);
        let ergebnis = std::panic::catch_unwind(|| f.anwenden(&g, &[1.0, 2.0]));
        assert!(ergebnis.is_err());
    }

    #[test]
    fn id_tabelle_ist_vollstaendig() {
        assert_eq!(IDS.len(), 58);
        for (soll, (_, i)) in IDS.iter().enumerate() {
            assert_eq!(soll, *i as usize);
        }
        assert_eq!(id("HEIGHT_Total"), Some(0));
        assert_eq!(id("LENGTH_VerticalTrunk"), Some(57));
        assert_eq!(id("GIBT_ES_NICHT"), None);
    }

    #[test]
    fn referenz_hat_32_werte_und_beginnt_mit_der_hoehe() {
        assert_eq!(REFERENZ.len(), 32);
        assert_eq!(REFERENZ[0].0, "HEIGHT_Total");
        assert_eq!(REFERENZ[0].1, 163.95);
    }
}
