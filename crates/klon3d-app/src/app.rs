//! The application window: state, actions, keyboard shortcuts, dialogs and the frame layout.
//!
//! [`App`] implements [`eframe::App`]. Every frame [`App::ui`] lays out, from the outside in:
//! menu bar and toolbar ([`crate::menue`]), status bar, the side panels ([`crate::panele`]) and the
//! 2D view ([`crate::ansicht2d`]) in the middle; dialogs float on top.
//!
//! Menu entries, toolbar buttons, keys and clicks do not change the state directly: they queue an
//! [`Aktion`], and [`App::ausfuehren`] runs the queue at the end of the frame. One place therefore
//! decides what a command does, whichever way it was given.
//!
//! The window is built without a native window in the tests: [`App::neu`] only needs a
//! [`eframe::CreationContext`] (kittest makes one), and [`App::muster_laden`] loads a pattern
//! without the file dialog.
//!
//! ## Persistence
//!
//! The theme (dark by default) and the last folder of the file dialog are stored through
//! eframe's storage ([`App::save`]) under [`SCHLUESSEL_THEMA`] and [`SCHLUESSEL_ORDNER`] and read
//! back in [`App::neu`]. The theme is applied every frame, not only at start: whoever builds the
//! context may set its own theme after the app was created (kittest does).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use eframe::egui;
use egui::{Key, KeyboardShortcut, Modifiers, Theme};

use crate::ansicht2d::{Ansicht2d, Klick};
use crate::dateiwahl::{passt, Antwort, Dateiwahl, Zweck};
use crate::muster::{delta4, Ladeauftrag, Muster, Nahtstand};

/// Storage key of the theme (`"dunkel"` or `"hell"`).
pub const SCHLUESSEL_THEMA: &str = "klon3d.thema";
/// Storage key of the last folder of the file dialog.
pub const SCHLUESSEL_ORDNER: &str = "klon3d.ordner";

/// Hover text of every entry that a later task brings.
pub const SPAETER: &str = "folgt in einer späteren Aufgabe";

/// Ctrl+O (Cmd+O on macOS).
pub const KUERZEL_OEFFNEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
/// Ctrl+Q (Cmd+Q on macOS).
pub const KUERZEL_BEENDEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);

/// How many frame intervals the frame rate is averaged over.
const BILDZEITEN: usize = 60;

/// A command, from a menu, a button, a key or a click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aktion {
    Oeffnen,
    MassdateiWaehlen,
    NahtlisteWaehlen,
    Beenden,
    Einpassen,
    LageUmschalten,
    /// `true` = laid out, `false` = construction coordinates.
    LageSetzen(bool),
    NaehteUmschalten,
    ThemaUmschalten,
    ThemaSetzen(Theme),
    NaechstesTeil,
    VorigesTeil,
    AuswahlAufheben,
    TeilWaehlen(usize),
    NahtWaehlen(usize),
    NahtlaengenZeigen,
    TastenkuerzelZeigen,
    UeberZeigen,
}

/// The help window shown, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hilfe {
    Tastenkuerzel,
    Ueber,
}

/// The application.
pub struct App {
    /// Dark or light; dark unless the storage says otherwise.
    pub thema: Theme,
    /// What was loaded last (successfully).
    pub auftrag: Option<Ladeauftrag>,
    pub muster: Option<Muster>,
    /// Selected piece (index into `muster.teile`).
    pub gewaehlt: Option<usize>,
    /// Seam selected in the seam list (index into the seam report).
    pub naht_gewaehlt: Option<usize>,
    pub ansicht: Ansicht2d,
    /// Search text of the left panel; filters pieces and seams.
    pub suche: String,
    /// The last message for the status bar, and whether it is an error.
    pub meldung: String,
    pub meldung_fehler: bool,
    /// Text of the error window while it is open.
    pub fehlerfenster: Option<String>,
    pub dateiwahl: Option<Dateiwahl>,
    pub hilfe: Option<Hilfe>,
    /// Folder the file dialog starts in.
    pub letzter_ordner: PathBuf,
    /// Set by "Beenden"; the window is asked to close.
    pub beenden_angefordert: bool,
    /// Scroll the seam list into view in the next frame.
    pub(crate) nahtliste_zeigen: bool,
    aktionen: Vec<Aktion>,
    bildzeiten: VecDeque<f64>,
    letzte_zeit: Option<f64>,
    gpu: Option<String>,
    titel: Option<String>,
}

