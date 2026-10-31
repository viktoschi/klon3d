//! Teilumrisse gegen Fehler, die kein Punkt-Sollwert zeigt.
//!
//! Die 1029 Sollwerte in `geometrie.rs` prüfen Konstruktionspunkte. Ein
//! Bogen geht dort nur ein, wenn ein Punkt an ihm hängt — in den vier
//! Prüfmustern hängt keiner daran. Bis zum 19.10.2026 lag deshalb jeder Bogen an der
//! Waagerechten durch seine Mitte gespiegelt, ohne dass ein Test es merkte;
//! die Stehkragen der Blusen hatten 11,2 m Naht. Aufgefallen ist es erst im
//! Betrachter. Diese Tests prüfen die Umrisse selbst.

mod muster;

use klon3d_pattern::geom::{lies_messungen, vektor, Achse, Auswertung, Pt};
use klon3d_pattern::kontur::{projektion, toleranz};
use klon3d_pattern::{read_path, Pattern};

use muster::*;

const ALLE: [&str; 4] = ["bustier_v1", "bluse2_v26", "bluse2_v29", "bluse3_v1"];

fn laden(name: &str) -> Pattern {
    read_path(&muster_pfad(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn auswertung(m: &Pattern) -> Auswertung<'_> {
    Auswertung::new(m, lies_messungen(&mess_pfad(), &m.unit).expect("Massdatei"))
}

fn ring_laenge(ev: &Auswertung<'_>, umriss: &[Pt]) -> f64 {
    let mut ring = umriss.to_vec();
    ring.push(umriss[0]);
    ev.umriss_laenge(&ring)
}

/// Bogen 407 der Bluse 2 (Kragenoberkante) läuft um 270°. In der Datei
/// zählt y nach unten; 270° zeigt also nach unten, der Bogen liegt **unter**
/// seiner Mitte — in der Zeichnung wie im Streckenzug, mit dem Seamly2D
/// schneidet. Anfang und Ende liegen dort, wo ein `endLine` mit demselben
/// Winkel und Radius hinführt.
#[test]
fn bogen_liegt_wo_seine_winkel_hinzeigen() {
    let m = laden("bluse2_v26");
    let ev = auswertung(&m);
    let b = ev.bogen("407").expect("Bogen 407");
    let d = ev.bogen_daten(b).unwrap();
    assert!(d.f1 < 270.0 && 270.0 < d.f2, "{} bis {}", d.f1, d.f2);

    let zeichnung = ev.kurve("407").unwrap();
    assert!(
        zeichnung.iter().all(|p| p.y > d.mitte.y),
        "Zeichnung liegt über der Mitte"
    );
    let endlinie = |w: f64| {
        let (dx, dy) = vektor(w, d.radius);
        Pt::new(d.mitte.x + dx, d.mitte.y + dy)
    };
    assert!(zeichnung[0].dist(endlinie(d.f1)) < 1e-9);
    assert!(zeichnung.last().unwrap().dist(endlinie(d.f2)) < 1e-9);

    let zug: Vec<Pt> = ev
        .seamly_zug("407")
        .unwrap()
        .iter()
        .map(|p| Pt::new(ev.von_px(p[0]), ev.von_px(p[1])))
        .collect();
    assert!(zug[0].dist(endlinie(d.f1)) < 1e-6);
    assert!(zug.last().unwrap().dist(endlinie(d.f2)) < 1e-6);
    for q in &zug {
        assert!(q.y > d.mitte.y, "Streckenzug liegt über der Mitte");
        // Bézier-Näherung je 45°: weit unter 0,1 % des Radius.
        assert!((q.dist(d.mitte) - d.radius).abs() < 1e-3 * d.radius);
    }
}

/// Jeder Punktknoten neben einem Bogenknoten liegt auf diesem Bogen — in
/// allen Teilen aller vier Muster: die Ecken des Bustiers (`#CornerR`,
/// vier Viertelkreise) wie die Stehkragen der Blusen. Nur dann wird der
/// Bogen auf das Stück zwischen seinen Nachbarn beschnitten; liegt er
/// gespiegelt, bleibt er ganz und die Kontur macht einen Umweg.
#[test]
fn punkte_neben_boegen_liegen_auf_ihnen() {
    let mut geprueft = 0;
    for name in ALLE {
        let m = laden(name);
        let ev = auswertung(&m);
        let tol = toleranz(&ev);
        for t in m.pieces() {
            let teil = t.name.clone().unwrap_or_default();
            let (knoten, offen) = ev.stueck_knoten(t);
            assert!(offen.is_empty(), "{teil}: {offen:?}");
            let n = knoten.len();
            for i in (0..n).filter(|&i| ev.bogen(&knoten[i].ref_id).is_some()) {
                for j in [(i + n - 1) % n, (i + 1) % n] {
                    if !knoten[j].ist_punkt {
                        continue;
                    }
                    let (_, abstand, _) = projektion(&knoten[i].punkte, knoten[j].punkte[0]);
                    assert!(
                        abstand <= tol,
                        "{name} / {teil}: Punkt {} liegt {abstand:.4} neben Bogen {}",
                        knoten[j].ref_id,
                        knoten[i].ref_id
                    );
                    geprueft += 1;
                }
            }
        }
    }
    // 4 Bögen im Bustier, je Bluse mehrere Dutzend — nicht leer durchlaufen.
    assert!(geprueft > 50, "nur {geprueft} Nachbarn geprüft");
}

/// Die Stehkragen sind Ringausschnitte um eine Mitte, rund 37 cm breit; ihre
/// Naht misst gut 80 cm. Gespiegelt kamen sie auf 11,2 m.
#[test]
fn stehkragen_haben_ihre_laenge() {
    for name in ["bluse2_v26", "bluse2_v29", "bluse3_v1"] {
        let m = laden(name);
        let ev = auswertung(&m);
        let kragen: Vec<_> = m
            .pieces()
            .into_iter()
            .filter(|p| p.name.as_deref().unwrap_or("").contains("Stehkragen"))
            .collect();
        assert_eq!(kragen.len(), 2, "{name}");
        for k in kragen {
            let umriss = ev.stueck_umriss(k).unwrap();
            let naht_mm = ring_laenge(&ev, &umriss) * ev.mm_pro_einheit();
            assert!(
                (700.0..1000.0).contains(&naht_mm),
                "{name} / {}: {naht_mm:.1} mm Naht",
                k.name.as_deref().unwrap_or("?")
            );
        }
    }
}

/// Am Übergang in die Szene (y nach oben) steht ein Teil aufrecht: die
/// Ärmelkugel liegt über dem Saum. In der Datei ist es umgekehrt — dort hat
/// die Kugel das kleinere y.
#[test]
fn szene_steht_aufrecht() {
    let m = laden("bluse2_v26");
    let ev = auswertung(&m);
    let aermel = m
        .pieces()
        .into_iter()
        .find(|p| p.name.as_deref() == Some("Bluse 2 - Aermel Kurz Links"))
        .expect("Ärmel");
    let umriss = ev.stueck_umriss(aermel).unwrap();
    let achse = Achse {
        faktor: ev.mm_pro_einheit(),
    };
    // Höchster Punkt der Kugel und tiefster des Saums, in der Datei.
    let kugel = umriss.iter().copied().reduce(|a, b| if b.y < a.y { b } else { a }).unwrap();
    let saum = umriss.iter().copied().reduce(|a, b| if b.y > a.y { b } else { a }).unwrap();
    assert!(kugel.y < saum.y);
    assert!(achse.in_szenene(kugel)[1] > achse.in_szenene(saum)[1]);
    assert_eq!(Achse { faktor: 10.0 }.in_szenene(Pt::new(1.0, 2.0)), [10.0, -20.0, 0.0]);
}

/// Kein Umriss ist länger als zweimal der Umfang seiner Hülle — sonst läuft
/// er hin und her. Ein schmaler, gebogener Streifen (Trim an der
/// Plunge-Kante) kommt auf das 1,5-Fache, ein Kreis auf das 0,8-Fache; die
/// gespiegelten Bögen brachten die Kragen auf das 5,3-Fache.
#[test]
fn kein_umriss_laeuft_hin_und_her() {
    for name in ALLE {
        let m = laden(name);
        let ev = auswertung(&m);
        for t in m.pieces() {
            let umriss = ev.stueck_umriss(t).unwrap();
            if umriss.len() < 3 {
                continue;
            }
            let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
            for p in &umriss {
                x0 = x0.min(p.x);
                x1 = x1.max(p.x);
                y0 = y0.min(p.y);
                y1 = y1.max(p.y);
            }
            let huelle = 2.0 * ((x1 - x0) + (y1 - y0));
            let laenge = ring_laenge(&ev, &umriss);
            assert!(
                laenge <= 2.0 * huelle,
                "{name} / {}: Umriss {laenge:.1} gegen Hüllumfang {huelle:.1}",
                t.name.as_deref().unwrap_or("?")
            );
        }
    }
}
