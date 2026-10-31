//! Körper an Zielmaße anpassen — CLOs Schleife mit unseren Messwerten.
//!
//! CLO iteriert `c += (Ziel − Ist)` mit einer festen Schrittweite von 1
//! (Befund 22). Das ist ein Festpunkt-Schritt mit
//! diagonaler Näherung der Jacobi-Matrix. Hier steht dieselbe Schleife,
//! aber mit der vollständigen Jacobi-Matrix der **aktiven Felder**:
//!
//! ```text
//! c[0] = Zielhöhe                        (Höhe ist der Koeffizient, cm)
//! A[i][j] = ∂(Maß i)/∂(c_aktiv[j])       (je Runde neu, Schritt 1,0)
//! δ = (A + λI)⁻¹ (Ziel − Ist)            (gedämpftes Newton, m × m)
//! c_aktiv += δ                           (mit Rücknahme)
//! ```
//!
//! Aktiv sind die Felder, die zu den Messlinien gehören
//! ([`Messort::feld`]) — bei den fünf Rumpflinien also `CIRCUMFERENCE_Bust`
//! bis `CIRCUMFERENCE_LowHip`. Die übrigen Felder bleiben stehen; sie mit
//! zu optimieren hat sich als instabil erwiesen (sie zogen etwa Wade und
//! Fuß mit, um die Taille zu treffen).
//!
//! Gemessen wird entweder auf **mitwandernden Landmarkenlinien**
//! ([`crate::spur`]) oder auf **festen Schnitten** bei einer bekannten
//! Zielhöhe. Letzteres ist richtig, sobald die Maßdatei die Höhen liefert:
//! dann setzt der Aufrufer auch die `HEIGHT_*`-Felder auf dieselben Werte.
//!
//! Gemessen wird die Länge der Schnittkontur — oder, mit
//! [`Messlinien::mit_massband`], wie ein Maßband: konvexe Hülle des
//! waagerechten Rumpfschnitts auf derselben Höhe ([`crate::massband`]).

use crate::formung::Formung;
use crate::massband::{rumpf_waagerecht, zentriert_cm, Messdaten};
use crate::spur::{spur, Spur};
use crate::{rumpf, Netz};
use std::sync::Arc;

/// Höhe der CLO-Referenzavatarin in Zentimetern (Befund 22).
pub const HOEHE_REF: f64 = 163.95;
/// Höhe der Brust (Apex) an der Referenz, cm.
pub const BRUSTHOEHE_REF: f64 = 119.4119;
/// Höhe der Unterbrust an der Referenz, cm.
pub const UNTERBRUSTHOEHE_REF: f64 = 113.1081;
/// Höhe der Taille an der Referenz, cm.
pub const TAILLENHOEHE_REF: f64 = 105.7056;
/// Höhe der High Hip an der Referenz, cm.
pub const HIGHHIPHOEHE_REF: f64 = 93.8775;
/// Höhe der Hüfte (Low Hip, Sitzhöhe) an der Referenz, cm.
pub const HUEFTHOEHE_REF: f64 = 82.2780;

/// Eine waagerechte Rumpfmesslinie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Messort {
    /// Brust / Apex.
    Brust,
    /// Unterbrust.
    Unterbrust,
    /// Taille.
    Taille,
    /// High Hip.
    HighHip,
    /// Hüfte / Low Hip.
    Huefte,
}

impl Messort {
    /// Alle Rumpflinien, von oben nach unten.
    pub const ALLE: [Messort; 5] = [
        Messort::Brust,
        Messort::Unterbrust,
        Messort::Taille,
        Messort::HighHip,
        Messort::Huefte,
    ];

