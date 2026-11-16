//! A body as ACIS records (written as SAT 400 or SAB 700 by `model`).
//!
//! The body goes through STEP first: solidcore writes its record as ISO 10303-42 topology on
//! exact surfaces and curves (`solidcore_step::write`), every face's side and loops already
//! settled. STEP and ACIS describe a B-rep the same way, entity for entity, so the STEP entities
//! become ACIS records one to one:
//!
//! | STEP | ACIS |
//! |---|---|
//! | MANIFOLD_SOLID_BREP, BREP_WITH_VOIDS | lump (all of a body's in one `body`) |
//! | CLOSED_SHELL | shell |
//! | ADVANCED_FACE (same_sense) | face (sense) |
//! | FACE_BOUND, FACE_OUTER_BOUND of an EDGE_LOOP | loop |
//! | ORIENTED_EDGE | coedge |
//! | EDGE_CURVE | edge, its parameters those of its vertices on its curve |
//! | VERTEX_POINT | vertex and point |
//! | LINE, CIRCLE, ELLIPSE | straight-curve, ellipse-curve |
//! | B_SPLINE_CURVE | intcurve-curve holding an `exactcur` |
//! | PLANE, CYLINDRICAL_ and CONICAL_SURFACE, SPHERICAL_, TOROIDAL_SURFACE | plane-, cone-, sphere-, torus-surface |
//! | B_SPLINE_SURFACE | spline-surface holding an `exactsur` |
//! | PCURVE of a SURFACE_CURVE or SEAM_CURVE, on a B-spline face | pcurve holding an `exppc` |
//!
//! In both, a face's loops run counter-clockwise about its outward normal. ACIS needs no seam on
//! a face that goes all the way round (a cylinder's side is bounded by its two circles alone), so
//! the seam edges STEP gives such faces are left out, and a sphere or torus with nothing else
//! bounding it is a face without loops (a sphere's poles are no loops either). A cone's apex is
//! a loop of its own, as AutoCAD writes it: one coedge on an edge with no curve that starts and
//! ends at the apex vertex. Without it ShapeManager takes the cone for an invalid solid.
//!
//! Spline faces are written as AutoCAD writes a loft's or a sweep's in DWG and DXF 2000: the
//! surface's B-spline (`nubs`, or `nurbs` with weights) with ACIS's knots (STEP's without the
//! first and last), poles along u first, and its discontinuities; `periodic` across a seam the
//! surface closes smoothly over. Such a face keeps its seam edge, and each of its coedges carries
//! its pcurve: the STEP one, on the ACIS edge's parameter (a linear map: an ACIS line runs on its
//! length, a circle on its angle, a spline on its own), turned round for a coedge that runs
//! against its edge, its surface the face's by reference (`ref`, the subtypes counted in file
//! order). Coedges on analytic faces carry none: the modeller works them out, as AutoCAD's do.

use std::collections::HashMap;
use std::f64::consts::TAU;

use solidcore_step::geom::{Associated, BSplineCurve, BSplineSurface, Curve, P3, Placement, Surface};
use solidcore_step::p21::{Entity, Exchange, Param};
use solidcore_step::read::{Reader, Units};

use super::model::{Field, Record, Tok};
use crate::body::{Body, Result, SolidError};

