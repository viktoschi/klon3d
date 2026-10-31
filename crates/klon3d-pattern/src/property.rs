//! Der Property-Stream: CLOs Baumformat für Projekte, Stoffe und Mustersätze.
//!
//! Ein CLO-Projekt ist ein ZIP-Archiv, in dem ein **Property-Stream**
//! liegt. Dieselbe Grammatik steckt in `.zfab` (Stoff), `.smp`
//! (Simulationseinstellungen), `.pos` (Lage), `.pac` (Mustersatz) und `.zprj`
//! bzw. `.zpac` (Projekt) - Befunde [57] und [62].
//!
//! ## Die Grammatik
//!
//! ```text
//! Map-Block
//!   u8    Anzahl der Felder
//!   n × u8   Länge des Feldnamens
//!   n × u8   Typcode
//!   n × u32  Größe des Wertes
//!   je Feld: Name (Länge Byte) + Wert (Größe Byte)
//!
//! Listen-Block
//!   u32   Anzahl der Einträge
//!   u8    Typcode aller Einträge   (nur wenn Anzahl > 0)
//!   n × u32  Größe des Eintrags
//!   dann die Einträge hintereinander - **kein Kopf je Eintrag**
//! ```
//!
//! Die Größen stehen alle **vor** den Daten. Das ist die Korrektur aus
//! Befund 57: die vier Bytes, die früher wie ein Marker aussahen, sind der
//! Größenwert des zweiten Eintrags.
//!
//! ## Der eine Unterschied zur Referenz
//!
//! Der Referenzparser aus Befund 57 verlangt `1 <= Anzahl <= 4096` und lehnt
//! damit **leere Maps** ab. In einem echten Projekt kommen sie vor:
//! `mapGarmentRiggingManager` ist genau ein Byte, nämlich `0x00`. Dieses Modul
//! erlaubt `Anzahl = 0`, hat aber an der 17,9-MB-`.pac` des Projekts
//! `japanese_school_uniform` geprüft, dass sonst **nichts** nachzugeben ist:
//! der Leser landet auf das Byte genau auf dem Dateiende.
//!
//! ## Geprüft
//!
//! An `japanese_school_uniform.zprj` (CLO):
//!
//! | Datei | Größe | Maps | Listen | Felder | Ende |
//! |---|---:|---:|---:|---:|---|
//! | `.pac` (Mustersatz) | 17 886 470 | 61 330 | 10 169 | 431 609 | exakt |
//! | `.pos` (Lage) | 158 141 | 745 | 287 | 4 635 | exakt |
//!
//! [57]: ../../../findings/57-clo-2026-1-property-stream-vollstaendig-gelesen.md
//! [62]: ../../../findings/62-clo-2026-1-der-mustersatz-pac.md

use std::fmt;

/// Typcodes, die als Block weiterverfolgt werden.
pub const TYP_MAP: u8 = 0x16;
pub const TYP_LIST: u8 = 0x14;

/// Höchstzahl Felder einer Map. Schutz gegen Unsinnsdaten, nicht gegen CLO:
/// die größte Map im geprüften Projekt hat 29 Felder.
const MAX_FELDER: usize = 4096;
/// Höchstzahl Einträge einer Liste. `listPattern` hat 31, `listFabric` 4.
const MAX_EINTRAEGE: usize = 4_000_000;

