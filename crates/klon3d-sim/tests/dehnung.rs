//! Acceptance for the strain map (stage 5, task #17).
//!
//! The map is the second inspection measure of stage 5: per face, how much the
//! cloth is stretched in its two principal directions, summarised per piece.
//!
//! Checked in this order:
//!
//! 1. **scale, exactly** — a flat piece mapped isometrically reads 0 % in both
//!    directions; scaled by 1.12 in x it reads exactly 12 % and −0 % ; the rest
//!    area is the pattern area times `(1 − r)²` to 1e-12;
//! 2. **the limit is applied** — a flat piece at 12 % counts every face as over
//!    the limit, at 5 % none;
//! 3. **reduction is carried, not hidden** — the same face reads 18.52 % against
//!    the reduced rest shape and 12 % against the pattern when cut 5.5 %
//!    smaller, exactly `(1 + e)(1 − r) − 1`;
//! 4. **CSV** — header, one line per face, six fields everywhere, piece repeated;
//! 5. **the solved tube** (release only) — the larger principal strain hits the
//!    analytic `1/(1 − r) − 1` = 5.82 % within the 4a tolerance of 0.5
//!    percentage points, and the smaller one stays near zero: a tube does not
//!    stretch along its axis.
//!
//! Tests 1–4 run without CLO data. The tube needs no data either but is slow in
//! the debug build, so it is `ignore`d there like the 4a acceptance.

use klon3d_sim::dehnung::{muster_dehnung, Karte, UEBER_DEHNUNG};
use klon3d_sim::roehre::{rohr_netz, Roehre};
use klon3d_sim::szene::Teilvorgabe;
use klon3d_sim::{Simparameter, Szene, RADICAL_RUBBER_04};

/// Masseinheit: Breite und Höhe des flachen Prüfstücks (m).
const BREITE: f64 = 0.56;
const HOEHE: f64 = 0.28;

/// Flaches Stück, in x um `faktor` gestreckt, sonst unverändert. Ohne Körper:
/// Dehnung ist eine Eigenschaft des Stoffs allein.
fn flach(faktor: f64, reduktion: f64) -> Szene {
    let netz = rohr_netz(BREITE, HOEHE, 10.0).expect("Rechteck vernetzbar");
    let vg = Teilvorgabe {
        material: RADICAL_RUBBER_04,
        reduktion,
    };
    let lage = move |_: usize, q: [f64; 2]| [q[0] * 1e-3 * faktor, q[1] * 1e-3, 0.0];
    Szene::aus_netz(&netz, &|_| vg, &lage, Vec::new(), Simparameter::default())
}

/// 1. Der Maßstab: ohne Streckung 0 %, mit 1,12 genau 12 % in der größten und
/// 0 % in der kleinsten Hauptrichtung. Damit ist festgelegt, was „Prozent"
/// hier heißt (λ − 1, nicht logarithmisch).
#[test]
fn flach_ungestreckt_ist_null_und_gestreckt_ist_die_zahl_selbst() {
    let k = Karte::messe(&flach(1.0, 0.0));
    assert_eq!(k.je_flaeche.len(), k.je_teil[0].flaechen);
    for (i, f) in k.je_flaeche.iter().enumerate() {
        assert!(f.groesste.abs() < 1e-12, "Fläche {i}: {:.3e}", f.groesste);
        assert!(f.kleinste.abs() < 1e-12, "Fläche {i}: {:.3e}", f.kleinste);
    }
    assert_eq!(k.je_teil[0].ueber, 0);

    let k = Karte::messe(&flach(1.12, 0.0));
    for (i, f) in k.je_flaeche.iter().enumerate() {
        assert!(
            (f.groesste - 0.12).abs() < 1e-12,
            "Fläche {i}: {:.12}",
            f.groesste
        );
        assert!(
            f.kleinste.abs() < 1e-12,
            "Fläche {i} kleinste: {:.3e}",
            f.kleinste
        );
        assert!((f.muster - 0.12).abs() < 1e-12);
    }
    let g = k.gesamt();
    assert!(
        (g.min - 12.0).abs() < 1e-10 && (g.max - 12.0).abs() < 1e-10,
        "{g:?}"
    );
}

