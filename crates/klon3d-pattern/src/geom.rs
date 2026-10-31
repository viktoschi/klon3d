//! Geometrieauswertung eines Seamly2D-Musters.
//!
//! Rechnet die Konstruktionspunkte in **Mustereinheiten** aus. Die Umrechnung
//! nach Millimetern und die Spiegelung der y-Achse bleiben dem Aufrufer
//! überlassen — siehe [`Achse`].
//!
//! ## Koordinaten
//!
//! Seamly2D rechnet mathematisch y-nach-oben, **speichert aber gespiegelt**.
//! In der Bluse liegt die Oberkante bei `y = 10`, die Saumkante bei `y = 100`;
//! alle `single`-Punkte sind positiv. Die Auswertung rechnet deshalb in den
//! gespeicherten Werten weiter, y nach **unten** — ein Winkel von 90° ergibt
//! hier `−y` und zeigt auf dem Bildschirm nach oben, wie in Seamly2D
//! ([`vektor`]; für Bögen gilt dasselbe, siehe [`crate::boegen`]). Erst am
//! Übergang zum Simulator wird gespiegelt, an genau einer Stelle
//! ([`Achse::in_szenene`]).
//!
//! ## Herkunft der Verfahren
//!
//! Die Punkttypen sind Seamly2Ds eigenem C++ nachgebildet
//! (`vtoolnormal.cpp: FindPoint`, `vtoollineintersect.cpp: Create`,
//! `vtoolpointofcontact.cpp: FindPoint`, …). Die Belege stehen in
//! `findings/13-schnitt-py-als-spezifikation.md`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::formula::{eval_str, Context, FormulaError};
use crate::model::{Arc, Line, Operation, Pattern, Point, Spline};
use crate::PatternError;

/// Toleranz „liegt auf der Kurve" in Millimetern.
///
/// `vgobject.cpp: accuracyPointOnLine` = (0,1555 mm / 25,4) · 96 px — ein
/// halbes Pixel bei 96 dpi.
pub const ON_CURVE_MM: f64 = 0.1555;

/// Ein Punkt in Mustereinheiten.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn dist(self, o: Self) -> f64 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
}

/// Die Umrechnung zwischen Musterkoordinaten und Szenenkoordinaten.
///
/// Gehört an genau eine Stelle im Programm. Andernfalls hängt die halbe
/// Musterwelt gespiegelt am Körper — und die Geometrie sieht für sich
/// genommen plausibel aus.
#[derive(Debug, Clone, Copy)]
pub struct Achse {
    /// Mustereinheit → Millimeter.
    pub faktor: f64,
}