/// Fehler beim Lesen eines Property-Streams.
#[derive(Debug, thiserror::Error)]
pub enum StreamFehler {
    #[error("Dateiende bei 0x{pos:X}: {was}")]
    Dateiende { pos: usize, was: &'static str },

    #[error("Map bei 0x{pos:X}: {anzahl} Felder sind unplausibel")]
    MapAnzahl { pos: usize, anzahl: usize },

    #[error("Liste bei 0x{pos:X}: {anzahl} Einträge sind unplausibel")]
    ListenAnzahl { pos: usize, anzahl: usize },

    #[error("Block {pfad:?} bei 0x{pos:X}: {gelesen} Byte gelesen, {deklariert} deklariert")]
    Groesse {
        pfad: String,
        pos: usize,
        gelesen: usize,
        deklariert: usize,
    },

    #[error("Block {pfad:?} bei 0x{pos:X}: Feldname ist kein ASCII")]
    Feldname { pfad: String, pos: usize },
}

/// Ein Wert im Baum. Blätter tragen ihre Rohbytes, Maps und Listen ihre Kinder.
pub enum Wert {
    Map(Map),
    Liste(Liste),
    /// Alles andere, ungedeutet: Zahlen, Strings, Matrizen, Puffer.
    Blatt(Blatt),
}

impl Wert {
    pub fn typ(&self) -> u8 {
        match self {
            Wert::Map(_) => TYP_MAP,
            Wert::Liste(_) => TYP_LIST,
            Wert::Blatt(b) => b.typ,
        }
    }

    pub fn als_map(&self) -> Option<&Map> {
        match self {
            Wert::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn als_liste(&self) -> Option<&Liste> {
        match self {
            Wert::Liste(l) => Some(l),
            _ => None,
        }
    }

    pub fn als_blatt(&self) -> Option<&Blatt> {
        match self {
            Wert::Blatt(b) => Some(b),
            _ => None,
        }
    }

    /// Größe in der Datei (Byte), wie deklariert.
    pub fn groesse(&self) -> usize {
        match self {
            Wert::Map(m) => m.groesse,
            Wert::Liste(l) => l.groesse,
            Wert::Blatt(b) => b.roh.len(),
        }
    }

    /// Kurzform für Fehlermeldungen und Diagnose.
    pub fn art(&self) -> &'static str {
        match self {
            Wert::Map(_) => "map",
            Wert::Liste(_) => "list",
            Wert::Blatt(_) => "blatt",
        }
    }
}

impl fmt::Debug for Wert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Wert::Map(m) => write!(f, "Map({} Felder, {} Byte)", m.felder.len(), m.groesse),
            Wert::Liste(l) => write!(f, "Liste({} Einträge, {} Byte)", l.eintraege.len(), l.groesse),
            Wert::Blatt(b) => write!(f, "Blatt(0x{:02X}, {} Byte)", b.typ, b.roh.len()),
        }
    }
}

/// Ein Feld einer Map: Name und Wert.
pub struct Feld {
    pub name: String,
    /// Typcode des Werts, wie in der Datei deklariert.
    pub typ: u8,
    pub wert: Wert,
}

impl fmt::Debug for Feld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.name, self.wert)
    }
}

/// Eine Map: benannte Felder, Reihenfolge wie in der Datei.
#[derive(Debug)]
pub struct Map {
    pub felder: Vec<Feld>,
    pub groesse: usize,
}

impl Map {
    /// Feld nach Namen. CLO schreibt jedes Feld genau einmal.
    pub fn feld(&self, name: &str) -> Option<&Wert> {
        self.felder
            .iter()
            .find(|f| f.name == name)
            .map(|f| &f.wert)
    }

    pub fn hat(&self, name: &str) -> bool {
        self.felder.iter().any(|f| f.name == name)
    }

    /// Pfad über mehrere Namen, z. B. `["mapElement", "qsName"]`.
    pub fn pfad(&self, namen: &[&str]) -> Option<&Wert> {
        let (erst, rest) = namen.split_first()?;
        let mut w = self.feld(erst)?;
        for n in rest {
            w = w.als_map()?.feld(n)?;
        }
        Some(w)
    }

    /// Map-Kind über mehrere Namen.
    pub fn pfad_map(&self, namen: &[&str]) -> Option<&Map> {
        self.pfad(namen)?.als_map()
    }

    /// Liste-Kind über mehrere Namen.
    pub fn pfad_liste(&self, namen: &[&str]) -> Option<&Liste> {
        self.pfad(namen)?.als_liste()
    }
}

