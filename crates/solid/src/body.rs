//! The body type and the kernel operations DraftCraft uses, over solidcore (a Rust port of
//! NVIDIA's SMLib B-rep modeler, Apache-2.0).
//!
//! Every kernel call runs inside solidcore's own session, which turns a panic in the kernel into
//! an error (on native builds; on wasm a panic aborts, which is why the web build keeps solids
//! off). A kernel failure is always a [`SolidError`] and never takes the drawing down.

use draftcraft_geom::{Mat4, Vec2, Vec3};
use solidcore_ops as ops;
use solidcore_record as rec;

/// A solid body. The kernel's type never leaves this crate. Operations copy their operands:
/// a body is never changed in place.
pub struct Body(pub(crate) ops::Body);

/// A kernel operation that failed, or input it refused. The message is for the user.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidError(pub String);

impl std::fmt::Display for SolidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SolidError {}

impl From<ops::Error> for SolidError {
    fn from(e: ops::Error) -> Self {
        SolidError(match e {
            ops::Error::Argument => "the solid modeler refused the input (a number out of range or a selection it can't use)".into(),
            ops::Error::Empty => "the operation left no solid".into(),
            ops::Error::NotSolid => "the operation did not give a closed solid".into(),
            ops::Error::NoRoom => "there is no room for it: the size is too large for the faces".into(),
            other => format!("the solid modeler failed ({other})"),
        })
    }
}

pub type Result<T> = std::result::Result<T, SolidError>;

fn err(m: impl Into<String>) -> SolidError {
    SolidError(m.into())
}

/// Largest coordinate or size accepted from input: beyond it tolerances lose their meaning.
/// Bodies far from the origin are kept near their own origin by the caller, with a placement
/// (docs/solids.md, "Units and scale").
pub const MAX_SIZE: f64 = 1e7;

/// The relative accuracy measurements are asked for.
const ACCURACY: f64 = 1e-7;

/// A size the kernel can work with: finite and more than its tolerance.
fn positive(v: f64) -> bool {
    v.is_finite() && v > 1e-9
}

fn finite(v: f64, what: &str) -> Result<f64> {
    if v.is_finite() && v.abs() < MAX_SIZE { Ok(v) } else { Err(err(format!("{what} must be a finite number below {MAX_SIZE}"))) }
}

fn point(p: Vec3, what: &str) -> Result<Vec3> {
    Ok(Vec3::new(finite(p.x, what)?, finite(p.y, what)?, finite(p.z, what)?))
}

pub(crate) fn arr(p: Vec3) -> [f64; 3] {
    [p.x, p.y, p.z]
}

pub(crate) fn from_arr(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

pub(crate) fn to_rec(p: Vec3) -> rec::Vec3 {
    rec::Vec3 { x: p.x, y: p.y, z: p.z }
}

pub(crate) fn from_rec(p: rec::Vec3) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

/// A plane with an orthonormal frame: points in it are `origin + u x + v y`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub x: Vec3,
    pub y: Vec3,
}

