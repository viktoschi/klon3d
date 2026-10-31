//! Zwei 3D-Netze vergleichen: Abstand zum nächsten Punkt der anderen
//! Oberfläche, in **beide** Richtungen (Stufe 5, Vorgabe in `PLAN.md`).
//!
//! Warum nicht über gleiche Punktnummern: klon3d und CLO haben verschiedene
//! Netze. Ein Vergleich `x[i]` gegen `y[i]` misst dann den Unterschied der
//! Vernetzung, nicht den der Simulation. Gemessen wird deshalb der Abstand
//! jedes Knotens zur **Fläche** des anderen Netzes; die Gegenrichtung kommt
//! dazu, weil ein Netz feiner sein kann als das andere.
//!
//! Kennzahlen je Richtung: Mittel, **Median** (der Mittelwert allein wird von
//! Ausreißern verzogen) und der größte Einzelwert. Dazu der Median je Teil,
//! wenn beide Netze Gruppennamen tragen.
//!
//! Die Oberfläche eines Netzes ist die Gesamtheit seiner Dreiecke. Gesucht
//! wird über ein gleichmäßiges Gitter über dem Hüllquader; weil Knoten auch
//! weit von der Gegenfläche liegen dürfen, wird der Suchradius so lange
//! verdoppelt, bis ein Dreieck gefunden ist.

use std::collections::BTreeMap;

/// Ein Dreiecksnetz in Millimetern mit optionalen Gruppennamen je Dreieck.
#[derive(Debug, Clone, Default)]
pub struct Netz {
    pub punkte: Vec<[f64; 3]>,
    pub dreiecke: Vec<[u32; 3]>,
    /// Gruppenname je Dreieck (aus `o`/`g` im OBJ); leer, wenn es keine gibt.
    pub gruppe: Vec<u32>,
    /// Gruppennamen zu [`Netz::gruppe`].
    pub namen: Vec<String>,
}

impl Netz {
    /// Liest ein OBJ. Nur `v`, `f`, `o` und `g` werden gedeutet; `vt`, `vn`,
    /// `s`, `usemtl` und Kommentare fallen weg. Polygone werden als Fächer
    /// trianguliert, negative Indizes sind erlaubt.
    pub fn lies_obj(text: &str) -> Result<Netz, String> {
        let mut netz = Netz::default();
        let mut aktuell: Option<u32> = None;
        for (nr, zeile) in text.lines().enumerate() {
            let mut teile = zeile.split_whitespace();
            let Some(kopf) = teile.next() else { continue };
            match kopf {
                "v" => {
                    let mut p = [0.0f64; 3];
                    for a in 0..3 {
                        let t = teile.next().ok_or_else(|| {
                            format!("Zeile {}: Punkt zu kurz", nr + 1)
                        })?;
                        p[a] = t
                            .parse::<f64>()
                            .map_err(|e| format!("Zeile {}: {e}", nr + 1))?;
                    }
                    netz.punkte.push(p);
                }
                "o" | "g" => {
                    let name = teile.collect::<Vec<_>>().join(" ");
                    let name = if name.is_empty() {
                        "(ohne Namen)".to_string()
                    } else {
                        name
                    };
                    aktuell = Some(match netz.namen.iter().position(|n| n == &name) {
                        Some(i) => i as u32,
                        None => {
                            netz.namen.push(name);
                            (netz.namen.len() - 1) as u32
                        }
                    });
                }
                "f" => {
                    let mut idx = Vec::new();
                    for t in teile {
                        let roh: i64 = t
                            .split('/')
                            .next()
                            .unwrap_or("")
                            .parse()
                            .map_err(|e| format!("Zeile {}: {e}", nr + 1))?;
                        let i = if roh < 0 {
                            netz.punkte.len() as i64 + roh
                        } else {
                            roh - 1
                        };
                        if i < 0 || i as usize >= netz.punkte.len() {
                            return Err(format!(
                                "Zeile {}: Punktindex {roh} ausserhalb von {}",
                                nr + 1,
                                netz.punkte.len()
                            ));
                        }
                        idx.push(i as u32);
                    }
                    if idx.len() < 3 {
                        return Err(format!("Zeile {}: Fläche mit {} Ecken", nr + 1, idx.len()));
                    }
                    // Fächer: (0, i, i+1)
                    for i in 1..idx.len() - 1 {
                        netz.dreiecke.push([idx[0], idx[i], idx[i + 1]]);
                        netz.gruppe.push(aktuell.unwrap_or(u32::MAX));
                    }
                }
                _ => {}
            }
        }
        Ok(netz)
    }