impl App {
    /// Build the app; reads theme and last folder from the storage if there is one.
    pub fn neu(cc: &eframe::CreationContext<'_>) -> App {
        let app = App::mit_speicher(cc.storage);
        cc.egui_ctx.set_theme(app.thema);
        app
    }

    /// Build the app from an optional storage (no egui context needed).
    pub fn mit_speicher(speicher: Option<&dyn eframe::Storage>) -> App {
        let thema = match speicher
            .and_then(|s| s.get_string(SCHLUESSEL_THEMA))
            .as_deref()
        {
            Some("hell") => Theme::Light,
            _ => Theme::Dark,
        };
        let letzter_ordner = speicher
            .and_then(|s| s.get_string(SCHLUESSEL_ORDNER))
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        App {
            thema,
            auftrag: None,
            muster: None,
            gewaehlt: None,
            naht_gewaehlt: None,
            ansicht: Ansicht2d::default(),
            suche: String::new(),
            meldung: "Bereit – Datei › Muster öffnen… (Strg+O)".into(),
            meldung_fehler: false,
            fehlerfenster: None,
            dateiwahl: None,
            hilfe: None,
            letzter_ordner,
            beenden_angefordert: false,
            nahtliste_zeigen: false,
            aktionen: Vec::new(),
            bildzeiten: VecDeque::with_capacity(BILDZEITEN),
            letzte_zeit: None,
            gpu: None,
            titel: None,
        }
    }

    /// Load a pattern with the measurement file it names and the seam list next to it.
    /// Returns whether it worked; on failure the message is in the status bar and the error
    /// window, and the previous pattern stays.
    pub fn muster_laden(&mut self, pfad: impl AsRef<Path>) -> bool {
        self.laden(Ladeauftrag::neu(pfad.as_ref()))
    }

    /// Load with explicitly chosen files (see [`Ladeauftrag`]).
    pub fn laden(&mut self, auftrag: Ladeauftrag) -> bool {
        match Muster::laden(&auftrag) {
            Ok(m) => {
                let naehte = match &m.naehte {
                    Nahtstand::Geprueft { bericht, .. } => {
                        format!("{} Nähte", bericht.naehte.len())
                    }
                    Nahtstand::Keine { .. } => "keine Nahtliste".into(),
                    Nahtstand::Fehler { .. } => "Nahtliste fehlerhaft".into(),
                };
                self.melden(
                    format!(
                        "{} geladen: {} Teile, {naehte}",
                        m.dateiname(),
                        m.teile.len()
                    ),
                    false,
                );
                if let Some(o) = auftrag.muster.parent().filter(|o| o.is_dir()) {
                    self.letzter_ordner = o.to_path_buf();
                }
                self.titel = Some(format!("klon3d – {}", m.dateiname()));
                self.muster = Some(m);
                self.auftrag = Some(auftrag);
                self.gewaehlt = None;
                self.naht_gewaehlt = None;
                self.ansicht.einpassen();
                true
            }
            Err(e) => {
                self.fehler(e);
                false
            }
        }
    }

    /// Show `text` in the status bar.
    pub fn melden(&mut self, text: impl Into<String>, fehler: bool) {
        self.meldung = text.into();
        self.meldung_fehler = fehler;
    }

    /// Show an error in the status bar and in the error window.
    pub fn fehler(&mut self, text: impl Into<String>) {
        let t = text.into();
        self.fehlerfenster = Some(t.clone());
        self.melden(t, true);
    }

    /// Queue a command for the end of the frame.
    pub fn aktion(&mut self, a: Aktion) {
        self.aktionen.push(a);
    }

    /// Frames per second, averaged over the last frame intervals; `None` before two frames.
    pub fn bilder_pro_sekunde(&self) -> Option<f64> {
        let summe: f64 = self.bildzeiten.iter().sum();
        (self.bildzeiten.len() >= 2 && summe > 0.0).then(|| self.bildzeiten.len() as f64 / summe)
    }

    /// "GPU: <adapter> (<backend>)" or "GPU: –" without a wgpu renderer.
    pub fn gpu_text(&self) -> &str {
        self.gpu.as_deref().unwrap_or("GPU: –")
    }

