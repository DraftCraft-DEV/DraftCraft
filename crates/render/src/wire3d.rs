//! Objects in 3D, for a 3D view: where an object sits in space, which the plan display list
//! (in the XY plane) leaves out.
//!
//! [`placement`] says how an object is drawn in a 3D view, as AutoCAD's 3D views draw it:
//! - an object lying in a plane parallel to XY with no thickness is its plan drawing lifted to
//!   its height ([`Placement::Lifted`]), so linetypes, widths, fills and text stay as in plan;
//! - an object off that plane, in another plane, with a thickness, or made of 3D points (3D
//!   faces, polyface meshes, 3D polylines, helices) is drawn from its own geometry as edges and
//!   faces ([`Placement::Spatial`]);
//! - anything else stays on the XY plane ([`Placement::Flat`]).

use draftcraft_doc::{Common, Entity, EntityKind};
use draftcraft_geom::{Mat4, PolyVertex, Vec2, Vec3};

/// An object's edges (polylines) and faces (triangles), in the drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wire3d {
    pub lines: Vec<Vec<Vec3>>,
    pub faces: Vec<[Vec3; 3]>,
}

/// How an object is drawn in a 3D view.
#[derive(Clone, Debug, PartialEq)]
pub enum Placement {
    /// Its plan drawing on the XY plane.
    Flat,
    /// Its plan drawing moved up to this height.
    Lifted(f64),
    /// Its own edges and faces.
    Spatial(Wire3d),
}

/// Segments a full circle is drawn with in 3D.
const CIRCLE_SEGMENTS: usize = 72;
/// Most points one object gives (the rest is left out).
const MAX_POINTS: usize = 200_000;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

/// Whether `n` is the +Z direction (the plan's own OCS).
fn up(n: Vec3) -> bool {
    n.x.abs() < 1e-12 && n.y.abs() < 1e-12 && n.z > 0.0
}

/// Whether `n` is ±Z: a plane parallel to XY (a mirrored OCS is still one).
fn level(n: Vec3) -> bool {
    n.x.abs() < 1e-12 && n.y.abs() < 1e-12 && n.z != 0.0
}

