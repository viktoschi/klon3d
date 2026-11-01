//! Summary of a distribution of measured values.
//!
//! Both inspection measures of stage 5 report the same seven numbers per
//! pattern piece, so the summary lives here instead of next to one of them.
//! Values arrive in millimetres or percent; the struct does not care, it only
//! sorts and counts.
//!
//! Not [`crate::vergleich::Kennzahlen`]: that one starts at zero by nature (two
//! meshes lying on each other) and has no minimum. For a distance map the
//! minimum is the value that matters — it is the one that shows penetration —
//! and for a strain map it is the one that shows compression. Both need it.

/// Distribution of single values.
///
/// The percentiles are taken from the sorted list at `floor(n·p)`, not
/// interpolated: with a few hundred faces per piece an interpolated value
/// suggests a precision the sample does not have.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verteilung {
    pub anzahl: usize,
    pub min: f64,
    pub mittel: f64,
    pub median: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: f64,
}

impl Verteilung {
    /// Summarises single values. Empty input gives all zeros.
    pub fn aus(mut d: Vec<f64>) -> Verteilung {
        if d.is_empty() {
            return Verteilung {
                anzahl: 0,
                min: 0.0,
                mittel: 0.0,
                median: 0.0,
                p90: 0.0,
                p99: 0.0,
                max: 0.0,
            };
        }
        d.sort_by(f64::total_cmp);
        let n = d.len();
        let q = |f: f64| d[((n as f64 * f) as usize).min(n - 1)];
        Verteilung {
            anzahl: n,
            min: d[0],
            mittel: d.iter().sum::<f64>() / n as f64,
            median: q(0.5),
            p90: q(0.9),
            p99: q(0.99),
            max: d[n - 1],
        }
    }

    /// Same values, weighted by `gewicht` (for example the face area): a big
    /// face counts more than a small one. Used for the mean, the rest is the
    /// unweighted sample. Empty or all-zero weights fall back to [`Self::aus`].
    pub fn aus_gewichtet(d: Vec<f64>, gewicht: &[f64]) -> Verteilung {
        let mut v = Self::aus(d.clone());
        let summe: f64 = gewicht.iter().sum();
        if d.is_empty() || gewicht.len() != d.len() || summe <= 0.0 {
            return v;
        }
        v.mittel = d.iter().zip(gewicht).map(|(x, w)| x * w).sum::<f64>() / summe;
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kennzahlen_stehen_an_ihrem_platz() {
        let v = Verteilung::aus(vec![3.0, 1.0, 2.0, 5.0, 4.0]);
        assert_eq!((v.anzahl, v.min, v.max), (5, 1.0, 5.0));
        assert_eq!(v.median, 3.0);
        assert_eq!(v.mittel, 3.0);
        // floor(n·p): p90 der fünf Werte ist der fünfte, p99 ebenso.
        assert_eq!(v.p90, 5.0);
        assert_eq!(v.p99, 5.0);
    }

    #[test]
    fn leer_ist_null_und_kein_nan() {
        let v = Verteilung::aus(Vec::new());
        assert_eq!(v.anzahl, 0);
        assert_eq!(
            (v.min, v.mittel, v.median, v.p90, v.p99, v.max),
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn gewicht_zieht_das_mittel_aber_nicht_den_median() {
        // Zwei Werte, der kleine hat das zehnfache Gewicht.
        let v = Verteilung::aus_gewichtet(vec![1.0, 10.0], &[10.0, 1.0]);
        assert!(
            (v.mittel - 20.0 / 11.0).abs() < 1e-12,
            "Mittel {}",
            v.mittel
        );
        assert_eq!(v.median, 10.0, "Median bleibt der Stichprobenwert");
        // Ohne Gewicht fällt es auf das ungewichtete Mittel zurück.
        let v = Verteilung::aus_gewichtet(vec![1.0, 10.0], &[0.0, 0.0]);
        assert_eq!(v.mittel, 5.5);
    }
}
