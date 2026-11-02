//! UI tests of the window arrangement (step 7b): menus and toolbars with their disabled entries,
//! showing and hiding the library, object browser, properties and the 3D and 2D windows, the
//! active window and Strg+Tab, the 2D toggles with their shortcuts, the recently opened patterns,
//! the library, the fabrics tab, "Informationen", seam properties, stored state of an older
//! version and resetting the arrangement. Headless with egui_kittest like `oberflaeche.rs`, with
//! the same independent evaluation through `klon3d-pattern`.

mod gemeinsam;

use std::path::PathBuf;

use eframe::egui;
use egui::{Key, Modifiers, Role, Theme};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_kittest::Harness;
use gemeinsam::*;
use klon3d_app::anordnung::{
    Anordnung, Anzeige2d, Bereich, Fenster, SCHLUESSEL_ANORDNUNG, SCHLUESSEL_ANZEIGE2D,
    SCHLUESSEL_ZULETZT,
};
use klon3d_app::ansicht2d::laenge_text;
use klon3d_app::app::{App, SCHLUESSEL_ORDNER, SCHLUESSEL_THEMA, SPAETER};
use klon3d_app::muster::Nahtstand;
use klon3d_pattern::geom::{lies_messungen, Auswertung};
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::naehte::{lies_nahtliste, pruefe_naehte};
use klon3d_pattern::read_path;

/// The rect of the area named `name` (AccessKit role "pane"), if it is shown.
fn flaeche(h: &Harness<'_, App>, name: &str) -> Option<egui::Rect> {
    h.query_by_role_and_label(Role::Pane, name)
        .map(|n| n.rect())
}

/// The five areas of the window in the order of Anzeige › Fenster.
const FLAECHEN: [(Bereich, &str); 5] = [
    (Bereich::Bibliothek, "Bibliothek"),
    (Bereich::Browser, "Objekt-Browser"),
    (Bereich::Eigenschaften, "Eigenschaften"),
    (Bereich::Fenster3d, "3D-Fenster"),
    (Bereich::Fenster2d, "2D-Fenster"),
];

/// A window like [`fenster`] whose test steps are 1/30 s: kittest feeds one event per step, and
/// with its default step (0.25 s) the two clicks of a double click lie 0.5 s apart – too slow for
/// egui (0.3 s).
fn schnelles_fenster() -> Harness<'static, App> {
    Harness::builder()
        .with_size([1600.0, 1400.0])
        .with_step_dt(1.0 / 30.0)
        .build_eframe(|cc| App::neu(cc))
}

/// Two quick primary clicks at `pos` – a double click in a [`schnelles_fenster`]. Some time
/// passes first, so that a click shortly before does not make it a triple click.
fn doppelklick_bei(h: &mut Harness<'_, App>, pos: egui::Pos2) {
    h.run_steps(20);
    h.hover_at(pos);
    for _ in 0..2 {
        for gedrueckt in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: gedrueckt,
                modifiers: Modifiers::NONE,
            });
        }
    }
    h.run();
}

/// Drag from `von` to `nach` with the primary button, over several frames.
fn ziehen(h: &mut Harness<'_, App>, von: egui::Pos2, nach: egui::Pos2) {
    h.hover_at(von);
    h.run();
    h.drag_at(von);
    h.run();
    for i in 1..=4 {
        h.hover_at(von + (nach - von) * (i as f32 / 4.0));
        h.run();
    }
    h.drop_at(nach);
    h.run();
}

/// Is a tooltip with a text containing `text` shown?
fn hinweis_da(h: &Harness<'_, App>, text: &str) -> bool {
    h.query_all_by_role(Role::Tooltip)
        .any(|t| t.query_by_label_contains(text).is_some())
}

