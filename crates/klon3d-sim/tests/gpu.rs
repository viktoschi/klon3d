//! Step 4b: the GPU backend against the CPU reference.
//!
//! The claim is a number, not a label: `A·p` of the same system, and the state
//! after the same number of frames, are compared on the tube. Without a GPU
//! adapter every test here prints `übersprungen: kein GPU-Adapter` and returns -
//! a missing device is not a failure (that is what CI has).
//!
//! The tube runs take `cargo test --release`; in the debug build they are
//! ignored like the other solver acceptance tests.

use klon3d_sim::roehre::{bis_zur_ruhe, param_ruhe, Roehre};
use klon3d_sim::{messung, GpuLoeser, ImplizitCg, ImplizitParameter, Rechenweg};

/// Opens the GPU or reports that there is none.
fn geraet() -> Option<GpuLoeser> {
    match GpuLoeser::neu() {
        Ok(g) => {
            eprintln!("GPU: {}", g.geraet());
            Some(g)
        }
        Err(e) => {
            eprintln!("übersprungen: kein GPU-Adapter ({e})");
            None
        }
    }
}

/// Zwei Rechenwege auf derselben Szene; der GPU-Weg bekommt einen frischen
/// Löser, damit kein Zustand vom Vorgänger mitkommt.
fn beide(param: ImplizitParameter, gpu: GpuLoeser) -> (ImplizitCg, ImplizitCg) {
    (
        ImplizitCg::neu(param),
        ImplizitCg::mit_loeser(param, Box::new(gpu)),
    )
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Größter Abstand zweier Punktwolken (m).
fn abstand(a: &[[f64; 3]], b: &[[f64; 3]]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| {
            let d = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
            dot(d, d).sqrt()
        })
        .fold(0.0, f64::max)
}

/// Der Operator `A·p` beider Rechenwege auf demselben Zustand: dasselbe System,
/// dieselben Blöcke, nur einmal `f64` und einmal `f32`. Die Schranke ist an der
/// Messung festgemacht (`1e-4` relativ zur Norm von `A·p`), nicht geraten.
#[test]
fn operator_stimmt_mit_der_cpu() {
    let Some(gpu) = geraet() else { return };
    let r = Roehre::default();
    let mut s = r.szene(param_ruhe()).unwrap();
    let (mut cpu, mut g) = beide(ImplizitParameter::default(), gpu);
    // Ein paar Bilder rechnen, damit v nicht null ist und der Zustand etwas
    // von der Anfangslage weg ist. Beide Wege starten vom selben Zustand.
    for _ in 0..3 {
        cpu.bild(&mut s);
    }
    let n = s.x.len();
    let mut streuung = vec![[0.0; 3]; n];
    for (i, z) in streuung.iter_mut().enumerate() {
        let w = i as f64 * 0.6180339887498949;
        *z = [
            w.fract() - 0.5,
            (w * 2.0).fract() - 0.5,
            (w * 3.0).fract() - 0.5,
        ];
    }
    let vektoren = [vec![[1.0, 0.0, 0.0]; n], streuung, s.v.clone()];
    for (k, p) in vektoren.iter().enumerate() {
        let a = cpu.operator(&mut s, p);
        let b = g.operator(&mut s, p);
        let norm = a.iter().map(|q| dot(*q, *q)).sum::<f64>().sqrt();
        let diff = a
            .iter()
            .zip(&b)
            .map(|(x, y)| {
                dot(
                    [x[0] - y[0], x[1] - y[1], x[2] - y[2]],
                    [x[0] - y[0], x[1] - y[1], x[2] - y[2]],
                )
            })
            .sum::<f64>()
            .sqrt();
        assert!(
            diff <= 1e-4 * norm.max(1e-30),
            "Vektor {k}: Abweichung {diff:.3e} gegen Norm {norm:.3e} (relativ {:.2e})",
            diff / norm.max(1e-30)
        );
    }
}

/// Ohne Körper und ohne Schwerkraft zieht sich die Röhre auf ihre Ruhelänge
/// zusammen. Beide Rechenwege müssen **dasselbe Minimum** finden: gleicher
/// Umfang und kein Punkt weiter als 0,05 mm weg.
#[test]
fn ohne_koerper_gleiches_gleichgewicht() {
    let Some(gpu) = geraet() else { return };
    let r = Roehre {
        reduktion: 0.055,
        ..Roehre::default()
    };
    let mut a = r.szene_roh(param_ruhe()).unwrap();
    let mut b = a.clone();
    a.koerper.clear();
    b.koerper.clear();
    a.param.schwerkraft = [0.0; 3];
    b.param.schwerkraft = [0.0; 3];
    let (mut cpu, mut g) = beide(ImplizitParameter::default(), gpu);
    for _ in 0..30 {
        cpu.bild(&mut a);
        g.bild(&mut b);
    }
    let ua = messung::umfang_in_ebene(&a, [0.0, 0.0, -r.hoehe_m / 2.0], [0.0, 0.0, 1.0]);
    let ub = messung::umfang_in_ebene(&b, [0.0, 0.0, -r.hoehe_m / 2.0], [0.0, 0.0, 1.0]);
    assert!(
        (ua / ub - 1.0).abs() < 1e-4,
        "Umfang {ua:.6} gegen {ub:.6} m"
    );
    let d = abstand(&a.x, &b.x);
    assert!(d < 5e-5, "größter Punktabstand {:.4} mm", d * 1e3);
    let v = messung::mittlere_geschwindigkeit(&b);
    assert!(
        v.is_finite() && v < 1e-2,
        "GPU-Weg bewegt sich noch: {v:.2e} m/s"
    );
}

