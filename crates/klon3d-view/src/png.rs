//! PNG schreiben, ohne Crate und ohne GDI+-Encoder.
//!
//! ## Warum selbst
//!
//! `GdipSaveImageToFile` verweigerte hier mit Code 7 (`FileNotFound`), obwohl
//! der Ordner existierte und die Datei angelegt wurde (Stand 19.10.2026,
//! Windows 11 22621). Der Bitmap-Speicher selbst war in Ordnung — nur der
//! Encoder wollte nicht. PNG ist einfach genug, es ohne fremde Hilfe zu
//! schreiben, und der Betrachter soll keine weitere Abhängigkeit bekommen.
//!
//! ## Aufbau
//!
//! ```text
//! 89 50 4E 47 0D 0A 1A 0A        Signatur
//! IHDR  Breite Höhe 8 Tiefe 6 Farbe 0 0 0      (4 + 13 Bytes)
//! IDAT  zlib-Strom über die Rohzeilen
//! IEND
//! ```
//!
//! Der zlib-Strom ist `78 01` (keine Kompression, 32-K-Fenster) gefolgt von
//! **ungespeicherten** Deflate-Blöcken: jeder Block trägt im Kopf ein
//! Schlussbit, seine Länge und deren Einerkomplement, danach höchstens
//! 65535 Rohbytes; am Ende steht Adler-32. Das ist gültiges DEFLATE und
//! braucht keine Kompressionsbibliothek. Die Datei wird dadurch
//! rund viermal so gross wie eine mit Kompression — fuer eine Bildschirm-
//! kopie ist das in Ordnung, und die Datei laesst sich ueberall lesen.

use std::io;