    /// Name für Meldungen und Tabellen.
    pub fn name(self) -> &'static str {
        match self {
            Messort::Brust => "Brust",
            Messort::Unterbrust => "Unterbrust",
            Messort::Taille => "Taille",
            Messort::HighHip => "HighHip",
            Messort::Huefte => "Hüfte",
        }
    }

    /// CLO-Feld, das diese Linie steuert (Befund 22, ID-Tabelle).
    pub fn feld(self) -> &'static str {
        match self {
            Messort::Brust => "CIRCUMFERENCE_Bust",
            Messort::Unterbrust => "CIRCUMFERENCE_UnderBust",
            Messort::Taille => "CIRCUMFERENCE_Waist",
            Messort::HighHip => "CIRCUMFERENCE_HighHip",
            Messort::Huefte => "CIRCUMFERENCE_LowHip",
        }
    }

    /// Höhe der Referenzlinie am ungeformten Grundkörper, cm.
    pub fn hoehe(self) -> f64 {
        match self {
            Messort::Brust => BRUSTHOEHE_REF,
            Messort::Unterbrust => UNTERBRUSTHOEHE_REF,
            Messort::Taille => TAILLENHOEHE_REF,
            Messort::HighHip => HIGHHIPHOEHE_REF,
            Messort::Huefte => HUEFTHOEHE_REF,
        }
    }
}

/// Zielmaße in Zentimetern.
#[derive(Clone, Debug)]
pub struct Ziel {
    pub hoehe: f64,
    /// Zielumfänge in der Reihenfolge der Messlinien, cm.
    pub masse: Vec<f64>,
}

/// Gemessene Maße in Zentimetern.
#[derive(Clone, Debug)]
pub struct Ist {
    /// Umfänge in der Reihenfolge der Messlinien, cm.
    pub masse: Vec<f64>,
}

impl Ist {
    pub fn fehler(&self, ziel: &Ziel) -> Vec<f64> {
        assert_eq!(
            self.masse.len(),
            ziel.masse.len(),
            "Messlinien und Zielmaße passen nicht zusammen"
        );
        self.masse
            .iter()
            .zip(&ziel.masse)
            .map(|(ist, soll)| soll - ist)
            .collect()
    }

    pub fn groesster_fehler(&self, ziel: &Ziel) -> f64 {
        self.fehler(ziel)
            .iter()
            .fold(0.0f64, |m, f| m.max(f.abs()))
    }
}

/// Eine Messlinie.
///
/// Zwei Arten, dieselbe Anatomie zu treffen:
///
/// * `spur = Some(…)` — eine **mitwandernde Materiallinie** von der
///   Referenzavatarin. Sie folgt der Formung, auch wenn sich die
///   Höhenfelder verschieben.
/// * `spur = None` — ein **fester Schnitt** bei `hoehe`. Das ist richtig,
///   sobald die Zielhöhe aus der Maßdatei bekannt ist.
#[derive(Clone, Debug)]
pub struct Messlinie {
    pub ort: Messort,
    pub spur: Option<Spur>,
    /// Höhe der Linie, cm.
    pub hoehe: f64,
}

impl Messlinie {
    /// Mittlere Höhe der Linie auf einem geformten Netz, cm.
    pub fn ist_hoehe(&self, netz: &Netz) -> f64 {
        match &self.spur {
            Some(s) => s.mittlere_hoehe(netz) / 10.0,
            // Ein fester Schnitt liegt per Definition auf dieser Höhe.
            None => self.hoehe,
        }
    }
}

/// Die Messlinien eines Körpers.
#[derive(Clone, Debug)]
pub struct Messlinien {
    pub linien: Vec<Messlinie>,
    /// Messdaten für das Maßband; `None` misst die Schnittkontur.
    pub massband: Option<Arc<Messdaten>>,
}

impl Messlinien {
    /// Misst künftig wie ein Maßband: konvexe Hülle des waagerechten
    /// Rumpfschnitts (Segment Rumpf) auf der Höhe der Linie, Ebene durch
    /// `(0, y, 0)`. Eine Materiallinie wird auf ihrer mittleren Höhe am
    /// jeweiligen Netz gemessen.
    pub fn mit_massband(mut self, d: Arc<Messdaten>) -> Messlinien {
        self.massband = Some(d);
        self
    }

    pub fn len(&self) -> usize {
        self.linien.len()
    }

    pub fn ist_leer(&self) -> bool {
        self.linien.is_empty()
    }

    pub fn orte(&self) -> Vec<Messort> {
        self.linien.iter().map(|l| l.ort).collect()
    }

