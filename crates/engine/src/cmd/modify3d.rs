//! Modifying in space: 3DMOVE, 3DROTATE, ROTATE3D, 3DSCALE, MIRROR3D, 3DALIGN and 3DARRAY. Every
//! kind of object moves, scales and turns about Z; turning out of the XY plane takes lines,
//! points, 3D polylines, faces, meshes, circles, arcs, ellipses, polylines and solids into the
//! plane they go to (the kinds that can't leave it yet are left as they were, and said so).

use draftcraft_doc::{Entity, Handle};
use draftcraft_geom::{Mat4, Vec2, Vec3};
use serde_json::{Value, json};

use super::machines::{SelOutcome, SelectPhase, number};
use super::{CommandSpec, bad, bool_or, f64_or, f64_req, str_param, targets};
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

#[cfg(test)]
mod tests;

/// Most copies one 3DARRAY makes (objects × items).
const MAX_ARRAY_COPIES: usize = 100_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("3dmove", "3D Move", run_3dmove)
            .menu(&["Modify", "3D Operations", "3D Move"])
            .alias(&["3m"])
            .params("{handles?, from?: [x,y,z], to?: [x,y,z] | delta: [dx,dy,dz], copy?: bool}")
            .interactive(|_| Ok(Box::new(Xform3M::new(Op3::Move)))),
        CommandSpec::new("3drotate", "3D Rotate", run_3drotate)
            .menu(&["Modify", "3D Operations", "3D Rotate"])
            .alias(&["3r"])
            .params("{handles?, base: [x,y,z], axis?: \"x\"|\"y\"|\"z\"|[dx,dy,dz] (default \"z\"), angle (degrees), copy?: bool}")
            .interactive(|_| Ok(Box::new(Xform3M::new(Op3::Rotate)))),
        CommandSpec::new("rotate3d", "Rotate 3D", run_rotate3d)
            .params("{handles?, p1: [x,y,z], p2: [x,y,z] (the axis, turning by the right-hand rule from p1 to p2), angle (degrees)}")
            .interactive(|_| Ok(Box::new(Xform3M::new(Op3::Rotate3d)))),
        CommandSpec::new("3dscale", "3D Scale", run_3dscale)
            .menu(&["Modify", "3D Operations", "3D Scale"])
            .params("{handles?, base: [x,y,z], factor, copy?: bool}")
            .interactive(|_| Ok(Box::new(Xform3M::new(Op3::Scale)))),
        CommandSpec::new("mirror3d", "3D Mirror", run_mirror3d)
            .menu(&["Modify", "3D Operations", "3D Mirror"])
            .params("{handles?, points: [p1,p2,p3] | plane: \"xy\"|\"yz\"|\"zx\", point?: [x,y,z], erase?: bool}")
            .interactive(|_| Ok(Box::new(Xform3M::new(Op3::Mirror)))),
        CommandSpec::new("3dalign", "3D Align", run_3dalign)
            .menu(&["Modify", "3D Operations", "3D Align"])
            .alias(&["3al"])
            .params("{handles?, source: [p1, p2?, p3?], dest: [q1, q2?, q3?] (as many), scale?: bool, copy?: bool}")
            .interactive(|_| Ok(Box::new(Xform3M::new(Op3::Align)))),
        CommandSpec::new("3darray", "3D Array", run_3darray)
            .menu(&["Modify", "3D Operations", "3D Array"])
            .alias(&["3a"])
            .params(
                "{handles?, type: \"rectangular\", rows?, columns?, levels?, rowSpacing?, columnSpacing?, levelSpacing?} | {handles?, type: \"polar\", count, fill? (degrees, default 360), rotate?: bool (default true), p1: [x,y,z], p2: [x,y,z]}",
            )
            .interactive(|_| Ok(Box::new(Xform3M::new(Op3::Array)))),
    ]
}

// ---------------- the operation ----------------

/// Apply `m` to each of `hs` (or to a copy of each when `copy`). Objects on locked layers are
/// left alone unless copied; objects that can't leave the XY plane yet are left as they were.
/// The handles edited or made, and how many were left.
pub(crate) fn transform3_entities(s: &mut Session, hs: &[Handle], m: &Mat4, copy: bool) -> Result<(Vec<Handle>, usize)> {
    if !m.is_affine() || m.det3().abs() <= 1e-300 || m.det3().is_nan() {
        return Err(EngineError::Other("That transform flattens the objects.".into()));
    }
    let d = s.doc_mut()?;
    let mut out = Vec::new();
    let mut left = 0;
    for h in hs {
        let Some(sp) = d.space_of(*h) else { continue };
        let Some(mut e) = d.entity(*h).map(|e| (**e).clone()) else { continue };
        if !copy && d.layer(&e.common.layer).is_some_and(|l| l.locked) {
            continue;
        }
        let mut common = e.common.clone();
        if e.kind.transform3(&mut common, m).is_err() {
            left += 1;
            continue;
        }
        e.common = common;
        if copy {
            let nh = d.new_handle();
            let ne = Entity { handle: nh, ..e };
            if let Some(st) = d.space_mut(&sp) {
                st.push(ne);
                out.push(nh);
            }
        } else {
            d.modify_entity(*h, |x| *x = e)?;
            out.push(*h);
        }
    }
    Ok((out, left))
}

