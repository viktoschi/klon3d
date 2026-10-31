//! `Anordnung` für den Körper aus CLOs `.avt` — Ende zu Ende.
//!
//! Geprüft wird nicht die Lesefunktion allein, sondern die ganze Kette:
//! `.zprj` → `body` (31 422 Knoten) → `range_list.rlist` + `IndexSet.txt` +
//! `symmetry_data.sym` → [`Anordnung`]. Die Prüfungen sind **Nummernaussagen**
//! aus CLOs eigenen Dateien, nicht aus Koordinaten — sie gelten deshalb auch
//! für anders posierte Avatare (das ist der Punkt von Befund 120).
//!
//! Fehlt die Prüfdatei oder CLOs Ordner, wird übersprungen.

use klon3d_koerper::anordnung_clo::{
    aus_clo_koerper, CloNummern, L_ELLBOGEN, L_HALS_HINTEN, L_HALS_SEITLICH, L_HALS_VORN,
    L_SCHULTER, SYMMETRIE_PUNKTE,
};
use klon3d_koerper::Netz;
use klon3d_pattern::zprj::Projekt;
use std::path::PathBuf;

const PRUEFPROJEKT: &str =
    r"D:\ai\rea\work\messung\japanese_school_uniform\japanese_school_uniform.zprj";
const FREMD: &str = r"D:\ai\rea\downloads\clo3d-patterns";

fn pruefprojekt() -> Option<PathBuf> {
    let p = PathBuf::from(PRUEFPROJEKT);
    p.is_file().then_some(p)
}

/// CLOs `Resizing`-Ordner: `CLO_FEMALE`, sonst die 2026er Installation, sonst
/// die 2020er.
///
/// Die Reihenfolge ist keine Bequemlichkeit: die **2020er** `IndexSet.txt`
/// nennt für L52 den Punkt 27 644, der **nicht** der Spiegelpartner von L49
/// ist (siehe `elf_und_zwanzig_hat_eine_andere_l52`). Nur die 2026er Fassung
/// ist dort richtig.
fn ordner() -> Option<PathBuf> {
    let kandidaten = [
        std::env::var_os("CLO_FEMALE").map(PathBuf::from),
        Some(PathBuf::from(
            r"C:\Program Files\CLO Standalone OnlineAuth\Preset\Avatar\Resizing\Female",
        )),
        Some(PathBuf::from(
            r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female",
        )),
    ];
    kandidaten.into_iter().flatten().find(|p| {
        ["BaseMesh.obj", "range_list.rlist", "IndexSet.txt"]
            .iter()
            .all(|f| p.join(f).is_file())
    })
}

fn ordner_2020() -> PathBuf {
    PathBuf::from(r"C:\Program Files\CLO Standalone\Preset\Avatar\Resizing\Female")
}

fn ordner_2026() -> PathBuf {
    PathBuf::from(r"C:\Program Files\CLO Standalone OnlineAuth\Preset\Avatar\Resizing\Female")
}

/// Der `body` aus einer `.zprj`.
fn body_aus(p: &std::path::Path) -> Option<Netz> {
    let projekt = Projekt::lies(p).ok()?;
    let k = projekt.koerper?;
    Some(Netz {
        punkte: k.punkte.iter().map(|q| [q.x, q.y, q.z]).collect(),
        dreiecke: k
            .dreiecke
            .iter()
            .map(|d| [d[0] as usize, d[1] as usize, d[2] as usize])
            .collect(),
    })
}

fn alle_fremden() -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "zprj") {
                out.push(p);
            }
        }
    }
    // `KLON3D_FREMD` wie in `klon3d-pattern/tests/zprj_netz.rs`.
    let ordner = std::env::var_os("KLON3D_FREMD").map_or_else(|| PathBuf::from(FREMD), PathBuf::from);
    walk(&ordner, &mut out);
    out.sort();
    out
}

