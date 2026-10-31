//! Nahtaufteilung an zwei Rechtecken: Kerben 1 mm versetzt und ein
//! Punktknoten nur auf einer Seite.
//!
//! A: (0,0)–K(40,0)–(100,0)–(100,50)–(0,50), Kerbe K bei 40 mm.
//! B: (0,100)–L(41,100)–M(70,100)–(100,100)–(100,150)–(0,150).
//! Naht N1: Unterkante A ↔ Unterkante B, beide 100 mm.

use std::collections::HashMap;

use klon3d_mesh::{pruefung, vernetze, Musternetz, Parameter, Stellenart};
use klon3d_pattern::geom::Auswertung;
use klon3d_pattern::kanten::Kurvenquelle;
use klon3d_pattern::naehte::{lies_nahtliste, pruefe_naehte};
use klon3d_pattern::read_str;

fn muster() -> String {
    let punkte = [
        (1, "A1", 0.0, 0.0),
        (2, "K", 4.0, 0.0),
        (3, "A2", 10.0, 0.0),
        (4, "A3", 10.0, 5.0),
        (5, "A4", 0.0, 5.0),
        (6, "B1", 0.0, 10.0),
        (7, "L", 4.1, 10.0),
        (8, "M", 7.0, 10.0),
        (9, "B2", 10.0, 10.0),
        (10, "B3", 10.0, 15.0),
        (11, "B4", 0.0, 15.0),
    ];
    let calc: String = punkte
        .iter()
        .map(|(id, n, x, y)| format!(r#"<point id="{id}" name="{n}" type="single" x="{x}" y="{y}" mx="0" my="0"/>"#))
        .collect();
    let model: String =
        punkte.iter().map(|(id, ..)| format!(r#"<point id="{}" idObject="{id}" type="modeling"/>"#, id + 20)).collect();
    let knoten = |ids: &[u32]| -> String {
        ids.iter().map(|id| format!(r#"<node idObject="{}" type="NodePoint"/>"#, id + 20)).collect()
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pattern><version>0.7.4</version><unit>cm</unit>
  <draftBlock name="A">
    <calculation>{calc}</calculation>
    <modeling>{model}</modeling>
    <pieces>
      <piece id="50" name="Teil A" seamAllowance="true" width="1"><nodes>{}</nodes></piece>
      <piece id="51" name="Teil B" seamAllowance="true" width="1"><nodes>{}</nodes></piece>
    </pieces>
  </draftBlock>
</pattern>"#,
        knoten(&[1, 2, 3, 4, 5]),
        knoten(&[6, 7, 8, 9, 10, 11])
    )
}

fn netz(p: &Parameter) -> Musternetz {
    let xml = muster();
    let m = read_str(&xml).unwrap();
    let ev = Auswertung::new(&m, HashMap::new());
    let defs = lies_nahtliste("N1 | Test | Teil A: A1 A2 v | Teil B: B1 B2 v").unwrap();
    let b = pruefe_naehte(&ev, &defs, Kurvenquelle::Seamly).unwrap();
    vernetze(&ev, Kurvenquelle::Seamly, &b.naehte, p).unwrap()
}

#[test]
fn kerben_zusammengelegt_und_eigener_punkt() {
    let netz = netz(&Parameter::default());
    let b = pruefung::pruefe(&netz);
    assert!(b.verstoesse(&netz.parameter).is_empty(), "{:?}", b.verstoesse(&netz.parameter));
    let n = &netz.naehte[0];
    let arten: Vec<(Stellenart, &str, &str)> = n.stellen.iter().map(|s| (s.art, s.name_a.as_str(), s.name_b.as_str())).collect();
    assert_eq!(
        arten,
        [
            (Stellenart::Ende, "A1", "B1"),
            (Stellenart::Gemeinsam, "K", "L"),
            (Stellenart::NurB, "", "M"),
            (Stellenart::Ende, "A2", "B2"),
        ]
    );
    // Versatz der zusammengelegten Kerbe: 1 mm.
    assert!((n.stellen[1].versatz_mm - (-1.0)).abs() < 1e-9, "{}", n.stellen[1].versatz_mm);
    // M (70 mm) liegt zwischen den Ankern (40|41) und (100|100): A bei 40 + 29/59·60.
    let soll = 40.0 + 29.0 / 59.0 * 60.0;
    assert!((n.stellen[2].rho_a * 100.0 - soll).abs() < 1e-9);

    let (ta, tb) = (&netz.teile[0], &netz.teile[1]);
    assert_eq!((ta.teil.as_str(), tb.teil.as_str()), ("Teil A", "Teil B"));
    assert_eq!(n.teilnaehte.len(), 1);
    let paare = &n.teilnaehte[0].paare;
    // Abschnitte 40|41 mm (5 Stücke), 29,49|29 mm (3), 30,51|30 mm (4): 13 Paare.
    assert_eq!(paare.len(), 13);
    let pk = |t: &klon3d_mesh::Teilnetz, name: &str| t.punktknoten.iter().find(|p| p.0 == name).unwrap().1;
    assert!(paare.iter().any(|q| q.a == pk(ta, "K") && q.b == pk(tb, "L")));
    let m = paare.iter().find(|q| q.b == pk(tb, "M")).unwrap();
    let a = ta.punkte[m.a as usize];
    assert!((a[0] - soll).abs() < 1e-6 && a[1].abs() < 1e-9, "{a:?}");
    // Freie Kanten bleiben bei höchstens 10 mm, gerade Kanten ohne Sehnenfehler.
    let tp = &b.teile[0];
    assert!(tp.sehne_max < 1e-9 && tp.laenge_max < 1e-9);
}

#[test]
fn enger_fang_gibt_beiden_seiten_eigene_punkte() {
    let p = Parameter { fang_mm: 0.5, ..Parameter::default() };
    let netz = netz(&p);
    let n = &netz.naehte[0];
    let arten: Vec<Stellenart> = n.stellen.iter().map(|s| s.art).collect();
    assert_eq!(arten, [Stellenart::Ende, Stellenart::NurA, Stellenart::NurB, Stellenart::NurB, Stellenart::Ende]);
    // Ohne Anker gilt CLOs reine Anteilsregel: K bei 40 mm auch auf B.
    assert!((n.stellen[1].rho_b - 0.4).abs() < 1e-12 && n.stellen[1].versatz_mm.abs() < 1e-9);
    let b = pruefung::pruefe(&netz);
    assert!(b.verstoesse(&p).is_empty(), "{:?}", b.verstoesse(&p));
    // K bekommt auf B einen eigenen Punkt bei 40 mm, L auf A einen bei 41 mm.
    let (ta, tb) = (&netz.teile[0], &netz.teile[1]);
    let pk = |t: &klon3d_mesh::Teilnetz, name: &str| t.punktknoten.iter().find(|p| p.0 == name).unwrap().1;
    let paare = &n.teilnaehte[0].paare;
    let zu_k = paare.iter().find(|q| q.a == pk(ta, "K")).unwrap();
    let zu_l = paare.iter().find(|q| q.b == pk(tb, "L")).unwrap();
    let (gb, ga) = (tb.punkte[zu_k.b as usize], ta.punkte[zu_l.a as usize]);
    assert!((gb[0] - 40.0).abs() < 1e-9 && (gb[1] - 100.0).abs() < 1e-9, "{gb:?}");
    assert!((ga[0] - 41.0).abs() < 1e-9 && ga[1].abs() < 1e-9, "{ga:?}");
    assert!(b.naehte[0].stueck_min <= 1.0 + 1e-9);
}