    /// Hüllquader über alle Knoten, `None` bei leerem Netz.
    pub fn huelle(&self) -> Option<([f64; 3], [f64; 3])> {
        let first = *self.punkte.first()?;
        let (mut lo, mut hi) = (first, first);
        for p in &self.punkte {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        Some((lo, hi))
    }
}

/// Kennzahlen einer Richtung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kennzahlen {
    pub anzahl: usize,
    pub mittel: f64,
    pub median: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: f64,
}

impl Kennzahlen {
    /// Rechnet die Kennzahlen aus Einzelwerten (mm).
    pub fn aus(mut d: Vec<f64>) -> Kennzahlen {
        if d.is_empty() {
            return Kennzahlen {
                anzahl: 0,
                mittel: 0.0,
                median: 0.0,
                p90: 0.0,
                p99: 0.0,
                max: 0.0,
            };
        }
        d.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = d.len();
        let q = |f: f64| d[((n as f64 * f) as usize).min(n - 1)];
        Kennzahlen {
            anzahl: n,
            mittel: d.iter().sum::<f64>() / n as f64,
            median: q(0.5),
            p90: q(0.9),
            p99: q(0.99),
            max: *d.last().unwrap(),
        }
    }
}

/// Vergleich beider Netze: `a_zu_b[i]` ist der Abstand von Knoten `i` aus `a`
/// zur Oberfläche von `b`.
#[derive(Debug, Clone)]
pub struct Vergleich {
    /// Abstände aller Knoten von `a` zur Fläche von `b`.
    pub a_zu_b: Vec<f64>,
    /// Abstände aller Knoten von `b` zur Fläche von `a`.
    pub b_zu_a: Vec<f64>,
}

impl Vergleich {
    /// Rechnet beide Richtungen.
    ///
    /// `zelle` ist die Kantenlänge der Suchgitter in Millimetern; klein heißt
    /// schneller, aber mehr Speicher. 20 mm ist für Kleidung ein guter Wert.
    pub fn rechne(a: &Netz, b: &Netz, zelle: f64) -> Result<Vergleich, String> {
        if a.dreiecke.is_empty() || b.dreiecke.is_empty() {
            return Err("eines der Netze hat keine Dreiecke".to_string());
        }
        let gb = Gitter::neu(b, zelle)?;
        let ga = Gitter::neu(a, zelle)?;
        Ok(Vergleich {
            a_zu_b: a.punkte.iter().map(|&p| gb.abstand(p)).collect(),
            b_zu_a: b.punkte.iter().map(|&p| ga.abstand(p)).collect(),
        })
    }

    pub fn hin(&self) -> Kennzahlen {
        Kennzahlen::aus(self.a_zu_b.clone())
    }

    pub fn zurueck(&self) -> Kennzahlen {
        Kennzahlen::aus(self.b_zu_a.clone())
    }