/// CRC-32 nach PNG (Polynom `0xEDB88320`, Startwert und Endwert `0xFFFFFFFF`).
fn crc32(daten: &[u8]) -> u32 {
    let mut tabelle = [0u32; 256];
    for (i, e) in tabelle.iter_mut().enumerate() {
        let mut w = i as u32;
        for _ in 0..8 {
            w = if w & 1 != 0 { 0xEDB8_8320 ^ (w >> 1) } else { w >> 1 };
        }
        *e = w;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in daten {
        c = tabelle[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Adler-32 — die Prüfsumme des zlib-Stroms.
fn adler32(daten: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in daten {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn stueck(typ: &[u8; 4], daten: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + daten.len());
    v.extend_from_slice(&(daten.len() as u32).to_be_bytes());
    v.extend_from_slice(typ);
    v.extend_from_slice(daten);
    let mut crc_daten = Vec::with_capacity(4 + daten.len());
    crc_daten.extend_from_slice(typ);
    crc_daten.extend_from_slice(daten);
    v.extend_from_slice(&crc32(&crc_daten).to_be_bytes());
    v
}

/// PNG aus RGBA-Bytes schreiben (oben links nach unten rechts).
pub fn schreibe(
    ziel: &std::path::Path,
    breite: i32,
    hoehe: i32,
    rgba: &[u8],
) -> io::Result<()> {
    if breite <= 0 || hoehe <= 0 {
        return Err(io::Error::other("Bild ohne Fläche"));
    }
    let b = breite as usize;
    let h = hoehe as usize;
    if rgba.len() < b * h * 4 {
        return Err(io::Error::other("zu wenige Bilddaten"));
    }

    let mut raus: Vec<u8> = Vec::with_capacity(b * h * 4 + b * h + 1024);
    raus.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(b as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 Bit je Kanal, Farbe 6 = RGBA
    raus.extend_from_slice(&stueck(b"IHDR", &ihdr));

    // Rohdaten: je Zeile ein Filterbyte 0, dann die Pixel.
    let mut roh = Vec::with_capacity((b * 4 + 1) * h);
    for y in 0..h {
        roh.push(0u8);
        let start = y * b * 4;
        roh.extend_from_slice(&rgba[start..start + b * 4]);
    }

    // zlib-Strom mit ungespeicherten DEFLATE-Bloecken.
    let mut zlib = Vec::with_capacity(roh.len() + 64);
    zlib.extend_from_slice(&[0x78, 0x01]);
    let mut rest = roh.as_slice();
    if rest.is_empty() {
        // Ein leerer Block ist gueltig; `rest` ist nie leer, aber der Zweig
        // haelt den Rand sichtbar.
        zlib.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    while !rest.is_empty() {
        let n = rest.len().min(0xFFFF);
        let letzter = n == rest.len();
        zlib.push(u8::from(letzter));
        zlib.extend_from_slice(&(n as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(n as u16)).to_le_bytes());
        zlib.extend_from_slice(&rest[..n]);
        rest = &rest[n..];
    }
    zlib.extend_from_slice(&adler32(&roh).to_be_bytes());
    raus.extend_from_slice(&stueck(b"IDAT", &zlib));

    raus.extend_from_slice(&stueck(b"IEND", &[]));
    std::fs::write(ziel, raus)
}

/// BGRA (wie eine DIB-Section aus GDI) in RGBA umsortieren, Alpha auf 255.
///
/// Das vierte Byte einer GDI-Zeichnung ist kein Alpha: Linien und Text
/// hinterlassen dort 0. Übernommen als Alpha, wären genau sie im PNG
/// durchsichtig — so geschehen am 19.10.2026, als 8.536 Pixel Nahtlinie und
/// die ganze Beschriftung im Bild standen, aber kein Betrachter sie zeigte.
pub fn aus_bgra(bgra: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(bgra.len());
    for p in bgra.chunks_exact(4) {
        v.extend_from_slice(&[p[2], p[1], p[0], 255]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein PNG aus `schreibe` zurücklesen: Blöcke, zlib-Kopf, ungespeicherte
    /// DEFLATE-Blöcke, Filterbytes — und die Prüfsummen dabei nachrechnen.
    fn lies(datei: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(&datei[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut i = 8;
        let (mut b, mut h, mut idat) = (0u32, 0u32, Vec::new());
        while i < datei.len() {
            let n = u32::from_be_bytes(datei[i..i + 4].try_into().unwrap()) as usize;
            let typ = &datei[i + 4..i + 8];
            let inhalt = &datei[i + 8..i + 8 + n];
            let crc = u32::from_be_bytes(datei[i + 8 + n..i + 12 + n].try_into().unwrap());
            assert_eq!(crc32(&datei[i + 4..i + 8 + n]), crc, "CRC {typ:?}");
            match typ {
                b"IHDR" => {
                    b = u32::from_be_bytes(inhalt[0..4].try_into().unwrap());
                    h = u32::from_be_bytes(inhalt[4..8].try_into().unwrap());
                    assert_eq!(&inhalt[8..13], &[8, 6, 0, 0, 0]);
                }
                b"IDAT" => idat.extend_from_slice(inhalt),
                _ => {}
            }
            i += 12 + n;
        }
        assert_eq!(&idat[..2], &[0x78, 0x01]);
        let (mut j, mut roh) = (2, Vec::new());
        loop {
            let letzter = idat[j] & 1 == 1;
            assert_eq!(idat[j] >> 1, 0, "nur ungespeicherte Blöcke");
            let len = u16::from_le_bytes([idat[j + 1], idat[j + 2]]) as usize;
            let nlen = u16::from_le_bytes([idat[j + 3], idat[j + 4]]);
            assert_eq!(nlen, !(len as u16));
            roh.extend_from_slice(&idat[j + 5..j + 5 + len]);
            j += 5 + len;
            if letzter {
                break;
            }
        }
        let adler = u32::from_be_bytes(idat[j..j + 4].try_into().unwrap());
        assert_eq!(adler32(&roh), adler);
        let zeile = b as usize * 4 + 1;
        let mut px = Vec::new();
        for z in roh.chunks_exact(zeile) {
            assert_eq!(z[0], 0, "Filter 0");
            px.extend_from_slice(&z[1..]);
        }
        (b, h, px)
    }

    #[test]
    fn bgra_wird_rgba_und_undurchsichtig() {
        // Zwei Bildpunkte, wie GDI sie hinterlässt: Alpha 0 und 255.
        let bgra = [30, 20, 10, 0, 3, 2, 1, 255];
        assert_eq!(aus_bgra(&bgra), [10, 20, 30, 255, 1, 2, 3, 255]);
    }

    /// Hin und zurück, über mehr als einen DEFLATE-Block (65535 Bytes).
    #[test]
    fn liest_sich_zurueck_wie_geschrieben() {
        let (b, h) = (130i32, 140i32);
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..b {
                px.extend_from_slice(&[x as u8, y as u8, (x ^ y) as u8, 255]);
            }
        }
        let ziel = std::env::temp_dir().join("klon3d_png_rueck.png");
        schreibe(&ziel, b, h, &px).unwrap();
        let datei = std::fs::read(&ziel).unwrap();
        std::fs::remove_file(&ziel).ok();
        let (lb, lh, lpx) = lies(&datei);
        assert_eq!((lb, lh), (b as u32, h as u32));
        assert_eq!(lpx, px);
    }

    /// Ein 2×1-Bild: rot und grün, Alpha undurchsichtig.
    #[test]
    fn schreibt_eine_gueltige_kennung_und_groesse() {
        let px = [255u8, 0, 0, 255, 0, 255, 0, 255];
        let dir = std::env::temp_dir();
        let ziel = dir.join("klon3d_png_test.png");
        schreibe(&ziel, 2, 1, &px).unwrap();
        let b = std::fs::read(&ziel).unwrap();
        assert_eq!(&b[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // IHDR muss Breite 2 und Höhe 1 melden.
        assert_eq!(&b[16..20], b"\x00\x00\x00\x02");
        assert_eq!(&b[20..24], b"\x00\x00\x00\x01");
        std::fs::remove_file(&ziel).ok();
    }

    #[test]
    fn crc_stimmt_gegen_einen_bekannten_wert() {
        // "IEND" mit leerem Inhalt ergibt 0xAE426082.
        let v = crc32(b"IEND");
        assert_eq!(v, 0xAE42_6082);
    }

}
