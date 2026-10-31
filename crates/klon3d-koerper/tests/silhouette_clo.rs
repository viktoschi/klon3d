//! Silhouette of the shaped body: does the shaping introduce self-intersections
//! or dents? (stage 2, finding 24).
//!
//! The absolute numbers are not the criterion. The base mesh that the shaping
//! starts from already carries contacts: its seams are open, it repeats the
//! seam vertices, and it has genuinely thin triangles at the crotch and the
//! feet. A check that demanded "no contact at all" would fail on the untouched
//! body and say nothing about the shaping.
//!
//! So the test compares **before and after**, triangle pair by triangle pair
//! (the topology is identical) and reads the **penetration width** of a crossing
//! rather than its bare existence. What the shaping must not do:
//!
//! | Check | Rule |
//! |---|---|
//! | topology | identical triangle list before and after, otherwise the comparison is meaningless |
//! | crossings | no new pair that penetrates wider than the widest crossing already present |
//! | overlaps | none at all — coplanar superimposed faces are an artefact, not a body |
//! | dents | the deepest dent may not grow by more than the dent threshold of the check itself |
//! | ring | the median ring radius stays in the order of the base mesh |
//!
//! Measured on the reference body (`measurements.smis`), the shaping
//! improves the mesh: 464 → 54 crossings, 114 → 82 dents, widest crossing
//! 1.46 → 1.18 mm. The rules above therefore hold with room to spare.
//!
//! Without the reference folder or without the measurement file, the test is
//! skipped.

mod common;

use common::json_mini::ordner;
use klon3d_koerper::koerper::aus_smis;
use klon3d_koerper::silhouette::{neue_kontakte, pruefe, Art, Parameter};
use klon3d_koerper::{lies_obj, Netz};
use std::path::PathBuf;

/// The base mesh's `body` group: points `0 … KOERPER-1`, triangles inside them.
///
/// The file also carries eyes, lashes and teeth as separate groups; those sit
/// inside the head and would be reported as intersections.
fn grundkoerper(o: &std::path::Path) -> Option<Netz> {
    let ganz = lies_obj(&o.join("BaseMesh.obj"), 10.0).ok()?;
    let k = klon3d_koerper::kette_referenz::KOERPER;
    if ganz.punkte.len() < k {
        return None;
    }
    Some(Netz {
        punkte: ganz.punkte[..k].to_vec(),
        dreiecke: ganz
            .dreiecke
            .iter()
            .copied()
            .filter(|d| d.iter().all(|&i| i < k))
            .collect(),
    })
}

/// The reference sample's measurement file (the user's values).
fn smis() -> Option<klon3d_pattern::Smis> {
    let p = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../klon3d-pattern/tests/muster/measurements.smis"
    ));
    klon3d_pattern::lies_smis(&p).ok()
}

fn breiteste(b: &klon3d_koerper::silhouette::Befund, art: Art) -> f64 {
    b.kontakte
        .iter()
        .filter(|c| c.art == art)
        .map(|c| c.breite_mm)
        .fold(0.0, f64::max)
}

#[test]
fn shaping_does_not_add_self_intersections() {
    let (Some(o), Some(smis)) = (ordner(), smis()) else {
        eprintln!("übersprungen: Referenzordner oder Maßdatei fehlt");
        return;
    };
    let Some(grund) = grundkoerper(&o) else {
        eprintln!("übersprungen: BaseMesh.obj ohne body-Gruppe");
        return;
    };
    let koerper = aus_smis(&o, &|n| smis.wert(n)).expect("Körper aus der Maßdatei");
    let p = Parameter::default();

    let vorher = pruefe(&grund, &p);
    let nachher = pruefe(&koerper.netz, &p);
    println!("Grund:   {}", vorher.als_text());
    println!("Geformt: {}", nachher.als_text());

    // --- Topology: without it the pair comparison says nothing ------------
    assert_eq!(
        grund.dreiecke, koerper.netz.dreiecke,
        "Topologie geändert, Vergleich unmöglich"
    );
    assert_eq!(grund.punkte.len(), koerper.netz.punkte.len(), "Punktzahl");

    // --- Overlaps: coplanar superimposed faces, never legitimate ----------
    assert_eq!(
        nachher.zahl(Art::Ueberdeckung),
        0,
        "Überdeckungen: {}",
        nachher.als_text()
    );

    // --- Crossings: no new one wider than the widest already there --------
    let grenze = breiteste(&vorher, Art::Kreuzung);
    let (neu, delle) = neue_kontakte(&vorher, &nachher, grenze);
    let neue_kreuzung: Vec<_> = neu.iter().filter(|k| k.art == Art::Kreuzung).collect();
    assert!(
        neue_kreuzung.is_empty(),
        "{} neue Kreuzungen über {grenze:.2} mm: {:?}\n{}",
        neue_kreuzung.len(),
        neue_kreuzung
            .iter()
            .map(|k| (k.a, k.b, (k.breite_mm * 1000.0).round() / 1000.0))
            .collect::<Vec<_>>(),
        nachher.als_text()
    );

    // --- Dents: the deepest may not grow by more than the threshold -------
    assert!(
        delle.is_none(),
        "Delle gewachsen: {delle:?}, Schwelle {:.2} mm",
        p.delle_mm
    );

    // --- Ring: the shaping may not coarsen or refine the mesh -------------
    assert!(
        nachher.ring_median_mm > 0.5 * vorher.ring_median_mm
            && nachher.ring_median_mm < 2.0 * vorher.ring_median_mm,
        "Ringradius {:.2} mm gegen {:.2} mm im Grundnetz",
        nachher.ring_median_mm,
        vorher.ring_median_mm
    );

    // The check has to have seen something, otherwise it proves nothing.
    assert!(vorher.paare > 0 && nachher.paare > 0, "keine Paare geprüft");
    assert!(
        nachher.ohne_ring == 0,
        "{} Punkte ohne Ring",
        nachher.ohne_ring
    );
    assert!(
        nachher.am_rand > 0,
        "kein Punkt am offenen Rand erkannt — Nahtprüfung greift nicht"
    );
}
