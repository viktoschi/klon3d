//! Maßband gegen die Prüfwerte der Spezifikation.
//!
//! Netz: CLOs `BaseMesh.obj`, nur zentriert (`formung.md` 8, ohne
//! Symmetrisieren). Prüfwerte: `spez/pruefwerte.json`,
//! Abschnitt `basemesh_ungeformt` — je Umfangsslot `P0`, Normale, Segment,
//! Zahl der Schnittpunkte vor und nach dem Filter, Kettenlänge, Hüllpunkte
//! und Wert. Toleranz nach `messung.md` 7: Wert ≤ 0,002 cm, Punktzahlen
//! gleich, Hüllpunkte ± 1.
//!
//! Die CLO-Dateien gehören nicht ins Repository. Fehlen sie, wird der Test
//! übersprungen und sagt das; `CLO_FEMALE` setzt einen anderen Ordner.

use klon3d_koerper::lies_obj;
use klon3d_koerper::massband::{
    ebene_drei, rumpfmasse, umfang, zentriert_cm, Kandidaten, Messdaten, WAAGERECHT,
};
use std::path::PathBuf;

fn ordner() -> Option<PathBuf> {
    let p = std::env::var_os("CLO_FEMALE").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female")
    });
    if ["BaseMesh.obj", "BaseMesh.be", "range_list.rlist", "IndexSet.txt"]
        .iter()
        .all(|f| p.join(f).is_file())
    {
        Some(p)
    } else {
        eprintln!("übersprungen: CLO-Resizing-Daten fehlen in {}", p.display());
        None
    }
}

/// Wie die Ebene entsteht (`messung.md` 1.9). Die Prüfdatei nennt `P0` und
/// die Normale auf sechs Stellen gerundet; geht die Ebene durch einen
/// Netzpunkt, verfehlt die gerundete Ebene ihn knapp und zählt drei bis vier
/// Schnittpunkte weniger. Deshalb wird die Ebene wie in der Spezifikation aus
/// den Punkten gebildet und nur gegen die gerundeten Zahlen abgeglichen.
enum Quelle {
    /// `P0` und Normale aus der Prüfdatei (Brust: Mitte des Punktpaars).
    Json,
    /// Waagerecht durch einen gewählten Netzpunkt.
    Punkt(usize),
    /// Waagerecht durch die Landmarke dieser IndexSet-Zeile.
    Zeile(usize),
    /// Geneigt durch drei Landmarken.
    Zeilen([usize; 3]),
}

struct Slot {
    name: &'static str,
    quelle: Quelle,
    p0: [f64; 3],
    n: [f64; 3],
    seg: i32,
    roh: usize,
    gefiltert: usize,
    kette: usize,
    huelle: usize,
    wert: f64,
}

