//! Körperformung wie CLO: aus 58 Maßen ein Netz (`formung.md`).
//!
//! Zwei Stufen, BASE und DETAIL. Jede formt linear vom unveränderten
//! Basisnetz aus, symmetrisiert, zentriert, misst die 58 Maße und regelt die
//! Merkmale nach, bis die Sollwerte getroffen sind. Danach die Handkorrektur.
//! Alles in Zentimetern; erst die Ausgabe ([`Kette::koerper_mm`]) rechnet in
//! Millimeter um.
//!
//! Konfiguration fest: Anker TotalHeight, Breite Bust, Detail Human
//! (`formung.md` 4). Die Listen der anderen Konfigurationen lassen sich mit
//! [`Stufe::laden`] trotzdem laden; geprüft ist zusätzlich Basic (Emma).

use crate::massband::Messdaten;
use crate::messung::{messen, platz, Messung, ANZAHL};
use crate::{lies_obj, Punkt3};
use std::fs;
use std::io;
use std::path::Path;

/// Punkte des ganzen Netzes (`formung.md`, Schreibweise).
pub const PUNKTE: usize = 47_650;
/// Punkte der Gruppe `body`: 0 … 31 421 (`formung.md` 14).
pub const KOERPER: usize = 31_422;
/// Der Partner, den die Haarpunkte ohne Eintrag bekommen (`formung.md` 2.5).
const HAAR_PARTNER: usize = 39_635;
/// CIRCUMFERENCE_HighHip und LENGTH_Waist_HighHip.
const HUEFTE_OBEN: usize = 7;
const LAENGE_TAILLE_HUEFTE_OBEN: usize = 42;
/// SeamlyMe-Name je Platz, `massdefinitionen.md` („klon3d setzt“). Plätze
/// ohne Eintrag bleiben 0; HighHip (7) leitet [`Kette::aus_smis`] ab.
pub const SMIS: [(usize, &str); 34] = [
    (0, "height"),
    (2, "bust_circ"),
    (3, "neck_mid_circ"),
    (4, "neck_circ"),
    (5, "lowbust_circ"),
    (6, "waist_circ"),
    (8, "hip_circ"),
    (9, "leg_thigh_upper_circ"),
    (10, "leg_thigh_mid_circ"),
    (11, "leg_knee_circ"),
    (12, "leg_calf_circ"),
    (13, "leg_ankle_circ"),
    (15, "arm_upper_circ"),
    (16, "arm_elbow_circ"),
    (17, "arm_wrist_circ"),
    (18, "height_neck_side"),
    (19, "height_shoulder_tip"),
    (20, "shoulder_slope_neck_side_length"),
    (21, "height_bustpoint"),
    (23, "height_waist_side"),
    (25, "height_hip"),
    (26, "leg_crotch_to_floor"),
    (29, "height_knee"),
    (30, "height_calf"),
    (31, "height_ankle"),
    (34, "across_chest_f"),
    (36, "shoulder_tip_to_shoulder_tip_b"),
    (40, "neck_front_to_waist_f"),
    (41, "neck_back_to_waist_b"),
    (43, "waist_to_hip_side"),
    (44, "bustpoint_to_bustpoint"),
    (45, "bustpoint_to_neck_side"),
    (47, "arm_shoulder_tip_to_wrist"),
    (49, "crotch_length"),
];
/// Höchstzahl der Formungen einer Stufe ohne Rückfall.
const HOECHSTENS: usize = 20;

fn ungueltig(pfad: &Path, was: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {was}", pfad.display()),
    )
}

/// Gewicht und Umfänge stehen im Merkmalsraum als Wurzel (`formung.md` 5).
fn wurzel(j: usize) -> bool {
    (1..=17).contains(&j)
}

/// cm → Merkmal.
pub fn g(j: usize, v: f64) -> f64 {
    if wurzel(j) {
        v.sqrt()
    } else {
        v
    }
}

/// Merkmal → cm.
pub fn f(j: usize, x: f64) -> f64 {
    if wurzel(j) {
        x * x
    } else {
        x
    }
}

/// Eine Stufe: Maske und Formungsmatrix (`formung.md` 2.6, 2.7).
pub struct Stufe {
    /// Zeile 0 der Liste, zugleich Name der Matrix.
    pub name: String,
    /// Gesetzte Plätze, aufsteigend; Spalte k gehört zu `maske[k]`.
    pub maske: Vec<usize>,
    /// Spaltenweise, K · 3N Werte; die letzte Spalte ist der Achsenabschnitt.
    spalten: Vec<f32>,
    r: usize,
}

