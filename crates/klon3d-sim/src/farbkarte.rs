//! Value → colour for the inspection maps (stage 5).
//!
//! Both maps answer "where is it bad" with a number per point. A number per
//! point is hard to read on a garment; a colour is not. This module is the
//! bridge: a fixed scale, one colour per value.
//!
//! # Why the scale is fixed and not normalised per run
//!
//! Normalising to the largest value in the run makes every run look equally
//! bad: 3 % in one garment and 300 % in the next would both reach red. Then a
//! colour in a picture means nothing. So the bounds are constants
//! ([`DEHNUNG_VON`], [`DEHNUNG_BIS`], [`ABSTAND_VON`], [`ABSTAND_BIS`]), they
//! are printed next to the map and they do not depend on the scene.
//!
//! # The ramp
//!
//! Blue → green → yellow → red, five stops, linear between them. That is the
//! usual reading for "cold to hot" and it survives colour blindness better than
//! a red-green ramp would if it were red-green only; the blue and yellow ends
//! stay distinguishable. Nothing is claimed beyond a guide to the eye — the
//! number in `dehnung.csv` / `abstand.csv` is the measurement.

/// A colour, one byte per channel.
pub type Rgb = [u8; 3];

/// Stops of the ramp from low to high. Five, so that the middle is a real
/// colour (yellow) and not a wash of two mixed ones.
const STUFEN: [Rgb; 5] = [
    [38, 70, 160],  // blue
    [50, 140, 130], // teal
    [110, 170, 70], // green
    [235, 170, 45], // yellow
    [190, 40, 45],  // red
];

/// Neutral grey for a value that is not a number.
pub const UNGUELTIG: Rgb = [140, 140, 140];

/// Strain scale: from this value (blue) to that value (red), in percent.
pub const DEHNUNG_VON: f64 = 0.0;
pub const DEHNUNG_BIS: f64 = 20.0;

/// Distance scale: from this value (blue) to that value (red), in mm.
///
/// The distance map starts at zero (lying on the skin) and runs upward; 30 mm
/// is about a sleeve cap standing off, which is the fault the map is meant to
/// show.
pub const ABSTAND_VON: f64 = 0.0;
pub const ABSTAND_BIS: f64 = 30.0;

/// Comparison scale: from this value (blue) to that value (red), in mm of
/// surface distance between two meshes.
///
/// The lower bound is **not** zero, and that is the point: a difference below
/// [`VERGLEICH_VON`] cannot be told apart from the scatter of the reference
/// implementation itself. Measured on the one comparison where it could be
/// measured — the same project computed twice with a different fabric — the
/// surface distance had a median of **2.14 mm** (90 % 9.49, max 58.05) between
/// two runs that differ only in the fabric. So the ramp starts at 2 mm:
/// everything at or below reads as the cold end colour, which says "inside the
/// noise, no statement", and only above it does the map claim a deviation.
///
/// That 2 mm is a **judgement from one measurement**, not a universal
/// constant. A comparison on another garment, another pose or another mesh
/// resolution has its own scatter, and the map would need its own floor. The
/// value is printed with the run so that the reader sees which floor is in
/// effect.
pub const VERGLEICH_VON: f64 = 2.0;

/// Upper end of the comparison scale, in mm. 30 mm is the same reading as the
/// distance map: a sleeve cap standing off the arm by that much is the fault a
/// comparison is meant to expose.
pub const VERGLEICH_BIS: f64 = 30.0;

/// Colour for `wert` on the ramp between `von` and `bis`.
///
/// Values outside the range are clamped to the end colours, so a map never
/// needs a second scale for outliers. A NaN or an infinite value has no place
/// on the ramp and reads as [`UNGUELTIG`] — visible, and clearly not a value.
pub fn farbe(wert: f64, von: f64, bis: f64) -> Rgb {
    if !wert.is_finite() || !von.is_finite() || !bis.is_finite() || bis <= von {
        return UNGUELTIG;
    }
    let t = ((wert - von) / (bis - von)).clamp(0.0, 1.0);
    // Position between two stops, `n` stops meaning `n − 1` intervals.
    let skala = t * (STUFEN.len() - 1) as f64;
    let i = (skala.floor() as usize).min(STUFEN.len() - 2);
    mische(STUFEN[i], STUFEN[i + 1], skala - i as f64)
}

