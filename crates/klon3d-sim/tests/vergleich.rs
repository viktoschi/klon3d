//! Zwei Netze gegeneinander messen (Stufe 5).
//!
//! Der Vergleich läuft über die **Fläche**, nicht über gleiche Punktnummern.
//! Die Tests prüfen beides: die Flächenrechnung gegen von Hand gerechnete
//! Abstände und den Fall, in dem Punktnummern absichtlich nichts miteinander
//! zu tun haben.

use klon3d_sim::vergleich::{Kennzahlen, Netz, Vergleich};

/// Ein Quadrat in der z = 0-Ebene, Kantenlänge 100 mm, zwei Dreiecke.
fn quadrat() -> Netz {
    Netz::lies_obj(
        "v 0 0 0\nv 100 0 0\nv 100 100 0\nv 0 100 0\n\
         f 1 2 3\nf 1 3 4\n",
    )
    .expect("Quadrat lesbar")
}

#[test]
fn obj_liest_punkte_dreiecke_und_gruppen() {
    let text = "o erstes\nv 0 0 0\nv 10 0 0\nv 0 10 0\nf 1 2 3\n\
                g zweites\nv 20 0 0\nv 30 0 0\nv 20 10 0\nf 4 5 6\n";
    let n = Netz::lies_obj(text).expect("lesbar");
    assert_eq!(n.punkte.len(), 6);
    assert_eq!(n.dreiecke.len(), 2);
    assert_eq!(n.namen, vec!["erstes".to_string(), "zweites".to_string()]);
    assert_eq!(n.gruppe, vec![0, 1]);
    assert_eq!(n.dreiecke[1], [3, 4, 5]);
}

#[test]
fn obj_trianguliert_polygone_als_faecher_und_kann_negative_indizes() {
    let text = "v 0 0 0\nv 10 0 0\nv 10 10 0\nv 0 10 0\nf -4 -3 -2 -1\n";
    let n = Netz::lies_obj(text).expect("lesbar");
    // Vier Ecken ergeben zwei Dreiecke, der Fächer beginnt bei der ersten.
    assert_eq!(n.dreiecke, vec![[0, 1, 2], [0, 2, 3]]);
}

#[test]
fn obj_meldet_krumme_zeilen() {
    assert!(Netz::lies_obj("v 0 0\n").is_err());
    assert!(Netz::lies_obj("v 0 0 0\nf 1 1 9\n").is_err());
    assert!(Netz::lies_obj("v 0 0 0\nv 1 0 0\nf 1 2\n").is_err());
}

/// Gleiches Netz gegen sich selbst: in beiden Richtungen exakt 0.
#[test]
fn gleiches_netz_hat_abstand_null() {
    let q = quadrat();
    let v = Vergleich::rechne(&q, &q, 20.0).expect("rechenbar");
    assert!(v.a_zu_b.iter().all(|d| *d == 0.0), "Hinweg nicht 0");
    assert!(v.b_zu_a.iter().all(|d| *d == 0.0), "Rückweg nicht 0");
    assert_eq!(v.hin().max, 0.0);
}

/// Ein Punkt 30 mm über der Fläche hat 30 mm Abstand - und zwar auch dann,
/// wenn er im anderen Netz die Punktnummer 0 trägt und dort auf der Fläche
/// liegt. Genau diesen Fall verwechselt ein Vergleich über Punktnummern.
#[test]
fn abstand_geht_ueber_die_flaeche_nicht_ueber_den_index() {
    let q = quadrat();
    let mut hoch = q.clone();
    hoch.punkte[0] = [10.0, 10.0, 30.0];
    let v = Vergleich::rechne(&hoch, &q, 20.0).expect("rechenbar");
    assert!((v.a_zu_b[0] - 30.0).abs() < 1e-9, "Knoten 0: {}", v.a_zu_b[0]);
    // Die Nachbarn liegen weiterhin auf der Fläche.
    assert_eq!(v.a_zu_b[1], 0.0);
    assert_eq!(v.a_zu_b[2], 0.0);
}

