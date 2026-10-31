//! Die 58 Maße der CLO-Körperformung (Körpertyp Female) auf einem festen Netz.
//!
//! Gebaut nach der Spezifikation `spez/messung.md`
//! (Abschnitte 1–6) und geprüft gegen `pruefwerte.json`. Die Umfänge
//! (Typ 0) kommen aus [`crate::massband`]; hier stehen die übrigen
//! Verfahren:
//!
//! * **Wege auf der Oberfläche** (Typ 1 und 5, 1.7/1.8): eine Ebene durch
//!   Anfangs- und Endpunkt, Schnittpunkte mit den Netzkanten, Lauf von Punkt
//!   zu Punkt innerhalb einer Linse um die Endpunkte, danach einseitig konvex
//!   gemacht. Rückenwege meiden die Vorderseite und umgekehrt.
//! * **Höhen** (Typ 2), **Geraden** (Typ 4), **Differenzen** (Typ 6) und
//!   **Summen** (Typ 7).
//! * **Berechnete Punkte** −2…−9 (2.2): Kreuzungen von Rohketten und
//!   Rohlinien mit der Mittelebene bzw. einem Punkt.
//!
//! Gemessen wird in der Reihenfolge 0…57 mit 8 vor 7 (Abschnitt 5); jede
//! Quelle ist vor ihrem Nutzer gemessen. Alle Werte in Zentimetern auf
//! zentrierten Punkten ([`crate::massband::zentriert_cm`]).

use crate::massband::{
    abstand, ebene_drei, kantenschnitt, kreuz, normiert, orientierung, rumpfmasse, segment, sub,
    umfang, waagerecht, Messdaten, WAAGERECHT,
};
use crate::Punkt3;

/// Zahl der Maße.
pub const ANZAHL: usize = 58;

/// Namen der Maße in Platzreihenfolge (`formung.md` 3).
pub const NAMEN: [&str; ANZAHL] = [
    "HEIGHT_Total",
    "WEIGHT_Total",
    "CIRCUMFERENCE_Bust",
    "CIRCUMFERENCE_MidNeck",
    "CIRCUMFERENCE_NeckBase",
    "CIRCUMFERENCE_UnderBust",
    "CIRCUMFERENCE_Waist",
    "CIRCUMFERENCE_HighHip",
    "CIRCUMFERENCE_LowHip",
    "CIRCUMFERENCE_Thigh",
    "CIRCUMFERENCE_MidThigh",
    "CIRCUMFERENCE_Knee",
    "CIRCUMFERENCE_Calf",
    "CIRCUMFERENCE_Ankle",
    "CIRCUMFERENCE_Armhole",
    "CIRCUMFERENCE_Bicep",
    "CIRCUMFERENCE_Elbow",
    "CIRCUMFERENCE_Wrist",
    "HEIGHT_HPS",
    "HEIGHT_Shoulder",
    "HEIGHT_ShoulderDrop",
    "HEIGHT_APEX",
    "HEIGHT_UnderBust",
    "HEIGHT_Waist",
    "HEIGHT_HighHip",
    "HEIGHT_LowHip",
    "HEIGHT_Crotch",
    "HEIGHT_Thigh",
    "HEIGHT_MidThigh",
    "HEIGHT_Knee",
    "HEIGHT_Calf",
    "HEIGHT_Ankle",
    "LENGTH_Bust_Width",
    "LENGTH_Bust_Depth",
    "LENGTH_AcrossFront",
    "LENGTH_AcrossBack",
    "LENGTH_AcrossShoulder",
    "LENGTH_CBNeck_Shoulder",
    "LENGTH_CFNeck_AcrossFront",
    "LENGTH_CBNeck_AcrossBack",
    "LENGTH_CFNeck_Waist",
    "LENGTH_CBNeck_Waist",
    "LENGTH_Waist_HighHip",
    "LENGTH_Waist_LowHip",
    "LENGTH_APEX_APEX",
    "LENGTH_HPS_APEX",
    "LENGTH_Halter",
    "LENGTH_Arm",
    "LENGTH_CBNeck_Wrist",
    "LENGTH_Total_Rise",
    "LENGTH_Front_Rise",
    "LENGTH_Back_Rise",
    "LENGTH_CrotchDepth",
    "LENGTH_CrotchWidth",
    "LENGTH_Crothch_MidThigh",
    "LENGTH_VerticalTrunk_Front",
    "LENGTH_VerticalTrunk_Back",
    "LENGTH_VerticalTrunk",
];