/// 2. Die Ruhefläche ist die Musterfläche mal `(1 − r)²` — die Reduktion
/// schrumpft beide Richtungen. Geprüft wird gegen das 2D-Rechteck, also gegen
/// eine Zahl, die nicht aus der Szene kommt.
#[test]
fn ruheflaeche_ist_die_musterflaeche_mal_der_reduktion_im_quadrat() {
    for reduktion in [0.0, 0.055, 0.10] {
        let k = Karte::messe(&flach(1.0, reduktion));
        let soll = BREITE * HOEHE * (1.0 - reduktion).powi(2);
        let ist = k.je_teil[0].ruheflaeche_m2;
        assert!(
            (ist / soll - 1.0).abs() < 1e-12,
            "r = {reduktion}: {ist:.15} gegen {soll:.15}"
        );
        let summe: f64 = k.je_flaeche.iter().map(|f| f.ruheflaeche_m2).sum();
        assert!(
            (summe / ist - 1.0).abs() < 1e-12,
            "Summe der Flächen {summe:.15} gegen {ist:.15}"
        );
    }
}

/// 3. Die Grenze greift: 12 % zählt jedes Dreieck als überdehnt, 5 % keines.
/// Das ist die Zahl, die im Bericht steht — nicht das Etikett „überdehnt".
///
/// Der Fall *genau auf der Grenze* wird bewusst nicht geprüft: `1.10/1.00 − 1`
/// ist in `f64` nicht exakt 0,1, ein Test darauf prüfte die Gleitkommarundung
/// und nicht die Grenze.
#[test]
fn grenze_wird_gezaehlt_und_nicht_geschaetzt() {
    let k = Karte::messe(&flach(1.12, 0.0));
    let t = &k.je_teil[0];
    assert_eq!(
        t.ueber,
        t.flaechen,
        "12 % > {:.0} %: alle Flächen zählen",
        UEBER_DEHNUNG * 100.0
    );

    let k = Karte::messe(&flach(1.05, 0.0));
    let t = &k.je_teil[0];
    assert_eq!(
        t.ueber,
        0,
        "5 % < {:.0} %: keine Fläche zählt",
        UEBER_DEHNUNG * 100.0
    );
}

/// 4. Reduktion wird mitgeführt, nicht versteckt: dasselbe gereckte Stück
/// liest gegen die reduzierte Ruhelage mehr und gegen das Muster wieder den
/// Ausgangswert. Die Umrechnung ist exakt, nicht genähert.
#[test]
fn reduktion_verschiebt_die_bezugslaenge_und_das_muster_bleibt() {
    let reduktion = 0.055;
    let k = Karte::messe(&flach(1.12, reduktion));
    let e_ruhe_soll = 1.12 / (1.0 - reduktion) - 1.0;
    for (i, f) in k.je_flaeche.iter().enumerate() {
        assert!(
            (f.groesste - e_ruhe_soll).abs() < 1e-12,
            "Fläche {i}: {:.12}",
            f.groesste
        );
        // Zurück auf das Muster: die Streckung 1,12 bleibt.
        assert!(
            (f.muster - 0.12).abs() < 1e-12,
            "Fläche {i} Muster: {:.12}",
            f.muster
        );
    }
    let t = &k.je_teil[0];
    assert_eq!(t.reduktion, reduktion);
    assert!((t.verteilung.median - e_ruhe_soll * 100.0).abs() < 1e-10);
    assert!(
        (t.muster.median - 12.0).abs() < 1e-10,
        "Muster-Median {:.6}",
        t.muster.median
    );

    // Und die Umrechnung selbst, an drei Punkten, die sich ohne Szene
    // nachrechnen lassen.
    for (e, r) in [
        (0.0, 0.0),
        (1.0 / (1.0 - 0.055) - 1.0, 0.055),
        (0.111_111_111_111_1, 0.10),
    ] {
        assert!(
            muster_dehnung(e, r).abs() < 1e-12,
            "e = {e:.12}, r = {r}: {:.3e}",
            muster_dehnung(e, r)
        );
    }
}