impl Stufe {
    /// Liest `liste` (eine `_BaseFeature_…`- oder `_DetailFeature_…`-Datei)
    /// und die Matrix aus demselben Ordner.
    pub fn laden(ordner: &Path, liste: &str) -> io::Result<Stufe> {
        let pfad = ordner.join(liste);
        let text = fs::read_to_string(&pfad)?;
        let mut zeilen = text
            .split('\n')
            .map(|z| z.trim_end_matches('\r'))
            .filter(|z| !z.is_empty());
        let name = zeilen
            .next()
            .ok_or_else(|| ungueltig(&pfad, "leer"))?
            .to_string();
        let mut gesetzt = [false; ANZAHL];
        for z in zeilen {
            // Exakter Vergleich; unbekannte Namen zählen nicht.
            if let Some(j) = platz(z) {
                gesetzt[j] = true;
            }
        }
        let maske: Vec<usize> = (0..ANZAHL).filter(|&j| gesetzt[j]).collect();

        let bd = ordner.join(format!("ConvertingMat_{name}.bd"));
        let roh = fs::read(&bd)?;
        if roh.len() < 8 {
            return Err(ungueltig(&bd, "zu kurz"));
        }
        let k = i32::from_le_bytes(roh[0..4].try_into().unwrap()) as usize;
        let r = i32::from_le_bytes(roh[4..8].try_into().unwrap()) as usize;
        if r != 3 * PUNKTE || roh.len() != 8 + 4 * k * r {
            return Err(ungueltig(
                &bd,
                format!("K = {k}, R = {r}, {} Byte", roh.len()),
            ));
        }
        if k != maske.len() + 1 {
            return Err(ungueltig(
                &pfad,
                format!("{} Merkmale, Matrix hat {k} Spalten", maske.len()),
            ));
        }
        let spalten = roh[8..]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        Ok(Stufe {
            name,
            maske,
            spalten,
            r,
        })
    }

    fn wert(&self, k: usize, i: usize, c: usize) -> f64 {
        self.spalten[k * self.r + 3 * i + c] as f64
    }
}

/// Warum die Kalibrierung einer Stufe aufgehört hat (`formung.md` 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ende {
    /// E < 0,01 · N_s.
    Getroffen,
    /// E stieg um mehr als 1 cm; zurück auf die vorigen Merkmale.
    Rueckfall,
    /// 20 Formungen.
    Hoechstzahl,
    /// Ein nachgeführtes Maß wäre negativ geworden.
    Negativ,
}

impl Ende {
    /// Bezeichnung wie in `pruefwerte.json`.
    pub fn als_text(self) -> &'static str {
        match self {
            Ende::Getroffen => "E<N*0.01",
            Ende::Rueckfall => "E>E_vor+1",
            Ende::Hoechstzahl => "n>=20",
            Ende::Negativ => "X<0",
        }
    }
}

/// Ergebnis einer kalibrierten Stufe.
pub struct Kalibrierung {
    pub punkte: Vec<Punkt3>,
    /// Schlussmessung auf `punkte`.
    pub messung: Messung,
    /// Ein E-Wert je Formung (Schritt 4); der Rückfall hat keinen.
    pub e_folge: Vec<f64>,
    pub ende: Ende,
    pub formungen: usize,
    /// Merkmale der letzten Formung (vor dem Zurücksetzen auf die Sollwerte).
    pub f_ende: [f64; ANZAHL],
}

/// Ergebnis der ganzen Kette.
pub struct Ergebnis {
    /// Die 58 Vorgaben, mit denen die DETAIL-Stufe lief.
    pub eingabe: [f64; ANZAHL],
    pub base: Kalibrierung,
    /// DETAIL-Stufe; `punkte` schon mit Handkorrektur.
    pub detail: Kalibrierung,
    /// Startvektor der DETAIL-Stufe im Merkmalsraum.
    pub f_detail_start: [f64; ANZAHL],
    /// Anteil der Handkorrektur (`formung.md` 12).
    pub hand_t: f64,
}

impl Ergebnis {
    /// Die 58 Endwerte, cm (Gewicht kg).
    pub fn werte(&self) -> &[f64; ANZAHL] {
        &self.detail.messung.werte
    }
}

/// Die festen Daten der Formung aus CLOs Resizing-Ordner.
pub struct Kette {
    /// Basisnetz B, cm, wie in `BaseMesh.obj`.
    pub basis: Vec<Punkt3>,
    /// Dreiecke der Gruppe `body` (alle Indizes < [`KOERPER`]).
    pub dreiecke: Vec<[usize; 3]>,
    pub messdaten: Messdaten,
    partner: Vec<usize>,
    hand_min: Vec<Punkt3>,
    hand_max: Vec<Punkt3>,
    pub base: Stufe,
    pub detail: Stufe,
}

