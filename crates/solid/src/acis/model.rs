//! ACIS records as DraftCraft writes them, and their two encodings (docs/solids.md, "The ACIS
//! translator"):
//!
//! - SAT text in save format 400, the ACIS 4.0 format AutoCAD writes in DWG and DXF 2000 files and
//!   every AutoCAD reads: AutoCAD 2027 discards a 700 solid from a DWG 2000 to 2010 file
//!   (error 75003), though it takes the same text from DXF.
//! - SAB binary in save format 700 (Spatial's published 7.0 format), what an AutoCAD 2013 and
//!   later data store record holds.
//!
//! A record is its name and its fields after the ones every entity starts with: its attribute
//! (null here) and, from format 700, its history index and one more reference (null). SAT writes
//! each field as a word; SAB writes the same fields as tagged binary tokens, one tag byte and its
//! little-endian value.
//!
//! A spline surface, curve or pcurve holds a subtype ([`Tok`]): `{ exactsur nubs 3 3 open … }`,
//! laid out as AutoCAD writes it in SAT 400 (its DXF 2000 files). SAB names the subtypes in
//! full (`exact_spl_sur`), writes the words as identifiers, the closures and singularities as
//! enumerations, and the numbers as integers and reals, as AutoCAD's data store does, with the
//! later formats' fields: the cache form (`full`) before a spline's data, and the tails AutoCAD's
//! data store records end with ([`Tok::Sat`] and [`Tok::Sab`] hold what one encoding has and the
//! other hasn't).

/// One field of a record.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Field {
    /// A reference to another record by its index; `None` is the null reference.
    Ref(Option<usize>),
    /// A real number.
    Real(f64),
    /// An edge's parameter at one of its vertices: format 700 on (format 400 works it out).
    Param(f64),
    /// An edge's convexity, `unknown`: format 700 on.
    Convexity,
    /// A position (lengths).
    Pos([f64; 3]),
    /// A vector (a direction, or an axis whose length counts).
    Vector([f64; 3]),
    /// A sense: forward (false) or reversed (true).
    Sense(bool),
    /// A face's sides: single.
    Single,
    /// A face's sides: double, its containment out (a region's face).
    Double,
    /// A surface's v sense: forward.
    ForwardV,
    /// An unbounded parameter range end (SAT `I`).
    Infinite,
    /// An integer (a pcurve's kind).
    Int(i64),
    /// A subtype: its tokens between the braces, the braces included.
    Sub(Vec<Tok>),
}

/// One token of a subtype.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Open,
    Close,
    /// A subtype's name: SAT's short one and SAB's full one (`exactsur`, `exact_spl_sur`).
    Name(&'static str, &'static str),
    /// A word: `nubs`, `spline`, `null_surface`…
    Word(&'static str),
    /// An enumeration: its SAT word and SAB value (`open` 0, `periodic` 2, `none` 0).
    Enum(&'static str, i32),
    Int(i64),
    Real(f64),
    /// A sense: forward (false) or reversed (true).
    Sense(bool),
    /// A range end: `I` unbounded, `F value` bounded.
    Bound(Option<f64>),
    /// A line break in SAT (nothing in SAB): AutoCAD breaks a spline's lists into lines.
    Line,
    /// Tokens SAT 400 has and SAB doesn't.
    Sat(Vec<Tok>),
    /// Tokens SAB (save format 700 on) has and SAT 400 doesn't.
    Sab(Vec<Tok>),
}

/// One record: its name (`plane-surface`, `coedge`…) and its fields.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Record {
    pub name: &'static str,
    pub fields: Vec<Field>,
}

/// The save format of SAT text.
pub(crate) const SAT_VERSION: u32 = 400;
/// The save format of SAB.
pub(crate) const VERSION: u32 = 700;
/// What the header says wrote the data.
const PRODUCT: &str = "DraftCraft";
const SAT_ACIS_VERSION: &str = "ACIS 4.0";
const ACIS_VERSION: &str = "ACIS 7.0";
/// The save date: fixed, so a solid saved twice keeps its bytes.
const DATE: &str = "Mon Jan 01 00:00:00 2001";
/// Millimetres per unit (the drawing's units are taken as they are), the absolute and the
/// normal resolution.
const SCALE: f64 = 1.0;
const RESABS: f64 = 1e-6;
const RESNOR: f64 = 1e-10;
/// The line (SAT) or name (SAB) that ends the data.
pub(crate) const END: &str = "End-of-ACIS-data";
/// What a binary ACIS stream starts with.
pub(crate) const SAB_MAGIC: &[u8; 15] = b"ACIS BinaryFile";

