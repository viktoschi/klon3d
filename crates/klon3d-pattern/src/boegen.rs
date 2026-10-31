//! Bögen.
//!
//! Seamly2D kennt zwei Bogentypen (`vpattern.cpp: ParseArcElement`):
//!
//! * `simple` nennt **beide** Winkel und läuft immer gegen den Uhrzeiger.
//! * `arcWithLength` nennt Startwinkel und **Länge**; Endwinkel und
//!   Laufrichtung folgen daraus.
//!
//! Fehlt ein Attribut, gelten Seamly2Ds Vorgaben (`ParseToolArc`,
//! `ParseToolArcWithLength`): `radius` 10, `angle1` 180, `angle2` 270,
//! `length` 10 — als Formeln in der Mustereinheit. Gemessen an sieben
//! solchen Bögen: Seamly2D führt den `simple`-Bogen ohne `angle2` bis 270,
//! den ohne `angle1` ab 180, den ohne `radius` mit 10 cm.
//!
//! Die gezeichnete Polylinie läuft wie Seamly2Ds Streckenzug
//! (`varc.cpp: getPoints`) immer gegen den Uhrzeiger: ein Bogen im
//! Uhrzeigersinn (`flipped`, also ein `arcWithLength` mit negativer Länge)
//! beginnt deshalb an seinem **Endwinkel**. Auf diese Reihenfolge verläßt
//! sich `piece_outline`.
//!
//! ## Wohin ein Winkel zeigt
//!
//! Der Punkt eines Bogens beim Winkel `w` ist `Mitte + r · (cos w, −sin w)`
//! — dieselbe Übersetzung wie [`crate::geom::vektor`], weil die Auswertung
//! in den gespeicherten Werten rechnet (y nach unten) und Seamly2D den Punkt
//! über `QLineF::setAngle` bildet. `schnitt.py` schreibt `+sin`, rechnet aber
//! y nach oben (`_base_point` nimmt `−y`); beim Übertragen blieb das `+`
//! zunächst stehen. Bis zum 19.10.2026 lag deshalb jeder Bogen an der
//! Waagerechten durch seine Mitte gespiegelt: die Kragen der Blusen
//! (Bögen um 270°, unter der Mitte) hingen als Halbkreis **über** ihr, die
//! Kontur sprang zwischen den richtigen Endpunkten und den gespiegelten
//! Bögen hin und her — 11,2 m Naht statt 0,9 m. Kein Sollwert hing an einem
//! Bogen, deshalb fiel es erst im Betrachter auf.

use crate::geom::{vektor, Auswertung, GeomError, Pt};
use crate::kurven::PX_PRO_MM;
use crate::model::{Arc, Point};

/// `M_2PI` — Seamly2D rechnet mit dieser Konstanten, nicht mit `2 * PI`.
const M_2PI: f64 = 6.283185307179586;

/// Die fünf Grössen, aus denen ein Bogen lebt.
#[derive(Debug, Clone)]
pub struct Bogendaten {
    pub mitte: Pt,
    /// Id der Mitte — der Name Arc_<Mitte>_<id> nennt sie.
    pub mitte_id: String,
    pub radius: f64,
    /// Startwinkel in Grad.
    pub f1: f64,
    /// Endwinkel in Grad.
    pub f2: f64,
    /// Läuft im Uhrzeigersinn.
    pub flipped: bool,
}

