//! [`crate::koerper::Anordnung`] für den Körper aus CLOs `.avt`.
//!
//! [`crate::koerper::anordnung`] liest Segmente und Landmarken aus dem
//! `Resizing`-Ordner und setzt voraus, dass das Netz **genau die Punktliste**
//! von `BaseMesh.obj` ist: `range_list.rlist` und `IndexSet.txt` sind
//! Punktnummern in dieser Liste. Der Körper aus der `.avt` (Befund 119) ist
//! derselbe BaseMesh in anderer Lage und teilweise neu vernetzt — die
//! Punktnummern gelten deshalb weiter, der Ordner ist aber nicht dabei.
//!
//! ## Warum die Indizes gelten (geprüft, nicht angenommen)
//!
//! Der `body` aus der `.avt` ist CLOs `Female`-BaseMesh, verformt vom
//! Resizing des Projekts. Belegt an der Prüfdatei
//! (`japanese_school_uniform`, 31 422 Knoten):
//!
//! | Prüfung | Ergebnis |
//! |---|---|
//! | Kantenmengen `.avt` gegen `BaseMesh.obj` | 92 728 von 94 081 gemeinsam (**98,56 %**) |
//! | Dreiecke, ungeordnet | 59 952 von 62 658 gemeinsam (95,7 %) |
//! | Punkte mit **identischer Nachbarschaft** | 28 965 von 31 422 (**92,2 %**) |
//!
//! Die abweichenden Stellen liegen dort, wo CLO beim Resizing neu vernetzt
//! (Brust 700–900 mm, Taille/Hüfte 1 000–1 400 mm). Es ist **kein** anderer
//! Körper: die Verschiebung `BaseMesh → .avt` ist zwischen zwei Netznachbarn
//! im Median **0,70 mm** groß; bei zufälliger Indexzuordnung wären es
//! 629,9 mm — die Zuordnung ist **900-fach glatter als der Zufall**.
//!
//! ## Die Prüfung: topologisch, nicht geometrisch
//!
//! Der erste Anlauf prüfte die Landmarken **geometrisch** — spiegelgleich um
//! eine Mittellinie, Drosselgrube über Schulter über Ellbogen. Am Prüfkörper
//! ging das mit 0,01 mm auf. An **40 fremden** Projekten fiel es durch: vier
//! davon (`catsuit_v5_base`, `miko`, `miko_alt`, `zero_two_base`) haben eine
//! **asymmetrische Pose** (Arme oben), dort steht die Schulter 233 mm über der
//! Drosselgrube und die Paare sind bis 78 mm unsymmetrisch. Die Geometrie
//! hängt also an der **Pose**, nicht an der Zuordnung.
//!
//! Richtig ist CLOs eigene `symmetry_data.sym`: Zahl *i* ist der
//! **Spiegelpartner** von Punkt *i* — eine Aussage über **Nummern**, nicht
//! über Koordinaten, also pose-unabhängig. Damit wird die Prüfung exakt
//! (Ganzzahl-Vergleich, keine Toleranz):
//!
//! * L6 ↔ L7, L8 ↔ L9, L49 ↔ L52 sind **gegenseitig** Spiegelpartner,
//! * L4 und L5 (Halsmittellinie) sind **ihr eigener** Partner.
//!
//! Das gilt bei **40 von 40** fremden Projekten. Es weist auch den CLO-Fehler
//! der 2020er Fassung ab (siehe unten), und zwar ohne jede Koordinate.
//!
//! Dazu kommt CLOs eigene `range_list.rlist`: jede Landmarke muss in dem
//! Segment liegen, das ihr Name nennt (Hals → 6, Schulter/Ellbogen → Arm
//! 0/1). Auch das ist eine Nummernaussage und bei **40 von 40** erfüllt.
//!
//! ## Ein Fehler in CLOs 2020er Fassung, den die Prüfung findet
//!
//! Die **2020er** `IndexSet.txt` (`CLO Standalone`) nennt für L52 den Punkt
//! **27 644** statt 27 631. `symmetry_data.sym` sagt eindeutig
//! `s(12 010) = 27 631` — 27 644 ist **nicht** der Spiegelpartner von L49.
//! Nur die 2026er Fassung (`CLO Standalone OnlineAuth`) ist dort richtig. Die
//! Prüfung weist die 2020er ab, statt mit einem falschen Ellbogen
//! weiterzurechnen; wer mit 2020 arbeitet, setzt `CLO_FEMALE` auf 2026.