#[test]
fn werkzeugleisten_mit_gesperrten_knoepfen() {
    let mut h = mit_bluse();
    h.ctx
        .global_style_mut(|s| s.interaction.tooltip_delay = 0.0);
    // The file toolbar and the 3D toolbar: later functions are disabled, their hover text says
    // so and names the shortcut where there is one.
    for (knopf, kuerzel) in [
        ("Neu", "Strg+N"),
        ("Speichern", "Strg+S"),
        ("Rückgängig", "Strg+Z"),
        ("Wiederholen", "Strg+Y"),
        ("Simulation", ""),
        ("Einzelschritt", ""),
        ("Zurücksetzen", ""),
        ("Avatar", "Umschalt+A"),
        ("Dehnungskarte", "Alt+7"),
        ("Passformkarte", "Alt+8"),
    ] {
        let k = h
            .query_all_by_role_and_label(Role::Button, knopf)
            .find(|n| !in_einem_menue(n))
            .unwrap_or_else(|| panic!("{knopf} in keiner Werkzeugleiste"));
        assert!(k.accesskit_node().is_disabled(), "{knopf} nicht gesperrt");
        k.hover();
        h.run();
        assert!(hinweis_da(&h, SPAETER), "{knopf}: kein Hinweis");
        assert!(hinweis_da(&h, kuerzel), "{knopf}: Kürzel {kuerzel} fehlt");
    }
    // The working buttons of the 2D toolbar.
    for knopf in [
        "Einpassen",
        "Auslage",
        "Konstruktion",
        "Nähte",
        "Längen",
        "Teilnamen",
    ] {
        let k = h.get_by_role_and_label(Role::Button, knopf);
        assert!(!k.accesskit_node().is_disabled(), "{knopf} gesperrt");
    }
    // Every area of the window is there.
    for (_, name) in FLAECHEN {
        assert!(flaeche(&h, name).is_some(), "{name} fehlt");
    }
}

#[test]
fn fenster_ein_und_ausblenden() {
    let mut h = mit_bluse();
    let (d3, d2) = (
        flaeche(&h, "3D-Fenster").unwrap(),
        flaeche(&h, "2D-Fenster").unwrap(),
    );
    // 3D left of 2D, side by side, about half and half.
    assert!(d3.right() <= d2.left() && (d3.top() - d2.top()).abs() < 1.0);
    assert!((d3.width() - d2.width()).abs() <= 2.0, "{d3:?} {d2:?}");
    let bib = flaeche(&h, "Bibliothek").unwrap();
    let browser = flaeche(&h, "Objekt-Browser").unwrap();
    let eigen = flaeche(&h, "Eigenschaften").unwrap();
    assert!(bib.right() <= d3.left() && d2.right() <= browser.left());
    assert!(browser.bottom() <= eigen.top() && (browser.left() - eigen.left()).abs() < 1.0);

    // Each area hides and comes back through Anzeige › Fenster.
    for (b, name) in FLAECHEN {
        menue_waehlen(&mut h, &["Anzeige", "Fenster"], name);
        assert!(!h.state().anordnung.sichtbar(b), "{name} noch an");
        assert!(flaeche(&h, name).is_none(), "{name} noch zu sehen");
        // The others stay.
        for (_, andere) in FLAECHEN.iter().filter(|(a, _)| *a != b) {
            assert!(flaeche(&h, andere).is_some(), "{andere} weg mit {name}");
        }
        match b {
            // The other window takes the whole middle.
            Bereich::Fenster3d => {
                let r = flaeche(&h, "2D-Fenster").unwrap();
                assert!((r.left() - d3.left()).abs() < 1.0 && (r.right() - d2.right()).abs() < 1.0);
            }
            Bereich::Fenster2d => {
                let r = flaeche(&h, "3D-Fenster").unwrap();
                assert!((r.left() - d3.left()).abs() < 1.0 && (r.right() - d2.right()).abs() < 1.0);
            }
            // The properties take the whole right side, and the other way round.
            Bereich::Browser => {
                let r = flaeche(&h, "Eigenschaften").unwrap();
                assert!(
                    (r.top() - browser.top()).abs() < 1.0 && r.bottom() >= eigen.bottom() - 1.0
                );
            }
            Bereich::Eigenschaften => {
                let r = flaeche(&h, "Objekt-Browser").unwrap();
                assert!(r.bottom() >= eigen.bottom() - 1.0);
            }
            // The middle grows to the left.
            Bereich::Bibliothek => {
                let r = flaeche(&h, "3D-Fenster").unwrap();
                assert!(r.left() < d3.left() - 100.0, "{r:?}");
            }
            _ => unreachable!(),
        }
        menue_waehlen(&mut h, &["Anzeige", "Fenster"], name);
        assert!(flaeche(&h, name).is_some(), "{name} nicht wieder da");
    }

    // Both windows hidden: a hint says how to bring them back.
    menue_waehlen(&mut h, &["Anzeige", "Fenster"], "3D-Fenster");
    menue_waehlen(&mut h, &["Anzeige", "Fenster"], "2D-Fenster");
    assert!(h.query_by_label("2D-Ansicht").is_none());
    assert!(h
        .query_by_label_contains("3D- und 2D-Fenster sind ausgeblendet")
        .is_some());
    menue_waehlen(&mut h, &["Anzeige", "Fenster"], "2D-Fenster");
    assert!(h.query_by_label("2D-Ansicht").is_some());

    // Toolbars: each one goes and comes back.
    for (leiste, knopf) in [
        ("Datei-Werkzeugleiste", "Hell/Dunkel"),
        ("3D-Werkzeugleiste", "Dehnungskarte"),
        ("2D-Werkzeugleiste", "Einpassen"),
    ] {
        if leiste == "3D-Werkzeugleiste" {
            menue_waehlen(&mut h, &["Anzeige", "Fenster"], "3D-Fenster");
        }
        assert!(h.query_by_role_and_label(Role::Button, knopf).is_some());
        menue_waehlen(&mut h, &["Anzeige", "Werkzeugleisten"], leiste);
        assert!(
            h.query_by_role_and_label(Role::Button, knopf).is_none(),
            "{knopf} mit {leiste} aus"
        );
        menue_waehlen(&mut h, &["Anzeige", "Werkzeugleisten"], leiste);
        assert!(h.query_by_role_and_label(Role::Button, knopf).is_some());
    }
}

