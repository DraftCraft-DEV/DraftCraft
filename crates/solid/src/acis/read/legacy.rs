//! What the decoder leaves out of save format 400: an `intcurve` whose subtype it can't type.
//!
//! AutoCAD's DWG and DXF 2000 write a SWEEP's rail as `{ parcur nubs 3 open … surf1 }`: the curve
//! on a surface as ACIS 4 laid it out (no serializer stamp, no cache form, `surf1` closing it),
//! which the decoder only reads word by word. Its edge then comes with no curve, and the edge's
//! two pcurves, which point into the curve (`pcurve $-1 1 $32 0 0`), with none either. Both are
//! in the words: the curve's B-spline cache first, then its surfaces and its two pcurves'
//! B-splines (`nubs 1 open 2 …` or `nullbs`). [`fill`] puts them into the decoded B-rep, where
//! the decoder would have: the curve as its edge's carrier, each pcurve on its coedge.
//!
//! AutoCAD's DXF 2004 to 2010 write a LOFT's or a SWEEP's faces as procedural surfaces in ASM's
//! text (`{ loft_spl_sur 23100 … full nubs 3 3 open open none none 8 2 … }`), which the decoder
//! reads in binary but not as text: the face comes with an unknown surface. The surface's
//! B-spline cache is in the words, after `full`, at the subtype's own depth; a `{ ref 7 }`
//! names the eighth subtype written. [`fill`] gives such a face that B-spline.
//!
//! The words come from a file: every count is checked and capped, and a curve that doesn't add
//! up is left out (its edge stays as the decoder left it).

use std::collections::HashMap;

use cadmpeg_asm::sab::{Record, Token};
use serde_json::{Value, json};

/// Most knots or poles taken from one B-spline.
const MAX_COUNT: usize = 1 << 20;
/// Highest degree taken.
const MAX_DEGREE: i64 = 25;

/// A B-spline read from the words: its degree, full knot vector and poles (each with its weight
/// last when rational).
#[derive(Clone, Debug)]
struct Spline {
    degree: usize,
    knots: Vec<f64>,
    poles: Vec<Vec<f64>>,
    rational: bool,
}

impl Spline {
    /// The same curve run the other way: `c(-t)`.
    fn reversed(&self) -> Spline {
        let mut s = self.clone();
        s.knots = self.knots.iter().rev().map(|k| -k).collect();
        s.poles.reverse();
        s
    }
}

/// An intcurve's cache and its two pcurves (`None` for `nullbs`), and whether the curve record
/// runs against its subtype (`reversed`).
#[derive(Debug)]
struct Legacy {
    curve: Spline,
    pcurves: [Option<Spline>; 2],
    reversed: bool,
}

fn number(t: Option<&Token>) -> Option<f64> {
    match t? {
        Token::Long(v) => Some(*v as f64),
        Token::Double(v) if v.is_finite() => Some(*v),
        _ => None,
    }
}

fn long(t: Option<&Token>) -> Option<i64> {
    match t? {
        Token::Long(v) => Some(*v),
        _ => None,
    }
}

fn word(t: Option<&Token>) -> Option<&str> {
    match t? {
        Token::Ident(w) | Token::SubIdent(w) | Token::Str(w) => Some(w),
        _ => None,
    }
}

/// One `nubs`/`nurbs` block at `tokens[at]` with poles of `dim` coordinates: the spline and the
/// position after it. ACIS lists each distinct knot with its multiplicity, the ends one short of
/// the full vector.
fn spline(tokens: &[Token], at: usize, dim: usize) -> Option<(Spline, usize)> {
    let rational = match word(tokens.get(at))? {
        "nubs" => false,
        "nurbs" => true,
        _ => return None,
    };
    let degree = long(tokens.get(at + 1)).filter(|d| (1..=MAX_DEGREE).contains(d))?;
    let degree = usize::try_from(degree).ok()?;
    // The closure: a word in text, an enumeration in binary.
    if !matches!(tokens.get(at + 2)?, Token::Ident(_) | Token::Enum(_)) {
        return None;
    }
    let count = usize::try_from(long(tokens.get(at + 3))?).ok().filter(|n| (2..=MAX_COUNT).contains(n))?;
    let (knots, mut i) = knot_vector(tokens, at + 4, count, degree)?;
    let poles = knots.len().checked_sub(degree + 1).filter(|n| *n > degree)?;
    let per = dim + usize::from(rational);
    let mut out = Vec::with_capacity(poles);
    for _ in 0..poles {
        let p: Vec<f64> = (0..per).map(|k| number(tokens.get(i + k))).collect::<Option<_>>()?;
        if rational && p.last().is_none_or(|w| *w <= 0.0) {
            return None;
        }
        out.push(p);
        i += per;
    }
    Some((Spline { degree, knots, poles: out, rational }, i))
}

