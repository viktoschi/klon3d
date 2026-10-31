//! Zeichenfläche im Speicher und GDI-Objekte, die sich selbst aufräumen.
//!
//! ## Warum alles über eine Leinwand geht
//!
//! Der Betrachter zeichnet nie direkt ins Fenster. Er zeichnet in eine
//! DIB-Section mit eigenem DC und kopiert sie danach in einem Zug hinein.
//! Das hat drei Folgen:
//!
//! * kein Flackern beim Ziehen und Zoomen;
//! * das PNG enthält **genau die gezeichneten Bytes** — keine Rückleseprobe
//!   vom Bildschirm, die vom Fenstermanager, von der Sperre des Rechners oder
//!   von einem verdeckten Fenster abhängt (die Messungen vom 19.10.2026
//!   scheiterten genau daran: `GetPixel` lieferte `CLR_INVALID`);
//! * die Zeichnung ist ohne Fenster prüfbar, also auch in `cargo test`.
//!
//! ## Was GDI mit dem vierten Byte macht
//!
//! Eine 32-Bit-DIB-Section liegt als **BGRA** im Speicher. GDI pflegt das
//! vierte Byte nicht: Linien und Text hinterlassen dort 0, manche Füllungen
//! 255. Ein PNG, das diese Bytes ungeprüft als Alpha übernimmt, ist an
//! genau den Stellen durchsichtig, an denen Nahtlinien und Schrift stehen.
//! [`Leinwand::rgba`] setzt Alpha deshalb überall auf 255.

use std::ffi::c_void;

use crate::png;
use crate::win::*;

/// Eine Zeichenfläche im Speicher: DIB-Section, oben links zuerst, mit
/// eigenem DC.
pub struct Leinwand {
    pub dc: HDC,
    bitmap: HGDIOBJ,
    alt: HGDIOBJ,
    bits: *mut u8,
    pub breite: i32,
    pub hoehe: i32,
}

impl Leinwand {
    /// Eine leere Leinwand; `None`, wenn sie keine Fläche hätte oder GDI
    /// keine hergibt.
    pub fn neu(breite: i32, hoehe: i32) -> Option<Leinwand> {
        if breite <= 0 || hoehe <= 0 {
            return None;
        }
        unsafe {
            // Ein DC ohne Fenster: mit dem Bildschirm verträglich, aber nur
            // im Speicher.
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let kopf = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: breite,
                biHeight: -hoehe, // negative Höhe: Zeilen von oben nach unten
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                dc,
                &kopf,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                return None;
            }
            let alt = SelectObject(dc, bitmap);
            Some(Leinwand {
                dc,
                bitmap,
                alt,
                bits: bits as *mut u8,
                breite,
                hoehe,
            })
        }
    }

    /// Die Bildpunkte als RGBA, oben links zuerst, überall undurchsichtig.
    pub fn rgba(&self) -> Vec<u8> {
        let n = self.breite as usize * self.hoehe as usize * 4;
        unsafe {
            // Ohne das können gesammelte GDI-Aufrufe noch ausstehen.
            GdiFlush();
            png::aus_bgra(std::slice::from_raw_parts(self.bits, n))
        }
    }
}

impl Drop for Leinwand {
    fn drop(&mut self) {
        unsafe {
            // Erst abwählen, dann löschen — eine gewählte Bitmap lässt sich
            // nicht löschen.
            SelectObject(self.dc, self.alt);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

/// Ein eigenes GDI-Objekt (Stift, Pinsel, Schrift), das beim Verlassen
/// gelöscht wird.
///
/// Zusammen mit [`Auswahl`] ist die Reihenfolge durch die Deklaration
/// festgelegt: Rust räumt in umgekehrter Reihenfolge auf, also wird das
/// Objekt erst abgewählt und dann gelöscht.
///
/// ```ignore
/// let stift = Objekt(CreatePen(PS_SOLID, 1, farbe));
/// let _s = Auswahl::neu(dc, stift.0);
/// // ... zeichnen ...
/// // Ende: _s setzt den alten Stift zurück, danach wird `stift` gelöscht.
/// ```
pub struct Objekt(pub HGDIOBJ);

impl Drop for Objekt {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                DeleteObject(self.0);
            }
        }
    }
}

/// Ein Objekt, das im DC ausgewählt ist; beim Verlassen wird das vorige
/// wieder eingesetzt.
pub struct Auswahl {
    dc: HDC,
    alt: HGDIOBJ,
}

impl Auswahl {
    /// # Safety
    /// `dc` und `obj` müssen gültige GDI-Handles sein, die länger leben als
    /// die Auswahl.
    pub unsafe fn neu(dc: HDC, obj: HGDIOBJ) -> Auswahl {
        Auswahl {
            dc,
            alt: SelectObject(dc, obj),
        }
    }
}

impl Drop for Auswahl {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.alt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine Fläche in einer Farbe mit drei verschiedenen Kanälen — der
    /// Durchstich von `rgb()` über die DIB-Section bis zu den PNG-Bytes.
    /// Genau hier lagen zwei Vertauschungen, die sich im PNG aufhoben.
    #[test]
    fn farbe_kommt_kanalrichtig_und_undurchsichtig_an() {
        let l = Leinwand::neu(8, 4).expect("Leinwand");
        unsafe {
            let pinsel = Objekt(CreateSolidBrush(rgb(200, 100, 50)));
            let rc = RECT {
                left: 0,
                top: 0,
                right: 8,
                bottom: 4,
            };
            FillRect(l.dc, &rc, pinsel.0);
        }
        let px = l.rgba();
        assert_eq!(px.len(), 8 * 4 * 4);
        for p in px.chunks_exact(4) {
            assert_eq!(p, [200, 100, 50, 255]);
        }
    }

    /// Eine Linie mit dem Stift kommt an — mit Alpha 255, obwohl GDI dort 0
    /// hinterlässt.
    #[test]
    fn linie_kommt_an() {
        let l = Leinwand::neu(16, 16).expect("Leinwand");
        unsafe {
            let weiss = Objekt(CreateSolidBrush(rgb(255, 255, 255)));
            let rc = RECT {
                left: 0,
                top: 0,
                right: 16,
                bottom: 16,
            };
            FillRect(l.dc, &rc, weiss.0);
            let stift = Objekt(CreatePen(PS_SOLID, 1, rgb(10, 20, 30)));
            let _s = Auswahl::neu(l.dc, stift.0);
            let linie = [POINT { x: 2, y: 8 }, POINT { x: 14, y: 8 }];
            Polyline(l.dc, linie.as_ptr(), 2);
        }
        let px = l.rgba();
        let bei = |x: usize, y: usize| &px[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
        assert_eq!(bei(2, 8), [10, 20, 30, 255]);
        assert_eq!(bei(13, 8), [10, 20, 30, 255]);
        // Der Endpunkt gehört bei GDI nicht zur Linie.
        assert_eq!(bei(14, 8), [255, 255, 255, 255]);
        assert_eq!(bei(8, 4), [255, 255, 255, 255]);
    }
}
