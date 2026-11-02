//! Side panels and status bar.
//!
//! * Left: search field, the measurement file in use, piece list (short name, circumference,
//!   nodes) and seam list (Kennung, Bezeichnung, L_A, L_B, Δ in cm with four decimals as
//!   `klon3d naehte` prints them), below it the doubly used edges and the pieces whose boundary
//!   cannot be resolved.
//! * Right: properties of the selected piece with its boundary edges.
//! * Bottom: frame rate, GPU, largest seam deviation, last message.
//!
//! Conspicuous seams (|Δ| > `TOLERANZ_CM` or a side without length) use the theme's warning
//! colour, errors its error colour.

use eframe::egui;
use egui::RichText;
use klon3d_pattern::naehte::TOLERANZ_CM;

use crate::app::{Aktion, App};
use crate::muster::{cm4, delta4, naht_auffaellig, naht_vermerk, Muster, Nahtstand, Teil};

/// Longest Bezeichnung shown in the seam list; the full text is in the hover text.
const BEZEICHNUNG_MAX: usize = 34;

/// Does `text` contain the search text (case-insensitive)? An empty search matches everything.
pub fn passt_zur_suche(suche: &str, texte: &[&str]) -> bool {
    let s = suche.trim().to_lowercase();
    s.is_empty() || texte.iter().any(|t| t.to_lowercase().contains(&s))
}

/// `text` cut to `max` characters with "…".
pub fn gekuerzt(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

/// Left panel: search, pieces, seams.
pub fn links(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let l = ui.label("Suchen:");
        ui.add(
            egui::TextEdit::singleline(&mut app.suche)
                .hint_text("Teil oder Naht")
                .desired_width(f32::INFINITY),
        )
        .labelled_by(l.id);
    });
    ui.separator();
    let Some(m) = app.muster.take() else {
        ui.weak("Kein Muster geladen.");
        return;
    };
    // Every length shown depends on the body measurements: name the file they come from.
    let herkunft = if m.massdatei_von_hand {
        "von Hand gewählt"
    } else {
        "vom Muster genannt"
    };
    ui.weak(format!("Maße: {} · {herkunft}", m.massdatei_name()))
        .on_hover_text(m.massdatei.display().to_string());
    // Both directions: the seam grid is wider than a narrow panel and must not widen it.
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            teileliste(app, &m, ui);
            ui.add_space(10.0);
            nahtliste(app, &m, ui);
        });
    app.muster = Some(m);
}

fn teileliste(app: &mut App, m: &Muster, ui: &mut egui::Ui) {
    ui.heading(format!("Teile ({})", m.teile.len()));
    egui::Grid::new("teileliste")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Teil");
            ui.strong("Umfang cm");
            ui.strong("Knoten");
            ui.end_row();
            for (i, t) in m.teile.iter().enumerate() {
                if !passt_zur_suche(&app.suche, &[&t.name]) {
                    continue;
                }
                let r = ui
                    .selectable_label(app.gewaehlt == Some(i), &t.kurz)
                    .on_hover_text(&t.name);
                if r.clicked() {
                    app.aktion(Aktion::TeilWaehlen(i));
                }
                umfang(ui, t, "");
                ui.label(t.knoten.to_string());
                ui.end_row();
            }
        });
    if !m.ohne_umriss.is_empty() {
        ui.weak(format!("ohne Nahtlinie: {}", m.ohne_umriss.join(", ")));
    }
}