    /// Median je Gruppe von `a` gegen die Fläche von `b`; Zuordnung über den
    /// Gruppennamen. Nur Gruppen, die in beiden Netzen vorkommen.
    pub fn je_gruppe(&self, a: &Netz, b: &Netz) -> Vec<(String, usize, Kennzahlen)> {
        // Gruppenname je Knoten von a (über das erste Dreieck, das ihn nennt)
        let mut name_je_knoten: Vec<Option<u32>> = vec![None; a.punkte.len()];
        for (t, d) in a.dreiecke.iter().enumerate() {
            let g = a.gruppe.get(t).copied().unwrap_or(u32::MAX);
            for &i in d {
                name_je_knoten[i as usize].get_or_insert(g);
            }
        }
        let mut korb: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
        for (i, g) in name_je_knoten.iter().enumerate() {
            let Some(g) = g else { continue };
            let Some(name) = a.namen.get(*g as usize) else { continue };
            korb.entry(name.as_str()).or_default().push(self.a_zu_b[i]);
        }
        // Nur Gruppen, die es auch in b gibt.
        let in_b: Vec<&str> = b.namen.iter().map(|s| s.as_str()).collect();
        korb.into_iter()
            .filter(|(n, _)| in_b.contains(n))
            .map(|(n, d)| {
                let k = Kennzahlen::aus(d.clone());
                (n.to_string(), d.len(), k)
            })
            .collect()
    }
}

// ── Das Suchgitter ──

/// Gleichmäßiges Gitter über dem Hüllquader eines Netzes; je Zelle die
/// Dreiecke, deren Hüllquader sie berührt.
struct Gitter {
    ursprung: [f64; 3],
    zelle: f64,
    zahl: [usize; 3],
    /// Größter Abstand, der im Hüllquader überhaupt vorkommen kann: danach
    /// ist die Suche fertig.
    weit: f64,
    /// Zelle `z` belegt `eintraege[anfang[z]..anfang[z + 1]]`.
    anfang: Vec<u32>,
    eintraege: Vec<u32>,
    netz: Netz,
}

impl Gitter {
    fn neu(netz: &Netz, zelle: f64) -> Result<Gitter, String> {
        let (lo, hi) = netz
            .huelle()
            .ok_or_else(|| "Netz ohne Punkte".to_string())?;
        let mut zahl = [1usize; 3];
        for a in 0..3 {
            let k = ((hi[a] - lo[a]) / zelle).ceil() as usize + 1;
            zahl[a] = k.max(1);
        }
        // Obergrenze gegen Speicher-Auswurf bei unsinniger Zellgröße
        let zellen = zahl[0].saturating_mul(zahl[1]).saturating_mul(zahl[2]);
        if zellen > 400_000_000 {
            return Err(format!(
                "Suchgitter zu fein: {} Zellen bei Zelle {zelle} mm",
                zellen
            ));
        }
        let diagonal = ((hi[0] - lo[0]).powi(2)
            + (hi[1] - lo[1]).powi(2)
            + (hi[2] - lo[2]).powi(2))
        .sqrt();
        let mut g = Gitter {
            ursprung: lo,
            zelle,
            zahl,
            weit: diagonal + zelle,
            anfang: Vec::new(),
            eintraege: Vec::new(),
            netz: netz.clone(),
        };
        // Zwei Durchläufe: erst zählen, dann füllen (wie die GPU-Puffer).
        // Zellen eines Dreiecks erst sammeln, damit `g` nicht gleichzeitig
        // geliehen und verändert wird.
        let mut zaehler = vec![0u32; zellen + 1];
        for t in &g.netz.dreiecke {
            let (a, b) = g.quader(t);
            for z in g.zellen(a, b) {
                zaehler[z + 1] += 1;
            }
        }
        for z in 0..zellen {
            zaehler[z + 1] += zaehler[z];
        }
        g.anfang = zaehler.clone();
        g.eintraege = vec![0u32; zaehler[zellen] as usize];
        let mut fuell = zaehler;
        for (i, t) in g.netz.dreiecke.iter().enumerate() {
            let (a, b) = g.quader(t);
            for z in g.zellen(a, b) {
                g.eintraege[fuell[z] as usize] = i as u32;
                fuell[z] += 1;
            }
        }
        Ok(g)
    }

