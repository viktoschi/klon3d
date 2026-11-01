//! A small file dialog inside the window.
//!
//! No `rfd`: on Linux it pulls in GTK or a D-Bus portal stack, and a native dialog cannot be driven
//! by the headless UI tests. This one shows the current folder with its subfolders and the files
//! that match the filter (`.sm2d`, `.smis`, `.naehte`), goes up, and takes any path typed into the
//! path field – absolute or relative to the current folder, on Windows also a bare drive letter
//! (`D:`). The listing and the path handling are plain functions with unit tests; [`Dateiwahl::ui`]
//! only shows them.

use std::path::{Path, PathBuf};

use eframe::egui;

/// What the dialog chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zweck {
    Muster,
    Massdatei,
    Nahtliste,
}

impl Zweck {
    /// File extensions shown with the filter on (lower case, without the dot).
    pub fn endungen(self) -> &'static [&'static str] {
        match self {
            Zweck::Muster => &["sm2d"],
            Zweck::Massdatei => &["smis"],
            Zweck::Nahtliste => &["naehte"],
        }
    }

    /// Window title.
    pub fn titel(self) -> &'static str {
        match self {
            Zweck::Muster => "Muster öffnen",
            Zweck::Massdatei => "Maßdatei wählen",
            Zweck::Nahtliste => "Nahtliste wählen",
        }
    }
}

/// One line of the listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eintrag {
    pub name: String,
    pub pfad: PathBuf,
    pub ordner: bool,
}

/// How the dialog ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Antwort {
    Gewaehlt(PathBuf),
    Abbruch,
}

/// Does the file name end in one of `endungen` (case-insensitive)?
pub fn passt(pfad: &Path, endungen: &[&str]) -> bool {
    pfad.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| endungen.iter().any(|w| e.eq_ignore_ascii_case(w)))
}

/// Subfolders first, then the files that match `endungen` (all files with `alle`), each sorted by
/// name without regard to case. Hidden entries (name starting with `.`) are left out.
pub fn auflisten(ordner: &Path, endungen: &[&str], alle: bool) -> Result<Vec<Eintrag>, String> {
    let lesen = std::fs::read_dir(ordner)
        .map_err(|e| format!("Ordner nicht lesbar: {}: {e}", ordner.display()))?;
    let mut ordner_liste = Vec::new();
    let mut dateien = Vec::new();
    for e in lesen.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let pfad = e.path();
        // `is_dir` follows links, so a link to a folder is a folder.
        if pfad.is_dir() {
            ordner_liste.push(Eintrag {
                name,
                pfad,
                ordner: true,
            });
        } else if alle || passt(&pfad, endungen) {
            dateien.push(Eintrag {
                name,
                pfad,
                ordner: false,
            });
        }
    }
    let nach_name = |a: &Eintrag, b: &Eintrag| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    };
    ordner_liste.sort_by(nach_name);
    dateien.sort_by(nach_name);
    ordner_liste.extend(dateien);
    Ok(ordner_liste)
}

/// The path meant by the text in the path field, seen from folder `basis`.
///
/// Quotes and surrounding blanks (as pasted from a file manager) are removed, `~` at the start
/// means the home folder, a relative path is taken relative to `basis`. On Windows a bare drive
/// letter `D:` means the root of that drive (`D:\`), not its current folder.
pub fn eingabe_aufloesen(text: &str, basis: &Path) -> PathBuf {
    let t = text.trim().trim_matches('"').trim();
    if t.is_empty() {
        return basis.to_path_buf();
    }
    if cfg!(windows) && ist_laufwerk(t) {
        return PathBuf::from(format!("{}\\", &t[..2]));
    }
    if let Some(rest) = t.strip_prefix('~') {
        if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') {
            if let Some(heim) = heimordner() {
                return heim.join(rest.trim_start_matches(['/', '\\']));
            }
        }
    }
    let p = Path::new(t);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        basis.join(p)
    }
}

