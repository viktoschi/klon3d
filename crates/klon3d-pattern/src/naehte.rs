//! Nahtpaare: welche Randkanten zusammengehören, und ob beide Seiten gleich
//! lang sind.
//!
//! Seamly2D speichert keine Nähte. Die Paare kommen deshalb aus einer
//! **Nahtliste** neben dem Muster (`<muster>.naehte`, Format unten). Eine
//! Nahtseite ist eine Folge **ganzer** Randkanten ([`crate::kanten`]) auf
//! einem oder mehreren Teilen — das deckt 1:1 und 1:N ab (Seitennaht aus drei
//! Teilen gegen ein Rückenteil). Teilstücke einer Kante gibt es nicht.
//!
//! ## Was geprüft wird
//!
//! * **Länge** je Seite = Summe ihrer Kanten. Die Abweichung Δ = L_A − L_B
//!   steht wie in CLOs Nahtanzeige als `Δ (L_A / L_B)`. CLO färbt eine Naht
//!   erst ab 5 mm oder 10 % rot (Spezifikation `naehte_2026_1.md`,
//!   Abschnitt 3); die Abnahme in Stufe 3 verlangt |Δ| ≤ 0,08 cm.
//! * **Kerben:** CLO ordnet beim Nähen gleiche Längenanteile einander zu
//!   (Abschnitt 4), Kerben kommen im Nahtmodell nicht vor. Hier werden sie
//!   trotzdem verglichen, weil sie zeigen, ob die Nahtteile unterwegs
//!   zusammenpassen: Kerben im Inneren einer Seite und die Grenzen zwischen
//!   ihren Teilstücken, gepaart mit dem gegenseitig nächsten Partner bis
//!   [`KERBE_FANG_CM`].
//! * **Abdeckung:** keine Kante in zwei Nähten; Kanten mit Nahtzugabe ohne
//!   Naht und genähte Kanten ohne Zugabe als Hinweis.
//!
//! ## Format der Nahtliste
//!
//! ```text
//! # Kommentar
//! Kennung | Bezeichnung | Seite A | Seite B
//! N04L | Seitennaht links | Rueckenteil Seite Links: RA_L RH_L v | Oberteil seitlich Links: VA_L VC1_L v + Untercup Links: VC1T_L VC3T_L v + Band seitlich Links: VC3_L VH_L v
//! ```
//!
//! Eine Seite besteht aus Abschnitten, getrennt durch `+`, in Nährichtung.
//! Ein Abschnitt ist `Teil: VON BIS v|r`: Teil, Anfangs- und Endpunkt
//! (Namen der Rechenpunkte) und die Laufrichtung, `v` vorwärts in der
//! Knotenreihenfolge des Teils, `r` rückwärts. Der Teilname darf ohne das
//! Präfix bis zum letzten ` - ` stehen (`Untercup Links` für
//! `Bluse 3 - Untercup Links`), solange er eindeutig bleibt. Enthält er `+`
//! oder `|`, steht er in Anführungszeichen:
//! `"B Abschluss Umschlag + Keyhole 0,25mm - Strumpf 1": B1 B2 v`.
//!
//! ## Abnahme
//!
//! [`Nahtbericht::bestanden`]: mindestens eine Naht, jede Seite länger als
//! 0, |Δ| höchstens die Toleranz, keine Kante doppelt. Ob die Liste alle
//! Nähte enthält, prüft sie nicht — dafür stehen die Hinweise „Zugabe ohne
//! Naht“ und „nicht auflösbar“ daneben.

use std::collections::{BTreeMap, HashMap};

use crate::geom::Auswertung;
use crate::kanten::{Kurvenquelle, Teilrand};

/// Größter Abstand zweier Kerben, die noch als Paar gelten (cm).
pub const KERBE_FANG_CM: f64 = 1.0;

/// Abnahme Stufe 3: größte zulässige Längendifferenz eines Nahtpaars (cm).
pub const TOLERANZ_CM: f64 = 0.08;

/// Ein Abschnitt einer Nahtseite, wie er in der Liste steht.
#[derive(Debug, Clone, PartialEq)]
pub struct Abschnittsbezug {
    pub teil: String,
    pub von: String,
    pub bis: String,
    pub rueckwaerts: bool,
}

