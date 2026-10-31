//! `klon3d-fenster` — die Nahtlinien der Teile eines Seamly2D-Musters ansehen.
//!
//! ## Was hier bewusst einfach gehalten ist
//!
//! Das Fenster entsteht aus der Win32-API und GDI, ohne Crate ausser dem
//! eigenen Musterleser. Kein Treiber, keine Laufzeit, kein Netz. Ein Schnitt
//! aus `.sm2d` ergibt vollständige Teileumrisse (`kontur.rs`), und die werden
//! hier gezeichnet — mehr behauptet das Programm nicht.
//!
//! Gezeichnet wird an genau einer Stelle, [`zeichnen`], und immer auf eine
//! [`Leinwand`] im Speicher. Das Fenster bekommt eine Kopie davon, das PNG
//! die Bytes selbst. Warum das so sein muss, steht in `leinwand.rs`.
//!
//! ## Bedienung
//!
//! | Taste / Maus | Wirkung |
//! |---|---|
//! | Mausrad, `+` / `-` | Zoom um die Mausposition bzw. die Fenstermitte |
//! | linke Taste ziehen, Pfeiltasten | verschieben |
//! | Klick | Teil wählen; erneut klicken wechselt durch übereinanderliegende Teile |
//! | `Tab` / `Umschalt+Tab` | nächstes / voriges Teil wählen |
//! | `Leertaste` | nur das gewählte Teil, eingepasst; noch einmal zurück |
//! | `A` | alle Teile ein-/ausblenden |
//! | `L` | Teile ausgelegt oder in der Konstruktionslage |
//! | `R` | das Sichtbare einpassen |
//! | `N` / `P` | nächstes / voriges Muster |
//! | `S` | Bild als PNG sichern (neben der `.exe`) |
//! | `Esc` | schliessen |
//!
//! ## Ohne Fenster
//!
//! ```text
//! klon3d-fenster --png <ordner> [--groesse 1600x1000] [--konstruktion] [--teil N] muster.sm2d ...
//! ```
//!
//! zeichnet jedes Muster einmal auf eine Leinwand und schreibt
//! `<ordner>/<muster>.png` — derselbe Zeichenweg wie im Fenster.
//!
//! ## Warum y im Bild nach unten zeigt
//!
//! Die Musterdatei rechnet y nach unten, das Bild auch — sonst stünde das
//! Muster auf dem Kopf. Muster- und Bildraum treffen sich nur in
//! [`Sicht::rechne`].
//!
//! ## Auslage
//!
//! In der Konstruktion liegen die Teile dort, wo sie gezeichnet wurden:
//! Nachbarteile teilen sich Nähte, Kragen und Vorderteile liegen über dem
//! Rücken. Zum Prüfen einzelner Umrisse legt der Betrachter die Teile
//! deshalb selbst aus ([`auslegen`]) — in Reihen, nach Höhe, ohne
//! Überlappung. Das ist die Vorgabe; `L` schaltet zur Konstruktion um.
//!
//! Die Lage, die die Datei je Teil mitbringt (`mx`/`my`), wird nicht
//! benutzt: in den Prüfmustern verteilt sie die Teile über drei Meter und
//! lässt trotzdem einige übereinander liegen (Befund 15).

mod leinwand;
mod png;
mod win;

use std::path::{Path, PathBuf};

use klon3d_pattern::geom::{lies_messungen, Auswertung, Pt};
use klon3d_pattern::read_path;

use leinwand::{Auswahl, Leinwand, Objekt};
use win::*;

// ─────────────────────────────────────────────────────────── Farben

/// Eine Farbe in RGB; [`Farbe::cref`] macht daraus GDIs `COLORREF`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Farbe(u8, u8, u8);

impl Farbe {
    const fn cref(self) -> u32 {
        rgb(self.0, self.1, self.2)
    }
}

const HINTERGRUND: Farbe = Farbe(246, 246, 248);
const NAHT: Farbe = Farbe(43, 43, 48);
/// Hell, damit die Nahtlinien die Führung behalten.
const FUELLUNG: Farbe = Farbe(206, 220, 242);
const AUSGEWAEHLT: Farbe = Farbe(232, 98, 44);
const AUSWAHL_FUELLUNG: Farbe = Farbe(251, 224, 206);
const TEXT: Farbe = Farbe(26, 26, 26);
const TEXT_SCHWACH: Farbe = Farbe(108, 108, 118);
const ETIKETT: Farbe = Farbe(52, 66, 96);

const TASTEN: &str = "Rad/+/\u{2212} Zoom   Ziehen/Pfeile schieben   Klick/Tab Teil   \
                      Leertaste nur Teil   L Lage   R einpassen   N/P Muster   S sichern   Esc Ende";

// ─────────────────────────────────────────────────────── Zeichenblatt

/// Achsparallele Hülle: x von, x bis, y von, y bis.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Huelle {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl Huelle {
    fn um(punkte: &[Pt]) -> Huelle {
        let mut h = Huelle {
            x0: f64::MAX,
            x1: f64::MIN,
            y0: f64::MAX,
            y1: f64::MIN,
        };
        for p in punkte {
            h.x0 = h.x0.min(p.x);
            h.x1 = h.x1.max(p.x);
            h.y0 = h.y0.min(p.y);
            h.y1 = h.y1.max(p.y);
        }
        h
    }

    fn verschoben(self, v: Pt) -> Huelle {
        Huelle {
            x0: self.x0 + v.x,
            x1: self.x1 + v.x,
            y0: self.y0 + v.y,
            y1: self.y1 + v.y,
        }
    }

    fn vereint(self, o: Huelle) -> Huelle {
        Huelle {
            x0: self.x0.min(o.x0),
            x1: self.x1.max(o.x1),
            y0: self.y0.min(o.y0),
            y1: self.y1.max(o.y1),
        }
    }
}

/// Ein Teil mit seiner fertigen Nahtlinie — ab hier wird nicht mehr gerechnet.
struct Teil {
    name: String,
    /// Der Name ohne den Vorsatz, den alle Teile des Musters tragen.
    kurz: String,
    /// Nahtlinie in Konstruktionskoordinaten, offen (letzter ≠ erster Punkt).
    punkte: Vec<Pt>,
    /// Versatz in der Auslage, in Mustereinheiten.
    auslage: Pt,
    knoten: usize,
    laenge: f64,
    /// Hülle in Konstruktionskoordinaten.
    huell: Huelle,
    /// Wo der Name hinkommt (Konstruktionskoordinaten) und wie breit das Teil
    /// dort ist.
    etikett: (Pt, f64),
}

impl Teil {
    /// Um so viel wird das Teil verschoben gezeichnet.
    fn versatz(&self, ausgelegt: bool) -> Pt {
        if ausgelegt {
            self.auslage
        } else {
            Pt::new(0.0, 0.0)
        }
    }
}

/// Ein Muster mit allen Teilen, in Mustereinheiten.
struct Blatt {
    pfad: PathBuf,
    einheit: String,
    mm_pro_einheit: f64,
    teile: Vec<Teil>,
}

