//! Platzierung und Anziehen (Stufe 4c) an den Prüfmustern: die Teile nach
//! `<muster>.lage` um den Körper der Prüfmuster-Maßdatei legen und am Körper
//! anziehen (Befunde 95, 96, 98) — Bluse3 v2, dazu Bluse1 v25 und Bustier v1
//! für die Allgemeinheit, alle mit denselben Vorgaben. Braucht CLOs
//! Resizing-Daten; fehlen sie, sagen die Tests das und hören auf. Nur mit
//! `cargo test --release`.

use std::path::PathBuf;

use klon3d_mesh::{vernetze, Musternetz, Parameter};
use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::lagevorschlag::schlage_lage_vor;
use klon3d_pattern::naehte::{lies_nahtliste, pruefe_naehte};
use klon3d_pattern::vorschlag::spiegelung;
use klon3d_sim::platzierung::{anziehen, lies_lage, platziere, Anziehen, Huellen, Lageliste};
use klon3d_sim::{messung, ImplizitCg, ImplizitParameter, Koerper, Koerpernetz, Simparameter, Szene, Teilvorgabe, RADICAL_RUBBER_04};

mod band;

/// Netz und Lageliste eines Prüfmusters (`<name>.sm2d`, `.naehte`, `.lage`).
fn pruefmuster(name: &str) -> (Musternetz, Lageliste) {
    let w = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../klon3d-pattern/tests/muster"));
    let pfad = w.join(format!("{name}.sm2d"));
    let m = klon3d_pattern::read_path(&pfad).unwrap();
    // Die Maßdatei liegt neben dem Muster (Unterordner haben ihre eigene).
    let ev = Auswertung::lade(&m, &pfad.with_file_name("measurements.smis")).unwrap();
    let defs = lies_nahtliste(&std::fs::read_to_string(w.join(format!("{name}.naehte"))).unwrap()).unwrap();
    let b = pruefe_naehte(&ev, &defs, Kurvenquelle::Seamly).unwrap();
    let netz = vernetze(&ev, Kurvenquelle::Seamly, &b.naehte, &Parameter::default()).unwrap();
    let liste = lies_lage(&std::fs::read_to_string(w.join(format!("{name}.lage"))).unwrap()).unwrap();
    (netz, liste)
}

fn bluse() -> (Musternetz, Lageliste) {
    pruefmuster("bluse3_v2")
}

/// Was ein Anziehen ergibt.
struct Ergebnis {
    /// Anteil der Nahtpaare bis 10 mm.
    anteil: f64,
    /// Median je Naht (Kennung, m).
    mediane: Vec<(String, f64)>,
    /// mittlere Geschwindigkeit am Ende (m/s)
    v: f64,
    /// tiefster Punkt (m, außen positiv)
    tiefster: f64,
    /// stärkstes Sinken eines Teils in der Haltephase (Teil, m)
    sinken: (String, f64),
}

/// Netz und Lageliste eines Prüfmusters ganz aus den Vorschlägen (Befund
/// 101): Nahtvorschlag, daraus der Lagevorschlag — keine Handdatei.
fn automatisch(name: &str) -> (Musternetz, Lageliste) {
    let w = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../klon3d-pattern/tests/muster"));
    let pfad = w.join(format!("{name}.sm2d"));
    let m = klon3d_pattern::read_path(&pfad).unwrap();
    let ev = Auswertung::lade(&m, &pfad.with_file_name("measurements.smis")).unwrap();
    let v = ev.schlage_naehte_vor(Kurvenquelle::Seamly).unwrap();
    let l = schlage_lage_vor(&ev, Kurvenquelle::Seamly, &v.naehte, &v.gespiegelt, name).unwrap();
    let b = pruefe_naehte(&ev, &v.naehte, Kurvenquelle::Seamly).unwrap();
    let netz = vernetze(&ev, Kurvenquelle::Seamly, &b.naehte, &Parameter::default()).unwrap();
    (netz, lies_lage(&l.text).unwrap())
}

/// Zieht ein Prüfmuster mit Radical 0,4 mm (Dicke je Teil aus der Lage) an.
fn anziehen_pruefmuster(name: &str, k: &Koerpernetz, h: &Huellen, ablauf: &Anziehen) -> Ergebnis {
    let (netz, liste) = pruefmuster(name);
    anziehen_netz(name, &netz, &liste, k, h, ablauf)
}

