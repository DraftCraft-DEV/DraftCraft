//! ACIS data as kernel bodies.
//!
//! `cadmpeg-asm` (Apache-2.0) frames the records, whatever the dialect (Spatial's ACIS save
//! formats, Autodesk's ShapeManager ones; text or binary), and decodes them into a neutral
//! B-rep: bodies to points, analytic surfaces and curves, NURBS ones from their caches, every
//! length in millimetres and every edge running along its curve. That B-rep is written as STEP
//! topology (`solidcore_step::write`'s entity writer) and read back by solidcore's STEP reader,
//! which works out what ACIS leaves to the modeller: pcurves, the seams of faces that go all the
//! way round, and the tolerances. Each of the data's solids becomes one body.
//!
//! The data comes from a file: every step is checked, and a solid that can't be read is an
//! error with the reason, never a panic.

use std::collections::HashMap;

use cadmpeg_core::decode::{DecodeArena, DecodeContext, DecodePolicy};
use serde_json::Value;
use solidcore_ops as ops;
use solidcore_ops::pcurve::{EdgeUse, TrimFace, Trims};
use solidcore_record::brep::Curve2;
use solidcore_step::geom::{BSplineCurve, BSplineSurface, Curve, CurveForm, KnotSpec, Logical, P3, Placement, Surface, SurfaceForm};
use solidcore_step::p21::Param;
use solidcore_step::write::{GeomWriter, Writer};

use super::model::{SAB_MAGIC, scramble};

mod legacy;
use crate::body::{Body, Result, SolidError};

fn err(m: impl Into<String>) -> SolidError {
    SolidError(m.into())
}

/// Largest ACIS data accepted for one entity.
pub const MAX_DATA: usize = 256 << 20;
/// Most faces read from one entity's data.
const MAX_FACES: usize = 200_000;

/// The kernel bodies of one entity's ACIS data: SAT text (as the entity's groups hold it,
/// scrambled or not) or SAB.
pub fn read(data: &[u8]) -> Result<Vec<Body>> {
    if data.is_empty() || data.len() > MAX_DATA {
        return Err(err("the solid's ACIS data is empty or too large"));
    }
    let (value, scale) = decode(data)?;
    to_bodies(&value, scale)
}

/// The data decoded into the neutral B-rep (as JSON), and its millimetres per unit.
pub(super) fn decode(data: &[u8]) -> Result<(Value, f64)> {
    if data.is_empty() || data.len() > MAX_DATA {
        return Err(err("the ACIS data is empty or too large"));
    }
    if data.starts_with(SAB_MAGIC) || data.starts_with(b"ASM BinaryFile") { decode_binary(data) } else { decode_text(data) }
}

/// The text as SAT: unscrambled when it doesn't start with the save format's number.
fn plain_text(data: &[u8]) -> Vec<u8> {
    let text: String = data.iter().map(|b| char::from(*b)).collect();
    let starts_plain = text.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit());
    let text = if starts_plain { text } else { scramble(&text) };
    text.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect()
}

fn decode_error(e: impl std::fmt::Display) -> SolidError {
    err(format!("the solid's ACIS data can't be read ({e})"))
}

fn format_id() -> Result<cadmpeg_asm::ids::IdFormat> {
    cadmpeg_asm::ids::IdFormat::from_literal("dwg").ok_or_else(|| err("the ACIS reader is not set up"))
}

/// Where a binary stream's end marker (`End-of-ACIS-data` or `End-of-ASM-data`, its name parts
/// with no record terminator after them) starts, at or after `start`.
fn end_marker(data: &[u8], start: usize) -> Option<usize> {
    const HEAD: &[u8] = b"\x0e\x03End\x0e\x02of\x0e";
    let rest = data.get(start..)?;
    rest.windows(HEAD.len()).enumerate().find_map(|(i, w)| {
        if w != HEAD {
            return None;
        }
        let tail = rest.get(i + HEAD.len()..)?;
        let ends = tail.starts_with(b"\x04ACIS\x0d\x04data") || tail.starts_with(b"\x03ASM\x0d\x04data");
        ends.then_some(start + i)
    })
}

/// What the decoder multiplies a binary stream's lengths by.
const BINARY_MM_PER_UNIT: f64 = 10.0;

/// Decoded text: the neutral B-rep (as JSON) and the data's millimetres per unit.
fn decode_text(data: &[u8]) -> Result<(Value, f64)> {
    let bytes = plain_text(data);
    let arena = DecodeArena::new();
    let (ctx, _) = DecodeContext::from_root_bytes(&bytes, &arena, &DecodePolicy::desktop()).map_err(decode_error)?;
    let parsed = cadmpeg_asm::sat::parse(&ctx, &bytes).map_err(|e| err(format!("the solid's ACIS text can't be read ({e:?})")))?;
    let header = parsed.header.as_kernel_header();
    // The decoder turns the text's units into millimetres with the header's millimetres per
    // unit; a drawing takes the coordinates as they are written, so that factor comes back off.
    let scale = Some(parsed.header.scale().get()).filter(|s| s.is_finite() && *s > 0.0).unwrap_or(1.0);
    let records = long_names(parsed.records);
    let brep =
        cadmpeg_asm::brep::decode_with_header(&ctx, &records, &bytes, Some(&header), "acis", format_id()?, cadmpeg_asm::brep::DecodePurpose::Model)
            .map_err(decode_error)?;
    let mut value = serde_json::to_value(&brep).map_err(decode_error)?;
    legacy::fill(&mut value, &records, scale);
    Ok((value, scale))
}

fn decode_binary(data: &[u8]) -> Result<(Value, f64)> {
    use cadmpeg_asm::{acis_header, asm_header};
    let arena = DecodeArena::new();
    let (ctx, _) = DecodeContext::from_root_bytes(data, &arena, &DecodePolicy::desktop()).map_err(decode_error)?;
    let (header, start, limit) = if let Some(h) = asm_header::parse(data) {
        let start = asm_header::record_stream_start_with_header(data, &h).ok_or_else(|| err("the solid's ACIS data has no records"))?;
        let limit = asm_header::solved_record_limit_with_header(data, &h).unwrap_or(data.len());
        (h, start, limit)
    } else if let Some(h) = acis_header::parse(data) {
        let start = acis_header::record_stream_start_with_header(data, &h).ok_or_else(|| err("the solid's ACIS data has no records"))?;
        let limit = acis_header::solved_record_limit(data).unwrap_or(data.len());
        (h, start, limit)
    } else {
        return Err(err("the solid's ACIS data has an unknown header"));
    };
    let limit = end_marker(data, start).map_or(limit, |end| end.min(limit));
    let records = cadmpeg_asm::sab::frame(&ctx, data, start, limit, header.width, None)
        .map_err(|e| err(format!("the solid's ACIS data can't be read ({e:?})")))?;
    let records = long_names(records);
    // The decoder reads binary lengths as centimetres and returns millimetres; a drawing takes
    // the coordinates as they are written, so that factor comes back off.
    let scale = BINARY_MM_PER_UNIT;
    let brep = cadmpeg_asm::brep::decode_with_header(
        &ctx,
        &records,
        data,
        Some(&header.metadata),
        "acis",
        format_id()?,
        cadmpeg_asm::brep::DecodePurpose::Model,
    )
    .map_err(decode_error)?;
    Ok((serde_json::to_value(&brep).map_err(decode_error)?, scale))
}