/// Platz eines Maßes nach Namen.
pub fn platz(name: &str) -> Option<usize> {
    NAMEN.iter().position(|n| *n == name)
}

/// Die gewählten Kandidaten eines Messdurchgangs (Abschnitt 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wahl {
    pub brustpaar: (usize, usize),
    pub unterbrust: usize,
    pub taille: usize,
    pub huefte_oben: usize,
    pub huefte_unten: usize,
    pub wade: (usize, usize),
}

/// Ergebnis eines Messdurchgangs.
#[derive(Clone, Debug)]
pub struct Messung {
    /// Die 58 Werte, cm (Gewicht 0).
    pub werte: [f64; ANZAHL],
    /// Werte der Slots 0 und 1 je Maß; `NaN`, wo es keinen Slot gibt.
    pub slots: [[f64; 2]; ANZAHL],
    /// Punkte der Ergebnislinie je Weg-Slot (0, wo es keinen Weg gibt).
    pub wegpunkte: [[usize; 2]; ANZAHL],
    pub wahl: Wahl,
}

/// Landmarken, die die Regeln der Wege vergleichen.
struct Lm {
    l5: usize,
    l8: usize,
    l9: usize,
    l18: Option<usize>,
    l19: Option<usize>,
}

/// Ein Eintrag einer Slotliste: Punkt und, wenn es ein Netzpunkt ist,
/// seine Nummer (für die Sonderregeln der Ebene).
type Eintrag = (Punkt3, Option<usize>);

fn mitte(a: Punkt3, b: Punkt3) -> Punkt3 {
    [
        (a[0] + b[0]) / 2.0,
        (a[1] + b[1]) / 2.0,
        (a[2] + b[2]) / 2.0,
    ]
}

/// Länge einer offenen Polylinie.
fn polylinie(p: &[Punkt3]) -> f64 {
    p.windows(2).map(|w| abstand(w[0], w[1])).sum()
}