fn nahtliste(app: &mut App, m: &Muster, ui: &mut egui::Ui) {
    let kopf = ui.heading("Nähte");
    if app.nahtliste_zeigen {
        kopf.scroll_to_me(Some(egui::Align::TOP));
        kopf.highlight();
        app.nahtliste_zeigen = false;
    }
    let warn = ui.visuals().warn_fg_color;
    let rot = ui.visuals().error_fg_color;
    let bericht = match &m.naehte {
        Nahtstand::Keine { gesucht } => {
            ui.label(format!(
                "Keine Nahtliste: {} fehlt. Datei › Nahtliste wählen… nimmt eine andere.",
                gesucht.display()
            ));
            return;
        }
        Nahtstand::Fehler { text, .. } => {
            ui.colored_label(rot, format!("Nahtliste nicht prüfbar: {text}"));
            return;
        }
        Nahtstand::Geprueft { pfad, bericht } => {
            ui.weak(format!(
                "{} · Längen in cm, Kurven nach Seamly2D",
                pfad.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| pfad.display().to_string())
            ))
            .on_hover_text(pfad.display().to_string());
            bericht
        }
    };
    egui::Grid::new("nahtliste")
        .num_columns(5)
        .striped(true)
        .show(ui, |ui| {
            for kopf in ["Naht", "Bezeichnung", "L_A", "L_B", "Δ"] {
                ui.strong(kopf);
            }
            ui.end_row();
            for (ni, n) in bericht.naehte.iter().enumerate() {
                if !passt_zur_suche(&app.suche, &[&n.kennung, &n.name]) {
                    continue;
                }
                let auff = naht_auffaellig(n);
                let kennung = if auff {
                    RichText::new(&n.kennung).color(warn)
                } else {
                    RichText::new(&n.kennung)
                };
                let r = ui
                    .selectable_label(app.naht_gewaehlt == Some(ni), kennung)
                    .on_hover_text("Naht in der Ansicht hervorheben");
                if r.clicked() {
                    app.aktion(Aktion::NahtWaehlen(ni));
                }
                ui.label(gekuerzt(&n.name, BEZEICHNUNG_MAX))
                    .on_hover_text(&n.name);
                ui.label(cm4(n.a.laenge_cm));
                ui.label(cm4(n.b.laenge_cm));
                let d = delta4(n.diff_cm);
                if auff {
                    ui.colored_label(warn, d).on_hover_text(naht_vermerk(n));
                } else {
                    ui.label(d);
                }
                ui.end_row();
            }
        });

    let ueber = bericht.ueber(TOLERANZ_CM).len();
    let ok = bericht.bestanden(TOLERANZ_CM);
    ui.add_space(4.0);
    let zusammenfassung = format!(
        "{} von {} Nähten in der Toleranz |Δ| ≤ {TOLERANZ_CM} cm – {}",
        bericht.naehte.len() - ueber,
        bericht.naehte.len(),
        if ok { "bestanden" } else { "nicht bestanden" }
    );
    if ok {
        ui.label(zusammenfassung);
    } else {
        ui.colored_label(warn, zusammenfassung);
    }

    ui.add_space(4.0);
    if bericht.doppelt.is_empty() {
        ui.weak("keine Kante in zwei Nähten");
    } else {
        ui.colored_label(rot, "Kanten in zwei Nähten:");
        for h in &bericht.doppelt {
            ui.colored_label(
                rot,
                format!(
                    "{}: {} › {} in {}",
                    h.teil,
                    h.von,
                    h.bis,
                    h.naehte.join(", ")
                ),
            );
        }
    }
    for f in &bericht.teile_fehler {
        ui.colored_label(rot, format!("nicht auflösbar: {f}"));
    }
    let hinweise = bericht.zugabe_ohne_naht.len() + bericht.genaeht_ohne_zugabe.len();
    if hinweise > 0 {
        egui::CollapsingHeader::new(format!("Hinweise ({hinweise})"))
            .id_salt("nahthinweise")
            .show(ui, |ui| {
                for h in &bericht.zugabe_ohne_naht {
                    ui.label(format!(
                        "Zugabe ohne Naht: {}: {} › {} ({})",
                        h.teil,
                        h.von,
                        h.bis,
                        cm4(h.laenge_cm)
                    ));
                }
                for h in &bericht.genaeht_ohne_zugabe {
                    ui.label(format!(
                        "genäht ohne Zugabe: {}: {} › {} in {}",
                        h.teil,
                        h.von,
                        h.bis,
                        h.naehte.join(", ")
                    ));
                }
            });
    }
}

/// The circumference of a piece in cm (followed by `einheit`), or a "–" in the error colour when
/// its boundary cannot be resolved – a length across a node that cannot be evaluated would be
/// wrong (`klon3d naehte` prints none either). The reason is in the hover text.
fn umfang(ui: &mut egui::Ui, t: &Teil, einheit: &str) {
    match (t.umfang_cm, &t.rand) {
        (Some(u), _) => {
            ui.label(format!("{}{einheit}", cm4(u)));
        }
        (None, rand) => {
            let grund = rand.as_ref().err().map_or("", String::as_str);
            ui.colored_label(ui.visuals().error_fg_color, "–")
                .on_hover_text(format!("Rand nicht auflösbar: {grund}"));
        }
    }
}