use crate::koerper::Anordnung;
use crate::massband::segment;
use crate::Netz;

/// Was bei der Prüfung der Landmarken auffiel.
///
/// Alle Varianten sind **Nummernaussagen** — keine Koordinate, keine Toleranz.
#[derive(Clone, Debug, PartialEq)]
pub enum Abweichung {
    /// Zwei Punkte, die laut `IndexSet.txt` ein Paar sind, sind es laut
    /// `symmetry_data.sym` nicht (oder nicht gegenseitig).
    KeinSpiegelpaar {
        paar: &'static str,
        links: usize,
        rechts: usize,
        /// Der Partner, den `symmetry_data.sym` für `links` nennt.
        partner_laut_sym: i64,
    },
    /// Ein Punkt, der auf der Mittellinie liegen muss, ist laut
    /// `symmetry_data.sym` nicht sein eigener Spiegelpartner.
    NichtAufMittellinie {
        landmarke: &'static str,
        punkt: usize,
        partner_laut_sym: i64,
    },
    /// Eine Landmarke liegt nicht im Segment, das ihr Name nennt.
    FalschesSegment {
        landmarke: &'static str,
        punkt: usize,
        gefunden: i32,
        erwartet: i32,
    },
    /// Der Körper hat nicht 31 422 Punkte — dann sind es nicht CLOs
    /// BaseMesh-Nummern, und `range_list.rlist`/`IndexSet.txt` passen nicht.
    Punkte { gefunden: usize, erwartet: usize },
}

impl std::fmt::Display for Abweichung {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Abweichung::KeinSpiegelpaar { paar, links, rechts, partner_laut_sym } => write!(
                f,
                "Landmarkenpaar {paar} ist laut symmetry_data.sym kein Spiegelpaar: \
                 {links} hat den Partner {partner_laut_sym}, in IndexSet.txt steht {rechts}"
            ),
            Abweichung::NichtAufMittellinie { landmarke, punkt, partner_laut_sym } => write!(
                f,
                "{landmarke} ({punkt}) muss auf der Mittellinie liegen, hat aber laut \
                 symmetry_data.sym den Spiegelpartner {partner_laut_sym}"
            ),
            Abweichung::FalschesSegment { landmarke, punkt, gefunden, erwartet } => write!(
                f,
                "{landmarke} ({punkt}) liegt in Segment {gefunden} statt in {erwartet} \
                 — range_list.rlist widerspricht dem Namen"
            ),
            Abweichung::Punkte { gefunden, erwartet } => write!(
                f,
                "Körper hat {gefunden} Punkte statt {erwartet} — die Nummern aus \
                 range_list.rlist/IndexSet.txt gelten nur für CLOs BaseMesh"
            ),
        }
    }
}

/// CLOs BaseMesh hat so viele Punkte in der Gruppe `body` (Befund 74, 119).
pub const BASE_MESH_PUNKTE: usize = 31_422;

/// So viele Einträge hat `symmetry_data.sym` (nur die Punkte 0…40 464 —
/// Augen, Wimpern und Zähne eingeschlossen; die 7 185 Haarpunkte fehlen).
pub const SYMMETRIE_PUNKTE: usize = 40_465;

/// `IndexSet.txt`-Zeilen, die [`Anordnung`] braucht.
///
/// | Zeile | Punkt | Lage |
/// |---:|---|---|
/// | 4 | Drosselgrube | vordere Halsmitte (Mittellinie) |
/// | 5 | 7. Halswirbel | hintere Halsmitte (Mittellinie) |
/// | 6, 7 | HPS links, rechts | seitlicher Hals |
/// | 8, 9 | Schulterpunkt links, rechts | Schulter |
/// | 49, 52 | Ellbogen hinten links, rechts | Ellbogen |
pub const L_HALS_VORN: usize = 4;
pub const L_HALS_HINTEN: usize = 5;
pub const L_HALS_SEITLICH: [usize; 2] = [6, 7];
pub const L_SCHULTER: [usize; 2] = [8, 9];
pub const L_ELLBOGEN: [usize; 2] = [49, 52];

/// Index in `IndexSet.txt` je Zeile, wie sie [`Anordnung`] braucht.
pub const BENUTZT: [usize; 8] = [
    L_HALS_VORN,
    L_HALS_HINTEN,
    L_HALS_SEITLICH[0],
    L_HALS_SEITLICH[1],
    L_SCHULTER[0],
    L_SCHULTER[1],
    L_ELLBOGEN[0],
    L_ELLBOGEN[1],
];