    /// Run one command.
    pub fn ausfuehren(&mut self, a: Aktion, ctx: &egui::Context) {
        let teile = self.muster.as_ref().map_or(0, |m| m.teile.len());
        match a {
            Aktion::Oeffnen => {
                self.dateiwahl = Some(Dateiwahl::neu(Zweck::Muster, &self.letzter_ordner))
            }
            Aktion::MassdateiWaehlen | Aktion::NahtlisteWaehlen => {
                let Some(auftrag) = &self.auftrag else {
                    self.melden("Erst ein Muster öffnen", true);
                    return;
                };
                let zweck = if a == Aktion::MassdateiWaehlen {
                    Zweck::Massdatei
                } else {
                    Zweck::Nahtliste
                };
                let ordner = auftrag
                    .muster
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| self.letzter_ordner.clone());
                self.dateiwahl = Some(Dateiwahl::neu(zweck, &ordner));
            }
            Aktion::Beenden => {
                self.beenden_angefordert = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Aktion::Einpassen => self.ansicht.einpassen(),
            Aktion::LageUmschalten => self.ansicht.lage_umschalten(),
            Aktion::LageSetzen(aus) => {
                if self.ansicht.ausgelegt != aus {
                    self.ansicht.lage_umschalten();
                }
            }
            Aktion::NaehteUmschalten => self.ansicht.naehte_zeigen = !self.ansicht.naehte_zeigen,
            Aktion::ThemaUmschalten => {
                self.thema = match self.thema {
                    Theme::Dark => Theme::Light,
                    Theme::Light => Theme::Dark,
                };
            }
            Aktion::ThemaSetzen(t) => self.thema = t,
            Aktion::NaechstesTeil if teile > 0 => {
                self.gewaehlt = Some(self.gewaehlt.map_or(0, |g| (g + 1) % teile));
            }
            Aktion::VorigesTeil if teile > 0 => {
                self.gewaehlt = Some(self.gewaehlt.map_or(teile - 1, |g| (g + teile - 1) % teile));
            }
            Aktion::NaechstesTeil | Aktion::VorigesTeil => {}
            Aktion::AuswahlAufheben => {
                self.gewaehlt = None;
                self.naht_gewaehlt = None;
            }
            Aktion::TeilWaehlen(i) if i < teile => self.gewaehlt = Some(i),
            Aktion::TeilWaehlen(_) => {}
            Aktion::NahtWaehlen(n) => {
                self.naht_gewaehlt = if self.naht_gewaehlt == Some(n) {
                    None
                } else {
                    Some(n)
                };
            }
            Aktion::NahtlaengenZeigen => self.nahtliste_zeigen = true,
            Aktion::TastenkuerzelZeigen => self.hilfe = Some(Hilfe::Tastenkuerzel),
            Aktion::UeberZeigen => self.hilfe = Some(Hilfe::Ueber),
        }
        ctx.request_repaint();
    }

    /// A file chosen in the dialog or dropped onto the window.
    fn datei_annehmen(&mut self, zweck: Zweck, pfad: PathBuf) {
        match zweck {
            Zweck::Muster => {
                self.muster_laden(&pfad);
            }
            Zweck::Massdatei | Zweck::Nahtliste => {
                let Some(mut auftrag) = self.auftrag.clone() else {
                    self.melden("Erst ein Muster öffnen", true);
                    return;
                };
                if zweck == Zweck::Massdatei {
                    auftrag.massdatei = Some(pfad);
                } else {
                    auftrag.nahtliste = Some(pfad);
                }
                self.laden(auftrag);
            }
        }
    }