/// Eine Naht der Liste.
#[derive(Debug, Clone, PartialEq)]
pub struct Nahtdef {
    pub kennung: String,
    pub name: String,
    pub a: Vec<Abschnittsbezug>,
    pub b: Vec<Abschnittsbezug>,
    /// Zeile in der Datei, ab 1.
    pub zeile: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum NahtFehler {
    #[error("Nahtliste Zeile {zeile}: {grund}")]
    Zeile { zeile: usize, grund: String },
    #[error("Naht {kennung}: {grund}")]
    Naht { kennung: String, grund: String },
}

/// Teilt `s` an `trenner`, aber nicht innerhalb von Anführungszeichen.
fn teile_aussen(s: &str, trenner: char) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut anfang, mut in_text) = (0, false);
    for (i, c) in s.char_indices() {
        if c == '"' {
            in_text = !in_text;
        } else if c == trenner && !in_text {
            out.push(&s[anfang..i]);
            anfang = i + c.len_utf8();
        }
    }
    out.push(&s[anfang..]);
    out
}

fn abschnitt_lesen(text: &str, zeile: usize) -> Result<Abschnittsbezug, NahtFehler> {
    let fehler = |grund: String| NahtFehler::Zeile { zeile, grund };
    let form = || fehler(format!("Abschnitt {text:?}: erwartet 'Teil: VON BIS v|r'"));
    let (teil, rest) = if let Some(innen) = text.strip_prefix('"') {
        let (teil, rest) = innen.split_once('"').ok_or_else(form)?;
        (teil, rest.trim_start().strip_prefix(':').ok_or_else(form)?)
    } else {
        text.rsplit_once(':').ok_or_else(form)?
    };
    let w: Vec<&str> = rest.split_whitespace().collect();
    let [von, bis, richtung] = w.as_slice() else {
        return Err(form());
    };
    let rueckwaerts = match *richtung {
        "v" => false,
        "r" => true,
        r => return Err(fehler(format!("Richtung {r:?}: erwartet v oder r"))),
    };
    let teil = teil.trim();
    if teil.is_empty() {
        return Err(fehler(format!("Abschnitt {text:?}: Teil fehlt")));
    }
    Ok(Abschnittsbezug { teil: teil.into(), von: (*von).into(), bis: (*bis).into(), rueckwaerts })
}

/// Liest eine Nahtliste.
pub fn lies_nahtliste(text: &str) -> Result<Vec<Nahtdef>, NahtFehler> {
    let mut out: Vec<Nahtdef> = Vec::new();
    // Mit BOM gespeicherte Dateien (Windows-Editor).
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (nr, z) in text.lines().enumerate() {
        let zeile = nr + 1;
        let z = z.trim();
        if z.is_empty() || z.starts_with('#') {
            continue;
        }
        if z.matches('"').count() % 2 == 1 {
            return Err(NahtFehler::Zeile { zeile, grund: "Anführungszeichen nicht geschlossen".into() });
        }
        let felder: Vec<&str> = teile_aussen(z, '|').into_iter().map(str::trim).collect();
        let [kennung, name, a, b] = felder.as_slice() else {
            return Err(NahtFehler::Zeile {
                zeile,
                grund: format!("{} Felder, erwartet 4 (Kennung | Bezeichnung | Seite A | Seite B)", felder.len()),
            });
        };
        if kennung.is_empty() {
            return Err(NahtFehler::Zeile { zeile, grund: "Kennung fehlt".into() });
        }
        if out.iter().any(|d| d.kennung == *kennung) {
            return Err(NahtFehler::Zeile { zeile, grund: format!("Kennung {kennung} doppelt") });
        }
        let seite = |s: &str| -> Result<Vec<Abschnittsbezug>, NahtFehler> {
            teile_aussen(s, '+').into_iter().map(|t| abschnitt_lesen(t.trim(), zeile)).collect()
        };
        out.push(Nahtdef {
            kennung: (*kennung).into(),
            name: (*name).into(),
            a: seite(a)?,
            b: seite(b)?,
            zeile,
        });
    }
    Ok(out)
}

