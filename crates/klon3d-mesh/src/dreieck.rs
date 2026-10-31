//! Triangulierung eines Teils mit festem Rand.
//!
//! 1. **Ohrenschneiden:** Das Randpolygon (Randpunkte in Knotenreihenfolge)
//!    wird in Dreiecke zerlegt. Ein Ohr ist eine streng konvexe Ecke, in
//!    deren Dreieck kein anderer Randpunkt liegt, auch nicht auf dem Rand.
//! 2. **Lawson-Kippen:** Jede innere Kante, deren Gegenpunkt im Umkreis
//!    liegt, wird gekippt. Das Ergebnis ist die eingeschränkte
//!    Delaunay-Triangulierung des Polygons; Randkanten werden nie gekippt.
//! 3. **Verfeinerung nach Ruppert:** Für jedes schlechte Dreieck (kleinster
//!    Winkel unter dem Ziel oder Umkreisradius über `GROESSE` × Kantenlänge)
//!    wird der Umkreismittelpunkt eingesetzt (Bowyer–Watson, der Hohlraum
//!    endet an Randkanten). Liegt er jenseits einer Randkante oder in ihrem
//!    Thaleskreis, wird er verworfen und die Randkante zum Teilen gemeldet.
//!    Geteilt wird hier nicht: Ein Randstück gehört womöglich zu einer Naht,
//!    und dann muss die Gegenseite an derselben Stelle geteilt werden
//!    (`crate::rand`). Der Aufrufer teilt und vernetzt neu.
//! 4. **Glätten:** Innere Punkte rücken in den Schwerpunkt ihrer Nachbarn,
//!    wenn das den kleinsten Winkel der anliegenden Dreiecke nicht
//!    verschlechtert; danach wieder Lawson.
//!
//! Alle Lageentscheidungen rechnen exakt auf dem Gitter ([`crate::exakt`]).

use std::collections::{BTreeSet, BinaryHeap, HashMap};

use crate::exakt::{orient, orient3, umkreis, Gitter, Ip};

pub(crate) const KEIN: u32 = u32::MAX;

/// Umkreisradius, ab dem ein Dreieck zu groß ist, als Anteil der
/// Zielkantenlänge. Ein gleichseitiges Dreieck mit Seite h hat h/√3 ≈ 0,577 h.
pub(crate) const GROESSE: f64 = 0.8;

#[derive(Debug, Clone, Copy)]
struct Tri {
    /// Ecken, positiv umlaufend.
    v: [u32; 3],
    /// Nachbar gegenüber `v[i]`, [`KEIN`] am Rand.
    n: [u32; 3],
    /// Die Kante gegenüber `v[i]` ist Rand.
    fest: [bool; 3],
}

/// Ziele der Verfeinerung.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ziele {
    pub kante_mm: f64,
    pub winkel_grad: f64,
    /// Kürzestes Randstück, das noch geteilt werden darf, ist doppelt so lang.
    pub min_rand_mm: f64,
    pub max_punkte: usize,
}

#[derive(Debug, Default)]
pub(crate) struct Ergebnis {
    /// Randstücke zum Teilen, als Index des Anfangs im Randumlauf
    /// (Stück `i` → `i + 1`).
    pub teilen: BTreeSet<usize>,
    /// Schlechte Dreiecke, die nicht verbessert werden konnten.
    pub aufgegeben: usize,
}

#[derive(Debug)]
pub(crate) struct Triangulierung {
    pub gitter: Gitter,
    pub pi: Vec<Ip>,
    /// Lage in mm. Randpunkte unverändert, innere Punkte auf dem Gitter.
    pub pf: Vec<[f64; 2]>,
    /// Die ersten `nrand` Punkte sind der Rand in Umlaufreihenfolge.
    pub nrand: usize,
    t: Vec<Tri>,
    lebt: Vec<bool>,
    frei: Vec<u32>,
    marke: Vec<u32>,
    stempel: u32,
}

enum Ort {
    In(u32),
    /// Der Weg verlässt das Teil über die Randkante (a, b).
    Blockiert(u32, u32),
    /// Nicht einsetzbar: auf einem vorhandenen Punkt oder Weg entartet.
    Nichts,
}