/// The records with ACIS 4's short subtype names spelt as the decoder looks them up.
///
/// Save format 400 (AutoCAD's DWG 2000, and what [`super::write`] writes for NURBS faces)
/// names a coedge's pcurve `exppc`; the decoder matches only the long `exp_par_cur`, and a
/// pcurve it doesn't match is lost, so the face can't be trimmed. Only a payload identifier
/// that is exactly the short name changes.
fn long_names(records: Vec<cadmpeg_asm::sab::Record>) -> Vec<cadmpeg_asm::sab::Record> {
    use cadmpeg_asm::sab::Token;
    const SHORT: [(&str, &str); 1] = [("exppc", "exp_par_cur")];
    let short = |t: &Token| matches!(t, Token::Ident(n) | Token::SubIdent(n) if SHORT.iter().any(|(s, _)| s == n));
    records
        .into_iter()
        .map(|mut r| {
            if r.tokens.iter().any(short) {
                let tokens: Vec<Token> = r
                    .tokens
                    .iter()
                    .map(|t| match t {
                        Token::Ident(n) => Token::Ident(SHORT.iter().find(|(s, _)| s == n).map_or_else(|| n.clone(), |(_, l)| (*l).to_owned())),
                        Token::SubIdent(n) => Token::SubIdent(SHORT.iter().find(|(s, _)| s == n).map_or_else(|| n.clone(), |(_, l)| (*l).to_owned())),
                        other => other.clone(),
                    })
                    .collect();
                r.tokens = tokens.into();
            }
            r
        })
        .collect()
}

/// The items of one list of the decoded B-rep, by id.
pub(super) fn by_id<'a>(v: &'a Value, list: &str) -> HashMap<&'a str, &'a Value> {
    v.get(list).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| Some((x.get("id")?.as_str()?, x))).collect()).unwrap_or_default()
}

pub(super) fn ids(v: Option<&Value>) -> Vec<&str> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64).filter(|x| x.is_finite())
}

fn xyz(v: &Value, key: &str) -> Option<P3> {
    let p = v.get(key)?;
    Some([num(p, "x")?, num(p, "y")?, num(p, "z")?])
}

/// The decoded B-rep as STEP geometry, lengths in the data's own units.
pub(super) struct Geometry {
    /// Millimetres per unit.
    mm: f64,
    /// The body's transform (ACIS's `transform` record): its linear part, its shift (in units)
    /// and its uniform scale.
    linear: [[f64; 3]; 3],
    shift: P3,
    scale: f64,
}

