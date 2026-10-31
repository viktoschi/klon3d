//! ZIP-Archive lesen, ohne Fremdcrate.
//!
//! CLO 2026.1 legt jedes Projekt und jeden inneren Behälter als ZIP an
//! (Befund 46: `.zmdr` und `.zblc` sind "ein ZIP-Archiv in einem 16-Byte-Mantel";
//! Befund 50 belegt die Regel für sechs Endungen). Für einen Leser genügt
//! deshalb ein kleiner Auspacker statt einer vollen ZIP-Bibliothek:
//! `zip` zog im Offline-Bau `zopfli` nach, `flate2` genügt und liegt vor.
//!
//! ## Was hier gelesen wird
//!
//! Nur das **zentrale Verzeichnis** (`PK\x01\x02`) am Dateiende. Es nennt je
//! Eintrag Name, Verfahren, komprimierte und rohe Größe und den Versatz des
//! lokalen Kopfs. Der lokale Kopf wird nur gelesen, um die Länge des
//! Namens- und des Zusatzfelds zu überspringen - die Größen stammen aus dem
//! zentralen Verzeichnis, weil im lokalen Kopf bei Streaming-Erzeugern Nullen
//! stehen können (so schreibt es `zip` mit gesetztem Bit 3).
//!
//! Unterstützt werden die beiden Verfahren, die CLO verwendet:
//! **0 = gespeichert** und **8 = deflate**. Alles andere wird gemeldet, nicht
//! geraten.
//!
//! ## Grenzen
//!
//! * Kein Passwort, keine Verschlüsselung (in CLO-Projekten nicht beobachtet).
//! * Kein ZIP64 (Dateien über 4 GB; die geprüften Projekte liegen bei 20 MB).
//! * Keine Prüfsummenkontrolle: CRC32 wird nicht nachgerechnet.

use std::io::Read;

/// Fehler beim Auspacken. Getrennt vom Property-Leser, weil ein kaputtes ZIP
/// eine andere Ursache hat als ein unbekannter Typcode.
#[derive(Debug, thiserror::Error)]
pub enum ZipFehler {
    #[error("kein ZIP: End-of-Central-Directory nicht gefunden (letzte 66 KiB geprüft)")]
    KeinZip,

    #[error("Eintrag {0:?} fehlt im Archiv")]
    Fehlt(String),

    #[error("Eintrag {name:?}: Verfahren {verfahren} wird nicht unterstützt")]
    Verfahren { name: String, verfahren: u16 },

    #[error("Eintrag {name:?}: entpackt {ist} Byte, angekündigt {soll}")]
    Groesse {
        name: String,
        ist: usize,
        soll: usize,
    },

    #[error("Eintrag {name:?}: komprimierte Daten beschädigt: {quelle}")]
    Deflate {
        name: String,
        #[source]
        quelle: std::io::Error,
    },

    #[error("Archiv beschädigt: {0}")]
    Struktur(String),
}

/// Ein Eintrag des Archivs, noch komprimiert.
#[derive(Debug, Clone)]
pub struct Eintrag {
    pub name: String,
    pub verfahren: u16,
    pub komprimiert: usize,
    pub roh: usize,
    versatz: usize,
}

impl Eintrag {
    /// Pfad im Archiv. CLO schreibt flache Namen ohne Verzeichnisse.
    pub fn ist_verzeichnis(&self) -> bool {
        self.name.ends_with('/')
    }
}

/// Ein gelesenes ZIP-Archiv. Hält die Bytes, damit Einträge bei Bedarf
/// entpackt werden (ein Projekt hat 16 Einträge, eines davon 13 MB).
pub struct Archiv<'a> {
    daten: &'a [u8],
    eintraege: Vec<Eintrag>,
    /// Versatz des zentralen Verzeichnisses, für genauere Fehlermeldungen.
    zd_versatz: usize,
    /// Versatz, um den die Datei vor dem eigentlichen ZIP beginnt.
    ///
    /// `.zprj` beginnt mit einem CLO-Kopf (`" ZPRJ       CLO "`, 436 425
    /// Byte), und die Offsets im ZIP zählen ab dem ZIP-Anfang. Ohne diesen
    /// Versatz zeigen alle Einträge ins Leere.
    vorspann: usize,
}