/// Eine Liste. Alle Einträge haben denselben Typcode.
#[derive(Debug)]
pub struct Liste {
    /// Typcode der Einträge (0, wenn die Liste leer ist).
    pub typ: u8,
    pub eintraege: Vec<Wert>,
    pub groesse: usize,
}

/// Ein Blatt: Typcode und Rohbytes. Die Deutung ist Sache des Aufrufers.
#[derive(Debug, Clone)]
pub struct Blatt {
    pub typ: u8,
    pub roh: Vec<u8>,
}

impl Blatt {
    /// Wert als `f32`, wenn das Blatt genau vier Byte hat.
    pub fn als_f32(&self) -> Option<f32> {
        (self.roh.len() == 4).then(|| f32::from_le_bytes(self.roh[..4].try_into().unwrap()))
    }

    /// Wert als `u32`.
    pub fn als_u32(&self) -> Option<u32> {
        (self.roh.len() == 4).then(|| u32::from_le_bytes(self.roh[..4].try_into().unwrap()))
    }

    /// Wert als `i32` (CLO schreibt `-1` für "kein Stoff").
    pub fn als_i32(&self) -> Option<i32> {
        (self.roh.len() == 4).then(|| i32::from_le_bytes(self.roh[..4].try_into().unwrap()))
    }

    /// Wert als `bool` (ein Byte, 0 oder 1).
    pub fn als_bool(&self) -> Option<bool> {
        (self.roh.len() == 1).then(|| self.roh[0] != 0)
    }

    /// Wert als Text. CLO schreibt ASCII; ungültige Bytes werden ersetzt.
    pub fn als_text(&self) -> Option<String> {
        Some(String::from_utf8_lossy(&self.roh).into_owned())
    }

    /// Zwei `f32` (Typ `0x0A`, z. B. `v2Position`).
    pub fn als_f32_2(&self) -> Option<[f32; 2]> {
        (self.roh.len() == 8).then(|| {
            [
                f32::from_le_bytes(self.roh[0..4].try_into().unwrap()),
                f32::from_le_bytes(self.roh[4..8].try_into().unwrap()),
            ]
        })
    }

    /// Drei `f32` (Typ `0x0B`, z. B. `v3Color`).
    pub fn als_f32_3(&self) -> Option<[f32; 3]> {
        (self.roh.len() == 12).then(|| {
            [
                f32::from_le_bytes(self.roh[0..4].try_into().unwrap()),
                f32::from_le_bytes(self.roh[4..8].try_into().unwrap()),
                f32::from_le_bytes(self.roh[8..12].try_into().unwrap()),
            ]
        })
    }

    /// Ein Array (Typ `0x15`): `u32` Elementzähler, `u8` Elementtyp, dann die
    /// Elemente.
    ///
    /// Der Typcode ist derselbe wie bei einem einzelnen Wert (`0x05` =
    /// `u32`, `0x0A` = zwei `f32`, …). Er steht **nur** bei nichtleeren
    /// Arrays; ein leeres Array ist genau vier Byte (`n = 0`).
    ///
    /// Geprüft an `japanese_school_uniform`: **2034 von 2034** Arrays dieser
    /// `.pac` gehen mit `5 + n · Breite(Typ)` auf, keines weicht ab.
    ///
    /// Gibt `(zähler, typ, rumpf)` zurück. Ob der Rumpf zu `zähler` passt,
    /// prüft der Aufrufer, weil die Elementbreite typabhängig ist.
    pub fn als_array(&self) -> Option<(u32, u8, &[u8])> {
        let n = u32::from_le_bytes(self.roh.get(0..4)?.try_into().ok()?);
        if n == 0 {
            // Leeres Array: nur der Zähler, kein Typbyte.
            (self.roh.len() == 4).then_some((0, 0, &self.roh[4..]))
        } else {
            (self.roh.len() >= 5).then(|| (n, self.roh[4], &self.roh[5..]))
        }
    }