/// A real number as SAT writes it: the shortest text that reads back as the same number.
fn real(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    // `-0` reads back as 0; write it as 0.
    if v == 0.0 { "0".into() } else { format!("{v}") }
}

fn triple(v: &[f64; 3]) -> String {
    format!("{} {} {}", real(v[0]), real(v[1]), real(v[2]))
}

/// A subtype's tokens as SAT text, each after a space (a line break before none).
fn sat_tokens(o: &mut String, toks: &[Tok]) {
    for t in toks {
        if matches!(t, Tok::Line) {
            o.push_str(" \n");
            continue;
        }
        o.push(' ');
        match t {
            Tok::Open => o.push('{'),
            Tok::Close => o.push('}'),
            Tok::Name(short, _) => o.push_str(short),
            Tok::Word(w) | Tok::Enum(w, _) => o.push_str(w),
            Tok::Int(i) => o.push_str(&i.to_string()),
            Tok::Real(v) => o.push_str(&real(*v)),
            Tok::Sense(reversed) => o.push_str(if *reversed { "reversed" } else { "forward" }),
            Tok::Bound(None) => o.push('I'),
            Tok::Bound(Some(v)) => {
                o.push_str("F ");
                o.push_str(&real(*v));
            }
            Tok::Sat(toks) => {
                // The tokens put their own spaces: drop the one pushed above.
                o.pop();
                sat_tokens(o, toks);
            }
            Tok::Line | Tok::Sab(_) => {}
        }
    }
}

/// The records as SAT text in save format 400, header and end line included, one record a line.
/// The header counts the records, as AutoCAD's does.
pub(crate) fn sat(records: &[Record], bodies: usize) -> String {
    let mut o = format!("{SAT_VERSION} {} {bodies} 0 \n", records.len());
    o.push_str(&format!("{} {} {} {} {} {} \n", PRODUCT.len(), PRODUCT, SAT_ACIS_VERSION.len(), SAT_ACIS_VERSION, DATE.len(), DATE));
    o.push_str(&format!("{} {} {} \n", real(SCALE), real(RESABS), real(RESNOR)));
    for r in records {
        o.push_str(r.name);
        o.push_str(" $-1");
        for f in r.fields.iter().filter(|f| !matches!(f, Field::Param(_) | Field::Convexity)) {
            o.push(' ');
            match f {
                Field::Ref(Some(i)) => o.push_str(&format!("${i}")),
                Field::Ref(None) => o.push_str("$-1"),
                Field::Real(v) | Field::Param(v) => o.push_str(&real(*v)),
                Field::Convexity => o.push_str("@7 unknown"),
                Field::Pos(p) | Field::Vector(p) => o.push_str(&triple(p)),
                Field::Sense(reversed) => o.push_str(if *reversed { "reversed" } else { "forward" }),
                Field::Single => o.push_str("single"),
                Field::Double => o.push_str("double out"),
                Field::ForwardV => o.push_str("forward_v"),
                Field::Infinite => o.push('I'),
                Field::Int(i) => o.push_str(&i.to_string()),
                Field::Sub(toks) => {
                    // The tokens put their own spaces: drop the one pushed above.
                    o.pop();
                    sat_tokens(&mut o, toks);
                }
            }
        }
        o.push_str(" #\n");
    }
    o.push_str(END);
    o.push_str(" \n");
    o
}

/// SAB's tags.
mod tag {
    pub const INT: u8 = 0x04;
    pub const REAL: u8 = 0x06;
    pub const STR: u8 = 0x07;
    pub const TRUE: u8 = 0x0a;
    pub const FALSE: u8 = 0x0b;
    pub const REF: u8 = 0x0c;
    pub const IDENT: u8 = 0x0d;
    pub const SUB_IDENT: u8 = 0x0e;
    pub const END: u8 = 0x11;
    pub const SUB_OPEN: u8 = 0x0f;
    pub const SUB_CLOSE: u8 = 0x10;
    pub const POS: u8 = 0x13;
    pub const VECTOR: u8 = 0x14;
    pub const ENUM: u8 = 0x15;
}

fn push_name(o: &mut Vec<u8>, t: u8, s: &str) {
    o.push(t);
    o.push(u8::try_from(s.len()).unwrap_or(u8::MAX));
    o.extend(s.bytes().take(usize::from(u8::MAX)));
}