impl<'a> Archiv<'a> {
    /// Liest das zentrale Verzeichnis. Prüft nicht, ob die Einträge
    /// entpackbar sind - das passiert erst beim Holen.
    pub fn neu(daten: &'a [u8]) -> Result<Self, ZipFehler> {
        let zd = finde_zentrales_verzeichnis(daten)?;
        // Die Offsets im Ende-Datensatz zählen ab dem Anfang des ZIP, nicht
        // ab dem Anfang der Datei. `.zprj` hat einen Vorspann von 436 425 Byte
        // (`" ZPRJ       CLO "`), deshalb wird der Versatz hier bestimmt:
        // `Vorspann = EOCD-Position − Länge des Verzeichnisses − dessen
        // gemeldeter Offset`. Für ein reines ZIP ist das 0.
        let vorspann = zd.eocd
            .saturating_sub(zd.laenge)
            .saturating_sub(zd.versatz);
        let zd_versatz = zd.versatz + vorspann;
        let ende = zd_versatz + zd.laenge;
        if zd.eintraege > 65_535 {
            return Err(ZipFehler::Struktur(format!(
                "{} Einträge - ZIP64 wird nicht unterstützt",
                zd.eintraege
            )));
        }

        let mut eintraege = Vec::with_capacity(zd.eintraege);
        let mut p = zd_versatz;
        for i in 0..zd.eintraege {
            let kopf = daten
                .get(p..p + 46)
                .ok_or_else(|| ZipFehler::Struktur(format!("Eintrag {i} abgeschnitten")))?;
            if &kopf[0..4] != b"PK\x01\x02" {
                return Err(ZipFehler::Struktur(format!(
                    "Eintrag {i} bei 0x{p:X}: Signatur {:02X?} statt PK\\x01\\x02",
                    &kopf[0..4]
                )));
            }
            let verfahren = u16::from_le_bytes([kopf[10], kopf[11]]);
            let komprimiert = u32::from_le_bytes([kopf[20], kopf[21], kopf[22], kopf[23]]) as usize;
            let roh = u32::from_le_bytes([kopf[24], kopf[25], kopf[26], kopf[27]]) as usize;
            let namenlaenge = u16::from_le_bytes([kopf[28], kopf[29]]) as usize;
            let zusatzlaenge = u16::from_le_bytes([kopf[30], kopf[31]]) as usize;
            let kommentarlaenge = u16::from_le_bytes([kopf[32], kopf[33]]) as usize;
            let versatz = u32::from_le_bytes([kopf[42], kopf[43], kopf[44], kopf[45]]) as usize;

            let name_bytes = daten
                .get(p + 46..p + 46 + namenlaenge)
                .ok_or_else(|| ZipFehler::Struktur(format!("Eintrag {i}: Name abgeschnitten")))?;
            // CLO schreibt ASCII-Namen; alles andere wird ersetzt statt zu scheitern.
            let name = String::from_utf8_lossy(name_bytes).into_owned();

            eintraege.push(Eintrag {
                name,
                verfahren,
                komprimiert,
                roh,
                versatz: versatz + vorspann,
            });
            p += 46 + namenlaenge + zusatzlaenge + kommentarlaenge;
            if p > ende {
                return Err(ZipFehler::Struktur(format!(
                    "zentrales Verzeichnis überläuft bei Eintrag {i}"
                )));
            }
        }

        Ok(Self {
            daten,
            eintraege,
            zd_versatz,
            vorspann,
        })
    }

    /// Versatz des ZIP-Anfangs in der Datei (`.zprj` hat einen CLO-Kopf).
    pub fn vorspann(&self) -> usize {
        self.vorspann
    }

