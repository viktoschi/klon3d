//! Menu bar and toolbar.
//!
//! Entries only queue an [`Aktion`]; the app runs it at the end of the frame. Entries that a later
//! task brings (body, export, simulation, 3D, maps, mesh check) are shown disabled with the hover
//! text [`SPAETER`], so the menu already has its final shape.

use eframe::egui;
use egui::{Button, Theme};

use crate::app::{Aktion, App, SPAETER};
use crate::muster::dateiname;

/// Hover text of an entry that needs a pattern.
const ERST_MUSTER: &str = "erst ein Muster öffnen";

/// A menu entry that queues `aktion` when clicked; `kuerzel` is shown on the right.
fn eintrag(ui: &mut egui::Ui, app: &mut App, text: &str, kuerzel: &str, aktion: Aktion) {
    let mut knopf = Button::new(text);
    if !kuerzel.is_empty() {
        knopf = knopf.shortcut_text(kuerzel);
    }
    if ui.add(knopf).clicked() {
        app.aktion(aktion);
    }
}

/// A menu entry that is enabled only with a loaded pattern.
fn eintrag_mit_muster(ui: &mut egui::Ui, app: &mut App, text: &str, kuerzel: &str, aktion: Aktion) {
    let da = app.muster.is_some();
    let mut knopf = Button::new(text);
    if !kuerzel.is_empty() {
        knopf = knopf.shortcut_text(kuerzel);
    }
    if ui
        .add_enabled(da, knopf)
        .on_disabled_hover_text(ERST_MUSTER)
        .clicked()
    {
        app.aktion(aktion);
    }
}

/// A menu entry that chooses a file for [`App::ladeziel`] – the pattern shown, or one that failed
/// for want of a measurement file; the hover text names it.
fn eintrag_fuer_ziel(ui: &mut egui::Ui, app: &mut App, text: &str, aktion: Aktion) {
    let ziel = app.ladeziel().map(|a| dateiname(&a.muster));
    let r = ui
        .add_enabled(ziel.is_some(), Button::new(text))
        .on_disabled_hover_text(ERST_MUSTER);
    let r = match &ziel {
        Some(z) => r.on_hover_text(format!("für {z}")),
        None => r,
    };
    if r.clicked() {
        app.aktion(aktion);
    }
}

/// A disabled entry of a later task.
fn spaeter(ui: &mut egui::Ui, text: &str) {
    ui.add_enabled(false, Button::new(text))
        .on_disabled_hover_text(SPAETER);
}

/// A toggle entry: shown selected while `an`.
fn schalter(ui: &mut egui::Ui, app: &mut App, text: &str, kuerzel: &str, an: bool, aktion: Aktion) {
    let mut knopf = Button::new(text).selected(an);
    if !kuerzel.is_empty() {
        knopf = knopf.shortcut_text(kuerzel);
    }
    if ui.add(knopf).clicked() {
        app.aktion(aktion);
    }
}

/// The menus Datei, Simulation, Ansicht, Prüfen, Hilfe.
pub fn menues(app: &mut App, ui: &mut egui::Ui) {
    ui.menu_button("Datei", |ui| {
        eintrag(ui, app, "Muster öffnen…", "Strg+O", Aktion::Oeffnen);
        eintrag_fuer_ziel(ui, app, "Maßdatei wählen…", Aktion::MassdateiWaehlen);
        eintrag_fuer_ziel(ui, app, "Nahtliste wählen…", Aktion::NahtlisteWaehlen);
        ui.separator();
        spaeter(ui, "Körper…");
        spaeter(ui, "Export OBJ…");
        spaeter(ui, "Export DXF…");
        ui.separator();
        eintrag(ui, app, "Beenden", "Strg+Q", Aktion::Beenden);
    });
    ui.menu_button("Simulation", |ui| {
        for text in [
            "Start",
            "Pause",
            "Einzelschritt",
            "Zurücksetzen",
            "Material…",
        ] {
            spaeter(ui, text);
        }
    });
    ui.menu_button("Ansicht", |ui| {
        // Only the 2D view exists so far; it is always the active one.
        ui.add(Button::new("2D-Muster").selected(true))
            .on_hover_text("die Teile des Musters in der Ebene");
        spaeter(ui, "3D");
        spaeter(ui, "Netz");
        let naehte = app.ansicht.naehte_zeigen;
        schalter(ui, app, "Nähte", "", naehte, Aktion::NaehteUmschalten);
        spaeter(ui, "Dehnungskarte");
        spaeter(ui, "Abstandskarte");
        ui.separator();
        let aus = app.ansicht.ausgelegt;
        schalter(ui, app, "Auslage", "L", aus, Aktion::LageSetzen(true));
        schalter(
            ui,
            app,
            "Konstruktionslage",
            "L",
            !aus,
            Aktion::LageSetzen(false),
        );
        eintrag_mit_muster(ui, app, "Einpassen", "F", Aktion::Einpassen);
        ui.separator();
        let dunkel = app.thema == Theme::Dark;
        schalter(
            ui,
            app,
            "Dunkel",
            "",
            dunkel,
            Aktion::ThemaSetzen(Theme::Dark),
        );
        schalter(
            ui,
            app,
            "Hell",
            "",
            !dunkel,
            Aktion::ThemaSetzen(Theme::Light),
        );
    });
    ui.menu_button("Prüfen", |ui| {
        eintrag_mit_muster(ui, app, "Nahtlängen", "", Aktion::NahtlaengenZeigen);
        spaeter(ui, "Netzprüfung");
    });
    ui.menu_button("Hilfe", |ui| {
        eintrag(ui, app, "Tastenkürzel", "", Aktion::TastenkuerzelZeigen);
        eintrag(ui, app, "Über klon3d", "", Aktion::UeberZeigen);
    });
}

/// The toolbar below the menu bar; hover texts name the shortcut.
pub fn werkzeugleiste(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui
            .button("Öffnen")
            .on_hover_text("Muster öffnen (Strg+O)")
            .clicked()
        {
            app.aktion(Aktion::Oeffnen);
        }
        ui.separator();
        if ui
            .add_enabled(app.muster.is_some(), Button::new("Einpassen"))
            .on_hover_text("alles einpassen (F)")
            .on_disabled_hover_text(format!("alles einpassen (F) – {ERST_MUSTER}"))
            .clicked()
        {
            app.aktion(Aktion::Einpassen);
        }
        let aus = app.ansicht.ausgelegt;
        if ui
            .add(Button::selectable(aus, "Auslage"))
            .on_hover_text("Teile nebeneinander ausgelegt (L)")
            .clicked()
        {
            app.aktion(Aktion::LageSetzen(true));
        }
        if ui
            .add(Button::selectable(!aus, "Konstruktion"))
            .on_hover_text("Teile in der Konstruktionslage (L)")
            .clicked()
        {
            app.aktion(Aktion::LageSetzen(false));
        }
        ui.separator();
        ui.add(Button::selectable(true, "2D"))
            .on_hover_text("2D-Ansicht des Musters");
        ui.add_enabled(false, Button::selectable(false, "3D"))
            .on_disabled_hover_text(SPAETER);
        ui.separator();
        let ziel = match app.thema {
            Theme::Dark => "helles",
            Theme::Light => "dunkles",
        };
        if ui
            .button("Hell/Dunkel")
            .on_hover_text(format!("zum {ziel} Design wechseln"))
            .clicked()
        {
            app.aktion(Aktion::ThemaUmschalten);
        }
    });
}