/// Teilweg W(Slot, modus) (1.7). Hängt an `roh` den gelaufenen Weg, an
/// `erg` den einseitig konvexen Weg an.
#[allow(clippy::too_many_arguments)]
fn teilweg(
    v: &[Punkt3],
    d: &Messdaten,
    e: &[Eintrag],
    modus: usize,
    seg: i32,
    hinten: bool,
    lm: &Lm,
    roh: &mut Vec<Punkt3>,
    erg: &mut Vec<Punkt3>,
) {
    let k = if modus == 2 {
        e.len() - 2
    } else if e.len() > 2 {
        3
    } else {
        2
    };
    let p: Vec<Punkt3> = (0..k).map(|i| e[modus + i].0).collect();
    let (a, b) = (p[0], p[k - 1]);

    // 3. Ebene. Verglichen werden die ersten Einträge des Slots.
    let n = if k == 2 {
        let mut c = mitte(a, b);
        let (id0, id1) = (e[0].1, e[1].1);
        if id0 == Some(lm.l5) && (id1 == Some(lm.l8) || id1 == Some(lm.l9)) {
            c[1] -= 50.0;
            c[2] += 50.0;
        } else {
            if id1.is_some() && id1 == lm.l18 {
                c[0] -= 20.0;
            }
            if id1.is_some() && id1 == lm.l19 {
                c[0] += 20.0;
            }
            c[2] -= 100.0;
        }
        normiert(kreuz(sub(c, a), sub(b, a)))
    } else {
        normiert(kreuz(sub(p[1], p[0]), sub(p[2], p[0])))
    };
    let Some(n) = n else {
        roh.push(a);
        erg.push(a);
        return;
    };

    // 4. Vorrat: zuerst B, dann die Schnittpunkte in Kantenreihenfolge.
    let mut vorrat = vec![b];
    for q in kantenschnitt(v, d, a, n, seg) {
        if vorrat.iter().all(|x| abstand(*x, q) >= 0.001) {
            vorrat.push(q);
        }
    }

    // 5. Lauf.
    let mut w = vec![a];
    let mut r = if k == 2 {
        abstand(b, a)
    } else {
        abstand(p[1], a)
    };
    while !vorrat.is_empty() {
        let letzt = *w.last().unwrap();
        let mut wahl = None;
        let mut schranke = 20.0;
        for (i, c) in vorrat.iter().enumerate() {
            let c = *c;
            let zulaessig = if k == 3 {
                abstand(c, p[1]) <= r
            } else {
                abstand(c, b) <= r
                    && abstand(c, a) <= r
                    && !(hinten
                        && modus != 2
                        && ((abstand(c, a) < 10.0 && c[2] > a[2])
                            || (abstand(c, b) < 10.0 && c[2] > b[2])))
            };
            if zulaessig {
                let dd = abstand(c, letzt);
                if dd < schranke {
                    schranke = dd;
                    wahl = Some(i);
                }
            }
        }
        let Some(i) = wahl else { break };
        let c = vorrat.remove(i);
        if abstand(c, letzt) > 0.001 {
            w.push(c);
        }
        let jetzt = *w.last().unwrap();
        if k == 3 && abstand(p[1], jetzt) < 0.001 {
            r = abstand(p[1], b);
        }
        if abstand(b, jetzt) < 0.0001 {
            break;
        }
    }
    // 6.
    if abstand(*w.last().unwrap(), b) > 0.1 {
        w.push(b);
    }
    // 7.
    roh.extend_from_slice(&w);
    // 8. Einseitig konvex.
    let mut i = 2;
    while i < w.len() {
        let ok = orientierung(n, w[i - 2], w[i - 1], w[i]);
        if (!hinten && ok) || (hinten && !ok) {
            w.remove(i - 1);
            i = (i - 2).max(2) + 1;
        } else {
            i += 1;
        }
    }
    // 9.
    erg.extend_from_slice(&w);
}

/// Länge als Weg für einen Slot (1.8): Wert, Ergebnislinie, Rohlinie.
fn weg(
    v: &[Punkt3],
    d: &Messdaten,
    e: &[Eintrag],
    seg: i32,
    hinten: bool,
    lm: &Lm,
) -> (f64, Vec<Punkt3>, Vec<Punkt3>) {
    let mut roh = Vec::new();
    let mut erg = Vec::new();
    if e.len() < 4 {
        teilweg(v, d, e, 0, seg, hinten, lm, &mut roh, &mut erg);
    } else if seg == segment::KOPF {
        teilweg(v, d, e, 0, seg, hinten, lm, &mut roh, &mut erg);
        let ziel = e[1].0;
        let mut schranke = 100.0;
        while let Some(letzt) = erg.last() {
            let dd = abstand(*letzt, ziel);
            if dd > schranke {
                break;
            }
            erg.pop();
            schranke = dd;
        }
        teilweg(v, d, e, 1, seg, hinten, lm, &mut roh, &mut erg);
    } else {
        teilweg(v, d, e, 0, seg, hinten, lm, &mut roh, &mut erg);
        erg.pop();
        teilweg(v, d, e, 2, seg, hinten, lm, &mut roh, &mut erg);
    }
    (polylinie(&erg), erg, roh)
}