impl Auswertung<'_> {
    /// Mittelpunkt, Radius, Start- und Endwinkel eines Bogens.
    pub fn bogen_daten(&self, b: &Arc) -> Result<Bogendaten, GeomError> {
        let mitte_id = b.center.clone().unwrap_or_default();
        let mitte = self.bez_pt(b.center.as_deref(), "center")?;
        let r = self.formel_zahl(b.radius.as_deref(), 10.0);
        let f1 = self.formel_zahl(b.angle1.as_deref(), 180.0);
        match b.kind.as_deref() {
            Some("simple") => Ok(Bogendaten {
                mitte,
                mitte_id,
                radius: r,
                f1: Self::seamly_normalize(f1),
                f2: Self::seamly_normalize(self.formel_zahl(b.angle2.as_deref(), 270.0)),
                flipped: false,
            }),
            Some("arcWithLength") => {
                if r == 0.0 {
                    // Seamly2D teilt hier durch 0 und führt den Endwinkel
                    // als "keine Zahl".
                    return Err(GeomError::Geometrie {
                        kind: "Bogen",
                        grund: format!("arcWithLength mit Radius 0 (id {})", b.id),
                    });
                }
                let laenge = self.formel_zahl(b.length.as_deref(), 10.0);
                // Radius und Länge gehen in **Pixeln** in die Rechnung
                // (`vtoolarcwithlength.cpp: Create`).
                let (f2, flipped) =
                    Self::seamly_find_f2(self.zu_px(r), f1, self.zu_px(laenge));
                Ok(Bogendaten { mitte, mitte_id, radius: r, f1, f2, flipped })
            }
            andere => Err(GeomError::Geometrie {
                kind: "Bogen",
                grund: format!(
                    "nicht implementierter Bogen-Typ: {}",
                    andere.unwrap_or("(kein Typ)")
                ),
            }),
        }
    }

    /// Ein Winkel, wie Seamly2D ihn für einen `simple`-Bogen ablegt.
    ///
    /// Quelle `def.cpp: normalize`, seit Commit 1217dd20 (15.05.2026) ruft
    /// `vtoolarc.cpp: Create` sie für Start- und Endwinkel. Ein Wert, dessen
    /// Betrag auf 1e-9 ein Vielfaches von 360 ist, wird 360 (auch −360, 720
    /// und 1e-10); nur 0 selbst bleibt 0. Sonst der Rest wie auf einer Uhr:
    /// −30 wird 330, 390 wird 30.
    pub fn seamly_normalize(wert: f64) -> f64 {
        let (start, ende) = (0.0f64, 360.0f64);
        if wert.abs() != start && (wert.abs() % ende) < 1e-9 {
            return ende;
        }
        let bereich = ende - start;
        let versatz = wert - start;
        (versatz - (versatz / bereich).floor() * bereich) + start
    }

    /// Ein Winkel, wie `QLineF` ihn zurückgibt: 0 bis unter 360 Grad.
    ///
    /// Der Winkel geht durch Kosinus, Sinus und Arkustangens; er kommt
    /// deshalb nicht Bit für Bit zurück, sondern auf etwa 1e-14 Grad.
    pub fn qline_winkel(grad: f64) -> f64 {
        let w = grad.to_radians();
        let (dx, dy) = (w.cos() * 100.0, -w.sin() * 100.0);
        let mut theta = (-dy).atan2(dx).to_degrees();
        if theta < 0.0 {
            theta += 360.0;
        }
        if (theta - 360.0).abs() * 1e12 <= theta.abs().min(360.0) {
            return 0.0;
        }
        theta
    }

    /// Endwinkel eines `arcWithLength` und ob er im Uhrzeigersinn läuft.
    ///
    /// Übernommen aus dem Quelltext (`varc.cpp: FindF2, MaxLength`;
    /// `vtoolarcwithlength.cpp: Create`), nicht nachempfunden:
    ///
    /// * eine negative Länge macht den Bogen `flipped`;
    /// * eine Länge ab dem vollen Umfang zählt als der volle Umfang — aber
    ///   nur die positive: eine negative wird **nicht** begrenzt, ihr Winkel
    ///   läuft über 360 Grad hinaus und der Bogen ist nur der Rest;
    /// * Bogenwinkel = |Länge / Radius| in Grad, bei `flipped` negativ;
    /// * Endwinkel = Startwinkel + Bogenwinkel, über `QLineF` auf 0 bis unter
    ///   360 Grad.
    ///
    /// Radius und Länge in Pixeln.
    pub fn seamly_find_f2(radius: f64, f1: f64, mut laenge: f64) -> (f64, bool) {
        let flipped = laenge < 0.0;
        let voll = M_2PI * radius;
        if laenge >= voll {
            laenge = voll;
        }
        let mut winkel = (laenge / radius).abs().to_degrees();
        if flipped {
            winkel = -winkel;
        }
        (Self::qline_winkel(f1 + winkel), flipped)
    }

    /// Öffnungswinkel eines Bogens in Grad (`vabstractarc.cpp: AngleArc`).
    ///
    /// Gleiche Winkel und 360 Grad Unterschied sind der Vollkreis; sonst der
    /// Winkel von `f1` gegen den Uhrzeiger bis `f2` (`QLineF::angleTo`), bei
    /// einem gespiegelten Bogen die Ergänzung zu 360.
    pub fn seamly_bogen_winkel(f1: f64, f2: f64, flipped: bool) -> f64 {
        let unterschied = (f1 - f2).abs();
        if unterschied <= 1e-12
            || (unterschied - 360.0).abs() * 1e12 <= unterschied.min(360.0)
        {
            return 360.0;
        }
        let winkel = (f2 - f1).rem_euclid(360.0); // QLineF::angleTo
        if flipped {
            360.0 - winkel
        } else {
            winkel
        }
    }

    /// Gezeichnete Polylinie eines Bogens, `schritte` gleiche Schritte.
    pub fn bogen_zeichnung(&self, b: &Arc, schritte: usize) -> Result<Vec<Pt>, GeomError> {
        Ok(Self::bogen_zeichnung_d(&self.bogen_daten(b)?, schritte))
    }

    /// Wie [`Auswertung::bogen_zeichnung`], aus den Bogendaten (auch einer
    /// Kopie oder eines Teilbogens, [`Auswertung::bogen_daten_id`]).
    pub fn bogen_zeichnung_d(d: &Bogendaten, schritte: usize) -> Vec<Pt> {
        let sweep = Self::seamly_bogen_winkel(d.f1, d.f2, d.flipped);
        let a1 = if d.flipped { d.f2 } else { d.f1 };
        if sweep.abs() <= 1e-12 {
            // `qFuzzyIsNull`: Seamly2D führt einen Punkt
            let w = a1.to_radians();
            return vec![Pt::new(
                d.mitte.x + d.radius * w.cos(),
                d.mitte.y - d.radius * w.sin(),
            )];
        }
        let n = schritte.max(1);
        (0..=n)
            .map(|i| {
                let ang = (a1 + sweep * i as f64 / n as f64).to_radians();
                Pt::new(
                    d.mitte.x + d.radius * ang.cos(),
                    d.mitte.y - d.radius * ang.sin(),
                )
            })
            .collect()
    }

    /// Der Streckenzug, in den Seamly2D einen Kreisbogen zerlegt (Pixel).
    ///
    /// Quelle `varc.cpp, getPoints`. Der Bogen wird in Abschnitte von 45°
    /// geteilt, dazu der Rest; jeder Abschnitt wird durch **eine** Bézier-Kurve
    /// angenähert (Griffe tangential, Länge `Radius * 4/3 * tan(Winkel / 4)`)
    /// und diese wie jede Spline zerlegt. Der Zug läuft immer gegen den
    /// Uhrzeiger — beim gespiegelten Bogen beginnt er deshalb am Endwinkel.
    ///
    /// Die 45°-Teilung zählt vom Anfang des Zugs; der Zug einer gespiegelten
    /// Kopie ist darum nicht das Spiegelbild des Zugs ihrer Quelle.
    ///
    /// Gemessen am Randfall-Muster: eine Achse aus der Mitte trifft den Bogen
    /// mit Radius 6 cm in Seamly2D bei 5,9995463 cm (230°) und 5,9988246 cm
    /// (262°) — der Zug liegt dort 0,005 und 0,012 mm **innerhalb** des
    /// Kreises. Die Zeichnung (64 gleiche Schritte) lag bis 0,046 mm daneben.
    pub fn bogen_zug(&self, b: &Arc) -> Result<Vec<[f64; 2]>, GeomError> {
        Ok(self.bogen_zug_d(&self.bogen_daten(b)?))
    }

    /// Wie [`Auswertung::bogen_zug`], aus den Bogendaten.
    pub fn bogen_zug_d(&self, d: &Bogendaten) -> Vec<[f64; 2]> {
        let mitte = [self.zu_px(d.mitte.x), self.zu_px(d.mitte.y)];
        let radius = self.zu_px(d.radius);
        Self::seamly_bogen_punkte(mitte, radius, d.f1, d.f2, d.flipped)
    }

    /// Punkt auf einer Bogen-Zerlegung, alles in Pixeln, y nach unten wie
    /// in Seamly2Ds Szene — deshalb sind `richtung` und `winkel` die
    /// `QLineF`-Winkel selbst, nicht ihr Spiegelbild aus `schnitt.py`.
    pub fn seamly_bogen_punkte(
        mitte: [f64; 2],
        radius: f64,
        f1: f64,
        f2: f64,
        flipped: bool,
    ) -> Vec<[f64; 2]> {
        // `QLineF::setAngle`
        let richtung = |grad: f64| {
            let w = grad.to_radians();
            (w.cos(), -w.sin())
        };
        // `QLineF(a, b).angle()`
        let winkel = |a: [f64; 2], b: [f64; 2]| {
            ((a[1] - b[1]).atan2(b[0] - a[0])).to_degrees().rem_euclid(360.0)
        };

        let (ux, uy) = richtung(if flipped { f2 } else { f1 });
        let mut start = [mitte[0] + radius * ux, mitte[1] + radius * uy];
        let gesamt = Self::seamly_bogen_winkel(f1, f2, flipped);
        if gesamt.abs() <= 1e-12 {
            return vec![start];
        }
        let ganze = (gesamt / 45.0).floor();
        let mut abschnitte = vec![45.0f64; ganze as usize];
        let rest = gesamt - ganze * 45.0;
        if rest > 0.0 {
            abschnitte.push(rest);
        }

        let mut zug: Vec<[f64; 2]> = Vec::new();
        let anzahl = abschnitte.len();
        for (i, w) in abschnitte.iter().enumerate() {
            let griff = radius * 4.0 / 3.0 * (w.to_radians() * 0.25).tan();
            let (ux, uy) = richtung(winkel(start, mitte) - 90.0); // lineP1P2
            let p2 = [start[0] + griff * ux, start[1] + griff * uy];
            let (ux, uy) = richtung(winkel(mitte, start) + w); // lineP4P3
            let p4 = [mitte[0] + radius * ux, mitte[1] + radius * uy];
            let (ux, uy) = richtung(winkel(p4, mitte) + 90.0);
            let p3 = [p4[0] + griff * ux, p4[1] + griff * uy];

            let mut stueck = vec![start];
            crate::kurven::teile_auf_oeffentlich(
                start[0], start[1], p2[0], p2[1], p3[0], p3[1], p4[0], p4[1], 0, &mut stueck,
            );
            stueck.push(p4);
            if i != anzahl - 1 {
                stueck.pop();
            }
            zug.extend(stueck);
            start = p4;
        }
        zug
    }

    /// Länge eines Bogens in Mustereinheiten — der Formelname `Arc_<Mitte>_<id>`.
    ///
    /// Radius mal Bogenwinkel, **negativ**, wenn der Bogen im Uhrzeigersinn
    /// läuft (`varc.cpp: GetLength`, wie `schnitt.py: _arc_groesse`).
    pub fn bogen_laenge(&self, b: &Arc) -> Result<f64, GeomError> {
        Ok(Self::bogen_laenge_d(&self.bogen_daten(b)?))
    }

    /// Wie [`Auswertung::bogen_laenge`], aus den Bogendaten.
    pub fn bogen_laenge_d(d: &Bogendaten) -> f64 {
        let l = d.radius * Self::seamly_bogen_winkel(d.f1, d.f2, d.flipped).to_radians();
        if d.flipped { -l } else { l }
    }

    /// Bogen und Winkel (Grad, nicht auf 0 bis 360 gebracht), bei dem
    /// Seamly2D ihn für den `cutArc`-Punkt `p` schneidet (`vtoolcutarc.cpp`,
    /// `varc.cpp: CutArc`; `schnitt.py: _cut_arc`):
    ///
    /// * die Länge zählt auf **vier** Nachkommastellen; `direction` außer
    ///   `forward` misst vom Bogenende;
    /// * geschnitten wird exakt (Winkel = Länge / Radius), aber mindestens
    ///   1 mm vor jedem Ende; eine negative Länge zählt vom Ende;
    /// * ein Bogen im Uhrzeigersinn (`flipped`) hat eine negative Länge, eine
    ///   positive Schnittlänge zählt dort vom Ende;
    /// * höchstens 2 mm lang: kein Schnitt.
    pub fn bogen_schnittwinkel(&self, p: &Point) -> Result<(Bogendaten, f64), GeomError> {
        let cid = p.arc.as_deref().filter(|s| !s.is_empty()).ok_or(GeomError::Geometrie {
            kind: "cutArc",
            grund: "@arc fehlt".into(),
        })?;
        let d = self.bogen_daten_id(cid)?;
        let radius = self.zu_px(d.radius);
        let mut voll = radius * Self::seamly_bogen_winkel(d.f1, d.f2, d.flipped).to_radians();
        if d.flipped {
            voll = -voll;
        }
        let verlangt = crate::schneiden::vier_stellen(self.feld(&p.length, "length")?);
        let vorwaerts = p.direction.as_deref().map(|r| r == "forward").unwrap_or(true);
        let laenge = if vorwaerts { self.zu_px(verlangt) } else { self.zu_px(self.von_px(voll) - verlangt) };
        let w = Self::seamly_cut_arc(d.f1, radius, voll, d.flipped, laenge).ok_or_else(|| GeomError::Geometrie {
            kind: "cutArc",
            grund: format!("Bogen {cid} ist zu kurz zum Schneiden"),
        })?;
        Ok((d, w))
    }

    /// Schnittwinkel auf einem Bogen (`varc.cpp: CutArc`), Radius, Bogenlänge
    /// `voll` (negativ bei `flipped`) und Schnittlänge in Pixeln.
    pub fn seamly_cut_arc(f1: f64, radius: f64, voll: f64, flipped: bool, mut laenge: f64) -> Option<f64> {
        let min = PX_PRO_MM;
        if voll.abs() <= 2.0 * min {
            return None;
        }
        if !flipped {
            if laenge < 0.0 {
                laenge += voll;
            }
            laenge = laenge.min(voll - min).max(min);
            return Some(f1 + (laenge / radius).to_degrees());
        }
        if laenge > 0.0 {
            laenge += voll;
        }
        laenge = laenge.min(-min).max(voll + min);
        Some(f1 - (laenge.abs() / radius).to_degrees())
    }

    /// Der Punkt eines `cutArc`: auf dem Bogen beim Schnittwinkel.
    pub fn bogen_punkt(&self, p: &Point) -> Result<Pt, GeomError> {
        let (d, w) = self.bogen_schnittwinkel(p)?;
        let (dx, dy) = vektor(w, d.radius);
        Ok(Pt::new(d.mitte.x + dx, d.mitte.y + dy))
    }

    /// Wert einer Bogengrösse `praefix + <Mitte>_<id>`.
    ///
    /// Der Name nennt die **Mitte** und die **id** des Bogens
    /// (`varc.cpp: InitArc`), nicht zwei Endpunkte — Boegen haben keine.
    pub fn bogen_groesse(&self, praefix: &str, rest: &str) -> Result<f64, GeomError> {
        // `rest` ist `<Mitte>_<id>`; die Mitte kann selbst einen Unterstrich
        // tragen, also jede Position durchprobieren.
        let mut treffer: Option<(&str, &str)> = None;
        for (i, c) in rest.char_indices() {
            if c != '_' || i == 0 {
                continue;
            }
            let (mitte, id) = (&rest[..i], &rest[i + 1..]);
            if self.id_von_name(mitte).is_some() {
                treffer = Some((mitte, id));
                break;
            }
        }
        let Some((mitte, id)) = treffer else {
            return Err(GeomError::Geometrie {
                kind: "Bogen",
                grund: format!("{praefix}{rest}: Mitte nicht gefunden"),
            });
        };
        if !self.ist_bogen(id) {
            return Err(GeomError::Geometrie {
                kind: "Bogen",
                grund: format!("Bogen {id} nicht gefunden"),
            });
        }
        // Die Mitte muss wirklich die Mitte dieses Bogens sein; eine Kopie
        // nennt die Mitte ihrer Quelle, ein Teilbogen die seines Bogens.
        let d = self.bogen_daten_id(id)?;
        if self.id_von_name(mitte) != Some(d.mitte_id.as_str())
        {
            return Err(GeomError::Geometrie {
                kind: "Bogen",
                grund: format!("{praefix}{rest}: Mitte passt nicht zu Bogen {id}"),
            });
        }
        match praefix {
            "Arc_" => Ok(Self::bogen_laenge_d(&d)),
            "RadiusArc_" => Ok(d.radius),
            "Angle1Arc_" => Ok(d.f1),
            "Angle2Arc_" => Ok(d.f2),
            andere => Err(GeomError::Geometrie {
                kind: "Bogen",
                grund: format!("unbekannte Bogengroesse {andere}"),
            }),
        }
    }
}