impl Blatt {
    fn laden(pfad: &Path) -> Result<Self, String> {
        let muster = read_path(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
        let mp = muster
            .measurements
            .as_ref()
            .ok_or_else(|| format!("{}: Muster nennt keine Massdatei", pfad.display()))?;
        let mp = if Path::new(mp).is_absolute() {
            PathBuf::from(mp)
        } else {
            pfad.parent().unwrap_or(Path::new(".")).join(mp)
        };
        let mess = lies_messungen(&mp, &muster.unit)
            .map_err(|e| format!("{}: {e}", mp.display()))?;
        let ev = Auswertung::new(&muster, mess);

        let mut teile = Vec::new();
        for p in muster.pieces() {
            let umriss = ev.stueck_umriss(p).unwrap_or_default();
            if umriss.len() < 3 {
                continue;
            }
            let mut ring = umriss.clone();
            ring.push(umriss[0]);
            let laenge = ev.umriss_laenge(&ring);
            teile.push(Teil {
                name: p.name.clone().unwrap_or_else(|| "<ohne Namen>".into()),
                kurz: String::new(),
                auslage: Pt::new(0.0, 0.0),
                laenge,
                knoten: p.nodes.as_ref().map(|n| n.items.len()).unwrap_or(0),
                huell: Huelle::um(&umriss),
                etikett: etikett(&umriss),
                punkte: umriss,
            });
        }
        if teile.is_empty() {
            return Err(format!(
                "{}: kein Teil mit brauchbarer Nahtlinie",
                pfad.display()
            ));
        }
        let namen: Vec<String> = teile.iter().map(|t| t.name.clone()).collect();
        let vorsatz = vorsatz_laenge(&namen);
        let huellen: Vec<Huelle> = teile.iter().map(|t| t.huell).collect();
        for (t, v) in teile.iter_mut().zip(auslegen(&huellen, 1.6)) {
            t.kurz = t.name[vorsatz..].trim().to_string();
            t.auslage = v;
        }
        Ok(Blatt {
            pfad: pfad.to_path_buf(),
            einheit: muster.unit.clone(),
            mm_pro_einheit: ev.mm_pro_einheit(),
            teile,
        })
    }

    fn ueberschrift(&self) -> String {
        self.pfad
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.pfad.display().to_string())
    }

    /// Dateiname ohne Endung — Name der PNG-Datei.
    fn stamm(&self) -> String {
        self.pfad
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "muster".into())
    }

    /// Hülle über die gezeigten Teile; `None`, wenn keins gezeigt wird.
    fn huelle(&self, ausgelegt: bool, zeigen: impl Fn(usize) -> bool) -> Option<Huelle> {
        self.teile
            .iter()
            .enumerate()
            .filter(|(i, _)| zeigen(*i))
            .map(|(_, t)| t.huell.verschoben(t.versatz(ausgelegt)))
            .reduce(Huelle::vereint)
    }
}

/// Teile in Reihen auslegen; Ergebnis ist der Versatz je Teil.
///
/// Das höchste Teil zuerst, von links nach rechts, mit einem Abstand von
/// 6 % des höchsten Teils; eine Reihe endet, wenn sie breiter würde als
/// nötig, damit das Ganze etwa `seitenverhaeltnis` (Breite zu Höhe) hat.
/// Gleich hohe Teile — linkes und rechtes — bleiben in ihrer Reihenfolge
/// nebeneinander. Gedreht wird nichts: die Lage im Muster ist der Fadenlauf.
fn auslegen(huellen: &[Huelle], seitenverhaeltnis: f64) -> Vec<Pt> {
    let breite = |h: &Huelle| h.x1 - h.x0;
    let hoehe = |h: &Huelle| h.y1 - h.y0;
    let abstand = 0.06 * huellen.iter().map(hoehe).fold(0.0, f64::max);
    let flaeche: f64 = huellen
        .iter()
        .map(|h| (breite(h) + abstand) * (hoehe(h) + abstand))
        .sum();
    let reihe_max = (flaeche * seitenverhaeltnis)
        .sqrt()
        .max(huellen.iter().map(breite).fold(0.0, f64::max));

    let mut folge: Vec<usize> = (0..huellen.len()).collect();
    folge.sort_by(|&a, &b| hoehe(&huellen[b]).total_cmp(&hoehe(&huellen[a])));

    let mut versatz = vec![Pt::new(0.0, 0.0); huellen.len()];
    let (mut x, mut y, mut reihe_h) = (0.0, 0.0, 0.0f64);
    for i in folge {
        let h = &huellen[i];
        if x > 0.0 && x + breite(h) > reihe_max {
            x = 0.0;
            y += reihe_h + abstand;
            reihe_h = 0.0;
        }
        versatz[i] = Pt::new(x - h.x0, y - h.y0);
        x += breite(h) + abstand;
        reihe_h = reihe_h.max(hoehe(h));
    }
    versatz
}

/// Länge des Vorsatzes, den **alle** Namen tragen — bis einschliesslich des
/// letzten `" - "` darin. „Bustier - Obercup rechts" wird so zu
/// „Obercup rechts"; ohne gemeinsame Trennstelle bleibt der Name ganz.
fn vorsatz_laenge(namen: &[String]) -> usize {
    let Some(erster) = namen.first() else {
        return 0;
    };
    let mut n = erster.len();
    for name in &namen[1..] {
        n = erster
            .bytes()
            .zip(name.bytes())
            .take(n)
            .take_while(|(a, b)| a == b)
            .count();
    }
    while !erster.is_char_boundary(n) {
        n -= 1;
    }
    erster[..n].rfind(" - ").map(|i| i + 3).unwrap_or(0)
}