    pub fn namen(&self) -> Vec<&'static str> {
        self.linien.iter().map(|l| l.ort.name()).collect()
    }

    pub fn hoehen(&self) -> Vec<f64> {
        self.linien.iter().map(|l| l.hoehe).collect()
    }

    /// Feldindex in `c` je Linie, aus der Namensliste des Modells.
    ///
    /// Das ist die Zuordnung „Linie steuert Feld“ für [`anpassen`].
    pub fn aktiv(&self, namen: &[String]) -> Result<Vec<usize>, String> {
        self.linien
            .iter()
            .map(|l| {
                let feld = l.ort.feld();
                namen
                    .iter()
                    .position(|n| n == feld)
                    .ok_or_else(|| format!("Feld {feld} fehlt im Modell"))
            })
            .collect()
    }
}

/// Legt alle fünf Rumpfmesslinien als Materiallinien an einem
/// Referenzkörper an.
pub fn messlinien(grund: &Netz) -> Result<Messlinien, String> {
    messlinien_aus(grund, &Messort::ALLE)
}

/// Legt die gewünschten Messlinien als Materiallinien an.
///
/// Die Reihenfolge wird auf die natürliche Reihenfolge der [`Messort`]
/// gebracht (von oben nach unten); doppelte Orte sind ein Fehler.
pub fn messlinien_aus(grund: &Netz, orte: &[Messort]) -> Result<Messlinien, String> {
    let wahl: Vec<(Messort, Option<f64>)> = orte.iter().map(|o| (*o, None)).collect();
    messlinien_gemischt(grund, &wahl)
}

/// Legt Messlinien an: je Ort entweder ein fester Schnitt (`Some(hoehe)`)
/// oder eine mitwandernde Materiallinie von der Referenz (`None`).
pub fn messlinien_gemischt(
    grund: &Netz,
    wahl: &[(Messort, Option<f64>)],
) -> Result<Messlinien, String> {
    if wahl.is_empty() {
        return Err("keine Messlinien gewählt".to_string());
    }
    let mut gewaehlt = wahl.to_vec();
    gewaehlt.sort_by_key(|(ort, _)| *ort);
    for fenster in gewaehlt.windows(2) {
        if fenster[0].0 == fenster[1].0 {
            return Err(format!("Messort {} doppelt", fenster[0].0.name()));
        }
    }

    let mut linien = Vec::with_capacity(gewaehlt.len());
    for (ort, hoehe) in gewaehlt {
        match hoehe {
            Some(y) => {
                if !y.is_finite() || y <= 0.0 {
                    return Err(format!("{}: unsinnige Höhe {y}", ort.name()));
                }
                // Nicht am Grundkörper prüfen: die Zielperson kann ganz
                // andere Landmarkenhöhen haben, und der Grundkörper ist an
                // den Nähten offen (Befund 25). Fehlt der Schnitt später
                // wirklich, liefert `messen` NaN und `anpassen` bricht ab.
                linien.push(Messlinie {
                    ort,
                    spur: None,
                    hoehe: y,
                });
            }
            None => {
                let y = ort.hoehe();
                let q = rumpf(grund, y * 10.0)
                    .ok_or_else(|| format!("kein geschlossener Rumpf bei {y:.2} cm"))?;
                let s = spur(grund, &q)
                    .ok_or_else(|| format!("Schnitt bei {y:.2} cm nicht zuordenbar"))?;
                linien.push(Messlinie {
                    ort,
                    spur: Some(s),
                    hoehe: y,
                });
            }
        }
    }
    Ok(Messlinien {
        linien,
        massband: None,
    })
}

/// Misst alle Linien eines Netzes.
///
/// Ein fester Schnitt, der auf dem geformten Netz keine geschlossene
/// Rumpfschleife trifft, liefert `NaN`; [`anpassen`] bricht dann mit einer
/// Meldung ab, statt mit einem falschen Wert weiterzurechnen. Mit Maßband
/// gilt dasselbe für einen leeren Schnitt oder Messdaten, die nicht zum Netz
/// passen.
pub fn messen(netz: &Netz, linien: &Messlinien) -> Ist {
    if let Some(d) = &linien.massband {
        let passt = d.segment.len() == netz.punkte.len();
        // Gemessen wird im Rahmen der Spezifikation (`formung.md` 8): x und z
        // um den Mittelwert, der tiefste Punkt auf y = 0. Die Zielhöhen der
        // Maßdatei zählen vom Boden; das geformte Netz steht aber nicht auf
        // y = 0 (bei der Anwenderin 1,95 cm darüber).
        let v = zentriert_cm(netz, 10.0);
        let boden = netz.hoehe().map(|(lo, _)| lo / 10.0).unwrap_or(0.0);
        return Ist {
            masse: linien
                .linien
                .iter()
                .map(|l| {
                    let y = match &l.spur {
                        Some(s) => s.mittlere_hoehe(netz) / 10.0 - boden,
                        None => l.hoehe,
                    };
                    let u = if passt {
                        rumpf_waagerecht(&v, d, y)
                    } else {
                        0.0
                    };
                    if u > 0.0 {
                        u
                    } else {
                        f64::NAN
                    }
                })
                .collect(),
        };
    }
    Ist {
        masse: linien
            .linien
            .iter()
            .map(|l| match &l.spur {
                Some(s) => s.laenge(netz) / 10.0,
                None => rumpf(netz, l.hoehe * 10.0)
                    .map(|q| q.laenge / 10.0)
                    .unwrap_or(f64::NAN),
            })
            .collect(),
    }
}