impl Geometry {
    pub(super) fn new(mm: f64) -> Geometry {
        Geometry { mm, linear: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], shift: [0.0; 3], scale: 1.0 }
    }

    /// The body's transform, from the decoded body's rows (row-major, applied to column points;
    /// the shift in the decoder's millimetres of the binary centimetre convention). A mirroring
    /// or flattening transform is refused.
    pub(super) fn transformed(&self, rows: Option<&Value>) -> Result<Geometry> {
        match self.placed(rows)? {
            (g, None) => Ok(g),
            (_, Some(_)) => Err(err("a mirrored or flattened body (not read yet)")),
        }
    }

    /// The body's transform as [`Geometry::transformed`], but a mirroring one (determinant below
    /// 0, MIRROR and MIRROR3D in AutoCAD) is not applied to the geometry: it comes back as the
    /// kernel's matrix (rows the images of the axes, row 3 the shift), for the kernel to mirror
    /// the body once it is made, which turns its faces and loops the right way round.
    pub(super) fn placed(&self, rows: Option<&Value>) -> Result<(Geometry, Option<[[f64; 4]; 4]>)> {
        let Some(rows) = rows.and_then(Value::as_array) else { return Ok((Geometry::new(self.mm), None)) };
        let mut m = [[0.0; 4]; 3];
        for (i, row) in rows.iter().take(3).enumerate() {
            for (j, v) in row.as_array().into_iter().flatten().take(4).enumerate() {
                let v = v.as_f64().filter(|v| v.is_finite()).ok_or_else(|| err("a body transform that is not a number"))?;
                if let Some(slot) = m.get_mut(i).and_then(|r| r.get_mut(j)) {
                    *slot = v;
                }
            }
        }
        let linear = [[m[0][0], m[0][1], m[0][2]], [m[1][0], m[1][1], m[1][2]], [m[2][0], m[2][1], m[2][2]]];
        let det = linear[0][0] * (linear[1][1] * linear[2][2] - linear[1][2] * linear[2][1])
            - linear[0][1] * (linear[1][0] * linear[2][2] - linear[1][2] * linear[2][0])
            + linear[0][2] * (linear[1][0] * linear[2][1] - linear[1][1] * linear[2][0]);
        let shift = [m[0][3] / BINARY_MM_PER_UNIT, m[1][3] / BINARY_MM_PER_UNIT, m[2][3] / BINARY_MM_PER_UNIT];
        if det.is_finite() && det < -1e-300 {
            // Rows of the kernel's matrix: the images of the axes (the columns of `linear`).
            let l = &linear;
            let mirror = [
                [l[0][0], l[1][0], l[2][0], 0.0],
                [l[0][1], l[1][1], l[2][1], 0.0],
                [l[0][2], l[1][2], l[2][2], 0.0],
                [shift[0], shift[1], shift[2], 1.0],
            ];
            return Ok((Geometry::new(self.mm), Some(mirror)));
        }
        if !(det.is_finite() && det > 1e-300) {
            return Err(err("a flattened body (its transform has no volume)"));
        }
        Ok((Geometry { mm: self.mm, linear, shift, scale: det.cbrt() }, None))
    }

    fn len(&self, v: f64) -> f64 {
        v / self.mm * self.scale
    }
    fn apply(&self, p: P3) -> P3 {
        let l = &self.linear;
        [
            l[0][0] * p[0] + l[0][1] * p[1] + l[0][2] * p[2],
            l[1][0] * p[0] + l[1][1] * p[1] + l[1][2] * p[2],
            l[2][0] * p[0] + l[2][1] * p[1] + l[2][2] * p[2],
        ]
    }
    pub(super) fn point(&self, v: &Value, key: &str) -> Option<P3> {
        let p = xyz(v, key)?;
        let q = self.apply([p[0] / self.mm, p[1] / self.mm, p[2] / self.mm]);
        Some(add(q, self.shift, 1.0))
    }
    fn direction(&self, v: &Value, key: &str) -> Option<P3> {
        unit(self.apply(xyz(v, key)?))
    }
    fn placement(&self, g: &Value, origin: &str, axis: &str, reference: &str) -> Option<Placement> {
        Some(Placement { origin: self.point(g, origin)?, axis: self.direction(g, axis)?, ref_direction: self.direction(g, reference)? })
    }

    pub(super) fn surface(&self, s: &Value) -> Result<Surface> {
        let g = solved(s.get("geometry"));
        let g = g.ok_or_else(|| err("a face without a surface"))?;
        let kind = g.get("kind").and_then(Value::as_str).unwrap_or("");
        let bad = || err(format!("a {kind} surface the reader can't take"));
        match kind {
            "plane" => Ok(Surface::Plane { position: self.placement(g, "origin", "normal", "u_axis").ok_or_else(bad)? }),
            "cylinder" => Ok(Surface::Cylinder {
                position: self.placement(g, "origin", "axis", "ref_direction").ok_or_else(bad)?,
                radius: self.len(num(g, "radius").ok_or_else(bad)?),
            }),
            "cone" => {
                let position = self.placement(g, "origin", "axis", "ref_direction").ok_or_else(bad)?;
                let radius = self.len(num(g, "radius").ok_or_else(bad)?);
                let ratio = num(g, "ratio").unwrap_or(1.0);
                let half = num(g, "half_angle").ok_or_else(bad)?;
                if (ratio - 1.0).abs() > 1e-9 {
                    return Err(err("an elliptical cone (not read yet)"));
                }
                if half.abs() < 1e-12 { Ok(Surface::Cylinder { position, radius }) } else { Ok(Surface::Cone { position, radius, semi_angle: half }) }
            }
            "sphere" => Ok(Surface::Sphere {
                position: self.placement(g, "center", "axis", "ref_direction").ok_or_else(bad)?,
                radius: self.len(num(g, "radius").ok_or_else(bad)?),
            }),
            "torus" => Ok(Surface::Torus {
                position: self.placement(g, "center", "axis", "ref_direction").ok_or_else(bad)?,
                major_radius: self.len(num(g, "major_radius").ok_or_else(bad)?),
                minor_radius: self.len(num(g, "minor_radius").ok_or_else(bad)?),
            }),
            "nurbs" => self.nurbs_surface(g).ok_or_else(bad),
            _ => Err(bad()),
        }
    }

    pub(super) fn curve(&self, c: &Value, procedural: &HashMap<&str, &Value>) -> Result<Option<Curve>> {
        let geometry = c.get("geometry");
        if let Some(def) = geometry.filter(|g| g.get("cache").is_none()).and_then(|g| g.get("construction")).and_then(Value::as_str)
            && let Some(def) = procedural.get(def).and_then(|p| p.get("definition"))
        {
            return self.procedural_curve(def).map(Some);
        }
        let g = solved(geometry).ok_or_else(|| err("an edge without a curve"))?;
        let kind = g.get("kind").and_then(Value::as_str).unwrap_or("");
        let bad = || err(format!("a {kind} curve the reader can't take"));
        Ok(Some(match kind {
            "line" => Curve::Line {
                origin: self.point(g, "origin").ok_or_else(bad)?,
                direction: self.direction(g, "direction").ok_or_else(bad)?,
                magnitude: 1.0,
            },
            "circle" => Curve::Circle {
                position: self.placement(g, "center", "axis", "ref_direction").ok_or_else(bad)?,
                radius: self.len(num(g, "radius").ok_or_else(bad)?),
            },
            "ellipse" => Curve::Ellipse {
                position: self.placement(g, "center", "axis", "ref_direction").ok_or_else(bad)?,
                semi_axis_1: self.len(num(g, "major_radius").ok_or_else(bad)?),
                semi_axis_2: self.len(num(g, "minor_radius").ok_or_else(bad)?),
            },
            "degenerate" => return Ok(None),
            "nurbs" => self.nurbs_curve(g).ok_or_else(bad)?,
            _ => return Err(bad()),
        }))
    }

    /// A length vector (no shift).
    fn vector(&self, v: &Value, key: &str) -> Option<P3> {
        let p = xyz(v, key)?;
        Some(self.apply([p[0] / self.mm, p[1] / self.mm, p[2] / self.mm]))
    }

    /// A procedural curve with no cached form: a helix (SWEEP along HELIX), as a cubic B-spline
    /// through it whose parameter is the helix's angle at every knot, so ACIS's pcurves, which
    /// run on that angle, fit it.
    fn procedural_curve(&self, def: &Value) -> Result<Curve> {
        let kind = def.get("kind").and_then(Value::as_str).unwrap_or("");
        let bad = || err(format!("a {kind} curve the reader can't take"));
        if kind != "helix" {
            return Err(bad());
        }
        if num(def, "apex_factor").is_some_and(|a| a.abs() > 1e-12) {
            return Err(err("a tapered helix (not read yet)"));
        }
        let center = self.point(def, "center").ok_or_else(bad)?;
        let (major, minor, pitch) =
            (self.vector(def, "major").ok_or_else(bad)?, self.vector(def, "minor").ok_or_else(bad)?, self.vector(def, "pitch").ok_or_else(bad)?);
        let range = def.get("angle_range").and_then(Value::as_array).ok_or_else(bad)?;
        let (t0, t1) = (range.first().and_then(Value::as_f64).ok_or_else(bad)?, range.get(1).and_then(Value::as_f64).ok_or_else(bad)?);
        if !(t0.is_finite() && t1.is_finite() && t1 > t0) {
            return Err(bad());
        }
        helix_spline(center, major, minor, pitch, t0, t1).ok_or_else(bad)
    }

    /// A pole: a point (`polynomial`) or a point and its weight (`rational`).
    fn pole(&self, v: &Value) -> Option<(P3, Option<f64>)> {
        match v.get("point") {
            Some(_) => Some((self.point(v, "point")?, Some(num(v, "weight").filter(|w| *w > 0.0)?))),
            None => Some((self.point(&serde_json::json!({ "p": v }), "p")?, None)),
        }
    }

    /// Poles in a row, with their weights when every pole has one.
    fn poles(&self, row: &[Value]) -> Option<(Vec<P3>, Option<Vec<f64>>)> {
        let mut points = Vec::with_capacity(row.len());
        let mut weights = Vec::with_capacity(row.len());
        for v in row {
            let (p, w) = self.pole(v)?;
            points.push(p);
            weights.extend(w);
        }
        let weights = (weights.len() == points.len() && !points.is_empty()).then_some(weights);
        Some((points, weights))
    }

    fn nurbs_curve(&self, g: &Value) -> Option<Curve> {
        let degree = usize::try_from(g.get("degree")?.as_u64()?).ok().filter(|d| (1..=MAX_DEGREE).contains(d))?;
        let row = g.get("poles")?.get("points")?.as_array()?;
        if row.len() > MAX_POLES {
            return None;
        }
        let (points, weights) = self.poles(row)?;
        let full: Vec<f64> = g.get("knots")?.as_array()?.iter().map(Value::as_f64).collect::<Option<_>>()?;
        if full.len() != points.len() + degree + 1 {
            return None;
        }
        let (knots, multiplicities) = distinct(&full);
        Some(Curve::BSpline(Box::new(BSplineCurve {
            degree,
            points,
            weights,
            knots,
            multiplicities,
            form: CurveForm::Unspecified,
            closed: Logical::Unknown,
            self_intersect: Logical::Unknown,
            knot_spec: KnotSpec::Unspecified,
        })))
    }

    fn nurbs_surface(&self, g: &Value) -> Option<Surface> {
        let ud = usize::try_from(g.get("u_degree")?.as_u64()?).ok().filter(|d| (1..=MAX_DEGREE).contains(d))?;
        let vd = usize::try_from(g.get("v_degree")?.as_u64()?).ok().filter(|d| (1..=MAX_DEGREE).contains(d))?;
        let rows = g.get("poles")?.get("rows")?.as_array()?;
        let (mut points, mut weights, mut rational) = (Vec::new(), Vec::new(), true);
        let mut total = 0usize;
        for row in rows {
            let row = row.as_array()?;
            total = total.checked_add(row.len())?;
            if total > MAX_POLES {
                return None;
            }
            let (p, w) = self.poles(row)?;
            if points.first().is_some_and(|f: &Vec<P3>| f.len() != p.len()) {
                return None;
            }
            match w {
                Some(w) => weights.push(w),
                None => rational = false,
            }
            points.push(p);
        }
        let uf: Vec<f64> = g.get("u_knots")?.as_array()?.iter().map(Value::as_f64).collect::<Option<_>>()?;
        let vf: Vec<f64> = g.get("v_knots")?.as_array()?.iter().map(Value::as_f64).collect::<Option<_>>()?;
        let columns = points.first().map_or(0, Vec::len);
        if uf.len() != points.len() + ud + 1 || vf.len() != columns + vd + 1 {
            return None;
        }
        let (u_knots, u_multiplicities) = distinct(&uf);
        let (v_knots, v_multiplicities) = distinct(&vf);
        Some(Surface::BSpline(Box::new(BSplineSurface {
            u_degree: ud,
            v_degree: vd,
            points,
            weights: rational.then_some(weights),
            u_knots,
            u_multiplicities,
            v_knots,
            v_multiplicities,
            form: SurfaceForm::Unspecified,
            u_closed: Logical::Unknown,
            v_closed: Logical::Unknown,
            self_intersect: Logical::Unknown,
            knot_spec: KnotSpec::Unspecified,
        })))
    }
}

/// Highest B-spline degree read, and most poles of one curve or surface.
const MAX_DEGREE: usize = 25;
const MAX_POLES: usize = 1 << 20;