/// Liegt `p` im Ring `punkte`? (Strahl nach rechts, Kanten zählen)
fn innen(punkte: &[Pt], p: Pt) -> bool {
    let n = punkte.len();
    let mut drin = false;
    for i in 0..n {
        let (a, b) = (punkte[i], punkte[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) {
            let schnitt = (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x;
            if p.x < schnitt {
                drin = !drin;
            }
        }
    }
    drin
}

/// Flächenschwerpunkt eines Rings (Gausssche Trapezformel); bei
/// entarteten Ringen der Mittelwert der Punkte.
fn schwerpunkt(punkte: &[Pt]) -> Pt {
    let n = punkte.len();
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (punkte[i], punkte[(i + 1) % n]);
        let k = p.x * q.y - q.x * p.y;
        a += k;
        cx += (p.x + q.x) * k;
        cy += (p.y + q.y) * k;
    }
    if a.abs() < 1e-12 {
        let m = n.max(1) as f64;
        return Pt::new(
            punkte.iter().map(|p| p.x).sum::<f64>() / m,
            punkte.iter().map(|p| p.y).sum::<f64>() / m,
        );
    }
    Pt::new(cx / (3.0 * a), cy / (3.0 * a))
}

/// Die Abschnitte der Waagerechten auf Höhe `y`, die im Ring liegen:
/// (von, bis) je Abschnitt, von links nach rechts.
fn abschnitte(punkte: &[Pt], y: f64) -> Vec<(f64, f64)> {
    let n = punkte.len();
    let mut xs = Vec::new();
    for i in 0..n {
        let (a, b) = (punkte[i], punkte[(i + 1) % n]);
        if (a.y > y) != (b.y > y) {
            xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
        }
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.chunks_exact(2).map(|p| (p[0], p[1])).collect()
}

/// Wo der Name eines Teils zuerst versucht wird, und wie breit das Teil
/// dort ist.
///
/// Auf der Waagerechten durch den Flächenschwerpunkt, in die Mitte des
/// breitesten Abschnitts, der im Teil liegt. Der Schwerpunkt allein liegt
/// bei Bogen- und Sichelformen (Kragen, Trim) ausserhalb des Teils.
fn etikett(punkte: &[Pt]) -> (Pt, f64) {
    let s = schwerpunkt(punkte);
    abschnitte(punkte, s.y)
        .into_iter()
        .max_by(|p, q| (p.1 - p.0).total_cmp(&(q.1 - q.0)))
        .map(|(a, b)| (Pt::new((a + b) / 2.0, s.y), b - a))
        .unwrap_or((s, 0.0))
}

/// Wo ein Textkasten von `halb_b` × `halb_h` (halbe Breite und Höhe, in
/// Mustereinheiten) ganz ins Teil passt — `None`, wenn nirgends.
///
/// Zuerst an [`etikett`], dann in der Mitte jedes Abschnitts auf neun
/// weiteren Waagerechten, die dem Schwerpunkt nächsten zuerst. „Ganz im
/// Teil" heisst: Ecken, Kantenmitten und Mitte des Kastens liegen innen.
fn etikett_platz(t: &Teil, halb_b: f64, halb_h: f64) -> Option<Pt> {
    let passt = |m: Pt| {
        [-1.0, 0.0, 1.0].iter().all(|&fx| {
            [-1.0, 0.0, 1.0]
                .iter()
                .all(|&fy| innen(&t.punkte, Pt::new(m.x + fx * halb_b, m.y + fy * halb_h)))
        })
    };
    let erst = t.etikett.0;
    if passt(erst) {
        return Some(erst);
    }
    let h = t.huell;
    let mut kandidaten: Vec<Pt> = (1..=9)
        .map(|k| h.y0 + (h.y1 - h.y0) * k as f64 / 10.0)
        .flat_map(|y| {
            abschnitte(&t.punkte, y)
                .into_iter()
                .filter(|(a, b)| b - a > 2.0 * halb_b)
                .map(move |(a, b)| Pt::new((a + b) / 2.0, y))
        })
        .collect();
    kandidaten.sort_by(|p, q| p.dist(erst).total_cmp(&q.dist(erst)));
    kandidaten.into_iter().find(|&m| passt(m))
}

// ───────────────────────────────────────────────────────── Ansicht

/// Freier Rand beim Einpassen, in Bildpunkten: oben Kopf, unten Fuss.
#[derive(Clone, Copy)]
struct Rand {
    oben: f64,
    unten: f64,
    seite: f64,
}

/// Umrechnung zwischen Musterkoordinaten und Bildpunkten.
struct Sicht {
    /// Bildpunkte je Mustereinheit.
    zoom: f64,
    /// Musterkoordinate links oben.
    versatz: Pt,
    breite: i32,
    hoehe: i32,
}

impl Sicht {
    fn rechne(&self, p: Pt) -> (f64, f64) {
        (
            (p.x - self.versatz.x) * self.zoom,
            (p.y - self.versatz.y) * self.zoom,
        )
    }

    fn zurueck(&self, x: i32, y: i32) -> Pt {
        Pt::new(
            x as f64 / self.zoom + self.versatz.x,
            y as f64 / self.zoom + self.versatz.y,
        )
    }

    /// Die Hülle in die freie Fläche einpassen, mittig.
    fn einpassen(&mut self, h: Huelle, rand: Rand, grenzen: (f64, f64)) {
        let bw = (h.x1 - h.x0).max(1e-9);
        let bh = (h.y1 - h.y0).max(1e-9);
        let frei_b = (self.breite as f64 - 2.0 * rand.seite).max(1.0);
        let frei_h = (self.hoehe as f64 - rand.oben - rand.unten).max(1.0);
        self.zoom = (frei_b / bw).min(frei_h / bh).clamp(grenzen.0, grenzen.1);
        let mitte_x = self.breite as f64 / 2.0;
        let mitte_y = rand.oben + frei_h / 2.0;
        self.versatz = Pt::new(
            (h.x0 + h.x1) / 2.0 - mitte_x / self.zoom,
            (h.y0 + h.y1) / 2.0 - mitte_y / self.zoom,
        );
    }

    /// Zoom um die Stelle (`mx`, `my`) in Bildpunkten; sie bleibt stehen.
    fn zoom_um(&mut self, faktor: f64, mx: i32, my: i32, grenzen: (f64, f64)) {
        let vorher = self.zurueck(mx, my);
        self.zoom = (self.zoom * faktor).clamp(grenzen.0, grenzen.1);
        let nachher = self.zurueck(mx, my);
        self.versatz.x += vorher.x - nachher.x;
        self.versatz.y += vorher.y - nachher.y;
    }
}

/// Liegt der Musterpunkt in diesem Teil, so wie es gezeichnet ist?
fn treffer(teil: &Teil, p: Pt, ausgelegt: bool) -> bool {
    let v = teil.versatz(ausgelegt);
    innen(&teil.punkte, Pt::new(p.x - v.x, p.y - v.y))
}

// ───────────────────────────────────────────────────────── Zustand

struct Zustand {
    blaetter: Vec<Blatt>,
    blatt: usize,
    sicht: Sicht,
    /// Teile ausgelegt ([`auslegen`]) statt in der Konstruktionslage.
    ausgelegt: bool,
    gewaehlt: Option<usize>,
    nur_gewaehlt: bool,
    alle_anzeigen: bool,
    /// Ansicht vor „nur das gewählte Teil" — dahin geht es zurück.
    vorher: Option<(f64, Pt)>,
    zieht: bool,
    bewegt: bool,
    zieht_start: (i32, i32),
    zieht_versatz: Pt,
    /// Beim ersten `WM_SIZE` wird eingepasst, danach nicht mehr von selbst.
    eingepasst: bool,
    /// Bildschirmauflösung: 96 dpi ergibt `skala` 1.
    dpi: f64,
    skala: f64,
    /// Kurze Rückmeldung im Fuss, bis zur nächsten Eingabe.
    meldung: Option<String>,
    /// Zwischenbild fürs Fenster; bei Grössenänderung neu angelegt.
    puffer: Option<Leinwand>,
}

static mut Z: *mut Zustand = std::ptr::null_mut();

/// Der Zustand liegt genau einmal da; ein zweiter Fensteraufruf wäre eine
/// globale Aliasregel, deshalb wird er hier bewusst über einen Rohzeiger
/// geteilt und nie gleichzeitig benutzt. Win32-Aufrufe, die selbst wieder
/// Nachrichten schicken (`SetCapture`, `ReleaseCapture`, `DestroyWindow`),
/// stehen deshalb **nach** dem letzten Gebrauch des Zustands.
#[allow(static_mut_refs)]
fn zustand() -> &'static mut Zustand {
    unsafe { &mut *Z }
}

impl Zustand {
    fn neu(blaetter: Vec<Blatt>) -> Zustand {
        Zustand {
            blaetter,
            blatt: 0,
            sicht: Sicht {
                zoom: 1.0,
                versatz: Pt::new(0.0, 0.0),
                breite: 0,
                hoehe: 0,
            },
            ausgelegt: true,
            gewaehlt: None,
            nur_gewaehlt: false,
            alle_anzeigen: true,
            vorher: None,
            zieht: false,
            bewegt: false,
            zieht_start: (0, 0),
            zieht_versatz: Pt::new(0.0, 0.0),
            eingepasst: false,
            dpi: 96.0,
            skala: 1.0,
            meldung: None,
            puffer: None,
        }
    }

    fn aktuell(&self) -> &Blatt {
        &self.blaetter[self.blatt]
    }

    fn sichtbar(&self, i: usize) -> bool {
        if Some(i) == self.gewaehlt {
            return true;
        }
        if self.nur_gewaehlt {
            return false;
        }
        self.alle_anzeigen
    }

    fn rand(&self) -> Rand {
        Rand {
            oben: 84.0 * self.skala,
            unten: 76.0 * self.skala,
            seite: 32.0 * self.skala,
        }
    }

    /// Zoomgrenzen in Bildpunkten je Mustereinheit: 0,01 bis 200 je mm.
    fn zoom_grenzen(&self) -> (f64, f64) {
        let mpe = self.aktuell().mm_pro_einheit;
        (0.01 * mpe, 200.0 * mpe)
    }

    /// Das Sichtbare einpassen; ist nichts sichtbar, bleibt die Ansicht.
    fn einpassen(&mut self) {
        let h = self.aktuell().huelle(self.ausgelegt, |i| self.sichtbar(i));
        if let Some(h) = h {
            let (rand, grenzen) = (self.rand(), self.zoom_grenzen());
            self.sicht.einpassen(h, rand, grenzen);
        }
    }

    fn zoom_mitte(&mut self, faktor: f64) {
        let grenzen = self.zoom_grenzen();
        let (mx, my) = (self.sicht.breite / 2, self.sicht.hoehe / 2);
        self.sicht.zoom_um(faktor, mx, my, grenzen);
    }

    /// Um ein Achtel der Fenstergrösse verschieben.
    fn schieben(&mut self, dx: f64, dy: f64) {
        let s = &mut self.sicht;
        s.versatz.x += dx * s.breite as f64 / 8.0 / s.zoom;
        s.versatz.y += dy * s.hoehe as f64 / 8.0 / s.zoom;
    }

    fn blatt_wechseln(&mut self, schritt: i32) {
        let n = self.blaetter.len() as i32;
        self.blatt = (self.blatt as i32 + schritt).rem_euclid(n) as usize;
        self.gewaehlt = None;
        self.nur_gewaehlt = false;
        self.vorher = None;
        self.einpassen();
    }

    /// Nächstes (`+1`) oder voriges (`-1`) Teil wählen, im Kreis.
    fn waehle_weiter(&mut self, schritt: i32) {
        let n = self.aktuell().teile.len() as i32;
        if n == 0 {
            return;
        }
        self.gewaehlt = Some(match self.gewaehlt {
            None if schritt > 0 => 0,
            None => (n - 1) as usize,
            Some(i) => (i as i32 + schritt).rem_euclid(n) as usize,
        });
    }

    /// Klick an (`x`, `y`): das getroffene Teil wählen. Liegt das gewählte
    /// unter dem Zeiger, kommt das nächste darunter dran — so erreicht man
    /// in der Konstruktion auch verdeckte Teile.
    fn klick(&mut self, x: i32, y: i32) {
        let w = self.sicht.zurueck(x, y);
        let b = self.aktuell();
        let hier: Vec<usize> = (0..b.teile.len())
            .filter(|&i| self.sichtbar(i) && treffer(&b.teile[i], w, self.ausgelegt))
            .collect();
        let neu = match self.gewaehlt.and_then(|g| hier.iter().position(|&i| i == g)) {
            Some(k) => Some(hier[(k + 1) % hier.len()]),
            None => hier.first().copied(),
        };
        // Im Modus „nur das gewählte Teil" lässt ein Klick daneben die Wahl
        // stehen, sonst wäre das Bild leer.
        if neu.is_some() || !self.nur_gewaehlt {
            self.gewaehlt = neu;
        }
    }

    /// 100 % ist natürliche Grösse auf dem Bildschirm.
    fn zoom_prozent(&self) -> f64 {
        let px_pro_mm = self.sicht.zoom / self.aktuell().mm_pro_einheit;
        px_pro_mm / (self.dpi / 25.4) * 100.0
    }

    fn kopf(&self) -> String {
        let b = self.aktuell();
        match self.gewaehlt {
            None => format!(
                "{} Teile \u{b7} Klick oder Tab w\u{e4}hlt ein Teil",
                b.teile.len()
            ),
            Some(i) => {
                let t = &b.teile[i];
                let f = b.mm_pro_einheit;
                format!(
                    "{}/{}  {}     {} Knoten   {} Punkte   Naht {} mm   {} \u{d7} {} mm",
                    i + 1,
                    b.teile.len(),
                    t.name,
                    t.knoten,
                    t.punkte.len(),
                    komma(t.laenge * f, 1),
                    komma((t.huell.x1 - t.huell.x0) * f, 1),
                    komma((t.huell.y1 - t.huell.y0) * f, 1),
                )
            }
        }
    }

    fn fuss(&self) -> String {
        let b = self.aktuell();
        let mut teile = Vec::new();
        if self.blaetter.len() > 1 {
            teile.push(format!("Muster {}/{}", self.blatt + 1, self.blaetter.len()));
        }
        teile.push(format!("1 {} = {} mm", b.einheit, fmt(b.mm_pro_einheit)));
        teile.push(format!("Zoom {} %", prozent(self.zoom_prozent())));
        teile.push(if self.ausgelegt { "ausgelegt" } else { "Konstruktionslage" }.into());
        if self.nur_gewaehlt {
            teile.push("nur gew\u{e4}hltes Teil".into());
        }
        teile.join("   ")
    }
}

/// Zahl ohne überflüssige Nullen, mit Komma als Dezimaltrenner.
fn fmt(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".into()
    } else {
        s.replace('.', ",")
    }
}