/// Art einer Marke auf einer Nahtseite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Markenart {
    /// Passzeichen im Inneren eines Abschnitts.
    Kerbe,
    /// Grenze zwischen zwei Abschnitten der Seite.
    Grenze,
}

/// Eine Marke auf einer Nahtseite.
#[derive(Debug, Clone)]
pub struct Marke {
    /// Punktname; bei einer Grenze `Ende/Anfang`.
    pub name: String,
    pub art: Markenart,
    /// Abstand vom Nahtanfang entlang der Seite (cm).
    pub abstand_cm: f64,
}

/// Ein ausgewerteter Abschnitt.
#[derive(Debug, Clone)]
pub struct Teilstueck {
    /// voller Teilname
    pub teil: String,
    pub von: String,
    pub bis: String,
    pub rueckwaerts: bool,
    /// Indizes in den Randkanten des Teils.
    pub kanten: Vec<usize>,
    pub laenge_cm: f64,
}

/// Eine ausgewertete Nahtseite.
#[derive(Debug, Clone)]
pub struct Nahtseite {
    pub teilstuecke: Vec<Teilstueck>,
    pub laenge_cm: f64,
    pub marken: Vec<Marke>,
}

/// Zwei Marken, die einander gegenüberliegen.
#[derive(Debug, Clone)]
pub struct Kerbpaar {
    pub a: Marke,
    pub b: Marke,
    /// Abstand A − Abstand B (cm).
    pub diff_cm: f64,
}

/// Ergebnis einer Naht.
#[derive(Debug, Clone)]
pub struct Nahtergebnis {
    pub kennung: String,
    pub name: String,
    pub a: Nahtseite,
    pub b: Nahtseite,
    /// L_A − L_B (cm).
    pub diff_cm: f64,
    pub kerbpaare: Vec<Kerbpaar>,
    /// Kerben ohne Gegenstück: (`'A'`/`'B'`, Marke).
    pub kerben_allein: Vec<(char, Marke)>,
}

/// Kürzeste Seite, die noch als Seite zählt (cm). CLO hält eine Naht mit
/// einer Seite unter 1e-5 ebenfalls für auffällig (Spezifikation, 3.1).
pub const MIN_SEITE_CM: f64 = 1e-5;

impl Nahtergebnis {
    /// Über der Toleranz oder eine Seite ohne Länge.
    pub fn auffaellig(&self, toleranz_cm: f64) -> bool {
        self.diff_cm.abs() > toleranz_cm
            || self.a.laenge_cm < MIN_SEITE_CM
            || self.b.laenge_cm < MIN_SEITE_CM
    }

    /// Kerbpaar mit der größten Abweichung.
    pub fn groesste_kerbabweichung(&self) -> Option<&Kerbpaar> {
        self.kerbpaare
            .iter()
            .max_by(|x, y| x.diff_cm.abs().total_cmp(&y.diff_cm.abs()))
    }
}

/// Eine Randkante, als Hinweis gemeldet.
#[derive(Debug, Clone)]
pub struct Kantenhinweis {
    pub teil: String,
    pub von: String,
    pub bis: String,
    pub laenge_cm: f64,
    /// Nähte, die die Kante benutzen.
    pub naehte: Vec<String>,
}

/// Ergebnis der ganzen Liste.
#[derive(Debug, Clone)]
pub struct Nahtbericht {
    pub quelle: Kurvenquelle,
    pub naehte: Vec<Nahtergebnis>,
    /// Kanten in mehr als einer Naht — ein Fehler der Liste.
    pub doppelt: Vec<Kantenhinweis>,
    /// Kanten mit Nahtzugabe, die keine Naht benutzt.
    pub zugabe_ohne_naht: Vec<Kantenhinweis>,
    /// Genähte Kanten mit Zugabe 0 an beiden Enden (freie Kanten).
    pub genaeht_ohne_zugabe: Vec<Kantenhinweis>,
    /// Teile, deren Rand nicht auflösbar ist (nur Teile ohne Naht).
    pub teile_fehler: Vec<String>,
}