/// 5. CSV: Kopfzeile, je Fläche eine Zeile, überall sechs Felder, der Teilname
/// wiederholt — die Datei ist ohne die Szene lesbar.
#[test]
fn csv_hat_je_flaeche_eine_zeile_mit_teil_und_beiden_richtungen() {
    let s = flach(1.12, 0.055);
    let k = Karte::messe(&s);
    let csv = k.csv(&s);
    let mut zeilen = csv.lines();
    assert_eq!(
        zeilen.next().unwrap(),
        "dreieck;teil;flaeche_cm2;groesste_prozent;kleinste_prozent;muster_prozent"
    );
    let rest: Vec<&str> = zeilen.collect();
    assert_eq!(rest.len(), s.dreiecke.len(), "eine Zeile je Dreieck");
    for z in &rest {
        let f: Vec<&str> = z.split(';').collect();
        assert_eq!(f.len(), 6, "{z}");
        assert!(!f[1].is_empty(), "Teilname fehlt: {z}");
        // Prozentzahlen mit Vorzeichen, damit Zunahme und Stauchung
        // unterscheidbar bleiben.
        for wert in &f[3..] {
            assert!(wert.starts_with(['+', '-']), "{z}: {wert}");
        }
    }
    // Die Fläche über alle Zeilen ist die Ruhefläche des Teils. Die Zeilen
    // sind auf 1e-4 cm² gerundet, die Summe weicht deshalb um bis zu
    // `n·5e-5` cm² ab — das ist die Toleranz, mehr nicht.
    let summe: f64 = rest
        .iter()
        .map(|z| z.split(';').nth(2).unwrap().parse::<f64>().unwrap())
        .sum::<f64>()
        / 1e4;
    let rundung = rest.len() as f64 * 5e-5 / 1e4;
    let abweichung = (summe - k.je_teil[0].ruheflaeche_m2).abs();
    assert!(
        abweichung <= rundung,
        "Summe {summe:.6} gegen {:.6} m², Abweichung {abweichung:.3e} über der Rundung {rundung:.3e}",
        k.je_teil[0].ruheflaeche_m2
    );
}

/// 6. Die gerechnete Röhre: die größte Hauptdehnung trifft die Rechnung
/// `1/(1 − r) − 1`, die kleinste bleibt bei null — axial dehnt die Röhre nicht.
///
/// Toleranz wie in der 4a-Abnahme (0,5 %-Punkte), damit beide dieselbe Zahl
/// meinen und nicht zwei verschiedene.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn roehre_trifft_die_rechnung_in_beiden_hauptrichtungen() {
    use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe};
    use klon3d_sim::CpuReferenz;

    let reduktion = 0.055;
    let r = Roehre {
        reduktion,
        ..Roehre::default()
    };
    let mut s = r.szene(param_ruhe()).expect("Röhre baubar");
    let l = bis_zur_ruhe(&mut s, &mut CpuReferenz::neu(), 1200, 1e-4, 10);
    assert!(l.ruhig, "nicht zur Ruhe gekommen: {l:?}");

    let k = Karte::messe(&s);
    let soll = (1.0 / (1.0 - reduktion) - 1.0) * 100.0;
    let g = k.gesamt();
    // Der Median der größten Hauptdehnung ist die Umfangsdehnung; der
    // Umfang selbst wird in der 4a-Abnahme geprüft, hier die Flächen.
    assert!(
        (g.median - soll).abs() <= 0.5,
        "Median {:.3} % gegen Rechnung {:.3} %",
        g.median,
        soll
    );
    // Axial: die kleinste Hauptdehnung bleibt nahe null, weder gestaucht noch
    // gestreckt (gemessen −0,09 % durch die Naht).
    let kleinste = k
        .je_flaeche
        .iter()
        .map(|f| f.kleinste * 100.0)
        .fold(f64::MIN, f64::max);
    assert!(
        kleinste.abs() <= 0.3,
        "axial größte Stauchung/Streckung {kleinste:+.3} %"
    );

    // Gegen das Muster gelesen liegt die Röhre wieder auf Musterlänge: der
    // Median ist 0 %.
    let m = k.gesamt_muster();
    assert!(m.median.abs() <= 0.5, "Muster-Median {:.3} %", m.median);
}