/// A helix as ASM's text writes it (`helix_int_cur 23100 F t0 F t1`, its centre, major and minor
/// axes, rise per turn, taper and axis, then four null supports): the cubic B-spline the binary
/// form is read as ([`super::helix_spline`]), on the same angle. A tapered one is left out.
fn helix(t: &[Token], at: usize) -> Option<Spline> {
    let mut i = at;
    // The serializer stamp.
    if long(t.get(i)).is_some_and(|s| s >= 100) {
        i += 1;
    }
    // The angle range: two bounded ends, `F value` each.
    let mut range = [0.0; 2];
    for end in &mut range {
        if !matches!(t.get(i)?, Token::Ident(_) | Token::True | Token::False) {
            return None;
        }
        *end = number(t.get(i + 1))?;
        i += 2;
    }
    let v: Vec<f64> = (0..16).map(|k| number(t.get(i + k))).collect::<Option<_>>()?;
    let xyz = |k: usize| Some([*v.get(k)?, *v.get(k + 1)?, *v.get(k + 2)?]);
    if v.get(12).is_none_or(|taper| taper.abs() > 1e-12) {
        return None;
    }
    let curve = super::helix_spline(xyz(0)?, xyz(3)?, xyz(6)?, xyz(9)?, range[0], range[1])?;
    let solidcore_step::geom::Curve::BSpline(b) = curve else { return None };
    let mut knots = Vec::new();
    for (k, m) in b.knots.iter().zip(&b.multiplicities) {
        knots.extend(std::iter::repeat_n(*k, *m));
    }
    Some(Spline { degree: b.degree, knots, poles: b.points.iter().map(|p| p.to_vec()).collect(), rational: false })
}