fn anziehen_netz(name: &str, netz: &Musternetz, liste: &Lageliste, k: &Koerpernetz, h: &Huellen, ablauf: &Anziehen) -> Ergebnis {
    let p = platziere(netz, liste, h).unwrap();
    let vg = |teil: &str| {
        let dicke = p.netz.teile.iter().position(|t| t.teil == teil).and_then(|ti| p.dicke(ti));
        Teilvorgabe { material: dicke.map_or(RADICAL_RUBBER_04, |d| RADICAL_RUBBER_04.mit_dicke(d)), reduktion: 0.0 }
    };
    let param = Simparameter { schwerkraft: [0.0, 0.0, -9.81], daempfung: 1.0, ..Simparameter::default() };
    let mut s = Szene::aus_netz(&p.netz, &vg, &|ti, q| p.lage(ti, q), vec![Koerper::netz(k.clone())], param);
    let mut rw = ImplizitCg::neu(ImplizitParameter::default());
    let erg = anziehen(&mut s, &mut rw, ablauf, &mut |_, _, _| {});
    let (mut alle, mut zu, mut mediane) = (0usize, 0usize, Vec::new());
    for (kennung, mut d) in messung::nahtabstaende(&s, &p.netz) {
        alle += d.len();
        zu += d.iter().filter(|&&x| x <= 0.01).count();
        d.sort_by(f64::total_cmp);
        eprintln!("{name} {kennung:<5} Median {:5.1} mm, größter {:6.1} mm", d[d.len() / 2] * 1e3, d[d.len() - 1] * 1e3);
        mediane.push((kennung, d[d.len() / 2]));
    }
    let e = Ergebnis {
        anteil: zu as f64 / alle as f64,
        mediane,
        v: messung::mittlere_geschwindigkeit(&s),
        tiefster: s.x.iter().map(|&q| k.naechster_alle(q).abstand).fold(f64::INFINITY, f64::min),
        sinken: erg.sinken[0].clone(),
    };
    eprintln!(
        "{name}: {zu} von {alle} Nahtpaaren bis 10 mm ({:.1} %), {:.4} m/s, tiefster Punkt {:+.2} mm, sinkt am stärksten {} {:+.1} mm",
        e.anteil * 100.0,
        e.v,
        e.tiefster * 1e3,
        e.sinken.0,
        e.sinken.1 * 1e3
    );
    e
}

/// Die Lage vor dem Nähen: 16 genähte Teile platziert, die 12 aufgeklebten
/// weggelassen, alle 24 Nähte da; kein Punkt tiefer als 5 mm in der Haut
/// (der Kollider findet bis 10 mm). Nähte zwischen Teilen eines Blocks mit
/// gemeinsamen Punkten (hintere Mitte, Untercup an Band Mitte) liegen schon
/// aufeinander.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn bluse_platzieren() {
    let Some((k, h)) = band::pruefkoerper() else { return };
    let (netz, liste) = bluse();
    let p = platziere(&netz, &liste, &h).unwrap();
    assert_eq!(p.netz.teile.len(), 16);
    assert_eq!(p.weggelassen.len(), 12);
    assert_eq!(p.netz.naehte.len(), 24);
    assert!(p.naehte_weggelassen.is_empty());
    let vg = Teilvorgabe { material: RADICAL_RUBBER_04, reduktion: 0.0 };
    let s = Szene::aus_netz(&p.netz, &|_| vg, &|ti, q| p.lage(ti, q), vec![], Simparameter::default());
    let tiefster = s.x.iter().map(|&q| k.naechster_alle(q).abstand).fold(f64::INFINITY, f64::min);
    assert!(tiefster > -5e-3, "Punkt {:.1} mm in der Haut", tiefster * 1e3);
    for (kennung, d) in messung::nahtabstaende(&s, &p.netz) {
        if kennung == "N01" || kennung.starts_with("N11") {
            let mut d = d;
            d.sort_by(f64::total_cmp);
            assert!(d[d.len() / 2] < 0.01, "{kennung}: Median {:.1} mm", d[d.len() / 2] * 1e3);
        }
    }
}

