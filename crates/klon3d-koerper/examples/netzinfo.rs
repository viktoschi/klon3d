//! Diagnose des CLO-Basismeshes. Kein Teil des Produkts.
//!
//! Beantwortet drei Fragen, die die Abnahme offen lässt:
//! * Ist das Netz eine einzige geschlossene Fläche oder aus Teilen gebaut?
//! * Welche Kanten sind offen (Rand)?
//! * Welche geschlossenen Schnittschleifen gibt es auf einer Höhe, und
//!   welche davon umschliesst die Achse?
//!
//! Aufruf: `cargo run --release -p klon3d-koerper --example netzinfo`

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

type P3 = [f64; 3];

fn lies_obj(pfad: &PathBuf) -> (Vec<P3>, Vec<[usize; 3]>) {
    let text = fs::read_to_string(pfad).expect("OBJ lesbar");
    let mut punkte = Vec::new();
    let mut dreiecke = Vec::new();
    for zeile in text.lines() {
        let mut teile = zeile.split_whitespace();
        match teile.next() {
            Some("v") => {
                let x: f64 = teile.next().unwrap().parse().unwrap();
                let y: f64 = teile.next().unwrap().parse().unwrap();
                let z: f64 = teile.next().unwrap().parse().unwrap();
                punkte.push([x * 10.0, y * 10.0, z * 10.0]);
            }
            Some("f") => {
                let mut idx = Vec::new();
                for t in teile {
                    let i: usize = t.split('/').next().unwrap().parse::<usize>().unwrap() - 1;
                    idx.push(i);
                }
                for k in 1..idx.len() - 1 {
                    dreiecke.push([idx[0], idx[k], idx[k + 1]]);
                }
            }
            _ => {}
        }
    }
    (punkte, dreiecke)
}

fn find(vater: &mut Vec<usize>, mut x: usize) -> usize {
    while vater[x] != x {
        vater[x] = vater[vater[x]];
        x = vater[x];
    }
    x
}

fn vereinige(vater: &mut Vec<usize>, a: usize, b: usize) {
    let (ra, rb) = (find(vater, a), find(vater, b));
    if ra != rb {
        vater[ra] = rb;
    }
}

const RASTER: f64 = 1.0e-3;

fn raster(p: (f64, f64)) -> (i64, i64) {
    ((p.0 / RASTER).round() as i64, (p.1 / RASTER).round() as i64)
}

fn umschliesst(ring: &[(f64, f64)]) -> bool {
    let mut k = 0;
    for i in 0..ring.len() {
        let (x1, z1) = ring[i];
        let (x2, z2) = ring[(i + 1) % ring.len()];
        if (z1 > 0.0) != (z2 > 0.0) {
            let t = -z1 / (z2 - z1);
            if x1 + t * (x2 - x1) > 0.0 {
                k += 1;
            }
        }
    }
    k % 2 == 1
}