#[test]
fn anordnung_aus_clo_koerper_und_clo_ordner() {
    let (Some(p), Some(ordner)) = (pruefprojekt(), ordner()) else {
        eprintln!("übersprungen: Prüfdatei oder CLO-Ordner fehlt");
        return;
    };
    let netz = body_aus(&p).expect("Körper aus der .avt");
    let nummern = CloNummern::aus_ordner(&ordner).expect("CLO-Listen");
    assert_eq!(nummern.segment.len(), 47_650, "range_list.rlist: ein Wert je BaseMesh-Punkt");
    assert!(nummern.landmarken.len() >= 53, "IndexSet.txt: mindestens L52");
    assert_eq!(nummern.spiegel.len(), SYMMETRIE_PUNKTE, "symmetry_data.sym");
    assert_eq!(
        nummern.spiegel.last().copied(),
        Some(39_635),
        "letzte Zahl der Datei (die Haarpunkte bekommen sie)"
    );

    let an = aus_clo_koerper(&netz, &nummern).expect("Anordnung aus dem .avt-Körper");

    // ── CLOs eigene Spiegelzuordnung: die Paare sind es wirklich ──
    for (zeile_l, zeile_r, name) in [
        (L_HALS_SEITLICH[0], L_HALS_SEITLICH[1], "Hals seitlich"),
        (L_SCHULTER[0], L_SCHULTER[1], "Schulter"),
        (L_ELLBOGEN[0], L_ELLBOGEN[1], "Ellbogen"),
    ] {
        let l = nummern.landmarke(zeile_l).expect("linke Landmarke");
        let r = nummern.landmarke(zeile_r).expect("rechte Landmarke");
        assert_eq!(
            nummern.spiegelpartner(l),
            Some(r as i64),
            "{name}: {l} muss laut symmetry_data.sym den Partner {r} haben"
        );
        assert_eq!(
            nummern.spiegelpartner(r),
            Some(l as i64),
            "{name}: die Zuordnung muss gegenseitig sein"
        );
    }

    // ── Drosselgrube und 7. Halswirbel liegen auf der Mittellinie ──
    for (zeile, name) in [(L_HALS_VORN, "Drosselgrube"), (L_HALS_HINTEN, "7. Halswirbel")] {
        let i = nummern.landmarke(zeile).expect("Landmarke");
        assert_eq!(
            nummern.spiegelpartner(i),
            Some(i as i64),
            "{name} ({i}) muss sein eigener Spiegelpartner sein"
        );
    }

    // ── Die Landmarken liegen in den Segmenten, die ihre Namen nennen ──
    for (zeile, name, soll) in [
        (L_HALS_VORN, "Drosselgrube", 6),
        (L_HALS_HINTEN, "7. Halswirbel", 6),
        (L_HALS_SEITLICH[0], "Hals seitlich links", 6),
        (L_HALS_SEITLICH[1], "Hals seitlich rechts", 6),
        (L_SCHULTER[0], "Schulterpunkt links", 1),
        (L_SCHULTER[1], "Schulterpunkt rechts", 0),
        (L_ELLBOGEN[0], "Ellbogen links", 1),
        (L_ELLBOGEN[1], "Ellbogen rechts", 0),
    ] {
        let i = nummern.landmarke(zeile).expect("Landmarke ist ein Punkt");
        assert_eq!(
            nummern.segment[i], soll,
            "{name} (Punkt {i}) liegt in Segment {} statt in {soll}",
            nummern.segment[i]
        );
    }

    // ── In der Geometrie: das Kinn ist über der Drosselgrube, der Nacken
    //    hinter ihr. Das ist die einzige Aussage, die auch bei einer anderen
    //    Pose gilt (vorn/hinten dreht sich nicht mit den Armen). ──
    assert!(
        an.hals_hinten[2] < an.hals_vorn[2],
        "Halswirbel z = {:.1} muss hinter der Drosselgrube z = {:.1} liegen",
        an.hals_hinten[2],
        an.hals_vorn[2]
    );
    assert!(
        an.hals_vorn[1] > an.schulter[0][1].min(an.schulter[1][1]),
        "Drosselgrube ({:.1}) muss über der tieferen Schulter ({:.1}) liegen",
        an.hals_vorn[1],
        an.schulter[0][1].min(an.schulter[1][1])
    );

    // ── Die Punktmengen ──
    assert_eq!(an.rumpf.len(), 5_090, "Rumpf (CLO segment 4)");
    let mut groessen = [an.arme[0].len(), an.arme[1].len()];
    groessen.sort();
    assert_eq!(groessen, [4_988, 4_990], "Armpunkte");
    assert!(
        an.rumpf.iter().all(|i| !an.arme[0].contains(i) && !an.arme[1].contains(i)),
        "Rumpfpunkte dürfen nicht im Arm liegen"
    );
    assert!(
        an.arme[0].iter().all(|i| !an.arme[1].contains(i)),
        "die Arme dürfen sich nicht überschneiden"
    );
}

/// Punktzahlen je Segment, wie CLO sie für den BaseMesh führt — für jeden
/// `body` dieselben, weil der BaseMesh projektunabhängig ist.
#[test]
fn segmente_haben_die_clo_groessen() {
    let (Some(p), Some(ordner)) = (pruefprojekt(), ordner()) else {
        eprintln!("übersprungen");
        return;
    };
    let netz = body_aus(&p).expect("Körper");
    let nummern = CloNummern::aus_ordner(&ordner).expect("CLO-Listen");
    let an = aus_clo_koerper(&netz, &nummern).expect("Anordnung");
    assert_eq!(an.rumpf.len(), 5_090, "Rumpf (CLO segment 4)");
    let mut groessen = [an.arme[0].len(), an.arme[1].len()];
    groessen.sort();
    assert_eq!(groessen, [4_988, 4_990], "Armpunkte");
}