/// Anziehen am Körper ([`anziehen`], Befunde 96, 98) mit den Vorgaben:
/// Nähte schließen sich mit 0,1 m/s über 200 Bilder, gehalten (ohne
/// Schwerkraft), Nahtfeder 2.000 N/m, dabei Körperpunkt gegen Stoffdreieck,
/// bis 600 CG-Iterationen; dann mit voller Schwerkraft einschwingen, bis
/// kein Teil mehr sinkt, und 10 s Haltephase (Befund 102). Kommt zur Ruhe
/// (unter 0,015 m/s: mit genau gelösten Schritten fehlt die Dämpfung des
/// abgebrochenen CG, Säume bewegen sich noch um 1 cm/s, ohne zu sinken),
/// kein Punkt in der Haut, jede Naht mit Median unter 3 mm, mindestens 98 %
/// aller Nahtpaare bis 10 mm (gemessen 98,9 %; Befund 98 mit 50
/// Iterationen 99,4 %, Befund 96 mit 500 N/m: 98,6 %), kein Teil
/// sinkt in der Haltephase mehr als 5 mm (ADR-023); ohne Körperpunkt gegen
/// Stoffdreieck weniger Nahtpaare.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn bluse_anziehen_am_koerper() {
    let Some((k, h)) = band::pruefkoerper() else { return };
    let mit = anziehen_pruefmuster("bluse3_v2", &k, &h, &Anziehen::default());
    let ohne = anziehen_pruefmuster("bluse3_v2", &k, &h, &Anziehen { koerperpunkt_durchgaenge: 0, ..Anziehen::default() });
    for (kennung, m) in &mit.mediane {
        assert!(*m < 3e-3, "{kennung}: Median {:.1} mm", m * 1e3);
    }
    assert!(mit.v < 0.015, "nicht in Ruhe: {:.4} m/s", mit.v);
    assert!(mit.tiefster > -0.1e-3, "Punkt {:.2} mm in der Haut", mit.tiefster * 1e3);
    assert!(mit.anteil >= 0.98, "nur {:.1} % der Nahtpaare zu", mit.anteil * 100.0);
    assert!(mit.sinken.1 < 5e-3, "{} sinkt {:.1} mm", mit.sinken.0, mit.sinken.1 * 1e3);
    assert!(mit.anteil > ohne.anteil, "Körperpunkt gegen Stoffdreieck hilft nicht: {:.3} gegen {:.3}", mit.anteil, ohne.anteil);
}

/// Allgemeinheit (Befund 100): der Bolero v37 aus dem Hexenensemble vom
/// 27.10.2026, am Körper seiner eigenen Maßdatei — ein Rückenteil über beide
/// Seiten, offene Front, Bischofärmel mit sieben Kugelfalten und sechs
/// Saumfalten (als Abnäher geschlossen), Manschetten, Stehkragen. Kommt zur
/// Ruhe, nichts in der Haut, hält (ADR-023), mindestens 94 % der Nahtpaare
/// bis 10 mm (gemessen 94,5 %, mit 50 CG-Iterationen 96,5 %; offen bleiben
/// die Saumecken der kurzen Seitennähte hinter und vor dem Arm und Paare an
/// Armloch und Manschette).
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn bolero_anziehen() {
    let smis = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../klon3d-pattern/tests/muster/hexen_2026_10_27/measurements.smis"));
    let Some((k, h)) = band::pruefkoerper_aus(&smis) else { return };
    let e = anziehen_pruefmuster("hexen_2026_10_27/bolero_v37", &k, &h, &Anziehen::default());
    assert!(e.v < 0.015, "nicht in Ruhe: {:.4} m/s", e.v);
    assert!(e.tiefster > -0.1e-3, "Punkt {:.2} mm in der Haut", e.tiefster * 1e3);
    assert!(e.anteil >= 0.94, "nur {:.1} % der Nahtpaare zu", e.anteil * 100.0);
    assert!(e.sinken.1 < 5e-3, "{} sinkt {:.1} mm", e.sinken.0, e.sinken.1 * 1e3);
}