    pub fn eintraege(&self) -> &[Eintrag] {
        &self.eintraege
    }

    pub fn namen(&self) -> impl Iterator<Item = &str> {
        self.eintraege.iter().map(|e| e.name.as_str())
    }

    pub fn hat(&self, name: &str) -> bool {
        self.eintraege.iter().any(|e| e.name == name)
    }

    /// Eintrag mit genau diesem Namen; `None`, wenn er fehlt.
    pub fn eintrag(&self, name: &str) -> Option<&Eintrag> {
        self.eintraege.iter().find(|e| e.name == name)
    }

    /// Erster Eintrag, dessen Name auf `endung` endet (z. B. `".zpac"`).
    pub fn erster_mit_endung(&self, endung: &str) -> Option<&Eintrag> {
        self.eintraege
            .iter()
            .find(|e| e.name.ends_with(endung) && !e.ist_verzeichnis())
    }

    /// Entpackt einen Eintrag vollständig in den Speicher.
    pub fn lies(&self, name: &str) -> Result<Vec<u8>, ZipFehler> {
        let e = self
            .eintrag(name)
            .ok_or_else(|| ZipFehler::Fehlt(name.to_string()))?;
        self.entpacke(e)
    }

    /// Entpackt den ersten Eintrag mit dieser Endung.
    pub fn lies_mit_endung(&self, endung: &str) -> Result<Vec<u8>, ZipFehler> {
        let e = self
            .erster_mit_endung(endung)
            .ok_or_else(|| ZipFehler::Fehlt(format!("*{endung}")))?;
        self.entpacke(e)
    }

    fn entpacke(&self, e: &Eintrag) -> Result<Vec<u8>, ZipFehler> {
        let kopf_start = e.versatz;
        let kopf = self
            .daten
            .get(kopf_start..kopf_start + 30)
            .ok_or_else(|| ZipFehler::Struktur(format!("{}: lokaler Kopf fehlt", e.name)))?;
        if &kopf[0..4] != b"PK\x03\x04" {
            return Err(ZipFehler::Struktur(format!(
                "{}: lokale Signatur {:02X?} statt PK\\x03\\x04",
                e.name,
                &kopf[0..4]
            )));
        }
        // Der lokale Kopf wiederholt Name und Zusatzfeld. Deren Längen können
        // von denen im zentralen Verzeichnis abweichen (anderes Zusatzfeld),
        // deshalb werden sie hier neu gelesen.
        let n = u16::from_le_bytes([kopf[26], kopf[27]]) as usize;
        let z = u16::from_le_bytes([kopf[28], kopf[29]]) as usize;
        let daten_start = kopf_start + 30 + n + z;

        let roh_bytes = self
            .daten
            .get(daten_start..daten_start + e.komprimiert)
            .ok_or_else(|| ZipFehler::Struktur(format!("{}: Daten abgeschnitten", e.name)))?;

        let aus = match e.verfahren {
            0 => roh_bytes.to_vec(),
            8 => {
                let mut z = flate2::read::DeflateDecoder::new(roh_bytes);
                let mut aus = Vec::with_capacity(e.roh);
                z.read_to_end(&mut aus)
                    .map_err(|quelle| ZipFehler::Deflate {
                        name: e.name.clone(),
                        quelle,
                    })?;
                aus
            }
            verfahren => {
                return Err(ZipFehler::Verfahren {
                    name: e.name.clone(),
                    verfahren,
                })
            }
        };

        if aus.len() != e.roh {
            return Err(ZipFehler::Groesse {
                name: e.name.clone(),
                ist: aus.len(),
                soll: e.roh,
            });
        }
        Ok(aus)
    }

    /// Diagnose für Fehlermeldungen: Versatz und Größe des Verzeichnisses.
    pub fn zentrales_verzeichnis(&self) -> (usize, usize) {
        (self.zd_versatz, self.eintraege.len())
    }
}

struct Ende {
    versatz: usize,
    laenge: usize,
    eintraege: usize,
    /// Position des Ende-Datensatzes in der Datei.
    eocd: usize,
}