impl Frame {
    /// The world XY plane at height `z`.
    pub fn xy(z: f64) -> Frame {
        Frame { origin: Vec3::new(0.0, 0.0, z), x: Vec3::new(1.0, 0.0, 0.0), y: Vec3::new(0.0, 1.0, 0.0) }
    }
    /// A frame on the plane through `origin` with `normal`, its x axis as near world X as the
    /// plane allows (world Y for a plane facing along X).
    pub fn from_normal(origin: Vec3, normal: Vec3) -> Result<Frame> {
        let n = unit(normal).ok_or_else(|| err("the plane needs a normal direction"))?;
        let seed = if n.x.abs() > 0.9 { Vec3::new(0.0, 1.0, 0.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let x = unit(seed - n * seed.dot(n)).ok_or_else(|| err("the plane needs a normal direction"))?;
        Ok(Frame { origin, x, y: n.cross(x) })
    }
    pub fn normal(&self) -> Vec3 {
        self.x.cross(self.y)
    }
    pub fn to_world(&self, p: Vec2) -> Vec3 {
        self.origin + self.x * p.x + self.y * p.y
    }
    pub fn to_local(&self, p: Vec3) -> Vec2 {
        let d = p - self.origin;
        Vec2::new(d.dot(self.x), d.dot(self.y))
    }
    /// The frame checked (axes perpendicular unit directions) as the record's plane.
    pub(crate) fn plane(&self) -> Result<rec::Plane> {
        let (x, y) = (unit(self.x), unit(self.y));
        match (x, y) {
            (Some(x), Some(y)) if x.dot(y).abs() < 1e-9 => {
                Ok(rec::Plane { origin: to_rec(point(self.origin, "the plane's origin")?), x: to_rec(x), y: to_rec(y) })
            }
            _ => Err(err("the plane's axes must be perpendicular directions")),
        }
    }
    /// The kernel's affine matrix taking the world axes to this frame (x, y, normal) and the
    /// world origin to its origin: rows 0-2 the images of the axes, row 3 the translation.
    fn placement(&self) -> [[f64; 4]; 4] {
        let n = self.normal();
        [
            [self.x.x, self.x.y, self.x.z, 0.0],
            [self.y.x, self.y.y, self.y.z, 0.0],
            [n.x, n.y, n.z, 0.0],
            [self.origin.x, self.origin.y, self.origin.z, 1.0],
        ]
    }
}

pub(crate) fn unit(v: Vec3) -> Option<Vec3> {
    let l = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    (l.is_finite() && l > 1e-12).then(|| v * (1.0 / l))
}

/// One piece of a 2D profile boundary or of a 2D result, in a plane's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Line {
        a: Vec2,
        b: Vec2,
    },
    /// Counter-clockwise for a positive `sweep` (radians), from the angle `start`.
    Arc {
        center: Vec2,
        radius: f64,
        start: f64,
        sweep: f64,
    },
}

impl Seg {
    pub fn start(&self) -> Vec2 {
        match *self {
            Seg::Line { a, .. } => a,
            Seg::Arc { center, radius, start, .. } => Vec2::new(center.x + radius * start.cos(), center.y + radius * start.sin()),
        }
    }
    pub fn end(&self) -> Vec2 {
        match *self {
            Seg::Line { b, .. } => b,
            Seg::Arc { center, radius, start, sweep } => {
                Vec2::new(center.x + radius * (start + sweep).cos(), center.y + radius * (start + sweep).sin())
            }
        }
    }
    pub fn reversed(&self) -> Seg {
        match *self {
            Seg::Line { a, b } => Seg::Line { a: b, b: a },
            Seg::Arc { center, radius, start, sweep } => Seg::Arc { center, radius, start: start + sweep, sweep: -sweep },
        }
    }
    fn to_rec(self) -> Result<rec::Seg2> {
        Ok(match self {
            Seg::Line { a, b } => rec::Seg2::Line { a: v2(a, "a profile point")?, b: v2(b, "a profile point")? },
            Seg::Arc { center, radius, start, sweep } => {
                if !(radius > 0.0 && start.is_finite() && sweep.is_finite() && sweep != 0.0) {
                    return Err(err("a profile arc needs a positive radius and a sweep"));
                }
                let sweep = sweep.clamp(-std::f64::consts::TAU, std::f64::consts::TAU);
                rec::Seg2::Arc { center: v2(center, "an arc centre")?, radius: finite(radius, "a radius")?, start, sweep }
            }
        })
    }
}

fn v2(p: Vec2, what: &str) -> Result<rec::Vec2> {
    Ok(rec::Vec2 { x: finite(p.x, what)?, y: finite(p.y, what)? })
}

/// Most segments of one profile, and loops of one region.
pub const MAX_PROFILE_SEGMENTS: usize = 100_000;

/// A closed planar region: its outer boundary and its holes, each a closed chain of segments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub outer: Vec<Seg>,
    pub holes: Vec<Vec<Seg>>,
}

fn loop2(segs: &[Seg]) -> Result<rec::Loop2> {
    Ok(rec::Loop2 { segs: segs.iter().map(|s| s.to_rec()).collect::<Result<_>>()? })
}