/// A carrier's solved geometry: its own, or the cache of a procedural one.
fn solved(g: Option<&Value>) -> Option<&Value> {
    let g = g?;
    if g.get("construction").is_some() { g.get("cache") } else { Some(g) }
}

/// Full knots as distinct knots and multiplicities.
fn distinct(full: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let (mut knots, mut mults) = (Vec::new(), Vec::<usize>::new());
    for &k in full {
        match (knots.last(), mults.last_mut()) {
            (Some(last), Some(m)) if *last == k => *m += 1,
            _ => {
                knots.push(k);
                mults.push(1);
            }
        }
    }
    (knots, mults)
}

/// Nudges tried, each way, to land a closed circle edge's vertex exactly on its circle's angle 0.
const CLOSED_CIRCLE_TRIES: i32 = 64;

/// Spans of a helix's spline per turn: a cubic through 64 points a turn strays less than a
/// millionth of the radius.
const HELIX_SPANS_PER_TURN: f64 = 64.0;
/// Most spans of one helix.
const MAX_HELIX_SPANS: usize = 1 << 16;

/// A helix `center + major cos t + minor sin t + pitch t / 2π` on `[t0, t1]` as a cubic B-spline:
/// a Bézier segment per span, its inner poles from the helix's tangents (Hermite), each knot the
/// angle there.
fn helix_spline(center: P3, major: P3, minor: P3, pitch: P3, t0: f64, t1: f64) -> Option<Curve> {
    let turns = (t1 - t0) / std::f64::consts::TAU;
    let spans = (turns * HELIX_SPANS_PER_TURN).ceil();
    if !(spans.is_finite() && spans >= 1.0 && spans <= MAX_HELIX_SPANS as f64) {
        return None;
    }
    let n = spans as usize;
    let k = 1.0 / std::f64::consts::TAU;
    let at = |t: f64| add(add(add(center, major, t.cos()), minor, t.sin()), pitch, t * k);
    let tangent = |t: f64| add(add(scale(major, -t.sin()), minor, t.cos()), pitch, k);
    let h = (t1 - t0) / spans;
    let mut points = vec![at(t0)];
    let mut knots = vec![t0];
    let mut multiplicities = vec![4];
    for i in 0..n {
        let (a, b) = (t0 + h * i as f64, if i + 1 == n { t1 } else { t0 + h * (i + 1) as f64 });
        points.push(add(at(a), tangent(a), h / 3.0));
        points.push(add(at(b), tangent(b), -h / 3.0));
        points.push(at(b));
        knots.push(b);
        multiplicities.push(if i + 1 == n { 4 } else { 3 });
    }
    Some(Curve::BSpline(Box::new(BSplineCurve {
        degree: 3,
        points,
        weights: None,
        knots,
        multiplicities,
        form: CurveForm::Unspecified,
        closed: Logical::False,
        self_intersect: Logical::False,
        knot_spec: KnotSpec::Unspecified,
    })))
}

fn scale(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// An ACIS pcurve as the kernel takes it: on its edge's interval, in its edge's direction. A
/// reversed coedge's pcurve runs on the negated parameter (`p(-t)`), so its poles and knots turn
/// round. `None` when it isn't a B-spline.
fn pcurve(p: &Value, reversed: bool) -> Option<Curve2> {
    let meta = p.get("metadata").and_then(|m| m.get("form"));
    if meta.and_then(|m| m.get("wrapper_reversed")).and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let offset = meta.and_then(|m| m.get("uv_offset")).and_then(Value::as_array);
    let off = |i: usize| offset.and_then(|o| o.get(i)).and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(0.0);
    let (du, dv) = (off(0), off(1));
    let g = p.get("geometry")?;
    // A pcurve marked periodic (one that goes once round a closed surface) comes with clamped
    // knots: read as an ordinary B-spline.
    if g.get("kind").and_then(Value::as_str) != Some("nurbs") {
        return None;
    }
    let degree = usize::try_from(g.get("degree")?.as_u64()?).ok().filter(|d| (1..=MAX_DEGREE).contains(d))?;
    let rows = g.get("poles")?.get("points")?.as_array()?;
    if rows.len() > MAX_POLES {
        return None;
    }
    let mut knots: Vec<f64> = g.get("knots")?.as_array()?.iter().map(|k| k.as_f64().filter(|k| k.is_finite())).collect::<Option<_>>()?;
    if knots.len() != rows.len().checked_add(degree)?.checked_add(1)? || knots.windows(2).any(|w| w[1] < w[0]) {
        return None;
    }
    let uv = |v: &Value| Some([v.get("u")?.as_f64().filter(|x| x.is_finite())? + du, v.get("v")?.as_f64().filter(|x| x.is_finite())? + dv]);
    let mut homogeneous: Vec<[f64; 3]> = Vec::with_capacity(rows.len());
    let mut rational = false;
    for v in rows {
        match v.get("point") {
            Some(q) => {
                let w = num(v, "weight").filter(|w| *w > 0.0)?;
                let [u, vv] = uv(q)?;
                rational = true;
                homogeneous.push([u * w, vv * w, w]);
            }
            None => {
                let [u, vv] = uv(v)?;
                homogeneous.push([u, vv, 1.0]);
            }
        }
    }
    if reversed {
        homogeneous.reverse();
        knots = knots.iter().rev().map(|k| -k).collect();
    }
    Some(if rational {
        Curve2::RationalSpline { knots, homogeneous }
    } else {
        Curve2::Spline { knots, points: homogeneous.iter().map(|h| [h[0], h[1]]).collect() }
    })
}

/// How a point of an edge gives its ACIS parameter: a line's distance along it, a circle's or an
/// ellipse's angle; a spline edge keeps its parameter (`None`).
#[derive(Clone, Copy, Debug)]
enum AcisParam {
    Line { origin: P3, direction: P3 },
    Conic { center: P3, x: P3, y: P3, a: f64, b: f64 },
}

impl AcisParam {
    fn of(c: &Curve) -> Option<AcisParam> {
        match c {
            Curve::Line { origin, direction, .. } => Some(AcisParam::Line { origin: *origin, direction: unit(*direction)? }),
            Curve::Circle { position, radius } => AcisParam::conic(position, *radius, *radius),
            Curve::Ellipse { position, semi_axis_1, semi_axis_2 } => AcisParam::conic(position, *semi_axis_1, *semi_axis_2),
            _ => None,
        }
    }
    fn conic(p: &Placement, a: f64, b: f64) -> Option<AcisParam> {
        let (z, x) = (unit(p.axis)?, unit(p.ref_direction)?);
        (a > 0.0 && b > 0.0).then(|| AcisParam::Conic { center: p.origin, x, y: cross(z, x), a, b })
    }
    /// The parameter of `q`, an angle taken within π of `near`.
    fn at(&self, q: P3, near: f64) -> f64 {
        let d = |o: P3| [q[0] - o[0], q[1] - o[1], q[2] - o[2]];
        let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        match *self {
            AcisParam::Line { origin, direction } => dot(d(origin), direction),
            AcisParam::Conic { center, x, y, a, b } => {
                let v = d(center);
                let t = (dot(v, y) / b).atan2(dot(v, x) / a);
                let turn = std::f64::consts::TAU;
                t + ((near - t) / turn).round() * turn
            }
        }
    }
}

/// How an edge's points give their ACIS parameter, and the edge's ACIS parameter range in its
/// direction: a pcurve may run on a range of its own (a loft's side on [0, 1] along an edge
/// 80 long), linear in the edge's parameter.
#[derive(Clone, Copy, Debug)]
struct EdgeParam {
    how: AcisParam,
    range: [f64; 2],
}

/// The ACIS pcurves of one face on a spline surface: each use's edge ends, whether the loop runs
/// along the edge, its pcurve, and how its edge's points give their ACIS parameter.
struct FacePcurves {
    uses: Vec<(P3, P3, bool, Curve2, Option<EdgeParam>)>,
}

/// The point of a B-spline at `t` (de Boor), its poles homogeneous or not.
fn de_boor<const N: usize>(knots: &[f64], poles: &[[f64; N]], t: f64) -> Option<[f64; N]> {
    let degree = knots.len().checked_sub(poles.len().checked_add(1)?)?;
    let (lo, hi) = (*knots.get(degree)?, *knots.get(poles.len())?);
    if !(t.is_finite() && hi > lo) {
        return None;
    }
    let t = t.clamp(lo, hi);
    // The span: knots[k] <= t < knots[k + 1], within [degree, poles - 1].
    let mut k = degree;
    while k + 1 < poles.len() && *knots.get(k + 1)? <= t {
        k += 1;
    }
    let mut d: Vec<[f64; N]> = (0..=degree).map(|j| poles.get(k + j - degree).copied()).collect::<Option<_>>()?;
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = k + j - degree;
            let (a, b) = (*knots.get(i)?, *knots.get(i + degree + 1 - r)?);
            let alpha = if b > a { (t - a) / (b - a) } else { 0.0 };
            let (prev, cur) = (*d.get(j - 1)?, *d.get(j)?);
            let slot = d.get_mut(j)?;
            for n in 0..N {
                slot[n] = (1.0 - alpha) * prev[n] + alpha * cur[n];
            }
            let _ = cur;
        }
    }
    d.get(degree).copied()
}