/// Kreuzung X(Linie, Q, nurX, hinten) (1.6); der letzte Treffer gilt.
fn kreuzung(linie: &[Punkt3], q: Punkt3, nur_x: bool, hinten: bool) -> Punkt3 {
    let mut r = [0.0; 3];
    for t in 0..linie.len().saturating_sub(1) {
        let (pt, pn) = (linie[t], linie[t + 1]);
        if !nur_x {
            let weiter = if hinten {
                pt[2] <= q[2] - 1.0
            } else {
                pt[2] >= q[2] + 1.0
            };
            if !weiter {
                continue;
            }
        }
        let d0 = pt[0] - q[0];
        let d1 = pn[0] - q[0];
        if d0 * d1 <= 0.0 {
            let s = d0.abs() + d1.abs();
            for a in 0..3 {
                r[a] = (pt[a] * d1.abs() + pn[a] * d0.abs()) / s;
            }
        }
    }
    r
}

/// Misst alle 58 Maße auf zentrierten Punkten (cm).
pub fn messen(v: &[Punkt3], d: &Messdaten) -> Result<Messung, String> {
    d.pruefe(v.len())?;
    let l = |zeile: usize| -> Result<usize, String> {
        d.landmarke(zeile)
            .ok_or_else(|| format!("Landmarke L{zeile} fehlt in IndexSet"))
    };
    let lm = Lm {
        l5: l(5)?,
        l8: l(8)?,
        l9: l(9)?,
        l18: d.landmarke(18),
        l19: d.landmarke(19),
    };
    let pkt = |i: usize| -> Eintrag { (v[i], Some(i)) };
    let y = |i: usize| v[i][1].abs();

    let mut werte = [0.0f64; ANZAHL];
    let mut slots = [[f64::NAN; 2]; ANZAHL];
    let mut wegpunkte = [[0usize; 2]; ANZAHL];
    // Beidseitiges Maß: Slot 0 + Slot 1, gemittelt ausser beim Halsansatz.
    fn beide(
        j: usize,
        s0: f64,
        s1: f64,
        mitteln: bool,
        werte: &mut [f64; ANZAHL],
        slots: &mut [[f64; 2]; ANZAHL],
    ) {
        slots[j] = [s0, s1];
        werte[j] = if mitteln { 0.5 * (s0 + s1) } else { s0 + s1 };
    }

    // 0 Höhe, 1 Gewicht (nicht gemessen).
    werte[0] = y(l(0)?);
    werte[1] = 0.0;

    // 2, 5, 6, 8, 7: die Rumpfumfänge mit Kandidatenwahl.
    let r = rumpfmasse(v, d)?;
    let k = r.kandidaten.clone();
    werte[2] = r.brust.laenge;
    werte[5] = r.unterbrust.laenge;
    werte[6] = r.taille.laenge;
    werte[7] = r.highhip.laenge;
    werte[8] = r.huefte.laenge;
    let (a, b) = k.brustpaar;
    let (u, w, hh, h) = (k.unterbrust, k.taille, k.huefte_oben, k.huefte_unten);

    // 3 Halsumfang, geneigte Ebene durch drei Punkte.
    {
        let (p0, n) = ebene_drei(v, l(1)?, l(2)?, l(3)?).ok_or("Ebene MidNeck entartet")?;
        werte[3] = umfang(v, d, p0, n, segment::KOPF).laenge;
    }

    // 4 Halsansatz: vorderer und hinterer Bogen, Summe.
    {
        let s0 = weg(
            v,
            d,
            &[pkt(l(6)?), pkt(l(64)?), pkt(l(65)?), pkt(l(7)?)],
            segment::KOPF,
            true,
            &lm,
        );
        let s1 = weg(
            v,
            d,
            &[pkt(l(7)?), pkt(l(5)?), pkt(l(6)?)],
            segment::KOPF,
            true,
            &lm,
        );
        wegpunkte[4] = [s0.1.len(), s1.1.len()];
        beide(4, s0.0, s1.0, false, &mut werte, &mut slots);
    }

    // 9–11, 13: waagerecht durch einen Beinpunkt je Seite.
    for (j, z0, z1) in [(9, 31, 32), (10, 33, 34), (11, 35, 36), (13, 39, 40)] {
        let s0 = umfang(v, d, v[l(z0)?], WAAGERECHT, segment::BEIN_LINKS).laenge;
        let s1 = umfang(v, d, v[l(z1)?], WAAGERECHT, segment::BEIN_RECHTS).laenge;
        beide(j, s0, s1, true, &mut werte, &mut slots);
    }

    // 12 Wade: je Seite der Kandidat mit dem grössten Umfang.
    let mut wade = [0usize; 2];
    {
        let mut s = [0.0; 2];
        for (seite, (zeilen, seg)) in [
            ([37, 96, 98], segment::BEIN_LINKS),
            ([38, 97, 99], segment::BEIN_RECHTS),
        ]
        .into_iter()
        .enumerate()
        {
            let mut wahl = None;
            let mut schranke = 0.0;
            for z in zeilen {
                let Some(e) = d.landmarke(z) else { continue };
                let uu = waagerecht(v, d, v[e], seg);
                if uu > schranke {
                    schranke = uu;
                    wahl = Some(e);
                }
            }
            let c = wahl.ok_or("kein Wadenkandidat")?;
            wade[seite] = c;
            s[seite] = umfang(v, d, v[c], WAAGERECHT, seg).laenge;
        }
        beide(12, s[0], s[1], true, &mut werte, &mut slots);
    }

    // 14 Armloch: zwei Teilwege je Seite, alle Kanten, Rückseite.
    {
        let s0 = weg(
            v,
            d,
            &[
                pkt(l(8)?),
                pkt(l(12)?),
                pkt(l(16)?),
                pkt(l(14)?),
                pkt(l(8)?),
            ],
            segment::ALLE,
            true,
            &lm,
        );
        let s1 = weg(
            v,
            d,
            &[
                pkt(l(9)?),
                pkt(l(13)?),
                pkt(l(17)?),
                pkt(l(15)?),
                pkt(l(9)?),
            ],
            segment::ALLE,
            true,
            &lm,
        );
        wegpunkte[14] = [s0.1.len(), s1.1.len()];
        beide(14, s0.0, s1.0, true, &mut werte, &mut slots);
    }

    // 15–17: geneigte Ebenen am Arm.
    for (j, z0, z1) in [
        (15, [41, 42, 43], [44, 45, 46]),
        (16, [49, 50, 51], [52, 53, 54]),
        (17, [57, 58, 59], [60, 61, 62]),
    ] {
        let mut s = [0.0; 2];
        for (seite, (z, seg)) in [(z0, segment::ARM_LINKS), (z1, segment::ARM_RECHTS)]
            .into_iter()
            .enumerate()
        {
            let (p0, n) = ebene_drei(v, l(z[0])?, l(z[1])?, l(z[2])?)
                .ok_or_else(|| format!("Ebene {} entartet", NAMEN[j]))?;
            s[seite] = umfang(v, d, p0, n, seg).laenge;
        }
        beide(j, s[0], s[1], true, &mut werte, &mut slots);
    }

    // 18–31 Höhen.
    beide(18, y(l(6)?), y(l(7)?), true, &mut werte, &mut slots);
    beide(19, y(l(8)?), y(l(9)?), true, &mut werte, &mut slots);
    beide(
        20,
        (v[l(6)?][1] - v[l(8)?][1]).abs(),
        (v[l(7)?][1] - v[l(9)?][1]).abs(),
        true,
        &mut werte,
        &mut slots,
    );
    beide(21, y(a), y(b), true, &mut werte, &mut slots);
    werte[22] = y(u);
    werte[23] = y(w);
    werte[24] = y(hh);
    werte[25] = y(h);
    werte[26] = y(l(30)?);
    beide(27, y(l(31)?), y(l(32)?), true, &mut werte, &mut slots);
    beide(28, y(l(33)?), y(l(34)?), true, &mut werte, &mut slots);
    beide(29, y(l(35)?), y(l(36)?), true, &mut werte, &mut slots);
    beide(30, y(wade[0]), y(wade[1]), true, &mut werte, &mut slots);
    beide(31, y(l(39)?), y(l(40)?), true, &mut werte, &mut slots);

    // Berechnete Punkte aus den Rohketten (2.2).
    let ub = &r.unterbrust.kette;
    let rand = |kleinster: bool| -> Punkt3 {
        let mut best = None;
        let mut schranke = if kleinster { 100.0 } else { -100.0 };
        for p in ub {
            if (kleinster && p[0] < schranke) || (!kleinster && p[0] > schranke) {
                schranke = p[0];
                best = Some(*p);
            }
        }
        best.unwrap_or([0.0; 3])
    };
    let m7 = rand(true);
    let m8 = rand(false);
    let m9 = kreuzung(ub, v[u], false, true);
    let m2 = kreuzung(&r.taille.kette, v[w], false, true);
    let m4 = kreuzung(&r.huefte.kette, v[h], false, false);

    // 32 Brustbreite: −7 und −8, z gemittelt.
    {
        let z = (m7[2] + m8[2]) / 2.0;
        werte[32] = abstand([m7[0], m7[1], z], [m8[0], m8[1], z]);
    }
    // 33 Brusttiefe [u, −9].
    werte[33] = abstand(v[u], m9);

    // 34, 35 Brust- und Rückenbreite; ihre Rohlinien liefern −5 und −6.
    let s34 = weg(v, d, &[pkt(l(12)?), pkt(l(13)?)], segment::ALLE, false, &lm);
    werte[34] = s34.0;
    wegpunkte[34][0] = s34.1.len();
    let s35 = weg(v, d, &[pkt(l(14)?), pkt(l(15)?)], segment::ALLE, true, &lm);
    werte[35] = s35.0;
    wegpunkte[35][0] = s35.1.len();
    let m5 = kreuzung(&s34.2, v[l(4)?], true, false);
    let m6 = kreuzung(&s35.2, v[l(5)?], true, false);

    // 36 Schulterbreite über den Nacken.
    {
        let s = weg(
            v,
            d,
            &[pkt(l(8)?), pkt(l(63)?), pkt(l(9)?)],
            segment::ALLE,
            true,
            &lm,
        );
        werte[36] = s.0;
        wegpunkte[36][0] = s.1.len();
    }
    // 37 7. Halswirbel zum Schulterpunkt, je Seite.
    {
        let s0 = weg(v, d, &[pkt(l(5)?), pkt(l(8)?)], segment::ALLE, false, &lm);
        let s1 = weg(v, d, &[pkt(l(5)?), pkt(l(9)?)], segment::ALLE, false, &lm);
        wegpunkte[37] = [s0.1.len(), s1.1.len()];
        beide(37, s0.0, s1.0, true, &mut werte, &mut slots);
    }
    // 38–43 Wege in der Mittelebene.
    for (j, e, hinten, seg) in [
        (38, vec![pkt(l(4)?), (m5, None)], false, segment::ALLE),
        (39, vec![pkt(l(5)?), (m6, None)], true, segment::ALLE),
        (40, vec![pkt(l(4)?), pkt(w)], false, segment::ALLE),
        (41, vec![pkt(l(5)?), (m2, None)], true, segment::ALLE),
        (42, vec![pkt(w), pkt(hh)], false, segment::ALLE),
        (43, vec![pkt(w), (m4, None)], false, segment::ALLE),
    ] {
        let s = weg(v, d, &e, seg, hinten, &lm);
        werte[j] = s.0;
        wegpunkte[j][0] = s.1.len();
    }
    // 44 Brustpunktabstand.
    werte[44] = abstand(v[a], v[b]);
    // 45 seitlicher Halspunkt zum Brustpunkt, je Seite.
    {
        let s0 = weg(v, d, &[pkt(l(6)?), pkt(a)], segment::ALLE, false, &lm);
        let s1 = weg(v, d, &[pkt(l(7)?), pkt(b)], segment::ALLE, false, &lm);
        wegpunkte[45] = [s0.1.len(), s1.1.len()];
        beide(45, s0.0, s1.0, true, &mut werte, &mut slots);
    }
    // 46 Halter.
    werte[46] = 2.0 * werte[45] + slots[4][1];
    // 47 Armlänge, je Seite.
    {
        let s0 = weg(
            v,
            d,
            &[pkt(l(8)?), pkt(l(47)?), pkt(l(49)?), pkt(l(57)?)],
            segment::ARM_LINKS,
            true,
            &lm,
        );
        let s1 = weg(
            v,
            d,
            &[pkt(l(9)?), pkt(l(48)?), pkt(l(52)?), pkt(l(60)?)],
            segment::ARM_RECHTS,
            true,
            &lm,
        );
        wegpunkte[47] = [s0.1.len(), s1.1.len()];
        beide(47, s0.0, s1.0, true, &mut werte, &mut slots);
    }
    // 48 Nacken zum Handgelenk.
    werte[48] = werte[37] + werte[47];
    // 49, 50 Schrittlängen im Rumpfsegment.
    {
        let s = weg(
            v,
            d,
            &[pkt(w), pkt(l(30)?), (m2, None)],
            segment::RUMPF,
            true,
            &lm,
        );
        werte[49] = s.0;
        wegpunkte[49][0] = s.1.len();
        let s = weg(v, d, &[pkt(w), pkt(l(30)?)], segment::RUMPF, false, &lm);
        werte[50] = s.0;
        wegpunkte[50][0] = s.1.len();
    }
    werte[51] = werte[49] - werte[50];
    werte[52] = werte[23] - werte[26];
    // 53 [−4, h].
    werte[53] = abstand(m4, v[h]);
    werte[54] = werte[26] - werte[28];
    // 55, 56 vom seitlichen Halspunkt zum Schritt, vorn und hinten.
    for (j, hinten) in [(55, false), (56, true)] {
        let s0 = weg(v, d, &[pkt(l(6)?), pkt(l(30)?)], segment::ALLE, hinten, &lm);
        let s1 = weg(v, d, &[pkt(l(7)?), pkt(l(30)?)], segment::ALLE, hinten, &lm);
        wegpunkte[j] = [s0.1.len(), s1.1.len()];
        beide(j, s0.0, s1.0, true, &mut werte, &mut slots);
    }
    werte[57] = werte[55] + werte[56];

    Ok(Messung {
        werte,
        slots,
        wegpunkte,
        wahl: Wahl {
            brustpaar: (a, b),
            unterbrust: u,
            taille: w,
            huefte_oben: hh,
            huefte_unten: h,
            wade: (wade[0], wade[1]),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namen_und_plaetze() {
        assert_eq!(NAMEN.len(), 58);
        assert_eq!(platz("CIRCUMFERENCE_Waist"), Some(6));
        assert_eq!(platz("LENGTH_VerticalTrunk"), Some(57));
    }

    #[test]
    fn kreuzung_nimmt_den_letzten_treffer() {
        let linie = [[-2.0, 0.0, 0.0], [2.0, 0.0, 0.0], [-2.0, 1.0, 0.0]];
        let r = kreuzung(&linie, [0.0, 0.0, 0.0], true, false);
        assert!((r[0]).abs() < 1e-12 && (r[1] - 0.5).abs() < 1e-12, "{r:?}");
    }

    #[test]
    fn polylinie_ist_offen() {
        let p = [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 4.0, 0.0]];
        assert!((polylinie(&p) - 7.0).abs() < 1e-12);
    }
}