#[test]
fn strg_tab_wechselt_das_aktive_fenster() {
    let mut h = mit_bluse();
    assert_eq!(h.state().aktiv, Fenster::Zwei, "2D ist am Anfang aktiv");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    h.run();
    assert_eq!(h.state().aktiv, Fenster::Drei);
    // Strg+Tab is not Tab: no piece was selected on the way.
    assert_eq!(h.state().gewaehlt, None);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    h.run();
    assert_eq!(h.state().aktiv, Fenster::Zwei);

    // A click into a window activates it; a click into the 2D view still selects nothing else.
    h.get_by_label("3D-Ansicht").click();
    h.run();
    assert_eq!(h.state().aktiv, Fenster::Drei);
    let mitte = h.get_by_label("2D-Ansicht").rect().min + egui::vec2(3.0, 3.0);
    klick_bei(&mut h, mitte);
    assert_eq!(h.state().aktiv, Fenster::Zwei);

    // Anzeige › Fenster › Fenster wechseln does the same.
    menue_waehlen(&mut h, &["Anzeige", "Fenster"], "Fenster wechseln Strg+Tab");
    assert_eq!(h.state().aktiv, Fenster::Drei);

    // Not while the search field has the keyboard.
    h.get_by_label("Suchen:").focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    h.run();
    assert_eq!(h.state().aktiv, Fenster::Drei, "Strg+Tab im Textfeld");
    h.key_press(Key::Escape);
    h.run();

    // Only between shown windows: hiding the active one passes it on, and Strg+Tab keeps it.
    menue_waehlen(&mut h, &["Anzeige", "Fenster"], "3D-Fenster");
    assert_eq!(h.state().aktiv, Fenster::Zwei);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    h.run();
    assert_eq!(h.state().aktiv, Fenster::Zwei);
    menue_oeffnen(&mut h, &["Anzeige", "Fenster"]);
    assert!(im_menue(&h, "Fenster wechseln Strg+Tab")
        .accesskit_node()
        .is_disabled());
}