/// The point of the kernel's edge curve at `t`.
fn record_point(c: &solidcore_record::brep::Curve, t: f64) -> Option<P3> {
    use solidcore_record::brep::Curve as C;
    match c {
        C::Line { start, end } => Some(add(*start, [end[0] - start[0], end[1] - start[1], end[2] - start[2]], t)),
        C::Circle { frame, radius } => Some(add(add(frame.origin, frame.x_axis, radius * t.cos()), frame.y_axis, radius * t.sin())),
        C::Ellipse { frame, radii } => Some(add(add(frame.origin, frame.x_axis, radii[0] * t.cos()), frame.y_axis, radii[1] * t.sin())),
        C::Spline { knots, points } => de_boor(knots, points, t),
        C::RationalSpline { knots, homogeneous } => {
            let h = de_boor(knots, homogeneous, t)?;
            (h[3].abs() > 1e-300).then(|| [h[0] / h[3], h[1] / h[3], h[2] / h[3]])
        }
        _ => None,
    }
}

/// The (u, v) of a pcurve at `t`.
fn uv_at(c: &Curve2, t: f64) -> Option<[f64; 2]> {
    match c {
        Curve2::Spline { knots, points } => de_boor(knots, points, t),
        Curve2::RationalSpline { knots, homogeneous } => {
            let h = de_boor(knots, homogeneous, t)?;
            (h[2].abs() > 1e-300).then(|| [h[0] / h[2], h[1] / h[2]])
        }
    }
}

/// Spans of a pcurve moved onto the kernel's parameter.
const REMAP_SPANS: usize = 32;

/// An ACIS pcurve on the kernel's parameter of its edge: for each kernel parameter `s`, the edge's
/// point there, its ACIS parameter, and the ACIS pcurve's (u, v) at that; a cubic through those
/// with their tangents (Hermite), a Bézier segment per span.
fn remapped(given: &Curve2, param: &EdgeParam, edge: &solidcore_record::brep::Curve, [t0, t1]: [f64; 2]) -> Option<Curve2> {
    let [a, b] = given.interval()?;
    let [e0, e1] = param.range;
    if !(t1 > t0 && (e1 - e0).abs() > 1e-300) {
        return None;
    }
    // `near` and the returned value are the edge's ACIS parameter; the pcurve's is linear in it.
    let f = |s: f64, near: f64| -> Option<(f64, [f64; 2])> {
        let e = param.how.at(record_point(edge, s)?, near);
        let tau = (a + (e - e0) / (e1 - e0) * (b - a)).clamp(a.min(b), a.max(b));
        Some((e, uv_at(given, tau)?))
    };
    let h = (t1 - t0) / REMAP_SPANS as f64;
    let eps = h * 1e-4;
    let mut near = e0;
    let mut nodes: Vec<([f64; 2], [f64; 2])> = Vec::with_capacity(REMAP_SPANS + 1);
    for i in 0..=REMAP_SPANS {
        let s = if i == REMAP_SPANS { t1 } else { t0 + h * i as f64 };
        let (tau, p) = f(s, near)?;
        near = tau;
        let (lo, hi) = ((s - eps).max(t0), (s + eps).min(t1));
        let (_, p0) = f(lo, tau)?;
        let (_, p1) = f(hi, tau)?;
        let ds = hi - lo;
        nodes.push((p, [(p1[0] - p0[0]) / ds, (p1[1] - p0[1]) / ds]));
    }
    let mut points = Vec::with_capacity(3 * REMAP_SPANS + 1);
    let mut knots = vec![t0; 4];
    points.push(nodes.first()?.0);
    for i in 0..REMAP_SPANS {
        let ((p, dp), (q, dq)) = (*nodes.get(i)?, *nodes.get(i + 1)?);
        points.push([p[0] + dp[0] * h / 3.0, p[1] + dp[1] * h / 3.0]);
        points.push([q[0] - dq[0] * h / 3.0, q[1] - dq[1] * h / 3.0]);
        points.push(q);
        let s = if i + 1 == REMAP_SPANS { t1 } else { t0 + h * (i + 1) as f64 };
        knots.extend(std::iter::repeat_n(s, if i + 1 == REMAP_SPANS { 4 } else { 3 }));
    }
    Some(Curve2::Spline { knots, points })
}

fn s(v: &str) -> Param {
    Param::Str(v.into())
}
fn r(id: u64) -> Param {
    Param::Ref(id)
}
fn b(v: bool) -> Param {
    Param::Enum(if v { "T" } else { "F" }.into())
}
fn list(ids: &[u64]) -> Param {
    Param::List(ids.iter().map(|i| Param::Ref(*i)).collect())
}

fn add(a: P3, b: P3, k: f64) -> P3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The angle (radians, in [0, 2π)) solidcore's STEP reader gives point `p` on the circle of
/// `position` and `radius`, its numbers worked out as the reader works them: the axis and
/// reference made unit, the reference made square to the axis, the point's angle about them.
fn step_circle_angle(position: &Placement, radius: f64, p: P3) -> Option<f64> {
    let step_unit = |v: P3| {
        let m = dot(v, v).sqrt();
        (m > 1e-12 && m.is_finite()).then(|| scale(v, 1.0 / m))
    };
    let z = step_unit(position.axis)?;
    let r = step_unit(position.ref_direction)?;
    let x = add(r, z, -dot(r, z));
    let m = dot(x, x).sqrt();
    if !(m > 1e-9 && m.is_finite()) {
        return None;
    }
    let x = scale(x, 1.0 / m);
    let y = cross(z, x);
    let q = add(p, position.origin, -1.0);
    let t = (dot(q, y) / radius).atan2(dot(q, x) / radius);
    Some(if t < 0.0 { t + core::f64::consts::TAU } else { t })
}