/// Ergebnis einer Anpassung.
#[derive(Clone, Debug)]
pub struct Ergebnis {
    pub c: Vec<f64>,
    pub ist: Ist,
    pub runden: usize,
    /// Fehler nach der letzten Runde, cm.
    pub fehler: Vec<f64>,
    /// Fehler je Runde, cm.
    pub verlauf: Vec<Vec<f64>>,
    /// Wurde die Iteration wegen Divergenz zurückgenommen?
    pub divergiert: bool,
}

/// Löst ein lineares Gleichungssystem mit Gauß-Elimination und
/// Spaltenpivot. Die Dimension kommt aus der rechten Seite.
fn loese(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for s in 0..n {
        let mut p = s;
        for z in s + 1..n {
            if a[z][s].abs() > a[p][s].abs() {
                p = z;
            }
        }
        a.swap(s, p);
        b.swap(s, p);
        let d = a[s][s];
        if d.abs() < 1.0e-300 {
            continue;
        }
        for z in s + 1..n {
            let f = a[z][s] / d;
            for k in s..n {
                a[z][k] -= f * a[s][k];
            }
            b[z] -= f * b[s];
        }
    }
    let mut x = vec![0.0f64; n];
    for s in (0..n).rev() {
        let mut summe = b[s];
        for k in s + 1..n {
            summe -= a[s][k] * x[k];
        }
        x[s] = if a[s][s].abs() > 1.0e-300 {
            summe / a[s][s]
        } else {
            0.0
        };
    }
    x
}

/// Grösster erlaubter Schritt je Runde, in Einheiten von `c`.
const MAX_SCHRITT: f64 = 8.0;
/// Abbruch, wenn alle Maße so nah sind, in Zentimetern.
const TOLERANZ: f64 = 0.01;
/// Höchstzahl der Runden.
const MAX_RUNDEN: usize = 30;