fn err(m: impl Into<String>) -> SolidError {
    SolidError(m.into())
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn scale(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn unit(a: P3) -> Option<P3> {
    let n = dot(a, a).sqrt();
    (n.is_finite() && n > 0.0).then(|| scale(a, 1.0 / n))
}

/// An ACIS curve: its fields, and how a point maps to its parameter.
#[derive(Clone, Debug)]
pub(super) enum ACurve {
    Straight {
        root: P3,
        dir: P3,
    },
    /// `center + cos t·major + sin t·ratio·(normal × major)`.
    Ellipse {
        center: P3,
        normal: P3,
        major: P3,
        ratio: f64,
    },
    /// A B-spline on its own parameter, the STEP curve's.
    Spline(Box<BSplineCurve>),
}

impl ACurve {
    fn param(&self, p: P3) -> f64 {
        match self {
            ACurve::Straight { root, dir } => dot(sub(p, *root), *dir),
            ACurve::Ellipse { center, normal, major, ratio } => {
                let a = dot(*major, *major).sqrt().max(f64::MIN_POSITIVE);
                let e1 = scale(*major, 1.0 / a);
                let e2 = cross(*normal, e1);
                let d = sub(p, *center);
                (dot(d, e2) / (a * ratio.max(f64::MIN_POSITIVE))).atan2(dot(d, e1) / a)
            }
            // The nearer end of its knot range (an edge's range comes from its pcurves).
            ACurve::Spline(b) => {
                let (Some(&k0), Some(&k1)) = (b.knots.first(), b.knots.last()) else { return 0.0 };
                let d = |q: Option<&P3>| q.map_or(f64::INFINITY, |q| dot(sub(p, *q), sub(p, *q)));
                if d(b.points.first()) <= d(b.points.last()) { k0 } else { k1 }
            }
        }
    }

    fn periodic(&self) -> bool {
        matches!(self, ACurve::Ellipse { .. })
    }

    fn record(&self) -> Record {
        match self {
            ACurve::Straight { root, dir } => {
                Record { name: "straight-curve", fields: vec![Field::Pos(*root), Field::Vector(*dir), Field::Infinite, Field::Infinite] }
            }
            ACurve::Ellipse { center, normal, major, ratio } => Record {
                name: "ellipse-curve",
                fields: vec![
                    Field::Pos(*center),
                    Field::Vector(*normal),
                    Field::Vector(*major),
                    Field::Real(*ratio),
                    Field::Infinite,
                    Field::Infinite,
                ],
            },
            ACurve::Spline(b) => {
                Record { name: "intcurve-curve", fields: vec![Field::Sense(false), Field::Sub(exact_curve(b)), Field::Infinite, Field::Infinite] }
            }
        }
    }
}

/// ACIS's knots of a B-spline from STEP's distinct knots and multiplicities: the full vector
/// without its first and last knot, as (value, multiplicity). `None` when they don't make one.
fn acis_knots(knots: &[f64], mults: &[usize]) -> Option<Vec<(f64, usize)>> {
    if knots.len() != mults.len() || knots.is_empty() || knots.len() > MAX_KNOTS {
        return None;
    }
    let mut out: Vec<(f64, usize)> = knots.iter().copied().zip(mults.iter().copied()).collect();
    if out.iter().any(|(k, m)| !k.is_finite() || *m == 0) || out.windows(2).any(|w| matches!(w, [a, b] if b.0 <= a.0)) {
        return None;
    }
    if let Some(first) = out.first_mut() {
        first.1 = first.1.checked_sub(1)?;
    }
    if let Some(last) = out.last_mut() {
        last.1 = last.1.checked_sub(1)?;
    }
    out.retain(|(_, m)| *m > 0);
    (!out.is_empty()).then_some(out)
}

/// Most distinct knots and poles of one spline written.
const MAX_KNOTS: usize = 1 << 16;
const MAX_POLES: usize = 1 << 20;

/// The knots as SAT lays them out: value and multiplicity, five to a line.
fn knot_tokens(t: &mut Vec<Tok>, knots: &[(f64, usize)]) {
    for (i, (k, m)) in knots.iter().enumerate() {
        t.push(Tok::Real(*k));
        t.push(Tok::Int(i64::try_from(*m).unwrap_or(i64::MAX)));
        if (i + 1) % 5 == 0 || i + 1 == knots.len() {
            t.push(Tok::Line);
        }
    }
}

/// The three discontinuity lists of one direction (C0, C1, C2): the interior knots whose
/// multiplicity leaves the spline that many derivatives fewer; across a periodic seam, the seam's
/// knot first among the smooth ones, as AutoCAD lists it.
fn discontinuity_tokens(t: &mut Vec<Tok>, knots: &[(f64, usize)], degree: usize, periodic: bool) {
    let mut lists: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    if periodic
        && let (Some((k, _)), Some(level)) = (knots.first(), degree.checked_sub(1))
        && let Some(l) = lists.get_mut(level)
    {
        l.push(*k);
    }
    for (k, m) in knots.iter().skip(1).take(knots.len().saturating_sub(2)) {
        if let Some(l) = degree.checked_sub(*m).and_then(|level| lists.get_mut(level)) {
            l.push(*k);
        }
    }
    for l in lists {
        t.push(Tok::Int(i64::try_from(l.len()).unwrap_or(i64::MAX)));
        t.extend(l.into_iter().map(Tok::Real));
        t.push(Tok::Line);
    }
}

fn degree_tok(d: usize) -> Tok {
    Tok::Int(i64::try_from(d).unwrap_or(i64::MAX))
}

fn count_tok(n: usize) -> Tok {
    Tok::Int(i64::try_from(n).unwrap_or(i64::MAX))
}

const OPEN: Tok = Tok::Enum("open", 0);
const PERIODIC: Tok = Tok::Enum("periodic", 2);

/// An edge's B-spline as an `exactcur`: its B-spline, fit tolerance 0, no supports and no
/// pcurves, unbounded, its discontinuities.
fn exact_curve(b: &BSplineCurve) -> Vec<Tok> {
    // Checked when the edge was read (`spline_curve`).
    let knots = acis_knots(&b.knots, &b.multiplicities).unwrap_or_default();
    let mut t = vec![
        Tok::Open,
        Tok::Name("exactcur", "exact_int_cur"),
        full(),
        Tok::Word(if b.weights.is_some() { "nurbs" } else { "nubs" }),
        degree_tok(b.degree),
        OPEN,
        count_tok(knots.len()),
        Tok::Line,
    ];
    knot_tokens(&mut t, &knots);
    for (i, p) in b.points.iter().enumerate() {
        t.extend(p.iter().map(|x| Tok::Real(*x)));
        if let Some(w) = b.weights.as_ref().and_then(|w| w.get(i)) {
            t.push(Tok::Real(*w));
        }
        t.push(Tok::Line);
    }
    t.extend([
        Tok::Real(0.0),
        Tok::Line,
        Tok::Word("null_surface"),
        Tok::Word("null_surface"),
        Tok::Word("nullbs"),
        Tok::Word("nullbs"),
        Tok::Bound(None),
        Tok::Bound(None),
        Tok::Line,
    ]);
    discontinuity_tokens(&mut t, &knots, b.degree, false);
    // The later formats' tail: no extension, and the unextended range unbounded.
    t.extend([Tok::Sab(vec![Tok::Int(0), Tok::Bound(None), Tok::Bound(None)]), Tok::Close]);
    t
}

/// The cache form the later formats write before a spline's data: the whole spline.
fn full() -> Tok {
    Tok::Sab(vec![Tok::Enum("full", 0)])
}

/// A STEP B-spline curve ACIS can take as an `exactcur`: valid knots, poles and weights.
fn spline_curve(b: &BSplineCurve) -> Result<ACurve> {
    let knots = acis_knots(&b.knots, &b.multiplicities).ok_or_else(|| err("a B-spline edge with knots ACIS can't take"))?;
    let total: usize = knots.iter().map(|(_, m)| m).sum();
    let ok_weights = b.weights.as_ref().is_none_or(|w| w.len() == b.points.len() && w.iter().all(|x| x.is_finite() && *x > 0.0));
    if b.degree == 0 || b.points.len() > MAX_POLES || total + 1 != b.points.len() + b.degree || !ok_weights {
        return Err(err("a B-spline edge whose poles don't match its knots"));
    }
    if b.points.iter().flatten().any(|x| !x.is_finite()) {
        return Err(err("a B-spline edge with a pole that is not a number"));
    }
    Ok(ACurve::Spline(Box::new((*b).clone())))
}

/// The ACIS form of a STEP curve, the edge running along it as its parameter grows.
fn curve(c: &Curve) -> Result<ACurve> {
    match c {
        Curve::Line { origin, direction, .. } => {
            Ok(ACurve::Straight { root: *origin, dir: unit(*direction).ok_or_else(|| err("a line without a direction"))? })
        }
        Curve::Circle { position, radius } => {
            Ok(ACurve::Ellipse { center: position.origin, normal: position.axis, major: scale(position.ref_direction, *radius), ratio: 1.0 })
        }
        Curve::Ellipse { position, semi_axis_1, semi_axis_2 } => {
            let (a, b) = (*semi_axis_1, *semi_axis_2);
            if !(a > 0.0 && b > 0.0) {
                return Err(err("an ellipse without size"));
            }
            if a >= b {
                Ok(ACurve::Ellipse { center: position.origin, normal: position.axis, major: scale(position.ref_direction, a), ratio: b / a })
            } else {
                // The major axis along Y: the same normal keeps the direction (t' = t - π/2).
                Ok(ACurve::Ellipse { center: position.origin, normal: position.axis, major: scale(position.y(), b), ratio: a / b })
            }
        }
        Curve::SurfaceCurve { curve_3d, .. } => curve(curve_3d),
        Curve::Trimmed { basis, .. } => curve(basis),
        Curve::BSpline(b) => spline_curve(b),
        _ => Err(err("an edge curve ACIS has no exact form for here")),
    }
}

/// The ACIS record of a STEP surface whose normal it keeps.
fn surface(s: &Surface) -> Result<Record> {
    let four = [Field::Infinite, Field::Infinite, Field::Infinite, Field::Infinite];
    let with = |name: &'static str, mut f: Vec<Field>| {
        f.extend(four.iter().cloned());
        Record { name, fields: f }
    };
    let cone = |p: &Placement, radius: f64, sin: f64, cos: f64| {
        with(
            "cone-surface",
            vec![
                Field::Pos(p.origin),
                Field::Vector(p.axis),
                Field::Vector(scale(p.ref_direction, radius)),
                Field::Real(1.0),
                Field::Infinite,
                Field::Infinite,
                Field::Real(sin),
                Field::Real(cos),
                Field::Real(radius),
                Field::Sense(false),
            ],
        )
    };
    match s {
        Surface::Plane { position } => Ok(with(
            "plane-surface",
            vec![Field::Pos(position.origin), Field::Vector(position.axis), Field::Vector(position.ref_direction), Field::ForwardV],
        )),
        Surface::Cylinder { position, radius } if *radius > 0.0 => Ok(cone(position, *radius, 0.0, 1.0)),
        Surface::Cone { position, radius, semi_angle } if semi_angle.is_finite() && semi_angle.abs() > 0.0 => {
            let (sin, cos) = semi_angle.sin_cos();
            if *radius > 0.0 {
                Ok(cone(position, *radius, sin, cos))
            } else {
                // The base ellipse can't be a point: move it one unit along the axis, to the side
                // the cone is on.
                let v = if *semi_angle > 0.0 { 1.0 } else { -1.0 };
                let p = Placement { origin: add(position.origin, scale(position.axis, v)), ..*position };
                Ok(cone(&p, (v * semi_angle.tan()).abs(), sin, cos))
            }
        }
        Surface::Sphere { position, radius } if *radius != 0.0 => Ok(with(
            "sphere-surface",
            vec![
                Field::Pos(position.origin),
                Field::Real(*radius),
                Field::Vector(position.ref_direction),
                Field::Vector(position.axis),
                Field::ForwardV,
            ],
        )),
        Surface::Torus { position, major_radius, minor_radius } if *major_radius > *minor_radius && *minor_radius > 0.0 => Ok(with(
            "torus-surface",
            vec![
                Field::Pos(position.origin),
                Field::Vector(position.axis),
                Field::Real(*major_radius),
                Field::Real(*minor_radius),
                Field::Vector(position.ref_direction),
                Field::ForwardV,
            ],
        )),
        Surface::BSpline(b) => spline_surface(b),
        _ => Err(err("a face surface ACIS has no exact form for here")),
    }
}

/// Whether the surface closes smoothly across its u (`along_u`) or v boundaries: its first and
/// last rows of poles the same and leaving them the same way, its knots clamped there. ACIS's
/// `periodic`, as AutoCAD marks a sweep's tube.
fn periodic(b: &BSplineSurface, along_u: bool) -> bool {
    let (degree, mults) = if along_u { (b.u_degree, &b.u_multiplicities) } else { (b.v_degree, &b.v_multiplicities) };
    let clamped = mults.first() == Some(&(degree + 1)) && mults.last() == Some(&(degree + 1));
    let rows: Vec<Vec<P3>> = if along_u {
        b.points.clone()
    } else {
        let n = b.points.first().map_or(0, Vec::len);
        (0..n).map(|j| b.points.iter().filter_map(|r| r.get(j).copied()).collect()).collect()
    };
    let n = rows.len();
    if !clamped || n < 3 {
        return false;
    }
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in rows.iter().flatten() {
        for k in 0..3 {
            if let (Some(l), Some(h), Some(x)) = (lo.get_mut(k), hi.get_mut(k), p.get(k)) {
                *l = l.min(*x);
                *h = h.max(*x);
            }
        }
    }
    let size = dot(sub(hi, lo), sub(hi, lo)).sqrt().max(1.0);
    let (Some(first), Some(second), Some(before), Some(last)) = (rows.first(), rows.get(1), rows.get(n - 2), rows.last()) else {
        return false;
    };
    let same = first.iter().zip(last).all(|(a, b)| dot(sub(*a, *b), sub(*a, *b)).sqrt() <= 1e-9 * size);
    let smooth = first.iter().zip(second).zip(before.iter().zip(last)).all(|((p0, p1), (q0, q1))| {
        let (a, b) = (sub(*p1, *p0), sub(*q1, *q0));
        let (na, nb) = (dot(a, a).sqrt(), dot(b, b).sqrt());
        na <= 1e-12 * size || nb <= 1e-12 * size || dot(a, b) >= (1.0 - 1e-6) * na * nb
    });
    let weights = b.weights.as_ref().is_none_or(|w| if along_u { w.first() == w.last() } else { w.iter().all(|r| r.first() == r.last()) });
    same && smooth && weights && first.len() == last.len()
}

/// A STEP B-spline surface as a `spline-surface` holding an `exactsur`, laid out as AutoCAD writes
/// one: degrees, closures, no singularity, the knot counts, both knot lists, the poles along u
/// first, fit tolerance 0, each direction's discontinuities, and the closing `F 1 F 0 F 1 F 0`.
fn spline_surface(b: &BSplineSurface) -> Result<Record> {
    let bad = || err("a B-spline face with knots or poles ACIS can't take");
    let uk = acis_knots(&b.u_knots, &b.u_multiplicities).ok_or_else(bad)?;
    let vk = acis_knots(&b.v_knots, &b.v_multiplicities).ok_or_else(bad)?;
    let (nu, nv) = (b.points.len(), b.points.first().map_or(0, Vec::len));
    let total = |k: &[(f64, usize)]| k.iter().map(|(_, m)| m).sum::<usize>();
    if b.u_degree == 0
        || b.v_degree == 0
        || nu.checked_mul(nv).is_none_or(|n| n > MAX_POLES || n == 0)
        || total(&uk) + 1 != nu + b.u_degree
        || total(&vk) + 1 != nv + b.v_degree
        || b.points.iter().any(|r| r.len() != nv || r.iter().flatten().any(|x| !x.is_finite()))
    {
        return Err(bad());
    }
    if let Some(w) = &b.weights
        && (w.len() != nu || w.iter().any(|r| r.len() != nv || r.iter().any(|x| !(x.is_finite() && *x > 0.0))))
    {
        return Err(bad());
    }
    let (pu, pv) = (periodic(b, true), periodic(b, false));
    let mut t = vec![
        Tok::Open,
        Tok::Name("exactsur", "exact_spl_sur"),
        full(),
        Tok::Word(if b.weights.is_some() { "nurbs" } else { "nubs" }),
        degree_tok(b.u_degree),
        degree_tok(b.v_degree),
        if pu { PERIODIC } else { OPEN },
        if pv { PERIODIC } else { OPEN },
        Tok::Enum("none", 0),
        Tok::Enum("none", 0),
        count_tok(uk.len()),
        count_tok(vk.len()),
        Tok::Line,
    ];
    knot_tokens(&mut t, &uk);
    knot_tokens(&mut t, &vk);
    for j in 0..nv {
        for (i, row) in b.points.iter().enumerate() {
            if let Some(p) = row.get(j) {
                t.extend(p.iter().map(|x| Tok::Real(*x)));
            }
            if let Some(w) = b.weights.as_ref().and_then(|w| w.get(i)).and_then(|r| r.get(j)) {
                t.push(Tok::Real(*w));
            }
            t.push(Tok::Line);
        }
    }
    t.extend([Tok::Real(0.0), Tok::Line]);
    discontinuity_tokens(&mut t, &uk, b.u_degree, pu);
    discontinuity_tokens(&mut t, &vk, b.v_degree, pv);
    // SAT 400 closes with four range ends, AutoCAD's data store with one false.
    t.extend([
        Tok::Sat(vec![Tok::Bound(Some(1.0)), Tok::Bound(Some(0.0)), Tok::Bound(Some(1.0)), Tok::Bound(Some(0.0))]),
        Tok::Sab(vec![Tok::Bound(None)]),
        Tok::Close,
    ]);
    Ok(Record {
        name: "spline-surface",
        fields: vec![Field::Sense(false), Field::Sub(t), Field::Infinite, Field::Infinite, Field::Infinite, Field::Infinite],
    })
}

/// A pcurve as STEP gives it: a 2D B-spline on the STEP edge's parameter, `[a, b]` its knot range,
/// running along the edge's curve.
#[derive(Clone, Debug)]
pub(super) struct Bs2 {
    degree: usize,
    /// The full knot vector.
    knots: Vec<f64>,
    poles: Vec<[f64; 2]>,
    weights: Option<Vec<f64>>,
}

impl Bs2 {
    /// From a STEP PCURVE's 2D B-spline; `None` for any other 2D curve or one that doesn't hold.
    fn of(c: &Curve) -> Option<Bs2> {
        let Curve::BSpline(b) = c else { return None };
        let mut knots = Vec::new();
        for (k, m) in b.knots.iter().zip(&b.multiplicities) {
            if knots.len().checked_add(*m)? > MAX_POLES || !k.is_finite() {
                return None;
            }
            knots.extend(std::iter::repeat_n(*k, *m));
        }
        let poles: Vec<[f64; 2]> = b.points.iter().map(|p| [p[0], p[1]]).collect();
        let fine = b.degree > 0
            && knots.len() == poles.len() + b.degree + 1
            && knots.windows(2).all(|w| matches!(w, [a, b] if a <= b))
            && poles.iter().flatten().all(|x| x.is_finite())
            && b.weights.as_ref().is_none_or(|w| w.len() == poles.len() && w.iter().all(|x| x.is_finite() && *x > 0.0));
        fine.then(|| Bs2 { degree: b.degree, knots, poles, weights: b.weights.clone() })
    }

    /// Its parameter range: the knots past the first `degree` and before the last.
    fn range(&self) -> Option<[f64; 2]> {
        let a = *self.knots.get(self.degree)?;
        let b = *self.knots.get(self.knots.len().checked_sub(self.degree + 1)?)?;
        (a < b).then_some([a, b])
    }

    /// Where it starts and ends in (u, v), along the edge (its end poles: the knots clamped).
    fn ends(&self) -> Option<([f64; 2], [f64; 2])> {
        Some((*self.poles.first()?, *self.poles.last()?))
    }

    /// As an `exppc` on the ACIS parameter: `[a, b]` mapped linearly onto `[t0, t1]`, and turned
    /// round (parameter negated) for a coedge running against its edge, on the face's surface,
    /// the `surface`-th subtype written.
    fn record(&self, [t0, t1]: [f64; 2], reversed: bool, surface: usize) -> Option<Record> {
        let [a, b] = self.range()?;
        let k = (t1 - t0) / (b - a);
        if !(k.is_finite() && k > 0.0) {
            return None;
        }
        let mut knots: Vec<f64> = self.knots.iter().map(|s| t0 + (s - a) * k).collect();
        let mut poles = self.poles.clone();
        let mut weights = self.weights.clone();
        if reversed {
            knots = knots.iter().rev().map(|s| -s).collect();
            poles.reverse();
            if let Some(w) = weights.as_mut() {
                w.reverse();
            }
        }
        // ACIS's knots: without the first and last.
        let mut distinct: Vec<(f64, usize)> = Vec::new();
        for s in knots.iter().skip(1).take(knots.len().saturating_sub(2)) {
            match distinct.last_mut() {
                Some((v, m)) if *v == *s => *m += 1,
                _ => distinct.push((*s, 1)),
            }
        }
        let mut t = vec![
            Tok::Open,
            Tok::Name("exppc", "exp_par_cur"),
            Tok::Word(if weights.is_some() { "nurbs" } else { "nubs" }),
            degree_tok(self.degree),
            OPEN,
            count_tok(distinct.len()),
            Tok::Line,
        ];
        knot_tokens(&mut t, &distinct);
        for (i, p) in poles.iter().enumerate() {
            t.extend([Tok::Real(p[0]), Tok::Real(p[1])]);
            if let Some(w) = weights.as_ref().and_then(|w| w.get(i)) {
                t.push(Tok::Real(*w));
            }
            t.push(Tok::Line);
        }
        t.extend([
            Tok::Real(0.0),
            Tok::Line,
            Tok::Word("spline"),
            Tok::Sense(false),
            Tok::Open,
            Tok::Name("ref", "ref"),
            count_tok(surface),
            Tok::Close,
            Tok::Bound(None),
            Tok::Bound(None),
            Tok::Bound(None),
            Tok::Bound(None),
            Tok::Line,
            Tok::Close,
        ]);
        Some(Record { name: "pcurve", fields: vec![Field::Int(0), Field::Sense(false), Field::Sub(t), Field::Real(0.0), Field::Real(0.0)] })
    }
}

fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

/// One use of an edge in a loop: the edge (into the edge table), whether the loop runs against
/// it, and its pcurve (into the pcurve table) on a spline face.
#[derive(Clone, Copy, Debug)]
pub(super) struct Use {
    pub(super) edge: usize,
    pub(super) reversed: bool,
    pub(super) pcurve: Option<usize>,
}

pub(super) struct AEdge {
    pub(super) start: usize,
    pub(super) end: usize,
    /// `None` for a point edge: a cone's apex.
    pub(super) curve: Option<ACurve>,
    /// The PCURVEs STEP gives the edge on spline faces: their surface and curve.
    pub(super) pcurves: Vec<(Surface, Bs2)>,
}

pub(super) struct AFace {
    pub(super) surface: Record,
    pub(super) reversed: bool,
    pub(super) loops: Vec<Vec<Use>>,
    /// On a spline surface: its coedges carry pcurves referring to it.
    pub(super) spline: bool,
    /// Double-sided, as a region's (a sheet, no inside): `double out`.
    pub(super) double: bool,
}

/// The body's B-rep read from the STEP entities, in ACIS terms.
#[derive(Default)]
pub(super) struct Model {
    /// Lumps, each its shells, each its faces.
    pub(super) lumps: Vec<Vec<Vec<AFace>>>,
    pub(super) edges: Vec<AEdge>,
    edge_of: HashMap<u64, usize>,
    pub(super) points: Vec<P3>,
    vertex_of: HashMap<u64, usize>,
    pcurves: Vec<Bs2>,
}

fn refs(p: Option<&Param>) -> Vec<u64> {
    p.and_then(Param::as_list).map(|l| l.iter().filter_map(Param::as_ref_id).collect()).unwrap_or_default()
}

fn entity(ex: &Exchange, id: u64) -> Result<&Entity> {
    ex.get(id).ok_or_else(|| err(format!("STEP entity #{id} is missing")))
}

impl Model {
    fn vertex(&mut self, ex: &Exchange, r: &Reader<'_>, id: u64) -> Result<usize> {
        if let Some(&v) = self.vertex_of.get(&id) {
            return Ok(v);
        }
        let point = entity(ex, id)?.params().get(1).and_then(Param::as_ref_id).ok_or_else(|| err("a vertex without a point"))?;
        let p = r.point(point).map_err(err)?;
        self.points.push(p);
        let v = self.points.len() - 1;
        self.vertex_of.insert(id, v);
        Ok(v)
    }

    /// The edge of an EDGE_CURVE, and whether it runs against the STEP edge.
    fn edge(&mut self, ex: &Exchange, r: &mut Reader<'_>, id: u64) -> Result<(usize, bool)> {
        let e = entity(ex, id)?;
        let p = e.params();
        let same_sense = p.get(4).and_then(Param::as_bool).unwrap_or(true);
        if let Some(&i) = self.edge_of.get(&id) {
            return Ok((i, !same_sense));
        }
        let (a, b, geometry) = match (p.get(1).and_then(Param::as_ref_id), p.get(2).and_then(Param::as_ref_id), p.get(3).and_then(Param::as_ref_id)) {
            (Some(a), Some(b), Some(g)) => (a, b, g),
            _ => return Err(err("an edge without its vertices or curve")),
        };
        let (start, end) = (self.vertex(ex, r, a)?, self.vertex(ex, r, b)?);
        let step_curve = r.curve(geometry).map_err(err)?;
        let c = curve(&step_curve)?;
        let mut pcurves = Vec::new();
        if let Curve::SurfaceCurve { associated, .. } = &*step_curve {
            for a in associated {
                if let Associated::Pcurve(p) = a
                    && matches!(*p.surface, Surface::BSpline(_))
                    && let Some(c2) = Bs2::of(&p.curve)
                {
                    pcurves.push(((*p.surface).clone(), c2));
                }
            }
        }
        // The ACIS edge runs as its curve does: from the STEP edge's start when the senses agree.
        let (start, end) = if same_sense { (start, end) } else { (end, start) };
        self.edges.push(AEdge { start, end, curve: Some(c), pcurves });
        let i = self.edges.len() - 1;
        self.edge_of.insert(id, i);
        Ok((i, !same_sense))
    }

    /// The loop at a cone's apex: one use of a point edge (no curve, starting and ending at the
    /// apex vertex), the way AutoCAD writes it.
    fn apex_loop(&mut self, v: usize) -> Vec<Use> {
        self.edges.push(AEdge { start: v, end: v, curve: None, pcurves: Vec::new() });
        vec![Use { edge: self.edges.len() - 1, reversed: true, pcurve: None }]
    }

    /// Each use of a loop on the spline surface `s` given its pcurve: the edge's one on `s`, or,
    /// for a seam's two, the one that runs on from the use before it and into the one after.
    fn give_pcurves(&mut self, s: &Surface, uses: &mut [Use]) {
        let candidates: Vec<Vec<&Bs2>> = uses
            .iter()
            .map(|u| self.edges.get(u.edge).map(|e| e.pcurves.iter().filter(|(on, _)| on == s).map(|(_, c)| c).collect()).unwrap_or_default())
            .collect();
        // Where a use starts and ends in (u, v), running its way.
        let directed = |c: &Bs2, u: &Use| c.ends().map(|(a, b)| if u.reversed { (b, a) } else { (a, b) });
        let n = uses.len();
        let mut chosen: Vec<Option<&Bs2>> = candidates.iter().map(|c| if c.len() == 1 { c.first().copied() } else { None }).collect();
        for i in 0..n {
            let (Some(cands), Some(u)) = (candidates.get(i), uses.get(i)) else { continue };
            if cands.len() < 2 {
                continue;
            }
            let neighbour = |k: usize| -> Option<([f64; 2], [f64; 2])> {
                let c = chosen.get(k).copied().flatten().or_else(|| candidates.get(k).and_then(|c| c.first().copied()))?;
                directed(c, uses.get(k)?)
            };
            let before = neighbour((i + n - 1) % n).map(|(_, e)| e);
            let after = neighbour((i + 1) % n).map(|(s, _)| s);
            let misfit =
                |c: &Bs2| directed(c, u).map_or(f64::INFINITY, |(s, e)| before.map_or(0.0, |b| dist2(b, s)) + after.map_or(0.0, |a| dist2(e, a)));
            let best = cands.iter().copied().min_by(|a, b| misfit(a).total_cmp(&misfit(b)));
            if let Some(slot) = chosen.get_mut(i) {
                *slot = best;
            }
        }
        let picked: Vec<Option<Bs2>> = chosen.into_iter().map(|c| c.cloned()).collect();
        for (u, c) in uses.iter_mut().zip(picked) {
            if let Some(c) = c {
                self.pcurves.push(c);
                u.pcurve = Some(self.pcurves.len() - 1);
            }
        }
    }

    /// The vertices of `all`'s edges that no edge of `kept` reaches: where only seams went (a
    /// cone's apex, which STEP reaches by the seam line).
    fn left_by_seams(&self, all: &[Vec<Use>], kept: &[Vec<Use>]) -> Vec<usize> {
        let ends = |loops: &[Vec<Use>]| -> Vec<usize> {
            loops.iter().flatten().filter_map(|u| self.edges.get(u.edge)).flat_map(|e| [e.start, e.end]).collect()
        };
        let reached = ends(kept);
        let mut out: Vec<usize> = Vec::new();
        for v in ends(all) {
            if !reached.contains(&v) && !out.contains(&v) {
                out.push(v);
            }
        }
        out
    }

    fn face(&mut self, ex: &Exchange, r: &mut Reader<'_>, id: u64, flip: bool) -> Result<AFace> {
        let e = entity(ex, id)?;
        if e.name() == "ORIENTED_FACE" {
            let inner = e.params().get(2).and_then(Param::as_ref_id).ok_or_else(|| err("an oriented face without its face"))?;
            let orientation = e.params().get(3).and_then(Param::as_bool).unwrap_or(true);
            return self.face(ex, r, inner, flip ^ !orientation);
        }
        if e.name() != "ADVANCED_FACE" {
            return Err(err(format!("a {} face", e.name())));
        }
        let p = e.params();
        let surface_id = p.get(2).and_then(Param::as_ref_id).ok_or_else(|| err("a face without a surface"))?;
        let same_sense = p.get(3).and_then(Param::as_bool).unwrap_or(true);
        let step_surface = r.surface(surface_id).map_err(err)?;
        let is_cone = matches!(*step_surface, Surface::Cone { .. });
        let spline = matches!(*step_surface, Surface::BSpline(_));
        let s = surface(&step_surface)?;
        let mut loops = Vec::new();
        for bound in refs(p.get(1)) {
            let b = entity(ex, bound)?;
            let lp = b.params().get(1).and_then(Param::as_ref_id).ok_or_else(|| err("a face bound without its loop"))?;
            let orientation = b.params().get(2).and_then(Param::as_bool).unwrap_or(true);
            let l = entity(ex, lp)?;
            if l.name() == "VERTEX_LOOP" {
                // A cone's apex is a loop of one point edge in ACIS; a sphere's pole is nothing.
                if is_cone {
                    let vertex = l.params().get(1).and_then(Param::as_ref_id).ok_or_else(|| err("an apex loop without its vertex"))?;
                    let v = self.vertex(ex, r, vertex)?;
                    loops.push(self.apex_loop(v));
                }
                continue;
            }
            if l.name() != "EDGE_LOOP" {
                return Err(err(format!("a {} loop", l.name())));
            }
            let mut uses = Vec::new();
            for oe in refs(l.params().get(1)) {
                let o = entity(ex, oe)?;
                let edge = o.params().get(3).and_then(Param::as_ref_id).ok_or_else(|| err("an oriented edge without its edge"))?;
                let forward = o.params().get(4).and_then(Param::as_bool).unwrap_or(true);
                let (edge, against) = self.edge(ex, r, edge)?;
                uses.push(Use { edge, reversed: !forward ^ against, pcurve: None });
            }
            if !(orientation ^ flip) {
                uses.reverse();
                for u in &mut uses {
                    u.reversed = !u.reversed;
                }
            }
            if spline {
                self.give_pcurves(&step_surface, &mut uses);
            }
            loops.push(uses);
        }
        // A spline face keeps its seam, as AutoCAD's sweep does.
        let mut kept = if spline { loops.clone() } else { self.without_seams(loops.clone()) };
        if is_cone {
            for v in self.left_by_seams(&loops, &kept) {
                let apex = self.apex_loop(v);
                kept.push(apex);
            }
        }
        Ok(AFace { surface: s, reversed: !(same_sense ^ flip), loops: kept, spline, double: false })
    }

    /// Where a use starts and ends.
    fn ends(&self, u: Use) -> Option<(usize, usize)> {
        let e = self.edges.get(u.edge)?;
        Some(if u.reversed { (e.end, e.start) } else { (e.start, e.end) })
    }

    /// A face's loops without the seams STEP needs and ACIS doesn't: the edges one face uses
    /// twice, the rest of each loop split where it closes. A loop that doesn't close without its
    /// seams keeps them.
    fn without_seams(&self, loops: Vec<Vec<Use>>) -> Vec<Vec<Use>> {
        let mut count: HashMap<usize, usize> = HashMap::new();
        for u in loops.iter().flatten() {
            *count.entry(u.edge).or_insert(0) += 1;
        }
        let mut out = Vec::new();
        for l in loops {
            let kept: Vec<Use> = l.iter().copied().filter(|u| count.get(&u.edge).copied().unwrap_or(0) < 2).collect();
            if kept.len() == l.len() {
                out.push(l);
                continue;
            }
            let mut split: Vec<Vec<Use>> = Vec::new();
            let mut current: Vec<Use> = Vec::new();
            let mut closes = true;
            for (i, u) in kept.iter().enumerate() {
                if let (Some(prev), Some(here)) = (current.last().and_then(|p| self.ends(*p)), self.ends(*u))
                    && prev.1 != here.0
                {
                    closes = false;
                }
                current.push(*u);
                let first = current.first().and_then(|f| self.ends(*f)).map(|e| e.0);
                if first.is_some() && first == self.ends(*u).map(|e| e.1) {
                    split.push(std::mem::take(&mut current));
                } else if i + 1 == kept.len() {
                    closes = false;
                }
            }
            if closes && current.is_empty() {
                out.extend(split);
            } else {
                out.push(l);
            }
        }
        out
    }
}

/// The STEP text's solids as one ACIS body's records.
fn records(ex: &Exchange) -> Result<Vec<Record>> {
    let mut r = Reader::new(ex, Units::default());
    let mut m = Model::default();
    for (_, e) in ex.entities.iter().filter(|(_, e)| matches!(e.name(), "MANIFOLD_SOLID_BREP" | "BREP_WITH_VOIDS")) {
        let p = e.params();
        let mut shells: Vec<(u64, bool)> = p.get(1).and_then(Param::as_ref_id).map(|s| vec![(s, false)]).unwrap_or_default();
        for void in refs(p.get(2)) {
            // ORIENTED_CLOSED_SHELL('', *, shell, orientation)
            let v = entity(ex, void)?;
            let inner = v.params().get(2).and_then(Param::as_ref_id).unwrap_or(void);
            let orientation = v.params().get(3).and_then(Param::as_bool).unwrap_or(true);
            shells.push((inner, !orientation));
        }
        let mut lump = Vec::new();
        for (shell, flip) in shells {
            let mut faces = Vec::new();
            for f in refs(entity(ex, shell)?.params().get(1)) {
                faces.push(m.face(ex, &mut r, f, flip)?);
            }
            lump.push(faces);
        }
        m.lumps.push(lump);
    }
    if m.lumps.is_empty() {
        return Err(err("the body has no solid to write"));
    }
    layout(&m)
}

/// An edge's ACIS parameters at its start and end: a periodic curve's end past its start, by up
/// to a turn; a spline's its pcurves' range (the STEP edge's), else its own ends'. A point edge
/// has the empty range AutoCAD gives it: from 1 to 0.
fn edge_range(m: &Model, edge: &AEdge) -> Result<[f64; 2]> {
    let (ps, pe) = (m.points.get(edge.start), m.points.get(edge.end));
    let (Some(ps), Some(pe)) = (ps, pe) else { return Err(err("an edge without its points")) };
    let given = edge.pcurves.first().and_then(|(_, c)| c.range());
    let (t0, mut t1) = match (&edge.curve, given) {
        (Some(ACurve::Spline(_)), Some([a, b])) => (a, b),
        (Some(c), _) => (c.param(*ps), c.param(*pe)),
        (None, _) => (1.0, 0.0),
    };
    if edge.curve.as_ref().is_some_and(ACurve::periodic) {
        while t1 <= t0 + 1e-12 {
            t1 += TAU;
        }
        while t1 > t0 + TAU + 1e-12 {
            t1 -= TAU;
        }
    }
    Ok([t0, t1])
}

/// The model's records in file order: the body, lumps, shells, faces, loops, coedges, edges,
/// vertices, points, surfaces, curves and pcurves.
pub(super) fn layout(m: &Model) -> Result<Vec<Record>> {
    let faces: Vec<&AFace> = m.lumps.iter().flatten().flatten().collect();
    let lumps = m.lumps.len();
    let shells: usize = m.lumps.iter().map(Vec::len).sum();
    let loops: usize = faces.iter().map(|f| f.loops.len()).sum();
    let coedges: usize = faces.iter().flat_map(|f| f.loops.iter()).map(Vec::len).sum();
    // Only edges some face uses, and their vertices.
    let mut edge_index: Vec<Option<usize>> = vec![None; m.edges.len()];
    let mut used_edges = Vec::new();
    for u in faces.iter().flat_map(|f| f.loops.iter()).flatten() {
        if let Some(slot) = edge_index.get_mut(u.edge)
            && slot.is_none()
        {
            *slot = Some(used_edges.len());
            used_edges.push(u.edge);
        }
    }
    let mut vertex_index: Vec<Option<usize>> = vec![None; m.points.len()];
    let mut used_vertices = Vec::new();
    for &e in &used_edges {
        let edge = m.edges.get(e).ok_or_else(|| err("an edge out of range"))?;
        for v in [edge.start, edge.end] {
            if let Some(slot) = vertex_index.get_mut(v)
                && slot.is_none()
            {
                *slot = Some(used_vertices.len());
                used_vertices.push(v);
            }
        }
    }
    let base_lump = 1;
    let base_shell = base_lump + lumps;
    let base_face = base_shell + shells;
    let base_loop = base_face + faces.len();
    let base_coedge = base_loop + loops;
    let base_edge = base_coedge + coedges;
    let base_vertex = base_edge + used_edges.len();
    let base_point = base_vertex + used_vertices.len();
    let base_surface = base_point + used_vertices.len();
    let base_curve = base_surface + faces.len();
    // Each used edge's curve record, point edges having none.
    let mut curve_index: Vec<Option<usize>> = Vec::with_capacity(used_edges.len());
    let mut curves = 0usize;
    for &e in &used_edges {
        let has = m.edges.get(e).is_some_and(|ed| ed.curve.is_some());
        curve_index.push(has.then(|| {
            curves += 1;
            base_curve + curves - 1
        }));
    }
    let base_pcurve = base_curve + curves;
    let mut ranges: Vec<[f64; 2]> = Vec::with_capacity(used_edges.len());
    for &e in &used_edges {
        ranges.push(edge_range(m, m.edges.get(e).ok_or_else(|| err("an edge out of range"))?)?);
    }
    // Each spline face's surface is the subtype it is in file order: the surfaces come before
    // every other subtype.
    let mut subtype_of: Vec<Option<usize>> = Vec::with_capacity(faces.len());
    let mut splines = 0usize;
    for f in &faces {
        subtype_of.push(f.spline.then(|| {
            splines += 1;
            splines - 1
        }));
    }

    // Coedges: their loop, place and use, and each edge's uses in order.
    struct Co {
        loop_index: usize,
        next: usize,
        prev: usize,
        u: Use,
    }
    let mut cos: Vec<Co> = Vec::with_capacity(coedges);
    let mut loop_first: Vec<usize> = Vec::with_capacity(loops);
    let mut face_first_loop: Vec<Option<usize>> = Vec::with_capacity(faces.len());
    let mut loop_face: Vec<usize> = Vec::with_capacity(loops);
    for (fi, f) in faces.iter().enumerate() {
        face_first_loop.push((!f.loops.is_empty()).then_some(loop_first.len()));
        for l in &f.loops {
            let li = loop_first.len();
            let first = cos.len();
            loop_first.push(first);
            loop_face.push(fi);
            let n = l.len();
            for (k, u) in l.iter().enumerate() {
                cos.push(Co { loop_index: li, next: first + (k + 1) % n, prev: first + (k + n - 1) % n, u: *u });
            }
        }
    }
    let mut uses_of: Vec<Vec<usize>> = vec![Vec::new(); used_edges.len()];
    for (ci, c) in cos.iter().enumerate() {
        if let Some(e) = edge_index.get(c.u.edge).copied().flatten()
            && let Some(list) = uses_of.get_mut(e)
        {
            list.push(ci);
        }
    }
    // The pcurve records of the coedges on spline faces, in coedge order.
    let mut pcurve_records: Vec<Record> = Vec::new();
    let mut pcurve_of: Vec<Option<usize>> = Vec::with_capacity(cos.len());
    for c in &cos {
        let face = loop_face.get(c.loop_index).copied().unwrap_or(0);
        let record = (|| {
            let surface = subtype_of.get(face).copied().flatten()?;
            let pc = m.pcurves.get(c.u.pcurve?)?;
            let range = ranges.get(edge_index.get(c.u.edge).copied().flatten()?).copied()?;
            pc.record(range, c.u.reversed, surface)
        })();
        pcurve_of.push(record.map(|r| {
            pcurve_records.push(r);
            base_pcurve + pcurve_records.len() - 1
        }));
    }

    let mut out: Vec<Record> = Vec::new();
    out.push(Record { name: "body", fields: vec![Field::Ref(Some(base_lump)), Field::Ref(None), Field::Ref(None)] });
    let mut shell_at = base_shell;
    let mut face_at = base_face;
    for (li, lump) in m.lumps.iter().enumerate() {
        out.push(Record {
            name: "lump",
            fields: vec![Field::Ref((li + 1 < lumps).then_some(base_lump + li + 1)), Field::Ref(Some(shell_at)), Field::Ref(Some(0))],
        });
        shell_at += lump.len();
    }
    let mut shell_index = base_shell;
    for (li, lump) in m.lumps.iter().enumerate() {
        for (si, shell) in lump.iter().enumerate() {
            out.push(Record {
                name: "shell",
                fields: vec![
                    Field::Ref((si + 1 < lump.len()).then_some(shell_index + 1)),
                    Field::Ref(None),
                    Field::Ref((!shell.is_empty()).then_some(face_at)),
                    Field::Ref(None),
                    Field::Ref(Some(base_lump + li)),
                ],
            });
            shell_index += 1;
            face_at += shell.len();
        }
    }
    let mut fi = 0usize;
    let mut shell_index = base_shell;
    for lump in &m.lumps {
        for shell in lump {
            for (k, f) in shell.iter().enumerate() {
                out.push(Record {
                    name: "face",
                    fields: vec![
                        Field::Ref((k + 1 < shell.len()).then_some(base_face + fi + 1)),
                        Field::Ref(face_first_loop.get(fi).copied().flatten().map(|l| base_loop + l)),
                        Field::Ref(Some(shell_index)),
                        Field::Ref(None),
                        Field::Ref(Some(base_surface + fi)),
                        Field::Sense(f.reversed),
                        if f.double { Field::Double } else { Field::Single },
                    ],
                });
                fi += 1;
            }
            shell_index += 1;
        }
    }
    for (li, &first) in loop_first.iter().enumerate() {
        let face = loop_face.get(li).copied().unwrap_or(0);
        let next = loop_face.get(li + 1).filter(|f| **f == face).map(|_| base_loop + li + 1);
        out.push(Record { name: "loop", fields: vec![Field::Ref(next), Field::Ref(Some(base_coedge + first)), Field::Ref(Some(base_face + face))] });
    }
    for (ci, c) in cos.iter().enumerate() {
        let e = edge_index.get(c.u.edge).copied().flatten().ok_or_else(|| err("a coedge without its edge"))?;
        let uses = uses_of.get(e).ok_or_else(|| err("an edge without uses"))?;
        let at = uses.iter().position(|u| *u == ci).unwrap_or(0);
        let partner = (uses.len() > 1).then(|| uses.get((at + 1) % uses.len()).copied().unwrap_or(ci)).map(|p| base_coedge + p);
        out.push(Record {
            name: "coedge",
            fields: vec![
                Field::Ref(Some(base_coedge + c.next)),
                Field::Ref(Some(base_coedge + c.prev)),
                Field::Ref(partner),
                Field::Ref(Some(base_edge + e)),
                Field::Sense(c.u.reversed),
                Field::Ref(Some(base_loop + c.loop_index)),
                Field::Ref(pcurve_of.get(ci).copied().flatten()),
            ],
        });
    }
    for (k, &e) in used_edges.iter().enumerate() {
        let edge = m.edges.get(e).ok_or_else(|| err("an edge out of range"))?;
        let [t0, t1] = ranges.get(k).copied().ok_or_else(|| err("an edge without its range"))?;
        let v = |p: usize| vertex_index.get(p).copied().flatten().map(|i| base_vertex + i);
        let first_use = uses_of.get(k).and_then(|u| u.first()).map(|c| base_coedge + c);
        out.push(Record {
            name: "edge",
            fields: vec![
                Field::Ref(v(edge.start)),
                Field::Param(t0),
                Field::Ref(v(edge.end)),
                Field::Param(t1),
                Field::Ref(first_use),
                Field::Ref(curve_index.get(k).copied().flatten()),
                Field::Sense(false),
                Field::Convexity,
            ],
        });
    }
    for (k, &p) in used_vertices.iter().enumerate() {
        let edge = used_edges.iter().position(|&e| m.edges.get(e).is_some_and(|ed| ed.start == p || ed.end == p)).map(|i| base_edge + i);
        out.push(Record { name: "vertex", fields: vec![Field::Ref(edge), Field::Ref(Some(base_point + k))] });
    }
    for &p in &used_vertices {
        out.push(Record { name: "point", fields: vec![Field::Pos(m.points.get(p).copied().unwrap_or([0.0; 3]))] });
    }
    for f in &faces {
        out.push(f.surface.clone());
    }
    for &e in &used_edges {
        if let Some(c) = &m.edges.get(e).ok_or_else(|| err("an edge out of range"))?.curve {
            out.push(c.record());
        }
    }
    out.extend(pcurve_records);
    Ok(out)
}

/// The body's ACIS records: through STEP, as the module says.
pub(crate) fn body_records(body: &Body) -> Result<Vec<Record>> {
    // The record is read from a copy: reading it gives the body's faces their UV trims.
    let copy = body.0.copy()?;
    let header = solidcore_step::write::product::Header::default();
    let text = solidcore_step::write::product::solidcore_bodies(&[("solid", &copy)], &header)
        .map_err(|e| err(format!("the solid can't be written ({e})")))?;
    let ex = solidcore_step::p21::parse(&text).map_err(|e| err(format!("the solid can't be written ({e})")))?;
    records(&ex)
}
