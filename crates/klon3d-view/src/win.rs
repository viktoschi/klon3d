//! Die Win32-Schnittstelle, so weit der Betrachter sie braucht.
//!
//! Alles über `extern "system"` gegen `user32`/`gdi32`, ohne Crate. Kein
//! Cargo-Abhängigkeitsbaum, kein Treiber, kein Grafikstack: `CreateWindowExW`
//! genügt, um eine Fläche zu bekommen, und GDI reicht, um 1000 Linien
//! hineinzuzeichnen.
//!
//! ## Zwei Regeln, die hier schon einmal falsch waren
//!
//! * **`BOOL` ist ein `int`**, vier Bytes. Rusts `bool` ist ein Byte; als
//!   Rückgabewert liest Rust davon nur das unterste Byte, und jeder Wert
//!   ausser 0 und 1 ist undefiniertes Verhalten. Deshalb [`BOOL`] = `i32`.
//! * **`COLORREF` ist `0x00BBGGRR`**, Rot im niedrigsten Byte — siehe [`rgb`].

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;

pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HGDIOBJ = *mut c_void;
pub type HFONT = *mut c_void;
pub type HPEN = *mut c_void;
pub type HANDLE = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HICON = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type LPARAM = isize;
pub type WPARAM = usize;
pub type LRESULT = isize;
/// Win32-`BOOL`: ein `int`, nicht Rusts Ein-Byte-`bool`.
pub type BOOL = i32;

pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;

pub const SW_SHOWNORMAL: i32 = 1;

pub const CS_HREDRAW: u32 = 0x0002;
pub const CS_VREDRAW: u32 = 0x0001;

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_MOUSEMOVE: u32 = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_MOUSEWHEEL: u32 = 0x020A;
pub const WM_CAPTURECHANGED: u32 = 0x0215;

pub const WHEEL_DELTA: i16 = 120;

/// Virtuelle Tasten. Buchstaben und Ziffern sind ihre **Grossbuchstaben**
/// in ASCII (`b'R'` = 0x52); `b'r'` = 0x72 wäre `VK_F3`.
pub const VK_TAB: u8 = 0x09;
pub const VK_SHIFT: i32 = 0x10;
pub const VK_ESCAPE: u8 = 0x1B;
pub const VK_SPACE: u8 = 0x20;
pub const VK_LEFT: u8 = 0x25;
pub const VK_UP: u8 = 0x26;
pub const VK_RIGHT: u8 = 0x27;
pub const VK_DOWN: u8 = 0x28;
pub const VK_ADD: u8 = 0x6B;
pub const VK_SUBTRACT: u8 = 0x6D;
pub const VK_OEM_PLUS: u8 = 0xBB;
pub const VK_OEM_MINUS: u8 = 0xBD;

pub const IDC_ARROW: usize = 32512;

/// `GetDeviceCaps`: Bildpunkte je Zoll waagerecht.
pub const LOGPIXELSX: i32 = 88;
/// `GetSystemMetrics`: Clientfläche eines Vollbildfensters (ohne Taskleiste).
pub const SM_CXFULLSCREEN: i32 = 16;
pub const SM_CYFULLSCREEN: i32 = 17;

/// `RECT` ist in beiden Aufrufen ein Zeiger — deshalb `*mut`, nicht By Value.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [u8; 32],
}

impl Default for PAINTSTRUCT {
    fn default() -> Self {
        PAINTSTRUCT {
            hdc: std::ptr::null_mut(),
            fErase: 0,
            rcPaint: RECT::default(),
            fRestore: 0,
            fIncUpdate: 0,
            rgbReserved: [0; 32],
        }
    }
}

pub type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WndProc,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HICON,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}

impl Default for MSG {
    fn default() -> Self {
        MSG {
            hwnd: std::ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: POINT::default(),
        }
    }
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetLastError() -> u32;
    pub fn GetModuleHandleW(modname: *const u16) -> HINSTANCE;
}

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> u16;
    pub fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HINSTANCE,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;
    pub fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn SetWindowTextW(hWnd: HWND, lpString: *const u16) -> BOOL;
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
    pub fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
    pub fn SetCapture(hWnd: HWND) -> HWND;
    pub fn ReleaseCapture() -> BOOL;
    pub fn GetKeyState(nVirtKey: i32) -> i16;
    pub fn BeginPaint(hWnd: HWND, lpPs: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hWnd: HWND, lpPs: *const PAINTSTRUCT) -> BOOL;
    pub fn FillRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    pub fn GetDC(hWnd: HWND) -> HDC;
    pub fn ReleaseDC(hWnd: HWND, hdc: HDC) -> i32;
    pub fn SetProcessDPIAware() -> BOOL;
    pub fn GetSystemMetrics(nIndex: i32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateSolidBrush(color: u32) -> HBRUSH;
    pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    pub fn GetStockObject(i: i32) -> HGDIOBJ;
    pub fn Polygon(hdc: HDC, punkte: *const POINT, n: i32) -> BOOL;
    pub fn Polyline(hdc: HDC, punkte: *const POINT, n: i32) -> BOOL;
    pub fn CreatePen(stil: i32, breite: i32, farbe: u32) -> HPEN;
    pub fn SetBkMode(hdc: HDC, iMode: i32) -> i32;
    pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
    pub fn TextOutW(hdc: HDC, x: i32, y: i32, s: *const u16, n: i32) -> BOOL;
    pub fn GetTextExtentPoint32W(hdc: HDC, s: *const u16, n: i32, groesse: *mut SIZE) -> BOOL;
    pub fn GetDeviceCaps(hdc: HDC, index: i32) -> i32;
    pub fn CreateFontW(
        hoehe: i32,
        breite: i32,
        escapement: i32,
        orientierung: i32,
        fett: i32,
        kursiv: u32,
        unterstrichen: u32,
        durchgestrichen: u32,
        zeichensatz: u32,
        ausgabe: u32,
        beschnitt: u32,
        qualitaet: u32,
        pitch: u32,
        name: *const u16,
    ) -> HFONT;
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn CreateDIBSection(
        hdc: HDC,
        pbmi: *const BITMAPINFOHEADER,
        usage: u32,
        ppvBits: *mut *mut c_void,
        section: HANDLE,
        offset: u32,
    ) -> HGDIOBJ;
    pub fn BitBlt(
        hdcDest: HDC,
        xDest: i32,
        yDest: i32,
        w: i32,
        h: i32,
        hdcSrc: HDC,
        xSrc: i32,
        ySrc: i32,
        rop: u32,
    ) -> BOOL;
    /// Wartet, bis GDI alle gesammelten Zeichenaufrufe ausgeführt hat — Pflicht,
    /// bevor die Bytes einer DIB-Section gelesen werden.
    pub fn GdiFlush() -> BOOL;
}