/// Right panel: properties of the selected piece.
pub fn rechts(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.heading("Eigenschaften");
    ui.separator();
    let Some(m) = &app.muster else {
        ui.weak("Kein Muster geladen.");
        return;
    };
    let Some(t) = app.gewaehlt.and_then(|i| m.teile.get(i)) else {
        ui.weak("Kein Teil gewählt – in der Liste oder in der Ansicht anklicken, Tab wählt das nächste.");
        return;
    };
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.strong(&t.kurz);
            egui::Grid::new("eigenschaften")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.label(&t.name);
                    ui.end_row();
                    ui.label("Umfang");
                    umfang(ui, t, " cm");
                    ui.end_row();
                    ui.label("Knoten");
                    ui.label(t.knoten.to_string());
                    ui.end_row();
                    ui.label("Einheit");
                    ui.label(&m.einheit);
                    ui.end_row();
                });
            ui.add_space(6.0);
            let rand = match &t.rand {
                Ok(r) => r,
                Err(e) => {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("Rand nicht auflösbar: {e}"),
                    );
                    return;
                }
            };
            ui.strong(format!("Randkanten ({})", rand.kanten.len()));
            let bericht = m.naehte.bericht();
            egui::Grid::new("randkanten")
                .num_columns(4)
                .striped(true)
                .show(ui, |ui| {
                    for kopf in ["Kante", "Länge cm", "Zugabe cm", "Naht"] {
                        ui.strong(kopf);
                    }
                    ui.end_row();
                    let zugabe = |z: Option<f64>| {
                        z.map_or("–".to_string(), |w| format!("{:.2}", rand.in_cm(w)))
                    };
                    for (k, kante) in rand.kanten.iter().enumerate() {
                        let (a, b) = (&rand.punktknoten[kante.von], &rand.punktknoten[kante.bis]);
                        let stern = |kerbe: bool| if kerbe { "*" } else { "" };
                        ui.label(format!(
                            "{}{} › {}{}",
                            a.name,
                            stern(a.kerbe),
                            b.name,
                            stern(b.kerbe)
                        ));
                        ui.label(cm4(rand.in_cm(kante.laenge)));
                        let (za, zb) = rand.zugabe(k);
                        ui.label(format!("{} / {}", zugabe(za), zugabe(zb)));
                        let naehte: Vec<&str> = t
                            .kanten_naehte
                            .get(k)
                            .into_iter()
                            .flatten()
                            .filter_map(|&n| bericht.and_then(|b| b.naehte.get(n)))
                            .map(|n| n.kennung.as_str())
                            .collect();
                        let mut text = naehte.join(", ");
                        if rand.ist_frei(k) {
                            if !text.is_empty() {
                                text.push_str(" · ");
                            }
                            text.push_str("frei");
                        }
                        ui.label(text);
                        ui.end_row();
                    }
                });
            ui.weak("* Passzeichen · Zugabe nach dem Anfang / vor dem Ende · frei: Zugabe 0");
        });
}

/// Bottom panel: frame rate, GPU, largest deviation, message.
pub fn statusleiste(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let fps = app
            .bilder_pro_sekunde()
            .map_or("– Bilder/s".to_string(), |f| format!("{f:.0} Bilder/s"));
        ui.label(fps).on_hover_text(
            "Mittel über die letzten Bildwechsel; ohne Bewegung zeichnet egui nicht neu",
        );
        ui.separator();
        ui.label(app.gpu_text());
        ui.separator();
        let abw = app.abweichung_text();
        let auff = app
            .muster
            .as_ref()
            .and_then(|m| m.groesste_abweichung())
            .is_some_and(naht_auffaellig);
        if auff {
            ui.colored_label(ui.visuals().warn_fg_color, abw);
        } else {
            ui.label(abw);
        }
        ui.separator();
        // Messages carry whole paths: cut at the window edge with "…", the full text on hover;
        // a click shows an error again in the error window.
        let mut text = RichText::new(&app.meldung);
        if app.meldung_fehler {
            text = text.color(ui.visuals().error_fg_color);
        }
        let r = ui.add(
            egui::Label::new(text)
                .truncate()
                .sense(egui::Sense::click()),
        );
        if app.meldung_fehler
            && r.on_hover_text("Klicken zeigt die Meldung im Fehlerfenster")
                .clicked()
        {
            app.aktion(Aktion::FehlerZeigen);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suche_ohne_gross_klein() {
        assert!(passt_zur_suche("", &["x"]));
        assert!(passt_zur_suche("  ", &[]));
        assert!(passt_zur_suche("cup", &["Bluse 3 - Untercup Links"]));
        assert!(passt_zur_suche("N07", &["n07l", "egal"]));
        assert!(!passt_zur_suche("Kragen", &["Band", "Ärmel"]));
    }

    #[test]
    fn kuerzen_nach_zeichen() {
        assert_eq!(gekuerzt("Ärmel", 5), "Ärmel");
        assert_eq!(gekuerzt("Ärmelnaht", 5), "Ärme…");
        assert_eq!(gekuerzt("", 3), "");
    }
}