/// **Der Test gegen „zurechtgebogen":** dieselben CLO-Listen müssen für
/// **jedes** fremde Projekt durchgehen.
///
/// Das ist der Grund, warum die Prüfung topologisch und nicht geometrisch
/// ist. Vier fremde Projekte (`catsuit_v5_base`, `miko`, `miko_alt`,
/// `zero_two_base`) haben **asymmetrische Posen** (Arme oben); eine
/// geometrische Symmetrieprüfung fällt dort durch, obwohl die Zuordnung
/// stimmt. `symmetry_data.sym` und `range_list.rlist` sind Aussagen über
/// Nummern und gelten für alle 40.
#[test]
fn anordnung_gilt_fuer_alle_fremden_projekte() {
    let Some(ordner) = ordner() else {
        eprintln!("übersprungen: CLO-Ordner fehlt");
        return;
    };
    let nummern = CloNummern::aus_ordner(&ordner).expect("CLO-Listen");
    let projekte = alle_fremden();
    if projekte.len() < 40 {
        eprintln!("übersprungen: nur {} fremde Projekte", projekte.len());
        return;
    }

    let (mut mit, mut ohne, mut fehler) = (0, 0, Vec::new());
    for p in &projekte {
        let Some(netz) = body_aus(p) else {
            ohne += 1;
            continue;
        };
        mit += 1;
        if let Err(e) = aus_clo_koerper(&netz, &nummern) {
            fehler.push(format!("{}: {e}", p.display()));
        }
    }

    assert!(
        fehler.is_empty(),
        "{} von {mit} Projekten scheiterten an der Anordnung:\n{}",
        fehler.len(),
        fehler.join("\n")
    );
    assert!(mit >= 40, "nur {mit} Projekte mit Körper (erwartet 40)");
    assert_eq!(mit + ohne, projekte.len(), "jedes Projekt ist eins von beidem");
    eprintln!("{mit} Projekte mit Körper bestanden, {ohne} ohne");
}

/// Die **2020er** `IndexSet.txt` nennt für L52 einen anderen Punkt als die
/// 2026er: 27 644 statt 27 631. `symmetry_data.sym` sagt eindeutig, welcher
/// der Spiegelpartner von L49 ist — die Prüfung muss die 2020er abweisen.
#[test]
fn elf_und_zwanzig_hat_eine_andere_l52() {
    let Some(p) = pruefprojekt() else {
        eprintln!("übersprungen: Prüfdatei fehlt");
        return;
    };
    let Some(netz) = body_aus(&p) else {
        eprintln!("übersprungen: kein Körper");
        return;
    };
    let (alt, neu) = (ordner_2020(), ordner_2026());
    if !alt.join("IndexSet.txt").is_file() || !neu.join("IndexSet.txt").is_file() {
        eprintln!("übersprungen: beide Installationen nötig");
        return;
    }

    let n20 = CloNummern::aus_ordner(&alt).expect("2020er Listen");
    let n26 = CloNummern::aus_ordner(&neu).expect("2026er Listen");
    let l52_20 = n20.landmarke(L_ELLBOGEN[1]).expect("L52 2020");
    let l52_26 = n26.landmarke(L_ELLBOGEN[1]).expect("L52 2026");
    assert_ne!(l52_20, l52_26, "die beiden Stände nennen verschiedene Punkte");

    // Die Wahrheit steht in symmetry_data.sym, nicht in IndexSet.txt.
    let l49 = n26.landmarke(L_ELLBOGEN[0]).expect("L49");
    assert_eq!(
        n26.spiegelpartner(l49),
        Some(l52_26 as i64),
        "der Spiegelpartner von L49 ({l49}) ist {l52_26}"
    );
    assert_ne!(
        n26.spiegelpartner(l49),
        Some(l52_20 as i64),
        "die 2020er Nummer ({l52_20}) ist NICHT der Spiegelpartner von L49"
    );

    // 2026 geht durch, 2020 nicht.
    assert!(aus_clo_koerper(&netz, &n26).is_ok(), "2026er Zuordnung muss durchgehen");
    let r = aus_clo_koerper(&netz, &n20);
    assert!(r.is_err(), "2020er L52 ({l52_20}) muss abgewiesen werden");
    let m = r.unwrap_err();
    assert!(m.contains("Ellbogen"), "Meldung nennt das Paar: {m}");
    assert!(m.contains("symmetry_data.sym"), "Meldung nennt die Quelle: {m}");
}