/// Zahl mit fester Stellenzahl und Komma.
fn komma(v: f64, stellen: usize) -> String {
    format!("{v:.stellen$}").replace('.', ",")
}

/// Prozentwert: ab 10 ganzzahlig, darunter eine Stelle.
fn prozent(v: f64) -> String {
    if v >= 10.0 {
        format!("{v:.0}")
    } else {
        komma(v, 1)
    }
}

// ─────────────────────────────────────────────────────────── Zeichnen

/// Ein Teil in Bildpunkten — der Ring wird **geschlossen**.
///
/// `Polyline` zeichnet keinen Ringschluss, deshalb kommt der erste Punkt am
/// Ende noch einmal dazu. `Polygon` stört der doppelte Punkt nicht.
fn zu_pixel(s: &Sicht, t: &Teil, ausgelegt: bool) -> Vec<POINT> {
    // GDI rechnet intern mit 28 Bit; weiter draussen liegt ohnehin nichts
    // Sichtbares.
    const GRENZE: f64 = (1 << 26) as f64;
    let v = t.versatz(ausgelegt);
    let mut r: Vec<POINT> = t
        .punkte
        .iter()
        .map(|p| {
            let (x, y) = s.rechne(Pt::new(p.x + v.x, p.y + v.y));
            POINT {
                x: x.round().clamp(-GRENZE, GRENZE) as i32,
                y: y.round().clamp(-GRENZE, GRENZE) as i32,
            }
        })
        .collect();
    if let Some(erste) = r.first().copied() {
        r.push(erste);
    }
    r
}

/// Eine Schrift; `CreateFontW` kopiert den Namen sofort in seine `LOGFONT`.
unsafe fn schrift(hoehe: i32, gewicht: i32) -> HGDIOBJ {
    let name = wide("Segoe UI");
    CreateFontW(
        -hoehe,
        0,
        0,
        0,
        gewicht,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        0,
        0,
        CLEARTYPE_QUALITY,
        FF_SWISS,
        name.as_ptr(),
    )
}

unsafe fn text(dc: HDC, x: i32, y: i32, s: &str) {
    let w: Vec<u16> = s.encode_utf16().collect();
    if !w.is_empty() {
        TextOutW(dc, x, y, w.as_ptr(), w.len() as i32);
    }
}

/// Breite und Höhe eines Textes in der gewählten Schrift.
unsafe fn textmass(dc: HDC, s: &str) -> (i32, i32) {
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut g = SIZE::default();
    GetTextExtentPoint32W(dc, w.as_ptr(), w.len() as i32, &mut g);
    (g.cx, g.cy)
}