/// A closed circle edge's circle with its reference through the edge's vertex `p`, nudged by
/// roundings until solidcore's STEP reader puts the vertex exactly on angle 0. Without one that lands, the reference through the vertex.
fn closed_circle_placement(position: &Placement, radius: f64, p: P3) -> Placement {
    let Some(z) = unit(position.axis) else {
        return *position;
    };
    let v = add(p, position.origin, -1.0);
    let Some(x) = unit(add(v, z, -dot(v, z))) else {
        return *position;
    };
    let side = cross(z, x);
    // 0, or above it by less than half a rounding of a whole turn (so a turn on still ends at 2π).
    let lands = |t: f64| (0.0..=f64::EPSILON).contains(&t);
    for k in 0..=CLOSED_CIRCLE_TRIES {
        for sign in [1.0, -1.0] {
            let nudge = f64::from(k) * sign * f64::EPSILON;
            let Some(ref_direction) = unit(add(x, side, nudge)) else {
                continue;
            };
            let candidate = Placement { ref_direction, ..*position };
            if step_circle_angle(&candidate, radius, p).is_some_and(lands) {
                return candidate;
            }
        }
    }
    Placement { ref_direction: x, ..*position }
}

fn unit(a: P3) -> Option<P3> {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (l.is_finite() && l > 1e-300).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// The seams that bound a whole sphere or torus, as solidcore writes them, and the points that
/// box the surface: a sphere's meridian from pole to pole, used both ways; a torus's meridian and
/// its outer equator, each used both ways.
fn seam_loop(w: &mut Writer, surface: &Surface) -> Option<(u64, Vec<P3>)> {
    let (position, outer, minor) = match surface {
        Surface::Sphere { position, radius } => (position, *radius, None),
        Surface::Torus { position, major_radius, minor_radius } => (position, major_radius + minor_radius, Some((*major_radius, *minor_radius))),
        _ => return None,
    };
    let (c, z, x) = (position.origin, unit(position.axis)?, unit(position.ref_direction)?);
    // The meridian's plane holds the axis and the reference direction; its normal is x × z.
    let normal = cross(x, z);
    let extent: Vec<P3> = [x, z, cross(z, x)].iter().flat_map(|d| [add(c, *d, outer), add(c, *d, -outer)]).collect();
    let mut g = GeomWriter::new(w);
    let loop_id = match minor {
        None => {
            let south = g.point(add(c, z, -outer));
            let north = g.point(add(c, z, outer));
            let meridian = g.curve(&Curve::Circle { position: Placement { origin: c, axis: normal, ref_direction: x }, radius: outer });
            let south = w.add_distinct("VERTEX_POINT", &[s(""), r(south)]);
            let north = w.add_distinct("VERTEX_POINT", &[s(""), r(north)]);
            let e = w.add_distinct("EDGE_CURVE", &[s(""), r(south), r(north), r(meridian), b(true)]);
            let back = w.add_distinct("ORIENTED_EDGE", &[s(""), Param::Derived, Param::Derived, r(e), b(false)]);
            let on = w.add_distinct("ORIENTED_EDGE", &[s(""), Param::Derived, Param::Derived, r(e), b(true)]);
            w.add_distinct("EDGE_LOOP", &[s(""), list(&[back, on])])
        }
        Some((major, small)) => {
            let at = g.point(add(c, x, outer));
            let meridian =
                g.curve(&Curve::Circle { position: Placement { origin: add(c, x, major), axis: normal, ref_direction: x }, radius: small });
            let equator = g.curve(&Curve::Circle { position: Placement { origin: c, axis: z, ref_direction: x }, radius: outer });
            let v = w.add_distinct("VERTEX_POINT", &[s(""), r(at)]);
            let m = w.add_distinct("EDGE_CURVE", &[s(""), r(v), r(v), r(meridian), b(true)]);
            let e = w.add_distinct("EDGE_CURVE", &[s(""), r(v), r(v), r(equator), b(true)]);
            let uses: Vec<u64> = [(m, false), (e, true), (m, true), (e, false)]
                .iter()
                .map(|(id, forward)| w.add_distinct("ORIENTED_EDGE", &[s(""), Param::Derived, Param::Derived, r(*id), b(*forward)]))
                .collect();
            w.add_distinct("EDGE_LOOP", &[s(""), list(&uses)])
        }
    };
    Some((loop_id, extent))
}

/// An edge's ACIS parameter: a line's range from its ends `ends` (the data's is in the decoder's
/// lengths), a conic's angles as the data has them.
fn edge_param_of(c: Option<&Curve>, e: &Value, ends: Option<&(P3, P3)>) -> Option<EdgeParam> {
    let how = AcisParam::of(c?)?;
    let range = match how {
        AcisParam::Line { .. } => {
            let (pa, pz) = ends?;
            [how.at(*pa, 0.0), how.at(*pz, 0.0)]
        }
        AcisParam::Conic { .. } => match e.get("carrier").and_then(|c| c.get("param_range")).and_then(Value::as_array) {
            Some(r) => [r.first()?.as_f64()?, r.get(1)?.as_f64()?],
            // Save format 400 keeps no parameter range on an edge (AutoCAD's DWG 2000): the
            // angles of its ends, going round the way the edge runs (along its curve), a whole
            // turn when they meet.
            None => {
                let (pa, pz) = ends?;
                let a = how.at(*pa, 0.0);
                let mut z = how.at(*pz, a + std::f64::consts::PI);
                if z <= a + 1e-12 {
                    z += std::f64::consts::TAU;
                }
                [a, z]
            }
        },
    };
    Some(EdgeParam { how, range })
}

/// An edge's curve, a closed circle edge's (`closed`, from vertex `start`) starting at its
/// circle's angle 0: the kernel takes a full turn from the vertex's angle in [0, 2π), so a
/// vertex a rounding below 0 would start it a turn later, and one a rounding above leaves a
/// sliver it can't certify.
fn closed_circle_edge(c: Option<Curve>, closed: bool, start: Option<P3>) -> Option<Curve> {
    match (c, start) {
        (Some(Curve::Circle { position, radius }), Some(p)) if closed => {
            Some(Curve::Circle { position: closed_circle_placement(&position, radius, p), radius })
        }
        (c, _) => c,
    }
}

/// A coedge's first ACIS pcurve, run its way (`reversed` against its edge).
fn coedge_pcurve(co: &Value, pcurves: &HashMap<&str, &Value>, reversed: bool) -> Option<Curve2> {
    let id = co.get("pcurves")?.as_array()?.first()?.get("pcurve")?.as_str()?;
    pcurve(pcurves.get(id)?, reversed)
}

/// The box `lo`..`hi` grown to take in `p`.
fn grow(lo: &mut [f64; 3], hi: &mut [f64; 3], p: P3) {
    for k in 0..3 {
        lo[k] = lo[k].min(p[k]);
        hi[k] = hi[k].max(p[k]);
    }
}

/// The decoded B-rep's solids as STEP text.
fn step(v: &Value, mm: f64) -> Result<(String, Vec<Owner>, Vec<FacePcurves>)> {
    let unplaced = Geometry::new(mm);
    let faces = by_id(v, "faces");
    if faces.len() > MAX_FACES {
        return Err(err("the solid has too many faces"));
    }
    let (loops, coedges, edges, vertices, points, surfaces, curves, shells, regions) = (
        by_id(v, "loops"),
        by_id(v, "coedges"),
        by_id(v, "edges"),
        by_id(v, "vertices"),
        by_id(v, "points"),
        by_id(v, "surfaces"),
        by_id(v, "curves"),
        by_id(v, "shells"),
        by_id(v, "regions"),
    );
    let pcurves = by_id(v, "pcurves");
    // Procedural curves by their own id (the list holds [carrier id, curve] pairs).
    let procedural: HashMap<&str, &Value> = v
        .get("procedural_curves")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.get(1).or(Some(x))).filter_map(|x| Some((x.get("id")?.as_str()?, x))).collect())
        .unwrap_or_default();
    let mut face_pcurves: Vec<FacePcurves> = Vec::new();
    let mut edge_ends: HashMap<&str, (P3, P3)> = HashMap::new();
    let mut edge_param: HashMap<&str, Option<EdgeParam>> = HashMap::new();
    let mut w = Writer::new();
    let mut vertex_ids: HashMap<String, u64> = HashMap::new();
    let mut edge_ids: HashMap<&str, Option<u64>> = HashMap::new();
    let mut solids = Vec::new();

    let bodies = v.get("bodies").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut owners: Vec<Owner> = Vec::new();
    for (body_index, body) in bodies.iter().enumerate() {
        if body.get("kind").and_then(Value::as_str) != Some("solid") {
            continue;
        }
        let (geometry, mirror) = unplaced.placed(body.get("transform"))?;
        for region in ids(body.get("regions")) {
            let Some(region) = regions.get(region) else { continue };
            // Each closed shell: its STEP shell, and the size of its box (the outer one is the
            // largest).
            let mut made: Vec<(u64, f64)> = Vec::new();
            for shell in ids(region.get("shells")) {
                let Some(shell) = shells.get(shell) else { continue };
                let mut face_ids = Vec::new();
                let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
                for member in shell.get("members").and_then(Value::as_array).into_iter().flatten() {
                    if member.get("kind").and_then(Value::as_str) != Some("face") {
                        continue;
                    }
                    let Some(face) = member.get("id").and_then(Value::as_str).and_then(|f| faces.get(f)) else { continue };
                    let surface =
                        face.get("surface").and_then(Value::as_str).and_then(|s| surfaces.get(s)).ok_or_else(|| err("a face without its surface"))?;
                    let surface_geometry = geometry.surface(surface)?;
                    let surface = GeomWriter::new(&mut w).surface(&surface_geometry);
                    let mut bounds = Vec::new();
                    let spline = matches!(surface_geometry, Surface::BSpline(_));
                    let mut given = FacePcurves { uses: Vec::new() };
                    for lp in ids(face.get("loops").and_then(|l| l.get("loops"))) {
                        let Some(lp) = loops.get(lp) else { continue };
                        let mut oriented = Vec::new();
                        let mut apex = None;
                        for co in ids(lp.get("boundary").and_then(|b| b.get("coedges"))) {
                            let Some(co) = coedges.get(co) else { continue };
                            let Some(edge_key) = co.get("edge").and_then(Value::as_str) else { continue };
                            let edge = match edge_ids.get(edge_key) {
                                Some(e) => *e,
                                None => {
                                    let e = edges.get(edge_key).ok_or_else(|| err("a coedge without its edge"))?;
                                    let mut vertex = |key: Option<&Value>| -> Result<u64> {
                                        let key = key.and_then(Value::as_str).ok_or_else(|| err("an edge without its vertex"))?;
                                        if let Some(id) = vertex_ids.get(key) {
                                            return Ok(*id);
                                        }
                                        let p = vertices
                                            .get(key)
                                            .and_then(|x| x.get("point"))
                                            .and_then(Value::as_str)
                                            .and_then(|p| points.get(p))
                                            .and_then(|p| geometry.point(p, "position"))
                                            .ok_or_else(|| err("a vertex without its point"))?;
                                        let pt = GeomWriter::new(&mut w).point(p);
                                        let id = w.add_distinct("VERTEX_POINT", &[s(""), r(pt)]);
                                        vertex_ids.insert(key.to_string(), id);
                                        Ok(id)
                                    };
                                    let (a, z) = (vertex(e.get("start"))?, vertex(e.get("end"))?);
                                    let at = |key: Option<&Value>| {
                                        key.and_then(Value::as_str)
                                            .and_then(|k| vertices.get(k))
                                            .and_then(|x| x.get("point"))
                                            .and_then(Value::as_str)
                                            .and_then(|p| points.get(p))
                                            .and_then(|p| geometry.point(p, "position"))
                                    };
                                    if let (Some(pa), Some(pz)) = (at(e.get("start")), at(e.get("end"))) {
                                        edge_ends.insert(edge_key, (pa, pz));
                                    }
                                    let carrier = e.get("carrier").and_then(|c| c.get("curve")).and_then(Value::as_str);
                                    let c = match carrier.and_then(|c| curves.get(c)) {
                                        Some(c) => geometry.curve(c, &procedural)?,
                                        None => None,
                                    };
                                    edge_param.insert(edge_key, edge_param_of(c.as_ref(), e, edge_ends.get(edge_key)));
                                    let c = closed_circle_edge(c, a == z, edge_ends.get(edge_key).map(|(pa, _)| *pa));
                                    let made = match c {
                                        Some(c) => {
                                            let c = GeomWriter::new(&mut w).curve(&c);
                                            Some(w.add_distinct("EDGE_CURVE", &[s(""), r(a), r(z), r(c), b(true)]))
                                        }
                                        // A point edge (an apex): its vertex only.
                                        None => {
                                            apex = Some(a);
                                            None
                                        }
                                    };
                                    edge_ids.insert(edge_key, made);
                                    made
                                }
                            };
                            match edge {
                                Some(e) => {
                                    let forward = co.get("sense").and_then(Value::as_str) != Some("reversed");
                                    if spline
                                        && let Some(c2) = coedge_pcurve(co, &pcurves, !forward)
                                        && let Some((pa, pz)) = edge_ends.get(edge_key)
                                    {
                                        given.uses.push((*pa, *pz, forward, c2, edge_param.get(edge_key).copied().flatten()));
                                    }
                                    oriented.push(w.add_distinct("ORIENTED_EDGE", &[s(""), Param::Derived, Param::Derived, r(e), b(forward)]));
                                }
                                None if apex.is_none() => {
                                    if let Some(vx) =
                                        edges.get(edge_key).and_then(|e| e.get("start")).and_then(Value::as_str).and_then(|k| vertex_ids.get(k))
                                    {
                                        apex = Some(*vx);
                                    }
                                }
                                None => {}
                            }
                        }
                        let lp = match (oriented.is_empty(), apex) {
                            (false, _) => w.add_distinct("EDGE_LOOP", &[s(""), list(&oriented)]),
                            (true, Some(vx)) => w.add_distinct("VERTEX_LOOP", &[s(""), r(vx)]),
                            (true, None) => continue,
                        };
                        bounds.push(w.add_distinct("FACE_BOUND", &[s(""), r(lp), b(true)]));
                    }
                    // A sphere or a torus face with no loops is the whole surface: STEP bounds it
                    // with its seams.
                    if bounds.is_empty()
                        && let Some((lp, extent)) = seam_loop(&mut w, &surface_geometry)
                    {
                        bounds.push(w.add_distinct("FACE_BOUND", &[s(""), r(lp), b(true)]));
                        for p in extent {
                            grow(&mut lo, &mut hi, p);
                        }
                    }
                    if !given.uses.is_empty() {
                        face_pcurves.push(given);
                    }
                    let same_sense = face.get("sense").and_then(Value::as_str) != Some("reversed");
                    face_ids.push(w.add_distinct("ADVANCED_FACE", &[s(""), list(&bounds), r(surface), b(same_sense)]));
                    // The face's box, from its edges' vertices.
                    for lp in ids(face.get("loops").and_then(|l| l.get("loops"))) {
                        for co in ids(loops.get(lp).and_then(|l| l.get("boundary")).and_then(|b| b.get("coedges"))) {
                            let e = coedges.get(co).and_then(|c| c.get("edge")).and_then(Value::as_str).and_then(|e| edges.get(e));
                            for end in ["start", "end"] {
                                let p = e
                                    .and_then(|e| e.get(end))
                                    .and_then(Value::as_str)
                                    .and_then(|k| vertices.get(k))
                                    .and_then(|x| x.get("point"))
                                    .and_then(Value::as_str)
                                    .and_then(|p| points.get(p))
                                    .and_then(|p| geometry.point(p, "position"));
                                if let Some(p) = p {
                                    grow(&mut lo, &mut hi, p);
                                }
                            }
                        }
                    }
                }
                if face_ids.is_empty() {
                    continue;
                }
                let size = (0..3).map(|k| (hi[k] - lo[k]).max(0.0)).product::<f64>();
                made.push((w.add_distinct("CLOSED_SHELL", &[s(""), list(&face_ids)]), if size.is_finite() { size } else { f64::INFINITY }));
            }
            made.sort_by(|a, b| b.1.total_cmp(&a.1));
            let Some((outer, _)) = made.first().copied() else { continue };
            let solid = if made.len() == 1 {
                w.add_distinct("MANIFOLD_SOLID_BREP", &[s(""), r(outer)])
            } else {
                let voids: Vec<u64> =
                    made.iter().skip(1).map(|(v, _)| w.add_distinct("ORIENTED_CLOSED_SHELL", &[s(""), Param::Derived, r(*v), b(true)])).collect();
                w.add_distinct("BREP_WITH_VOIDS", &[s(""), r(outer), list(&voids)])
            };
            solids.push(solid);
            owners.push(Owner { body: body_index, mirror });
        }
    }
    Ok((w.file("acis", "AUTOMOTIVE_DESIGN"), owners, face_pcurves))
}