impl Achse {
    /// Musterpunkt → Szenenpunkt in Millimetern, y nach oben.
    ///
    /// Die Auswertung rechnet in den gespeicherten Werten, y nach **unten**
    /// (siehe Modulkopf) — hier wird deshalb gespiegelt. Bis zum 19.10.2026
    /// stand hier nur die Einheitenumrechnung, begründet mit „die Auswertung
    /// rechnet bereits in mathematischen Koordinaten"; das stimmte nicht.
    /// Noch ruft niemand die Funktion, der Fehler hätte erst am Körper
    /// gegriffen.
    pub fn in_szenene(self, p: Pt) -> [f64; 3] {
        [p.x * self.faktor, -p.y * self.faktor, 0.0]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GeomError {
    #[error("unbekannter Punkttyp {0:?}")]
    UnsupportedPointType(String),

    #[error("Punkt {0:?} nicht gefunden")]
    UnknownPoint(String),

    #[error("Messwert {0:?} ist nicht definiert")]
    UnknownMeasurement(String),

    #[error("zirkuläre Definition bei {0:?}")]
    Cycle(String),

    #[error("Formel: {0}")]
    Formula(#[from] FormulaError),

    #[error("Schnittstelle {kind}: {grund}")]
    Geometrie { kind: &'static str, grund: String },
}

// ───────────────────────────────────────────────────────── Messdatei ──

/// Liest eine `.smis`-Messdatei.
///
/// Die Werte stehen in der Einheit der Datei (`<unit>`) und werden auf
/// `ziel_einheit` umgerechnet.
pub fn lies_messungen(
    pfad: &Path,
    ziel_einheit: &str,
) -> Result<HashMap<String, f64>, PatternError> {
    let s = std::fs::read_to_string(pfad)
        .map_err(|e| PatternError::Xml(format!("{}: {e}", pfad.display())))?;
    lies_messungen_str(&s, ziel_einheit)
}

pub fn lies_messungen_str(xml: &str, ziel_einheit: &str) -> Result<HashMap<String, f64>, PatternError> {
    // `<unit>` steht vor den Masswerten.
    let datei_einheit = quick_xml_unit(xml).unwrap_or_else(|| ziel_einheit.to_string());
    let von = einheit_nach_mm(&datei_einheit)?;
    let nach = einheit_nach_mm(ziel_einheit)?;
    // Wert in Dateieinheit × mm je Dateieinheit ÷ mm je Mustereinheit. Bis
    // zum 27.10.2026 stand hier `nach / von`: 43 cm wurden in einem
    // mm-Muster 4,3 statt 430 (L036, mrdooslatexprimer). Gleiche Einheiten,
    // wie in allen bisherigen Prüfmustern, gaben 1 und verbargen es.
    let k = von / nach;

    let mut rdr = quick_xml::Reader::from_str(xml);
    rdr.config_mut().trim_text(true);
    let mut out = HashMap::new();
    let mut buf = Vec::new();
    loop {
        let ev = rdr
            .read_event_into(&mut buf)
            .map_err(|e| PatternError::Xml(e.to_string()))?;
        match ev {
            quick_xml::events::Event::Eof => break,
            quick_xml::events::Event::Start(ref e) | quick_xml::events::Event::Empty(ref e)
                if e.local_name().as_ref() == "m" =>
            {
                let mut name = String::new();
                let mut wert = String::new();
                for a in e.attributes().flatten() {
                    // `key`/`value` dereferenzieren zu `str`.
                    match a.key.as_ref() {
                        "name" => name = String::from_utf8_lossy(a.value.as_ref().as_bytes()).into_owned(),
                        "value" => wert = String::from_utf8_lossy(a.value.as_ref().as_bytes()).into_owned(),
                        _ => {}
                    }
                }
                if let Ok(v) = wert.trim().parse::<f64>() {
                    out.insert(name, v * k);
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn quick_xml_unit(xml: &str) -> Option<String> {
    let a = xml.find("<unit>")? + 6;
    let b = xml[a..].find("</unit>")? + a;
    Some(xml[a..b].trim().to_string())
}

/// Faktor nach Millimeter. Deckt die Einheiten ab, die Seamly2D kennt.
pub fn einheit_nach_mm(e: &str) -> Result<f64, PatternError> {
    Ok(match e {
        "mm" => 1.0,
        "cm" => 10.0,
        "m" => 1000.0,
        "inch" | "in" => 25.4,
        "ft" => 304.8,
        other => return Err(PatternError::UnsupportedUnit(other.to_string())),
    })
}

// ───────────────────────────────────────────────────── Auswertung ──

/// Rechnet ein Muster aus: Messungen, Variablen, Punkte, Kurven, Umrisse.
pub struct Auswertung<'a> {
    muster: &'a Pattern,
    mess: HashMap<String, f64>,
    /// Variablenname (ohne `#`) → Formel.
    formeln: HashMap<String, String>,
    /// Rechenobjekte über alle Blöcke. Die ids sind musterweit eindeutig —
    /// in 20 Stichprobenmustern kam keine doppelte id vor.
    pub(crate) punkte: HashMap<String, &'a Point>,
    /// Punktname → id. Für Geometrie-Referenzen wie `Line_A1_G`.
    nach_name: HashMap<String, String>,
    linien: HashMap<String, &'a Line>,
    splines: HashMap<String, &'a Spline>,
    boegen: HashMap<String, &'a Arc>,
    /// Operationen in Dateireihenfolge ([`crate::operationen`]).
    pub(crate) operationen: Vec<&'a Operation>,
    /// Ziel-id → (Index der Operation, Quell-id).
    pub(crate) kopie_von: HashMap<String, (usize, String)>,
    /// Namen der Punkte, die nicht als eigener `<point>` in der Datei stehen:
    /// Punktkopien (Quellname + Suffix) und die Punkte eines `trueDarts`.
    kopie_name: HashMap<String, String>,
    /// Punkt-id → (id des `trueDarts`, 1 oder 2).
    abnaeher_von: HashMap<String, (String, u8)>,
    /// Endpunktnamen kopierter Kurven: `Spl_`/`SplPath_` nennen sie, auch
    /// wenn der Punkt selbst nicht mitkopiert wurde.
    kopie_enden: HashSet<String>,

    var_cache: RefCell<HashMap<String, f64>>,
    punkt_cache: RefCell<HashMap<String, Pt>>,
    pub(crate) kurve_cache: RefCell<HashMap<String, Vec<Pt>>>,
    im_lauf: RefCell<HashSet<String>>,
    /// Zusatzgrössen, die Seamly2D der Formel stellt (`CurrentLength`).
    kontext: RefCell<HashMap<String, f64>>,
    /// Protokoll nicht lösbarer Punkte.
    bericht: RefCell<Vec<String>>,
}

impl<'a> Auswertung<'a> {
    /// Baut die Auswertung auf. `mess` bereits in Mustereinheiten.
    pub fn new(muster: &'a Pattern, mess: HashMap<String, f64>) -> Self {
        let mut formeln = HashMap::new();
        if let Some(inc) = &muster.increments {
            for i in &inc.items {
                // Der gespeicherte Name traegt das `#` selbst.
                formeln.insert(i.name.trim_start_matches('#').to_string(), i.formula.clone());
            }
        }

        let mut punkte = HashMap::new();
        let mut nach_name: HashMap<String, String> = HashMap::new();
        let mut linien = HashMap::new();
        let mut splines = HashMap::new();
        let mut boegen = HashMap::new();
        let mut operationen: Vec<&'a Operation> = Vec::new();
        for b in &muster.draft_blocks {
            if let Some(c) = &b.calculation {
                for p in &c.points {
                    punkte.insert(p.id.clone(), p);
                    if let Some(n) = &p.name {
                        nach_name.insert(n.clone(), p.id.clone());
                    }
                }
                for l in &c.lines {
                    linien.insert(l.id.clone(), l);
                }
                for s in &c.splines {
                    splines.insert(s.id.clone(), s);
                }
                for a in &c.arcs {
                    boegen.insert(a.id.clone(), a);
                }
                operationen.extend(c.operations.iter());
            }
        }

        // `trueDarts` erzeugt zwei Punkte unter `point1`/`point2`, benannt
        // `name1`/`name2`.
        let mut kopie_name: HashMap<String, String> = HashMap::new();
        let mut abnaeher_von: HashMap<String, (String, u8)> = HashMap::new();
        for p in punkte.values().filter(|p| p.kind == "trueDarts") {
            for (teil, id, name) in [(1u8, &p.point1, &p.name1), (2, &p.point2, &p.name2)] {
                let Some(id) = id.as_deref().filter(|s| !s.is_empty()) else { continue };
                abnaeher_von.insert(id.to_string(), (p.id.clone(), teil));
                if let Some(n) = name.as_deref().filter(|s| !s.is_empty()) {
                    nach_name.entry(n.to_string()).or_insert_with(|| id.to_string());
                    kopie_name.insert(id.to_string(), n.to_string());
                }
            }
        }

        // Operationen der Reihe nach: Ziele und Namen der Punktkopien. Eine
        // Quelle darf selbst das Ziel einer früheren Operation sein.
        let mut kopie_von: HashMap<String, (usize, String)> = HashMap::new();
        for (i, op) in operationen.iter().enumerate() {
            for (q, z) in op.quellen.iter().zip(&op.ziele) {
                kopie_von.insert(z.clone(), (i, q.clone()));
                let quellname = punkte
                    .get(q)
                    .and_then(|p: &&Point| p.name.clone())
                    .or_else(|| kopie_name.get(q).cloned());
                if let Some(n) = quellname.filter(|n| !n.is_empty()) {
                    let neu = format!("{n}{}", op.suffix);
                    nach_name.entry(neu.clone()).or_insert_with(|| z.clone());
                    kopie_name.insert(z.clone(), neu);
                }
            }
        }

        let mut ev = Self {
            muster,
            mess,
            formeln,
            punkte,
            nach_name,
            linien,
            splines,
            boegen,
            operationen,
            kopie_von,
            kopie_name,
            abnaeher_von,
            kopie_enden: HashSet::new(),
            var_cache: RefCell::new(HashMap::new()),
            punkt_cache: RefCell::new(HashMap::new()),
            kurve_cache: RefCell::new(HashMap::new()),
            im_lauf: RefCell::new(HashSet::new()),
            kontext: RefCell::new(HashMap::new()),
            bericht: RefCell::new(Vec::new()),
        };
        let enden: HashSet<String> = ev
            .abgeleitete_kurven()
            .iter()
            .filter_map(|z| ev.kurven_enden(z))
            .flat_map(|(a, b)| [a, b])
            .collect();
        ev.kopie_enden = enden;
        ev
    }

    /// Lädt Muster und Messdatei in einem Schritt.
    pub fn lade(muster: &'a Pattern, messpfad: &Path) -> Result<Self, PatternError> {
        let mess = lies_messungen(messpfad, &muster.unit)?;
        Ok(Self::new(muster, mess))
    }

    pub fn bericht(&self) -> Vec<String> {
        self.bericht.borrow().clone()
    }

    /// Wert einer Variablen in Mustereinheiten.
    pub fn variable(&self, name: &str) -> Result<f64, GeomError> {
        let key = name.trim_start_matches('#').to_string();
        if let Some(v) = self.var_cache.borrow().get(&key) {
            return Ok(*v);
        }
        {
            let mut lauf = self.im_lauf.borrow_mut();
            if !lauf.insert(format!("var:{key}")) {
                return Err(GeomError::Cycle(format!("#{key}")));
            }
        }
        let r = match self.formeln.get(&key) {
            Some(f) => eval_str(f, self).map_err(GeomError::from),
            None => self.mess_err(&key),
        };
        self.im_lauf.borrow_mut().remove(&format!("var:{key}"));
        if let Ok(v) = r {
            self.var_cache.borrow_mut().insert(key.clone(), v);
        }
        r
    }

    fn mess_err(&self, name: &str) -> Result<f64, GeomError> {
        self.mess
            .get(name)
            .copied()
            .ok_or_else(|| GeomError::UnknownMeasurement(name.to_string()))
    }

    // ─────────────────────────────────────────────────────── Punkte ──

    /// Koordinate eines Punktes in Mustereinheiten.
    pub fn punkt(&self, id: &str) -> Result<Pt, GeomError> {
        if let Some(p) = self.punkt_cache.borrow().get(id) {
            return Ok(*p);
        }
        {
            let mut lauf = self.im_lauf.borrow_mut();
            if !lauf.insert(format!("pt:{id}")) {
                return Err(GeomError::Cycle(format!("Punkt {id}")));
            }
        }
        let r = self.punkt_roh(id);
        self.im_lauf.borrow_mut().remove(&format!("pt:{id}"));
        match r {
            Ok(p) => {
                self.punkt_cache.borrow_mut().insert(id.to_string(), p);
                Ok(p)
            }
            Err(e) => {
                self.bericht.borrow_mut().push(format!("Punkt {id}: {e}"));
                Err(e)
            }
        }
    }

    /// Wie [`Auswertung::punkt`], aber ohne Cache und ohne Zyklenschutz.
    /// Ein Ziel einer Operation ist das Abbild seiner Quelle.
    fn punkt_roh(&self, id: &str) -> Result<Pt, GeomError> {
        if let Some(p) = self.punkte.get(id) {
            return self.nach_typ(p);
        }
        if let Some((op, q)) = self.kopie(id) {
            let quelle = self.punkt(q)?;
            return self.abbilden(op, quelle);
        }
        if let Some((werkzeug, teil)) = self.abnaeher_von.get(id) {
            let (p1, p2) = self.abnaeher(self.punkte[werkzeug.as_str()])?;
            return Ok(if *teil == 1 { p1 } else { p2 });
        }
        Err(GeomError::UnknownPoint(id.to_string()))
    }

    /// Die beiden Punkte eines `trueDarts` (`vtooltruedarts.cpp: FindPoint`):
    /// das Ende der Basislinie wird um die Spitze `dartP2` um den
    /// Abnäherwinkel (von Schenkel `dartP3` zu Schenkel `dartP1`) gedreht;
    /// die Gerade vom Anfang der Basislinie dorthin schneidet die Gerade
    /// durch Spitze und `dartP1` im ersten Punkt; der zweite ist der erste,
    /// um die Spitze zurückgedreht.
    pub fn abnaeher(&self, p: &Point) -> Result<(Pt, Pt), GeomError> {
        let b1 = self.bezug(p, "baseLineP1", &p.base_line_p1, "trueDarts")?;
        let b2 = self.bezug(p, "baseLineP2", &p.base_line_p2, "trueDarts")?;
        let d1 = self.bezug(p, "dartP1", &p.dart_p1, "trueDarts")?;
        let d2 = self.bezug(p, "dartP2", &p.dart_p2, "trueDarts")?;
        let d3 = self.bezug(p, "dartP3", &p.dart_p3, "trueDarts")?;
        // `QLineF::angle`, 0 bis unter 360 Grad
        let w = |a: Pt, b: Pt| winkel_von(a, b).rem_euclid(360.0);
        // `d2d3.angleTo(d2d1)`
        let grad = (w(d2, d1) - w(d2, d3)).rem_euclid(360.0);
        let um_spitze = |q: Pt, dreh: f64| {
            let (dx, dy) = vektor(w(d2, q) + dreh, d2.dist(q));
            Pt::new(d2.x + dx, d2.y + dy)
        };
        let p1 = Self::geraden_schnitt((b1, um_spitze(b2, grad)), (d2, d1)).ok_or(GeomError::Geometrie {
            kind: "trueDarts",
            grund: "Basislinie und Abnäherschenkel sind parallel — Seamly2D setzt beide Punkte auf den Nullpunkt".into(),
        })?;
        Ok((p1, um_spitze(p1, -grad)))
    }

    /// Benannte Punkte, die nicht als eigener `<point>` in der Datei stehen
    /// (Punktkopien aus Operationen, die Punkte eines `trueDarts`): Name und id.
    pub fn abgeleitete_punkte(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self.kopie_name.iter().map(|(id, n)| (n.clone(), id.clone())).collect();
        v.sort();
        v
    }

    pub(crate) fn ref_pt(&self, id: &str) -> Result<Pt, GeomError> {
        if id.is_empty() {
            return Err(GeomError::UnknownPoint(String::new()));
        }
        self.punkt(id)
    }

    /// Zahlenfeld eines Punktobjekts — ist fast immer eine Formel.
    pub(crate) fn feld(&self, wert: &Option<String>, kurz: &'static str) -> Result<f64, GeomError> {
        match wert {
            Some(t) => {
                let t = t.trim();
                if let Ok(z) = t.parse::<f64>() {
                    return Ok(z);
                }
                eval_str(t, self).map_err(GeomError::from)
            }
            None => Err(GeomError::Geometrie {
                kind: kurz,
                grund: "Feld fehlt".into(),
            }),
        }
    }

    fn bezug(
        &self,
        p: &Point,
        key: &str,
        feld: &Option<String>,
        kurz: &'static str,
    ) -> Result<Pt, GeomError> {
        // `p` wird nur für die Fehlermeldung gebraucht (welcher Typ fehlt).
        let _ = p;
        let id = feld
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or(GeomError::Geometrie {
                kind: kurz,
                grund: format!("{key} fehlt"),
            })?;
        self.ref_pt(id)
    }


    /// Rechnet die 15 von Seamly2D bekannten Punkttypen.
    pub fn nach_typ(&self, p: &Point) -> Result<Pt, GeomError> {
        let kurz: &'static str = "Punkt";
        match p.kind.as_str() {
            "single" => Ok(Pt::new(
                self.feld(&p.x, "single.x")?,
                self.feld(&p.y, "single.y")?,
            )),

            "endLine" => {
                let b = self.bezug(p, "basePoint", &p.base_point, kurz)?;
                let (dx, dy) = vektor(self.feld(&p.angle, "angle")?, self.feld(&p.length, "length")?);
                Ok(Pt::new(b.x + dx, b.y + dy))
            }

            "alongLine" => {
                let p1 = self.bezug(p, "firstPoint", &p.first_point, kurz)?;
                let p2 = self.bezug(p, "secondPoint", &p.second_point, kurz)?;
                let gesamt = p1.dist(p2);
                // Seamly2D stellt `CurrentLength` bereit — die Länge der
                // Strecke firstPoint→secondPoint. Ohne sie gelten alle
                // alongLine-Punkte als unauflösbar.
                // Die Länge muss **innerhalb** des Kontexts ausgewertet
                // werden — sonst sieht die Formel `CurrentLength` nicht.
                let laenge = self.mit_kontext("CurrentLength", gesamt, || {
                    self.feld(&p.length, "length")
                })?;
                if gesamt == 0.0 {
                    // Beide Bezugspunkte fallen zusammen — kommt bei
                    // Spiegelungen vor, wenn ein Punkt auf der Achse liegt.
                    return Ok(p1);
                }
                let f = laenge / gesamt;
                Ok(Pt::new(
                    p1.x + (p2.x - p1.x) * f,
                    p1.y + (p2.y - p1.y) * f,
                ))
            }

            "normal" => {
                // vtoolnormal.cpp: FindPoint — QLineF::normalVector() ist
                // (dy, −dx), am Bildschirm also +90° gegen den Uhrzeiger.
                // Mit −90 lagen die Punkte 86,7 mm daneben (gemessen am
                // Strumpf v20 gegen Seamly2Ds eigenen Export).
                let p1 = self.bezug(p, "firstPoint", &p.first_point, kurz)?;
                let p2 = self.bezug(p, "secondPoint", &p.second_point, kurz)?;
                let basis = winkel_von(p1, p2);
                let ang = basis + self.feld(&p.angle, "angle")? + 90.0;
                let (dx, dy) = vektor(ang, self.feld(&p.length, "length")?);
                Ok(Pt::new(p1.x + dx, p1.y + dy))
            }

            "intersectXY" => {
                // point_intersectxy_tool.cpp: Create —
                // ``QPointF point(firstPoint->x(), secondPoint->y())``
                let p1 = self.bezug(p, "firstPoint", &p.first_point, kurz)?;
                let p2 = self.bezug(p, "secondPoint", &p.second_point, kurz)?;
                Ok(Pt::new(p1.x, p2.y))
            }

            "lineIntersect" => {
                // vtoollineintersect.cpp: Create — die Ziffer HINTER "Line"
                // nennt die Gerade, die davor den Punkt.
                let l1 = self.gerade2(p, &p.p1_line1, &p.p2_line1, "p1Line1", "p2Line1")?;
                let l2 = self.gerade2(p, &p.p1_line2, &p.p2_line2, "p1Line2", "p2Line2")?;
                self.schnitt_punkt(&l1, &l2)
            }

            "lineIntersectAxis" => {
                let g = self.gerade2(p, &p.p1_line, &p.p2_line, "p1Line", "p2Line")?;
                let b = self.bezug(p, "basePoint", &p.base_point, kurz)?;
                let (dx, dy) = richtung(self.feld(&p.angle, "angle")?);
                let achse = (b, Pt::new(b.x + dx, b.y + dy));
                self.schnitt_punkt(&g, &achse)
            }

            "height" => {
                // Fusspunkt des Lots von basePoint auf die Gerade.
                let bp = self.bezug(p, "basePoint", &p.base_point, kurz)?;
                let (l1, l2) = self.gerade2(p, &p.p1_line, &p.p2_line, "p1Line", "p2Line")?;
                let (dx, dy) = (l2.x - l1.x, l2.y - l1.y);
                let n = dx * dx + dy * dy;
                if n == 0.0 {
                    return Err(GeomError::Geometrie {
                        kind: "height",
                        grund: "Punktstrecke der Länge 0".into(),
                    });
                }
                let f = ((bp.x - l1.x) * dx + (bp.y - l1.y) * dy) / n;
                Ok(Pt::new(l1.x + f * dx, l1.y + f * dy))
            }

            // Schneiden auf Kurven und Achsenschnitt.
            "cutSpline" | "cutSplinePath" | "curveIntersectAxis" => self.punkt_auf_kurve(p),

            "cutArc" => self.bogen_punkt(p),
            // Das Werkzeug selbst ist kein Punkt; seine beiden Punkte stehen
            // unter `point1`/`point2` ([`Auswertung::abnaeher`]).
            "trueDarts" => Err(GeomError::Geometrie {
                kind: "trueDarts",
                grund: "Werkzeug ohne eigenen Punkt".into(),
            }),
            "pointOfIntersectionArcs" => self.schnitt_boegen(p),
            "pointOfIntersectionCurves" => self.schnitt_kurven(p),

            "pointOfContact" => {
                // vtoolpointofcontact.cpp: FindPoint — Schnitt eines KREISES
                // mit der GERADEN durch firstPoint/secondPoint. Bei zwei
                // Treffern: liegt genau einer auf der STRECKE, wird dieser
                // genommen; sonst der nähere zu firstPoint.
                let c = self.bezug(p, "center", &p.center, kurz)?;
                let r = self.feld(&p.radius, "radius")?;
                let p1 = self.bezug(p, "firstPoint", &p.first_point, kurz)?;
                let p2 = self.bezug(p, "secondPoint", &p.second_point, kurz)?;
                let treffer = Self::kreis_gegerade(c, r, p1, p2);
                if treffer.is_empty() {
                    return Err(GeomError::Geometrie {
                        kind: "pointOfContact",
                        grund: "Kreis und Gerade schneiden sich nicht".into(),
                    });
                }
                if treffer.len() == 1 {
                    return Ok(treffer[0]);
                }
                let tol = 1e-9; // schnitt.py `_on_segment`, Standard 1e-9
                let auf = treffer
                    .iter()
                    .map(|h| Self::auf_strecke(*h, p1, p2, tol))
                    .collect::<Vec<_>>();
                if auf[0] == auf[1] {
                    return Ok(treffer
                        .iter()
                        .copied()
                        .min_by(|a, b| p1.dist(*a).total_cmp(&p1.dist(*b)))
                        .unwrap());
                }
                Ok(if auf[0] { treffer[0] } else { treffer[1] })
            }

            other => Err(GeomError::UnsupportedPointType(other.to_string())),
        }
    }

    fn mit_kontext<T>(
        &self,
        name: &str,
        wert: f64,
        f: impl FnOnce() -> Result<T, GeomError>,
    ) -> Result<T, GeomError> {
        let vorher = self.kontext.borrow_mut().insert(name.to_string(), wert);
        let r = f();
        match vorher {
            Some(v) => {
                self.kontext.borrow_mut().insert(name.to_string(), v);
            }
            None => {
                self.kontext.borrow_mut().remove(name);
            }
        }
        r
    }

    fn gerade2(
        &self,
        p: &Point,
        p1: &Option<String>,
        p2: &Option<String>,
        n1: &'static str,
        n2: &'static str,
    ) -> Result<(Pt, Pt), GeomError> {
        Ok((self.bezug(p, n1, p1, "gerade")?, self.bezug(p, n2, p2, "gerade")?))
    }

    fn schnitt_punkt(&self, a: &(Pt, Pt), b: &(Pt, Pt)) -> Result<Pt, GeomError> {
        Self::geraden_schnitt(*a, *b).ok_or(GeomError::Geometrie {
            kind: "Schnitt",
            grund: "Geraden sind parallel oder deckungsgleich".into(),
        })
    }

    /// Schnittpunkt zweier Geraden.
    pub fn geraden_schnitt(
        (p1, q1): (Pt, Pt),
        (p2, q2): (Pt, Pt),
    ) -> Option<Pt> {
        let (dx1, dy1) = (q1.x - p1.x, q1.y - p1.y);
        let (dx2, dy2) = (q2.x - p2.x, q2.y - p2.y);
        let n = dx1 * dy2 - dy1 * dx2;
        if n.abs() < 1e-12 {
            return None;
        }
        let (rx, ry) = (p2.x - p1.x, p2.y - p1.y);
        let t = (rx * dy2 - ry * dx2) / n;
        Some(Pt::new(p1.x + t * dx1, p1.y + t * dy1))
    }

    /// Schnitt Kreis mit **unendlicher** Gerade.
///
/// Nach `vtoolpointofcontact.cpp: FindPoint` — in der Merkform von
    /// `schnitt.py: _circle_line`. Die quadratische Gleichung lautet
    /// `a·t² + b·t + c = 0`, also `t = (−b ± √(b²−4ac)) / (2a)`: **das Minus vor
    /// `b` ist Teil der Formel**. Ohne es kamen beide Treffer an den falschen
    /// Stellen der Geraden zu; an Bluse 2 v26 lag `RB2_L` dadurch 1,70 mm daneben,
    /// was über die Kontrolllinie `K10` eine Abweichung von 1,67 cm erzeugte.
    pub fn kreis_gegerade(c: Pt, r: f64, p1: Pt, p2: Pt) -> Vec<Pt> {
        let (dx, dy) = (p2.x - p1.x, p2.y - p1.y);
        let a = dx * dx + dy * dy;
        if a == 0.0 {
            return Vec::new();
        }
        let (fx, fy) = (p1.x - c.x, p1.y - c.y);
        let b = 2.0 * (fx * dx + fy * dy);
        let cc = fx * fx + fy * fy - r * r;
        let mut disc = b * b - 4.0 * a * cc;
        if disc < -1e-12 {
            return Vec::new();
        }
        if disc < 0.0 {
            disc = 0.0;
        }
        let w = disc.sqrt();
        let t1 = (-b - w) / (2.0 * a);
        let t2 = (-b + w) / (2.0 * a);
        let p = Pt::new(p1.x + t1 * dx, p1.y + t1 * dy);
        // Tangente: ein Treffer, nicht zwei gleiche.
        if w == 0.0 {
            return vec![p];
        }
        vec![p, Pt::new(p1.x + t2 * dx, p1.y + t2 * dy)]
    }

    fn auf_strecke(h: Pt, a: Pt, b: Pt, tol: f64) -> bool {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let n = (dx * dx + dy * dy).sqrt();
        if n == 0.0 {
            return h.dist(a) <= tol;
        }
        let f = ((h.x - a.x) * dx + (h.y - a.y) * dy) / (dx * dx + dy * dy);
        (-tol..=1.0 + tol / n).contains(&f) && h.dist(Pt::new(a.x + f * dx, a.y + f * dy)) <= tol
    }

    // ─────────────────────────────────────────────── Formelkontext ──

    /// Mustereinheit → Millimeter (`def.cpp: UnitConvertor`).
    pub fn mm_pro_einheit(&self) -> f64 {
        self.muster.unit_to_mm().unwrap_or(1.0)
    }

    /// Wert eines Feldes, das eine Formel oder eine Zahl ist, mit Vorgabe.
    ///
    /// Seamly2Ds Vorgaben für fehlende Attribute sind hier die Vorgabewerte:
    /// eine fehlende Formel gilt als 0, ein fehlender Radius als 10 usw.
    /// Der Aufrufer entscheidet, welcher Vorgabewert zu welchem Feld gehört.
    pub fn formel_zahl(&self, wert: Option<&str>, vorgabe: f64) -> f64 {
        let Some(t) = wert.map(str::trim).filter(|s| !s.is_empty()) else {
            return vorgabe;
        };
        if let Ok(z) = t.parse::<f64>() {
            return z;
        }
        match eval_str(t, self) {
            Ok(v) => v,
            Err(_) => vorgabe,
        }
    }

    /// Alle Splines des Musters — Reihenfolge wie im Dokument.
    pub fn alle_splines(&self) -> Vec<&'a Spline> {
        self.splines.values().copied().collect()
    }

    /// Id des Punktes mit diesem Namen — `None` für unbekannte Namen.
    ///
    /// `id_von_name("")` liefert `None`, damit leere Attributwerte nichts
    /// treffen.
    pub fn id_von_name(&self, name: &str) -> Option<&str> {
        if name.is_empty() {
            return None;
        }
        self.nach_name.get(name).map(|s| s.as_str())
    }

    /// Name des Rechenpunkts mit dieser id — `None`, wenn er keinen hat.
    /// Eine Punktkopie heißt wie ihre Quelle mit dem Suffix der Operation.
    pub fn punkt_name(&self, id: &str) -> Option<&str> {
        match self.punkte.get(id) {
            Some(p) => p.name.as_deref(),
            None => self.kopie_name.get(id).map(String::as_str),
        }
    }

    /// Präfixe, mit denen eine Formel auf die Zeichnung zugreift.
    ///
    /// Reihenfolge wie bei Seamly2D: `SplPath_` vor `Spl_`, sonst schluckt
    /// das kürzere Präfix den Anfang des längeren.
    const GEO_PRAEFIXE: &[&str] = &[
        "SplPath_", "Spl_", "Line_", "Arc_", "AngleLine_", "RadiusArc_", "Angle1Arc_",
        "Angle2Arc_",
    ];

    /// Zerlegt `rest` an **jeder** `_`-Position, für die beide Hälften
    /// bekannte Punktnamen sind.
    ///
    /// `RT_L_RJM_L` ist `RT_L | RJM_L`, könnte bei anderen Namen aber ebenso
    /// `RT | L_RJM_L` sein. `Line_` nimmt die erste Zerlegung von links;
    /// `SplPath_` die, zu der es das Objekt gibt.
    pub fn zerlege_geo(&self, rest: &str) -> Vec<(String, String)> {
        let mut paare = Vec::new();
        for (i, c) in rest.char_indices() {
            if c != '_' || i == 0 {
                continue;
            }
            let (links, rechts) = (&rest[..i], &rest[i + 1..]);
            let bekannt = |n: &str| self.nach_name.contains_key(n) || self.kopie_enden.contains(n);
            if bekannt(links) && bekannt(rechts) {
                paare.push((links.to_string(), rechts.to_string()));
            }
        }
        paare
    }

    /// Wert einer Geometrie-Referenz wie `Line_A1_G`.
    pub fn geo(&self, name: &str) -> Result<f64, GeomError> {
        for praefix in Self::GEO_PRAEFIXE {
            let Some(rest) = name.strip_prefix(praefix) else {
                continue;
            };
            return match *praefix {
                "Line_" => {
                    let paare = self.zerlege_geo(rest);
                    let Some((a, b)) = paare.first() else {
                        return Err(GeomError::Geometrie {
                            kind: "Line_",
                            grund: format!("{name}: keine Zerlegung in zwei Punktnamen"),
                        });
                    };
                    Ok(self.punkt(&self.nach_name[a])?.dist(self.punkt(&self.nach_name[b])?))
                }
                "AngleLine_" => {
                    let paare = self.zerlege_geo(rest);
                    let Some((a, b)) = paare.first() else {
                        return Err(GeomError::Geometrie {
                            kind: "AngleLine_",
                            grund: format!("{name}: keine Zerlegung in zwei Punktnamen"),
                        });
                    };
                    let pa = self.punkt(&self.nach_name[a])?;
                    let pb = self.punkt(&self.nach_name[b])?;
                    Ok(winkel_von(pa, pb))
                }
                // Braucht Kurven.
                "Spl_" => {
                    let paare = self.zerlege_geo(rest);
                    let Some((a, b)) = paare.first() else {
                        return Err(GeomError::Geometrie {
                            kind: "Spl_",
                            grund: format!("{name}: keine Zerlegung in zwei Punktnamen"),
                        });
                    };
                    self.spline_zwischen(a, b)
                }
                "SplPath_" => {
                    // Die Zerlegung ist **nicht fest**: Seamly2D probiert alle
                    // und nimmt die erste, zu der es einen Pfad gibt
                    // (`_geo: SplPath_`). `B1_AM0_AE1` ist `B1|AM0_AE1` und
                    // `B1_AM0|AE1` zugleich; nur die zweite hat einen Pfad
                    // (id 6032, Kinder 301..6028). Mit der festen ersten
                    // Zerlegung war Pfad 6032 unerreichbar und die
                    // Kontrolllinie K13 wich um 14,78 cm ab.
                    let paare = self.zerlege_geo(rest);
                    if paare.is_empty() {
                        return Err(GeomError::Geometrie {
                            kind: "SplPath_",
                            grund: format!("{name}: keine Zerlegung in zwei Punktnamen"),
                        });
                    }
                    let mut letzter = None;
                    for (a, b) in &paare {
                        match self.pfad_zwischen(a, b) {
                            Ok(w) => return Ok(w),
                            Err(e) => letzter = Some(e),
                        }
                    }
                    return Err(letzter.unwrap_or_else(|| GeomError::Geometrie {
                        kind: "SplPath_",
                        grund: format!("kein Pfad {}->{}", paare[0].0, paare[0].1),
                    }));
                }
                // `Arc_`, `RadiusArc_`, `Angle1Arc_`, `Angle2Arc_` nennen
                // **Mitte und id** des Bogens (`varc.cpp: InitArc`), nicht
                // zwei Punkte — `rest` ist `<Mitte>_<id>`.
                "Arc_" | "RadiusArc_" | "Angle1Arc_" | "Angle2Arc_" => self.bogen_groesse(*praefix, rest),
                _ => unreachable!("Praefix ist aus der Liste"),
            };
        }
        Err(GeomError::UnknownMeasurement(name.to_string()))
    }

    pub fn linie(&self, id: &str) -> Option<&Line> {
        self.linien.get(id).copied()
    }
    pub fn spline(&self, id: &str) -> Option<&Spline> {
        self.splines.get(id).copied()
    }
    pub fn bogen(&self, id: &str) -> Option<&Arc> {
        self.boegen.get(id).copied()
    }

    pub fn muster(&self) -> &Pattern {
        self.muster
    }
}

impl Context for Auswertung<'_> {
    fn resolve(&self, name: &str) -> Result<f64, FormulaError> {
        // `CurrentLength` und Verwandte stellt Seamly2D temporär bereit.
        if let Some(v) = self.kontext.borrow().get(name) {
            return Ok(*v);
        }
        if let Ok(v) = self.variable(name) {
            return Ok(v);
        }
        if let Some(v) = self.mess.get(name) {
            return Ok(*v);
        }
        // Zuletzt die Zeichnung: `Line_A1_G` ist die Streckenlaenge zwischen
        // zwei benannten Punkten.
        match self.geo(name) {
            Ok(v) => Ok(v),
            Err(e) => Err(FormulaError::UnknownName {
                name: format!("{name} ({e})"),
            }),
        }
    }
}

/// Bequemlichkeit für den Aufrufer.
pub fn alle_punkte(ev: &Auswertung<'_>) -> Result<Vec<(String, Pt)>, GeomError> {
    let ids: Vec<String> = ev
        .muster()
        .draft_blocks
        .iter()
        .filter_map(|b| b.calculation.as_ref())
        .flat_map(|c| c.points.iter().map(|p| p.id.clone()))
        .collect();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(p) = ev.punkt(&id) {
            out.push((id, p));
        }
    }
    Ok(out)
}


// ────────────────────────────────────── Winkel und Richtungen ──
    /// Übersetzt einen mathematischen Winkel in einen Richtungsvektor.
///
/// **Das ist die einzige Stelle, an der die beiden Konventionen
/// aufeinandertreffen.** Seamly2D rechnet Winkel mathematisch (y nach oben),
/// speichert die Koordinaten aber gespiegelt (y nach unten). Die Auswertung
/// rechnet in den gespeicherten Werten, deshalb wird hier die
/// y-Wirkung **negiert**.
///
/// `angle="90"`, Länge 23, Basis auf y = 100 ergibt so y = 77 — belegt an
/// `I_R` des Bustiers. Ohne die Negierung käme y = 123 heraus.
pub fn vektor(ang: f64, ln: f64) -> (f64, f64) {
    let r = ang.to_radians();
    (ln * r.cos(), -ln * r.sin())
}

/// Einheitsrichtung eines mathematischen Winkels, in gespeicherten Werten.
pub fn richtung(ang: f64) -> (f64, f64) {
    let r = ang.to_radians();
    (r.cos(), -r.sin())
}

/// mathematischer Winkel der Strecke `a`→`b` in Grad.
///
/// In gespeicherten Werten ist das der **negative** Winkel, weil y dort nach
/// unten zählt.
pub fn winkel_von(a: Pt, b: Pt) -> f64 {
    (a.y - b.y).atan2(b.x - a.x).to_degrees()
}