impl Kette {
    /// Lädt alles für TotalHeight / Bust / Human.
    pub fn laden(ordner: &Path) -> io::Result<Kette> {
        Kette::laden_mit(
            ordner,
            "_BaseFeature_Bust_TotalHeight.txt",
            "_DetailFeature_Human_Bust_TotalHeight.txt",
        )
    }

    /// Lädt mit anderen Listen (zum Prüfen gegen Emma, Detail Basic).
    pub fn laden_mit(ordner: &Path, base: &str, detail: &str) -> io::Result<Kette> {
        let obj = ordner.join("BaseMesh.obj");
        let netz = lies_obj(&obj, 1.0)?;
        if netz.punkte.len() != PUNKTE {
            return Err(ungueltig(&obj, format!("{} Punkte", netz.punkte.len())));
        }
        let dreiecke = netz
            .dreiecke
            .iter()
            .copied()
            .filter(|d| d.iter().all(|&i| i < KOERPER))
            .collect();
        Ok(Kette {
            basis: netz.punkte,
            dreiecke,
            messdaten: Messdaten::laden(ordner)?,
            partner: lies_partner(&ordner.join("symmetry_data.sym"))?,
            hand_min: lies_hand(&ordner.join("hand_min.txt"))?,
            hand_max: lies_hand(&ordner.join("hand_max.txt"))?,
            base: Stufe::laden(ordner, base)?,
            detail: Stufe::laden(ordner, detail)?,
        })
    }

    /// Lineare Formung, Symmetrisieren, Zentrieren (`formung.md` 6–8).
    pub fn formen(&self, s: &Stufe, merkmal: &[f64; ANZAHL]) -> Vec<Punkt3> {
        let letzte = s.maske.len();
        let roh: Vec<Punkt3> = (0..PUNKTE)
            .map(|i| {
                let mut p = self.basis[i];
                for c in 0..3 {
                    let mut d = 0.0;
                    for (k, &j) in s.maske.iter().enumerate() {
                        d += merkmal[j] * s.wert(k, i, c);
                    }
                    p[c] = p[c] + d + s.wert(letzte, i, c);
                }
                p
            })
            .collect();

        let mut v: Vec<Punkt3> = (0..PUNKTE)
            .map(|i| {
                let (p, t) = (roh[i], self.partner[i]);
                if t == i {
                    [0.0, p[1], p[2]]
                } else {
                    let q = roh[t];
                    [
                        (p[0] - q[0]) / 2.0,
                        (p[1] + q[1]) / 2.0,
                        (p[2] + q[2]) / 2.0,
                    ]
                }
            })
            .collect();

        let n = PUNKTE as f64;
        let mx = v.iter().map(|p| p[0]).sum::<f64>() / n;
        let mz = v.iter().map(|p| p[2]).sum::<f64>() / n;
        let my = v.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        for p in &mut v {
            p[0] -= mx;
            p[1] -= my;
            p[2] -= mz;
        }
        v
    }

    /// Eine Stufe nachregeln (`formung.md` 10). `merkmal` steht danach wieder
    /// auf den Sollwerten.
    pub fn kalibrieren(&self, s: &Stufe, merkmal: &[f64; ANZAHL]) -> Result<Kalibrierung, String> {
        let soll = *merkmal;
        let mut fm = *merkmal;
        let mut f_vor = *merkmal;
        let mut e_vor = 1000.0;
        let mut e_folge = Vec::new();
        let grenze = 0.01 * s.maske.len() as f64;
        let aktiv: Vec<usize> = s.maske.iter().copied().filter(|&j| j != 1).collect();

        let mut punkte;
        let mut n = 0;
        let ende = loop {
            punkte = self.formen(s, &fm);
            let m = messen(&punkte, &self.messdaten)?;
            let e: f64 = aktiv
                .iter()
                .map(|&j| (m.werte[j] - f(j, soll[j])).abs())
                .sum();
            e_folge.push(e);
            if e < grenze {
                break Ende::Getroffen;
            }
            n += 1;
            if e > e_vor + 1.0 {
                fm = f_vor;
                punkte = self.formen(s, &fm);
                break Ende::Rueckfall;
            }
            if n >= HOECHSTENS {
                break Ende::Hoechstzahl;
            }
            let mut negativ = false;
            for &j in &aktiv {
                f_vor[j] = fm[j];
                let x = f(j, fm[j]) + f(j, soll[j]) - m.werte[j];
                if x < 0.0 {
                    negativ = true;
                    break;
                }
                fm[j] = g(j, x);
            }
            if negativ {
                break Ende::Negativ;
            }
            e_vor = e;
        };
        let formungen = e_folge.len() + usize::from(ende == Ende::Rueckfall);
        let messung = messen(&punkte, &self.messdaten)?;
        Ok(Kalibrierung {
            punkte,
            messung,
            e_folge,
            ende,
            formungen,
            f_ende: fm,
        })
    }