/// Allgemeinheit (Befund 98): Bluse1 v25 (Keyhole, durchgehende
/// Teilungsnähte) und das trägerlose Bustier v1 (dreiteilige Cups 1,0 mm,
/// Rumpf 2 × 1,0 mm) mit denselben Vorgaben wie die Bluse3. Beide kommen zur
/// Ruhe, nichts in der Haut, kein Teil sinkt in der Haltephase mehr als 5 mm;
/// Bluse1 mindestens 98 % der Nahtpaare bis 10 mm (gemessen 98,9 %), Bustier
/// mindestens 93 % (gemessen 93,6 %; offen bleiben Ecken der Cups und die
/// Oberkante der Seitennähte).
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn pruefmuster_anziehen() {
    let Some((k, h)) = band::pruefkoerper() else { return };
    for (name, mindestens) in [("bluse1_v25", 0.98), ("bustier_v1", 0.93)] {
        let e = anziehen_pruefmuster(name, &k, &h, &Anziehen::default());
        assert!(e.v < 0.015, "{name}: nicht in Ruhe: {:.4} m/s", e.v);
        assert!(e.tiefster > -0.1e-3, "{name}: Punkt {:.2} mm in der Haut", e.tiefster * 1e3);
        assert!(e.anteil >= mindestens, "{name}: nur {:.1} % der Nahtpaare zu", e.anteil * 100.0);
        assert!(e.sinken.1 < 5e-3, "{name}: {} sinkt {:.1} mm", e.sinken.0, e.sinken.1 * 1e3);
    }
}

/// Lagevorschlag aus der Handliste gegen die Handlage (Befund 101): je Teil
/// der mittlere Abstand seiner Netzpunkte zwischen beiden Platzierungen am
/// Prüfkörper, gemittelt über die Teile. Gemessen Bluse3 2,8 cm, Bluse1 1,8,
/// Bustier 0,3, Bolero 1,6, Rock 0,5 (geweitete Hülle, Befund 102; der Bund
/// liegt im Vorschlag 2,2 cm tiefer; die Blusen: die hinteren Seitenteile
/// liegen von Hand 7 cm weiter hinten; der Bolero-Rumpf am Armloch mit
/// 0,5 statt 2 cm Abstand — angezogen gleich gut, 96,4 statt 96,5 %).
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn lagevorschlag_wie_handlage() {
    let w = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../klon3d-pattern/tests/muster"));
    for (name, hoechstens) in [
        ("bluse3_v2", 0.035),
        ("bluse1_v25", 0.025),
        ("bustier_v1", 0.005),
        ("hexen_2026_10_27/bolero_v37", 0.02),
        ("hexen_2026_10_27/rock_v36", 0.0075),
    ] {
        let pfad = w.join(format!("{name}.sm2d"));
        let Some((_, h)) = band::pruefkoerper_aus(&pfad.with_file_name("measurements.smis")) else { return };
        let m = klon3d_pattern::read_path(&pfad).unwrap();
        let ev = Auswertung::lade(&m, &pfad.with_file_name("measurements.smis")).unwrap();
        let defs = lies_nahtliste(&std::fs::read_to_string(w.join(format!("{name}.naehte"))).unwrap()).unwrap();
        let (gespiegelt, _) = spiegelung(&ev, Kurvenquelle::Seamly, &defs).unwrap();
        let l = schlage_lage_vor(&ev, Kurvenquelle::Seamly, &defs, &gespiegelt, name).unwrap();
        let (netz, hand) = pruefmuster(name);
        let (p, q) = (platziere(&netz, &lies_lage(&l.text).unwrap(), &h).unwrap(), platziere(&netz, &hand, &h).unwrap());
        assert_eq!(p.netz.teile.len(), q.netz.teile.len(), "{name}: andere Teile platziert");
        let mut summe = 0.0;
        for (ti, t) in p.netz.teile.iter().enumerate() {
            let tj = q.netz.teile.iter().position(|u| u.teil == t.teil).unwrap();
            let d: f64 = t
                .punkte
                .iter()
                .map(|&x| {
                    let (a, b) = (p.lage(ti, x), q.lage(tj, x));
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
                })
                .sum::<f64>()
                / t.punkte.len() as f64;
            summe += d;
        }
        let mittel = summe / p.netz.teile.len() as f64;
        eprintln!("{name}: Lagevorschlag im Mittel {:.1} cm neben der Handlage", mittel * 100.0);
        assert!(mittel <= hoechstens, "{name}: {:.1} cm", mittel * 100.0);
    }
}