#[test]
fn umschalt_s_und_z_schalten_naehte_und_laengen() {
    let mut h = mit_bluse();
    let a = h.state().ansicht.anzeige;
    assert!(a.naehte && !a.laengen, "Vorgabe: Nähte an, Längen aus");
    h.key_press_modifiers(Modifiers::SHIFT, Key::S);
    h.run();
    assert!(!h.state().ansicht.anzeige.naehte);
    h.key_press_modifiers(Modifiers::SHIFT, Key::Z);
    h.run();
    assert!(h.state().ansicht.anzeige.laengen);
    // The menu shows the state as checkboxes.
    menue_oeffnen(&mut h, &["Anzeige", "2D-Schnitt"]);
    let haken = |h: &Harness<'_, App>, e: &str| {
        im_menue(h, e).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
    };
    assert!(!haken(&h, "Nähte Umschalt+S") && haken(&h, "Längen Umschalt+Z"));
    menue_schliessen(&mut h, "Anzeige");

    // In the search field the keys are text, not shortcuts.
    h.get_by_label("Suchen:").focus();
    h.run();
    h.key_press_modifiers(Modifiers::SHIFT, Key::S);
    h.key_press_modifiers(Modifiers::SHIFT, Key::Z);
    h.run();
    let a = h.state().ansicht.anzeige;
    assert!(!a.naehte && a.laengen, "Umschalt+S/Z im Textfeld");
    h.key_press(Key::Escape);
    h.run();
    h.key_press_modifiers(Modifiers::SHIFT, Key::S);
    h.run();
    assert!(h.state().ansicht.anzeige.naehte);

    // The 2D toolbar toggles the same and the piece names too.
    h.get_by_role_and_label(Role::Button, "Längen").click();
    h.run();
    assert!(!h.state().ansicht.anzeige.laengen);
    h.get_by_role_and_label(Role::Button, "Teilnamen").click();
    h.run();
    assert!(!h.state().ansicht.anzeige.teilnamen);
    menue_waehlen(&mut h, &["Anzeige", "2D-Schnitt"], "Maßstab");
    assert!(!h.state().ansicht.anzeige.massstab);
}