impl Profile {
    fn check(&self) -> Result<()> {
        let total = self.outer.len().saturating_add(self.holes.iter().map(Vec::len).sum());
        if self.outer.is_empty() || total > MAX_PROFILE_SEGMENTS || self.holes.iter().any(Vec::is_empty) {
            return Err(err("the profile must be a closed boundary of 1 to 100000 segments"));
        }
        Ok(())
    }
    fn to_region(&self) -> Result<rec::Region2> {
        self.check()?;
        Ok(rec::Region2 { outer: loop2(&self.outer)?, holes: self.holes.iter().map(|h| loop2(h)).collect::<Result<_>>()? })
    }
}

/// A loop of a profile as the sweep's 3D curves on `plane`.
fn curves(segs: &[Seg], plane: &rec::Plane) -> Result<Vec<ops::sweep::Curve>> {
    segs.iter()
        .map(|s| ops::sweep::Curve::from_seg2(&s.to_rec()?, plane).ok_or_else(|| err("a profile segment can't be swept (an arc of no sweep?)")))
        .collect()
}

/// Mass properties of a body (MASSPROP), in drawing units, at unit density.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mass {
    pub volume: f64,
    pub area: f64,
    pub centroid: Vec3,
    pub min: Vec3,
    pub max: Vec3,
    /// Moments of inertia about axes through the centroid parallel to X, Y and Z.
    pub moments: Vec3,
    /// Products of inertia about the centroid: (Iyz, Izx, Ixy).
    pub products: Vec3,
}

/// What a body looks like: its edges as polylines and its faces as triangles, in the body's
/// coordinates. The document keeps it with the solid, so drawing never calls the kernel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Display {
    pub wires: Vec<Vec<Vec3>>,
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// The face each triangle belongs to.
    pub tri_face: Vec<u32>,
}

/// Which solids a boolean keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Subtract,
    Intersect,
}

// ---- construction ----

/// A box with opposite corners `a` and `b`, its sides along the axes.
pub fn box_solid(a: Vec3, b: Vec3) -> Result<Body> {
    let (a, b) = (point(a, "a corner")?, point(b, "a corner")?);
    let lo = Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z));
    let size = [(a.x - b.x).abs(), (a.y - b.y).abs(), (a.z - b.z).abs()];
    if !size.iter().all(|s| positive(*s)) {
        return Err(err("a box needs a length, a width and a height"));
    }
    Ok(Body(ops::make_box(arr(lo), size)?))
}

/// A body made about +Z at the origin, placed on the frame at `base` facing along `axis`.
fn placed(body: ops::Body, base: Vec3, axis: Vec3) -> Result<Body> {
    let frame = Frame::from_normal(base, axis)?;
    body.transform(&frame.placement())?;
    Ok(Body(body))
}

/// A cylinder standing on the circle of `radius` about `base`, `height` along `axis`.
pub fn cylinder(base: Vec3, axis: Vec3, radius: f64, height: f64) -> Result<Body> {
    let (base, axis) = (point(base, "the base centre")?, point(axis, "the axis")?);
    let (radius, height) = (finite(radius, "the radius")?, finite(height, "the height")?);
    if !positive(height) || !positive(radius) {
        return Err(err("the radius and the height must be positive"));
    }
    unit(axis).ok_or_else(|| err("the axis needs a direction"))?;
    placed(ops::make_cylinder([0.0; 3], radius, height)?, base, axis)
}

/// A cone (or a frustum when `top_radius` is above 0) on the circle of `radius` about `base`,
/// `height` along `axis`.
pub fn cone(base: Vec3, axis: Vec3, radius: f64, top_radius: f64, height: f64) -> Result<Body> {
    let (base, axis) = (point(base, "the base centre")?, point(axis, "the axis")?);
    let (radius, top, height) = (finite(radius, "the radius")?, finite(top_radius, "the top radius")?, finite(height, "the height")?);
    if !(radius > 1e-9 && top >= 0.0 && height > 1e-9) {
        return Err(err("a cone needs a positive base radius and height, and a top radius of 0 or more"));
    }
    unit(axis).ok_or_else(|| err("the axis needs a direction"))?;
    if (top - radius).abs() <= 1e-12 * radius.max(1.0) {
        return cylinder(base, axis, radius, height);
    }
    placed(ops::make_cone([0.0; 3], radius, top, height)?, base, axis)
}