struct Hohlstueck {
    a: u32,
    b: u32,
    aussen: u32,
    fest: bool,
}

fn im_dreieck(a: Ip, b: Ip, c: Ip, x: Ip) -> bool {
    orient(a, b, x) >= 0 && orient(b, c, x) >= 0 && orient(c, a, x) >= 0
}

/// Schneiden oder berühren sich die Strecken p1–p2 und q1–q2?
fn strecken_treffen(p1: Ip, p2: Ip, q1: Ip, q2: Ip) -> bool {
    let (d1, d2) = (orient(q1, q2, p1), orient(q1, q2, p2));
    let (d3, d4) = (orient(p1, p2, q1), orient(p1, p2, q2));
    if d1 * d2 < 0 && d3 * d4 < 0 {
        return true;
    }
    let auf = |a: Ip, b: Ip, c: Ip| {
        c[0] >= a[0].min(b[0]) && c[0] <= a[0].max(b[0]) && c[1] >= a[1].min(b[1]) && c[1] <= a[1].max(b[1])
    };
    (d1 == 0 && auf(q1, q2, p1))
        || (d2 == 0 && auf(q1, q2, p2))
        || (d3 == 0 && auf(p1, p2, q1))
        || (d4 == 0 && auf(p1, p2, q2))
}