/// The texts painted in the last frame that look like an edge length ("12,3").
fn gemalte_laengen(h: &Harness<'_, App>) -> Vec<String> {
    fn sammeln(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| sammeln(s, out)),
            egui::Shape::Text(t) => {
                let text = t.galley.text();
                let ist_laenge = text.split_once(',').is_some_and(|(a, b)| {
                    !a.is_empty()
                        && a.bytes().all(|c| c.is_ascii_digit())
                        && b.len() == 1
                        && b.bytes().all(|c| c.is_ascii_digit())
                });
                if ist_laenge {
                    out.push(text.to_string());
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        sammeln(&c.shape, &mut out);
    }
    out.sort();
    out
}

#[test]
fn laengen_am_gewaehlten_teil() {
    let mut h = mit_bluse();
    assert!(gemalte_laengen(&h).is_empty(), "Längen sind am Anfang aus");
    h.key_press_modifiers(Modifiers::SHIFT, Key::Z);
    h.run();
    let alle = gemalte_laengen(&h);
    // A selected piece: only its edges carry lengths, each the length of one of its edges.
    h.get_by_role_and_label(Role::Button, "Band Mitte").click();
    h.run();
    let soll = soll_bluse();
    let (_, _, kanten) = soll
        .teile
        .iter()
        .find(|t| t.0.ends_with(" - Band Mitte"))
        .unwrap();
    let moeglich: Vec<String> = kanten.iter().map(|(_, l)| laenge_text(*l)).collect();
    let gewaehlt = gemalte_laengen(&h);
    assert!(!gewaehlt.is_empty(), "keine Länge an Band Mitte");
    assert!(gewaehlt.len() <= kanten.len());
    for l in &gewaehlt {
        assert!(moeglich.contains(l), "{l} ist keine Kante von Band Mitte");
    }
    assert!(
        alle.len() > gewaehlt.len(),
        "ohne Wahl alle Teile: {}",
        alle.len()
    );
}

#[test]
fn zuletzt_geoeffnet_im_menue() {
    let ordner = frischer_ordner("zuletzt");
    let kopie = ordner.join("kopie.sm2d");
    std::fs::copy(bluse(), &kopie).unwrap();
    std::fs::copy(
        muster_ordner().join("measurements.smis"),
        ordner.join("measurements.smis"),
    )
    .unwrap();
    let bluse1 = muster_ordner().join("bluse1_v25.sm2d");

    let mut h = fenster();
    h.run();
    menue_oeffnen(&mut h, &["Datei", "Zuletzt geöffnet"]);
    assert!(h.query_by_label("noch keine Muster geöffnet").is_some());
    assert!(im_menue(&h, "Liste leeren").accesskit_node().is_disabled());
    menue_schliessen(&mut h, "Datei");

    // Opened one after the other: newest first.
    for p in [bluse(), kopie.clone(), bluse1.clone()] {
        assert!(h.state_mut().muster_laden(&p));
    }
    h.run();
    let erwartet: Vec<PathBuf> = [&bluse1, &kopie, &bluse()]
        .iter()
        .map(|p| std::path::absolute(p).unwrap())
        .collect();
    assert_eq!(h.state().zuletzt, erwartet);
    menue_oeffnen(&mut h, &["Datei", "Zuletzt geöffnet"]);
    let oben: Vec<f32> = ["bluse1_v25.sm2d", "kopie.sm2d", "bluse3_v2.sm2d"]
        .iter()
        .map(|n| im_menue(&h, n).rect().top())
        .collect();
    assert!(oben[0] < oben[1] && oben[1] < oben[2], "{oben:?}");
    menue_schliessen(&mut h, "Datei");

    // Saved and read back.
    let mut speicher = Speicher::default();
    eframe::App::save(h.state_mut(), &mut speicher);
    assert!(speicher.0.contains_key(SCHLUESSEL_ZULETZT));
    drop(h);
    let mut h = Harness::builder()
        .with_size([1600.0, 1400.0])
        .build_eframe(|cc| {
            cc.storage = Some(&speicher);
            App::neu(cc)
        });
    h.run();
    assert_eq!(h.state().zuletzt, erwartet);
    assert!(h.state().muster.is_none());

    // Clicking one opens it and moves it to the front.
    menue_waehlen(&mut h, &["Datei", "Zuletzt geöffnet"], "bluse3_v2.sm2d");
    assert_eq!(gezeigt(&h).as_deref(), Some("bluse3_v2.sm2d"));
    assert_eq!(h.state().muster.as_ref().unwrap().teile.len(), 28);
    assert_eq!(h.state().zuletzt[0], erwartet[2]);

    // A file that is gone is shown disabled, with the reason on hover.
    std::fs::remove_file(&kopie).unwrap();
    h.ctx
        .global_style_mut(|s| s.interaction.tooltip_delay = 0.0);
    menue_oeffnen(&mut h, &["Datei", "Zuletzt geöffnet"]);
    let weg = im_menue(&h, "kopie.sm2d");
    assert!(weg.accesskit_node().is_disabled());
    weg.hover();
    h.run();
    assert!(hinweis_da(&h, "Datei fehlt"));
    assert!(!im_menue(&h, "bluse1_v25.sm2d")
        .accesskit_node()
        .is_disabled());
    // "Liste leeren" empties it.
    im_menue(&h, "Liste leeren").click();
    h.run();
    assert!(h.state().zuletzt.is_empty());
    let _ = std::fs::remove_dir_all(&ordner);
}

#[test]
fn bibliothek_oeffnet_und_ordnet_zu() {
    let ordner = frischer_ordner("bibliothek");
    for (von, nach) in [
        (bluse(), "bluse3_v2.sm2d"),
        (
            muster_ordner().join("measurements.smis"),
            "measurements.smis",
        ),
        (bluse().with_extension("naehte"), "bluse3_v2.naehte"),
        (bluse().with_extension("naehte"), "andere.naehte"),
    ] {
        std::fs::copy(von, ordner.join(nach)).unwrap();
    }
    std::fs::write(ordner.join("stoff.zfab"), "x").unwrap();
    std::fs::write(ordner.join("notiz.txt"), "x").unwrap();

    let mut h = schnelles_fenster();
    h.run();
    // "Zuletzt" jumps to the last folder; set it as a load would.
    h.state_mut().letzter_ordner = ordner.clone();
    h.get_by_role_and_label(Role::Button, "Zuletzt").click();
    h.run();
    assert_eq!(
        h.state().bibliothek.liste.ordner,
        std::path::absolute(&ordner).unwrap()
    );
    assert!(
        h.query_by_label(&ordner.display().to_string()).is_some(),
        "Pfad"
    );
    // The kinds klon3d knows; a fabric greyed, other files not listed.
    for name in [
        "bluse3_v2.sm2d",
        "measurements.smis",
        "bluse3_v2.naehte",
        "andere.naehte",
    ] {
        let z = h.get_by_role_and_label(Role::Button, name);
        assert!(!z.accesskit_node().is_disabled(), "{name} gesperrt");
    }
    assert!(h
        .get_by_role_and_label(Role::Button, "stoff.zfab")
        .accesskit_node()
        .is_disabled());
    assert!(h.query_by_label("notiz.txt").is_none());
    // Nothing marked: the button below does nothing yet.
    assert!(h
        .get_by_role_and_label(Role::Button, "Datei verwenden")
        .accesskit_node()
        .is_disabled());

    // Double click on the pattern loads it.
    let p = h
        .get_by_role_and_label(Role::Button, "bluse3_v2.sm2d")
        .rect()
        .center();
    doppelklick_bei(&mut h, p);
    let m = h.state().muster.as_ref().expect("geladen");
    assert_eq!(m.teile.len(), 28);
    assert_eq!(
        m.pfad,
        std::path::absolute(ordner.join("bluse3_v2.sm2d")).unwrap()
    );
    assert!(
        matches!(&m.naehte, Nahtstand::Geprueft { pfad, .. } if pfad.ends_with("bluse3_v2.naehte"))
    );

    // Double click on another seam list assigns it to the pattern shown.
    let p = h
        .get_by_role_and_label(Role::Button, "andere.naehte")
        .rect()
        .center();
    doppelklick_bei(&mut h, p);
    let m = h.state().muster.as_ref().unwrap();
    match &m.naehte {
        Nahtstand::Geprueft { pfad, bericht } => {
            assert!(pfad.ends_with("andere.naehte"), "{}", pfad.display());
            assert_eq!(bericht.naehte.len(), 24);
        }
        andere => panic!("{andere:?}"),
    }
    assert_eq!(m.pfad.file_name().unwrap(), "bluse3_v2.sm2d");

    // One click marks, the button uses: the measurement file by hand.
    h.get_by_role_and_label(Role::Button, "measurements.smis")
        .click();
    h.run();
    let knopf = h.get_by_role_and_label(Role::Button, "Als Maßdatei nehmen");
    assert!(!knopf.accesskit_node().is_disabled());
    knopf.click();
    h.run();
    let m = h.state().muster.as_ref().unwrap();
    assert!(m.massdatei_von_hand && m.massdatei.ends_with("measurements.smis"));

    // "Muster" jumps to the pattern's folder, "Stoffe" (Materialien › Stoffbibliothek too) to the
    // fabrics.
    let stoffe = h.state().stoffordner.clone().expect("assets/stoffe");
    h.get_by_role_and_label(Role::Button, "Stoffe").click();
    h.run();
    assert_eq!(h.state().bibliothek.liste.ordner, stoffe);
    h.get_by_role_and_label(Role::Button, "Muster").click();
    h.run();
    assert_eq!(
        h.state().bibliothek.liste.ordner,
        std::path::absolute(&ordner).unwrap()
    );
    menue_waehlen(&mut h, &["Anzeige", "Fenster"], "Bibliothek");
    assert!(flaeche(&h, "Bibliothek").is_none());
    menue_waehlen(&mut h, &["Materialien"], "Stoffbibliothek");
    assert!(flaeche(&h, "Bibliothek").is_some());
    assert_eq!(h.state().bibliothek.liste.ordner, stoffe);
    let _ = std::fs::remove_dir_all(&ordner);
}

#[test]
fn stoffe_und_informationen() {
    let mut h = mit_bluse();
    // The Stoffe tab lists the fabric files of the repository's fabrics folder.
    h.get_by_role_and_label(Role::Tab, "Stoffe").click();
    h.run();
    let stoffe: Vec<String> = std::fs::read_dir(h.state().stoffordner.as_ref().unwrap())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".zfab"))
        .collect();
    assert!(!stoffe.is_empty());
    for s in &stoffe {
        assert!(h.query_by_label(s).is_some(), "{s} fehlt");
    }
    assert!(h
        .query_by_label("Werte folgen mit dem Stoff-Leser")
        .is_some());

    // Datei › Informationen… names the files of the pattern shown.
    menue_waehlen(&mut h, &["Datei"], "Informationen…");
    assert!(h.state().informationen);
    let wert = |h: &Harness<'_, App>, name: &str| zeile_nach(h, Role::Label, name, 1)[0].clone();
    assert_eq!(wert(&h, "Teile"), "28");
    assert_eq!(wert(&h, "Nähte"), "24");
    assert_eq!(
        wert(&h, "Einheit"),
        h.state().muster.as_ref().unwrap().einheit
    );
    assert!(wert(&h, "Maßdatei").ends_with("measurements.smis (vom Muster genannt)"));
    assert!(wert(&h, "Nahtliste").ends_with("bluse3_v2.naehte"));
    assert!(wert(&h, "Muster").ends_with("bluse3_v2.sm2d"));
    h.get_by_label("Informationen schließen").click();
    h.run();
    assert!(!h.state().informationen);
}