    /// Elementbreite eines Array-Typs in Byte, `None` bei unbekanntem Typ.
    pub fn breite(typ: u8) -> Option<usize> {
        match typ {
            0x01 => Some(1),  // bool
            0x04 | 0x05 | 0x06 | 0x07 => Some(4), // int/uint/float
            0x0A => Some(8),  // zwei f32
            0x0B => Some(12), // drei f32
            0x0D => Some(36), // 3×3
            _ => None,
        }
    }

    /// Ein Array aus `i32`-Werten, z. B. `arrMeshPointIndex`.
    ///
    /// Prüft Zähler **und** Typ (`0x04`/`0x05`/`0x07`, also vier Byte je
    /// Element); sonst `None` statt einer Fehldeutung.
    pub fn als_i32_array(&self) -> Option<Vec<i32>> {
        let (n, typ, rumpf) = self.als_array()?;
        if n == 0 {
            return Some(Vec::new());
        }
        if !matches!(typ, 0x04 | 0x05 | 0x07) {
            return None;
        }
        let n = n as usize;
        if rumpf.len() != n * 4 {
            return None;
        }
        Some(
            rumpf
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        )
    }
}

/// Liest einen Property-Stream ab einer Basisposition.
pub struct Leser<'a> {
    d: &'a [u8],
    p: usize,
    /// Nur für Fehlermeldungen: Pfad des gerade gelesenen Blocks.
    pfad: String,
}

impl<'a> Leser<'a> {
    pub fn neu(d: &'a [u8]) -> Self {
        Self {
            d,
            p: 0,
            pfad: String::new(),
        }
    }

    /// Liest ab einer Basisposition (CLO-Dateien mit Kopf nennen sie bei
    /// `0x20`). `basis` muss auf den ersten Map-Block zeigen.
    pub fn ab(d: &'a [u8], basis: usize) -> Result<Self, StreamFehler> {
        if basis > d.len() {
            return Err(StreamFehler::Dateiende {
                pos: basis,
                was: "Basisposition liegt hinter dem Dateiende",
            });
        }
        Ok(Self {
            d,
            p: basis,
            pfad: String::new(),
        })
    }

    /// Position des nächsten ungelesenen Bytes.
    pub fn position(&self) -> usize {
        self.p
    }

    pub fn rest(&self) -> usize {
        self.d.len().saturating_sub(self.p)
    }

    fn byte(&mut self) -> Result<u8, StreamFehler> {
        let b = *self.d.get(self.p).ok_or(StreamFehler::Dateiende {
            pos: self.p,
            was: "Byte",
        })?;
        self.p += 1;
        Ok(b)
    }

    fn u32(&mut self) -> Result<u32, StreamFehler> {
        let s = self
            .d
            .get(self.p..self.p + 4)
            .ok_or(StreamFehler::Dateiende {
                pos: self.p,
                was: "u32",
            })?;
        self.p += 4;
        Ok(u32::from_le_bytes(s.try_into().unwrap()))
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], StreamFehler> {
        let s = self.d.get(self.p..self.p + n).ok_or(StreamFehler::Dateiende {
            pos: self.p,
            was: "Wertbytes",
        })?;
        self.p += n;
        Ok(s)
    }