/// A sphere.
pub fn sphere(center: Vec3, radius: f64) -> Result<Body> {
    let (center, radius) = (point(center, "the centre")?, finite(radius, "the radius")?);
    if !positive(radius) {
        return Err(err("the radius must be positive"));
    }
    Ok(Body(ops::make_sphere(arr(center), radius)?))
}

/// A torus about the world Z axis through `center`.
pub fn torus(center: Vec3, major: f64, minor: f64) -> Result<Body> {
    let (center, major, minor) = (point(center, "the centre")?, finite(major, "the torus radius")?, finite(minor, "the tube radius")?);
    if !(positive(minor) && minor < major) {
        return Err(err("the tube radius must be positive and smaller than the torus radius"));
    }
    Ok(Body(ops::make_torus(arr(center), major, minor)?))
}

/// Extrude a region from 0 to `height` along the plane's normal (negative goes the other way),
/// its sides leaning in by `taper` radians (0: straight).
pub fn extrude(frame: &Frame, profile: &Profile, height: f64, taper: f64) -> Result<Vec<Body>> {
    let plane = frame.plane()?;
    let height = finite(height, "the height")?;
    if height.abs() <= 1e-9 {
        return Err(err("the height must not be 0"));
    }
    if !(taper.is_finite() && taper.abs() < std::f64::consts::FRAC_PI_2) {
        return Err(err("the taper angle must be between -90 and 90 degrees"));
    }
    if taper == 0.0 {
        let region = profile.to_region()?;
        let (a, b) = if height > 0.0 { (0.0, height) } else { (height, 0.0) };
        return Ok(vec![Body(ops::prism::extrude_planar_regions(&plane, &[region], a, b)?)]);
    }
    // Drafted: the outer loop leans in (the kernel's positive draft), each hole the other way
    // (the material leans in at its walls too) and is taken away.
    profile.check()?;
    let dir = arr(frame.normal() * height.signum());
    let draft = |segs: &[Seg], angle: f64| -> Result<ops::Body> {
        Ok(ops::sweep::extrude(&curves(segs, &plane)?, dir, height.abs(), angle, ops::sweep::DraftCorner::Curvature, true)?)
    };
    let mut body = draft(&profile.outer, taper)?;
    for hole in &profile.holes {
        let cut = draft(hole, -taper)?;
        body = ops::boolean::boolean(body, cut, ops::BooleanOp::Subtract)?.ok_or_else(|| err("the holes leave no solid"))?;
    }
    Ok(vec![Body(body)])
}

/// Revolve a region about the axis through `axis_origin` along `axis_dir` (both in the plane's
/// coordinates) by `angle` radians.
pub fn revolve(frame: &Frame, profile: &Profile, axis_origin: Vec2, axis_dir: Vec2, angle: f64) -> Result<Vec<Body>> {
    let plane = frame.plane()?;
    profile.check()?;
    let (o, d) = (v2(axis_origin, "the axis point")?, v2(axis_dir, "the axis direction")?);
    if !(angle.is_finite() && angle.abs() > 1e-9) {
        return Err(err("the angle must not be 0"));
    }
    let origin = arr(frame.to_world(Vec2::new(o.x, o.y)));
    let axis = unit(frame.x * d.x + frame.y * d.y).ok_or_else(|| err("the axis needs a direction"))?;
    // A negative angle turns the other way: about the reversed axis.
    let axis = arr(if angle < 0.0 { -axis } else { axis });
    let angle = angle.abs().min(std::f64::consts::TAU);
    let turn = |segs: &[Seg]| -> Result<ops::Body> { Ok(ops::sweep::revolve(&curves(segs, &plane)?, origin, axis, angle, true)?) };
    let mut body = turn(&profile.outer)?;
    for hole in &profile.holes {
        let cut = turn(hole)?;
        body = ops::boolean::boolean(body, cut, ops::BooleanOp::Subtract)?.ok_or_else(|| err("the holes leave no solid"))?;
    }
    Ok(vec![Body(body)])
}