/// Wie `schleifen`, liefert zusätzlich die offenen Ketten als
/// `(Länge, Anzahl Stücke, Bounding-Box, Anfang, Ende)`.
fn schleifen_und_offen(
    punkte: &[P3],
    dreiecke: &[[usize; 3]],
    y: f64,
) -> (
    Vec<(f64, bool, [f64; 4], usize)>,
    Vec<(f64, usize, [f64; 4], (f64, f64), (f64, f64))>,
    usize,
) {
    let mut stuecke: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for &[a, b, c] in dreiecke {
        let mut treffer: Vec<(f64, f64)> = Vec::with_capacity(2);
        for e in 0..3 {
            let p = punkte[[a, b, c][e]];
            let q = punkte[[a, b, c][(e + 1) % 3]];
            if (p[1] - y) * (q[1] - y) > 0.0 || p[1] == q[1] {
                continue;
            }
            let t = (y - p[1]) / (q[1] - p[1]);
            treffer.push((p[0] + t * (q[0] - p[0]), p[2] + t * (q[2] - p[2])));
        }
        if treffer.len() == 2 {
            stuecke.push((treffer[0], treffer[1]));
        }
    }

    let mut an_endpunkt: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in stuecke.iter().enumerate() {
        an_endpunkt.entry(raster(s.0)).or_default().push(i);
        an_endpunkt.entry(raster(s.1)).or_default().push(i);
    }

    let mut benutzt = vec![false; stuecke.len()];
    let mut ergebnis = Vec::new();
    let mut offen = Vec::new();
    for start in 0..stuecke.len() {
        if benutzt[start] {
            continue;
        }
        benutzt[start] = true;
        let mut ring = vec![start];
        let mut punkt = stuecke[start].1;
        while let Some(&weiter) = an_endpunkt
            .get(&raster(punkt))
            .and_then(|k| k.iter().find(|j| !benutzt[**j]))
        {
            benutzt[weiter] = true;
            let (p, q) = stuecke[weiter];
            punkt = if raster(p) == raster(punkt) { q } else { p };
            ring.push(weiter);
        }
        let pts: Vec<(f64, f64)> = ring.iter().map(|i| stuecke[*i].1).collect();
        let laenge: f64 = pts
            .iter()
            .zip(pts.iter().cycle().skip(1))
            .map(|(p, q)| ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt())
            .sum();
        let (mut x0, mut x1, mut z0, mut z1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for (x, z) in &pts {
            x0 = x0.min(*x);
            x1 = x1.max(*x);
            z0 = z0.min(*z);
            z1 = z1.max(*z);
        }
        let bbox = [x0, x1, z0, z1];
        if raster(stuecke[ring[ring.len() - 1]].1) != raster(stuecke[start].0) {
            offen.push((
                laenge,
                ring.len(),
                bbox,
                stuecke[start].0,
                stuecke[ring[ring.len() - 1]].1,
            ));
            continue;
        }
        ergebnis.push((laenge, umschliesst(&pts), bbox, ring.len()));
    }
    ergebnis.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    offen.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    (ergebnis, offen, stuecke.len())
}

fn main() {
    let ordner = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female".into()),
    );
    let (punkte, dreiecke) = lies_obj(&ordner.join("BaseMesh.obj"));
    println!("{} Punkte, {} Dreiecke", punkte.len(), dreiecke.len());

    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for p in &punkte {
        lo = lo.min(p[1]);
        hi = hi.max(p[1]);
    }
    println!("Höhe {:.2} .. {:.2} cm", lo / 10.0, hi / 10.0);

    // Zusammenhangskomponenten über gemeinsame Punkte.
    let mut vater: Vec<usize> = (0..punkte.len()).collect();
    for &[a, b, c] in &dreiecke {
        vereinige(&mut vater, a, b);
        vereinige(&mut vater, b, c);
    }
    let mut komponenten: HashMap<usize, (usize, [f64; 6])> = HashMap::new();
    for (i, p) in punkte.iter().enumerate() {
        let r = find(&mut vater, i);
        let e = komponenten.entry(r).or_insert((0, [f64::MAX, f64::MIN, f64::MAX, f64::MIN, f64::MAX, f64::MIN]));
        e.0 += 1;
        for c in 0..3 {
            e.1[c * 2] = e.1[c * 2].min(p[c]);
            e.1[c * 2 + 1] = e.1[c * 2 + 1].max(p[c]);
        }
    }
    let mut kl: Vec<_> = komponenten.into_iter().collect();
    kl.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    println!("\n=== Zusammenhangskomponenten: {} ===", kl.len());
    for (_, (n, b)) in kl.iter().take(8) {
        println!(
            "{n:7} Punkte   x {:.1}..{:.1}  y {:.1}..{:.1}  z {:.1}..{:.1} cm",
            b[0] / 10.0, b[1] / 10.0, b[2] / 10.0, b[3] / 10.0, b[4] / 10.0, b[5] / 10.0
        );
    }

    // Offene Kanten: nur von einem Dreieck benutzt.
    let mut kanten: HashMap<(usize, usize), usize> = HashMap::new();
    for &[a, b, c] in &dreiecke {
        for (p, q) in [(a, b), (b, c), (c, a)] {
            *kanten.entry((p.min(q), p.max(q))).or_default() += 1;
        }
    }
    let offen = kanten.values().filter(|n| **n == 1).count();
    let mehrfach = kanten.values().filter(|n| **n > 2).count();
    println!("\nKanten: {} gesamt, {offen} offen (Rand), {mehrfach} mehr als zweimal", kanten.len());

    // Verschweissen: Punkte mit gleicher Lage zusammenfassen. CLOs Netz ist
    // an den Naehten geteilt; erst danach ist der Schnitt geschlossen.
    let mut karte: HashMap<(i64, i64, i64), usize> = HashMap::new();
    let mut neu: Vec<usize> = vec![usize::MAX; punkte.len()];
    let mut geschweisst: Vec<P3> = Vec::new();
    for (i, p) in punkte.iter().enumerate() {
        let k = (
            (p[0] / 1.0e-3).round() as i64,
            (p[1] / 1.0e-3).round() as i64,
            (p[2] / 1.0e-3).round() as i64,
        );
        let e = *karte.entry(k).or_insert_with(|| {
            geschweisst.push(*p);
            geschweisst.len() - 1
        });
        neu[i] = e;
    }
    let mut dt: Vec<[usize; 3]> = dreiecke
        .iter()
        .map(|d| [neu[d[0]], neu[d[1]], neu[d[2]]])
        .filter(|d| d[0] != d[1] && d[1] != d[2] && d[0] != d[2])
        .collect();
    dt.sort();
    dt.dedup();
    let mut kanten2: HashMap<(usize, usize), usize> = HashMap::new();
    for &[a, b, c] in &dt {
        for (p, q) in [(a, b), (b, c), (c, a)] {
            *kanten2.entry((p.min(q), p.max(q))).or_default() += 1;
        }
    }
    let offen2 = kanten2.values().filter(|n| **n == 1).count();
    println!(
        "Nach Verschweissen: {} Punkte, {} Dreiecke, {} offene Kanten",
        geschweisst.len(),
        dt.len(),
        offen2
    );

    println!("\n=== Schnittschleifen je Höhe ===");
    for y in (60..=160).step_by(4) {
        let y_mm = y as f64 * 10.0 + 1.0e-6;
        let (s, o, n_stuecke) = schleifen_und_offen(&geschweisst, &dt, y_mm);        let achse: Vec<_> = s.iter().filter(|x| x.1).collect();
        let offen_text = if o.is_empty() {
            String::new()
        } else {
            format!(
                ", {} offene Ketten (längste {:.1} cm, {} Stück, x {:.0}..{:.0}, Spalt {:.3} cm)",
                o.len(),
                o[0].0 / 10.0,
                o[0].1,
                o[0].2[0] / 10.0,
                o[0].2[1] / 10.0,
                ((o[0].3 .0 - o[0].4 .0).powi(2) + (o[0].3 .1 - o[0].4 .1).powi(2)).sqrt() / 10.0
            )
        };
        let text = if achse.is_empty() {
            format!("{n_stuecke} Stücke, {} Schleifen, keine um die Achse{offen_text}", s.len())
        } else {
            let (l, _, b, n) = achse[0];
            format!(
                "{n_stuecke} Stücke, {} Schleifen, Achse: {:.2} cm ({} Pkt, x {:.0}..{:.0}, z {:.0}..{:.0}){offen_text}",
                s.len(),
                l / 10.0,
                n,
                b[0] / 10.0,
                b[1] / 10.0,
                b[2] / 10.0,
                b[3] / 10.0
            )
        };
        println!("{y:4} cm  {text}");
    }
}