#[test]
fn klick_auf_eine_naht_zeigt_ihre_eigenschaften() {
    let mut h = mit_bluse();
    // Independently: the seam report as `klon3d naehte` computes it.
    let muster = read_path(&bluse()).unwrap();
    let mess = lies_messungen(&muster_ordner().join("measurements.smis"), &muster.unit).unwrap();
    let ev = Auswertung::new(&muster, mess);
    let defs = lies_nahtliste(&std::fs::read_to_string(bluse().with_extension("naehte")).unwrap())
        .unwrap();
    let bericht = pruefe_naehte(&ev, &defs, Kurvenquelle::Seamly).unwrap();
    // A seam with notch pairs if there is one.
    let (ni, n) = bericht
        .naehte
        .iter()
        .enumerate()
        .max_by_key(|(_, n)| n.kerbpaare.len())
        .unwrap();

    menue_waehlen(&mut h, &["2D-Schnitt", "Nähen"], "Nahtlängen prüfen");
    assert!(
        h.query_by_role_and_label(Role::Tab, "Nähte")
            .is_some_and(|t| t.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)),
        "Nahtlängen prüfen zeigt den Reiter Nähte"
    );
    h.get_by_role_and_label(Role::Button, &n.kennung).click();
    h.run();
    assert_eq!(
        h.state().naht_gewaehlt,
        Some(ni),
        "Naht in 2D hervorgehoben"
    );
    // Back to the pieces tab – the seam stays in the properties, and the seam list's column
    // titles no longer share their names with the property rows.
    h.get_by_role_and_label(Role::Tab, "Teile").click();
    h.run();
    let wert = |h: &Harness<'_, App>, name: &str| zeile_nach(h, Role::Label, name, 1)[0].clone();
    assert_eq!(wert(&h, "Kennung"), n.kennung);
    assert_eq!(wert(&h, "Bezeichnung"), n.name);
    assert_eq!(wert(&h, "L_A cm"), format!("{:.4}", n.a.laenge_cm));
    assert_eq!(wert(&h, "L_B cm"), format!("{:.4}", n.b.laenge_cm));
    assert_eq!(wert(&h, "Δ cm"), delta_wie_cli(n.diff_cm));
    assert!(h
        .query_by_label(&format!("Seite A · {:.4} cm", n.a.laenge_cm))
        .is_some());
    assert!(h
        .query_by_label(&format!("Passzeichenpaare ({})", n.kerbpaare.len()))
        .is_some());
    // The sections of both sides name their pieces by the short name (the piece list shows the
    // same name as a button).
    for ts in n.a.teilstuecke.iter().chain(&n.b.teilstuecke) {
        let kurz = kurzname(&ts.teil);
        assert!(
            h.query_all_by_label(kurz)
                .any(|k| k.accesskit_node().role() == Role::Label),
            "{kurz}: nicht in den Seiten der Naht"
        );
    }

    // Choosing a piece shows the piece again.
    h.get_by_role_and_label(Role::Button, "Band Mitte").click();
    h.run();
    assert!(h.query_by_label("L_A cm").is_none());
    assert!(h.query_by_label_contains("Randkanten (").is_some());
}