fn report(s: &mut Session, hs: Vec<Handle>, left: usize) -> Value {
    if left > 0 {
        s.echo(format!(
            "{left} object(s) can't be turned out of the XY plane yet (text, dimensions, hatches, block references…) and were left as they were."
        ));
    }
    json!({ "handles": hs.iter().map(|h| h.hex()).collect::<Vec<_>>(), "left": left })
}

/// A 3D point parameter: `[x, y]` (at Z 0) or `[x, y, z]`.
fn p3(v: &Value) -> Option<Vec3> {
    super::helpers::point3_value(v)
}

fn p3_req(cmd: &str, p: &Value, key: &str) -> Result<Vec3> {
    p.get(key).and_then(p3).ok_or_else(|| bad(cmd, format!("`{key}` ([x, y] or [x, y, z]) is required")))
}

fn axis_of(cmd: &str, v: Option<&Value>) -> Result<Vec3> {
    match v {
        None | Some(Value::Null) => Ok(Vec3::Z),
        Some(Value::String(t)) => match t.to_ascii_lowercase().as_str() {
            "x" => Ok(Vec3::X),
            "y" => Ok(Vec3::Y),
            "z" => Ok(Vec3::Z),
            _ => Err(bad(cmd, "`axis` is \"x\", \"y\", \"z\" or a direction [dx, dy, dz]")),
        },
        Some(v) => p3(v).filter(|a| a.len() > 1e-12).ok_or_else(|| bad(cmd, "`axis` is \"x\", \"y\", \"z\" or a direction [dx, dy, dz]")),
    }
}

fn rotation(cmd: &str, o: Vec3, axis: Vec3, deg: f64) -> Result<Mat4> {
    Mat4::rotation_about(o, axis, deg.to_radians()).ok_or_else(|| bad(cmd, "the axis has no direction"))
}

fn plane_normal(cmd: &str, name: &str) -> Result<Vec3> {
    match name.to_ascii_lowercase().as_str() {
        "xy" => Ok(Vec3::Z),
        "yz" => Ok(Vec3::X),
        "zx" | "xz" => Ok(Vec3::Y),
        _ => Err(bad(cmd, "`plane` is \"xy\", \"yz\" or \"zx\"")),
    }
}

/// The mirror in the plane through three points.
fn mirror3(cmd: &str, a: Vec3, b: Vec3, c: Vec3) -> Result<Mat4> {
    let n = (b - a).cross(c - a);
    if n.len() < 1e-12 {
        return Err(bad(cmd, "the three points are in a line: they don't make a plane"));
    }
    Mat4::mirror_plane(a, n).ok_or_else(|| bad(cmd, "the plane has no direction"))
}

/// The turn taking direction `a` to direction `b` (about their common perpendicular).
fn turn_onto(a: Vec3, b: Vec3) -> Option<Mat4> {
    let (a, b) = (a.normalized(), b.normalized());
    if a == Vec3::ZERO || b == Vec3::ZERO {
        return None;
    }
    let axis = a.cross(b);
    let cos = a.dot(b).clamp(-1.0, 1.0);
    if axis.len() < 1e-12 {
        if cos > 0.0 {
            return Some(Mat4::IDENTITY);
        }
        // Opposite: half a turn about any perpendicular.
        let p = if a.x.abs() < 0.9 { a.cross(Vec3::X) } else { a.cross(Vec3::Y) };
        return Mat4::rotation_about(Vec3::ZERO, p, std::f64::consts::PI);
    }
    Mat4::rotation_about(Vec3::ZERO, axis, axis.len().atan2(cos))
}

/// An orthonormal frame from three points: X along a→b, Z normal to the plane abc. Rows.
fn frame(a: Vec3, b: Vec3, c: Vec3) -> Option<[Vec3; 3]> {
    let x = (b - a).normalized();
    let z = x.cross(c - a).normalized();
    if x == Vec3::ZERO || z == Vec3::ZERO {
        return None;
    }
    Some([x, z.cross(x), z])
}