/// The intcurve of a curve record whose subtype the decoder only read word by word.
fn legacy(r: &Record) -> Option<Legacy> {
    let t = &r.tokens;
    let open = t.iter().position(|x| matches!(x, Token::SubtypeOpen))?;
    // The record's sense, just before the subtype.
    let reversed = matches!(t.get(open.checked_sub(1)?)?, Token::True);
    if word(t.get(open + 1)) == Some("helix_int_cur") {
        return Some(Legacy { curve: helix(t, open + 2)?, pcurves: [None, None], reversed });
    }
    if !matches!(word(t.get(open + 1))?, "parcur" | "par_int_cur" | "exactcur" | "exact_int_cur" | "surfintcur" | "surf_int_cur") {
        return None;
    }
    // A typed scope (a stamp, a cache form) is the decoder's own: only the bare word layout here.
    let (curve, mut i) = spline(t, open + 2, 3)?;
    // The rest of the scope at its own depth: the two pcurves are its first two B-spline blocks
    // (`nullbs` for none), the surfaces' scopes skipped.
    let mut pcurves: Vec<Option<Spline>> = Vec::new();
    let mut depth = 0usize;
    while pcurves.len() < 2 {
        match t.get(i)? {
            Token::SubtypeOpen => depth += 1,
            Token::SubtypeClose if depth == 0 => break,
            Token::SubtypeClose => depth -= 1,
            Token::Ident(w) if depth == 0 && w == "nullbs" => pcurves.push(None),
            Token::Ident(w) if depth == 0 && (w == "nubs" || w == "nurbs") => {
                let (p, next) = spline(t, i, 2)?;
                pcurves.push(Some(p));
                i = next;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    let mut it = pcurves.into_iter();
    Some(Legacy { curve, pcurves: [it.next().flatten(), it.next().flatten()], reversed })
}

/// A B-spline surface read from the words: its degrees, full knot vectors and poles by u then v
/// (each with its weight last when rational).
#[derive(Clone, Debug)]
struct Patch {
    degrees: [usize; 2],
    knots: [Vec<f64>; 2],
    rows: Vec<Vec<Vec<f64>>>,
    rational: bool,
}

/// ACIS's distinct knots and multiplicities from `tokens[at]`, `count` pairs, as a full vector
/// (one more of each end knot), and the position after them.
fn knot_vector(tokens: &[Token], at: usize, count: usize, degree: usize) -> Option<(Vec<f64>, usize)> {
    let mut knots = Vec::new();
    let mut i = at;
    for _ in 0..count {
        let k = number(tokens.get(i))?;
        let m = usize::try_from(long(tokens.get(i + 1))?).ok().filter(|m| (1..=degree + 1).contains(m))?;
        if knots.len().checked_add(m)? > MAX_COUNT {
            return None;
        }
        knots.extend(std::iter::repeat_n(k, m));
        i += 2;
    }
    let (first, last) = (*knots.first()?, *knots.last()?);
    knots.insert(0, first);
    knots.push(last);
    if knots.windows(2).any(|w| w[1] < w[0]) {
        return None;
    }
    Some((knots, i))
}

/// One `nubs`/`nurbs` surface block at `tokens[at]`: degrees, an optional `u`/`v`/`both`, the
/// closures and singularities, the knot counts, both knot lists, and the poles with u running
/// fastest.
fn patch(tokens: &[Token], at: usize) -> Option<Patch> {
    let rational = match word(tokens.get(at))? {
        "nubs" => false,
        "nurbs" => true,
        _ => return None,
    };
    let degree = |t: Option<&Token>| long(t).filter(|d| (1..=MAX_DEGREE).contains(d)).and_then(|d| usize::try_from(d).ok());
    let (du, dv) = (degree(tokens.get(at + 1))?, degree(tokens.get(at + 2))?);
    let mut i = at + 3;
    if matches!(word(tokens.get(i)), Some("u" | "v" | "both")) {
        i += 1;
    }
    // Two closures and two singularities: words in text, enumerations in binary.
    for _ in 0..4 {
        if !matches!(tokens.get(i)?, Token::Ident(_) | Token::Enum(_)) {
            return None;
        }
        i += 1;
    }
    let count = |t: Option<&Token>| long(t).and_then(|n| usize::try_from(n).ok()).filter(|n| (2..=MAX_COUNT).contains(n));
    let (nu, nv) = (count(tokens.get(i))?, count(tokens.get(i + 1))?);
    let (uk, next) = knot_vector(tokens, i + 2, nu, du)?;
    let (vk, next) = knot_vector(tokens, next, nv, dv)?;
    let pu = uk.len().checked_sub(du + 1).filter(|n| *n > du)?;
    let pv = vk.len().checked_sub(dv + 1).filter(|n| *n > dv)?;
    if pu.checked_mul(pv)? > MAX_COUNT {
        return None;
    }
    let per = 3 + usize::from(rational);
    let mut rows = vec![Vec::with_capacity(pv); pu];
    let mut i = next;
    for _ in 0..pv {
        for row in rows.iter_mut() {
            let p: Vec<f64> = (0..per).map(|k| number(tokens.get(i + k))).collect::<Option<_>>()?;
            if rational && p.last().is_none_or(|w| *w <= 0.0) {
                return None;
            }
            row.push(p);
            i += per;
        }
    }
    Some(Patch { degrees: [du, dv], knots: [uk, vk], rows, rational })
}

/// Every subtype the records define, in the order they are written (a scope opening with a name
/// other than `ref`, nested ones included): what `{ ref k }` counts.
fn subtypes(records: &[Record]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (ri, r) in records.iter().enumerate() {
        for (pos, t) in r.tokens.iter().enumerate() {
            if matches!(t, Token::SubtypeOpen) && word(r.tokens.get(pos + 1)).is_some_and(|w| w != "ref") {
                if out.len() >= MAX_COUNT {
                    return out;
                }
                out.push((ri, pos));
            }
        }
    }
    out
}

/// The B-spline cache of the subtype opening at `tokens[open]`: the `full` block at its own
/// depth. A `{ ref k }` is the cache of the subtype it names.
fn cache(records: &[Record], defs: &[(usize, usize)], tokens: &[Token], open: usize) -> Option<Patch> {
    if word(tokens.get(open + 1)) == Some("ref") {
        let k = usize::try_from(long(tokens.get(open + 2))?).ok()?;
        let (ri, pos) = *defs.get(k)?;
        let r = records.get(ri)?;
        return own_cache(&r.tokens, pos);
    }
    own_cache(tokens, open)
}

fn own_cache(tokens: &[Token], open: usize) -> Option<Patch> {
    let mut depth = 0usize;
    let mut i = open + 1;
    loop {
        match tokens.get(i)? {
            Token::SubtypeOpen => depth += 1,
            Token::SubtypeClose if depth == 0 => return None,
            Token::SubtypeClose => depth -= 1,
            Token::Ident(w) if depth == 0 && w == "full" => {
                if let Some(p) = patch(tokens, i + 1) {
                    return Some(p);
                }
            }
            // A binary cache form.
            Token::Enum(0) if depth == 0 && matches!(word(tokens.get(i + 1)), Some("nubs" | "nurbs")) => {
                if let Some(p) = patch(tokens, i + 1) {
                    return Some(p);
                }
            }
            _ => {}
        }
        i += 1;
    }
}

/// A surface as the decoded B-rep writes one, poles scaled to millimetres.
fn surface_geometry(p: &Patch, mm: f64) -> Value {
    let rows: Vec<Value> = p
        .rows
        .iter()
        .map(|row| {
            Value::Array(
                row.iter()
                    .map(|q| {
                        let point = json!({ "x": q.first().copied().unwrap_or(0.0) * mm, "y": q.get(1).copied().unwrap_or(0.0) * mm, "z": q.get(2).copied().unwrap_or(0.0) * mm });
                        match (p.rational, q.last()) {
                            (true, Some(w)) => json!({ "point": point, "weight": w }),
                            _ => point,
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    json!({ "kind": "nurbs", "u_degree": p.degrees[0], "v_degree": p.degrees[1], "u_knots": p.knots[0], "v_knots": p.knots[1], "poles": { "rows": rows } })
}

/// The surfaces the decoder left unknown given their B-spline cache.
fn fill_surfaces(v: &mut Value, records: &[Record], by_index: &HashMap<usize, (usize, &Record)>, prefix: &str, mm: f64) {
    let mut defs: Option<Vec<(usize, usize)>> = None;
    let Some(surfaces) = v.get_mut("surfaces").and_then(Value::as_array_mut) else { return };
    for s in surfaces.iter_mut() {
        if s.get("geometry").and_then(|g| g.get("kind")).and_then(Value::as_str) != Some("unknown") {
            continue;
        }
        let Some((_, r)) = s
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| id.strip_prefix(prefix))
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|i| by_index.get(&i))
        else {
            continue;
        };
        let Some(open) = r.tokens.iter().position(|t| matches!(t, Token::SubtypeOpen)) else { continue };
        let defs = defs.get_or_insert_with(|| subtypes(records));
        let Some(p) = cache(records, defs, &r.tokens, open) else { continue };
        if let Some(obj) = s.as_object_mut() {
            obj.insert("geometry".into(), surface_geometry(&p, mm));
        }
    }
}

/// The last reference of a record: an edge's curve, a coedge's pcurve.
fn last_ref(r: &Record) -> Option<usize> {
    r.tokens.iter().rev().find_map(|t| match t {
        Token::Ref(i) if *i >= 0 => usize::try_from(*i).ok(),
        _ => None,
    })
}

/// A spline as the decoded B-rep writes one: 3D poles scaled to millimetres, 2D ones as they are
/// (`uv`), each with its weight when rational.
fn geometry(s: &Spline, mm: f64, uv: bool) -> Value {
    let poles: Vec<Value> = s
        .poles
        .iter()
        .map(|p| {
            let point = if uv {
                json!({ "u": p.first().copied().unwrap_or(0.0), "v": p.get(1).copied().unwrap_or(0.0) })
            } else {
                json!({ "x": p.first().copied().unwrap_or(0.0) * mm, "y": p.get(1).copied().unwrap_or(0.0) * mm, "z": p.get(2).copied().unwrap_or(0.0) * mm })
            };
            match (s.rational, p.last()) {
                (true, Some(w)) => json!({ "point": point, "weight": w }),
                _ => point,
            }
        })
        .collect();
    json!({ "kind": "nurbs", "degree": s.degree, "knots": s.knots, "poles": { "points": poles } })
}

/// The decoded B-rep `v` with what the decoder left out of save format 400 put in (the module
/// says what); `mm` is the data's millimetres per unit.
pub(super) fn fill(v: &mut Value, records: &[Record], mm: f64) {
    let by_index: HashMap<usize, &Record> = records.iter().map(|r| (r.index, r)).collect();
    let by_position: HashMap<usize, (usize, &Record)> = records.iter().enumerate().map(|(k, r)| (r.index, (k, r))).collect();
    // The decoded B-rep's ids: its own prefix and the record's index.
    let Some(prefix) = v
        .get("edges")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|e| e.get("id"))
        .and_then(Value::as_str)
        .and_then(|id| id.rfind('#').map(|k| id[..=k].to_owned()))
    else {
        return;
    };
    fill_surfaces(v, records, &by_position, &prefix, mm);
    let index = |id: &str| id.strip_prefix(prefix.as_str()).and_then(|n| n.parse::<usize>().ok());
    let mut found: HashMap<usize, Option<Legacy>> = HashMap::new();
    let mut curves: Vec<Value> = Vec::new();
    // The edges with no curve whose record names an intcurve the words describe.
    if let Some(edges) = v.get_mut("edges").and_then(Value::as_array_mut) {
        for e in edges.iter_mut() {
            if e.get("carrier").and_then(|c| c.get("curve")).is_some() {
                continue;
            }
            let Some(c) = e.get("id").and_then(Value::as_str).and_then(index).and_then(|i| by_index.get(&i)).and_then(|r| last_ref(r)) else {
                continue;
            };
            let Some(record) = by_index.get(&c).filter(|r| r.head() == "intcurve") else { continue };
            let entry = found.entry(c).or_insert_with(|| legacy(record));
            let Some(l) = entry.as_ref() else { continue };
            let id = format!("{prefix}{c}");
            if !curves.iter().any(|x| x.get("id").and_then(Value::as_str) == Some(id.as_str())) {
                let s = if l.reversed { l.curve.reversed() } else { l.curve.clone() };
                curves.push(json!({ "id": id, "geometry": geometry(&s, mm, false) }));
            }
            if let Some(obj) = e.as_object_mut() {
                obj.insert("carrier".into(), json!({ "curve": id }));
            }
        }
    }
    if curves.is_empty() {
        return;
    }
    if let Some(list) = v.get_mut("curves").and_then(Value::as_array_mut) {
        list.extend(curves);
    }
    // The coedges whose pcurve points into one of those curves: `pcurve $-1 ±k $curve du dv`.
    let mut pcurves: Vec<Value> = Vec::new();
    if let Some(coedges) = v.get_mut("coedges").and_then(Value::as_array_mut) {
        for co in coedges.iter_mut() {
            if co.get("pcurves").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
                continue;
            }
            let Some(pi) = co.get("id").and_then(Value::as_str).and_then(index).and_then(|i| by_index.get(&i)).and_then(|r| last_ref(r)) else {
                continue;
            };
            let Some(prec) = by_index.get(&pi).filter(|r| r.head() == "pcurve") else { continue };
            // After the entity header: the selector, the curve, the uv offset.
            let tail: Vec<&Token> = prec.tokens.iter().skip(3).collect();
            let (Some(selector), Some(Token::Ref(c))) = (long(tail.first().copied()), tail.get(1).copied()) else { continue };
            let Some(Some(l)) = usize::try_from(*c).ok().and_then(|c| found.get(&c)) else { continue };
            let slot = match selector {
                1 | -1 => 0,
                2 | -2 => 1,
                _ => continue,
            };
            let Some(p) = l.pcurves.get(slot).and_then(Option::as_ref) else { continue };
            let du = number(tail.get(2).copied()).unwrap_or(0.0);
            let dv = number(tail.get(3).copied()).unwrap_or(0.0);
            // On the coedge's parameter: turned round for a negative selector, and again when the
            // curve record runs against its subtype.
            let p = if (selector < 0) != l.reversed { p.reversed() } else { p.clone() };
            let id = format!("{prefix}{pi}");
            pcurves.push(json!({
                "id": id,
                "geometry": geometry(&p, 1.0, true),
                "metadata": { "form": { "uv_offset": [du, dv], "wrapper_reversed": false } },
            }));
            if let Some(obj) = co.as_object_mut() {
                obj.insert("pcurves".into(), json!([{ "pcurve": id }]));
            }
        }
    }
    if let Some(list) = v.get_mut("pcurves").and_then(Value::as_array_mut) {
        list.extend(pcurves);
    }
}