/// The kernel making the pcurves STEP leaves out: ACIS's own where the data has them (a face on
/// a spline surface: the kernel has no exact pcurve of a spline edge there), else each use's
/// curve projected onto its face's surface.
struct Kernel {
    faces: Vec<FacePcurves>,
}

fn near(a: P3, b: P3) -> bool {
    let size = 1.0_f64.max(a.iter().chain(b.iter()).fold(0.0, |m, x| m.max(x.abs())));
    (0..3).all(|k| (a[k] - b[k]).abs() <= 1e-7 * size)
}

impl Kernel {
    /// The ACIS pcurve of each of the face's uses, from the face of the data that has one for
    /// every use (matched by their edge's ends and direction; a loop the kernel turned round
    /// matches with every direction flipped). Empty when no face of the data matches.
    fn given(&self, face: &TrimFace<'_>) -> Vec<Vec<Option<(Curve2, Option<EdgeParam>)>>> {
        let ends = |u: &solidcore_ops::pcurve::TrimUse| -> Option<(P3, P3)> {
            let e = face.edges.get(usize::try_from(u.edge).ok()?)?;
            let v = |i: u32| face.vertices.get(usize::try_from(i).ok()?).copied();
            Some((v(e.vertices[0])?, v(e.vertices[1])?))
        };
        let mut best: (usize, Vec<Vec<Option<(Curve2, Option<EdgeParam>)>>>) = (0, Vec::new());
        for f in &self.faces {
            for flip in [false, true] {
                let mut taken = vec![false; f.uses.len()];
                let mut found = 0usize;
                let picked: Vec<Vec<Option<(Curve2, Option<EdgeParam>)>>> = face
                    .loops
                    .iter()
                    .map(|l| {
                        l.iter()
                            .map(|u| {
                                let (a, z) = ends(u)?;
                                let i = f.uses.iter().enumerate().position(|(i, (pa, pz, fw, _, _))| {
                                    !taken.get(i).copied().unwrap_or(true) && near(*pa, a) && near(*pz, z) && (*fw != flip) == u.forward
                                })?;
                                if let Some(t) = taken.get_mut(i) {
                                    *t = true;
                                }
                                found += 1;
                                f.uses.get(i).map(|x| (x.3.clone(), x.4))
                            })
                            .collect()
                    })
                    .collect();
                // Only a face whose every use matches: another face sharing a few of its edges
                // is on another surface.
                let total: usize = face.loops.iter().map(Vec::len).sum();
                if found == total && found > best.0 {
                    best = (found, picked);
                }
            }
        }
        best.1
    }
}