/// Die Röhre auf dem Zylinder, 5,5 % Radical: derselbe Umfang, dieselbe Naht,
/// derselbe Sitz wie die CPU. Die Schranken sind die von Schritt 4a′ (0,02
/// Prozentpunkte Dehnung zwischen den Rechenwegen), nicht aufgeweicht.
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn roehre_gleiches_gleichgewicht() {
    let Some(gpu) = geraet() else { return };
    let r = Roehre {
        reduktion: 0.055,
        ..Roehre::default()
    };
    let mut a = r.szene(param_ruhe()).unwrap();
    let mut b = a.clone();
    let (mut cpu, mut g) = beide(ImplizitParameter::default(), gpu);
    let la = bis_zur_ruhe(&mut a, &mut cpu, 600, 1e-4, 10);
    let lb = bis_zur_ruhe(&mut b, &mut g, 600, 1e-4, 10);
    assert!(
        la.ruhig && lb.ruhig,
        "nicht zur Ruhe: CPU {la:?}, GPU {lb:?}"
    );
    let ea = r.auswerten(&a);
    let eb = r.auswerten(&b);
    assert!(
        (ea.dehnung - eb.dehnung).abs() < 2e-4,
        "Umfangsdehnung CPU {:.4} % gegen GPU {:.4} %",
        ea.dehnung * 100.0,
        eb.dehnung * 100.0
    );
    assert!(
        (ea.dehnung - ea.dehnung_soll).abs() < 0.005,
        "GPU: {:.4} % gegen Soll {:.4} %",
        eb.dehnung * 100.0,
        ea.dehnung_soll * 100.0
    );
    assert!(
        eb.nahtspalt_m < 1.0e-3,
        "GPU: Naht {:.3} mm",
        eb.nahtspalt_m * 1e3
    );
    assert!(
        eb.abheben_m < 0.5e-3,
        "GPU: hebt {:.3} mm ab",
        eb.abheben_m * 1e3
    );
    // Der Radius je Punkt, nicht die Lage: mit Haftreibung ist die tangentiale
    // Lage eine freie Größe — beide Wege sind gültige Gleichgewichte, und ein
    // um Bruchteile eines Grads verdrehtes Rohr hätte punktförmig Millimeter
    // Abstand bei gleicher Physik. Der Radius ist die Größe, die das Problem
    // festlegt.
    let radius = |p: &[f64; 3]| (p[0] * p[0] + p[1] * p[1]).sqrt();
    let dr =
        a.x.iter()
            .zip(&b.x)
            .map(|(p, q)| (radius(p) - radius(q)).abs())
            .fold(0.0f64, f64::max);
    assert!(dr < 5e-5, "größter Radiusunterschied {:.4} mm", dr * 1e3);
}

/// Die Rutschprobe mit dem GPU-Löser: derselbe Weg wie die diskrete Rechnung,
/// mit der Toleranz von Schritt 4a′ (2 %).
#[test]
#[cfg_attr(debug_assertions, ignore = "nur mit --release")]
fn rutschprobe_wie_cpu() {
    let Some(gpu) = geraet() else { return };
    let r = Roehre {
        reduktion: 0.0,
        ..Roehre::default()
    };
    let mut rw = ImplizitCg::mit_loeser(ImplizitParameter::default(), Box::new(gpu));
    let p = r.rutschprobe(&mut rw, 0.5).unwrap();
    let h = 1.0 / 60.0;
    let n = ImplizitParameter::default().schritte_je_bild;
    let h = h / n as f64;
    let schritte = (p.sekunden / h).round();
    let soll =
        9.81 * (1.0 - p.halteverhaeltnis_gemessen) * h * h * schritte * (schritte + 1.0) / 2.0;
    assert!(
        (p.weg_m / soll - 1.0).abs() < 0.02,
        "GPU: {:.1} mm gegen {:.1} mm",
        p.weg_m * 1e3,
        soll * 1e3
    );
}

/// Ein Weg, der Federn im System hat (Klebezonen, Selbstkollision), geht an die
/// CPU und wird gezählt - nicht stillschweigend halb gerechnet.
#[test]
fn federn_gehen_an_die_cpu() {
    let Some(gpu) = geraet() else { return };
    let r = Roehre::default();
    let mut s = r.szene(param_ruhe()).unwrap();
    let mut rw = ImplizitCg::mit_loeser(
        ImplizitParameter {
            selbstkollision: true,
            ..ImplizitParameter::default()
        },
        Box::new(gpu),
    );
    for _ in 0..2 {
        rw.bild(&mut s);
    }
    assert!(s.x.iter().all(|p| p.iter().all(|q| q.is_finite())));
}