/// Ein Bild des aktuellen Musters auf `dc`, in der Grösse der Sicht.
///
/// Fenster, Taste `S` und `--png` gehen alle hier durch — es gibt keinen
/// zweiten Zeichenweg, der anders aussehen könnte.
unsafe fn zeichnen(z: &Zustand, dc: HDC) {
    let b = z.aktuell();
    let s = &z.sicht;
    let k = |v: f64| (v * z.skala).round() as i32;

    // 0. Grund. `FillRect` zieht keinen Rand, anders als `Rectangle`.
    let grund = Objekt(CreateSolidBrush(HINTERGRUND.cref()));
    let rc = RECT {
        left: 0,
        top: 0,
        right: s.breite,
        bottom: s.hoehe,
    };
    FillRect(dc, &rc, grund.0);

    let ringe: Vec<Option<Vec<POINT>>> = b
        .teile
        .iter()
        .enumerate()
        .map(|(i, t)| z.sichtbar(i).then(|| zu_pixel(s, t, z.ausgelegt)))
        .collect();

    // 1. Füllung, das gewählte Teil zuletzt. Der leere Stift ist Pflicht:
    //    `Polygon` zieht sonst die Kontur mit dem gerade gewählten Stift mit —
    //    beim Standardstift schwarz, über jeder Nahtlinie.
    {
        let _kein_rand = Auswahl::neu(dc, GetStockObject(NULL_PEN));
        let fuellung = Objekt(CreateSolidBrush(FUELLUNG.cref()));
        let auswahl = Objekt(CreateSolidBrush(AUSWAHL_FUELLUNG.cref()));
        {
            let _p = Auswahl::neu(dc, fuellung.0);
            for (i, r) in ringe.iter().enumerate() {
                match r {
                    Some(r) if Some(i) != z.gewaehlt => {
                        Polygon(dc, r.as_ptr(), r.len() as i32);
                    }
                    _ => {}
                }
            }
        }
        if let Some(r) = z.gewaehlt.and_then(|i| ringe[i].as_ref()) {
            let _p = Auswahl::neu(dc, auswahl.0);
            Polygon(dc, r.as_ptr(), r.len() as i32);
        }
    }

    // 2. Nahtlinien, darüber die gewählte dick und farbig.
    {
        let naht = Objekt(CreatePen(PS_SOLID, k(1.0).max(1), NAHT.cref()));
        let _s = Auswahl::neu(dc, naht.0);
        for r in ringe.iter().flatten() {
            Polyline(dc, r.as_ptr(), r.len() as i32);
        }
    }
    if let Some(r) = z.gewaehlt.and_then(|i| ringe[i].as_ref()) {
        let dick = Objekt(CreatePen(PS_SOLID, k(3.0).max(2), AUSGEWAEHLT.cref()));
        let _s = Auswahl::neu(dc, dick.0);
        Polyline(dc, r.as_ptr(), r.len() as i32);
    }

    SetBkMode(dc, TRANSPARENT);

    // 3. Teilenamen — nur, wo der ganze Text ins Teil passt.
    {
        let klein = Objekt(schrift(k(13.0), FW_NORMAL));
        let _f = Auswahl::neu(dc, klein.0);
        SetTextColor(dc, ETIKETT.cref());
        for (i, t) in b.teile.iter().enumerate() {
            if ringe[i].is_none() || t.kurz.is_empty() {
                continue;
            }
            let (tb, th) = textmass(dc, &t.kurz);
            // Kasten mit etwas Luft zur Naht, in Mustereinheiten.
            let halb_b = (tb as f64 / 2.0 + k(4.0) as f64) / s.zoom;
            let halb_h = (th as f64 / 2.0 + k(1.0) as f64) / s.zoom;
            let Some(mitte) = etikett_platz(t, halb_b, halb_h) else {
                continue;
            };
            let v = t.versatz(z.ausgelegt);
            let (x, y) = s.rechne(Pt::new(mitte.x + v.x, mitte.y + v.y));
            text(dc, x.round() as i32 - tb / 2, y.round() as i32 - th / 2, &t.kurz);
        }
    }

    // 4. Kopf, Fuss, Massstab.
    let titel = Objekt(schrift(k(18.0), FW_SEMIBOLD));
    let normal = Objekt(schrift(k(14.0), FW_NORMAL));
    {
        let _f = Auswahl::neu(dc, titel.0);
        SetTextColor(dc, TEXT.cref());
        text(dc, k(20.0), k(14.0), &b.ueberschrift());
    }
    let _f = Auswahl::neu(dc, normal.0);
    SetTextColor(dc, TEXT_SCHWACH.cref());
    text(dc, k(20.0), k(46.0), &z.kopf());
    let unten = s.hoehe - k(30.0);
    let rechts = z.meldung.as_deref().unwrap_or(TASTEN);
    text(dc, k(20.0), unten, &format!("{}        {}", z.fuss(), rechts));
    massstab(dc, z, s.breite - k(28.0), unten - k(12.0));
}

/// Massstab rechts unten: die längste runde Strecke bis 160 Bildpunkte.
unsafe fn massstab(dc: HDC, z: &Zustand, rechts: i32, unten: i32) {
    const STUFEN: [f64; 13] = [
        1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0,
    ];
    let k = |v: f64| (v * z.skala).round() as i32;
    let px_pro_mm = z.sicht.zoom / z.aktuell().mm_pro_einheit;
    let Some(&mm) = STUFEN
        .iter()
        .rev()
        .find(|&&mm| mm * px_pro_mm <= 160.0 * z.skala)
    else {
        return;
    };
    let laenge = (mm * px_pro_mm).round() as i32;
    if laenge < 6 {
        return;
    }
    let (x0, x1) = (rechts - laenge, rechts);
    let strich = [
        POINT { x: x0, y: unten - k(5.0) },
        POINT { x: x0, y: unten },
        POINT { x: x1, y: unten },
        POINT { x: x1, y: unten - k(6.0) },
    ];
    let stift = Objekt(CreatePen(PS_SOLID, k(1.0).max(1), TEXT_SCHWACH.cref()));
    let _s = Auswahl::neu(dc, stift.0);
    Polyline(dc, strich.as_ptr(), strich.len() as i32);
    let wert = if mm >= 1000.0 {
        format!("{} m", fmt(mm / 1000.0))
    } else if mm >= 10.0 {
        format!("{} cm", fmt(mm / 10.0))
    } else {
        format!("{} mm", fmt(mm))
    };
    let (tb, th) = textmass(dc, &wert);
    text(dc, (x0 + x1) / 2 - tb / 2, unten - k(7.0) - th, &wert);
}

/// Ins Fenster malen: auf den Puffer zeichnen, dann in einem Zug kopieren.
unsafe fn malen(hdc: HDC) {
    let z = zustand();
    let (b, h) = (z.sicht.breite, z.sicht.hoehe);
    let mut puffer = z.puffer.take();
    if puffer.as_ref().map(|l| (l.breite, l.hoehe)) != Some((b, h)) {
        puffer = Leinwand::neu(b, h);
    }
    if let Some(l) = &puffer {
        zeichnen(z, l.dc);
        BitBlt(hdc, 0, 0, b, h, l.dc, 0, 0, SRCCOPY);
    }
    z.puffer = puffer;
}

/// Das aktuelle Muster in der Grösse der Sicht als PNG.
fn bild(z: &Zustand, ziel: &Path) -> Result<(), String> {
    let l = Leinwand::neu(z.sicht.breite, z.sicht.hoehe)
        .ok_or_else(|| format!("keine Leinwand {}x{}", z.sicht.breite, z.sicht.hoehe))?;
    unsafe { zeichnen(z, l.dc) };
    png::schreibe(ziel, l.breite, l.hoehe, &l.rgba()).map_err(|e| e.to_string())
}

/// Taste `S`: neben der `.exe` sichern, ohne ein früheres Bild zu
/// überschreiben (`bustier_v1.png`, `bustier_v1-2.png`, …).
fn bild_sichern(z: &Zustand) -> Result<PathBuf, String> {
    let ordner = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let stamm = z.aktuell().stamm();
    let ziel = (1..10_000)
        .map(|n| match n {
            1 => ordner.join(format!("{stamm}.png")),
            n => ordner.join(format!("{stamm}-{n}.png")),
        })
        .find(|p| !p.exists())
        .ok_or_else(|| "kein freier Dateiname".to_string())?;
    bild(z, &ziel)?;
    Ok(ziel)
}

/// `--png`: jedes Muster einmal, eingepasst, ohne Fenster.
fn bilder_schreiben(z: &mut Zustand, ordner: &Path, teil: Option<usize>) -> bool {
    if let Err(e) = std::fs::create_dir_all(ordner) {
        eprintln!("  {}: {e}", ordner.display());
        return false;
    }
    let mut gut = true;
    for i in 0..z.blaetter.len() {
        z.blatt = i;
        z.gewaehlt = teil.filter(|&t| t < z.aktuell().teile.len());
        z.einpassen();
        let ziel = ordner.join(format!("{}.png", z.aktuell().stamm()));
        match bild(z, &ziel) {
            Ok(()) => println!("  {}", ziel.display()),
            Err(e) => {
                eprintln!("  {}: {e}", ziel.display());
                gut = false;
            }
        }
    }
    gut
}