/// Ganz automatisch (Befund 101): Bustier und Bluse1 aus Naht- und
/// Lagevorschlag angezogen, mit den Vorgaben. Kommen zur Ruhe, nichts in der
/// Haut, halten (ADR-023); Bustier mindestens 94 % der Nahtpaare bis 10 mm
/// (gemessen 95,0 %, von Hand 93,6 %), Bluse1 mindestens 97 % (gemessen
/// 98,0 %, von Hand 98,9 %; Befund 102).
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn automatisch_anziehen() {
    let Some((k, h)) = band::pruefkoerper() else { return };
    for (name, mindestens) in [("bustier_v1", 0.94), ("bluse1_v25", 0.97)] {
        let (netz, liste) = automatisch(name);
        let e = anziehen_netz(name, &netz, &liste, &k, &h, &Anziehen::default());
        assert!(e.v < 0.015, "{name}: nicht in Ruhe: {:.4} m/s", e.v);
        assert!(e.tiefster > -0.1e-3, "{name}: Punkt {:.2} mm in der Haut", e.tiefster * 1e3);
        assert!(e.anteil >= mindestens, "{name}: nur {:.1} % der Nahtpaare zu", e.anteil * 100.0);
        assert!(e.sinken.1 < 5e-3, "{name}: {} sinkt {:.1} mm", e.sinken.0, e.sinken.1 * 1e3);
    }
}

/// Weite der Rumpfhülle (Befund 102): Beim Rock v36 sind die ausgestellten
/// Bahnen unten breiter als das Prisma — die Hülle weitet sich dort, in der
/// Taille nicht, und nach unten nimmt die Weite nie ab. Die Bluse3 v2 ist
/// enger als der Körper und bleibt auf dem senkrechten Prisma.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn weite_nur_wo_die_teile_breiter_sind() {
    let w = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../klon3d-pattern/tests/muster/hexen_2026_10_27/measurements.smis"));
    let Some((_, h)) = band::pruefkoerper_aus(&w) else { return };
    let (netz, liste) = pruefmuster("hexen_2026_10_27/rock_v36");
    let p = platziere(&netz, &liste, &h).unwrap();
    let (unten, oben) = p.rumpf_hoehen;
    let w: Vec<f64> = (0..=100).map(|i| p.weite_bei(unten + (oben - unten) * i as f64 / 100.0)).collect();
    assert!(w.windows(2).all(|x| x[1] <= x[0] + 1e-12), "Weite nimmt nach unten ab");
    assert!(w[0] > 1.5, "unten nur {:.2}", w[0]);
    let taille = h.hoehen.hals_seitlich - 0.37;
    assert!((p.weite_bei(taille) - 1.0).abs() < 1e-9, "in der Taille {:.3}", p.weite_bei(taille));
    eprintln!("Rock: Weite am Saum {:.2}, Mitte der Höhe {:.2}", w[0], w[50]);

    let Some((_, h)) = band::pruefkoerper() else { return };
    let (netz, liste) = bluse();
    let p = platziere(&netz, &liste, &h).unwrap();
    let (unten, oben) = p.rumpf_hoehen;
    assert!((0..=20).all(|i| p.weite_bei(unten + (oben - unten) * i as f64 / 20.0) == 1.0));
}

/// Der Rock v36 (Befund 102): Mit genau gelösten Schritten (bis 600
/// CG-Iterationen) und der geweiteten Hülle kommt er zur Ruhe, hält (ADR-023)
/// und schließt mindestens 98 % der Nahtpaare (gemessen 99,2 %, mit 0,2 mm
/// anderer Anfangslage ebenso; mit 50 Iterationen auf dem senkrechten Prisma
/// 95,1 %, unruhig, und 0,2 mm andere Anfangslage entschieden, ob er hält).
/// Der Saum über den Knöcheln bewegt sich noch um 1 cm/s.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn rock_anziehen() {
    let smis = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../klon3d-pattern/tests/muster/hexen_2026_10_27/measurements.smis"));
    let Some((k, h)) = band::pruefkoerper_aus(&smis) else { return };
    let e = anziehen_pruefmuster("hexen_2026_10_27/rock_v36", &k, &h, &Anziehen::default());
    assert!(e.v < 0.015, "nicht in Ruhe: {:.4} m/s", e.v);
    assert!(e.tiefster > -0.1e-3, "Punkt {:.2} mm in der Haut", e.tiefster * 1e3);
    assert!(e.anteil >= 0.98, "nur {:.1} % der Nahtpaare zu", e.anteil * 100.0);
    assert!(e.sinken.1 < 5e-3, "{} sinkt {:.1} mm", e.sinken.0, e.sinken.1 * 1e3);
}