/// 3DALIGN's transform: the first source point onto the first destination point; with two, the
/// source direction onto the destination direction (scaled to it when `scale`); with three, the
/// source plane onto the destination plane too.
pub(crate) fn align(src: &[Vec3], dst: &[Vec3], scale: bool) -> Result<Mat4> {
    let bad_pts = || bad("3dalign", "`source` and `dest` need the same number of points, 1 to 3");
    if src.is_empty() || src.len() > 3 || src.len() != dst.len() {
        return Err(bad_pts());
    }
    let (s1, d1) = (src[0], dst[0]);
    let to_origin = Mat4::translation(-s1);
    let back = Mat4::translation(d1);
    let lin = match src.len() {
        1 => Mat4::IDENTITY,
        2 => turn_onto(src[1] - s1, dst[1] - d1).ok_or_else(|| bad("3dalign", "two of the points coincide"))?,
        _ => {
            let fs = frame(s1, src[1], src[2]).ok_or_else(|| bad("3dalign", "the source points are in a line"))?;
            let fd = frame(d1, dst[1], dst[2]).ok_or_else(|| bad("3dalign", "the destination points are in a line"))?;
            let (fs, fd) = (fs.map(|v| [v.x, v.y, v.z]), fd.map(|v| [v.x, v.y, v.z]));
            // R = Fdᵀ Fs: source frame axes onto destination frame axes.
            let mut m = Mat4::IDENTITY;
            for (i, row) in m.m.iter_mut().take(3).enumerate() {
                for (j, cell) in row.iter_mut().take(3).enumerate() {
                    *cell = fd.iter().zip(&fs).map(|(d, s)| d[i] * s[j]).sum();
                }
            }
            m
        }
    };
    let mut f = 1.0;
    if scale && src.len() >= 2 {
        let (ls, ld) = ((src[1] - s1).len(), (dst[1] - d1).len());
        if ls > 1e-12 && ld > 1e-12 {
            f = ld / ls;
        }
    }
    Ok(back.mul(&Mat4::scale_about(Vec3::ZERO, f)).mul(&lin).mul(&to_origin))
}

// ---------------- JSON forms ----------------

fn run_3dmove(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let d = match p.get("delta") {
        Some(v) => p3(v).ok_or_else(|| bad("3dmove", "`delta` is [dx, dy] or [dx, dy, dz]"))?,
        None => p3_req("3dmove", p, "to")? - p3_req("3dmove", p, "from")?,
    };
    let (out, left) = transform3_entities(s, &hs, &Mat4::translation(d), bool_or(p, "copy", false))?;
    Ok(report(s, out, left))
}

fn run_3drotate(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let m = rotation("3drotate", p3_req("3drotate", p, "base")?, axis_of("3drotate", p.get("axis"))?, f64_req("3drotate", p, "angle")?)?;
    let (out, left) = transform3_entities(s, &hs, &m, bool_or(p, "copy", false))?;
    Ok(report(s, out, left))
}

fn run_rotate3d(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let a = p3_req("rotate3d", p, "p1")?;
    let b = p3_req("rotate3d", p, "p2")?;
    let m = rotation("rotate3d", a, b - a, f64_req("rotate3d", p, "angle")?)?;
    let (out, left) = transform3_entities(s, &hs, &m, false)?;
    Ok(report(s, out, left))
}

fn run_3dscale(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let f = f64_req("3dscale", p, "factor")?;
    if !(f > 0.0 && f.is_finite()) {
        return Err(bad("3dscale", "`factor` must be positive"));
    }
    let (out, left) = transform3_entities(s, &hs, &Mat4::scale_about(p3_req("3dscale", p, "base")?, f), bool_or(p, "copy", false))?;
    Ok(report(s, out, left))
}

fn run_mirror3d(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let m = match str_param(p, "plane") {
        Some(name) => {
            let o = match p.get("point") {
                None | Some(Value::Null) => Vec3::ZERO,
                Some(_) => p3_req("mirror3d", p, "point")?,
            };
            Mat4::mirror_plane(o, plane_normal("mirror3d", name)?).ok_or_else(|| bad("mirror3d", "the plane has no direction"))?
        }
        None => {
            let pts: Vec<Vec3> = p.get("points").and_then(Value::as_array).map(|a| a.iter().filter_map(p3).collect()).unwrap_or_default();
            let [a, b, c] = pts.as_slice() else {
                return Err(bad("mirror3d", "`points` (three points on the mirror plane) or `plane` is required"));
            };
            mirror3("mirror3d", *a, *b, *c)?
        }
    };
    let (out, left) = transform3_entities(s, &hs, &m, !bool_or(p, "erase", false))?;
    Ok(report(s, out, left))
}

