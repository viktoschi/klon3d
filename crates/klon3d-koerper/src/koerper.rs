//! Der Körper der Anwenderin aus ihrer Maßdatei: Eingang von `klon3d.exe`
//! und später der Simulation.
//!
//! Erster Weg ist die Formungskette nach CLO ([`crate::kette`], Befund 31).
//! Fehlen im Resizing-Ordner Daten der Kette (Human-Liste, Messdaten,
//! Symmetrie, Hand), formt der alte Weg über [`crate::person`] und
//! [`crate::anpassen`] (Befunde 23, 24) mit der Schnittkontur. Er braucht nur
//! `BaseMesh.obj` und die Felder von `DETAIL_Advanced`. Ausgegeben wird in
//! beiden Fällen die Gruppe `body` (Punkte 0 … 31 421) in mm mit ihren
//! Dreiecken.

use std::path::{Path, PathBuf};

use crate::anpassen::anpassen;
use crate::formung::{namen_vektor, referenz_vektor, Formung};
use crate::kette::{Kette, KOERPER};
use crate::person::zielperson;
use crate::{lies_obj, Netz, Punkt3};

/// CLOs Resizing-Ordner der Standardinstallation.
pub const STANDARD_ORDNER: &str =
    r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female";

/// Resizing-Ordner: `CLO_FEMALE`, sonst [`STANDARD_ORDNER`].
pub fn ordner() -> PathBuf {
    std::env::var_os("CLO_FEMALE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(STANDARD_ORDNER))
}

/// Womit der Körper geformt wurde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weg {
    /// Formungskette nach CLO (`kette.rs`).
    Kette,
    /// Alter Weg, Newton-Anpassung auf die Schnittkontur (`anpassen.rs`).
    Anpassen,
}

impl Weg {
    pub fn name(self) -> &'static str {
        match self {
            Weg::Kette => "Formungskette",
            Weg::Anpassen => "anpassen (Fallback)",
        }
    }
}

/// Der geformte Körper.
pub struct Koerper {
    /// Gruppe `body` in mm; alle Dreiecksindizes < [`KOERPER`].
    pub netz: Netz,
    pub weg: Weg,
    /// Was beim Formen auffiel, Zeile für Zeile (Grund des Fallbacks,
    /// Formungen, Ende-Grund).
    pub meldungen: Vec<String>,
    /// Bei der Kette ihr Ergebnis (E-Folgen, 58 Maße).
    pub kette: Option<crate::kette::Ergebnis>,
}

/// Formt den Körper aus SeamlyMe-Maßen; `wert` fragt ein Maß nach seinem
/// Namen ab, in cm.
///
/// Der Fallback greift nur, wenn [`Kette::laden`] scheitert. Scheitert die
/// Kette selbst an den Maßen, ist das ein Fehler.
pub fn aus_smis(ordner: &Path, wert: &dyn Fn(&str) -> Option<f64>) -> Result<Koerper, String> {
    match Kette::laden(ordner) {
        Ok(k) => {
            let e = k.aus_smis(wert)?;
            let meldungen = vec![
                format!(
                    "BASE: {} Formungen, {}",
                    e.base.formungen,
                    e.base.ende.als_text()
                ),
                format!(
                    "DETAIL: {} Formungen, {}",
                    e.detail.formungen,
                    e.detail.ende.als_text()
                ),
                format!("Handanteil t = {:.4}", e.hand_t),
            ];
            let netz = Netz {
                punkte: Kette::koerper_mm(&e.detail.punkte),
                dreiecke: k.dreiecke.clone(),
            };
            Ok(Koerper {
                netz,
                weg: Weg::Kette,
                meldungen,
                kette: Some(e),
            })
        }
        Err(grund) => {
            let mut k = alter_weg(ordner, wert)?;
            k.meldungen
                .insert(0, format!("Formungskette nicht ladbar aus {}: {grund}", ordner.display()));
            Ok(k)
        }
    }
}