    /// Hüllquader eines Dreiecks.
    fn quader(&self, t: &[u32; 3]) -> ([f64; 3], [f64; 3]) {
        let p = |i: u32| self.netz.punkte[i as usize];
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let mut lo = [0.0f64; 3];
        let mut hi = [0.0f64; 3];
        for k in 0..3 {
            lo[k] = a[k].min(b[k]).min(c[k]);
            hi[k] = a[k].max(b[k]).max(c[k]);
        }
        (lo, hi)
    }

    fn zellindex(&self, x: [f64; 3]) -> [i64; 3] {
        let mut k = [0i64; 3];
        for a in 0..3 {
            k[a] = ((x[a] - self.ursprung[a]) / self.zelle).floor() as i64;
            k[a] = k[a].clamp(0, self.zahl[a] as i64 - 1);
        }
        k
    }

    fn zellnummer(&self, k: [i64; 3]) -> usize {
        ((k[2] as usize) * self.zahl[1] + k[1] as usize) * self.zahl[0] + k[0] as usize
    }

    /// Zellnummern im Quader [lo, hi].
    fn zellen(&self, lo: [f64; 3], hi: [f64; 3]) -> Vec<usize> {
        let a = self.zellindex(lo);
        let b = self.zellindex(hi);
        let mut out = Vec::new();
        for z in a[2]..=b[2] {
            for y in a[1]..=b[1] {
                for x in a[0]..=b[0] {
                    out.push(self.zellnummer([x, y, z]));
                }
            }
        }
        out
    }

    /// Abstand von `p` zur Oberfläche des Netzes. Der Suchradius wächst, bis
    /// ein Dreieck gefunden ist - ein Knoten darf beliebig weit entfernt sein.
    fn abstand(&self, p: [f64; 3]) -> f64 {
        let mut r = self.zelle;
        let weit = self.weit;
        loop {
            let lo = [p[0] - r, p[1] - r, p[2] - r];
            let hi = [p[0] + r, p[1] + r, p[2] + r];
            let mut best = f64::INFINITY;
            for z in self.zellen(lo, hi) {
                for &e in &self.eintraege[self.anfang[z] as usize..self.anfang[z + 1] as usize] {
                    let t = &self.netz.dreiecke[e as usize];
                    let pnt = |i: u32| self.netz.punkte[i as usize];
                    let q = naechster_auf_dreieck(p, pnt(t[0]), pnt(t[1]), pnt(t[2]));
                    let dx = p[0] - q[0];
                    let dy = p[1] - q[1];
                    let dz = p[2] - q[2];
                    let d = (dx * dx + dy * dy + dz * dz).sqrt();
                    if d < best {
                        best = d;
                    }
                }
            }
            // Gefunden und der Suchwürfel ist sicher größer als der Treffer?
            // Kantenlänge 2r, also reicht best <= r für "kein näheres Dreieck
            // außerhalb".
            if best.is_finite() && best <= r {
                return best;
            }
            if r > weit {
                // Nichts in Reichweite: das Netz ist leer an dieser Stelle.
                return if best.is_finite() { best } else { f64::NAN };
            }
            r *= 2.0;
        }
    }
}

/// Nächster Punkt auf einem Dreieck (Ericson, *Real-Time Collision
/// Detection*, 5.1.5) — dieselbe Rechnung wie im Körpernetz, hier ohne
/// Vorzeichen und ohne Region.
fn naechster_auf_dreieck(p: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    let sub = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let add = |x: [f64; 3], y: [f64; 3]| [x[0] + y[0], x[1] + y[1], x[2] + y[2]];
    let mul = |x: [f64; 3], s: f64| [x[0] * s, x[1] * s, x[2] * s];
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let quot = |z: f64, n: f64| if n > 0.0 { z / n } else { 0.0 };

    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return add(a, mul(ab, quot(d1, d1 - d3)));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return add(a, mul(ac, quot(d2, d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = quot(d4 - d3, (d4 - d3) + (d5 - d6));
        return add(b, mul(sub(c, b), w));
    }
    let s = va + vb + vc;
    let (v, w) = (quot(vb, s), quot(vc, s));
    add(a, add(mul(ab, v), mul(ac, w)))
}
