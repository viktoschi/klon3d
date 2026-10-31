//! Ausdruckssprache der Seamly2D-/Valentina-Muster.
//!
//! Musterformeln sind keine reinen Zahlen. Sie verweisen auf Messwerte,
//! abgeleitete Variablen und auf Strecken zwischen benannten Punkten:
//!
//! ```text
//! #FrontWidth+#ArmSkyeWidth+#BackWidth
//! Line_BustLine_A1/2
//! sqrt(#BustCirc/2) * sin(#ArmholeDepthDeg)
//! #Bust > 90 ? #Bust/10 : #Bust/12
//! ```
//!
//! Die Grammatik folgt dem von Seamly2D verwendeten QMuParser-Dialekt.
//! Bezeichner werden nicht aufgeteilt: der Aufrufer entscheidet, ob
//! `#BustWidth`, `Line_A1_A2` oder ein Messwert gemeint ist.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Fehler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum FormulaError {
    /// Unbekanntes Zeichen an Position `pos` im Ausdruck.
    UnexpectedChar { ch: char, pos: usize },
    /// Ausdruck endet unerwartet.
    UnexpectedEnd { pos: usize },
    /// Klammern passen nicht.
    UnbalancedParen { pos: usize },
    /// Bezeichner unbekannt.
    UnknownName { name: String },
    /// Unbekannte Funktion.
    UnknownFunction { name: String },
    /// Falsche Anzahl Argumente.
    Arity { func: &'static str, expected: &'static str, got: usize },
    /// Division durch Null.
    DivisionByZero,
    /// Ergebnis ist keine Zahl (NaN oder unendlich).
    NotFinite { what: &'static str },
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedChar { ch, pos } => {
                write!(f, "unerwartetes Zeichen {ch:?} an Position {pos}")
            }
            Self::UnexpectedEnd { pos } => write!(f, "unerwartetes Ende an Position {pos}"),
            Self::UnbalancedParen { pos } => write!(f, "Klammer nicht geschlossen bei {pos}"),
            Self::UnknownName { name } => write!(f, "unbekannter Bezeichner {name:?}"),
            Self::UnknownFunction { name } => write!(f, "unbekannte Funktion {name:?}"),
            Self::Arity { func, expected, got } => {
                write!(f, "{func} erwartet {expected} Argumente, bekam {got}")
            }
            Self::DivisionByZero => write!(f, "Division durch Null"),
            Self::NotFinite { what } => write!(f, "{what} ist nicht endlich"),
        }
    }
}

impl std::error::Error for FormulaError {}

// ---------------------------------------------------------------------------
// Kontext
// ---------------------------------------------------------------------------

/// Liefert die Werte für Bezeichner aus einem Musterausdruck.
///
/// Implementierungen entscheiden anhand des Namens, worum es sich handelt.
/// Ein Name kann sein:
///
/// * `BustWidth`        – Messwert oder abgeleitete Variable (ohne `#`)
/// * `Line_A1_A2`      – Länge der Strecke zwischen den Punkten `A1` und `A2`
/// * `Length_Vendor`   – Länge einer Seite im Detail
/// * `Height_Lapel`    – Höhe einer Form
pub trait Context {
    fn resolve(&self, name: &str) -> Result<f64, FormulaError>;

    /// Bequeme Kurzform; `?` und `!` werden als `||` bzw. `&&` akzeptiert.
    fn resolve_any(&self, name: &str) -> Result<f64, FormulaError> {
        self.resolve(name)
    }
}

/// Einfache Kontextimplementierung für Tests und Werkzeuge.
#[derive(Debug, Default, Clone)]
pub struct MapContext {
    vars: HashMap<String, f64>,
}

impl MapContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, name: impl Into<String>, value: f64) -> Self {
        self.vars.insert(name.into(), value);
        self
    }

    pub fn insert(&mut self, name: impl Into<String>, value: f64) {
        self.vars.insert(name.into(), value);
    }
}