/// A record name, its parts joined by `-`: every part but the last a sub-identifier.
fn push_record_name(o: &mut Vec<u8>, name: &str) {
    let parts: Vec<&str> = name.split('-').collect();
    for (i, part) in parts.iter().enumerate() {
        push_name(o, if i + 1 == parts.len() { tag::IDENT } else { tag::SUB_IDENT }, part);
    }
}

fn push_i32(o: &mut Vec<u8>, t: u8, v: i32) {
    o.push(t);
    o.extend_from_slice(&v.to_le_bytes());
}

fn push_real(o: &mut Vec<u8>, v: f64) {
    o.push(tag::REAL);
    o.extend_from_slice(&(if v.is_finite() { v } else { 0.0 }).to_le_bytes());
}

fn push_triple(o: &mut Vec<u8>, t: u8, v: &[f64; 3]) {
    o.push(t);
    for x in v {
        o.extend_from_slice(&(if x.is_finite() { *x } else { 0.0 }).to_le_bytes());
    }
}

fn reference(i: Option<usize>) -> i32 {
    i.and_then(|i| i32::try_from(i).ok()).unwrap_or(-1)
}

/// An integer as SAB's 32-bit one (clamped: no count written comes near).
fn int32(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

/// A subtype's tokens as SAB.
fn sab_tokens(o: &mut Vec<u8>, toks: &[Tok]) {
    for t in toks {
        match t {
            Tok::Open => o.push(tag::SUB_OPEN),
            Tok::Close => o.push(tag::SUB_CLOSE),
            Tok::Name(_, full) => push_name(o, tag::IDENT, full),
            Tok::Word(w) => push_name(o, tag::IDENT, w),
            Tok::Enum(_, v) => push_i32(o, tag::ENUM, *v),
            Tok::Int(i) => push_i32(o, tag::INT, int32(*i)),
            Tok::Real(v) => push_real(o, *v),
            Tok::Sense(reversed) => o.push(if *reversed { tag::TRUE } else { tag::FALSE }),
            Tok::Bound(None) => o.push(tag::FALSE),
            Tok::Bound(Some(v)) => {
                o.push(tag::TRUE);
                push_real(o, *v);
            }
            Tok::Sab(toks) => sab_tokens(o, toks),
            Tok::Line | Tok::Sat(_) => {}
        }
    }
}

/// The records as SAB in save format 700, header and end marker included.
pub(crate) fn sab(records: &[Record], bodies: usize) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(SAB_MAGIC);
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    for v in [VERSION, 0, count(bodies), 0] {
        o.extend_from_slice(&v.to_le_bytes());
    }
    for s in [PRODUCT, ACIS_VERSION, DATE] {
        push_name(&mut o, tag::STR, s);
    }
    for v in [SCALE, RESABS, RESNOR] {
        push_real(&mut o, v);
    }
    for r in records {
        push_record_name(&mut o, r.name);
        push_i32(&mut o, tag::REF, -1);
        push_i32(&mut o, tag::INT, -1);
        push_i32(&mut o, tag::REF, -1);
        for f in &r.fields {
            match f {
                Field::Ref(i) => push_i32(&mut o, tag::REF, reference(*i)),
                Field::Real(v) | Field::Param(v) => push_real(&mut o, *v),
                Field::Convexity => push_name(&mut o, tag::STR, "unknown"),
                Field::Pos(p) => push_triple(&mut o, tag::POS, p),
                Field::Vector(p) => push_triple(&mut o, tag::VECTOR, p),
                Field::Sense(reversed) => o.push(if *reversed { tag::TRUE } else { tag::FALSE }),
                Field::Single | Field::ForwardV | Field::Infinite => o.push(tag::FALSE),
                Field::Double => o.extend([tag::TRUE, tag::FALSE]),
                Field::Int(i) => push_i32(&mut o, tag::INT, int32(*i)),
                Field::Sub(toks) => sab_tokens(&mut o, toks),
            }
        }
        o.push(tag::END);
    }
    push_record_name(&mut o, END);
    o
}

/// DXF and DWG store SAT text scrambled: each printable character `c` (`!` to `~`) as `159 - c`.
/// The same map undoes it.
pub fn scramble(text: &str) -> String {
    text.chars().map(|c| if ('!'..='~').contains(&c) { char::from(159 - c as u8) } else { c }).collect()
}