fn points3(cmd: &str, p: &Value, key: &str) -> Result<Vec<Vec3>> {
    let a = p.get(key).and_then(Value::as_array).ok_or_else(|| bad(cmd, format!("`{key}` (1 to 3 points) is required")))?;
    if a.is_empty() || a.len() > 3 {
        return Err(bad(cmd, format!("`{key}` takes 1 to 3 points")));
    }
    a.iter().map(|v| p3(v).ok_or_else(|| bad(cmd, format!("`{key}`: {v} is not a point")))).collect()
}

fn run_3dalign(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let m = align(&points3("3dalign", p, "source")?, &points3("3dalign", p, "dest")?, bool_or(p, "scale", false))?;
    let (out, left) = transform3_entities(s, &hs, &m, bool_or(p, "copy", false))?;
    Ok(report(s, out, left))
}

fn count(cmd: &str, p: &Value, key: &str, default: u64) -> Result<usize> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default as usize),
        Some(v) => v
            .as_u64()
            .filter(|n| (1..=MAX_ARRAY_COPIES as u64).contains(n))
            .map(|n| n as usize)
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number from 1 to {MAX_ARRAY_COPIES}"))),
    }
}

/// The transforms of a rectangular 3D array's items after the first.
fn rect_items(rows: usize, cols: usize, levels: usize, dr: f64, dc: f64, dl: f64) -> Vec<Mat4> {
    let mut v = Vec::new();
    for l in 0..levels {
        for r in 0..rows {
            for c in 0..cols {
                if (r, c, l) != (0, 0, 0) {
                    v.push(Mat4::translation(Vec3::new(c as f64 * dc, r as f64 * dr, l as f64 * dl)));
                }
            }
        }
    }
    v
}

/// The transforms of a polar 3D array's items after the first: `n` items over `fill` degrees
/// about the axis a→b; turned with it, or only carried round (about `reference`) when not.
fn polar_items(n: usize, fill: f64, rotate: bool, a: Vec3, b: Vec3, reference: Vec3) -> Result<Vec<Mat4>> {
    let full = (fill.abs() - 360.0).abs() < 1e-9;
    let step = if full { fill / n as f64 } else { fill / (n.saturating_sub(1).max(1)) as f64 };
    let mut v = Vec::new();
    for k in 1..n {
        let r = rotation("3darray", a, b - a, step * k as f64)?;
        v.push(if rotate { r } else { Mat4::translation(r.apply(reference) - reference) });
    }
    Ok(v)
}

fn array_copies(s: &mut Session, hs: &[Handle], items: &[Mat4]) -> Result<Value> {
    if items.len().saturating_mul(hs.len()) > MAX_ARRAY_COPIES {
        return Err(EngineError::Other(format!("That array would make more than {MAX_ARRAY_COPIES} objects.")));
    }
    let mut out = Vec::new();
    let mut left = 0;
    for m in items {
        let (h, l) = transform3_entities(s, hs, m, true)?;
        out.extend(h);
        left = left.max(l);
    }
    Ok(report(s, out, left))
}

/// The middle of the objects' extents, at the height of the first point they have: the point a
/// polar array that doesn't turn its items carries round.
fn reference_point(s: &Session, hs: &[Handle]) -> Result<Vec3> {
    let d = s.doc()?;
    let mut b = draftcraft_geom::Bounds2::EMPTY;
    for h in hs {
        if let Some(e) = d.entity(*h) {
            b = b.union(&draftcraft_doc::entity_bounds(d, e, 0));
        }
    }
    let c = if b.is_empty() { Vec2::ZERO } else { b.center() };
    Ok(c.to3(0.0))
}

fn run_3darray(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let kind = str_param(p, "type").unwrap_or("rectangular").to_ascii_lowercase();
    let items = if kind.starts_with('p') {
        let n = count("3darray", p, "count", 0)?;
        if n < 2 {
            return Err(bad("3darray", "`count` (2 or more items) is required"));
        }
        let fill = f64_or(p, "fill", 360.0);
        if !fill.is_finite() || fill == 0.0 || fill.abs() > 360.0 {
            return Err(bad("3darray", "`fill` is an angle from -360 to 360 degrees, not 0"));
        }
        let (a, b) = (p3_req("3darray", p, "p1")?, p3_req("3darray", p, "p2")?);
        let reference = reference_point(s, &hs)?;
        polar_items(n, fill, bool_or(p, "rotate", true), a, b, reference)?
    } else if kind.starts_with('r') {
        let (rows, cols, levels) = (count("3darray", p, "rows", 1)?, count("3darray", p, "columns", 1)?, count("3darray", p, "levels", 1)?);
        if rows.saturating_mul(cols).saturating_mul(levels) > MAX_ARRAY_COPIES {
            return Err(EngineError::Other(format!("That array would make more than {MAX_ARRAY_COPIES} objects.")));
        }
        let sp = |k: &str| f64_or(p, k, 1.0);
        let (dr, dc, dl) = (sp("rowSpacing"), sp("columnSpacing"), sp("levelSpacing"));
        if ![dr, dc, dl].iter().all(|v| v.is_finite()) {
            return Err(bad("3darray", "spacings must be numbers"));
        }
        rect_items(rows, cols, levels, dr, dc, dl)
    } else {
        return Err(bad("3darray", "`type` is \"rectangular\" or \"polar\""));
    };
    array_copies(s, &hs, &items)
}