/// Linear mix of two colours, `t` = 0 gives `a`, `t` = 1 gives `b`.
fn mische(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let k = |i: usize| {
        (a[i] as f64 + (b[i] as f64 - a[i] as f64) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [k(0), k(1), k(2)]
}

/// Colour of a strain value in percent, on [`DEHNUNG_VON`] … [`DEHNUNG_BIS`].
pub fn dehnung(prozent: f64) -> Rgb {
    farbe(prozent, DEHNUNG_VON, DEHNUNG_BIS)
}

/// Colour of a distance value in mm, on [`ABSTAND_VON`] … [`ABSTAND_BIS`].
pub fn abstand(mm: f64) -> Rgb {
    farbe(mm, ABSTAND_VON, ABSTAND_BIS)
}

/// Colour of a surface distance between two meshes, in mm, on
/// [`VERGLEICH_VON`] … [`VERGLEICH_BIS`].
///
/// Below the lower bound — inside the scatter of the reference — the cold end
/// colour, never a colour further up the ramp.
pub fn vergleich(mm: f64) -> Rgb {
    farbe(mm, VERGLEICH_VON, VERGLEICH_BIS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_enden_sind_die_endfarben() {
        assert_eq!(farbe(DEHNUNG_VON, DEHNUNG_VON, DEHNUNG_BIS), STUFEN[0]);
        assert_eq!(farbe(DEHNUNG_BIS, DEHNUNG_VON, DEHNUNG_BIS), STUFEN[4]);
        // Darüber und darunter wird geklemmt, nicht neu skaliert.
        assert_eq!(farbe(-50.0, DEHNUNG_VON, DEHNUNG_BIS), STUFEN[0]);
        assert_eq!(farbe(1e9, DEHNUNG_VON, DEHNUNG_BIS), STUFEN[4]);
    }

    #[test]
    fn gleicher_wert_gleiche_farbe() {
        for v in [-5.0, 0.0, 3.7, 10.0, 20.0, 999.0] {
            assert_eq!(dehnung(v), dehnung(v));
        }
        // Zwei Werte, die auf dieselbe Stelle der Skala fallen, sind gleich.
        assert_eq!(farbe(5.0, 0.0, 10.0), farbe(10.0, 0.0, 20.0));
    }

    #[test]
    fn die_skala_ist_monoton() {
        // The ramp guarantees exactly three things, and only those are
        // asserted: blue falls from cold to warm without ever rising, the cold
        // end is dominated by blue, the warm end by red, and no two of the
        // five stops are the same colour. A monotone red channel does *not*
        // hold — yellow is brighter than red, so red dips at the warm end —
        // and green rises on the cold half. Asserting either would be a false
        // claim about the palette.
        let mut vorher = dehnung(DEHNUNG_VON);
        for i in 0..=100 {
            let v = DEHNUNG_VON + (DEHNUNG_BIS - DEHNUNG_VON) * i as f64 / 100.0;
            let c = dehnung(v);
            assert!(
                c[2] <= vorher[2],
                "Blau steigt bei {v}: {vorher:?} -> {c:?}"
            );
            vorher = c;
        }
        let kalt = dehnung(DEHNUNG_VON);
        let warm = dehnung(DEHNUNG_BIS);
        assert!(
            kalt[2] > kalt[0] && kalt[2] > kalt[1],
            "kalt ist blau: {kalt:?}"
        );
        assert!(
            warm[0] > warm[1] && warm[0] > warm[2],
            "warm ist rot: {warm:?}"
        );
        for i in 0..STUFEN.len() {
            for j in (i + 1)..STUFEN.len() {
                assert_ne!(STUFEN[i], STUFEN[j], "Stufe {i} und {j} gleich");
            }
        }
    }

    #[test]
    fn zwischen_zwei_stufen_wird_gemischt() {
        // Genau auf einer Stufe steht die Stufenfarbe.
        let m = farbe(
            DEHNUNG_VON + (DEHNUNG_BIS - DEHNUNG_VON) * 0.5,
            DEHNUNG_VON,
            DEHNUNG_BIS,
        );
        assert_eq!(m, STUFEN[2]);
        // Auf halbem Weg zwischen Stufe 0 und 1 steht der Mittelwert.
        let t = 0.25 * 1.0 / (STUFEN.len() - 1) as f64;
        let h = farbe(
            DEHNUNG_VON + (DEHNUNG_BIS - DEHNUNG_VON) * 2.0 * t,
            DEHNUNG_VON,
            DEHNUNG_BIS,
        );
        for i in 0..3 {
            let soll = ((STUFEN[0][i] as f64 + STUFEN[1][i] as f64) / 2.0).round() as u8;
            assert_eq!(h[i], soll, "Kanal {i}: {h:?}");
        }
    }

    #[test]
    fn ungueltige_werte_sind_grau_und_keine_farbe() {
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(dehnung(v), UNGUELTIG, "{v}");
        }
        // Eine leere Skala ist ein Programmierfehler, aber kein Absturz.
        assert_eq!(farbe(1.0, 5.0, 5.0), UNGUELTIG);
        assert_eq!(farbe(1.0, 5.0, 1.0), UNGUELTIG);
    }

    #[test]
    fn alle_kanaele_bleiben_im_bereich() {
        for i in 0..=1000 {
            let v = -100.0 + 400.0 * i as f64 / 1000.0;
            let c = dehnung(v);
            // u8 kann nicht ausserhalb liegen; geprüft wird, dass die Mischung
            // nicht in eine Richtung ausreisst, die die Stufen nicht hergeben.
            let (min, max) = STUFEN.iter().fold((255u8, 0u8), |(mn, mx), s| {
                (
                    mn.min(*s.iter().min().unwrap()),
                    mx.max(*s.iter().max().unwrap()),
                )
            });
            for k in 0..3 {
                assert!(c[k] >= min && c[k] <= max, "Kanal {k} = {} bei {v}", c[k]);
            }
        }
    }

    #[test]
    fn die_beiden_skalen_sind_getrennt() {
        // Dieselbe Zahl bedeutet in beiden Karten etwas anderes; die Grenzen
        // sind benannt und nicht identisch.
        assert_ne!((DEHNUNG_VON, DEHNUNG_BIS), (ABSTAND_VON, ABSTAND_BIS));
        assert_eq!(abstand(ABSTAND_VON), STUFEN[0]);
        assert_eq!(abstand(ABSTAND_BIS), STUFEN[4]);
        // 20 % Dehnung ist rot, 20 mm Abstand ist noch nicht am Ende.
        assert_eq!(dehnung(DEHNUNG_BIS), STUFEN[4]);
        assert_ne!(abstand(20.0), STUFEN[4]);
    }
}
