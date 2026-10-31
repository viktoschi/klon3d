//! Alle 58 Maße gegen die Prüfwerte der Spezifikation.
//!
//! Netz: CLOs `BaseMesh.obj` in Zentimetern, nur zentriert. Prüfwerte:
//! `spez/pruefwerte.json`, `basemesh_ungeformt` (Werte, Werte
//! je Slot, Punkte der Wege, gewählte Kandidaten). Toleranz nach
//! `messung.md` 7: 0,002 cm. Ebenen genau durch drei Netzpunkte (geneigte
//! Umfänge und Wege mit drei Punkten) kippen am letzten Bit
//! (Befund 30); dort gilt die Toleranz der ganzen Kette, 0,03 cm.
//!
//! Fehlen CLOs Dateien, wird der Test übersprungen; `CLO_FEMALE` setzt einen
//! anderen Ordner.

use klon3d_koerper::lies_obj;
use klon3d_koerper::massband::{zentriert_cm, Messdaten};
use klon3d_koerper::messung::{messen, Wahl, ANZAHL, NAMEN};
use std::path::PathBuf;

fn ordner() -> Option<PathBuf> {
    let p = std::env::var_os("CLO_FEMALE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female")
        });
    if [
        "BaseMesh.obj",
        "BaseMesh.be",
        "range_list.rlist",
        "IndexSet.txt",
    ]
    .iter()
    .all(|f| p.join(f).is_file())
    {
        Some(p)
    } else {
        eprintln!("übersprungen: CLO-Resizing-Daten fehlen in {}", p.display());
        None
    }
}

/// `basemesh_ungeformt.werte`.
#[rustfmt::skip]
const WERTE: [f64; ANZAHL] = [163.812512, 0.0, 97.733055, 33.301993, 38.368874, 84.624559, 79.961621, 97.504884, 103.42468, 60.997047, 50.305009, 36.533457, 36.023101, 22.609133, 34.53929, 30.093075, 23.661495, 15.071142, 139.751012, 135.690012, 4.061, 119.615012, 113.208512, 106.305512, 93.265112, 81.390312, 76.737312, 74.093112, 61.449962, 45.027262, 32.672162, 7.652337, 29.159832, 20.150405, 34.006726, 33.371582, 38.665639, 19.219391, 7.909486, 11.578307, 32.533749, 36.064894, 13.179, 25.569346, 18.307856, 25.977506, 70.774585, 55.475261, 74.694652, 77.207668, 33.325946, 43.881722, 29.5682, 22.608475, 15.28735, 72.761882, 83.161357, 155.92324];

/// Werte je Slot (`slot0`, `slot1`), `NaN` ohne Slot.
#[rustfmt::skip]
const SLOTS: [[f64; 2]; ANZAHL] = [[f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [19.5493, 18.819574], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [60.655194, 61.338901], [50.16689, 50.443129], [36.579711, 36.487203], [36.007006, 36.039196], [22.435767, 22.782499], [42.765353, 26.313228], [30.167997, 30.018152], [23.542528, 23.780462], [14.788056, 15.354227], [139.842512, 139.659512], [135.878512, 135.501512], [3.964, 4.158], [119.683512, 119.546512], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [74.079112, 74.107112], [61.480912, 61.419012], [45.080312, 44.974212], [32.707112, 32.637212], [7.648612, 7.656062], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [19.211325, 19.227457], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [26.083377, 25.871635], [f64::NAN, f64::NAN], [55.032213, 55.91831], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [f64::NAN, f64::NAN], [72.866608, 72.657157], [83.290272, 83.032443], [f64::NAN, f64::NAN]];

/// Punkte der Ergebnislinie je Weg-Slot.
#[rustfmt::skip]
const WEGPUNKTE: [[usize; 2]; ANZAHL] = [[0, 0], [0, 0], [0, 0], [0, 0], [36, 34], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [55, 28], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [31, 0], [34, 0], [39, 0], [13, 13], [6, 0], [3, 0], [23, 0], [28, 0], [9, 0], [22, 0], [0, 0], [18, 16], [0, 0], [52, 48], [0, 0], [55, 0], [28, 0], [0, 0], [0, 0], [0, 0], [0, 0], [45, 45], [40, 39], [0, 0]];

/// Maße, deren Ebene genau durch drei Netzpunkte geht.
const DREI_PUNKTE: [usize; 9] = [3, 4, 14, 15, 16, 17, 36, 47, 49];
/// Davon abhängige Summen.
const ABHAENGIG: [usize; 3] = [46, 48, 51];

const TOLERANZ: f64 = 0.002;
const TOLERANZ_DREI: f64 = 0.03;

#[test]
fn alle_58_masse_treffen_die_pruefwerte() {
    let Some(o) = ordner() else { return };
    let netz = lies_obj(&o.join("BaseMesh.obj"), 1.0).expect("BaseMesh.obj");
    let d = Messdaten::laden(&o).expect("Messdaten");
    let v = zentriert_cm(&netz, 1.0);
    let m = messen(&v, &d).expect("Messung");

    assert_eq!(
        m.wahl,
        Wahl {
            brustpaar: (5969, 21601),
            unterbrust: 20337,
            taille: 15538,
            huefte_oben: 15741,
            huefte_unten: 5000,
            wade: (8041, 23672),
        }
    );

    let mut fehler = Vec::new();
    for j in 0..ANZAHL {
        let tol = if DREI_PUNKTE.contains(&j) || ABHAENGIG.contains(&j) {
            TOLERANZ_DREI
        } else {
            TOLERANZ
        };
        let abw = m.werte[j] - WERTE[j];
        let mut zeile = format!(
            "{j:2} {:28} {:10.5} soll {:10.5} ({:+.5})",
            NAMEN[j], m.werte[j], WERTE[j], abw
        );
        for s in 0..2 {
            if !SLOTS[j][s].is_nan() {
                zeile += &format!("  S{s} {:+.5}", m.slots[j][s] - SLOTS[j][s]);
            }
        }
        if WEGPUNKTE[j] != [0, 0] {
            zeile += &format!("  Wege {:?} soll {:?}", m.wegpunkte[j], WEGPUNKTE[j]);
        }
        println!("{zeile}");
        if abw.abs() > tol {
            fehler.push(format!(
                "{} ({j}): {:.5} statt {:.5}",
                NAMEN[j], m.werte[j], WERTE[j]
            ));
        }
    }
    assert!(
        fehler.is_empty(),
        "{}",
        fehler.join(
            "
"
        )
    );
}