fn abstand(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Kleinster Winkel (als Sinus), Umkreisradius, Kantenlängen gegenüber den
/// Ecken eines Dreiecks.
pub(crate) fn form(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> (f64, f64, [f64; 3]) {
    let l = [abstand(b, c), abstand(c, a), abstand(a, b)];
    let flaeche2 = ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs();
    if flaeche2 <= 0.0 {
        return (0.0, f64::INFINITY, l);
    }
    let r = l[0] * l[1] * l[2] / (2.0 * flaeche2);
    let lmin = l[0].min(l[1]).min(l[2]);
    (lmin / (2.0 * r), r, l)
}

#[derive(PartialEq)]
struct Eintrag {
    wert: f64,
    t: u32,
    v: [u32; 3],
}
impl Eq for Eintrag {}
impl PartialOrd for Eintrag {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Eintrag {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.wert.total_cmp(&o.wert).then(self.t.cmp(&o.t))
    }
}

impl Triangulierung {
    /// Triangulierung des Polygons `rand` (mm, Umlaufreihenfolge, ohne
    /// Wiederholung des Anfangs). Beide Umlaufsinne sind erlaubt; die
    /// Dreiecke laufen immer positiv um.
    pub fn neu(rand: &[[f64; 2]]) -> Result<Self, String> {
        let n = rand.len();
        if n < 3 {
            return Err(format!("Rand mit {n} Punkten"));
        }
        let gitter = Gitter::fuer(rand);
        let pi: Vec<Ip> = rand.iter().map(|&p| gitter.ein(p)).collect();
        let mut gesehen: HashMap<Ip, usize> = HashMap::with_capacity(n);
        for (i, p) in pi.iter().enumerate() {
            if let Some(j) = gesehen.insert(*p, i) {
                return Err(format!("Randpunkte {j} und {i} fallen zusammen"));
            }
        }
        // Einfaches Polygon: keine zwei nicht benachbarten Randstücke treffen sich.
        for i in 0..n {
            let (p1, p2) = (pi[i], pi[(i + 1) % n]);
            for j in i + 2..n {
                if (j + 1) % n == i {
                    continue;
                }
                if strecken_treffen(p1, p2, pi[j], pi[(j + 1) % n]) {
                    return Err(format!("Rand schneidet sich (Randstücke {i} und {j})"));
                }
            }
        }
        let mut a2: i128 = 0;
        for i in 0..n {
            let (p, q) = (pi[i], pi[(i + 1) % n]);
            a2 += p[0] as i128 * q[1] as i128 - q[0] as i128 * p[1] as i128;
        }
        if a2 == 0 {
            return Err("Rand ohne Fläche".into());
        }
        let ring: Vec<u32> = if a2 > 0 { (0..n as u32).collect() } else { (0..n as u32).rev().collect() };
        let mut tr = Triangulierung {
            gitter,
            pi,
            pf: rand.to_vec(),
            nrand: n,
            t: Vec::new(),
            lebt: Vec::new(),
            frei: Vec::new(),
            marke: Vec::new(),
            stempel: 0,
        };
        let dreiecke = tr.ohrenschneiden(&ring)?;
        tr.verknuepfe(&dreiecke)?;
        let alle = tr.innere_kanten();
        tr.lawson(alle);
        Ok(tr)
    }

    fn ohrenschneiden(&self, ring: &[u32]) -> Result<Vec<[u32; 3]>, String> {
        let n = ring.len();
        let p = |k: usize| self.pi[ring[k] as usize];
        let mut vor: Vec<usize> = (0..n).map(|k| (k + n - 1) % n).collect();
        let mut nach: Vec<usize> = (0..n).map(|k| (k + 1) % n).collect();
        let mut drin = vec![true; n];
        let mut aktiv = n;
        let mut out = Vec::with_capacity(n - 2);
        let ist_ohr = |k: usize, vor: &[usize], nach: &[usize], drin: &[bool]| -> bool {
            let (a, b, c) = (p(vor[k]), p(k), p(nach[k]));
            if orient(a, b, c) <= 0 {
                return false;
            }
            let lo = [a[0].min(b[0]).min(c[0]), a[1].min(b[1]).min(c[1])];
            let hi = [a[0].max(b[0]).max(c[0]), a[1].max(b[1]).max(c[1])];
            (0..n).all(|j| {
                if !drin[j] || j == k || j == vor[k] || j == nach[k] {
                    return true;
                }
                let x = p(j);
                x[0] < lo[0] || x[0] > hi[0] || x[1] < lo[1] || x[1] > hi[1] || !im_dreieck(a, b, c, x)
            })
        };
        let (mut k, mut fehlversuche) = (0, 0);
        while aktiv > 3 {
            if ist_ohr(k, &vor, &nach, &drin) {
                out.push([ring[vor[k]], ring[k], ring[nach[k]]]);
                let (v, w) = (vor[k], nach[k]);
                nach[v] = w;
                vor[w] = v;
                drin[k] = false;
                aktiv -= 1;
                fehlversuche = 0;
                k = w;
            } else {
                k = nach[k];
                fehlversuche += 1;
                if fehlversuche > aktiv {
                    return Err("kein Ohr gefunden".into());
                }
            }
        }
        let (v, w) = (vor[k], nach[k]);
        if orient(p(v), p(k), p(w)) <= 0 {
            return Err("letztes Dreieck entartet".into());
        }
        out.push([ring[v], ring[k], ring[w]]);
        Ok(out)
    }

    fn verknuepfe(&mut self, dreiecke: &[[u32; 3]]) -> Result<(), String> {
        self.t = dreiecke.iter().map(|&v| Tri { v, n: [KEIN; 3], fest: [true; 3] }).collect();
        self.lebt = vec![true; self.t.len()];
        self.marke = vec![0; self.t.len()];
        let mut kanten: HashMap<(u32, u32), u32> = HashMap::with_capacity(3 * self.t.len());
        for (ti, tri) in self.t.iter().enumerate() {
            for i in 0..3 {
                let (a, b) = (tri.v[(i + 1) % 3], tri.v[(i + 2) % 3]);
                if kanten.insert((a, b), ti as u32).is_some() {
                    return Err(format!("Kante {a}–{b} doppelt"));
                }
            }
        }
        let mut rand = 0;
        for ti in 0..self.t.len() {
            for i in 0..3 {
                let (a, b) = (self.t[ti].v[(i + 1) % 3], self.t[ti].v[(i + 2) % 3]);
                match kanten.get(&(b, a)) {
                    Some(&u) => {
                        self.t[ti].n[i] = u;
                        self.t[ti].fest[i] = false;
                    }
                    None => rand += 1,
                }
            }
        }
        if rand != self.nrand {
            return Err(format!("{rand} Randkanten statt {}", self.nrand));
        }
        Ok(())
    }

    fn innere_kanten(&self) -> Vec<(u32, usize)> {
        let mut out = Vec::new();
        for (ti, tri) in self.t.iter().enumerate() {
            if !self.lebt[ti] {
                continue;
            }
            for i in 0..3 {
                if !tri.fest[i] && (ti as u32) < tri.n[i] {
                    out.push((ti as u32, i));
                }
            }
        }
        out
    }

    /// Stelle der Kante {a, b} in Dreieck `x` (Index der Gegenecke).
    fn kante_index(&self, x: u32, a: u32, b: u32) -> usize {
        let v = self.t[x as usize].v;
        (0..3).find(|&k| v[k] != a && v[k] != b).expect("Kante nicht im Dreieck")
    }

    fn ersetze_nachbar(&mut self, x: u32, alt: u32, neu: u32) {
        let n = &mut self.t[x as usize].n;
        if let Some(k) = (0..3).find(|&k| n[k] == alt) {
            n[k] = neu;
        }
    }

    fn lawson(&mut self, mut stapel: Vec<(u32, usize)>) {
        while let Some((t, i)) = stapel.pop() {
            if !self.lebt[t as usize] {
                continue;
            }
            let tri = self.t[t as usize];
            let u = tri.n[i];
            if u == KEIN || tri.fest[i] {
                continue;
            }
            let j = self.kante_index(u, tri.v[(i + 1) % 3], tri.v[(i + 2) % 3]);
            let d = self.t[u as usize].v[j];
            let p = |v: u32| self.pi[v as usize];
            if umkreis(p(tri.v[0]), p(tri.v[1]), p(tri.v[2]), p(d)) > 0 {
                self.kippen(t, i);
                stapel.extend([(t, 0), (t, 2), (u, 0), (u, 1)]);
            }
        }
    }

    /// Kippt die Kante gegenüber `v[i]` von `t`: aus (a b c) und (d c b)
    /// werden (a b d) und (a d c).
    fn kippen(&mut self, t: u32, i: usize) {
        let tt = self.t[t as usize];
        let (a, b, c) = (tt.v[i], tt.v[(i + 1) % 3], tt.v[(i + 2) % 3]);
        let u = tt.n[i];
        let uu = self.t[u as usize];
        let j = self.kante_index(u, b, c);
        let d = uu.v[j];
        debug_assert!(uu.v[(j + 1) % 3] == c && uu.v[(j + 2) % 3] == b);
        let (n_ab, f_ab) = (tt.n[(i + 2) % 3], tt.fest[(i + 2) % 3]);
        let (n_ca, f_ca) = (tt.n[(i + 1) % 3], tt.fest[(i + 1) % 3]);
        let (n_bd, f_bd) = (uu.n[(j + 1) % 3], uu.fest[(j + 1) % 3]);
        let (n_dc, f_dc) = (uu.n[(j + 2) % 3], uu.fest[(j + 2) % 3]);
        self.t[t as usize] = Tri { v: [a, b, d], n: [n_bd, u, n_ab], fest: [f_bd, false, f_ab] };
        self.t[u as usize] = Tri { v: [a, d, c], n: [n_dc, n_ca, t], fest: [f_dc, f_ca, false] };
        if n_bd != KEIN {
            self.ersetze_nachbar(n_bd, u, t);
        }
        if n_ca != KEIN {
            self.ersetze_nachbar(n_ca, t, u);
        }
    }

    /// Gerader Weg vom Schwerpunkt von `start` zu `ziel`.
    fn gehe(&self, start: u32, ziel: Ip) -> Ort {
        let s = self.t[start as usize].v;
        let drei = |p: Ip| [3 * p[0] as i128, 3 * p[1] as i128];
        let mut g3 = [0i128; 2];
        for &v in &s {
            g3[0] += self.pi[v as usize][0] as i128;
            g3[1] += self.pi[v as usize][1] as i128;
        }
        let z3 = drei(ziel);
        let seite = |v: u32| if orient3(g3, z3, drei(self.pi[v as usize])) >= 0 { 1 } else { -1 };
        let mut cur = start;
        for _ in 0..(4 * self.t.len() + 16) {
            let tri = self.t[cur as usize];
            let p = |k: usize| self.pi[tri.v[k] as usize];
            let o = [orient(p(1), p(2), ziel), orient(p(2), p(0), ziel), orient(p(0), p(1), ziel)];
            if o.iter().all(|&x| x >= 0) {
                if (0..3).any(|k| p(k) == ziel) {
                    return Ort::Nichts;
                }
                return Ort::In(cur);
            }
            // Austritt: Anfang rechts der Geraden, Ende links oder auf ihr.
            let Some(i) = (0..3).find(|&i| seite(tri.v[(i + 1) % 3]) < 0 && seite(tri.v[(i + 2) % 3]) > 0)
            else {
                return Ort::Nichts;
            };
            if o[i] >= 0 {
                return Ort::Nichts;
            }
            if tri.fest[i] || tri.n[i] == KEIN {
                return Ort::Blockiert(tri.v[(i + 1) % 3], tri.v[(i + 2) % 3]);
            }
            cur = tri.n[i];
        }
        Ort::Nichts
    }

    /// Bowyer–Watson-Hohlraum um `p` ab dem Dreieck `t0`, das `p` enthält.
    /// `None`, wenn `p` nicht jedes Randstück des Hohlraums sieht.
    fn hohlraum(&mut self, t0: u32, p: Ip) -> Option<(Vec<u32>, Vec<Hohlstueck>)> {
        self.stempel = self.stempel.wrapping_add(1).max(1);
        let st = self.stempel;
        let mut drin = vec![t0];
        self.marke[t0 as usize] = st;
        let mut rand = Vec::new();
        let mut k = 0;
        while k < drin.len() {
            let tri = self.t[drin[k] as usize];
            k += 1;
            for i in 0..3 {
                let (a, b) = (tri.v[(i + 1) % 3], tri.v[(i + 2) % 3]);
                let u = tri.n[i];
                if tri.fest[i] || u == KEIN {
                    rand.push(Hohlstueck { a, b, aussen: u, fest: true });
                    continue;
                }
                if self.marke[u as usize] == st {
                    continue;
                }
                let uv = self.t[u as usize].v;
                let q = |v: u32| self.pi[v as usize];
                if umkreis(q(uv[0]), q(uv[1]), q(uv[2]), p) > 0 {
                    self.marke[u as usize] = st;
                    drin.push(u);
                } else {
                    rand.push(Hohlstueck { a, b, aussen: u, fest: false });
                }
            }
        }
        if rand.iter().any(|h| orient(self.pi[h.a as usize], self.pi[h.b as usize], p) <= 0) {
            return None;
        }
        Some((drin, rand))
    }

    fn neues(&mut self, tri: Tri) -> u32 {
        if let Some(id) = self.frei.pop() {
            self.t[id as usize] = tri;
            self.lebt[id as usize] = true;
            id
        } else {
            self.t.push(tri);
            self.lebt.push(true);
            self.marke.push(0);
            (self.t.len() - 1) as u32
        }
    }

    fn einsetzen(&mut self, p: Ip, pf: [f64; 2], drin: &[u32], rand: &[Hohlstueck]) -> Vec<u32> {
        let pv = self.pi.len() as u32;
        self.pi.push(p);
        self.pf.push(pf);
        for &t in drin {
            self.lebt[t as usize] = false;
            self.frei.push(t);
        }
        let mut neu = Vec::with_capacity(rand.len());
        let mut ab_a: HashMap<u32, u32> = HashMap::with_capacity(rand.len());
        let mut ab_b: HashMap<u32, u32> = HashMap::with_capacity(rand.len());
        for h in rand {
            let id = self.neues(Tri { v: [h.a, h.b, pv], n: [KEIN, KEIN, h.aussen], fest: [false, false, h.fest] });
            if h.aussen != KEIN {
                let j = self.kante_index(h.aussen, h.a, h.b);
                self.t[h.aussen as usize].n[j] = id;
            }
            ab_a.insert(h.a, id);
            ab_b.insert(h.b, id);
            neu.push(id);
        }
        for &id in &neu {
            let [a, b, _] = self.t[id as usize].v;
            // Kante (b, pv) teilt das Dreieck, das bei b beginnt; (pv, a) das, das bei a endet.
            self.t[id as usize].n[0] = ab_a[&b];
            self.t[id as usize].n[1] = ab_b[&a];
        }
        neu
    }

    fn umkreismitte(&self, t: u32) -> [f64; 2] {
        let v = self.t[t as usize].v;
        let (a, b, c) = (self.pf[v[0] as usize], self.pf[v[1] as usize], self.pf[v[2] as usize]);
        let (bx, by, cx, cy) = (b[0] - a[0], b[1] - a[1], c[0] - a[0], c[1] - a[1]);
        let d = 2.0 * (bx * cy - by * cx);
        let (b2, c2) = (bx * bx + by * by, cx * cx + cy * cy);
        [a[0] + (cy * b2 - by * c2) / d, a[1] + (bx * c2 - cx * b2) / d]
    }

    fn form_von(&self, t: u32) -> (f64, f64, [f64; 3]) {
        let v = self.t[t as usize].v;
        form(self.pf[v[0] as usize], self.pf[v[1] as usize], self.pf[v[2] as usize])
    }

    /// Randstück {a, b} als Index seines Anfangs im Umlauf.
    fn randstueck(&self, a: u32, b: u32) -> Option<usize> {
        let (a, b, n) = (a as usize, b as usize, self.nrand);
        if a >= n || b >= n {
            None
        } else if b == (a + 1) % n {
            Some(a)
        } else if a == (b + 1) % n {
            Some(b)
        } else {
            None
        }
    }

    /// Verfeinert, bis kein Dreieck mehr schlecht ist oder nur noch Randstücke
    /// geteilt werden könnten.
    pub fn verfeinere(&mut self, z: &Ziele) -> Ergebnis {
        let sin_ziel = z.winkel_grad.to_radians().sin();
        let r_max = GROESSE * z.kante_mm;
        let mut erg = Ergebnis::default();
        let bewerte = |s: &Self, t: u32| -> Option<(f64, bool, bool)> {
            let (sin_min, r, _) = s.form_von(t);
            let qual = sin_min < sin_ziel;
            let gross = r > r_max;
            (qual || gross).then(|| ((sin_ziel / sin_min.max(1e-12)).max(r / r_max), qual, gross))
        };
        let mut heap = BinaryHeap::new();
        for t in 0..self.t.len() as u32 {
            if self.lebt[t as usize] {
                if let Some((w, _, _)) = bewerte(self, t) {
                    heap.push(Eintrag { wert: w, t, v: self.t[t as usize].v });
                }
            }
        }
        while let Some(e) = heap.pop() {
            if !self.lebt[e.t as usize] || self.t[e.t as usize].v != e.v {
                continue;
            }
            let Some((_, qual, gross)) = bewerte(self, e.t) else { continue };
            if self.pi.len() >= z.max_punkte {
                erg.aufgegeben += 1;
                continue;
            }
            // Kleinster Winkel zwischen zwei Randkanten: Eingangswinkel, nicht zu verbessern.
            let tri = self.t[e.t as usize];
            let (_, _, l) = self.form_von(e.t);
            let kurz = (0..3).min_by(|&i, &j| l[i].total_cmp(&l[j])).unwrap();
            if qual && !gross && tri.fest[(kurz + 1) % 3] && tri.fest[(kurz + 2) % 3] {
                erg.aufgegeben += 1;
                continue;
            }
            let mitte = self.umkreismitte(e.t);
            let schwer = {
                let p = |k: usize| self.pf[tri.v[k] as usize];
                [(p(0)[0] + p(1)[0] + p(2)[0]) / 3.0, (p(0)[1] + p(1)[1] + p(2)[1]) / 3.0]
            };
            let mitte = if mitte[0].is_finite() && mitte[1].is_finite() {
                self.gitter.im_bereich(schwer, mitte)
            } else {
                erg.aufgegeben += 1;
                continue;
            };
            let ziel = self.gitter.ein(mitte);
            let melden = |s: &Self, a: u32, b: u32, erg: &mut Ergebnis| {
                let l = abstand(s.pf[a as usize], s.pf[b as usize]);
                let darf = (qual && l >= 2.0 * z.min_rand_mm) || (gross && l > z.kante_mm * 1.0001);
                match (darf, s.randstueck(a, b)) {
                    (true, Some(i)) => {
                        erg.teilen.insert(i);
                    }
                    _ => erg.aufgegeben += 1,
                }
            };
            match self.gehe(e.t, ziel) {
                Ort::Nichts => erg.aufgegeben += 1,
                Ort::Blockiert(a, b) => melden(self, a, b, &mut erg),
                Ort::In(tc) => {
                    let Some((drin, rand)) = self.hohlraum(tc, ziel) else {
                        erg.aufgegeben += 1;
                        continue;
                    };
                    let pf = self.gitter.aus(ziel);
                    let bedraengt: Vec<(u32, u32)> = rand
                        .iter()
                        .filter(|h| {
                            let (a, b) = (self.pf[h.a as usize], self.pf[h.b as usize]);
                            h.fest && (a[0] - pf[0]) * (b[0] - pf[0]) + (a[1] - pf[1]) * (b[1] - pf[1]) < 0.0
                        })
                        .map(|h| (h.a, h.b))
                        .collect();
                    if !bedraengt.is_empty() {
                        for (a, b) in bedraengt {
                            melden(self, a, b, &mut erg);
                        }
                        continue;
                    }
                    for t in self.einsetzen(ziel, pf, &drin, &rand) {
                        if let Some((w, _, _)) = bewerte(self, t) {
                            heap.push(Eintrag { wert: w, t, v: self.t[t as usize].v });
                        }
                    }
                }
            }
        }
        erg
    }

    /// Glättet die inneren Punkte (`schritte` Durchgänge).
    pub fn glaetten(&mut self, schritte: usize) {
        for _ in 0..schritte {
            let mut um: Vec<Vec<u32>> = vec![Vec::new(); self.pi.len()];
            for (ti, tri) in self.t.iter().enumerate() {
                if self.lebt[ti] {
                    for &v in &tri.v {
                        um[v as usize].push(ti as u32);
                    }
                }
            }
            let mut bewegt = false;
            for v in self.nrand..self.pi.len() {
                let ts = &um[v];
                if ts.is_empty() {
                    continue;
                }
                let kleinster = |s: &Self| -> f64 {
                    ts.iter().map(|&t| s.form_von(t).0).fold(f64::INFINITY, f64::min)
                };
                let vorher = kleinster(self);
                let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
                for &t in ts {
                    for &w in &self.t[t as usize].v {
                        if w as usize != v {
                            sx += self.pf[w as usize][0];
                            sy += self.pf[w as usize][1];
                            n += 1.0;
                        }
                    }
                }
                // Jeder Nachbar zählt doppelt (zwei anliegende Dreiecke).
                let neu_i = self.gitter.ein([sx / n, sy / n]);
                if neu_i == self.pi[v] {
                    continue;
                }
                let (alt_i, alt_f) = (self.pi[v], self.pf[v]);
                self.pi[v] = neu_i;
                self.pf[v] = self.gitter.aus(neu_i);
                let gueltig = ts.iter().all(|&t| {
                    let tv = self.t[t as usize].v;
                    orient(self.pi[tv[0] as usize], self.pi[tv[1] as usize], self.pi[tv[2] as usize]) > 0
                });
                if gueltig && kleinster(self) >= vorher {
                    bewegt = true;
                } else {
                    self.pi[v] = alt_i;
                    self.pf[v] = alt_f;
                }
            }
            let alle = self.innere_kanten();
            self.lawson(alle);
            if !bewegt {
                break;
            }
        }
    }

    pub fn dreiecke(&self) -> Vec<[u32; 3]> {
        (0..self.t.len()).filter(|&t| self.lebt[t]).map(|t| self.t[t].v).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ziele() -> Ziele {
        Ziele { kante_mm: 10.0, winkel_grad: 25.0, min_rand_mm: 1.0, max_punkte: 100_000 }
    }

    /// Prüft Umlauf, Nachbarschaft und Eulerzahl.
    fn pruefe(tr: &Triangulierung) {
        let d = tr.dreiecke();
        let mut kanten: HashMap<(u32, u32), usize> = HashMap::new();
        for t in &d {
            let p = |k: usize| tr.pi[t[k] as usize];
            assert!(orient(p(0), p(1), p(2)) > 0);
            for i in 0..3 {
                *kanten.entry((t[i], t[(i + 1) % 3])).or_default() += 1;
            }
        }
        let mut rand = 0;
        for (&(a, b), &n) in &kanten {
            assert_eq!(n, 1);
            if !kanten.contains_key(&(b, a)) {
                rand += 1;
                assert!(tr.randstueck(a, b).is_some(), "Randkante {a}-{b} nicht im Umlauf");
            }
        }
        assert_eq!(rand, tr.nrand);
        let e = (kanten.len() + rand) / 2;
        assert_eq!(tr.pi.len() as i64 - e as i64 + d.len() as i64, 1, "Eulerzahl");
    }

    fn quadrat(seite: f64, n: usize) -> Vec<[f64; 2]> {
        let mut r = Vec::new();
        for (a, b) in [([0.0, 0.0], [seite, 0.0]), ([seite, 0.0], [seite, seite]), ([seite, seite], [0.0, seite]), ([0.0, seite], [0.0, 0.0])] {
            for k in 0..n {
                let t = k as f64 / n as f64;
                r.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
            }
        }
        r
    }

    #[test]
    fn quadrat_ohne_verfeinerung() {
        // Kollineare Randpunkte: genau der Fall, an dem Gleitkomma scheitert.
        let tr = Triangulierung::neu(&quadrat(100.0, 10)).unwrap();
        pruefe(&tr);
        assert_eq!(tr.dreiecke().len(), 40 - 2);
    }

    #[test]
    fn quadrat_verfeinert_beide_umlaufsinne() {
        for umgedreht in [false, true] {
            let mut r = quadrat(100.0, 10);
            if umgedreht {
                r.reverse();
            }
            let mut tr = Triangulierung::neu(&r).unwrap();
            let erg = tr.verfeinere(&ziele());
            tr.glaetten(4);
            pruefe(&tr);
            assert!(erg.teilen.is_empty(), "{:?}", erg.teilen);
            let min = tr.dreiecke().iter().map(|t| form(tr.pf[t[0] as usize], tr.pf[t[1] as usize], tr.pf[t[2] as usize]).0).fold(1.0, f64::min);
            assert!(min.asin().to_degrees() >= 25.0, "{}", min.asin().to_degrees());
            let flaeche: f64 = tr.dreiecke().iter().map(|t| {
                let (a, b, c) = (tr.pf[t[0] as usize], tr.pf[t[1] as usize], tr.pf[t[2] as usize]);
                0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
            }).sum();
            assert!((flaeche - 10_000.0).abs() < 1e-6, "{flaeche}");
        }
    }

    #[test]
    fn l_form_und_spitze_ecke() {
        // L-Form mit einspringender Ecke und eine Ecke von 30°.
        let r: Vec<[f64; 2]> = vec![[0.0, 0.0], [60.0, 0.0], [60.0, 20.0], [20.0, 20.0], [20.0, 60.0], [0.0, 60.0]];
        let tr = Triangulierung::neu(&r).unwrap();
        pruefe(&tr);
        let spitz: Vec<[f64; 2]> = vec![[0.0, 0.0], [80.0, 0.0], [0.0, 80.0 * 30f64.to_radians().tan()]];
        let mut tr = Triangulierung::neu(&spitz).unwrap();
        tr.verfeinere(&ziele());
        pruefe(&tr);
    }

    #[test]
    fn fehlerhafte_raender() {
        assert!(Triangulierung::neu(&[[0.0, 0.0], [1.0, 0.0]]).is_err());
        let acht = [[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [0.0, 10.0]];
        assert!(Triangulierung::neu(&acht).unwrap_err().contains("schneidet"));
        let doppelt = [[0.0, 0.0], [10.0, 0.0], [10.0, 0.0], [0.0, 10.0]];
        assert!(Triangulierung::neu(&doppelt).unwrap_err().contains("fallen zusammen"));
    }
}