/// Sucht das End-of-Central-Directory-Sigel `PK\x05\x06`.
///
/// Es steht am Dateiende, kann aber von einem Kommentar bis 65 535 Byte
/// gefolgt sein. Rückwärts gesucht wird deshalb nur im zulässigen Bereich.
fn finde_zentrales_verzeichnis(daten: &[u8]) -> Result<Ende, ZipFehler> {
    if daten.len() < 22 {
        return Err(ZipFehler::KeinZip);
    }
    let unten = daten.len().saturating_sub(22 + 65_535);
    let mut p = daten.len() - 22;
    loop {
        if &daten[p..p + 4] == b"PK\x05\x06" {
            let eintraege = u16::from_le_bytes([daten[p + 10], daten[p + 11]]) as usize;
            let laenge = u32::from_le_bytes([
                daten[p + 12],
                daten[p + 13],
                daten[p + 14],
                daten[p + 15],
            ]) as usize;
            let versatz = u32::from_le_bytes([
                daten[p + 16],
                daten[p + 17],
                daten[p + 18],
                daten[p + 19],
            ]) as usize;
            // Nur eine grobe Plausibilität: der gemeldete Versatz darf nicht
            // hinter dem Ende-Datensatz liegen. `.zprj` hat einen Vorspann,
            // deshalb wird hier *nicht* gegen `daten.len()` geprüft.
            if versatz > p || laenge > p {
                return Err(ZipFehler::Struktur(format!(
                    "zentrales Verzeichnis 0x{versatz:X}+{laenge} liegt hinter dem Ende-Datensatz 0x{p:X}"
                )));
            }
            return Ok(Ende {
                versatz,
                laenge,
                eintraege,
                eocd: p,
            });
        }
        if p == unten {
            return Err(ZipFehler::KeinZip);
        }
        p -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baut ein minimales ZIP mit einem gespeicherten Eintrag.
    fn zip_gespeichert(name: &str, inhalt: &[u8]) -> Vec<u8> {
        let mut roh = Vec::new();
        // lokaler Kopf (30 Byte, dann Name, dann Daten)
        roh.extend_from_slice(b"PK\x03\x04");
        roh.extend_from_slice(&20u16.to_le_bytes()); // Version
        roh.extend_from_slice(&0u16.to_le_bytes()); // Flags
        roh.extend_from_slice(&0u16.to_le_bytes()); // Verfahren 0
        roh.extend_from_slice(&[0; 2]); // Zeit
        roh.extend_from_slice(&[0; 2]); // Datum
        roh.extend_from_slice(&0u32.to_le_bytes()); // CRC (nicht geprüft)
        roh.extend_from_slice(&(inhalt.len() as u32).to_le_bytes()); // komprimiert
        roh.extend_from_slice(&(inhalt.len() as u32).to_le_bytes()); // roh
        roh.extend_from_slice(&(name.len() as u16).to_le_bytes());
        roh.extend_from_slice(&0u16.to_le_bytes());
        roh.extend_from_slice(name.as_bytes());
        roh.extend_from_slice(inhalt);

        let zd_versatz = roh.len();
        // zentrales Verzeichnis (46 Byte, dann Name)
        roh.extend_from_slice(b"PK\x01\x02");
        roh.extend_from_slice(&20u16.to_le_bytes()); // Version erstellt
        roh.extend_from_slice(&20u16.to_le_bytes()); // Version nötig
        roh.extend_from_slice(&0u16.to_le_bytes()); // Flags
        roh.extend_from_slice(&0u16.to_le_bytes()); // Verfahren
        roh.extend_from_slice(&[0; 2]); // Zeit
        roh.extend_from_slice(&[0; 2]); // Datum
        roh.extend_from_slice(&0u32.to_le_bytes()); // CRC
        roh.extend_from_slice(&(inhalt.len() as u32).to_le_bytes()); // komprimiert
        roh.extend_from_slice(&(inhalt.len() as u32).to_le_bytes()); // roh
        roh.extend_from_slice(&(name.len() as u16).to_le_bytes());
        roh.extend_from_slice(&0u16.to_le_bytes()); // Zusatzfeld
        roh.extend_from_slice(&0u16.to_le_bytes()); // Kommentar
        roh.extend_from_slice(&0u16.to_le_bytes()); // Diskette
        roh.extend_from_slice(&0u16.to_le_bytes()); // intern
        roh.extend_from_slice(&0u32.to_le_bytes()); // extern
        roh.extend_from_slice(&0u32.to_le_bytes()); // lokaler Kopf
        roh.extend_from_slice(name.as_bytes());

        let zd_laenge = roh.len() - zd_versatz;
        roh.extend_from_slice(b"PK\x05\x06");
        roh.extend_from_slice(&[0; 4]);
        roh.extend_from_slice(&1u16.to_le_bytes());
        roh.extend_from_slice(&1u16.to_le_bytes());
        roh.extend_from_slice(&(zd_laenge as u32).to_le_bytes());
        roh.extend_from_slice(&(zd_versatz as u32).to_le_bytes());
        roh.extend_from_slice(&0u16.to_le_bytes());
        roh
    }

    #[test]
    fn liest_gespeicherten_eintrag() {
        let daten = zip_gespeichert("a.txt", b"hallo");
        let a = Archiv::neu(&daten).unwrap();
        assert_eq!(a.namen().collect::<Vec<_>>(), vec!["a.txt"]);
        assert_eq!(a.lies("a.txt").unwrap(), b"hallo");
    }

    #[test]
    fn meldet_fehlenden_eintrag() {
        let daten = zip_gespeichert("a.txt", b"x");
        let a = Archiv::neu(&daten).unwrap();
        assert!(matches!(a.lies("b.txt"), Err(ZipFehler::Fehlt(_))));
    }

    #[test]
    fn findet_erste_endung() {
        let daten = zip_gespeichert("projekt.zpac", b"inhalt");
        let a = Archiv::neu(&daten).unwrap();
        assert_eq!(a.liest_mit_endung_roh(".zpac").unwrap(), b"inhalt");
    }

    #[test]
    fn lehnt_nicht_zip_ab() {
        assert!(matches!(Archiv::neu(b"das ist kein zip"), Err(ZipFehler::KeinZip)));
        // Genau groß genug, aber ohne Sigel.
        assert!(matches!(Archiv::neu(&[0u8; 40]), Err(ZipFehler::KeinZip)));
    }

    #[test]
    fn meldet_unbekanntes_verfahren() {
        let mut daten = zip_gespeichert("a.txt", b"x");
        // Verfahren im zentralen Verzeichnis (erster Eintrag) auf 12 setzen.
        let zd = finde_zentrales_verzeichnis(&daten).unwrap();
        daten[zd.versatz + 10] = 12;
        let a = Archiv::neu(&daten).unwrap();
        assert!(matches!(a.lies("a.txt"), Err(ZipFehler::Verfahren { .. })));
    }

    #[test]
    fn liest_zip_mit_vorspann() {
        // `.zprj` beginnt mit einem CLO-Kopf; die Offsets im ZIP zählen ab
        // dem ZIP-Anfang. Ohne Vorspann-Korrektur zeigen sie ins Leere.
        let rein = zip_gespeichert("projekt.zpac", b"inhalt");
        let mut mit = vec![0u8; 500];
        mit[..16].copy_from_slice(b" ZPRJ       CLO ");
        mit.extend_from_slice(&rein);
        let a = Archiv::neu(&mit).unwrap();
        assert_eq!(a.vorspann(), 500);
        assert_eq!(a.lies("projekt.zpac").unwrap(), b"inhalt");
    }

    impl<'a> Archiv<'a> {
        fn liest_mit_endung_roh(&self, endung: &str) -> Result<Vec<u8>, ZipFehler> {
            self.lies_mit_endung(endung)
        }
    }
}