impl Trims for Kernel {
    fn trims(&self, face: &TrimFace<'_>) -> std::result::Result<Vec<Vec<Curve2>>, String> {
        let surface = face.nurbs.unwrap_or(face.surface);
        let given = if self.faces.is_empty() { Vec::new() } else { self.given(face) };
        // ACIS's pcurves on the kernel's parameter of their edges, where that differs.
        let mut own: Vec<Vec<Option<Curve2>>> = Vec::with_capacity(face.loops.len());
        for (li, l) in face.loops.iter().enumerate() {
            let mut row = Vec::with_capacity(l.len());
            for (ui, u) in l.iter().enumerate() {
                let e = face.edges.get(usize::try_from(u.edge).map_err(|e| e.to_string())?).ok_or("an edge out of range")?;
                let pc = given.get(li).and_then(|g| g.get(ui)).and_then(Option::as_ref).and_then(|(c, how)| {
                    let [a, b] = c.interval()?;
                    let [t0, t1] = e.interval;
                    let slack = 1e-9 * t0.abs().max(t1.abs()).max(1.0);
                    if (a - t0).abs() <= slack && (b - t1).abs() <= slack {
                        return Some(c.clone());
                    }
                    remapped(c, how.as_ref()?, e.nurbs.unwrap_or(e.curve), e.interval)
                });
                row.push(pc);
            }
            own.push(row);
        }
        let mut loops = Vec::new();
        for (li, l) in face.loops.iter().enumerate() {
            let mut uses = Vec::new();
            for (ui, u) in l.iter().enumerate() {
                let e = face.edges.get(usize::try_from(u.edge).map_err(|e| e.to_string())?).ok_or("an edge out of range")?;
                let acis = own.get(li).and_then(|g| g.get(ui)).and_then(Option::as_ref);
                uses.push(EdgeUse {
                    edge: u.edge,
                    curve: e.nurbs.unwrap_or(e.curve),
                    interval: e.interval,
                    forward: u.forward,
                    tolerance: e.tolerance,
                    // ACIS's own first: a seam's two uses each have theirs, a period apart, where
                    // the closed form the kernel works out once puts both on one side.
                    given: acis.or(u.pcurve.as_ref()),
                });
            }
            loops.push(uses);
        }
        solidcore_ops::pcurve::pcurves_within(surface, &loops, face.tolerance, Some(face.domain)).map_err(|e| e.to_string())
    }
}

/// Which body of the data a STEP solid (one lump) comes from, and the mirror its body's
/// transform leaves for the kernel.
struct Owner {
    body: usize,
    mirror: Option<[[f64; 4]; 4]>,
}

/// Most lumps put together into one solid.
const MAX_LUMPS: usize = 10_000;

/// One kernel body per body of the data: its lumps (a 3DSOLID made of separate pieces, as
/// AutoCAD's UNION of solids apart leaves it) are put together into one body.
fn to_bodies(v: &Value, mm: f64) -> Result<Vec<Body>> {
    let (text, owners, faces) = step(v, mm)?;
    if owners.is_empty() {
        return Err(err("the ACIS data holds no solid"));
    }
    if owners.len() > MAX_LUMPS {
        return Err(err("the solid has too many separate pieces"));
    }
    let ex = solidcore_step::p21::parse(&text).map_err(|e| err(format!("the solid can't be read ({e})")))?;
    let read = solidcore_step::topology::read_solids_with(&ex, &Kernel { faces });
    if let Some(f) = read.failures.first() {
        return Err(err(format!("the solid can't be read ({})", f.message)));
    }
    if read.solids.len() != owners.len() {
        return Err(err("the solid can't be read (its pieces don't match its records)"));
    }
    let mut out: Vec<(usize, Body)> = Vec::new();
    for (solid, owner) in read.solids.iter().zip(&owners) {
        let (b, _) = ops::Body::from_record(&solid.brep, false)?;
        if let Some(m) = &owner.mirror {
            b.transform(m)?;
        }
        match out.last_mut() {
            Some((at, whole)) if *at == owner.body => {
                *whole = crate::body::join(whole, &Body(b))?;
            }
            _ => out.push((owner.body, Body(b))),
        }
    }
    if out.is_empty() {
        return Err(err("the ACIS data holds no solid"));
    }
    Ok(out.into_iter().map(|(_, b)| b).collect())
}
