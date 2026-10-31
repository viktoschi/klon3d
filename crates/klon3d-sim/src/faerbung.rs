//! Graphfärbung der Constraints.
//!
//! Zwei Constraints derselben Farbe teilen keinen Punkt. Innerhalb einer
//! Farbe sind die Korrekturen deshalb unabhängig: die CPU-Referenz rechnet sie
//! nacheinander, ein Compute-Shader (ADR-018) gleichzeitig, und beide kommen
//! zum selben Ergebnis — ohne Atomics.

/// Färbt Constraints gierig in Eingabereihenfolge. `punkte(c)` nennt die
/// Punkte von Constraint `c`. Ergebnis: Farbe je Constraint und Zahl der
/// Farben.
pub fn faerbe<'a>(
    anzahl: usize,
    npunkte: usize,
    punkte: impl Fn(usize) -> &'a [u32],
) -> (Vec<u32>, usize) {
    // Je Punkt die belegten Farben als Bitmaske; mehr als 64 Farben kommen
    // bei Dreiecksnetzen nicht vor, sonst greift die Liste.
    let mut maske = vec![0u64; npunkte];
    let mut mehr: Vec<Vec<u32>> = vec![Vec::new(); npunkte];
    let mut farbe = vec![0u32; anzahl];
    let mut zahl = 0usize;
    for c in 0..anzahl {
        let p = punkte(c);
        let belegt = p.iter().fold(0u64, |m, &i| m | maske[i as usize]);
        let f = if belegt != u64::MAX {
            (!belegt).trailing_zeros()
        } else {
            (64u32..)
                .find(|f| p.iter().all(|&i| !mehr[i as usize].contains(f)))
                .unwrap()
        };
        for &i in p {
            if f < 64 {
                maske[i as usize] |= 1 << f;
            } else {
                mehr[i as usize].push(f);
            }
        }
        farbe[c] = f;
        zahl = zahl.max(f as usize + 1);
    }
    (farbe, zahl)
}

/// Reihenfolge nach Farbe (stabil) und die Bereiche je Farbe.
pub fn ordnung(farbe: &[u32], zahl: usize) -> (Vec<usize>, Vec<std::ops::Range<usize>>) {
    let mut idx: Vec<usize> = (0..farbe.len()).collect();
    idx.sort_by_key(|&c| farbe[c]);
    let mut bereiche = Vec::with_capacity(zahl);
    let mut start = 0;
    for f in 0..zahl as u32 {
        let ende = start + idx[start..].iter().take_while(|&&c| farbe[c] == f).count();
        bereiche.push(start..ende);
        start = ende;
    }
    (idx, bereiche)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keine_farbe_teilt_einen_punkt() {
        // Kette 0-1-2-3-4 und ein Stern um 5.
        let c: Vec<[u32; 2]> = vec![[0, 1], [1, 2], [2, 3], [3, 4], [5, 0], [5, 1], [5, 2], [5, 3]];
        let (f, n) = faerbe(c.len(), 6, |i| &c[i][..]);
        assert!(n >= 4, "Stern mit 4 Strahlen braucht 4 Farben, n = {n}");
        for a in 0..c.len() {
            for b in a + 1..c.len() {
                if f[a] == f[b] {
                    assert!(c[a].iter().all(|p| !c[b].contains(p)), "{a} und {b}");
                }
            }
        }
        let (idx, ber) = ordnung(&f, n);
        assert_eq!(ber.len(), n);
        assert_eq!(ber.last().unwrap().end, c.len());
        for (k, r) in ber.iter().enumerate() {
            assert!(idx[r.clone()].iter().all(|&i| f[i] == k as u32));
        }
    }
}