/// `a` combined with `b`; `None` when nothing is left (an empty intersection or subtraction).
pub fn boolean(a: &Body, b: &Body, op: BoolOp) -> Result<Option<Body>> {
    let op = match op {
        BoolOp::Union => ops::BooleanOp::Union,
        BoolOp::Subtract => ops::BooleanOp::Subtract,
        BoolOp::Intersect => ops::BooleanOp::Intersect,
    };
    match ops::boolean::boolean(a.0.copy()?, b.0.copy()?, op) {
        Ok(r) => Ok(r.map(Body)),
        Err(ops::Error::Empty) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The parts of `body` on each side of a plane: (the side the normal points to, the other side).
pub fn slice(body: &Body, origin: Vec3, normal: Vec3) -> Result<(Option<Body>, Option<Body>)> {
    let origin = point(origin, "the plane's point")?;
    let n = unit(normal).ok_or_else(|| err("the plane needs a normal direction"))?;
    let (lo, hi) = body.bounds()?;
    let centre = (lo + hi) * 0.5;
    // A half-space as a box much larger than the body, on each side of the plane through the
    // point of the plane nearest the body's centre.
    let foot = centre - n * (centre - origin).dot(n);
    let reach = (hi - lo).len() * 2.0 + (centre - origin).dot(n).abs() + 1.0;
    let frame = Frame::from_normal(foot, n)?;
    let half = |z0: f64| -> Result<Option<Body>> {
        let b = ops::make_box([-reach, -reach, z0], [2.0 * reach, 2.0 * reach, reach])?;
        b.transform(&frame.placement())?;
        boolean(body, &Body(b), BoolOp::Intersect)
    };
    Ok((half(0.0)?, half(-reach)?))
}

/// The record edge nearest each point (once each).
fn nearest_edges(body: &ops::Body, order: &ops::Order, points: &[Vec3]) -> Result<Vec<usize>> {
    let mut out: Vec<usize> = Vec::new();
    for p in points {
        let mut best: Option<(usize, f64)> = None;
        for i in 0..order.edges.len() {
            if let Some(c) = body.edge_closest_point(order, i, arr(*p))?
                && c.distance.is_finite()
                && best.is_none_or(|(_, d)| c.distance < d)
            {
                best = Some((i, c.distance));
            }
        }
        let (i, _) = best.ok_or_else(|| err("no edge of the solid is near the point picked"))?;
        if !out.contains(&i) {
            out.push(i);
        }
    }
    Ok(out)
}

/// The record face nearest each point (once each).
fn nearest_faces(body: &ops::Body, order: &ops::Order, points: &[Vec3]) -> Result<Vec<usize>> {
    let pts: Vec<[f64; 3]> = points.iter().map(|p| arr(*p)).collect();
    let mut best: Vec<Option<(usize, f64)>> = vec![None; pts.len()];
    for f in 0..order.faces.len() {
        for (i, d) in body.face_distances(order, f, &pts)?.into_iter().enumerate() {
            if let (Some(d), Some(slot)) = (d, best.get_mut(i))
                && d.is_finite()
                && slot.is_none_or(|(_, b)| d < b)
            {
                *slot = Some((f, d));
            }
        }
    }
    let mut out: Vec<usize> = Vec::new();
    for b in best {
        let (f, _) = b.ok_or_else(|| err("no face of the solid is near the point picked"))?;
        if !out.contains(&f) {
            out.push(f);
        }
    }
    Ok(out)
}

fn blend(body: &Body, edges: &[Vec3], size: f64, kind: ops::Blend) -> Result<Body> {
    let work = body.0.copy()?;
    let (_, order) = work.to_record()?;
    let picked = nearest_edges(&work, &order, edges)?;
    ops::blend_ordered(&work, &order, kind, &picked, &[size], &[], ops::Corners::Filled, ops::BlendOptions::default())?;
    Ok(Body(work))
}

/// Round the edges nearest to `edges` (points on them) with `radius`.
pub fn fillet(body: &Body, edges: &[Vec3], radius: f64) -> Result<Body> {
    let pts = edges.iter().map(|p| point(*p, "an edge point")).collect::<Result<Vec<_>>>()?;
    let radius = finite(radius, "the radius")?;
    if pts.is_empty() || !positive(radius) {
        return Err(err("select edges and give a positive radius"));
    }
    blend(body, &pts, radius, ops::Blend::Round)
}

/// Bevel the edges nearest to `edges` (points on them) by `distance` on both faces.
pub fn chamfer(body: &Body, edges: &[Vec3], distance: f64) -> Result<Body> {
    let pts = edges.iter().map(|p| point(*p, "an edge point")).collect::<Result<Vec<_>>>()?;
    let distance = finite(distance, "the distance")?;
    if pts.is_empty() || !positive(distance) {
        return Err(err("select edges and give a positive distance"));
    }
    blend(body, &pts, distance, ops::Blend::Chamfer)
}

/// Hollow a body: walls `thickness` thick inside it, the faces nearest to `open` removed.
pub fn shell(body: &Body, open: &[Vec3], thickness: f64) -> Result<Body> {
    let pts = open.iter().map(|p| point(*p, "a face point")).collect::<Result<Vec<_>>>()?;
    let thickness = finite(thickness, "the thickness")?;
    if !positive(thickness) {
        return Err(err("the shell thickness must be positive"));
    }
    let work = body.0.copy()?;
    let (_, order) = work.to_record()?;
    let faces = if pts.is_empty() { Vec::new() } else { nearest_faces(&work, &order, &pts)? };
    // The kernel's positive thickness grows the body outward; a shell's walls go inside it.
    ops::shell(&work, &order, &faces, -thickness)?;
    Ok(Body(work))
}

// ---- reading a body ----

/// What [`Body::encode`] writes first: the format and its version, then solidcore's binary
/// `.smb` contents.
const MAGIC: &[u8; 8] = b"DCSMB001";

/// Largest encoded body accepted from a file.
const MAX_ENCODED: usize = 256 << 20;

/// A body's record, the record order and its display mesh (for drawings).
pub(crate) struct Meshed {
    pub body: ops::Body,
    pub order: ops::Order,
    pub mesh: solidcore_tess::Mesh,
    pub refused: usize,
    pub chord: f64,
}

impl Body {
    /// The body in solidcore's exact binary format (`.smb`) behind DraftCraft's 8-byte header:
    /// what DraftCraft saves.
    pub fn encode(&self) -> Result<Vec<u8>> {
        // Read back once and written again: the form a decoded body writes, so a solid that is
        // opened and saved again keeps its bytes.
        let smb = ops::Body::from_smb(self.0.to_smb(false)?, false)?.to_smb(false)?;
        let mut out = Vec::with_capacity(smb.len() + MAGIC.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&smb);
        Ok(out)
    }

    /// A body from [`Body::encode`]'s bytes. The bytes come from a file: they are checked, and a
    /// body that doesn't decode is an error, never a panic.
    pub fn decode(bytes: &[u8]) -> Result<Body> {
        let smb = bytes.strip_prefix(MAGIC.as_slice()).ok_or_else(|| err("not a DraftCraft solid (unknown format)"))?;
        if smb.is_empty() || smb.len() > MAX_ENCODED {
            return Err(err("the solid's data is empty or too large"));
        }
        let b = ops::Body::from_smb(smb.to_vec(), false)?;
        let [faces, ..] = b.counts()?;
        if faces == 0 {
            return Err(err("the solid's data has no faces"));
        }
        Ok(Body(b))
    }

    /// The body moved by `m` (row-major, `world = m * p`, as `draftcraft_geom::Mat4`): rotations,
    /// translations, reflections and scaling.
    pub fn transformed(&self, m: &Mat4) -> Result<Body> {
        // The kernel's matrix is the transpose: rows the images of the axes, row 3 the
        // translation (`p' = p·A + t`).
        let mut k = [[0.0; 4]; 4];
        for (i, row) in m.m.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                if let Some(cell) = k.get_mut(j).and_then(|col| col.get_mut(i)) {
                    *cell = *v;
                }
            }
        }
        let b = self.0.copy()?;
        b.transform(&k)?;
        Ok(Body(b))
    }

    /// The bounding box.
    pub fn bounds(&self) -> Result<(Vec3, Vec3)> {
        let (lo, hi) = self.0.bounds()?;
        Ok((from_arr(lo), from_arr(hi)))
    }

    /// The body's size: the diagonal of its bounding box (0 when it can't be measured).
    pub fn size(&self) -> f64 {
        self.bounds().map(|(lo, hi)| (hi - lo).len()).ok().filter(|s| s.is_finite()).unwrap_or(0.0)
    }

    pub fn face_count(&self) -> usize {
        self.0.counts().map(|c| c[0]).unwrap_or(0)
    }

    pub fn edge_count(&self) -> usize {
        self.0.counts().map(|c| c[1]).unwrap_or(0)
    }

    /// Volume, surface area, centroid, bounding box and moments of inertia.
    pub fn mass(&self) -> Result<Mass> {
        let m = self.0.mass_properties(ACCURACY)?;
        let (min, max) = self.bounds()?;
        Ok(Mass {
            volume: m.volume,
            area: m.area,
            centroid: from_arr(m.centroid),
            min,
            max,
            moments: from_arr(m.moments),
            products: from_arr(m.products),
        })
    }

    /// The record, its order and the display mesh at chord `tol` (0: 1/1000 of the size).
    pub(crate) fn meshed(&self, tol: f64) -> Result<Meshed> {
        let size = self.size().max(1e-6);
        let chord = if tol.is_finite() && tol > 0.0 { tol } else { (size * 1e-3).max(1e-6) };
        // The record is read from a copy: reading it gives the body's faces their UV trims,
        // which would change what the body saves as.
        let body = self.0.copy()?;
        let (record, order) = body.to_record()?;
        let (plan, _) = solidcore_tess::plan(&record, &solidcore_tess::MeshTolerance::display(chord), size)
            .map_err(|e| err(format!("the solid can't be drawn ({e})")))?;
        let (mesh, refused) = plan.evaluate();
        Ok(Meshed { body, order, mesh, refused: refused.len(), chord })
    }

    /// Edges and triangles to draw it, to a chord tolerance of `tol` (0: 1/1000 of its size).
    pub fn display(&self, tol: f64) -> Result<Display> {
        let m = self.meshed(tol)?.mesh;
        let wires = m
            .edges
            .iter()
            .enumerate()
            .filter(|(i, _)| m.edge_visible(*i) && !is_seam(&m, *i))
            .map(|(_, e)| e.iter().map(|p| from_rec(*p)).collect())
            .collect();
        Ok(Display { wires, positions: m.positions.into_iter().map(from_rec).collect(), triangles: m.triangles, tri_face: m.tri_face })
    }

    /// Whether the two bodies touch or overlap (INTERFERE).
    pub fn meets(&self, other: &Body) -> Result<bool> {
        let tol = (self.size().max(other.size()) * 1e-9).max(1e-9);
        Ok(self.0.meets(&other.0, tol)?)
    }

    /// How many separate solids the body is made of.
    pub fn solid_count(&self) -> Result<usize> {
        Ok(self.0.solid_count()?)
    }

    /// Whether `p` is inside the body (on its boundary counts as inside).
    pub fn contains(&self, p: Vec3) -> bool {
        matches!(self.0.classify_point(arr(p)), Ok(ops::PointClass::Inside | ops::PointClass::OnBoundary))
    }
}

/// Whether mesh edge `i` is a seam: a closed face (a cylinder's side) meeting itself. Seams are
/// not drawn.
pub(crate) fn is_seam(m: &solidcore_tess::Mesh, i: usize) -> bool {
    m.seams.get(i).copied().unwrap_or(false) || m.edge_faces.get(i).is_some_and(|f| f.len() == 1 || f.windows(2).all(|w| w.first() == w.get(1)))
}

/// A record segment as ours, for lines and arcs (others: `None`).
pub(crate) fn from_seg2(s: &rec::Seg2) -> Option<Seg> {
    match *s {
        rec::Seg2::Line { a, b } => Some(Seg::Line { a: Vec2::new(a.x, a.y), b: Vec2::new(b.x, b.y) }),
        rec::Seg2::Arc { center, radius, start, sweep } => Some(Seg::Arc { center: Vec2::new(center.x, center.y), radius, start, sweep }),
        _ => None,
    }
}