fn finite(p: Vec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

/// The object's thickness, when it has one that shows (finite, not 0).
fn thickness(c: &Common) -> Option<f64> {
    (c.thickness.is_finite() && c.thickness != 0.0).then_some(c.thickness)
}

/// The unit normal of the object's plane (its extrusion), +Z when it has none.
fn normal(c: &Common) -> Vec3 {
    let n = c.extrusion.normalized();
    if finite(n) && n != Vec3::ZERO { n } else { Vec3::Z }
}

/// How `e` is drawn in a 3D view (objects inside blocks too: the caller places them).
pub fn placement(e: &Entity) -> Placement {
    let c = &e.common;
    let n = normal(c);
    let thick = thickness(c);
    let lifted = |z: f64| if z.is_finite() && z != 0.0 { Placement::Lifted(z) } else { Placement::Flat };
    match &e.kind {
        EntityKind::Line(l) => {
            if thick.is_none() && near(l.a.z, l.b.z) {
                lifted(l.a.z)
            } else {
                spatial(e)
            }
        }
        EntityKind::Point(p) if thick.is_none() => lifted(p.p.z),
        EntityKind::Circle(ci) if thick.is_none() && level(n) => lifted(ci.center.z),
        EntityKind::Arc(a) if thick.is_none() && level(n) => lifted(a.center.z),
        EntityKind::Ellipse(el) if level(n) && el.major.z.abs() < 1e-12 => lifted(el.center.z),
        EntityKind::LwPolyline(pl) if thick.is_none() && level(n) => lifted(pl.elevation * n.z.signum()),
        EntityKind::Solid(s) | EntityKind::Trace(s) if thick.is_none() && s.corners.iter().all(|p| near(p.z, s.corners[0].z)) => {
            lifted(s.corners[0].z)
        }
        EntityKind::Text(t) if up(n) => lifted(t.insert.z),
        EntityKind::MText(t) if up(n) => lifted(t.insert.z),
        EntityKind::Hatch(h) if up(n) => lifted(h.elevation),
        EntityKind::Polyline3d(p) => match p.points.first() {
            Some(first) if p.points.iter().all(|q| near(q.z, first.z)) => lifted(first.z),
            _ => spatial(e),
        },
        EntityKind::Helix(h) if h.height == 0.0 => lifted(h.center.z),
        EntityKind::Point(_)
        | EntityKind::Circle(_)
        | EntityKind::Arc(_)
        | EntityKind::Ellipse(_)
        | EntityKind::LwPolyline(_)
        | EntityKind::Solid(_)
        | EntityKind::Trace(_)
        | EntityKind::Face3d(_)
        | EntityKind::PolyfaceMesh(_)
        | EntityKind::Helix(_) => spatial(e),
        _ => Placement::Flat,
    }
}

fn spatial(e: &Entity) -> Placement {
    match wire(e) {
        Some(w) => Placement::Spatial(w),
        None => Placement::Flat,
    }
}

/// The plane frame of an object (`origin` + its OCS axes): the arbitrary axis algorithm.
fn frame(n: Vec3) -> (Vec3, Vec3) {
    let m = Mat4::ocs(n);
    (Vec3::new(m.m[0][0], m.m[1][0], m.m[2][0]), Vec3::new(m.m[0][1], m.m[1][1], m.m[2][1]))
}

/// Points around an arc of `center`, `radius` from `start` to `end` (radians, counter-clockwise
/// about the plane's normal) in the plane with axes `x`, `y`.
fn arc_points(center: Vec3, radius: f64, start: f64, end: f64, x: Vec3, y: Vec3) -> Vec<Vec3> {
    let mut sweep = end - start;
    while sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    let sweep = sweep.min(std::f64::consts::TAU);
    let n = ((sweep / std::f64::consts::TAU * CIRCLE_SEGMENTS as f64).ceil() as usize).clamp(2, CIRCLE_SEGMENTS);
    (0..=n)
        .map(|i| {
            let a = start + sweep * i as f64 / n as f64;
            center + x * (radius * a.cos()) + y * (radius * a.sin())
        })
        .collect()
}

/// The points of a 2D polyline with bulges (in its plane's coordinates).
fn bulged(vertices: &[PolyVertex], closed: bool) -> Vec<Vec2> {
    let mut out = Vec::new();
    let count = vertices.len();
    for (i, v) in vertices.iter().enumerate() {
        if out.len() >= MAX_POINTS {
            break;
        }
        out.push(v.p);
        let next = match vertices.get(i + 1) {
            Some(n) => n,
            None if closed && count > 1 => match vertices.first() {
                Some(f) => f,
                None => break,
            },
            None => break,
        };
        if let Some((arc, ccw)) = draftcraft_geom::bulge_to_arc(v.p, next.p, v.bulge) {
            let mut sweep = arc.end - arc.start;
            while sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            let steps = ((sweep / std::f64::consts::TAU * CIRCLE_SEGMENTS as f64).ceil() as usize).max(2);
            for k in 1..steps {
                let f = k as f64 / steps as f64;
                let t = if ccw { arc.start + sweep * f } else { arc.end - sweep * f };
                out.push(Vec2::polar(arc.center, arc.radius, t));
            }
        }
    }
    if closed && let Some(f) = vertices.first().map(|v| v.p) {
        out.push(f);
    }
    out
}

/// A polyline swept along `dir` (the object's thickness): the path at both ends, a line at each
/// corner, and the side faces between.
fn extruded(path: &[Vec3], dir: Vec3, out: &mut Wire3d) {
    let top: Vec<Vec3> = path.iter().map(|p| *p + dir).collect();
    for w in path.windows(2) {
        if let [a, b] = w {
            out.faces.push([*a, *b, *b + dir]);
            out.faces.push([*a, *b + dir, *a + dir]);
        }
    }
    for p in path {
        out.lines.push(vec![*p, *p + dir]);
    }
    out.lines.push(path.to_vec());
    out.lines.push(top);
}

/// The object's own edges and faces, when it is drawn that way.
pub fn wire(e: &Entity) -> Option<Wire3d> {
    let c = &e.common;
    let n = normal(c);
    let (x, y) = frame(n);
    let mut out = Wire3d::default();
    // A path with the object's thickness along its normal, or the path alone.
    let path = |pts: Vec<Vec3>, out: &mut Wire3d| {
        if pts.len() > MAX_POINTS || !pts.iter().all(|p| finite(*p)) {
            return;
        }
        match thickness(c) {
            Some(t) => extruded(&pts, n * t, out),
            None => out.lines.push(pts),
        }
    };
    match &e.kind {
        EntityKind::Line(l) => path(vec![l.a, l.b], &mut out),
        EntityKind::Point(p) => path(vec![p.p, p.p], &mut out),
        EntityKind::Circle(ci) => path(arc_points(ci.center, ci.radius, 0.0, std::f64::consts::TAU, x, y), &mut out),
        EntityKind::Arc(a) => {
            // A mirrored OCS keeps its angles in plan (the reader turned them): about +Z.
            let (ax, ay) = if level(n) { (Vec3::X, Vec3::Y) } else { (x, y) };
            path(arc_points(a.center, a.radius, a.start, a.end, ax, ay), &mut out)
        }
        EntityKind::Ellipse(el) => {
            let minor = n.cross(el.major) * el.ratio;
            let (s, z) = (el.start, if el.end > el.start { el.end } else { el.end + std::f64::consts::TAU });
            let steps = CIRCLE_SEGMENTS;
            let pts = (0..=steps)
                .map(|i| {
                    let t = s + (z - s) * i as f64 / steps as f64;
                    el.center + el.major * t.cos() + minor * t.sin()
                })
                .collect();
            path(pts, &mut out)
        }
        EntityKind::LwPolyline(pl) => {
            let pts = bulged(&pl.vertices, pl.closed);
            let z = pl.elevation;
            let world: Vec<Vec3> = if level(n) {
                pts.iter().map(|p| Vec3::new(p.x, p.y, z * n.z.signum())).collect()
            } else {
                pts.iter().map(|p| x * p.x + y * p.y + n * z).collect()
            };
            path(world, &mut out)
        }
        EntityKind::Polyline3d(p) => {
            let mut pts: Vec<Vec3> = p.points.iter().take(MAX_POINTS).copied().collect();
            if p.closed
                && let Some(f) = pts.first().copied()
            {
                pts.push(f);
            }
            path(pts, &mut out)
        }
        EntityKind::Solid(s) | EntityKind::Trace(s) => {
            // A 2D solid's corners go 1, 2, 4, 3 round its outline.
            let [a, b, cc, d] = s.corners;
            let outline = if cc == d { vec![a, b, cc, a] } else { vec![a, b, d, cc, a] };
            if !outline.iter().all(|p| finite(*p)) {
                return None;
            }
            let fill = |dz: Vec3, out: &mut Wire3d| {
                out.faces.push([a + dz, b + dz, cc + dz]);
                if cc != d {
                    out.faces.push([b + dz, d + dz, cc + dz]);
                }
            };
            fill(Vec3::ZERO, &mut out);
            if let Some(t) = thickness(c) {
                fill(n * t, &mut out);
                extruded(&outline, n * t, &mut out);
            } else {
                out.lines.push(outline);
            }
        }
        EntityKind::Face3d(f) => {
            let [a, b, cc, d] = f.corners;
            if !f.corners.iter().all(|p| finite(*p)) {
                return None;
            }
            out.faces.push([a, b, cc]);
            if cc != d {
                out.faces.push([a, cc, d]);
            }
            for (p, q) in f.visible_edges() {
                out.lines.push(vec![p, q]);
            }
        }
        EntityKind::PolyfaceMesh(m) => {
            for f in m.faces3d().iter().take(MAX_POINTS) {
                let [a, b, cc, d] = f.corners;
                if !f.corners.iter().all(|p| finite(*p)) {
                    continue;
                }
                out.faces.push([a, b, cc]);
                if cc != d {
                    out.faces.push([a, cc, d]);
                }
                for (p, q) in f.visible_edges() {
                    out.lines.push(vec![p, q]);
                }
            }
        }
        EntityKind::Helix(h) => {
            let h = h.clone().sanitized();
            let steps = ((h.turns * CIRCLE_SEGMENTS as f64).ceil() as usize).clamp(8, MAX_POINTS);
            path((0..=steps).map(|i| h.point_at(i as f64 / steps as f64)).collect(), &mut out)
        }
        _ => return None,
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;
    use draftcraft_doc::{Arc as ArcE, Circle, Face3d, Handle, Line};

    fn ent(kind: EntityKind, common: Common) -> Entity {
        Entity { handle: Handle(1), common, kind }
    }

    #[test]
    fn plan_objects_at_a_height_are_lifted_and_others_drawn_in_space() {
        let line = |a: Vec3, b: Vec3| ent(EntityKind::Line(Line { a, b }), Common::default());
        assert_eq!(placement(&line(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))), Placement::Flat);
        assert_eq!(placement(&line(Vec3::new(0.0, 0.0, 5.0), Vec3::new(1.0, 0.0, 5.0))), Placement::Lifted(5.0));
        let Placement::Spatial(w) = placement(&line(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 50.0))) else { panic!("a vertical line") };
        assert_eq!(w.lines, vec![vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 50.0)]]);
    }

    #[test]
    fn thickness_extrudes_and_a_tilted_circle_stands_up() {
        let thick = Common { thickness: 30.0, ..Common::default() };
        let Placement::Spatial(w) = placement(&ent(EntityKind::Circle(Circle { center: Vec3::new(100.0, 0.0, 0.0), radius: 20.0 }), thick)) else {
            panic!("a circle with thickness")
        };
        assert!(w.faces.len() >= 2 * CIRCLE_SEGMENTS);
        let top = w.lines.iter().flatten().fold(f64::MIN, |m, p| m.max(p.z));
        assert!((top - 30.0).abs() < 1e-9);
        // A circle in a plane facing -Y (drawn in a UCS turned 90° about X).
        let tilted = Common { extrusion: Vec3::new(0.0, -1.0, 0.0), ..Common::default() };
        let Placement::Spatial(w) = placement(&ent(EntityKind::Circle(Circle { center: Vec3::ZERO, radius: 15.0 }), tilted)) else {
            panic!("a tilted circle")
        };
        let pts: Vec<&Vec3> = w.lines.iter().flatten().collect();
        assert!(pts.iter().all(|p| p.y.abs() < 1e-9 && ((p.x * p.x + p.z * p.z).sqrt() - 15.0).abs() < 1e-9));
        // An arc stays in plan at its height.
        let arc = ArcE { center: Vec3::new(0.0, 0.0, 7.0), radius: 3.0, start: 0.0, end: 1.0 };
        assert_eq!(placement(&ent(EntityKind::Arc(arc), Common::default())), Placement::Lifted(7.0));
    }

    #[test]
    fn a_3d_face_is_a_face_with_its_visible_edges() {
        let f = Face3d {
            corners: [Vec3::new(0.0, 0.0, 0.0), Vec3::new(50.0, 0.0, 0.0), Vec3::new(50.0, 50.0, 20.0), Vec3::new(0.0, 50.0, 20.0)],
            hidden_edges: 0b0100,
        };
        let Placement::Spatial(w) = placement(&ent(EntityKind::Face3d(f), Common::default())) else { panic!("a 3D face") };
        assert_eq!(w.faces.len(), 2);
        assert_eq!(w.lines.len(), 3);
    }

    #[test]
    fn hostile_numbers_give_nothing_to_draw_and_no_panic() {
        let bad = Common { thickness: f64::NAN, extrusion: Vec3::new(f64::NAN, 0.0, 0.0), ..Common::default() };
        for kind in [
            EntityKind::Line(Line { a: Vec3::new(f64::INFINITY, 0.0, 0.0), b: Vec3::new(0.0, 0.0, 1.0) }),
            EntityKind::Circle(Circle { center: Vec3::new(0.0, 0.0, 1.0), radius: f64::NAN }),
        ] {
            let _ = placement(&ent(kind, bad.clone()));
        }
    }
}