impl Nahtbericht {
    /// Naht mit der größten |Δ|.
    pub fn groesste_abweichung(&self) -> Option<&Nahtergebnis> {
        self.naehte
            .iter()
            .max_by(|x, y| x.diff_cm.abs().total_cmp(&y.diff_cm.abs()))
    }

    /// Auffällige Nähte ([`Nahtergebnis::auffaellig`]).
    pub fn ueber(&self, toleranz_cm: f64) -> Vec<&Nahtergebnis> {
        self.naehte.iter().filter(|n| n.auffaellig(toleranz_cm)).collect()
    }

    /// Abnahme: mindestens eine Naht, keine auffällig, keine Kante doppelt.
    pub fn bestanden(&self, toleranz_cm: f64) -> bool {
        !self.naehte.is_empty() && self.ueber(toleranz_cm).is_empty() && self.doppelt.is_empty()
    }
}

/// Teil nach Namen unter allen Teilen des Musters: genau gleich, sonst
/// über den Teil nach dem letzten ` - ` des vollen Namens — jeweils nur,
/// wenn es genau einen Treffer gibt.
fn teil_finden(namen: &[String], name: &str) -> Result<usize, String> {
    let treffer = |f: &dyn Fn(&String) -> bool| -> Vec<usize> {
        (0..namen.len()).filter(|&i| f(&namen[i])).collect()
    };
    let genau = treffer(&|n| n == name);
    let wahl = if genau.is_empty() { treffer(&|n| n.rsplit(" - ").next() == Some(name)) } else { genau };
    match wahl.as_slice() {
        [i] => Ok(*i),
        [] => Err(format!("kein Teil {name:?}")),
        _ => Err(format!("Teilname {name:?} ist mehrdeutig ({} Teile)", wahl.len())),
    }
}

/// Gegenseitig nächste Partner bis [`KERBE_FANG_CM`].
fn kerben_paaren(a: &[Marke], b: &[Marke]) -> (Vec<Kerbpaar>, Vec<(char, Marke)>) {
    let naechster = |m: &Marke, andere: &[Marke]| -> Option<usize> {
        (0..andere.len())
            .min_by(|&i, &j| {
                (andere[i].abstand_cm - m.abstand_cm)
                    .abs()
                    .total_cmp(&(andere[j].abstand_cm - m.abstand_cm).abs())
            })
            .filter(|&i| (andere[i].abstand_cm - m.abstand_cm).abs() <= KERBE_FANG_CM)
    };
    let mut paare = Vec::new();
    let mut b_belegt = vec![false; b.len()];
    let mut allein = Vec::new();
    for ma in a {
        match naechster(ma, b) {
            Some(j) if naechster(&b[j], a).map(|i| std::ptr::eq(&a[i], ma)) == Some(true) => {
                b_belegt[j] = true;
                paare.push(Kerbpaar {
                    a: ma.clone(),
                    b: b[j].clone(),
                    diff_cm: ma.abstand_cm - b[j].abstand_cm,
                });
            }
            _ if ma.art == Markenart::Kerbe => allein.push(('A', ma.clone())),
            _ => {}
        }
    }
    for (j, mb) in b.iter().enumerate() {
        if !b_belegt[j] && mb.art == Markenart::Kerbe {
            allein.push(('B', mb.clone()));
        }
    }
    (paare, allein)
}