impl Context for MapContext {
    fn resolve(&self, name: &str) -> Result<f64, FormulaError> {
        self.vars
            .get(name)
            .copied()
            .ok_or_else(|| FormulaError::UnknownName { name: name.to_owned() })
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    /// Bezeichner; ein führendes `#` ist bereits entfernt.
    Ident(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
    Question,
    Colon,
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self { src: src.as_bytes(), pos: 0 }
    }

    fn next_token(&mut self) -> Result<Option<(Tok, usize)>, FormulaError> {
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        let start = self.pos;
        if self.pos >= self.src.len() {
            return Ok(None);
        }
        let c = self.src[self.pos];

        // Zahl (auch .5 und 5.)
        if c.is_ascii_digit() || (c == b'.' && self.peek(1).is_some_and(|d| d.is_ascii_digit()))
        {
            let mut seen_dot = false;
            let mut seen_exp = false;
            while self.pos < self.src.len() {
                let d = self.src[self.pos];
                if d.is_ascii_digit() {
                    self.pos += 1;
                } else if d == b'.' && !seen_dot && !seen_exp {
                    seen_dot = true;
                    self.pos += 1;
                } else if (d == b'e' || d == b'E')
                    && !seen_exp
                    && self.peek(1).is_some_and(|n| {
                        n.is_ascii_digit()
                            || (matches!(n, b'+' | b'-') && self.peek(2).is_some_and(|m| m.is_ascii_digit()))
                    })
                {
                    seen_exp = true;
                    self.pos += 2;
                } else {
                    break;
                }
            }
            let text = std::str::from_utf8(&self.src[start..self.pos]).map_err(|_| {
                FormulaError::UnexpectedChar { ch: '?', pos: start }
            })?;
            let v: f64 = text.parse().map_err(|_| FormulaError::UnexpectedChar {
                ch: text.chars().next().unwrap_or('?'),
                pos: start,
            })?;
            return Ok(Some((Tok::Num(v), start)));
        }

        // Bezeichner, optional mit führendem '#'.
        //
        // `#` wird **entfernt** — die Variable `#Hoehe` wird unter `Hoehe`
        // geführt.
        //
        // `@` wird **mitgenommen** — anders als `#` gehört es zum gespeicherten
        // Namen. Die Massdatei von Bluse 2 enthält
        // `<m name="@armhole_width_b" value="4"/>`, und die Formel von
        // `#ArmWidthB` lautet `@armhole_width_b * #EaseBust`. Entfernt man
        // auch das `@`, findet der Messtabelle-Eintrag nichts.
        if c == b'#' || c == b'@' || c.is_ascii_alphabetic() || c == b'_' {
            let mit_raeffchen = c == b'@';
            if c == b'#' || c == b'@' {
                self.pos += 1;
            }
            let name_start = self.pos;
            while self.pos < self.src.len() {
                let d = self.src[self.pos];
                if d.is_ascii_alphanumeric() || d == b'_' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if self.pos == name_start {
                return Err(FormulaError::UnexpectedChar {
                    ch: c as char,
                    pos: start,
                });
            }
            let name = String::from_utf8_lossy(&self.src[name_start..self.pos]).into_owned();
            let name = if mit_raeffchen { format!("@{name}") } else { name };
            return Ok(Some((Tok::Ident(name), start)));
        }

        self.pos += 1;
        let two = if self.pos < self.src.len() {
            &self.src[start..start + 2]
        } else {
            &[][..]
        };
        let tok = match two {
            b"<=" => { self.pos += 1; Tok::Op("<=") }
            b">=" => { self.pos += 1; Tok::Op(">=") }
            b"==" => { self.pos += 1; Tok::Op("==") }
            b"!=" => { self.pos += 1; Tok::Op("!=") }
            b"&&" => { self.pos += 1; Tok::Op("&&") }
            b"||" => { self.pos += 1; Tok::Op("||") }
            _ => match c {
                b'+' => Tok::Op("+"),
                b'-' => Tok::Op("-"),
                b'*' => Tok::Op("*"),
                b'/' => Tok::Op("/"),
                b'^' => Tok::Op("^"),
                b'%' => Tok::Op("%"),
                b'<' => Tok::Op("<"),
                b'>' => Tok::Op(">"),
                b'(' => Tok::LParen,
                b')' => Tok::RParen,
                b',' => Tok::Comma,
                b'?' => Tok::Question,
                b':' => Tok::Colon,
                _ => {
                    return Err(FormulaError::UnexpectedChar {
                        ch: c as char,
                        pos: start,
                    })
                }
            },
        };
        Ok(Some((tok, start)))
    }

    fn peek(&self, n: usize) -> Option<u8> {
        self.src.get(self.pos + n).copied()
    }
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Node {
    Num(f64),
    Name(String),
    Neg(Box<Node>),
    Binary(&'static str, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

/// Ein übersetzter, wiederverwendbarer Ausdruck.
#[derive(Debug, Clone)]
pub struct Expression {
    root: Node,
}

impl Expression {
    /// Übersetzt `src` einmalig; `eval` ist danach ohne erneutes Parsen nutzbar.
    pub fn parse(src: &str) -> Result<Self, FormulaError> {
        let mut parser = Parser::new(src)?;
        let node = parser.parse_expr()?;
        // Trailing input means the expression was not fully consumed.
        if parser.peek().is_some() {
            let pos = parser.pos();
            return Err(if matches!(parser.peek(), Some(Tok::RParen)) {
                FormulaError::UnbalancedParen { pos }
            } else {
                FormulaError::UnexpectedEnd { pos }
            });
        }
        Ok(Self { root: node })
    }

    pub fn eval<C: Context>(&self, ctx: &C) -> Result<f64, FormulaError> {
        eval(&self.root, ctx)
    }
}

/// Kurzform: parsen und sofort auswerten.
pub fn eval_str<C: Context>(src: &str, ctx: &C) -> Result<f64, FormulaError> {
    Expression::parse(src)?.eval(ctx)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    lexer: Lexer<'a>,
    cur: Option<(Tok, usize)>,
    /// Position des zuletzt gelesenen Tokens. Bleibt erhalten, wenn `cur`
    /// leer läuft, damit Fehlermeldungen nicht auf 0 zurueckfallen.
    last_end: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Result<Self, FormulaError> {
        let mut lexer = Lexer::new(src);
        let cur = lexer.next_token()?;
        let last_end = lexer.pos;
        Ok(Self { lexer, cur, last_end })
    }

    fn peek(&self) -> Option<&Tok> {
        self.cur.as_ref().map(|(t, _)| t)
    }

    fn pos(&self) -> usize {
        match self.cur {
            Some((_, p)) => p,
            None => self.last_end,
        }
    }

    fn bump(&mut self) -> Result<Option<Tok>, FormulaError> {
        if let Some((_, p)) = self.cur {
            self.last_end = p;
        }
        let next = self.lexer.next_token()?;
        if next.is_none() {
            self.last_end = self.lexer.pos;
        }
        Ok(std::mem::replace(&mut self.cur, next).map(|(t, _)| t))
    }

    fn eat_op(&mut self, op: &str) -> Result<bool, FormulaError> {
        if matches!(self.peek(), Some(Tok::Op(o)) if *o == op) {
            self.bump()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn eat_keyword(&mut self, kw: &str) -> Result<bool, FormulaError> {
        match self.peek() {
            Some(Tok::Ident(n)) if n.eq_ignore_ascii_case(kw) => {
                self.bump()?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn expect(&mut self, want: Tok) -> Result<(), FormulaError> {
        let missing_is_unbalanced = matches!(want, Tok::RParen);
        match self.bump()? {
            Some(t) if t == want => Ok(()),
            Some(_) => Err(FormulaError::UnbalancedParen { pos: self.pos() }),
            // Eine fehlende schliessende Klammel ist genau das, wonach die
            // Fehlermeldung benannt ist.
            None if missing_is_unbalanced => {
                Err(FormulaError::UnbalancedParen { pos: self.pos() })
            }
            None => Err(FormulaError::UnexpectedEnd { pos: self.pos() }),
        }
    }

    // expr := ternary
    fn parse_expr(&mut self) -> Result<Node, FormulaError> {
        let cond = self.parse_or()?;
        if matches!(self.peek(), Some(Tok::Question)) {
            self.bump()?;
            let then = self.parse_expr()?;
            self.expect(Tok::Colon)?;
            let other = self.parse_expr()?;
            // `c ? a : b` als If-Funktion abbilden
            return Ok(Node::Call("if".into(), vec![cond, then, other]));
        }
        Ok(cond)
    }

    fn parse_or(&mut self) -> Result<Node, FormulaError> {
        let mut lhs = self.parse_and()?;
        loop {
            let is_or = self.eat_op("||")? || self.eat_keyword("or")?;
            if !is_or {
                break;
            }
            let rhs = self.parse_and()?;
            lhs = Node::Binary("||", Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Node, FormulaError> {
        let mut lhs = self.parse_cmp()?;
        loop {
            let is_and = self.eat_op("&&")? || self.eat_keyword("and")?;
            if !is_and {
                break;
            }
            let rhs = self.parse_cmp()?;
            lhs = Node::Binary("&&", Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Node, FormulaError> {
        let lhs = self.parse_add()?;
        for op in ["<=", ">=", "==", "!=", "<", ">"] {
            if self.eat_op(op)? {
                let rhs = self.parse_add()?;
                return Ok(Node::Binary(op, Box::new(lhs), Box::new(rhs)));
            }
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Node, FormulaError> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = if self.eat_op("+")? {
                "+"
            } else if self.eat_op("-")? {
                "-"
            } else {
                break;
            };
            let rhs = self.parse_mul()?;
            lhs = Node::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Node, FormulaError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = if self.eat_op("*")? {
                "*"
            } else if self.eat_op("/")? {
                "/"
            } else if self.eat_op("%")? {
                "%"
            } else {
                break;
            };
            let rhs = self.parse_unary()?;
            lhs = Node::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    /// Unäres Minus bindet schwächer als `^`, wie in QMuParser:
    /// `-2^2` ergibt `-4`.
    fn parse_unary(&mut self) -> Result<Node, FormulaError> {
        if self.eat_op("-")? {
            return Ok(Node::Neg(Box::new(self.parse_unary()?)));
        }
        if self.eat_op("+")? {
            return self.parse_unary();
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<Node, FormulaError> {
        let base = self.parse_primary()?;
        if self.eat_op("^")? {
            // rechts-assoziativ
            let exp = self.parse_unary()?;
            return Ok(Node::Binary("^", Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn parse_primary(&mut self) -> Result<Node, FormulaError> {
        let pos = self.pos();
        match self.bump()? {
            Some(Tok::Num(v)) => Ok(Node::Num(v)),
            Some(Tok::Ident(name)) => {
                if matches!(self.peek(), Some(Tok::LParen)) {
                    self.bump()?; // (
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        loop {
                            args.push(self.parse_expr()?);
                            if matches!(self.peek(), Some(Tok::Comma)) {
                                self.bump()?;
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Tok::RParen)?;
                    Ok(Node::Call(name, args))
                } else {
                    Ok(Node::Name(name))
                }
            }
            Some(Tok::LParen) => {
                let inner = self.parse_expr()?;
                self.expect(Tok::RParen)?;
                Ok(inner)
            }
            Some(Tok::RParen) => Err(FormulaError::UnbalancedParen { pos }),
            Some(_) => Err(FormulaError::UnexpectedEnd { pos }),
            None => Err(FormulaError::UnexpectedEnd { pos }),
        }
    }
}

// ---------------------------------------------------------------------------
// Auswertung
// ---------------------------------------------------------------------------

fn eval<C: Context>(node: &Node, ctx: &C) -> Result<f64, FormulaError> {
    match node {
        Node::Num(v) => Ok(*v),
        Node::Name(n) => ctx.resolve(n),
        Node::Neg(inner) => Ok(-eval(inner, ctx)?),
        Node::Binary(op, a, b) => {
            let x = eval(a, ctx)?;
            let y = eval(b, ctx)?;
            let v = match *op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => {
                    if y == 0.0 {
                        return Err(FormulaError::DivisionByZero);
                    }
                    x / y
                }
                "%" => {
                    if y == 0.0 {
                        return Err(FormulaError::DivisionByZero);
                    }
                    x % y
                }
                "^" => x.powf(y),
                "<" => (x < y) as i32 as f64,
                ">" => (x > y) as i32 as f64,
                "<=" => (x <= y) as i32 as f64,
                ">=" => (x >= y) as i32 as f64,
                "==" => (x == y) as i32 as f64,
                "!=" => (x != y) as i32 as f64,
                "&&" => ((x != 0.0) && (y != 0.0)) as i32 as f64,
                "||" => ((x != 0.0) || (y != 0.0)) as i32 as f64,
                _ => unreachable!("unbekannter Operator {op}"),
            };
            Ok(v)
        }
        Node::Call(name, args) => call(name, args, ctx),
    }
}

/// Bogenmaß in Grad.
fn to_deg(r: f64) -> f64 {
    r * 180.0 / std::f64::consts::PI
}

/// Grad in Bogenmaß.
fn to_rad(d: f64) -> f64 {
    d * std::f64::consts::PI / 180.0
}

fn finite(v: f64, what: &'static str) -> Result<f64, FormulaError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(FormulaError::NotFinite { what })
    }
}

fn call<C: Context>(name: &str, args: &[Node], ctx: &C) -> Result<f64, FormulaError> {
    // Kurze Formen treffen oft auf; lange Form auswerten.
    let arity_err = |expected: &'static str| FormulaError::Arity {
        func: match name {
            "sin" | "sind" | "cos" | "cosd" | "tan" | "tand" => "sin/cos/tan",
            "sqrt" | "abs" | "rint" | "sign" | "exp" => "sqrt/abs/rint",
            "min" | "max" | "sum" | "avg" => "min/max/sum/avg",
            _ => "",
        },
        expected,
        got: args.len(),
    };

    macro_rules! one {
        () => {{
            if args.len() != 1 {
                return Err(arity_err("1"));
            }
            eval(&args[0], ctx)?
        }};
    }
    

    let lower = name.to_ascii_lowercase();
    let v = match lower.as_str() {
        // trigonometrisch, Bogenmaß
        "sin" => one!().sin(),
        "cos" => one!().cos(),
        "tan" => one!().tan(),
        "asin" => one!().asin(),
        "acos" => one!().acos(),
        "atan" => one!().atan(),
        "sinh" => one!().sinh(),
        "cosh" => one!().cosh(),
        "tanh" => one!().tanh(),
        "asinh" => one!().asinh(),
        "acosh" => one!().acosh(),
        "atanh" => one!().atanh(),

        // trigonometrisch, Grad als Eingabe
        "sind" => to_rad(one!()).sin(),
        "cosd" => to_rad(one!()).cos(),
        "tand" => to_rad(one!()).tan(),
        // trigonometrisch, Grad als Ergebnis
        "asind" => to_deg(one!().asin()),
        "acosd" => to_deg(one!().acos()),
        "atand" => to_deg(one!().atan()),

        // exponentiell und logarithmisch
        "exp" => one!().exp(),
        "ln" | "log" => one!().ln(),
        "log10" => one!().log10(),
        "log2" => one!().log2(),
        "sqrt" => finite(one!().sqrt(), "Wurzel")?,
        "abs" => one!().abs(),
        "sign" => one!().signum(),
        "rint" => one!().round(),
        "floor" => one!().floor(),
        "ceil" => one!().ceil(),
        "trunc" => one!().trunc(),

        // bedingt
        "if" => {
            if args.len() != 3 {
                return Err(arity_err("3"));
            }
            let c = eval(&args[0], ctx)?;
            if c != 0.0 {
                eval(&args[1], ctx)?
            } else {
                eval(&args[2], ctx)?
            }
        }

        // variabel
        "min" | "max" | "sum" | "avg" => {
            if args.is_empty() {
                return Err(arity_err("mindestens 1"));
            }
            let mut it = args.iter().map(|a| eval(a, ctx));
            let first = it.next().unwrap()?;
            let mut acc = first;
            for v in it {
                let v = v?;
                acc = match lower.as_str() {
                    "min" => acc.min(v),
                    "max" => acc.max(v),
                    _ => acc + v,
                };
            }
            if lower == "avg" {
                acc / args.len() as f64
            } else {
                acc
            }
        }

        other => return Err(FormulaError::UnknownFunction { name: other.to_owned() }),
    };
    Ok(v)
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        // std::f64 PartialEq wird hier nicht genutzt; AST-Vergleich
        // behandelt NaN wie alle anderen Werte.
        match (self, other) {
            (Node::Num(a), Node::Num(b)) => a.to_bits() == b.to_bits(),
            (Node::Name(a), Node::Name(b)) => a == b,
            (Node::Neg(a), Node::Neg(b)) => a == b,
            (Node::Binary(o1, a1, b1), Node::Binary(o2, a2, b2)) => {
                o1 == o2 && a1 == a2 && b1 == b2
            }
            (Node::Call(n1, a1), Node::Call(n2, a2)) => n1 == n2 && a1 == a2,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> MapContext {
        MapContext::new()
            .with("BustWidth", 24.0)
            .with("ArmSkyeWidth", 5.5)
            .with("BackWidth", 20.0)
            .with("Line_BustLine_A1", 12.0)
    }

    #[test]
    fn rechnet_Grundoperationen() {
        let c = ctx();
        assert_eq!(eval_str("2+3", &c).unwrap(), 5.0);
        assert_eq!(eval_str("10-4", &c).unwrap(), 6.0);
        assert_eq!(eval_str("6*7", &c).unwrap(), 42.0);
        assert_eq!(eval_str("10/4", &c).unwrap(), 2.5);
        assert_eq!(eval_str("2^10", &c).unwrap(), 1024.0);
        assert_eq!(eval_str("10%3", &c).unwrap(), 1.0);
    }

    #[test]
    fn versteht_hash_und_verkettete_Bezeichner() {
        let c = ctx();
        // Exakt der Ausdruck aus male_shirt.sm2d
        let e = "#BustWidth+#ArmSkyeWidth+#BackWidth";
        assert_eq!(eval_str(e, &c).unwrap(), 49.5);
        // Reihenfolge und Verschachtelung
        assert_eq!(eval_str("1+2*3", &c).unwrap(), 7.0);
        assert_eq!(eval_str("(1+2)*3", &c).unwrap(), 9.0);
    }

    #[test]
    fn versteht_Line_Streckenverweise() {
        let c = ctx();
        // Exakt der Ausdruck aus male_shirt.sm2d
        assert_eq!(eval_str("Line_BustLine_A1/2", &c).unwrap(), 6.0);
        assert_eq!(eval_str("Line_BustLine_A1", &c).unwrap(), 12.0);
    }

    #[test]
    fn klammerbindung_von_potenz_und_vorzeichen() {
        let c = ctx();
        assert_eq!(eval_str("-2^2", &c).unwrap(), -4.0);
        assert_eq!(eval_str("(-2)^2", &c).unwrap(), 4.0);
        assert_eq!(eval_str("2^3^2", &c).unwrap(), 512.0); // rechts-assoziativ
    }

    #[test]
    fn funktionen_in_bogenmass_und_grad() {
        let c = ctx();
        assert!((eval_str("sin(0)", &c).unwrap()).abs() < 1e-12);
        assert!((eval_str("cos(0)", &c).unwrap() - 1.0).abs() < 1e-12);
        assert!((eval_str("sind(90)", &c).unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(eval_str("sqrt(16)", &c).unwrap(), 4.0);
        assert_eq!(eval_str("abs(-3)", &c).unwrap(), 3.0);
        assert_eq!(eval_str("min(3,1,2)", &c).unwrap(), 1.0);
        assert_eq!(eval_str("max(3,1,2)", &c).unwrap(), 3.0);
    }

    #[test]
    fn ternaer_und_vergleich() {
        let c = ctx();
        assert_eq!(eval_str("BustWidth>20 ? 1 : 2", &c).unwrap(), 1.0);
        assert_eq!(eval_str("BustWidth>30 ? 1 : 2", &c).unwrap(), 2.0);
        assert_eq!(eval_str("if(BustWidth>20, 7, 8)", &c).unwrap(), 7.0);
        assert_eq!(eval_str("BustWidth==24", &c).unwrap(), 1.0);
        assert_eq!(eval_str("BustWidth!=24", &c).unwrap(), 0.0);
    }

    #[test]
    fn meldet_fehler_präzise() {
        let c = ctx();
        assert_eq!(
            eval_str("1+", &c).unwrap_err(),
            FormulaError::UnexpectedEnd { pos: 2 }
        );
        assert_eq!(
            eval_str("(1+2", &c).unwrap_err(),
            FormulaError::UnbalancedParen { pos: 4 }
        );
        assert_eq!(
            eval_str("1/0", &c).unwrap_err(),
            FormulaError::DivisionByZero
        );
        match eval_str("UnknownVar", &c).unwrap_err() {
            FormulaError::UnknownName { name } => assert_eq!(name, "UnknownVar"),
            other => panic!("unerwartet: {other:?}"),
        }
    }

    #[test]
    fn ausdruck_ist_wiederverwendbar() {
        let e = Expression::parse("#BustWidth*2").unwrap();
        let c1 = MapContext::new().with("BustWidth", 10.0);
        let c2 = MapContext::new().with("BustWidth", 20.0);
        assert_eq!(e.eval(&c1).unwrap(), 20.0);
        assert_eq!(e.eval(&c2).unwrap(), 40.0);
    }

    #[test]
    fn leere_und_leerzeichen() {
        let c = ctx();
        assert!(eval_str("", &c).is_err());
        assert_eq!(eval_str("  7  ", &c).unwrap(), 7.0);
    }
}