/// `X:` or `X:\` / `X:/` – a drive letter on its own.
fn ist_laufwerk(t: &str) -> bool {
    let b = t.as_bytes();
    (b.len() == 2 || (b.len() == 3 && (b[2] == b'\\' || b[2] == b'/')))
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
}

fn heimordner() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// The roots one can jump to: on Windows every existing drive, elsewhere `/`.
pub fn laufwerke() -> Vec<PathBuf> {
    if cfg!(windows) {
        (b'A'..=b'Z')
            .map(|c| PathBuf::from(format!("{}:\\", c as char)))
            .filter(|p| p.is_dir())
            .collect()
    } else {
        vec![PathBuf::from("/")]
    }
}

/// Remove `.` and `..` components where that is possible without the file system, so that the
/// path field shows `/a/b` and not `/a/b/c/..`.
pub fn bereinigt(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                let kann_hoch = matches!(out.components().next_back(), Some(Component::Normal(_)));
                if kann_hoch {
                    out.pop();
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            andere => out.push(andere.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// The dialog's state while it is open.
#[derive(Debug, Clone)]
pub struct Dateiwahl {
    pub zweck: Zweck,
    /// The folder shown.
    pub ordner: PathBuf,
    /// Text of the path field.
    pub pfad_text: String,
    /// Show all files, not only the matching ones.
    pub alle: bool,
    pub eintraege: Vec<Eintrag>,
    /// Why the last action did not work.
    pub fehler: Option<String>,
    markiert: Option<PathBuf>,
}

impl Dateiwahl {
    /// Open the dialog in `ordner` (falls back to the current folder if it is not readable).
    pub fn neu(zweck: Zweck, ordner: &Path) -> Dateiwahl {
        let mut d = Dateiwahl {
            zweck,
            ordner: PathBuf::new(),
            pfad_text: String::new(),
            alle: false,
            eintraege: Vec::new(),
            fehler: None,
            markiert: None,
        };
        let start = if ordner.is_dir() {
            ordner.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        d.wechseln(&start);
        d
    }

    /// Show folder `ordner`; on an error the old folder stays and the error is shown.
    pub fn wechseln(&mut self, ordner: &Path) {
        let ziel = bereinigt(&std::path::absolute(ordner).unwrap_or_else(|_| ordner.to_path_buf()));
        match auflisten(&ziel, self.zweck.endungen(), self.alle) {
            Ok(e) => {
                self.eintraege = e;
                self.pfad_text = ziel.display().to_string();
                self.ordner = ziel;
                self.fehler = None;
                self.markiert = None;
            }
            Err(e) => self.fehler = Some(e),
        }
    }

    /// One folder up.
    pub fn hoch(&mut self) {
        if let Some(p) = self.ordner.parent().map(Path::to_path_buf) {
            self.wechseln(&p);
        }
    }

    /// Re-read the folder (e.g. after switching the filter).
    pub fn neu_lesen(&mut self) {
        let o = self.ordner.clone();
        self.wechseln(&o);
    }

    /// What the path field means: a folder is opened, a matching file is the answer.
    pub fn bestaetigen(&mut self) -> Option<Antwort> {
        let ziel = eingabe_aufloesen(&self.pfad_text, &self.ordner);
        if ziel.is_dir() {
            self.wechseln(&ziel);
            return None;
        }
        if !ziel.is_file() {
            self.fehler = Some(format!("Datei nicht gefunden: {}", ziel.display()));
            return None;
        }
        if !self.alle && !passt(&ziel, self.zweck.endungen()) {
            self.fehler = Some(format!(
                "{} ist keine {}-Datei (Filter ausschalten mit „alle Dateien“)",
                ziel.display(),
                self.filter_text()
            ));
            return None;
        }
        Some(Antwort::Gewaehlt(bereinigt(&ziel)))
    }

    fn filter_text(&self) -> String {
        self.zweck
            .endungen()
            .iter()
            .map(|e| format!(".{e}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Show the dialog as a window; returns the answer once the user chose or cancelled.
    pub fn ui(&mut self, ctx: &egui::Context) -> Option<Antwort> {
        let mut antwort = None;
        egui::Window::new(self.zweck.titel())
            .id(egui::Id::unique("dateiwahl"))
            .collapsible(false)
            .resizable(true)
            .default_size([620.0, 440.0])
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("Hoch")
                        .on_hover_text("übergeordneter Ordner")
                        .clicked()
                    {
                        self.hoch();
                    }
                    for lw in laufwerke() {
                        let name = lw.display().to_string();
                        if ui
                            .button(&name)
                            .on_hover_text(format!("zu {name} wechseln"))
                            .clicked()
                        {
                            self.wechseln(&lw);
                        }
                    }
                    if ui
                        .checkbox(&mut self.alle, "alle Dateien")
                        .on_hover_text(format!("aus: nur {}", self.filter_text()))
                        .changed()
                    {
                        self.neu_lesen();
                    }
                });
                ui.horizontal(|ui| {
                    let l = ui.label("Pfad:");
                    let feld = ui
                        .add(
                            egui::TextEdit::singleline(&mut self.pfad_text)
                                .desired_width(f32::INFINITY),
                        )
                        .labelled_by(l.id);
                    if feld.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        antwort = self.bestaetigen();
                    }
                });
                if let Some(f) = &self.fehler {
                    ui.colored_label(ui.visuals().error_fg_color, f);
                }
                ui.separator();
                let unten = 36.0;
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - unten).max(80.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.eintraege.is_empty() {
                            ui.weak(format!(
                                "keine Ordner und keine {}-Dateien",
                                self.filter_text()
                            ));
                        }
                        let mut wechsel = None;
                        for e in &self.eintraege {
                            let text = if e.ordner {
                                format!("{}{}", e.name, std::path::MAIN_SEPARATOR)
                            } else {
                                e.name.clone()
                            };
                            let gewaehlt = self.markiert.as_ref() == Some(&e.pfad);
                            let r = ui.selectable_label(gewaehlt, text);
                            if e.ordner && r.clicked() {
                                wechsel = Some(e.pfad.clone());
                            } else if !e.ordner && r.double_clicked() {
                                antwort = Some(Antwort::Gewaehlt(e.pfad.clone()));
                            } else if !e.ordner && r.clicked() {
                                self.markiert = Some(e.pfad.clone());
                                self.pfad_text = e.pfad.display().to_string();
                            }
                        }
                        if let Some(p) = wechsel {
                            self.wechseln(&p);
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    // Not just "Öffnen": the toolbar has a button of that name.
                    let titel = match self.zweck {
                        Zweck::Muster => "Datei öffnen",
                        _ => "Datei wählen",
                    };
                    if ui.button(titel).clicked() {
                        antwort = self.bestaetigen();
                    }
                    if ui.button("Abbrechen").clicked() {
                        antwort = Some(Antwort::Abbruch);
                    }
                    ui.weak(format!("Filter: {}", self.filter_text()));
                });
            });
        antwort
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder with a few files and subfolders.
    fn ordner(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "klon3d-app-dateiwahl-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("Zweig")).unwrap();
        std::fs::create_dir_all(d.join("ast")).unwrap();
        std::fs::create_dir_all(d.join(".versteckt")).unwrap();
        for f in [
            "b.sm2d",
            "A.SM2D",
            "c.smis",
            "d.naehte",
            "notiz.txt",
            ".geheim.sm2d",
        ] {
            std::fs::write(d.join(f), "x").unwrap();
        }
        d
    }

    fn namen(e: &[Eintrag]) -> Vec<&str> {
        e.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn ordner_zuerst_dann_passende_dateien() {
        let d = ordner("liste");
        let e = auflisten(&d, Zweck::Muster.endungen(), false).unwrap();
        assert_eq!(namen(&e), ["ast", "Zweig", "A.SM2D", "b.sm2d"]);
        assert!(e[0].ordner && !e[2].ordner);
        let e = auflisten(&d, Zweck::Nahtliste.endungen(), false).unwrap();
        assert_eq!(namen(&e), ["ast", "Zweig", "d.naehte"]);
        let e = auflisten(&d, &[], true).unwrap();
        assert_eq!(
            namen(&e),
            [
                "ast",
                "Zweig",
                "A.SM2D",
                "b.sm2d",
                "c.smis",
                "d.naehte",
                "notiz.txt"
            ]
        );
        assert!(auflisten(&d.join("gibt_es_nicht"), &["sm2d"], false).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn eingabe_relativ_absolut_und_heim() {
        let basis = std::env::temp_dir();
        assert_eq!(eingabe_aufloesen("", &basis), basis);
        assert_eq!(
            eingabe_aufloesen("  \"x.sm2d\" ", &basis),
            basis.join("x.sm2d")
        );
        let abs = std::env::temp_dir().join("y.sm2d");
        assert_eq!(
            eingabe_aufloesen(&abs.display().to_string(), Path::new("woanders")),
            abs
        );
        if let Some(h) = heimordner() {
            assert_eq!(eingabe_aufloesen("~", &basis), h);
            assert_eq!(eingabe_aufloesen("~/muster", &basis), h.join("muster"));
        }
    }

    #[test]
    fn laufwerksbuchstaben() {
        assert!(ist_laufwerk("D:") && ist_laufwerk("d:\\") && ist_laufwerk("C:/"));
        assert!(!ist_laufwerk("D:x") && !ist_laufwerk("1:") && !ist_laufwerk("D"));
        let basis = Path::new("basis");
        if cfg!(windows) {
            assert_eq!(eingabe_aufloesen("D:", basis), PathBuf::from("D:\\"));
            assert_eq!(eingabe_aufloesen("d:/", basis), PathBuf::from("d:\\"));
        } else {
            // Elsewhere `D:` is an ordinary (relative) name.
            assert_eq!(eingabe_aufloesen("D:", basis), basis.join("D:"));
            assert_eq!(laufwerke(), vec![PathBuf::from("/")]);
        }
    }

    #[test]
    fn punkte_im_pfad_bereinigen() {
        assert_eq!(
            bereinigt(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
        assert_eq!(bereinigt(Path::new("/..")), PathBuf::from("/"));
        assert_eq!(bereinigt(Path::new("../x")), PathBuf::from("../x"));
        assert_eq!(bereinigt(Path::new("a/..")), PathBuf::from("."));
    }

    #[test]
    fn dialog_wechselt_ordner_und_waehlt_datei() {
        let d = ordner("dialog");
        let mut w = Dateiwahl::neu(Zweck::Muster, &d);
        assert_eq!(w.ordner, bereinigt(&std::path::absolute(&d).unwrap()));
        assert_eq!(w.eintraege.len(), 4);

        // A folder in the path field is opened.
        w.pfad_text = "Zweig".into();
        assert_eq!(w.bestaetigen(), None);
        assert!(w.ordner.ends_with("Zweig"));
        assert!(w.eintraege.is_empty());
        w.hoch();
        assert_eq!(w.ordner, bereinigt(&std::path::absolute(&d).unwrap()));

        // A matching file is the answer, with any case of the extension.
        w.pfad_text = "A.SM2D".into();
        assert_eq!(
            w.bestaetigen(),
            Some(Antwort::Gewaehlt(w.ordner.join("A.SM2D")))
        );

        // A file that does not match the filter or does not exist gives a message.
        w.pfad_text = "notiz.txt".into();
        assert_eq!(w.bestaetigen(), None);
        assert!(w.fehler.as_deref().unwrap().contains("keine .sm2d-Datei"));
        w.alle = true;
        assert!(w.bestaetigen().is_some());
        w.pfad_text = "fehlt.sm2d".into();
        assert_eq!(w.bestaetigen(), None);
        assert!(w
            .fehler
            .as_deref()
            .unwrap()
            .starts_with("Datei nicht gefunden"));

        // An unreadable folder keeps the old one.
        let vorher = w.ordner.clone();
        w.wechseln(&d.join("gibt_es_nicht"));
        assert_eq!(w.ordner, vorher);
        assert!(w.fehler.is_some());
        let _ = std::fs::remove_dir_all(&d);
    }
}