    /// Keyboard shortcuts. Letters and Tab only act while no text field has the keyboard and no
    /// dialog is open.
    fn tasten(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&KUERZEL_OEFFNEN)) {
            self.aktion(Aktion::Oeffnen);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KUERZEL_BEENDEN)) {
            self.aktion(Aktion::Beenden);
        }
        // Escape is only read, not consumed: egui closes menus and popups with it as well.
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.dateiwahl.is_some() {
                self.dateiwahl = None;
            } else if self.fehlerfenster.is_some() {
                self.fehlerfenster = None;
            } else if self.hilfe.is_some() {
                self.hilfe = None;
            } else {
                self.aktion(Aktion::AuswahlAufheben);
            }
        }
        if self.dateiwahl.is_some() || ctx.text_edit_focused() {
            return;
        }
        let (f, l, zurueck, vor) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::F),
                i.consume_key(Modifiers::NONE, Key::L),
                // Shift+Tab first: `consume_key` ignores extra Shift.
                i.consume_key(Modifiers::SHIFT, Key::Tab),
                i.consume_key(Modifiers::NONE, Key::Tab),
            )
        });
        if f {
            self.aktion(Aktion::Einpassen);
        }
        if l {
            self.aktion(Aktion::LageUmschalten);
        }
        if zurueck || vor {
            // Tab selects pieces here; egui must not also move the keyboard focus.
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
            self.aktion(if zurueck {
                Aktion::VorigesTeil
            } else {
                Aktion::NaechstesTeil
            });
        }
    }

    /// Files dropped onto the window: a pattern is opened, a measurement file or seam list is
    /// used for the open pattern.
    fn abgelegte_dateien(&mut self, ctx: &egui::Context) {
        let pfade: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        for p in pfade {
            let zweck = [Zweck::Muster, Zweck::Massdatei, Zweck::Nahtliste]
                .into_iter()
                .find(|z| passt(&p, z.endungen()));
            match zweck {
                Some(z) => self.datei_annehmen(z, p),
                None => self.fehler(format!(
                    "{}: kein Muster (.sm2d), keine Maßdatei (.smis) und keine Nahtliste (.naehte)",
                    p.display()
                )),
            }
        }
    }

    /// Frame intervals for the frame rate.
    fn zeit_messen(&mut self, ctx: &egui::Context) {
        let t = ctx.input(|i| i.time);
        if let Some(v) = self.letzte_zeit {
            let dt = t - v;
            if dt > 0.0 {
                if self.bildzeiten.len() == BILDZEITEN {
                    self.bildzeiten.pop_front();
                }
                self.bildzeiten.push_back(dt);
            }
        }
        self.letzte_zeit = Some(t);
    }

    /// Adapter name and backend of the wgpu renderer, once.
    fn gpu_erfassen(&mut self, frame: &eframe::Frame) {
        if self.gpu.is_some() {
            return;
        }
        if let Some(rs) = frame.wgpu_render_state() {
            let info = rs.adapter.get_info();
            let backend = match info.backend.to_str() {
                "vulkan" => "Vulkan",
                "metal" => "Metal",
                "dx12" => "DirectX 12",
                "gl" => "OpenGL",
                "webgpu" => "WebGPU",
                andere => andere,
            };
            self.gpu = Some(format!("GPU: {} ({backend})", info.name));
        }
    }

    /// The pattern with the largest seam deviation for the status bar.
    pub fn abweichung_text(&self) -> String {
        let Some(m) = &self.muster else {
            return "Nähte: –".into();
        };
        match (&m.naehte, m.groesste_abweichung()) {
            (_, Some(n)) => format!("größte Abweichung {} Δ {} cm", n.kennung, delta4(n.diff_cm)),
            (Nahtstand::Keine { .. }, None) => "keine Nahtliste".into(),
            (Nahtstand::Fehler { .. }, None) => "Nahtliste fehlerhaft".into(),
            (Nahtstand::Geprueft { .. }, None) => "Nahtliste leer".into(),
        }
    }

    fn fenster(&mut self, ctx: &egui::Context) {
        if let Some(d) = &mut self.dateiwahl {
            let zweck = d.zweck;
            match d.ui(ctx) {
                Some(Antwort::Gewaehlt(p)) => {
                    if let Some(o) = p.parent().filter(|o| o.is_dir()) {
                        self.letzter_ordner = o.to_path_buf();
                    }
                    self.dateiwahl = None;
                    self.datei_annehmen(zweck, p);
                }
                Some(Antwort::Abbruch) => self.dateiwahl = None,
                None => {}
            }
        }
        if let Some(text) = self.fehlerfenster.clone() {
            let mut zu = false;
            egui::Window::new("Fehler")
                .id(egui::Id::unique("fehlerfenster"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(520.0);
                    ui.colored_label(ui.visuals().error_fg_color, &text);
                    ui.add_space(6.0);
                    zu = ui.button("Fehler schließen").clicked();
                });
            if zu {
                self.fehlerfenster = None;
            }
        }
        if let Some(h) = self.hilfe {
            let mut zu = false;
            let titel = match h {
                Hilfe::Tastenkuerzel => "Tastenkürzel",
                Hilfe::Ueber => "Über klon3d",
            };
            egui::Window::new(titel)
                .id(egui::Id::unique("hilfefenster"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    match h {
                        Hilfe::Tastenkuerzel => tastenkuerzel(ui),
                        Hilfe::Ueber => ueber(ui),
                    }
                    ui.add_space(6.0);
                    zu = ui.button("Hilfe schließen").clicked();
                });
            if zu {
                self.hilfe = None;
            }
        }
    }

    /// Paint a hint while files are dragged over the window.
    fn ablage_hinweis(&self, ctx: &egui::Context) {
        let namen: Vec<String> = ctx.input(|i| {
            i.raw
                .hovered_files
                .iter()
                .filter_map(|f| f.path.as_ref())
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .collect()
        });
        if ctx.input(|i| i.raw.hovered_files.is_empty()) {
            return;
        }
        let maler = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::unique("ablage"),
        ));
        let r = ctx.content_rect();
        maler.rect_filled(r, 0.0, egui::Color32::from_black_alpha(160));
        maler.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            format!("Loslassen zum Öffnen\n{}", namen.join("\n")),
            egui::FontId::proportional(20.0),
            egui::Color32::WHITE,
        );
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.zeit_messen(&ctx);
        self.gpu_erfassen(frame);
        if ctx.theme() != self.thema {
            ctx.set_theme(self.thema);
        }
        if let Some(t) = self.titel.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(t));
        }
        self.abgelegte_dateien(&ctx);
        self.tasten(&ctx);

        egui::Panel::top("menueleiste").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| crate::menue::menues(self, ui));
        });
        egui::Panel::top("werkzeugleiste").show(ui, |ui| crate::menue::werkzeugleiste(self, ui));
        egui::Panel::bottom("statusleiste").show(ui, |ui| crate::panele::statusleiste(self, ui));
        egui::Panel::left("teile_und_naehte")
            .default_size(380.0)
            .min_size(220.0)
            .show(ui, |ui| crate::panele::links(self, ui));
        egui::Panel::right("eigenschaften")
            .default_size(320.0)
            .min_size(200.0)
            .show(ui, |ui| crate::panele::rechts(self, ui));
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let klick =
                self.ansicht
                    .ui(ui, self.muster.as_ref(), self.gewaehlt, self.naht_gewaehlt);
            match klick {
                Some(Klick::Teil(i)) => self.aktion(Aktion::TeilWaehlen(i)),
                Some(Klick::Leer) => self.aktion(Aktion::AuswahlAufheben),
                None => {}
            }
        });
        self.fenster(&ctx);
        self.ablage_hinweis(&ctx);

        for a in std::mem::take(&mut self.aktionen) {
            self.ausfuehren(a, &ctx);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let thema = match self.thema {
            Theme::Dark => "dunkel",
            Theme::Light => "hell",
        };
        storage.set_string(SCHLUESSEL_THEMA, thema.into());
        storage.set_string(SCHLUESSEL_ORDNER, self.letzter_ordner.display().to_string());
    }
}