/// Passt den Körper an die Zielmaße an.
///
/// `c_ref` ist der Referenzvektor aus [`crate::formung::referenz_vektor`]
/// (Länge = Felder − 1). Der Vektor darf vorher an die Zielperson angepasst
/// sein (z. B. andere `HEIGHT_*`-Felder); überschrieben wird nur `c[0]` mit
/// `ziel.hoehe`. `aktiv` nennt je Messlinie den Feldindex in `c` (siehe
/// [`Messlinien::aktiv`]); nur diese Felder werden verschoben.
pub fn anpassen(
    grund: &Netz,
    formung: &Formung,
    c_ref: &[f64],
    linien: &Messlinien,
    ziel: &Ziel,
    aktiv: &[usize],
) -> Result<Ergebnis, String> {
    let n = c_ref.len();
    if n + 1 != formung.felder() {
        return Err(format!(
            "{n} Koeffizienten für {} Felder",
            formung.felder()
        ));
    }
    let m = linien.len();
    if m != ziel.masse.len() {
        return Err(format!("{m} Messlinien, aber {} Zielmaße", ziel.masse.len()));
    }
    if m != aktiv.len() {
        return Err(format!("{m} Messlinien, aber {} aktive Felder", aktiv.len()));
    }
    if linien.ist_leer() {
        return Err("keine Messlinien".to_string());
    }
    if ziel.masse.iter().any(|w| !w.is_finite()) {
        return Err("die Zielmaße enthalten keinen endlichen Wert".to_string());
    }
    let mut gesehen = vec![false; n];
    for &k in aktiv {
        if k == 0 || k >= n {
            return Err(format!("aktives Feld {k} liegt ausserhalb von 1..{n}"));
        }
        if gesehen[k] {
            return Err(format!("aktives Feld {k} doppelt"));
        }
        gesehen[k] = true;
    }

    let mut c = c_ref.to_vec();
    // Die Höhe ist in CLOs Modell der Koeffizient selbst (Befund 22).
    c[0] = ziel.hoehe;

    let mut ist = messen(&formung.anwenden(grund, &c), linien);
    if ist.masse.iter().any(|w| !w.is_finite()) {
        return Err("die Messung liefert keine endlichen Werte".to_string());
    }

    let mut verlauf = vec![ist.fehler(ziel)];
    let mut divergiert = false;
    let mut runden = 0;
    for runde in 0..MAX_RUNDEN {
        runden = runde + 1;
        let r = ist.fehler(ziel);
        if r.iter().fold(0.0f64, |m, f| m.max(f.abs())) < TOLERANZ {
            break;
        }

        // Jacobi der aktiven Spalten an der aktuellen Form:
        // A[i][j] = ∂(Maß i)/∂(c_aktiv[j]), Schritt 1,0.
        let mut a = vec![vec![0.0f64; m]; m];
        for (j, &k) in aktiv.iter().enumerate() {
            let mut c2 = c.clone();
            c2[k] += 1.0;
            let mess = messen(&formung.anwenden(grund, &c2), linien);
            for i in 0..m {
                a[i][j] = mess.masse[i] - ist.masse[i];
            }
        }
        // Dämpfung relativ zur grössten Zeile.
        let norm = a
            .iter()
            .flat_map(|zeile| zeile.iter())
            .fold(0.0f64, |m, w| m.max(w.abs()));
        let lambda = 1.0e-9 + 1.0e-6 * norm;
        for (i, zeile) in a.iter_mut().enumerate() {
            zeile[i] += lambda;
        }
        let mut delta = loese(a, r.clone());
        let max = delta.iter().fold(0.0f64, |m, d| m.max(d.abs()));
        if max > MAX_SCHRITT {
            let f = MAX_SCHRITT / max;
            for d in &mut delta {
                *d *= f;
            }
        }

        // Rücknahme, wenn der Fehler wächst (wie CLOs Treiber).
        let mut faktor = 1.0;
        let mut neu = c.clone();
        let mut neu_ist = ist.clone();
        let mut angenommen = false;
        for _ in 0..8 {
            for (j, &k) in aktiv.iter().enumerate() {
                neu[k] = c[k] + delta[j] * faktor;
            }
            let mess = messen(&formung.anwenden(grund, &neu), linien);
            let fehler_neu = mess.groesster_fehler(ziel);
            if fehler_neu.is_finite() && fehler_neu < ist.groesster_fehler(ziel) {
                neu_ist = mess;
                angenommen = true;
                break;
            }
            faktor *= 0.5;
        }
        if !angenommen {
            divergiert = true;
            break;
        }
        c = neu;
        ist = neu_ist;
        verlauf.push(ist.fehler(ziel));
    }

    Ok(Ergebnis {
        fehler: ist.fehler(ziel),
        c,
        ist,
        runden,
        verlauf,
        divergiert,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loese_reproduziert_ein_bekanntes_system() {
        // 5×5, diagonal dominant, Lösung x = [1, 2, 3, 4, 5].
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let mut a = vec![vec![0.0f64; 5]; 5];
        for i in 0..5 {
            for j in 0..5 {
                a[i][j] = if i == j { 10.0 } else { 0.1 * (i + j) as f64 };
            }
        }
        let b: Vec<f64> = (0..5)
            .map(|i| (0..5).map(|j| a[i][j] * x[j]).sum())
            .collect();
        let geloest = loese(a, b);
        for i in 0..5 {
            assert!((geloest[i] - x[i]).abs() < 1.0e-9, "{i}: {}", geloest[i]);
        }
    }

    #[test]
    fn umfang_eines_zylinders_wird_als_zentimeter_gemessen() {
        let n = Netz::zylinder(100.0, 3000.0, 720, 20);
        let linien = messlinien(&n).expect("Linien");
        assert_eq!(linien.len(), 5);
        let ist = messen(&n, &linien);
        for (i, wert) in ist.masse.iter().enumerate() {
            assert!((wert - 62.83).abs() < 0.01, "Linie {i}: {wert}");
        }
    }

    #[test]
    fn teilmenge_der_linien_haelt_ihre_reihenfolge() {
        let n = Netz::zylinder(100.0, 3000.0, 720, 20);
        let linien = messlinien_aus(&n, &[Messort::Huefte, Messort::Brust]).expect("Linien");
        assert_eq!(linien.orte(), vec![Messort::Brust, Messort::Huefte]);
        assert_eq!(linien.hoehen(), vec![BRUSTHOEHE_REF, HUEFTHOEHE_REF]);
    }

    #[test]
    fn leer_oder_doppelt_ist_ein_fehler() {
        let n = Netz::zylinder(100.0, 3000.0, 720, 20);
        assert!(messlinien_aus(&n, &[]).is_err());
        assert!(messlinien_aus(&n, &[Messort::Taille, Messort::Taille]).is_err());
        assert!(messlinien_gemischt(&n, &[(Messort::Taille, Some(0.0))]).is_err());
    }

    #[test]
    fn fester_schnitt_misst_auf_der_hoehe() {
        let n = Netz::zylinder(100.0, 3000.0, 720, 20);
        let linien = messlinien_gemischt(&n, &[(Messort::Taille, Some(150.0))]).expect("Linien");
        let ist = messen(&n, &linien);
        assert!((ist.masse[0] - 62.83).abs() < 0.01, "{}", ist.masse[0]);
        assert!((linien.linien[0].ist_hoehe(&n) - 150.0).abs() < 1.0e-9);
    }

    #[test]
    fn fester_schnitt_ohne_rumpf_liefert_nan() {
        // Linie bei 190 cm: am hohen Zylinder gebaut, am kurzen nicht.
        let hoch = Netz::zylinder(100.0, 3000.0, 720, 20);
        let linien =
            messlinien_gemischt(&hoch, &[(Messort::Brust, Some(190.0))]).expect("Linien");
        let kurz = Netz::zylinder(100.0, 1500.0, 720, 20);
        let ist = messen(&kurz, &linien);
        assert!(ist.masse[0].is_nan());
    }

    #[test]
    fn massband_misst_den_zylinder_als_band() {
        let n = Netz::zylinder(100.0, 3000.0, 720, 20);
        let d = Arc::new(Messdaten::aus_netz(&n, crate::massband::segment::RUMPF));
        let linien = messlinien_gemischt(&n, &[(Messort::Taille, Some(150.0))])
            .expect("Linien")
            .mit_massband(d);
        let ist = messen(&n, &linien);
        // 720 Ecken, nach dem 0,5-cm-Filter jede sechste: die Sehnen sind
        // kaum kürzer als der Kreis.
        assert!((ist.masse[0] - 62.83).abs() < 0.01, "{}", ist.masse[0]);
    }

    #[test]
    fn massband_ohne_passende_messdaten_liefert_nan() {
        let n = Netz::zylinder(100.0, 3000.0, 720, 20);
        let fremd = Netz::zylinder(100.0, 3000.0, 36, 2);
        let d = Arc::new(Messdaten::aus_netz(&fremd, crate::massband::segment::RUMPF));
        let linien = messlinien_gemischt(&n, &[(Messort::Taille, Some(150.0))])
            .expect("Linien")
            .mit_massband(d);
        assert!(messen(&n, &linien).masse[0].is_nan());
    }

    #[test]
    fn aktiv_findet_die_fuenf_felder() {
        let namen: Vec<String> = [
            "HEIGHT_Total",
            "CIRCUMFERENCE_Bust",
            "CIRCUMFERENCE_UnderBust",
            "CIRCUMFERENCE_Waist",
            "CIRCUMFERENCE_HighHip",
            "CIRCUMFERENCE_LowHip",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let n = Netz::zylinder(100.0, 3000.0, 720, 20);
        let linien = messlinien(&n).expect("Linien");
        let aktiv = linien.aktiv(&namen).expect("Zuordnung");
        assert_eq!(aktiv, vec![1, 2, 3, 4, 5]);
    }
}