#[test]
fn alter_gespeicherter_stand_startet_mit_vorgaben() {
    // A storage written by the version before step 7b: theme and folder only.
    let mut speicher = Speicher::default();
    speicher.0.insert(SCHLUESSEL_THEMA.into(), "hell".into());
    speicher.0.insert(
        SCHLUESSEL_ORDNER.into(),
        muster_ordner().display().to_string(),
    );
    let mut h = Harness::builder()
        .with_size([1600.0, 1000.0])
        .build_eframe(|cc| {
            cc.storage = Some(&speicher);
            App::neu(cc)
        });
    h.run();
    let app = h.state();
    assert_eq!(app.thema, Theme::Light);
    assert_eq!(app.anordnung, Anordnung::default());
    assert_eq!(app.ansicht.anzeige, Anzeige2d::default());
    assert!(app.zuletzt.is_empty());
    for (_, name) in FLAECHEN {
        assert!(flaeche(&h, name).is_some(), "{name} fehlt");
    }
    drop(h);

    // Damaged values of the new keys are no reason to fail either.
    speicher.0.insert(
        SCHLUESSEL_ANORDNUNG.into(),
        "bibliothek=vielleicht;teilung=NaN".into(),
    );
    speicher.0.insert(SCHLUESSEL_ANZEIGE2D.into(), "?".into());
    speicher.0.insert(SCHLUESSEL_ZULETZT.into(), "\n\n".into());
    let mut h = Harness::builder()
        .with_size([1600.0, 1000.0])
        .build_eframe(|cc| {
            cc.storage = Some(&speicher);
            App::neu(cc)
        });
    h.run();
    assert_eq!(h.state().anordnung, Anordnung::default());
    assert_eq!(h.state().ansicht.anzeige, Anzeige2d::default());
    assert!(h.state().zuletzt.is_empty());

    // What this version stores comes back.
    menue_waehlen(&mut h, &["Anzeige", "Fenster"], "Bibliothek");
    h.key_press_modifiers(Modifiers::SHIFT, Key::Z);
    h.run();
    let mut neu = Speicher::default();
    eframe::App::save(h.state_mut(), &mut neu);
    let vorher = (h.state().anordnung.clone(), h.state().ansicht.anzeige);
    drop(h);
    let mut h = Harness::builder()
        .with_size([1600.0, 1000.0])
        .build_eframe(|cc| {
            cc.storage = Some(&neu);
            App::neu(cc)
        });
    h.run();
    assert_eq!(
        (h.state().anordnung.clone(), h.state().ansicht.anzeige),
        vorher
    );
    assert!(!h.state().anordnung.bibliothek && h.state().ansicht.anzeige.laengen);
    assert!(flaeche(&h, "Bibliothek").is_none());
}