    /// Liest den Map-Block an der aktuellen Position.
    ///
    /// `groesse` ist die vom Elternblock deklarierte Länge; sie wird gegen die
    /// tatsächlich gelesene geprüft. `None` nur für die Wurzel.
    pub fn map(&mut self, groesse: Option<usize>) -> Result<Map, StreamFehler> {
        let p0 = self.p;
        let anzahl = self.byte()? as usize;
        if anzahl > MAX_FELDER {
            return Err(StreamFehler::MapAnzahl { pos: p0, anzahl });
        }
        let mut laengen = Vec::with_capacity(anzahl);
        for _ in 0..anzahl {
            laengen.push(self.byte()? as usize);
        }
        let mut typen = Vec::with_capacity(anzahl);
        for _ in 0..anzahl {
            typen.push(self.byte()?);
        }
        let mut groessen = Vec::with_capacity(anzahl);
        for _ in 0..anzahl {
            groessen.push(self.u32()? as usize);
        }

        let mut felder = Vec::with_capacity(anzahl);
        for i in 0..anzahl {
            let namen_pos = self.p;
            let roh_name = self.bytes(laengen[i])?;
            let name = std::str::from_utf8(roh_name)
                .map_err(|_| StreamFehler::Feldname {
                    pfad: self.pfad.clone(),
                    pos: namen_pos,
                })?
                .to_string();
            let typ = typen[i];
            let g = groessen[i];
            let wert = self.wert(typ, g, &name)?;
            felder.push(Feld { name, typ, wert });
        }

        let gelesen = self.p - p0;
        if let Some(soll) = groesse {
            if gelesen != soll {
                return Err(StreamFehler::Groesse {
                    pfad: self.pfad.clone(),
                    pos: p0,
                    gelesen,
                    deklariert: soll,
                });
            }
        }
        Ok(Map {
            felder,
            groesse: gelesen,
        })
    }

    fn liste(&mut self, groesse: Option<usize>) -> Result<Liste, StreamFehler> {
        let p0 = self.p;
        let anzahl = self.u32()? as usize;
        if anzahl > MAX_EINTRAEGE {
            return Err(StreamFehler::ListenAnzahl { pos: p0, anzahl });
        }
        let mut eintraege = Vec::new();
        let mut typ = 0u8;
        if anzahl > 0 {
            typ = self.byte()?;
            let mut groessen = Vec::with_capacity(anzahl);
            for _ in 0..anzahl {
                groessen.push(self.u32()? as usize);
            }
            eintraege.reserve(anzahl);
            for (i, g) in groessen.into_iter().enumerate() {
                let neu = format!("{}[{i}]", self.pfad);
                let alt = std::mem::replace(&mut self.pfad, neu);
                let w = self.wert(typ, g, "")?;
                self.pfad = alt;
                eintraege.push(w);
            }
        }

        let gelesen = self.p - p0;
        if let Some(soll) = groesse {
            if gelesen != soll {
                return Err(StreamFehler::Groesse {
                    pfad: self.pfad.clone(),
                    pos: p0,
                    gelesen,
                    deklariert: soll,
                });
            }
        }
        Ok(Liste {
            typ,
            eintraege,
            groesse: gelesen,
        })
    }

    fn wert(&mut self, typ: u8, groesse: usize, _name: &str) -> Result<Wert, StreamFehler> {
        match typ {
            TYP_MAP => {
                let alt = std::mem::take(&mut self.pfad);
                let m = self.map(Some(groesse))?;
                self.pfad = alt;
                Ok(Wert::Map(m))
            }
            TYP_LIST => {
                let alt = std::mem::take(&mut self.pfad);
                let l = self.liste(Some(groesse))?;
                self.pfad = alt;
                Ok(Wert::Liste(l))
            }
            _ => {
                let roh = self.bytes(groesse)?.to_vec();
                Ok(Wert::Blatt(Blatt { typ, roh }))
            }
        }
    }

    /// Setzt den Pfad für Fehlermeldungen (Wurzelname).
    pub fn mit_pfad(mut self, pfad: &str) -> Self {
        self.pfad = pfad.to_string();
        self
    }
}

/// Liest eine CLO-Datei **mit Kopf** (`.pos`, `.arr`, `.pan`, `.zprj` innere
/// Dateien): `u32` bei `0x20` ist der Beginn des Streams, `u32` bei `0x24`
/// seine Länge, und beide zusammen ergeben die Dateigröße (Befund 57, 46 von
/// 46 Dateien).
///
/// Gibt `(Map, basis, laenge)` zurück.
pub fn lies_mit_kopf(d: &[u8]) -> Result<(Map, usize, usize), StreamFehler> {
    if d.len() < 0x28 {
        return Err(StreamFehler::Dateiende {
            pos: d.len(),
            was: "Kopf einer CLO-Datei (40 Byte)",
        });
    }
    let basis = u32::from_le_bytes(d[0x20..0x24].try_into().unwrap()) as usize;
    let laenge = u32::from_le_bytes(d[0x24..0x28].try_into().unwrap()) as usize;
    let mut leser = Leser::ab(d, basis)?.mit_pfad("wurzel");
    let map = leser.map(Some(laenge))?;
    Ok((map, basis, laenge))
}