// ---------------- prompts ----------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op3 {
    Move,
    Rotate,
    Rotate3d,
    Scale,
    Mirror,
    Align,
    Array,
}

/// The prompts of the 3D modify commands: select objects, then points, numbers and keywords in
/// turn, as AutoCAD asks them. A typed `x,y,z` keeps its Z; a picked point is at ELEVATION.
struct Xform3M {
    op: Op3,
    sel: SelectPhase,
    picked: Option<Vec<Handle>>,
    pts: Vec<Vec3>,
    /// An axis or plane named by keyword (X/Y/Z, XY/YZ/ZX): its direction.
    named: Option<Vec3>,
    /// 3DALIGN: how many source points (once the source points are in), whether copying.
    sources: Option<usize>,
    copy: bool,
    /// 3DARRAY: polar or rectangular (once chosen), and the numbers typed so far.
    polar: Option<bool>,
    nums: Vec<f64>,
    rotate_items: Option<bool>,
}

fn elevation(s: &Session) -> f64 {
    s.doc().map(|d| d.header.f64("ELEVATION", 0.0)).ok().filter(|z| z.is_finite()).unwrap_or(0.0)
}

impl Xform3M {
    fn new(op: Op3) -> Self {
        Xform3M {
            op,
            sel: SelectPhase::default(),
            picked: None,
            pts: Vec::new(),
            named: None,
            sources: None,
            copy: false,
            polar: None,
            nums: Vec::new(),
            rotate_items: None,
        }
    }
    fn objs(&self) -> Vec<Handle> {
        self.picked.clone().unwrap_or_default()
    }
    fn point(s: &Session, p: Vec2) -> Vec3 {
        p.to3(s.point_z.unwrap_or_else(|| elevation(s)))
    }
    fn last(&self) -> Option<Vec2> {
        self.pts.last().map(|p| p.xy())
    }
    fn finish(&mut self, s: &mut Session, m: &Mat4, copy: bool) -> Result<Step> {
        let hs = self.objs();
        let (out, left) = transform3_entities(s, &hs, m, copy)?;
        report(s, out, left);
        s.set_selection(Vec::new());
        Ok(Step::Done)
    }
    fn angle(s: &Session, i: &Input, base: Option<Vec2>) -> Result<Option<f64>> {
        Ok(match i {
            Input::Text(t) => Some(
                s.angle_settings()
                    .rotation(t)
                    .filter(|a| a.is_finite())
                    .ok_or_else(|| EngineError::Other("Requires an angle or point.".into()))?
                    .to_degrees(),
            ),
            Input::Point(p) => base.map(|b| b.angle_to(*p).to_degrees()),
            _ => None,
        })
    }
    fn number(i: &Input) -> Option<f64> {
        match i {
            Input::Text(t) => number(t).filter(|v| v.is_finite()),
            _ => None,
        }
    }
    fn yes_no(i: &Input, default: bool) -> Option<bool> {
        match i {
            Input::Enter => Some(default),
            Input::Keyword(k) | Input::Text(k) => match k.chars().next().map(|c| c.to_ascii_uppercase()) {
                Some('Y') => Some(true),
                Some('N') => Some(false),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Interactive for Xform3M {
    fn name(&self) -> &'static str {
        match self.op {
            Op3::Move => "3DMOVE",
            Op3::Rotate => "3DROTATE",
            Op3::Rotate3d => "ROTATE3D",
            Op3::Scale => "3DSCALE",
            Op3::Mirror => "MIRROR3D",
            Op3::Align => "3DALIGN",
            Op3::Array => "3DARRAY",
        }
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.sel = SelectPhase::begin(s);
        if self.sel.done {
            self.picked = Some(self.sel.picked.clone());
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.picked.is_none() {
            return self.sel.prompt();
        }
        let n = self.pts.len();
        let with_base = |p: Prompt| match self.last() {
            Some(b) => p.base(b),
            None => p,
        };
        match self.op {
            Op3::Move => match n {
                0 => Prompt::new("Specify base point", Accept::POINT),
                _ => with_base(Prompt::new("Specify second point or <use first point as displacement>", Accept::POINT)),
            },
            Op3::Scale => match n {
                0 => Prompt::new("Specify base point", Accept::POINT),
                _ => Prompt::new("Specify scale factor", Accept::NUMBER),
            },
            Op3::Rotate => match (n, self.named) {
                (0, _) => Prompt::new("Specify base point", Accept::POINT),
                (1, None) => with_base(Prompt::new("Pick a rotation axis or a second point on it", Accept::POINT).kw(&["X", "Y", "Z"]).default("Z")),
                _ => with_base(Prompt::new("Specify angle start point or type an angle", Accept::POINT_OR_NUMBER)),
            },
            Op3::Rotate3d => match (n, self.named) {
                (0, None) => Prompt::new("Specify first point on axis or define axis by", Accept::POINT)
                    .kw(&["Xaxis", "Yaxis", "Zaxis", "2points"])
                    .default("2points"),
                (0, Some(_)) => Prompt::new("Specify a point on the axis", Accept::POINT).default("0,0,0"),
                (1, None) => with_base(Prompt::new("Specify second point on axis", Accept::POINT)),
                _ => Prompt::new("Specify rotation angle", Accept::POINT_OR_NUMBER),
            },
            Op3::Mirror => match (n, self.named) {
                (0, None) => Prompt::new("Specify first point of mirror plane (3 points) or", Accept::POINT)
                    .kw(&["XY", "YZ", "ZX", "3points"])
                    .default("3points"),
                (0, Some(_)) => Prompt::new("Specify point on plane", Accept::POINT).default("0,0,0"),
                (1, None) => with_base(Prompt::new("Specify second point on mirror plane", Accept::POINT)),
                (2, None) => with_base(Prompt::new("Specify third point on mirror plane", Accept::POINT)),
                _ => Prompt::new("Delete source objects?", Accept::NUMBER).kw(&["Yes", "No"]).default("No"),
            },
            Op3::Align => match self.sources {
                None => match n {
                    0 => Prompt::new("Specify base point", Accept::POINT).kw(&["Copy"]),
                    1 => with_base(Prompt::new("Specify second point", Accept::POINT).kw(&["Continue"]).default("C")),
                    _ => with_base(Prompt::new("Specify third point", Accept::POINT).kw(&["Continue"]).default("C")),
                },
                Some(k) => match n - k {
                    0 => Prompt::new("Specify first destination point", Accept::POINT),
                    1 => with_base(Prompt::new("Specify second destination point", Accept::POINT)),
                    _ => with_base(Prompt::new("Specify third destination point", Accept::POINT)),
                },
            },
            Op3::Array => match (self.polar, self.nums.len(), self.rotate_items, n) {
                (None, ..) => Prompt::new("Enter the type of array", Accept::NUMBER).kw(&["Rectangular", "Polar"]).default("R"),
                (Some(false), 0, ..) => Prompt::new("Enter the number of rows (---)", Accept::NUMBER).default("1"),
                (Some(false), 1, ..) => Prompt::new("Enter the number of columns (|||)", Accept::NUMBER).default("1"),
                (Some(false), 2, ..) => Prompt::new("Enter the number of levels (...)", Accept::NUMBER).default("1"),
                (Some(false), 3, ..) => Prompt::new("Specify the distance between rows (---)", Accept::NUMBER),
                (Some(false), 4, ..) => Prompt::new("Specify the distance between columns (|||)", Accept::NUMBER),
                (Some(false), ..) => Prompt::new("Specify the distance between levels (...)", Accept::NUMBER),
                (Some(true), 0, ..) => Prompt::new("Enter the number of items in the array", Accept::NUMBER),
                (Some(true), 1, ..) => Prompt::new("Specify the angle to fill (+=ccw, -=cw)", Accept::NUMBER).default("360"),
                (Some(true), _, None, _) => Prompt::new("Rotate arrayed objects?", Accept::NUMBER).kw(&["Yes", "No"]).default("Y"),
                (Some(true), _, Some(_), 0) => Prompt::new("Specify center point of array", Accept::POINT),
                (Some(true), ..) => with_base(Prompt::new("Specify second point on axis of rotation", Accept::POINT)),
            },
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if self.picked.is_none() {
            return match self.sel.feed(s, &i)? {
                SelOutcome::More => Ok(Step::Continue),
                SelOutcome::Empty => Ok(Step::Done),
                SelOutcome::Done(hs) => {
                    self.picked = Some(hs);
                    Ok(Step::Continue)
                }
            };
        }
        if matches!(i, Input::Cancel) {
            return Ok(Step::Cancel);
        }
        let n = self.pts.len();
        match self.op {
            Op3::Move => match (n, i) {
                (0, Input::Point(p)) => self.pts.push(Self::point(s, p)),
                (1, Input::Point(p)) => {
                    let d = Self::point(s, p) - self.pts[0];
                    return self.finish(s, &Mat4::translation(d), false);
                }
                (1, Input::Enter) => {
                    let d = self.pts[0];
                    return self.finish(s, &Mat4::translation(d), false);
                }
                _ => {}
            },
            Op3::Scale => match (n, &i) {
                (0, Input::Point(p)) => self.pts.push(Self::point(s, *p)),
                (1, _) => {
                    let Some(f) = Self::number(&i).filter(|f| *f > 0.0) else {
                        s.echo("Requires a positive number.");
                        return Ok(Step::Continue);
                    };
                    let base = self.pts[0];
                    return self.finish(s, &Mat4::scale_about(base, f), false);
                }
                _ => {}
            },
            Op3::Rotate => match (n, &i) {
                (0, Input::Point(p)) => self.pts.push(Self::point(s, *p)),
                (1, Input::Keyword(k)) if self.named.is_none() => self.named = Some(axis_of("3drotate", Some(&Value::String(k.clone())))?),
                (1, Input::Enter) if self.named.is_none() => self.named = Some(Vec3::Z),
                (1, Input::Point(p)) if self.named.is_none() => {
                    let q = Self::point(s, *p);
                    if (q - self.pts[0]).len() < 1e-12 {
                        s.echo("The axis needs two different points.");
                    } else {
                        self.named = Some(q - self.pts[0]);
                    }
                }
                (1, _) => {
                    let Some(a) = Self::angle(s, &i, self.last())? else { return Ok(Step::Continue) };
                    let m = rotation("3drotate", self.pts[0], self.named.unwrap_or(Vec3::Z), a)?;
                    return self.finish(s, &m, false);
                }
                _ => {}
            },
            Op3::Rotate3d => match (n, self.named, &i) {
                (0, None, Input::Keyword(k)) => match k.to_ascii_uppercase().chars().next() {
                    Some('X') => self.named = Some(Vec3::X),
                    Some('Y') => self.named = Some(Vec3::Y),
                    Some('Z') => self.named = Some(Vec3::Z),
                    _ => {}
                },
                (0, None, Input::Enter) => {}
                (0, None, Input::Point(p)) => self.pts.push(Self::point(s, *p)),
                (0, Some(_), Input::Point(p)) => self.pts.push(Self::point(s, *p)),
                (0, Some(_), Input::Enter) => self.pts.push(Vec3::ZERO),
                (1, None, Input::Point(p)) => {
                    let q = Self::point(s, *p);
                    if (q - self.pts[0]).len() < 1e-12 {
                        s.echo("The axis needs two different points.");
                    } else {
                        self.named = Some(q - self.pts[0]);
                        self.pts.push(q);
                    }
                }
                (1.., Some(axis), _) => {
                    let Some(a) = Self::angle(s, &i, None)? else { return Ok(Step::Continue) };
                    let m = rotation("rotate3d", self.pts[0], axis, a)?;
                    return self.finish(s, &m, false);
                }
                _ => {}
            },
            Op3::Mirror => {
                let ready = (self.named.is_some() && n == 1) || n == 3;
                if ready {
                    let Some(erase) = Self::yes_no(&i, false) else { return Ok(Step::Continue) };
                    let m = match self.named {
                        Some(nrm) => Mat4::mirror_plane(self.pts[0], nrm).ok_or_else(|| bad("mirror3d", "the plane has no direction"))?,
                        None => mirror3("mirror3d", self.pts[0], self.pts[1], self.pts[2])?,
                    };
                    return self.finish(s, &m, !erase);
                }
                match (n, self.named, &i) {
                    (0, None, Input::Keyword(k)) if !k.starts_with('3') => self.named = Some(plane_normal("mirror3d", k)?),
                    (0, Some(_), Input::Enter) => self.pts.push(Vec3::ZERO),
                    (_, _, Input::Point(p)) => {
                        let q = Self::point(s, *p);
                        if self.pts.iter().any(|o| (*o - q).len() < 1e-12) {
                            s.echo("That point is already on the plane: specify another.");
                        } else {
                            self.pts.push(q);
                            if self.pts.len() == 3 && (self.pts[1] - self.pts[0]).cross(self.pts[2] - self.pts[0]).len() < 1e-12 {
                                s.echo("The three points are in a line: specify the third point again.");
                                self.pts.pop();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Op3::Align => match (self.sources, &i) {
                (None, Input::Keyword(k)) if n == 0 && k.eq_ignore_ascii_case("copy") => {
                    self.copy = true;
                    s.echo("Copies will be made.");
                }
                (None, Input::Keyword(_) | Input::Enter) if n > 0 => self.sources = Some(n),
                (None, Input::Point(p)) => {
                    self.pts.push(Self::point(s, *p));
                    if self.pts.len() == 3 {
                        self.sources = Some(3);
                    }
                }
                (Some(k), Input::Point(p)) => {
                    self.pts.push(Self::point(s, *p));
                    if self.pts.len() == 2 * k {
                        let (src, dst) = self.pts.split_at(k);
                        let m = align(src, dst, false)?;
                        let copy = self.copy;
                        return self.finish(s, &m, copy);
                    }
                }
                _ => {}
            },
            Op3::Array => return self.array_input(s, i),
        }
        Ok(Step::Continue)
    }
}

impl Xform3M {
    fn array_input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let Some(polar) = self.polar else {
            match &i {
                Input::Enter => self.polar = Some(false),
                Input::Keyword(k) | Input::Text(k) => match k.chars().next().map(|c| c.to_ascii_uppercase()) {
                    Some('R') => self.polar = Some(false),
                    Some('P') => self.polar = Some(true),
                    _ => {}
                },
                _ => {}
            }
            return Ok(Step::Continue);
        };
        let k = self.nums.len();
        if !polar {
            let v = match (&i, k) {
                (Input::Enter, 0..=2) => Some(1.0),
                _ => Self::number(&i),
            };
            let Some(v) = v else { return Ok(Step::Continue) };
            if k <= 2 && !(v >= 1.0 && v.fract() == 0.0 && v <= MAX_ARRAY_COPIES as f64) {
                s.echo("Requires a whole number of 1 or more.");
                return Ok(Step::Continue);
            }
            self.nums.push(v);
            // Distances are asked only for the directions with more than one item.
            while let Some(&count) = self.nums.get(self.nums.len().wrapping_sub(3)).filter(|_| (3..6).contains(&self.nums.len())) {
                if count > 1.0 {
                    break;
                }
                self.nums.push(1.0);
            }
            if self.nums.len() >= 6 {
                let [r, c, l, dr, dc, dl] = [0, 1, 2, 3, 4, 5].map(|j| self.nums.get(j).copied().unwrap_or(1.0));
                if (r * c * l) as usize > MAX_ARRAY_COPIES {
                    return Err(EngineError::Other(format!("That array would make more than {MAX_ARRAY_COPIES} objects.")));
                }
                let items = rect_items(r as usize, c as usize, l as usize, dr, dc, dl);
                let hs = self.objs();
                array_copies(s, &hs, &items)?;
                s.set_selection(Vec::new());
                return Ok(Step::Done);
            }
            return Ok(Step::Continue);
        }
        match (k, self.rotate_items, &i) {
            (0, ..) => {
                let Some(v) = Self::number(&i).filter(|v| *v >= 2.0 && v.fract() == 0.0 && *v <= MAX_ARRAY_COPIES as f64) else {
                    s.echo("Requires a whole number of 2 or more.");
                    return Ok(Step::Continue);
                };
                self.nums.push(v);
            }
            (1, ..) => {
                let v = if matches!(i, Input::Enter) { Some(360.0) } else { Self::number(&i) };
                match v.filter(|v| *v != 0.0 && v.abs() <= 360.0) {
                    Some(v) => self.nums.push(v),
                    None => s.echo("Requires an angle from -360 to 360, not 0."),
                }
            }
            (_, None, _) => self.rotate_items = Self::yes_no(&i, true),
            (_, Some(rot), Input::Point(p)) => {
                let q = Self::point(s, *p);
                if self.pts.is_empty() {
                    self.pts.push(q);
                } else if (q - self.pts[0]).len() < 1e-12 {
                    s.echo("The axis needs two different points.");
                } else {
                    let hs = self.objs();
                    let reference = reference_point(s, &hs)?;
                    let n = self.nums.first().copied().unwrap_or(2.0) as usize;
                    let fill = self.nums.get(1).copied().unwrap_or(360.0);
                    let items = polar_items(n, fill, rot, self.pts[0], q, reference)?;
                    array_copies(s, &hs, &items)?;
                    s.set_selection(Vec::new());
                    return Ok(Step::Done);
                }
            }
            _ => {}
        }
        Ok(Step::Continue)
    }
}
