//! Shared helpers for the reference checks: small JSON reader and paths.
//!
//! `serde_json` is not in the offline cache, and the reference file contains only
//! objects, lists, numbers, strings and booleans. So a reader that can do
//! exactly that.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

/// Folder of the reference values: `KLON3D_SPEZ`, otherwise `D:\ai\rea\work\spez` — the
/// specifications and reference values live outside the repo. Sessions without
/// this folder report "übersprungen".
pub fn spez() -> PathBuf {
    std::env::var_os("KLON3D_SPEZ")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\ai\rea\work\spez"))
}

/// CLO's `Resizing` folder of the reference installation, if it has the core files.
pub fn ordner() -> Option<PathBuf> {
    let p = std::env::var_os("CLO_FEMALE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                r"C:\Program Files\CLO Standalone OnlineAuth\Preset\Avatar\Resizing\Female",
            )
        });
    ["BaseMesh.obj", "FeatureData.bd", "PCA_weight.bd"]
        .iter()
        .all(|f| p.join(f).is_file())
        .then_some(p)
}

#[derive(Debug, Clone)]
pub enum J {
    Zahl(f64),
    Text(String),
    Liste(Vec<J>),
    Objekt(HashMap<String, J>),
    Wahr(bool),
    Nichts,
}

impl J {
    pub fn zahl(&self) -> f64 {
        match self {
            J::Zahl(z) => *z,
            _ => panic!("keine Zahl: {self:?}"),
        }
    }
    pub fn liste(&self) -> &[J] {
        match self {
            J::Liste(l) => l,
            _ => panic!("keine Liste: {self:?}"),
        }
    }
    pub fn objekt(&self) -> &HashMap<String, J> {
        match self {
            J::Objekt(o) => o,
            _ => panic!("kein Objekt: {self:?}"),
        }
    }
    pub fn feld(&self, name: &str) -> &J {
        self.objekt()
            .get(name)
            .unwrap_or_else(|| panic!("Feld {name} fehlt"))
    }
    /// Number at a field.
    pub fn z(&self, name: &str) -> f64 {
        self.feld(name).zahl()
    }
    /// Three numbers as a point.
    pub fn punkt(&self) -> [f64; 3] {
        let l = self.liste();
        [l[0].zahl(), l[1].zahl(), l[2].zahl()]
    }
}

/// Reads the reference file; `None` if it is missing.
pub fn pruef() -> Option<J> {
    let t = std::fs::read_to_string(spez().join("pruefwerte_2026_1.json")).ok()?;
    Some(lies_json(&t))
}

pub fn lies_json(text: &str) -> J {
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    wert(&b, &mut i)
}

fn ueber(b: &[char], i: &mut usize) {
    while *i < b.len() && b[*i].is_whitespace() {
        *i += 1;
    }
}

fn wert(b: &[char], i: &mut usize) -> J {
    ueber(b, i);
    match b[*i] {
        '{' => {
            *i += 1;
            let mut o = HashMap::new();
            loop {
                ueber(b, i);
                if b[*i] == '}' {
                    *i += 1;
                    return J::Objekt(o);
                }
                if b[*i] == ',' {
                    *i += 1;
                    continue;
                }
                let k = text(b, i);
                ueber(b, i);
                assert_eq!(b[*i], ':', "Doppelpunkt fehlt");
                *i += 1;
                o.insert(k, wert(b, i));
            }
        }
        '[' => {
            *i += 1;
            let mut l = Vec::new();
            loop {
                ueber(b, i);
                if b[*i] == ']' {
                    *i += 1;
                    return J::Liste(l);
                }
                if b[*i] == ',' {
                    *i += 1;
                    continue;
                }
                l.push(wert(b, i));
            }
        }
        '"' => J::Text(text(b, i)),
        't' => {
            *i += 4;
            J::Wahr(true)
        }
        'f' => {
            *i += 5;
            J::Wahr(false)
        }
        'n' => {
            *i += 4;
            J::Nichts
        }
        _ => {
            let a = *i;
            while *i < b.len()
                && (b[*i] == '-'
                    || b[*i] == '.'
                    || b[*i] == 'e'
                    || b[*i] == 'E'
                    || b[*i].is_ascii_digit())
            {
                *i += 1;
            }
            let s: String = b[a..*i].iter().collect();
            J::Zahl(s.parse().unwrap_or_else(|_| panic!("keine Zahl: {s:?}")))
        }
    }
}

fn text(b: &[char], i: &mut usize) -> String {
    assert_eq!(b[*i], '"');
    *i += 1;
    let mut s = String::new();
    while b[*i] != '"' {
        if b[*i] == '\\' {
            *i += 1;
        }
        s.push(b[*i]);
        *i += 1;
    }
    *i += 1;
    s
}

/// Step lists of the klon3d configuration: (step 1 = BASE, step 0 = DETAIL).
pub fn listen(p: &J) -> (Vec<usize>, Vec<usize>) {
    let l = p.feld("konfiguration_klon3d").feld("listen");
    let lies = |name: &str| -> Vec<usize> {
        l.feld(name)
            .liste()
            .iter()
            .map(|j| j.zahl() as usize)
            .collect()
    };
    (lies("1"), lies("0"))
}

/// Codes per measure (67).
pub fn codes(p: &J) -> Vec<u8> {
    p.feld("konfiguration_klon3d")
        .feld("codes")
        .liste()
        .iter()
        .map(|c| c.zahl() as u8)
        .collect()
}