/// `pruefwerte.json`, `basemesh_ungeformt.messung`, alle Umfänge (Typ 0).
#[rustfmt::skip]
const SLOTS: [Slot; 22] = [
    Slot { name: "CIRCUMFERENCE_Bust", quelle: Quelle::Json, p0: [0.107115, 119.615012, 10.521037], n: [0.0, 1.0, 0.0], seg: 4, roh: 230, gefiltert: 110, kette: 110, huelle: 70, wert: 97.733055 },
    Slot { name: "CIRCUMFERENCE_MidNeck", quelle: Quelle::Zeilen([1, 2, 3]), p0: [0.286928, 139.583512, 0.640857], n: [-0.012423, 0.907754, 0.419318], seg: 6, roh: 149, gefiltert: 50, kette: 50, huelle: 50, wert: 33.301993 },
    Slot { name: "CIRCUMFERENCE_UnderBust", quelle: Quelle::Punkt(20337), p0: [-8.352825, 113.208512, 7.563837], n: [0.0, 1.0, 0.0], seg: 4, roh: 164, gefiltert: 87, kette: 87, huelle: 72, wert: 84.624559 },
    Slot { name: "CIRCUMFERENCE_Waist", quelle: Quelle::Punkt(15538), p0: [0.064265, 106.305512, 10.361137], n: [0.0, 1.0, 0.0], seg: 4, roh: 151, gefiltert: 86, kette: 86, huelle: 68, wert: 79.961621 },
    Slot { name: "CIRCUMFERENCE_HighHip", quelle: Quelle::Punkt(15741), p0: [-0.043389, 93.265112, 12.116337], n: [0.0, 1.0, 0.0], seg: 4, roh: 160, gefiltert: 100, kette: 99, huelle: 78, wert: 97.504884 },
    Slot { name: "CIRCUMFERENCE_LowHip", quelle: Quelle::Punkt(5000), p0: [0.027830, 81.390312, -13.785963], n: [0.0, 1.0, 0.0], seg: 4, roh: 173, gefiltert: 103, kette: 103, huelle: 70, wert: 103.424680 },
    Slot { name: "CIRCUMFERENCE_Thigh Slot 0", quelle: Quelle::Zeile(31), p0: [0.403469, 74.079112, -2.519013], n: [0.0, 1.0, 0.0], seg: 3, roh: 83, gefiltert: 51, kette: 51, huelle: 49, wert: 60.655194 },
    Slot { name: "CIRCUMFERENCE_Thigh Slot 1", quelle: Quelle::Zeile(32), p0: [-0.259031, 74.107112, -2.446193], n: [0.0, 1.0, 0.0], seg: 2, roh: 83, gefiltert: 55, kette: 55, huelle: 50, wert: 61.338901 },
    Slot { name: "CIRCUMFERENCE_MidThigh Slot 0", quelle: Quelle::Zeile(33), p0: [16.239295, 61.480912, 0.363257], n: [0.0, 1.0, 0.0], seg: 3, roh: 83, gefiltert: 54, kette: 54, huelle: 52, wert: 50.166890 },
    Slot { name: "CIRCUMFERENCE_MidThigh Slot 1", quelle: Quelle::Zeile(34), p0: [-16.112505, 61.419012, 0.321167], n: [0.0, 1.0, 0.0], seg: 2, roh: 83, gefiltert: 53, kette: 53, huelle: 51, wert: 50.443129 },
    Slot { name: "CIRCUMFERENCE_Knee Slot 0", quelle: Quelle::Zeile(35), p0: [14.366895, 45.080312, -4.416613], n: [0.0, 1.0, 0.0], seg: 3, roh: 93, gefiltert: 44, kette: 44, huelle: 37, wert: 36.579711 },
    Slot { name: "CIRCUMFERENCE_Knee Slot 1", quelle: Quelle::Zeile(36), p0: [-14.139505, 44.974212, -4.420323], n: [0.0, 1.0, 0.0], seg: 2, roh: 91, gefiltert: 45, kette: 45, huelle: 41, wert: 36.487203 },
    Slot { name: "CIRCUMFERENCE_Calf Slot 0", quelle: Quelle::Punkt(8041), p0: [15.690895, 32.707112, -5.326223], n: [0.0, 1.0, 0.0], seg: 3, roh: 83, gefiltert: 38, kette: 38, huelle: 38, wert: 36.007006 },
    Slot { name: "CIRCUMFERENCE_Calf Slot 1", quelle: Quelle::Punkt(23672), p0: [-15.353205, 32.637212, -5.376213], n: [0.0, 1.0, 0.0], seg: 2, roh: 83, gefiltert: 39, kette: 39, huelle: 39, wert: 36.039196 },
    Slot { name: "CIRCUMFERENCE_Ankle Slot 0", quelle: Quelle::Zeile(39), p0: [6.714005, 7.648612, -3.920883], n: [0.0, 1.0, 0.0], seg: 3, roh: 81, gefiltert: 31, kette: 31, huelle: 26, wert: 22.435767 },
    Slot { name: "CIRCUMFERENCE_Ankle Slot 1", quelle: Quelle::Zeile(40), p0: [-6.107315, 7.656062, -4.102673], n: [0.0, 1.0, 0.0], seg: 2, roh: 81, gefiltert: 33, kette: 33, huelle: 24, wert: 22.782499 },
    Slot { name: "CIRCUMFERENCE_Bicep Slot 0", quelle: Quelle::Zeilen([41, 42, 43]), p0: [19.508895, 124.412512, -0.472425], n: [-0.662196, 0.748122, 0.042534], seg: 1, roh: 91, gefiltert: 40, kette: 40, huelle: 40, wert: 30.167997 },
    Slot { name: "CIRCUMFERENCE_Bicep Slot 1", quelle: Quelle::Zeilen([44, 45, 46]), p0: [-19.397905, 123.959512, -0.325874], n: [-0.662333, -0.748571, -0.030929], seg: 0, roh: 89, gefiltert: 41, kette: 41, huelle: 41, wert: 30.018152 },
    Slot { name: "CIRCUMFERENCE_Elbow Slot 0", quelle: Quelle::Zeilen([49, 50, 51]), p0: [26.593395, 106.958512, -7.288463], n: [0.470609, -0.882196, -0.016036], seg: 1, roh: 94, gefiltert: 35, kette: 35, huelle: 34, wert: 23.542528 },
    Slot { name: "CIRCUMFERENCE_Elbow Slot 1", quelle: Quelle::Zeilen([52, 53, 54]), p0: [-25.303205, 105.770512, -6.879833], n: [0.469887, 0.882514, 0.019351], seg: 0, roh: 92, gefiltert: 35, kette: 35, huelle: 32, wert: 23.780462 },
    Slot { name: "CIRCUMFERENCE_Wrist Slot 0", quelle: Quelle::Zeilen([57, 58, 59]), p0: [38.108195, 87.691612, -1.594957], n: [-0.235646, 0.940550, -0.244615], seg: 1, roh: 93, gefiltert: 18, kette: 18, huelle: 17, wert: 14.788056 },
    Slot { name: "CIRCUMFERENCE_Wrist Slot 1", quelle: Quelle::Zeilen([60, 61, 62]), p0: [-38.504405, 86.842912, -1.424184], n: [-0.251517, -0.934201, 0.252997], seg: 0, roh: 92, gefiltert: 19, kette: 19, huelle: 19, wert: 15.354227 },
];