/// Was die Platzierung (Stufe 4c) über den geformten Körper wissen muss: die
/// Punkte von Rumpf und Armen nach CLOs Segmenten (`range_list.rlist`) und
/// die Landmarken aus `IndexSet.txt` (mm, Rahmen von CLO). Die Zeilen sind
/// die der 5.2-Fassung (`spez/messung.md`), passend zur
/// Formungskette.
#[derive(Clone, Debug)]
pub struct Anordnung {
    pub rumpf: Vec<usize>,
    /// links, rechts
    pub arme: [Vec<usize>; 2],
    /// L4 vordere Halsmitte (Drosselgrube), L5 hintere Halsmitte (7. Halswirbel)
    pub hals_vorn: Punkt3,
    pub hals_hinten: Punkt3,
    /// L6, L7 seitlicher Halspunkt links, rechts
    pub hals_seitlich: [Punkt3; 2],
    /// L8, L9 Schulterpunkt links, rechts
    pub schulter: [Punkt3; 2],
    /// L49, L52 Ellbogen hinten links, rechts
    pub ellbogen: [Punkt3; 2],
}

/// Segmente und Landmarken zum geformten `netz` aus dem Resizing-Ordner.
pub fn anordnung(ordner: &Path, netz: &Netz) -> Result<Anordnung, String> {
    use crate::massband::{segment, Messdaten};
    let d = Messdaten::laden(ordner).map_err(|e| format!("Segmente und Landmarken: {e}"))?;
    let n = netz.punkte.len();
    if d.segment.len() < n {
        return Err(format!("{} Segmente für {n} Punkte", d.segment.len()));
    }
    let teil = |s: i32| (0..n).filter(|&i| d.segment[i] == s).collect::<Vec<_>>();
    let lm = |zeile: usize| {
        d.landmarke(zeile)
            .filter(|&i| i < n)
            .map(|i| netz.punkte[i])
            .ok_or_else(|| format!("Landmarke L{zeile} fehlt"))
    };
    Ok(Anordnung {
        rumpf: teil(segment::RUMPF),
        arme: [teil(segment::ARM_LINKS), teil(segment::ARM_RECHTS)],
        hals_vorn: lm(4)?,
        hals_hinten: lm(5)?,
        hals_seitlich: [lm(6)?, lm(7)?],
        schulter: [lm(8)?, lm(9)?],
        ellbogen: [lm(49)?, lm(52)?],
    })
}

/// Der alte Weg wie `examples/forme.rs`, ohne Maßband (das braucht die
/// Messdaten, die der Kette dann meist auch fehlen).
fn alter_weg(ordner: &Path, wert: &dyn Fn(&str) -> Option<f64>) -> Result<Koerper, String> {
    let txt = ordner.join("_DetailFeature_Advanced_Bust_TotalHeight.txt");
    let bd = ordner.join("ConvertingMat_DETAIL_Advanced_TotalHeight.bd");
    let grund = lies_obj(&ordner.join("BaseMesh.obj"), 10.0)
        .map_err(|e| format!("BaseMesh.obj: {e}"))?;
    let formung =
        Formung::laden(&bd, grund.punkte.len()).map_err(|e| format!("Formungsdatei: {e}"))?;
    let c_ref = referenz_vektor(&txt).map_err(|e| format!("Referenzvektor: {e}"))?;
    let namen = namen_vektor(&txt).map_err(|e| format!("Maßnamen: {e}"))?;
    let p = zielperson(&grund, &namen, &c_ref, wert)?;
    let e = anpassen(&grund, &formung, &p.c_start, &p.linien, &p.ziel, &p.aktiv)?;
    let mut geformt = formung.anwenden(&grund, &e.c);
    geformt.punkte.truncate(KOERPER.min(geformt.punkte.len()));
    let n = geformt.punkte.len();
    geformt.dreiecke.retain(|d| d.iter().all(|&i| i < n));
    let mut meldungen = vec![format!(
        "{} Runden{}",
        e.runden,
        if e.divergiert { ", divergiert" } else { "" }
    )];
    for (name, f) in p.linien.namen().iter().zip(&e.fehler) {
        meldungen.push(format!("{name}: Fehler {f:+.3} cm"));
    }
    Ok(Koerper {
        netz: geformt,
        weg: Weg::Anpassen,
        meldungen,
        kette: None,
    })
}