/// Punkt genau auf einer Kante: der Abstand ist der senkrechte Abstand zur
/// Dreiecksfläche, nicht der zur nächsten Ecke (Ericson gegen Handrechnung).
#[test]
fn punkt_ueber_einer_kante_misst_senkrecht() {
    let mut q = quadrat();
    // Zusätzlicher Knoten nicht nötig: ein Netz aus einem Dreieck genügt,
    // der Punkt liegt über der Mitte der Kante 1-2.
    let mut eins = quadrat();
    eins.dreiecke.truncate(1);
    eins.punkte.truncate(3);
    eins.gruppe.truncate(1);
    q.punkte.push([50.0, 0.0, 12.0]);
    let v = Vergleich::rechne(&q, &eins, 20.0).expect("rechenbar");
    assert!((v.a_zu_b[4] - 12.0).abs() < 1e-9, "Kantenmitte: {}", v.a_zu_b[4]);
}

/// Zwei Flächen im Abstand 5 mm parallel: jeder Knoten hat 5 mm. Die
/// Gegenrichtung ist gleich, weil beide gleich dicht sind.
#[test]
fn parallele_flaechen_haben_ueberall_den_gleichen_abstand() {
    let mut b = quadrat();
    for p in &mut b.punkte {
        p[2] = 5.0;
    }
    let a = quadrat();
    let v = Vergleich::rechne(&a, &b, 20.0).expect("rechenbar");
    for d in v.a_zu_b.iter().chain(v.b_zu_a.iter()) {
        assert!((d - 5.0).abs() < 1e-9, "Abstand {d} statt 5");
    }
}

/// Große Abstände: ein Knoten 500 mm weg, bei 20 mm Zelle - die Suche muss
/// den Radius über die Zellgrenzen hinaus wachsen lassen und trotzdem den
/// richtigen Wert liefern.
#[test]
fn weiter_punkt_ueber_lebt_die_suche_aus() {
    let q = quadrat();
    let mut weit = q.clone();
    weit.punkte[0] = [50.0, 50.0, 500.0];
    let v = Vergleich::rechne(&weit, &q, 20.0).expect("rechenbar");
    assert!((v.a_zu_b[0] - 500.0).abs() < 1e-9, "weiter Punkt: {}", v.a_zu_b[0]);
}

/// Der Eichwert für Stufe 5 aus Befund 118: Punktabstand über den
/// Stoffwechsel. Hier als Form der Kennzahlen - Median liegt unter dem
/// Mittel, weil wenige Knoten weit wandern. Median bleibt Median.
#[test]
fn kennzahlen_setzen_median_gegen_ausreisser() {
    // 999 ruhige Knoten, einer wandert weit: der Median bleibt beim
    // ruhigen Wert, das Mittel wird gezogen, erst das 99 %-Perzentil
    // verrät den Ausreißer nicht mehr (1 % von 1000 sind 10 Knoten).
    let mut d = vec![1.0; 999];
    d.push(1000.0);
    let k = Kennzahlen::aus(d);
    assert_eq!(k.anzahl, 1000);
    assert_eq!(k.median, 1.0);
    assert_eq!(k.max, 1000.0);
    assert!((k.mittel - 1.999).abs() < 1e-9, "Mittel {}", k.mittel);
    assert_eq!(k.p90, 1.0);
    assert_eq!(k.p99, 1.0);
}

/// Gruppen: der Median wird je Gruppenname gebildet, und zwar nur für
/// Gruppen, die es in beiden Netzen gibt.
#[test]
fn gruppen_median_je_teil() {
    let a = Netz::lies_obj(
        "o oben\nv 0 0 0\nv 10 0 0\nv 0 10 0\nf 1 2 3\n\
         o unten\nv 0 0 100\nv 10 0 100\nv 0 10 100\nf 4 5 6\n",
    )
    .expect("a");
    let b = Netz::lies_obj("o oben\nv 0 0 0\nv 10 0 0\nv 0 10 0\nf 1 2 3\n").expect("b");
    let v = Vergleich::rechne(&a, &b, 20.0).expect("rechenbar");
    let g = v.je_gruppe(&a, &b);
    assert_eq!(g.len(), 1, "nur die Gruppe, die es in b gibt");
    assert_eq!(g[0].0, "oben");
    assert_eq!(g[0].2.max, 0.0);
}

#[test]
fn leeres_netz_ist_ein_fehler() {
    let q = quadrat();
    let leer = Netz::default();
    assert!(Vergleich::rechne(&q, &leer, 20.0).is_err());
    assert!(Vergleich::rechne(&leer, &q, 20.0).is_err());
}