    /// Die ganze Kette aus 58 Maßen (`formung.md` 11, 12). `v` in cm
    /// (Gewicht kg), 0 = nicht vorgegeben; Höhe und Brust müssen gesetzt sein.
    pub fn aus_massen(&self, v: &[f64; ANZAHL]) -> Result<Ergebnis, String> {
        self.kette(*v, |_, _| Ok(()))
    }

    /// Die ganze Kette aus SeamlyMe-Maßen nach `massdefinitionen.md`
    /// („klon3d setzt“). `wert` fragt ein Maß nach seinem SeamlyMe-Namen ab,
    /// in cm.
    ///
    /// HighHip (Platz 7) misst CLO nicht am Beckenkamm, sondern am
    /// Kandidaten nächst der halben Höhe Taille–Hüfte. Den Abstand d dorthin
    /// (LENGTH_Waist_HighHip) liefert erst das BASE-Netz; der Umfang an
    /// dieser Lage wird zwischen `highhip_circ` (bei `waist_to_highhip_side`)
    /// und `hip_circ` (bei `waist_to_hip_side`) linear eingeschoben. Fehlt
    /// eins der vier Maße, bleibt der Platz 0 (BASE-Vorhersage).
    pub fn aus_smis(&self, wert: &dyn Fn(&str) -> Option<f64>) -> Result<Ergebnis, String> {
        let mut v = [0.0; ANZAHL];
        for &(j, name) in &SMIS {
            v[j] = wert(name).unwrap_or(0.0);
        }
        for &j in &self.base.maske {
            if !(v[j] > 0.0) {
                return Err(format!(
                    "Maß {:?} fehlt in der Maßdatei",
                    SMIS.iter().find(|s| s.0 == j).map_or("?", |s| s.1)
                ));
            }
        }
        let huefte_oben = (
            wert("highhip_circ"),
            wert("hip_circ"),
            wert("waist_to_highhip_side"),
            wert("waist_to_hip_side"),
        );
        self.kette(v, |base, v| {
            if let (Some(u0), Some(u1), Some(a0), Some(a1)) = huefte_oben {
                if a1 == a0 {
                    return Err("waist_to_highhip_side = waist_to_hip_side".into());
                }
                let d = base.werte[LAENGE_TAILLE_HUEFTE_OBEN];
                v[HUEFTE_OBEN] = u0 + (u1 - u0) * (d - a0) / (a1 - a0);
            }
            Ok(())
        })
    }

    /// `nach_base` darf die Vorgaben ändern, sobald das BASE-Netz gemessen ist.
    fn kette(
        &self,
        mut v: [f64; ANZAHL],
        nach_base: impl FnOnce(&Messung, &mut [f64; ANZAHL]) -> Result<(), String>,
    ) -> Result<Ergebnis, String> {
        for &j in &self.base.maske {
            if !(v[j] > 0.0) {
                return Err(format!("Maß {j} der BASE-Stufe fehlt"));
            }
        }
        let mut fm = [0.0; ANZAHL];
        for &j in &self.base.maske {
            fm[j] = g(j, v[j]);
        }
        let base = self.kalibrieren(&self.base, &fm)?;
        nach_base(&base.messung, &mut v)?;

        for j in 0..ANZAHL {
            if !self.base.maske.contains(&j) {
                fm[j] = g(j, base.messung.werte[j]);
            }
        }
        for &j in &self.detail.maske {
            if v[j] != 0.0 {
                fm[j] = g(j, v[j]);
            }
        }
        let mut detail = self.kalibrieren(&self.detail, &fm)?;

        let hand_t = 0.5 * ((detail.messung.werte[0] - 140.0) / 110.0).clamp(0.0, 1.0);
        for (i, p) in detail.punkte.iter_mut().enumerate() {
            let (lo, hi) = (self.hand_min[i], self.hand_max[i]);
            for c in 0..3 {
                p[c] += lo[c] + hand_t * (hi[c] - lo[c]);
            }
        }
        Ok(Ergebnis {
            eingabe: v,
            base,
            detail,
            f_detail_start: fm,
            hand_t,
        })
    }