/// Prüft, ob eine Datei einen CLO-Kopf trägt: 16 Byte Magic, dann `u32`, und
/// `u32[0x20] + u32[0x24] == Dateigröße`.
///
/// Die `.pac` eines Projekts hat **keinen** Kopf und beginnt direkt mit dem
/// Map-Block (Befund 58: "der 12-Byte-Kopf war keiner"). Diese Funktion
/// erkennt den Unterschied, statt ihn zu vermuten.
pub fn hat_kopf(d: &[u8]) -> bool {
    if d.len() < 0x28 {
        return false;
    }
    let basis = u32::from_le_bytes(d[0x20..0x24].try_into().unwrap()) as usize;
    let laenge = u32::from_le_bytes(d[0x24..0x28].try_into().unwrap()) as usize;
    basis >= 0x28 && laenge > 0 && basis + laenge == d.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baut einen Map-Block mit einem gespeicherten Blatt.
    fn map_mit_zahl(name: &str, typ: u8, wert: u32) -> Vec<u8> {
        let mut v = vec![1u8]; // ein Feld
        v.push(name.len() as u8);
        v.push(typ);
        v.extend_from_slice(&4u32.to_le_bytes());
        v.extend_from_slice(name.as_bytes());
        v.extend_from_slice(&wert.to_le_bytes());
        v
    }

    #[test]
    fn liest_map_mit_einem_feld() {
        let d = map_mit_zahl("uiVersion", 0x05, 1000);
        let mut l = Leser::neu(&d);
        let m = l.map(None).unwrap();
        assert_eq!(m.felder.len(), 1);
        assert_eq!(
            m.feld("uiVersion").unwrap().als_blatt().unwrap().als_u32(),
            Some(1000)
        );
        assert_eq!(l.rest(), 0);
    }

    #[test]
    fn akzeptiert_leere_map() {
        // Der eine Punkt, an dem die Referenz aus Befund 57 zu streng war:
        // mapGarmentRiggingManager ist genau ein 0x00-Byte.
        let d = [0u8];
        let mut l = Leser::neu(&d);
        let m = l.map(None).unwrap();
        assert!(m.felder.is_empty());
        assert_eq!(m.groesse, 1);
        assert_eq!(l.rest(), 0);
    }

    #[test]
    fn liest_liste_mit_zwei_eintraegen() {
        // Liste mit zwei Maps, Größen stehen vor den Daten.
        let a = map_mit_zahl("a", 0x05, 1);
        let b = map_mit_zahl("b", 0x05, 2);
        let mut d = Vec::new();
        d.extend_from_slice(&2u32.to_le_bytes());
        d.push(TYP_MAP);
        d.extend_from_slice(&(a.len() as u32).to_le_bytes());
        d.extend_from_slice(&(b.len() as u32).to_le_bytes());
        d.extend_from_slice(&a);
        d.extend_from_slice(&b);

        let mut l = Leser::neu(&d);
        let liste = l.liste(None).unwrap();
        assert_eq!(liste.eintraege.len(), 2);
        assert_eq!(liste.typ, TYP_MAP);
        assert_eq!(l.rest(), 0);
    }

    #[test]
    fn leere_liste_ist_vier_byte() {
        let d = 0u32.to_le_bytes();
        let mut l = Leser::neu(&d);
        let liste = l.liste(None).unwrap();
        assert!(liste.eintraege.is_empty());
        assert_eq!(liste.groesse, 4);
    }

    #[test]
    fn meldet_falsche_groesse() {
        let mut d = map_mit_zahl("x", 0x05, 7);
        let mut l = Leser::neu(&d);
        // Eine zu kleine deklarierte Größe muss auffallen.
        let fehler = l.map(Some(3)).unwrap_err();
        assert!(matches!(fehler, StreamFehler::Groesse { .. }));
        // Und eine zu große ebenfalls.
        d.push(0);
        let mut l = Leser::neu(&d);
        assert!(l.map(Some(d.len() + 5)).is_err());
    }

    #[test]
    fn blatt_deutungen() {
        let b = Blatt {
            typ: 0x0A,
            roh: {
                let mut v = Vec::new();
                v.extend_from_slice(&1.5f32.to_le_bytes());
                v.extend_from_slice(&(-2.5f32).to_le_bytes());
                v
            },
        };
        assert_eq!(b.als_f32_2(), Some([1.5, -2.5]));
        assert_eq!(b.als_f32(), None); // acht Byte, nicht vier

        let neg = Blatt {
            typ: 0x05,
            roh: (-1i32).to_le_bytes().to_vec(),
        };
        assert_eq!(neg.als_i32(), Some(-1));
        assert_eq!(neg.als_u32(), Some(u32::MAX));
    }

    #[test]
    fn array_mit_zaehler_und_typ() {
        // Kopf: u32 Anzahl, u8 Elementtyp, dann die Elemente.
        let mut roh = 3u32.to_le_bytes().to_vec();
        roh.push(0x05); // uint32
        for v in [10i32, 20, 30] {
            roh.extend_from_slice(&v.to_le_bytes());
        }
        let b = Blatt { typ: 0x15, roh };
        assert_eq!(b.als_i32_array(), Some(vec![10, 20, 30]));

        // Leeres Array ist genau vier Byte, ohne Typbyte.
        let b = Blatt {
            typ: 0x15,
            roh: 0u32.to_le_bytes().to_vec(),
        };
        assert_eq!(b.als_i32_array(), Some(Vec::new()));
        assert_eq!(b.als_array().map(|(n, t, r)| (n, t, r.len())), Some((0, 0, 0)));

        // Zähler passt nicht zur Rumpflänge: lieber None als raten.
        let b = Blatt {
            typ: 0x15,
            roh: 5u32.to_le_bytes().to_vec(),
        };
        assert_eq!(b.als_i32_array(), None);
    }

    #[test]
    fn array_lehnt_fremden_elementtyp_ab() {
        // Ein Array aus zwei f32-Paaren ist kein i32-Array.
        let mut roh = 2u32.to_le_bytes().to_vec();
        roh.push(0x0A);
        roh.extend_from_slice(&[0u8; 16]);
        let b = Blatt { typ: 0x15, roh };
        assert_eq!(b.als_i32_array(), None);
        assert_eq!(b.als_array().map(|(n, t, r)| (n, t, r.len())), Some((2, 0x0A, 16)));
    }

    #[test]
    fn kopf_erkennung() {
        // Datei mit Kopf: 40 Byte Kopf, Stream ab 0x28.
        let mut d = vec![0u8; 0x28];
        let strom = map_mit_zahl("x", 0x05, 1);
        d[0x20..0x24].copy_from_slice(&0x28u32.to_le_bytes());
        d[0x24..0x28].copy_from_slice(&(strom.len() as u32).to_le_bytes());
        d.extend_from_slice(&strom);
        assert!(hat_kopf(&d));
        let (m, basis, laenge) = lies_mit_kopf(&d).unwrap();
        assert_eq!(basis, 0x28);
        assert_eq!(laenge, strom.len());
        assert!(m.hat("x"));

        // Die .pac beginnt ohne Kopf direkt mit dem Map-Block.
        let pac = map_mit_zahl("y", 0x05, 2);
        assert!(!hat_kopf(&pac));
    }
}