/// Segment- und Landmarkenlisten und die Spiegelzuordnung, wie CLO sie für
/// den BaseMesh führt.
///
/// Alle drei sind Punktnummern in CLOs `BaseMesh`-Liste. Sie werden aus dem
/// `Resizing`-Ordner gelesen (`range_list.rlist`, `IndexSet.txt`,
/// `symmetry_data.sym`) und auf den verformten Körper aus der `.avt`
/// angewendet.
#[derive(Clone, Debug)]
pub struct CloNummern {
    /// Körpersegment je Punkt (`range_list.rlist`).
    pub segment: Vec<i32>,
    /// Punktnummer je `IndexSet.txt`-Zeile; negativ = berechnet/unbenutzt.
    pub landmarken: Vec<i64>,
    /// Spiegelpartner je Punkt (`symmetry_data.sym`).
    pub spiegel: Vec<i64>,
}

impl CloNummern {
    /// Liest alle drei Listen aus einem CLO-`Resizing`-Ordner.
    ///
    /// `symmetry_data.sym` fehlt in sehr alten Installationen; dann bleibt
    /// [`CloNummern::spiegel`] leer und die Spiegelprüfung entfällt (die
    /// Segmentprüfung bleibt).
    pub fn aus_ordner(ordner: &std::path::Path) -> Result<CloNummern, String> {
        let zahlen = |datei: &str| -> Result<Vec<i64>, String> {
            let t = std::fs::read_to_string(ordner.join(datei))
                .map_err(|e| format!("{datei}: {e}"))?;
            t.split_whitespace()
                .map(|w| w.parse::<i64>().map_err(|e| format!("{datei}: {e}")))
                .collect()
        };
        let spiegel = match std::fs::read_to_string(ordner.join("symmetry_data.sym")) {
            Ok(t) => t
                .split_whitespace()
                .filter_map(|w| w.parse::<i64>().ok())
                .collect(),
            Err(_) => Vec::new(),
        };
        Ok(CloNummern {
            segment: zahlen("range_list.rlist")?.into_iter().map(|v| v as i32).collect(),
            landmarken: zahlen("IndexSet.txt")?,
            spiegel,
        })
    }

    /// Die Punktnummer der Landmarke `zeile`, wenn sie ein Punkt ist.
    pub fn landmarke(&self, zeile: usize) -> Option<usize> {
        match self.landmarken.get(zeile) {
            Some(&v) if v >= 0 => Some(v as usize),
            _ => None,
        }
    }

    /// Der Spiegelpartner von `punkt` laut `symmetry_data.sym`.
    pub fn spiegelpartner(&self, punkt: usize) -> Option<i64> {
        self.spiegel.get(punkt).copied()
    }
}

/// Baut die [`Anordnung`] für einen Körper aus der `.avt`.
///
/// `netz` ist der `body` (mm, Avatarraum); `nummern` kommt aus CLOs
/// `Resizing`-Ordner. Geprüft wird, dass die Landmarken laut CLOs eigenen
/// Listen zusammengehören — siehe [`Abweichung`].
pub fn aus_clo_koerper(netz: &Netz, nummern: &CloNummern) -> Result<Anordnung, String> {
    let n = netz.punkte.len();
    if n != BASE_MESH_PUNKTE {
        return Err(Abweichung::Punkte { gefunden: n, erwartet: BASE_MESH_PUNKTE }.to_string());
    }
    if nummern.segment.len() < n {
        return Err(format!(
            "{} Segmente für {n} Punkte — die Liste gehört zu einem anderen Körper",
            nummern.segment.len()
        ));
    }
    let punkte: Vec<usize> = (0..n).collect();
    let teil = |s: i32| -> Vec<usize> {
        punkte.iter().copied().filter(|&i| nummern.segment[i] == s).collect()
    };
    let nummer = |zeile: usize| -> Result<usize, String> {
        let i = nummern
            .landmarke(zeile)
            .ok_or_else(|| format!("Landmarke L{zeile} ist keine Punktnummer"))?;
        if i >= n {
            return Err(format!("Landmarke L{zeile} nennt Punkt {i}, der Körper hat {n}"));
        }
        Ok(i)
    };

    let i_hals_vorn = nummer(L_HALS_VORN)?;
    let i_hals_hinten = nummer(L_HALS_HINTEN)?;
    let i_hals_seitlich = [nummer(L_HALS_SEITLICH[0])?, nummer(L_HALS_SEITLICH[1])?];
    let i_schulter = [nummer(L_SCHULTER[0])?, nummer(L_SCHULTER[1])?];
    let i_ellbogen = [nummer(L_ELLBOGEN[0])?, nummer(L_ELLBOGEN[1])?];

    pruefe_nummern(nummern, &[
        ("Hals seitlich", i_hals_seitlich, Some(segment::KOPF)),
        ("Schulter", i_schulter, Some(segment::ARM_LINKS)),
        ("Ellbogen", i_ellbogen, Some(segment::ARM_LINKS)),
    ])?;
    // Mittellinie: die beiden Halspunkte sind laut CLO ihr eigener Partner.
    pruefe_mittellinie(nummern, "Drosselgrube", i_hals_vorn)?;
    pruefe_mittellinie(nummern, "7. Halswirbel", i_hals_hinten)?;

    Ok(Anordnung {
        rumpf: teil(segment::RUMPF),
        arme: [teil(segment::ARM_LINKS), teil(segment::ARM_RECHTS)],
        hals_vorn: netz.punkte[i_hals_vorn],
        hals_hinten: netz.punkte[i_hals_hinten],
        hals_seitlich: i_hals_seitlich.map(|i| netz.punkte[i]),
        schulter: i_schulter.map(|i| netz.punkte[i]),
        ellbogen: i_ellbogen.map(|i| netz.punkte[i]),
    })
}