// ─────────────────────────────────────────────────────── FensterProc

unsafe fn neu_zeichnen(hwnd: HWND) {
    InvalidateRect(hwnd, std::ptr::null(), 0);
}

unsafe fn titel_setzen(hwnd: HWND, datei: &str) {
    let t = wide(&format!("klon3d \u{2014} {datei}"));
    SetWindowTextW(hwnd, t.as_ptr());
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            malen(hdc);
            EndPaint(hwnd, &ps);
            0
        }
        // Der Puffer deckt alles ab; ohne das flackert das Fenster.
        WM_ERASEBKGND => 1,
        WM_SIZE => {
            let mut rc = RECT::default();
            GetClientRect(hwnd, &mut rc);
            let z = zustand();
            z.sicht.breite = rc.right - rc.left;
            z.sicht.hoehe = rc.bottom - rc.top;
            if !z.eingepasst && z.sicht.breite > 0 && z.sicht.hoehe > 0 {
                z.einpassen();
                z.eingepasst = true;
            }
            0
        }
        WM_LBUTTONDOWN => {
            {
                let z = zustand();
                z.zieht = true;
                z.bewegt = false;
                z.zieht_start = (lparam_x(lp), lparam_y(lp));
                z.zieht_versatz = z.sicht.versatz;
            }
            SetCapture(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            let z = zustand();
            if z.zieht {
                let (mx, my) = (lparam_x(lp), lparam_y(lp));
                let (dx, dy) = (mx - z.zieht_start.0, my - z.zieht_start.1);
                if dx.abs() + dy.abs() > 3 {
                    z.bewegt = true;
                }
                if z.bewegt {
                    z.sicht.versatz = Pt::new(
                        z.zieht_versatz.x - dx as f64 / z.sicht.zoom,
                        z.zieht_versatz.y - dy as f64 / z.sicht.zoom,
                    );
                    neu_zeichnen(hwnd);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            {
                let z = zustand();
                if z.zieht && !z.bewegt {
                    z.klick(lparam_x(lp), lparam_y(lp));
                    z.meldung = None;
                }
                z.zieht = false;
            }
            ReleaseCapture();
            neu_zeichnen(hwnd);
            0
        }
        WM_CAPTURECHANGED => {
            zustand().zieht = false;
            0
        }
        WM_MOUSEWHEEL => {
            let delta = wheel_delta(wp);
            if delta != 0 {
                let mut p = POINT {
                    x: lparam_x(lp),
                    y: lparam_y(lp),
                };
                ScreenToClient(hwnd, &mut p);
                let z = zustand();
                let grenzen = z.zoom_grenzen();
                let faktor = 1.15f64.powf(delta as f64 / WHEEL_DELTA as f64);
                z.sicht.zoom_um(faktor, p.x, p.y, grenzen);
                neu_zeichnen(hwnd);
            }
            0
        }
        WM_KEYDOWN => {
            taste(hwnd, (wp & 0xFF) as u8);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn taste(hwnd: HWND, t: u8) {
    if t == VK_ESCAPE {
        DestroyWindow(hwnd);
        return;
    }
    let z = zustand();
    z.meldung = None;
    match t {
        b'R' => z.einpassen(),
        b'N' | b'P' => {
            z.blatt_wechseln(if t == b'N' { 1 } else { -1 });
            let datei = z.aktuell().ueberschrift();
            titel_setzen(hwnd, &datei);
        }
        b'L' => {
            z.ausgelegt = !z.ausgelegt;
            z.einpassen();
        }
        b'A' => z.alle_anzeigen = !z.alle_anzeigen,
        VK_SPACE => {
            if z.gewaehlt.is_none() {
                z.meldung = Some("Erst ein Teil w\u{e4}hlen: Klick oder Tab".into());
            } else if !z.nur_gewaehlt {
                z.vorher = Some((z.sicht.zoom, z.sicht.versatz));
                z.nur_gewaehlt = true;
                z.einpassen();
            } else {
                z.nur_gewaehlt = false;
                if let Some((zoom, versatz)) = z.vorher.take() {
                    z.sicht.zoom = zoom;
                    z.sicht.versatz = versatz;
                }
            }
        }
        VK_TAB => {
            let zurueck = GetKeyState(VK_SHIFT) < 0;
            z.waehle_weiter(if zurueck { -1 } else { 1 });
            if z.nur_gewaehlt {
                z.einpassen();
            }
        }
        VK_ADD | VK_OEM_PLUS => z.zoom_mitte(1.25),
        VK_SUBTRACT | VK_OEM_MINUS => z.zoom_mitte(1.0 / 1.25),
        VK_LEFT => z.schieben(-1.0, 0.0),
        VK_RIGHT => z.schieben(1.0, 0.0),
        VK_UP => z.schieben(0.0, -1.0),
        VK_DOWN => z.schieben(0.0, 1.0),
        b'S' => {
            let m = match bild_sichern(z) {
                Ok(p) => format!("Gesichert: {}", p.display()),
                Err(e) => format!("Nicht gesichert: {e}"),
            };
            println!("{m}");
            z.meldung = Some(m);
        }
        _ => return,
    }
    neu_zeichnen(hwnd);
}

// ───────────────────────────────────────────────────────────── Start

/// Was auf der Kommandozeile stand.
struct Aufruf {
    pfade: Vec<PathBuf>,
    png: Option<PathBuf>,
    groesse: (i32, i32),
    konstruktion: bool,
    /// Gewähltes Teil, ab 0 gezählt (auf der Kommandozeile ab 1).
    teil: Option<usize>,
}

impl Aufruf {
    fn lesen(args: &[String]) -> Result<Aufruf, String> {
        let mut a = Aufruf {
            pfade: Vec::new(),
            png: None,
            groesse: (1600, 1000),
            konstruktion: false,
            teil: None,
        };
        let mut it = args.iter();
        while let Some(x) = it.next() {
            match x.as_str() {
                "--png" => {
                    a.png = Some(PathBuf::from(it.next().ok_or("--png braucht einen Ordner")?));
                }
                "--groesse" => {
                    let s = it.next().ok_or("--groesse braucht BREITExHOEHE")?;
                    a.groesse = groesse(s)
                        .ok_or_else(|| format!("--groesse {s}: erwartet BREITExHOEHE, etwa 1600x1000"))?;
                }
                "--konstruktion" => a.konstruktion = true,
                "--teil" => {
                    let s = it.next().ok_or("--teil braucht eine Nummer ab 1")?;
                    match s.parse::<usize>() {
                        Ok(n) if n >= 1 => a.teil = Some(n - 1),
                        _ => return Err(format!("--teil {s}: erwartet eine Nummer ab 1")),
                    }
                }
                "-h" | "--hilfe" | "--help" => return Err(String::new()),
                s if s.starts_with("--") => return Err(format!("unbekannte Option {s}")),
                s => a.pfade.push(PathBuf::from(s)),
            }
        }
        Ok(a)
    }
}

/// `1600x1000` → (1600, 1000); höchstens 16000 je Seite.
fn groesse(s: &str) -> Option<(i32, i32)> {
    let (b, h) = s.split_once(['x', 'X', '\u{d7}'])?;
    let (b, h): (i32, i32) = (b.trim().parse().ok()?, h.trim().parse().ok()?);
    ((1..=16000).contains(&b) && (1..=16000).contains(&h)).then_some((b, h))
}

fn hilfe() {
    eprintln!("klon3d-fenster <muster.sm2d> [weiteres.sm2d ...]");
    eprintln!("    zeigt die Nahtlinien der Teile; N und P wechseln zwischen den Mustern");
    eprintln!();
    eprintln!(
        "klon3d-fenster --png <ordner> [--groesse 1600x1000] [--konstruktion] [--teil N] <muster.sm2d> ..."
    );
    eprintln!("    zeichnet jedes Muster ohne Fenster und schreibt <ordner>\\<muster>.png");
    eprintln!();
    if let Some(b) = beispiel() {
        eprintln!("  Beispiel:");
        eprintln!("    klon3d-fenster \"{}\"", b.display());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let aufruf = match Aufruf::lesen(&args) {
        Ok(a) if !a.pfade.is_empty() => a,
        Ok(_) => {
            hilfe();
            std::process::exit(2);
        }
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
                eprintln!();
            }
            hilfe();
            std::process::exit(2);
        }
    };

    let mut blaetter = Vec::new();
    for p in &aufruf.pfade {
        match Blatt::laden(p) {
            Ok(b) => {
                println!("  {:<38} {:>3} Teile", b.ueberschrift(), b.teile.len());
                blaetter.push(b);
            }
            Err(e) => eprintln!("  {e}"),
        }
    }
    if blaetter.is_empty() {
        eprintln!("kein Muster lesbar");
        std::process::exit(1);
    }

    let mut z = Zustand::neu(blaetter);
    z.ausgelegt = !aufruf.konstruktion;

    if let Some(ordner) = &aufruf.png {
        z.sicht.breite = aufruf.groesse.0;
        z.sicht.hoehe = aufruf.groesse.1;
        let gut = bilder_schreiben(&mut z, ordner, aufruf.teil);
        std::process::exit(if gut { 0 } else { 1 });
    }

    // Ab hier mit Fenster. Ohne `SetProcessDPIAware` streckt Windows bei
    // 125 % oder 150 % Skalierung das fertige Bild — Linien und Schrift
    // werden unscharf. So zeichnet der Betrachter selbst in voller Auflösung
    // und skaliert Schrift, Ränder und Stifte mit.
    unsafe {
        SetProcessDPIAware();
        z.dpi = bildschirm_dpi();
    }
    z.skala = z.dpi / 96.0;
    z.gewaehlt = aufruf.teil.filter(|&t| t < z.aktuell().teile.len());
    let titel = z.aktuell().ueberschrift();

    unsafe {
        let breite = ((1280.0 * z.skala) as i32).min(GetSystemMetrics(SM_CXFULLSCREEN) * 94 / 100);
        let hoehe = ((900.0 * z.skala) as i32).min(GetSystemMetrics(SM_CYFULLSCREEN) * 94 / 100);
        Z = Box::into_raw(Box::new(z));
        // Ab hier ist jeder Win32-Aufruf unsicher — er geht über
        // `extern "system"`, und ein Rohzeiger zeigt es an.
        fenster_aufmachen(&titel, breite.max(640), hoehe.max(480));

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        drop(Box::from_raw(Z));
        Z = std::ptr::null_mut();
    }
    println!("Fenster geschlossen.");
}

/// Bildpunkte je Zoll des Bildschirms; 96, wenn GDI nichts sagt.
unsafe fn bildschirm_dpi() -> f64 {
    let dc = GetDC(std::ptr::null_mut());
    if dc.is_null() {
        return 96.0;
    }
    let dpi = GetDeviceCaps(dc, LOGPIXELSX);
    ReleaseDC(std::ptr::null_mut(), dc);
    if dpi > 0 {
        dpi as f64
    } else {
        96.0
    }
}

/// Fensterklasse anmelden und das Fenster öffnen.
unsafe fn fenster_aufmachen(datei: &str, breite: i32, hoehe: i32) {
    let klasse = wide("Klon3dFenster");
    let inst = GetModuleHandleW(std::ptr::null());

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: wndproc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: inst,
        hIcon: std::ptr::null_mut(),
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW as *const u16),
        // Kein Hintergrundpinsel: der Puffer deckt jeden Bildpunkt ab.
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: klasse.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    if RegisterClassExW(&wc) == 0 {
        eprintln!("Fensterklasse liess sich nicht anmelden (Fehler {})", GetLastError());
        std::process::exit(1);
    }

    let titel = wide(&format!("klon3d \u{2014} {datei}"));
    let hwnd = CreateWindowExW(
        0,
        wc.lpszClassName,
        titel.as_ptr(),
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        breite,
        hoehe,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        inst,
        std::ptr::null_mut(),
    );
    if hwnd.is_null() {
        eprintln!("Fenster liess sich nicht oeffnen (Fehler {})", GetLastError());
        std::process::exit(1);
    }
    ShowWindow(hwnd, SW_SHOWNORMAL);
    UpdateWindow(hwnd);
}

/// `CW_USEDEFAULT` ist `INT_MIN`.
const CW_USEDEFAULT: i32 = i32::MIN;

/// Muster aus dem Prüfbestand, damit der Aufruf ohne Argument nicht ins Leere
/// läuft.
fn beispiel() -> Option<PathBuf> {
    let w = Path::new(env!("CARGO_MANIFEST_DIR")).join("../klon3d-pattern/tests/muster");
    let mut sortiert: Vec<PathBuf> = std::fs::read_dir(w)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "sm2d").unwrap_or(false))
        .collect();
    sortiert.sort();
    sortiert.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn muster(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../klon3d-pattern/tests/muster")
            .join(name)
    }

    fn rgba(f: Farbe) -> [u8; 4] {
        [f.0, f.1, f.2, 255]
    }

    /// Ein Muster eingepasst auf eine Leinwand zeichnen.
    fn gezeichnet(datei: &str, breite: i32, hoehe: i32, teil: Option<usize>) -> (Zustand, Vec<u8>) {
        let mut z = Zustand::neu(vec![Blatt::laden(&muster(datei)).unwrap()]);
        z.sicht.breite = breite;
        z.sicht.hoehe = hoehe;
        z.gewaehlt = teil;
        z.einpassen();
        let l = Leinwand::neu(breite, hoehe).unwrap();
        unsafe { zeichnen(&z, l.dc) };
        let px = l.rgba();
        (z, px)
    }

    fn bei(px: &[u8], breite: i32, x: i32, y: i32) -> [u8; 4] {
        let i = ((y * breite + x) * 4) as usize;
        px[i..i + 4].try_into().unwrap()
    }

    /// Der Prüffall aus Befund 15: Nahtlinien und Schrift stehen im Bild, und
    /// zwar undurchsichtig und kanalrichtig.
    #[test]
    fn nahtlinien_und_schrift_stehen_im_bild() {
        let (b, h) = (1200, 800);
        let (z, px) = gezeichnet("bustier_v1.sm2d", b, h, None);
        assert!(px.chunks_exact(4).all(|p| p[3] == 255), "Alpha überall 255");
        assert_eq!(bei(&px, b, 2, 2), rgba(HINTERGRUND));

        let (mut naht, mut alle) = (0, 0);
        for t in &z.aktuell().teile {
            for p in zu_pixel(&z.sicht, t, z.ausgelegt) {
                if p.x >= 0 && p.y >= 0 && p.x < b && p.y < h {
                    alle += 1;
                    if bei(&px, b, p.x, p.y) == rgba(NAHT) {
                        naht += 1;
                    }
                }
            }
        }
        assert!(alle > 500, "nur {alle} Ringpunkte im Bild");
        assert!(naht * 100 >= alle * 97, "{naht} von {alle} Ringpunkten in Nahtfarbe");

        // Füllung ist da, und zwar in der richtigen Farbe.
        let fuellung = px.chunks_exact(4).filter(|p| *p == rgba(FUELLUNG)).count();
        assert!(fuellung > 20_000, "{fuellung} Bildpunkte Füllung");

        // Kopfzeile: dunkle Schrift auf hellem Grund.
        let mut dunkel = 0;
        for y in 10..40 {
            for x in 10..400 {
                let p = bei(&px, b, x, y);
                if p[0] < 90 && p[1] < 90 && p[2] < 90 {
                    dunkel += 1;
                }
            }
        }
        assert!(dunkel > 100, "{dunkel} dunkle Bildpunkte im Kopf");
    }

    /// Das gewählte Teil hat eine farbige, dicke Kontur und eine eigene
    /// Füllung.
    #[test]
    fn gewaehltes_teil_ist_hervorgehoben() {
        let (b, h) = (1000, 700);
        let (z, px) = gezeichnet("bluse3_v1.sm2d", b, h, Some(0));
        let t = &z.aktuell().teile[0];
        let ring = zu_pixel(&z.sicht, t, z.ausgelegt);
        let orange = ring
            .iter()
            .filter(|p| bei(&px, b, p.x, p.y) == rgba(AUSGEWAEHLT))
            .count();
        assert!(orange * 100 >= ring.len() * 97, "{orange} von {}", ring.len());
        let flaeche = px.chunks_exact(4).filter(|p| *p == rgba(AUSWAHL_FUELLUNG)).count();
        assert!(flaeche > 1000, "{flaeche}");
    }

    /// Wie viele Hüllenpaare sich überschneiden.
    fn ueberlappungen(b: &Blatt, ausgelegt: bool) -> usize {
        let hs: Vec<Huelle> = b
            .teile
            .iter()
            .map(|t| t.huell.verschoben(t.versatz(ausgelegt)))
            .collect();
        let mut n = 0;
        for i in 0..hs.len() {
            for j in i + 1..hs.len() {
                let (a, c) = (hs[i], hs[j]);
                if a.x0 < c.x1 && c.x0 < a.x1 && a.y0 < c.y1 && c.y0 < a.y1 {
                    n += 1;
                }
            }
        }
        n
    }

    /// Ausgelegt überschneidet sich in keinem Prüfmuster eine Hülle mit
    /// einer anderen; in der Konstruktionslage der Bluse 3 schon.
    #[test]
    fn auslage_ueberlappt_nicht() {
        for datei in ["bustier_v1.sm2d", "bluse2_v26.sm2d", "bluse2_v29.sm2d", "bluse3_v1.sm2d"] {
            let b = Blatt::laden(&muster(datei)).unwrap();
            assert_eq!(ueberlappungen(&b, true), 0, "{datei}");
        }
        let b = Blatt::laden(&muster("bluse3_v1.sm2d")).unwrap();
        assert!(ueberlappungen(&b, false) > 0);
    }

    #[test]
    fn vorsatz_nur_an_der_trennstelle() {
        let n = |v: &[&str]| vorsatz_laenge(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(n(&["Bustier - Vorderteil", "Bustier - Rueckteil"]), "Bustier - ".len());
        // Gemeinsam ist "Bluse 2 - R"; geschnitten wird nur an " - ".
        assert_eq!(n(&["Bluse 2 - Rueckenteil", "Bluse 2 - Raglan"]), "Bluse 2 - ".len());
        assert_eq!(n(&["Vorderteil", "Rueckteil"]), 0);
        // Gemeinsames erstes Byte eines Umlauts darf nicht zerschneiden.
        assert_eq!(n(&["\u{c4}rmel - a", "\u{d6}se - b"]), 0);
        assert_eq!(n(&[]), 0);
    }

    #[test]
    fn etikett_liegt_auch_in_einer_u_form_im_teil() {
        // Der Flächenschwerpunkt eines U liegt in der Öffnung, ausserhalb.
        let u = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (7.0, 10.0),
            (7.0, 3.0),
            (3.0, 3.0),
            (3.0, 10.0),
            (0.0, 10.0),
        ]
        .map(|(x, y)| Pt::new(x, y));
        assert!(!innen(&u, schwerpunkt(&u)));
        let (p, weite) = etikett(&u);
        assert!(innen(&u, p), "{p:?}");
        assert!((weite - 3.0).abs() < 1e-9);
    }

    /// Passt der Name nicht auf die Schwerpunktlinie, rückt er dorthin, wo
    /// der ganze Kasten im Teil liegt — hier in den Steg des U.
    #[test]
    fn etikett_weicht_aus_wenn_der_kasten_nicht_passt() {
        let punkte: Vec<Pt> = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (7.0, 10.0),
            (7.0, 3.0),
            (3.0, 3.0),
            (3.0, 10.0),
            (0.0, 10.0),
        ]
        .map(|(x, y)| Pt::new(x, y))
        .to_vec();
        let t = Teil {
            name: "U".into(),
            kurz: "U".into(),
            auslage: Pt::new(0.0, 0.0),
            knoten: 8,
            laenge: 0.0,
            huell: Huelle::um(&punkte),
            etikett: etikett(&punkte),
            punkte,
        };
        // 4 breit: in die Schenkel (3 breit) passt es nicht, in den Steg schon.
        let m = etikett_platz(&t, 2.0, 0.5).expect("Platz im Steg");
        assert!(m.y > 0.5 && m.y < 2.5, "{m:?}");
        assert!((m.x - 5.0).abs() < 1e-9);
        // 12 breit passt nirgends.
        assert!(etikett_platz(&t, 6.0, 0.5).is_none());
    }

    #[test]
    fn zoom_haelt_den_punkt_unter_der_maus_fest() {
        let mut s = Sicht {
            zoom: 3.0,
            versatz: Pt::new(10.0, 20.0),
            breite: 800,
            hoehe: 600,
        };
        let vorher = s.zurueck(200, 150);
        s.zoom_um(1.15, 200, 150, (0.01, 1000.0));
        assert!(vorher.dist(s.zurueck(200, 150)) < 1e-9);
        assert!((s.zoom - 3.45).abs() < 1e-12);
    }

    #[test]
    fn klick_wechselt_durch_uebereinanderliegende_teile() {
        let mut z = Zustand::neu(vec![Blatt::laden(&muster("bluse3_v1.sm2d")).unwrap()]);
        z.ausgelegt = false; // Konstruktionslage: Kragen liegen über dem Rücken
        z.sicht.breite = 1200;
        z.sicht.hoehe = 800;
        z.einpassen();
        // Eine Stelle suchen, an der mindestens zwei Teile liegen.
        let b = z.aktuell();
        let stelle = (0..1200)
            .step_by(7)
            .flat_map(|x| (0..800).step_by(7).map(move |y| (x, y)))
            .find(|&(x, y)| {
                let w = z.sicht.zurueck(x, y);
                b.teile.iter().filter(|t| treffer(t, w, false)).count() >= 2
            })
            .expect("überlappende Teile in der Konstruktion");
        z.klick(stelle.0, stelle.1);
        let erstes = z.gewaehlt.expect("ein Teil gewählt");
        z.klick(stelle.0, stelle.1);
        assert_ne!(z.gewaehlt, Some(erstes));
    }

    #[test]
    fn aufruf_liest_png_groesse_und_teil() {
        let a = Aufruf::lesen(
            &["--png", "bilder", "--groesse", "800x600", "--teil", "3", "a.sm2d"]
                .map(String::from),
        )
        .unwrap();
        assert_eq!(a.png, Some(PathBuf::from("bilder")));
        assert_eq!(a.groesse, (800, 600));
        assert_eq!(a.teil, Some(2));
        assert_eq!(a.pfade, vec![PathBuf::from("a.sm2d")]);
        assert!(Aufruf::lesen(&["--teil".to_string(), "0".to_string()]).is_err());
        assert!(Aufruf::lesen(&["--unsinn".to_string()]).is_err());
        assert_eq!(groesse("1600x1000"), Some((1600, 1000)));
        assert_eq!(groesse("0x10"), None);
    }
}