/// Prüft eine Nahtliste am Muster.
pub fn pruefe_naehte(
    ev: &Auswertung<'_>,
    defs: &[Nahtdef],
    quelle: Kurvenquelle,
) -> Result<Nahtbericht, NahtFehler> {
    // Ränder aller Teile; nicht auflösbare zählen nur, wenn eine Naht sie
    // braucht.
    let mut namen = Vec::new();
    let mut platz: Vec<Option<usize>> = Vec::new();
    let mut raender = Vec::new();
    let mut fehler: HashMap<String, String> = HashMap::new();
    for t in ev.muster().pieces() {
        let name = t.name.clone().unwrap_or_else(|| t.id.clone());
        match ev.teilrand(t, quelle) {
            Ok(r) => {
                platz.push(Some(raender.len()));
                raender.push(r);
            }
            Err(e) => {
                platz.push(None);
                fehler.insert(name.clone(), e.to_string());
            }
        }
        namen.push(name);
    }

    // (Teilindex, Kante) → Nähte
    let mut belegung: BTreeMap<(usize, usize), Vec<String>> = BTreeMap::new();
    let mut naehte = Vec::new();
    for d in defs {
        let mut seiten = Vec::new();
        for bezuege in [&d.a, &d.b] {
            let mut teilstuecke = Vec::new();
            let mut marken = Vec::new();
            let mut summe = 0.0;
            for (nr, z) in bezuege.iter().enumerate() {
                let naht_fehler = |grund: String| NahtFehler::Naht { kennung: d.kennung.clone(), grund };
                let gefunden = teil_finden(&namen, &z.teil).map_err(naht_fehler)?;
                let Some(ti) = platz[gefunden] else {
                    let name = &namen[gefunden];
                    return Err(naht_fehler(format!("Teil {name} nicht auflösbar: {}", fehler[name])));
                };
                let r = &raender[ti];
                let von = r.knoten_nach_name(&z.von).map_err(naht_fehler)?;
                let bis = r.knoten_nach_name(&z.bis).map_err(naht_fehler)?;
                let ab = r.abschnitt(von, bis, z.rueckwaerts);
                if nr > 0 {
                    let vorher: &Teilstueck = teilstuecke.last().unwrap();
                    marken.push(Marke {
                        name: format!("{}/{}", vorher.bis, z.von),
                        art: Markenart::Grenze,
                        abstand_cm: summe,
                    });
                }
                for &(k, abstand) in &ab.innen {
                    let p = &r.punktknoten[k];
                    if p.kerbe {
                        marken.push(Marke {
                            name: p.name.clone(),
                            art: Markenart::Kerbe,
                            abstand_cm: summe + r.in_cm(abstand),
                        });
                    }
                }
                for &k in &ab.kanten {
                    belegung.entry((ti, k)).or_default().push(d.kennung.clone());
                }
                let laenge_cm = r.in_cm(ab.laenge);
                summe += laenge_cm;
                teilstuecke.push(Teilstueck {
                    teil: r.teil.clone(),
                    von: z.von.clone(),
                    bis: z.bis.clone(),
                    rueckwaerts: z.rueckwaerts,
                    kanten: ab.kanten,
                    laenge_cm,
                });
            }
            seiten.push(Nahtseite { teilstuecke, laenge_cm: summe, marken });
        }
        let b = seiten.pop().unwrap();
        let a = seiten.pop().unwrap();
        let (kerbpaare, kerben_allein) = kerben_paaren(&a.marken, &b.marken);
        naehte.push(Nahtergebnis {
            kennung: d.kennung.clone(),
            name: d.name.clone(),
            diff_cm: a.laenge_cm - b.laenge_cm,
            a,
            b,
            kerbpaare,
            kerben_allein,
        });
    }

    let hinweis = |ti: usize, k: usize, naehte: Vec<String>| {
        let r: &Teilrand = &raender[ti];
        let kante = &r.kanten[k];
        Kantenhinweis {
            teil: r.teil.clone(),
            von: r.punktknoten[kante.von].name.clone(),
            bis: r.punktknoten[kante.bis].name.clone(),
            laenge_cm: r.in_cm(kante.laenge),
            naehte,
        }
    };
    let mut doppelt = Vec::new();
    let mut genaeht_ohne_zugabe = Vec::new();
    for ((ti, k), n) in &belegung {
        if n.len() > 1 {
            doppelt.push(hinweis(*ti, *k, n.clone()));
        }
        if raender[*ti].ist_frei(*k) {
            genaeht_ohne_zugabe.push(hinweis(*ti, *k, n.clone()));
        }
    }
    let mut zugabe_ohne_naht = Vec::new();
    for (ti, r) in raender.iter().enumerate() {
        for k in 0..r.kanten.len() {
            let (vor, nach) = r.zugabe(k);
            let zugabe = vor.unwrap_or(0.0).abs() > 1e-9 || nach.unwrap_or(0.0).abs() > 1e-9;
            if zugabe && !belegung.contains_key(&(ti, k)) {
                zugabe_ohne_naht.push(hinweis(ti, k, Vec::new()));
            }
        }
    }
    let mut teile_fehler: Vec<String> = fehler.iter().map(|(n, e)| format!("{n}: {e}")).collect();
    teile_fehler.sort();

    Ok(Nahtbericht { quelle, naehte, doppelt, zugabe_ohne_naht, genaeht_ohne_zugabe, teile_fehler })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liste_lesen() {
        let t = "# Kopf\n\nN1 | Seite | Teil A: X Y v | B - Teil B: P Q r + Teil C: R S v\n";
        let d = lies_nahtliste(t).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].zeile, 3);
        assert_eq!(d[0].a, [Abschnittsbezug { teil: "Teil A".into(), von: "X".into(), bis: "Y".into(), rueckwaerts: false }]);
        assert_eq!(d[0].b.len(), 2);
        assert_eq!(d[0].b[0].teil, "B - Teil B");
        assert!(d[0].b[0].rueckwaerts && !d[0].b[1].rueckwaerts);
    }

    #[test]
    fn liste_fehler() {
        for (t, teil) in [
            ("N1 | x | A: X Y v", "Felder"),
            ("N1 | x | A: X Y | B: P Q v", "erwartet"),
            ("N1 | x | A: X Y w | B: P Q v", "Richtung"),
            ("N1 | x | A X Y v | B: P Q v", "erwartet"),
            ("N1 | x | A: X Y v | B: P Q v\nN1 | y | A: X Y v | B: P Q v", "doppelt"),
            (" | x | A: X Y v | B: P Q v", "Kennung"),
        ] {
            let e = lies_nahtliste(t).unwrap_err().to_string();
            assert!(e.contains(teil), "{t:?}: {e}");
        }
    }

    #[test]
    fn bom_und_anfuehrungszeichen() {
        let t = "\u{feff}# Kopf\nN1 | x | \"B Umschlag + Keyhole - Strumpf 1\": B1 B2 v | \"a|b\": P Q r + C: R S v\n";
        let d = lies_nahtliste(t).unwrap();
        assert_eq!(d[0].kennung, "N1");
        assert_eq!(d[0].a[0].teil, "B Umschlag + Keyhole - Strumpf 1");
        assert_eq!(d[0].b[0].teil, "a|b");
        assert_eq!(d[0].b[1].teil, "C");
        let e = lies_nahtliste("N1 | x | \"offen: A B v | C: R S v").unwrap_err().to_string();
        assert!(e.contains("Anführungszeichen"), "{e}");
    }

    #[test]
    fn teilnamen_eindeutig() {
        let n: Vec<String> = ["Rock - Band", "Bluse - Band", "Gleich", "Gleich", "Bluse - Kragen"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(teil_finden(&n, "Band").unwrap_err().contains("mehrdeutig"));
        assert!(teil_finden(&n, "Gleich").unwrap_err().contains("mehrdeutig"));
        assert_eq!(teil_finden(&n, "Kragen"), Ok(4));
        assert_eq!(teil_finden(&n, "Rock - Band"), Ok(0));
        assert!(teil_finden(&n, "Hose").is_err());
    }

    #[test]
    fn kerben_gegenseitig_naechste() {
        let m = |n: &str, art, a| Marke { name: n.into(), art, abstand_cm: a };
        let a = [m("RSN1", Markenart::Kerbe, 10.51), m("RW", Markenart::Kerbe, 21.01)];
        let b = [
            m("VC1/VC1T", Markenart::Grenze, 9.765),
            m("VSN1T", Markenart::Kerbe, 10.5055),
            m("VC3T/VC3", Markenart::Grenze, 14.44),
        ];
        let (p, allein) = kerben_paaren(&a, &b);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].a.name.as_str(), p[0].b.name.as_str()), ("RSN1", "VSN1T"));
        assert_eq!(allein.len(), 1);
        assert_eq!((allein[0].0, allein[0].1.name.as_str()), ('A', "RW"));
    }
}