#[test]
fn anordnung_zuruecksetzen() {
    let mut h = mit_bluse();
    let bib = flaeche(&h, "Bibliothek").unwrap();
    let d3 = flaeche(&h, "3D-Fenster").unwrap();

    // Move the divider between 3D and 2D and widen the library by their edges.
    let teiler = h.get_by_label("Teilung 3D | 2D").rect().center();
    ziehen(&mut h, teiler, teiler - egui::vec2(120.0, 0.0));
    assert!(
        h.state().anordnung.teilung < 0.45,
        "{}",
        h.state().anordnung.teilung
    );
    let rand = egui::pos2(d3.left() - 1.0, d3.center().y);
    ziehen(&mut h, rand, rand + egui::vec2(50.0, 0.0));
    let breiter = flaeche(&h, "Bibliothek").unwrap();
    assert!(
        breiter.width() > bib.width() + 20.0,
        "{bib:?} → {breiter:?}"
    );
    // Hide a few parts.
    menue_waehlen(&mut h, &["Anzeige", "Fenster"], "Eigenschaften");
    menue_waehlen(&mut h, &["Anzeige", "Werkzeugleisten"], "2D-Werkzeugleiste");
    assert_ne!(h.state().anordnung, Anordnung::default());

    menue_waehlen(&mut h, &["Anzeige"], "Anordnung zurücksetzen");
    assert_eq!(h.state().anordnung, Anordnung::default());
    for (_, name) in FLAECHEN {
        assert!(flaeche(&h, name).is_some(), "{name} fehlt");
    }
    let (d3, d2) = (
        flaeche(&h, "3D-Fenster").unwrap(),
        flaeche(&h, "2D-Fenster").unwrap(),
    );
    assert!((d3.width() - d2.width()).abs() <= 2.0, "{d3:?} {d2:?}");
    let zurueck = flaeche(&h, "Bibliothek").unwrap();
    assert!(
        (zurueck.width() - bib.width()).abs() <= 1.0,
        "{bib:?} → {zurueck:?}"
    );
    assert!(h
        .query_by_role_and_label(Role::Button, "Einpassen")
        .is_some());
}