/// The table of the help window "Tastenkürzel".
fn tastenkuerzel(ui: &mut egui::Ui) {
    egui::Grid::new("tastenkuerzel")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for (taste, wirkung) in [
                ("Strg+O", "Muster öffnen"),
                ("Strg+Q", "Beenden"),
                ("F", "alles einpassen"),
                ("L", "Auslage / Konstruktionslage"),
                ("Tab", "nächstes Teil wählen"),
                ("Umschalt+Tab", "voriges Teil wählen"),
                ("Esc", "Auswahl aufheben, Dialog schließen"),
                ("Mausrad", "Zoom um den Mauszeiger"),
                ("Ziehen", "Ansicht verschieben"),
                ("Klick", "Teil wählen"),
            ] {
                ui.strong(taste);
                ui.label(wirkung);
                ui.end_row();
            }
        });
    ui.weak("Buchstaben und Tab wirken nicht, solange ein Textfeld den Fokus hat.");
}

/// The text of the help window "Über klon3d".
fn ueber(ui: &mut egui::Ui) {
    ui.heading(format!("klon3d {}", env!("CARGO_PKG_VERSION")));
    ui.label("3D-Simulator für Latex- und Textilkleidung.");
    ui.label(
        "Dieses Fenster (Stufe 6) zeigt Seamly2D-Muster und prüft Nahtlängen mit denselben Zahlen \
         wie „klon3d naehte“. 3D-Ansicht, Simulation, Material und Export folgen.",
    );
    ui.label("Lizenz: MIT OR Apache-2.0");
}