    /// Punkte der Gruppe `body` in mm (`formung.md` 14).
    pub fn koerper_mm(punkte: &[Punkt3]) -> Vec<Punkt3> {
        punkte[..KOERPER]
            .iter()
            .map(|p| [p[0] * 10.0, p[1] * 10.0, p[2] * 10.0])
            .collect()
    }
}

/// `symmetry_data.sym`; fehlende Einträge (Haar) bekommen 39 635.
fn lies_partner(pfad: &Path) -> io::Result<Vec<usize>> {
    let text = fs::read_to_string(pfad)?;
    let mut s = Vec::with_capacity(PUNKTE);
    for t in text.split_whitespace() {
        let z: usize = t
            .parse()
            .map_err(|e| ungueltig(pfad, format!("{t:?}: {e}")))?;
        if z >= PUNKTE {
            return Err(ungueltig(pfad, format!("Partner {z}")));
        }
        s.push(z);
    }
    if s.len() > PUNKTE {
        return Err(ungueltig(pfad, format!("{} Einträge", s.len())));
    }
    s.resize(PUNKTE, HAAR_PARTNER);
    Ok(s)
}

/// `hand_min.txt` / `hand_max.txt`: je Punkt eine Verschiebung in cm.
fn lies_hand(pfad: &Path) -> io::Result<Vec<Punkt3>> {
    let text = fs::read_to_string(pfad)?;
    let mut v = Vec::with_capacity(PUNKTE);
    for z in text.lines().filter(|z| !z.trim().is_empty()) {
        let mut p = [0.0; 3];
        let mut teile = z.split_whitespace();
        for c in &mut p {
            let t = teile
                .next()
                .ok_or_else(|| ungueltig(pfad, "Zeile zu kurz"))?;
            *c = t
                .parse()
                .map_err(|e| ungueltig(pfad, format!("{t:?}: {e}")))?;
        }
        v.push(p);
    }
    if v.len() != PUNKTE {
        return Err(ungueltig(pfad, format!("{} Zeilen", v.len())));
    }
    Ok(v)
}

/// Kopf einer `.avs`: (Anker, Detail, Breite) wie in `formung.md` 2.9.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvsKopf {
    pub anker: i32,
    pub detail: i32,
    pub breite: i32,
}

/// Liest eine Avatar-Maßdatei nach der robusten Leseregel (`formung.md` 2.9).
pub fn lies_avs(pfad: &Path) -> io::Result<(AvsKopf, [f64; ANZAHL])> {
    let b = fs::read(pfad)?;
    let finde = |text: &[u8], ab: usize| -> Option<usize> {
        (ab..b.len().saturating_sub(text.len())).find(|&i| &b[i..i + text.len()] == text)
    };
    let int_nach = |text: &[u8], ausser: Option<&[u8]>| -> io::Result<i32> {
        let mut ab = 0;
        while let Some(i) = finde(text, ab) {
            let teil = ausser.is_some_and(|w| {
                let d = w.len() - text.len();
                i >= d && &b[i - d..i + text.len()] == w
            });
            if !teil {
                let o = i + text.len();
                return Ok(i32::from_le_bytes(b[o..o + 4].try_into().unwrap()));
            }
            ab = i + 1;
        }
        Err(ungueltig(
            pfad,
            format!("{} fehlt", String::from_utf8_lossy(text)),
        ))
    };
    let kopf = AvsKopf {
        anker: int_nach(b"BaseFeature", Some(b"WidthBaseFeature"))?,
        detail: int_nach(b"DetailFeature", None)?,
        breite: int_nach(b"WidthBaseFeature", None)?,
    };
    let l = finde(b"listFeatureValues:", 0).ok_or_else(|| ungueltig(pfad, "keine Werte"))?;
    let mut o = l + "listFeatureValues:".len() + 4;
    let mut n = 0;
    while o + 4 <= b.len() && b[o..o + 4] == [4, 0, 0, 0] {
        n += 1;
        o += 4;
    }
    if n != ANZAHL || o + 4 * n > b.len() {
        return Err(ungueltig(pfad, format!("{n} Werte")));
    }
    let mut v = [0.0; ANZAHL];
    for (j, w) in v.iter_mut().enumerate() {
        let x = f32::from_le_bytes(b[o + 4 * j..o + 4 * j + 4].try_into().unwrap()) as f64;
        // CLO nimmt nur 0 … 300 an.
        *w = if (0.0..=300.0).contains(&x) { x } else { 0.0 };
    }
    Ok((kopf, v))
}