/// Prüft je Paar, dass CLO selbst die beiden Punkte als Spiegelpartner führt.
///
/// `segment_soll` ist das Segment, in dem die Landmarke laut ihrem Namen
/// liegen muss; bei `None` wird nur die Spiegelung geprüft.
fn pruefe_nummern(
    nummern: &CloNummern,
    paare: &[(&'static str, [usize; 2], Option<i32>)],
) -> Result<(), String> {
    let spiegel_da = !nummern.spiegel.is_empty();
    for (name, paar, seg_soll) in paare {
        let [links, rechts] = *paar;
        if spiegel_da {
            let partner = nummern
                .spiegelpartner(links)
                .ok_or_else(|| format!("symmetry_data.sym endet vor Punkt {links}"))?;
            if partner != rechts as i64 {
                return Err(Abweichung::KeinSpiegelpaar {
                    paar: name,
                    links,
                    rechts,
                    partner_laut_sym: partner,
                }
                .to_string());
            }
            // Gegenrichtung: CLOs Zuordnung ist nicht überall gegenseitig.
            if let Some(zurueck) = nummern.spiegelpartner(rechts) {
                if zurueck != links as i64 {
                    return Err(Abweichung::KeinSpiegelpaar {
                        paar: name,
                        links,
                        rechts,
                        partner_laut_sym: partner,
                    }
                    .to_string());
                }
            }
        }
        // Segment laut CLOs range_list.rlist. Beide Seiten eines Paares
        // liegen in verschiedenen Segmenten (Arm links = 1, rechts = 0);
        // geprüft wird deshalb nur, dass die Landmarke in einem der beiden
        // Armsegmente liegt, wenn ein Arm erwartet ist.
        if let Some(erwartet) = *seg_soll {
            for &i in paar {
                let ist = nummern.segment[i];
                let passt = if erwartet == segment::ARM_LINKS {
                    ist == segment::ARM_LINKS || ist == segment::ARM_RECHTS
                } else {
                    ist == erwartet
                };
                if !passt {
                    return Err(Abweichung::FalschesSegment {
                        landmarke: name,
                        punkt: i,
                        gefunden: ist,
                        erwartet,
                    }
                    .to_string());
                }
            }
        }
    }
    Ok(())
}

/// Ein Punkt der Mittellinie muss in `symmetry_data.sym` sein eigener
/// Spiegelpartner sein.
fn pruefe_mittellinie(
    nummern: &CloNummern,
    name: &'static str,
    punkt: usize,
) -> Result<(), String> {
    if nummern.spiegel.is_empty() {
        return Ok(());
    }
    let partner = nummern
        .spiegelpartner(punkt)
        .ok_or_else(|| format!("symmetry_data.sym endet vor Punkt {punkt}"))?;
    if partner != punkt as i64 {
        return Err(Abweichung::NichtAufMittellinie {
            landmarke: name,
            punkt,
            partner_laut_sym: partner,
        }
        .to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein `CloNummern` mit gerader Spiegelzuordnung: Paare (a, b).
    fn nummern_mit(paare: &[(usize, usize)], mitte: &[usize]) -> CloNummern {
        // -1 = „kein Partner bekannt"; nur die genannten Punkte bekommen einen.
        let mut spiegel: Vec<i64> = vec![-1; 200];
        for &(a, b) in paare {
            spiegel[a] = b as i64;
            spiegel[b] = a as i64;
        }
        for &m in mitte {
            spiegel[m] = m as i64;
        }
        let mut segment = vec![6i32; 200];
        for i in 100..120 {
            segment[i] = segment::ARM_LINKS;
        }
        CloNummern {
            segment,
            landmarken: vec![0; 60],
            spiegel,
        }
    }

    #[test]
    fn spiegelpaare_werden_geprueft() {
        let gut = nummern_mit(&[(10, 11), (12, 13), (14, 15)], &[8, 9]);
        let r = pruefe_nummern(
            &gut,
            &[
                ("Hals seitlich", [10, 11], Some(segment::KOPF)),
                ("Schulter", [12, 13], Some(segment::KOPF)),
                ("Ellbogen", [14, 15], Some(segment::KOPF)),
            ],
        );
        assert!(r.is_ok(), "echte Paare müssen durchgehen: {r:?}");

        // 15 ist nicht der Partner von 14, sondern von 16.
        let falsch = nummern_mit(&[(10, 11), (12, 13), (14, 16)], &[8, 9]);
        let r = pruefe_nummern(&falsch, &[("Ellbogen", [14, 15], None)]);
        assert!(r.is_err(), "falsches Paar muss auffallen");
        let m = r.unwrap_err();
        assert!(m.contains("Ellbogen"), "Meldung nennt das Paar: {m}");
        assert!(m.contains("symmetry_data.sym"), "Meldung nennt die Quelle: {m}");
    }

    #[test]
    fn mittellinie_wird_geprueft() {
        let gut = nummern_mit(&[], &[8]);
        assert!(pruefe_mittellinie(&gut, "Drosselgrube", 8).is_ok());
        // Punkt 9 ist sein eigener Partner? Nein, er ist es nicht.
        let r = pruefe_mittellinie(&gut, "Drosselgrube", 9);
        assert!(r.is_err(), "Punkt neben der Mittellinie muss auffallen");
        assert!(r.unwrap_err().contains("Mittellinie"));
    }

    #[test]
    fn falsches_segment_wird_geprueft() {
        let n = nummern_mit(&[(10, 11)], &[]);
        // 10/11 liegen in Segment 6, erwartet wird ein Arm.
        let r = pruefe_nummern(&n, &[("Schulter", [10, 11], Some(segment::ARM_LINKS))]);
        assert!(r.is_err(), "Segment 6 als Schulter muss auffallen");
        assert!(r.unwrap_err().contains("range_list.rlist"));
        // 100/101 liegen in Segment 1 — das ist ein Arm.
        let n2 = nummern_mit(&[(100, 101)], &[]);
        assert!(pruefe_nummern(&n2, &[("Schulter", [100, 101], Some(segment::ARM_LINKS))]).is_ok());
    }

    /// Ohne `symmetry_data.sym` bleibt die Segmentprüfung, die Spiegelprüfung
    /// entfällt — eine alte Installation soll nicht alles blockieren.
    #[test]
    fn ohne_symmetry_datei_bleibt_die_segmentpruefung() {
        let mut n = nummern_mit(&[], &[]);
        n.spiegel.clear();
        assert!(pruefe_nummern(&n, &[("Schulter", [10, 11], Some(segment::KOPF))]).is_ok());
        assert!(pruefe_nummern(&n, &[("Schulter", [10, 11], Some(segment::ARM_LINKS))]).is_err());
        assert!(pruefe_mittellinie(&n, "Drosselgrube", 42).is_ok());
    }

    #[test]
    fn falsche_punktzahl_wird_abgewiesen() {
        let netz = Netz { punkte: vec![[0.0, 0.0, 0.0]; 10], dreiecke: vec![] };
        let nummern = CloNummern {
            segment: vec![4; 10],
            landmarken: vec![0; 60],
            spiegel: vec![0; 10],
        };
        let r = aus_clo_koerper(&netz, &nummern);
        assert!(r.is_err(), "10 Punkte sind kein BaseMesh");
        assert!(r.unwrap_err().contains("31422"), "Meldung nennt die erwartete Zahl");
    }
}