/// `PS_SOLID`
pub const PS_SOLID: i32 = 0;

/// `GetStockObject`: der leere Stift. Mit ihm füllt `Polygon`, ohne eine
/// Kontur zu ziehen.
pub const NULL_PEN: i32 = 8;

/// `TRANSPARENT` — Text ohne Kasten. (Hier stand bis zum 19.10.2026
/// `0x001F_0001`; der gültige Wert ist 1, `OPAQUE` ist 2.)
pub const TRANSPARENT: i32 = 1;

/// Schriftgewichte und Schriftwahl für [`CreateFontW`].
pub const FW_NORMAL: i32 = 400;
pub const FW_SEMIBOLD: i32 = 600;
pub const DEFAULT_CHARSET: u32 = 1;
pub const CLEARTYPE_QUALITY: u32 = 5;
pub const FF_SWISS: u32 = 0x20;

/// `DIB_RGB_COLORS`
pub const DIB_RGB_COLORS: u32 = 0;
/// `BI_RGB`: unkomprimiert.
pub const BI_RGB: u32 = 0;
/// `SRCCOPY`
pub const SRCCOPY: u32 = 0x00CC_0020;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

/// `COLORREF` ist `0x00BBGGRR`: Rot steht im **niedrigsten** Byte.
///
/// Bis zum 19.10.2026 stand hier `r << 16 | g << 8 | b`, also `0x00RRGGBB` —
/// jede Farbe im Fenster hatte Rot und Blau vertauscht. Im PNG fiel das nicht
/// auf, weil dort ein zweiter Tausch (BGRA als RGBA gelesen) den ersten
/// aufhob.
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// LPARAM enthält Mauskoordinaten (je 16 Bit mit Vorzeichen).
///
/// Bei `WM_MOUSEWHEEL` sind es **Bildschirm**koordinaten, sonst
/// Clientkoordinaten.
pub fn lparam_x(l: LPARAM) -> i32 {
    (l & 0xFFFF) as i16 as i32
}

pub fn lparam_y(l: LPARAM) -> i32 {
    ((l >> 16) & 0xFFFF) as i16 as i32
}

/// Raddrehung aus `WM_MOUSEWHEEL`, in Vielfachen von [`WHEEL_DELTA`].
///
/// Sie steht im **oberen** Wort von wParam (`GET_WHEEL_DELTA_WPARAM`); im
/// unteren stehen die gedrückten Tasten. Bis zum 19.10.2026 las der
/// Betrachter das untere Wort — ohne gedrückte Taste immer 0, das Rad
/// zoomte nie.
pub fn wheel_delta(w: WPARAM) -> i16 {
    ((w >> 16) & 0xFFFF) as u16 as i16
}

/// Zeichenkette als `*const u16` fuer Win32-Aufrufe (mit Endnull).
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_hat_rot_im_niedrigsten_byte() {
        assert_eq!(rgb(0x12, 0x34, 0x56), 0x0056_3412);
        assert_eq!(rgb(255, 0, 0), 0x0000_00FF);
        assert_eq!(rgb(0, 0, 255), 0x00FF_0000);
    }

    #[test]
    fn raddrehung_steht_im_oberen_wort() {
        // Rad eine Raste zurück (−120), dabei MK_SHIFT (0x0004) gedrückt.
        let w: WPARAM = ((0xFF88u32 as usize) << 16) | 0x0004;
        assert_eq!(wheel_delta(w), -120);
        // Eine Raste vor, keine Taste.
        assert_eq!(wheel_delta(120usize << 16), 120);
        // Nur Tasten, kein Rad: 0 — das war der alte Messwert.
        assert_eq!(wheel_delta(0x0008), 0);
    }

    #[test]
    fn mauskoordinaten_haben_ein_vorzeichen() {
        // x = −5, y = 300 (auf einem zweiten Bildschirm links).
        let l: LPARAM = ((300isize) << 16) | (0xFFFBisize);
        assert_eq!(lparam_x(l), -5);
        assert_eq!(lparam_y(l), 300);
    }
}