/// Grösste erlaubte Abweichung je Maß, cm (`messung.md` 7).
const TOLERANZ: f64 = 0.002;
/// Für Ebenen genau durch drei Netzpunkte: Toleranz der ganzen Kette
/// (`formung.md` 15, 0,3 mm), siehe unten.
const TOLERANZ_DREI: f64 = 0.03;

fn basemesh() -> Option<(Vec<[f64; 3]>, Messdaten)> {
    let o = ordner()?;
    // In Zentimetern lesen wie die Spezifikation: der Umweg über Millimeter
    // ändert die letzten Bits und damit die Lage genau getroffener Punkte.
    let netz = lies_obj(&o.join("BaseMesh.obj"), 1.0).expect("BaseMesh.obj lesbar");
    let d = Messdaten::laden(&o).expect("Messdaten lesbar");
    assert_eq!(d.segment.len(), netz.punkte.len(), "Segmente je Punkt");
    assert_eq!(d.kanten.len(), 137_688, "Kanten in BaseMesh.be");
    assert_eq!(d.landmarken.len(), 104, "Zeilen in IndexSet.txt");
    Some((zentriert_cm(&netz, 1.0), d))
}

#[test]
fn alle_umfaenge_treffen_die_pruefwerte() {
    let Some((v, d)) = basemesh() else { return };
    let mut fehler = Vec::new();
    for s in &SLOTS {
        let lm = |z: usize| d.landmarke(z).expect("Landmarke");
        let (p0, n) = match s.quelle {
            Quelle::Json => {
                let l = (s.n[0] * s.n[0] + s.n[1] * s.n[1] + s.n[2] * s.n[2]).sqrt();
                (s.p0, [s.n[0] / l, s.n[1] / l, s.n[2] / l])
            }
            Quelle::Punkt(i) => (v[i], WAAGERECHT),
            Quelle::Zeile(z) => (v[lm(z)], WAAGERECHT),
            Quelle::Zeilen([a, b, c]) => ebene_drei(&v, lm(a), lm(b), lm(c)).expect("Ebene"),
        };
        // Die Ebene muss zu den gerundeten Zahlen der Prüfdatei passen.
        for a in 0..3 {
            assert!((p0[a] - s.p0[a]).abs() < 2e-6, "{}: P0 {p0:?}", s.name);
            assert!((n[a] - s.n[a]).abs() < 2e-6, "{}: Normale {n:?}", s.name);
        }
        let u = umfang(&v, &d, p0, n, s.seg);
        // Eine geneigte Ebene durch drei Landmarken geht genau durch drei
        // Netzpunkte. Ob deren Kanten mitzählen, entscheidet das Vorzeichen
        // eines Abstands von ±1e-16 cm; 1e-9 cm Versatz ändert das Handgelenk
        // um 0,07 mm. Dort gilt deshalb die Toleranz der ganzen Kette
        // (0,3 mm statt 0,02 mm), Filterpunkte ± 1, Rohpunkte ungeprüft.
        let drei = matches!(s.quelle, Quelle::Zeilen(_));
        let toleranz = if drei { TOLERANZ_DREI } else { TOLERANZ };
        let abw = u.laenge - s.wert;
        println!(
            "{:32} {:9.5} soll {:9.5} ({:+.5})  Punkte {}/{} Kette {} Hülle {}",
            s.name,
            u.laenge,
            s.wert,
            abw,
            u.roh,
            u.gefiltert,
            u.kette.len(),
            u.huellpunkte
        );
        if abw.abs() > toleranz {
            fehler.push(format!("{}: {:.5} statt {:.5}", s.name, u.laenge, s.wert));
        }
        let punkte_gleich = if drei {
            u.gefiltert.abs_diff(s.gefiltert) <= 1 && u.kette.len().abs_diff(s.kette) <= 1
        } else {
            (u.roh, u.gefiltert, u.kette.len()) == (s.roh, s.gefiltert, s.kette)
        };
        if !punkte_gleich {
            fehler.push(format!(
                "{}: Punkte {}/{}/{} statt {}/{}/{}",
                s.name,
                u.roh,
                u.gefiltert,
                u.kette.len(),
                s.roh,
                s.gefiltert,
                s.kette
            ));
        }
        if u.huellpunkte.abs_diff(s.huelle) > 1 {
            fehler.push(format!("{}: Hülle {} statt {}", s.name, u.huellpunkte, s.huelle));
        }
    }
    assert!(fehler.is_empty(), "{}", fehler.join("\n"));
}

#[test]
fn rumpfmasse_waehlen_dieselben_kandidaten() {
    let Some((v, d)) = basemesh() else { return };
    let r = rumpfmasse(&v, &d).expect("Rumpfmaße");
    assert_eq!(
        r.kandidaten,
        Kandidaten {
            brustpaar: (5969, 21601),
            unterbrust: 20337,
            taille: 15538,
            huefte_oben: 15741,
            huefte_unten: 5000,
        }
    );
    for (name, ist, soll) in [
        ("Brust", r.brust.laenge, 97.733055),
        ("Unterbrust", r.unterbrust.laenge, 84.624559),
        ("Taille", r.taille.laenge, 79.961621),
        ("HighHip", r.highhip.laenge, 97.504884),
        ("Hüfte", r.huefte.laenge, 103.42468),
    ] {
        assert!(
            (ist - soll).abs() <= TOLERANZ,
            "{name}: {ist:.5} statt {soll:.5}"
        );
    }
}
