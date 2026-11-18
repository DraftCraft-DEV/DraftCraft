//! Objects moved, turned, scaled and mirrored in space (3DMOVE, 3DROTATE, 3DSCALE, MIRROR3D,
//! ROTATE3D, 3DALIGN, 3DARRAY), as AutoCAD does it: a transform that keeps the XY plane level is a
//! plan transform with the heights mapped; one that tilts it takes each object out of its plane,
//! so a circle, an arc, an ellipse or a polyline gets the plane its points go to (its extrusion
//! direction) and its angles in that plane.

use draftcraft_geom::{Mat3, Mat4, Vec2, Vec3};

use super::{Common, EntityKind};

/// Why an object can't take a transform: kinds that can't leave the XY plane yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotIn3d(pub &'static str);

/// The plan part of a transform that keeps XY level: its XY block and shift.
fn plan(m: &Mat4) -> Mat3 {
    Mat3 { a: m.m[0][0], b: m.m[1][0], c: m.m[0][1], d: m.m[1][1], e: m.m[0][3], f: m.m[1][3] }
}

fn level(n: Vec3) -> bool {
    n.x.abs() < 1e-12 && n.y.abs() < 1e-12 && n.z != 0.0
}

/// The OCS axes of a plane's normal (X, Y), with the reader's rule for a mirrored plan OCS
/// (normal −Z): its points and angles are kept as in plan.
fn axes(n: Vec3) -> (Vec3, Vec3) {
    if level(n) {
        return (Vec3::X, Vec3::Y);
    }
    let m = Mat4::ocs(n);
    (Vec3::new(m.m[0][0], m.m[1][0], m.m[2][0]), Vec3::new(m.m[0][1], m.m[1][1], m.m[2][1]))
}

fn unit_normal(c: &Common) -> Vec3 {
    let n = c.extrusion.normalized();
    if n.is_finite() && n != Vec3::ZERO { n } else { Vec3::Z }
}

/// A new plane normal: kept as +Z when it is level (the doc's plan convention: points and
/// angles in plan), and whether that turned it over (then circular directions run the other
/// way and the thickness points down).
fn settle(n: Vec3) -> (Vec3, bool) {
    if level(n) { (Vec3::Z, n.z < 0.0) } else { (n, false) }
}

/// Angle of `p` about `center` in the plane with axes `x`, `y`.
fn angle_in(p: Vec3, center: Vec3, x: Vec3, y: Vec3) -> f64 {
    let d = p - center;
    d.dot(y).atan2(d.dot(x))
}

/// The counter-clockwise sweep from `a` to `b`, in (0, 2π].
fn ccw(a: f64, b: f64) -> f64 {
    let mut s = b - a;
    while s <= 0.0 {
        s += std::f64::consts::TAU;
    }
    s.min(std::f64::consts::TAU)
}

impl EntityKind {
    /// Apply the 3D affine transform `m` (rotations, moves, uniform scales, mirrors) to the object
    /// with properties `common`. A transform keeping the XY plane level works on every kind; one
    /// tilting it is refused for kinds that can't leave the XY plane yet (text, dimensions,
    /// hatches, block references…), with the object unchanged.
    pub fn transform3(&mut self, common: &mut Common, m: &Mat4) -> Result<(), NotIn3d> {
        if !m.is_affine() {
            return Err(NotIn3d("the transform is not finite"));
        }
        let scale = m.det3().abs().cbrt();
        if !(scale.is_finite() && scale > 1e-12) {
            return Err(NotIn3d("the transform flattens the object"));
        }
        if let EntityKind::Solid3d(so) = self {
            so.transform3(m);
            return Ok(());
        }
        if m.keeps_xy() {
            let p = plan(m);
            self.transform(&p);
            self.map_heights(m.m[2][2], m.m[2][3]);
            common.thickness *= m.m[2][2];
            return Ok(());
        }
        let n = unit_normal(common);
        let mirror = m.det3() < 0.0;
        let pt = |p: &mut Vec3| *p = m.apply(*p);
        match self {
            EntityKind::Line(l) => {
                pt(&mut l.a);
                pt(&mut l.b);
            }
            EntityKind::Point(p) => pt(&mut p.p),
            EntityKind::Polyline3d(p) => p.points.iter_mut().for_each(pt),
            EntityKind::Face3d(f) => f.corners.iter_mut().for_each(pt),
            EntityKind::PolyfaceMesh(pm) => pm.vertices.iter_mut().for_each(pt),
            EntityKind::Leader(l) => l.vertices.iter_mut().for_each(pt),
            EntityKind::Ray(r) | EntityKind::XLine(r) => {
                pt(&mut r.base);
                r.dir = m.apply_vec(r.dir).normalized();
            }
            EntityKind::Solid(s) | EntityKind::Trace(s) => s.corners.iter_mut().for_each(pt),
            EntityKind::Circle(c) => {
                pt(&mut c.center);
                c.radius *= scale;
            }
            EntityKind::Arc(a) => {
                let (x, y) = axes(n);
                let at = |t: f64| a.center + x * (a.radius * t.cos()) + y * (a.radius * t.sin());
                let mid = a.start + ccw(a.start, a.end) / 2.0;
                let (s, md, e) = (m.apply(at(a.start)), m.apply(at(mid)), m.apply(at(a.end)));
                let center = m.apply(a.center);
                let (nn, _) = settle(m.apply_normal(n));
                let (x2, y2) = axes(nn);
                let (ts, tm, te) = (angle_in(s, center, x2, y2), angle_in(md, center, x2, y2), angle_in(e, center, x2, y2));
                // The arc runs counter-clockwise from start to end through its middle.
                let (start, end) = if ccw(ts, tm) < ccw(ts, te) { (ts, te) } else { (te, ts) };
                a.center = center;
                a.radius *= scale;
                a.start = draftcraft_geom::norm_angle(start);
                a.end = draftcraft_geom::norm_angle(end);
            }
            EntityKind::Ellipse(el) => {
                let minor = n.cross(el.major) * el.ratio;
                let at = |t: f64| el.center + el.major * t.cos() + minor * t.sin();
                let full = (ccw(el.start, el.end) - std::f64::consts::TAU).abs() < 1e-9;
                let mid = el.start + ccw(el.start, el.end) / 2.0;
                let (s, md, e) = (m.apply(at(el.start)), m.apply(at(mid)), m.apply(at(el.end)));
                el.center = m.apply(el.center);
                el.major = m.apply_vec(el.major);
                let nn = m.apply_normal(n);
                if !full {
                    // Parameters on the moved ellipse: from its axes.
                    let mj = el.major;
                    let mn = nn.cross(mj) * el.ratio;
                    let par = |p: Vec3| {
                        let d = p - el.center;
                        let (u, v) = (d.dot(mj) / mj.dot(mj).max(1e-300), d.dot(mn) / mn.dot(mn).max(1e-300));
                        v.atan2(u)
                    };
                    let (ts, tm, te) = (par(s), par(md), par(e));
                    let (start, end) = if ccw(ts, tm) < ccw(ts, te) { (ts, te) } else { (te, ts) };
                    el.start = draftcraft_geom::norm_angle(start);
                    el.end = draftcraft_geom::norm_angle(end);
                    if (el.end - el.start).abs() < 1e-12 {
                        el.end = el.start + std::f64::consts::TAU;
                    }
                }
                common.extrusion = nn;
                return Ok(());
            }
            EntityKind::LwPolyline(pl) => {
                let (x, y) = axes(n);
                let lift = if level(n) { pl.elevation * n.z.signum() } else { pl.elevation };
                let up = if level(n) { Vec3::Z } else { n };
                let world: Vec<Vec3> = pl.vertices.iter().map(|v| m.apply(x * v.p.x + y * v.p.y + up * lift)).collect();
                let (nn, turned) = settle(m.apply_normal(n));
                let (x2, y2) = axes(nn);
                for (v, w) in pl.vertices.iter_mut().zip(&world) {
                    v.p = Vec2::new(w.dot(x2), w.dot(y2));
                    v.start_width *= scale;
                    v.end_width *= scale;
                    if mirror != turned {
                        v.bulge = -v.bulge;
                    }
                }
                pl.const_width *= scale;
                pl.elevation = world.first().map_or(0.0, |w| w.dot(nn));
                common.extrusion = nn;
                if turned {
                    common.thickness = -common.thickness;
                }
                common.thickness *= scale;
                return Ok(());
            }
            EntityKind::Text(_)
            | EntityKind::MText(_)
            | EntityKind::AttDef(_)
            | EntityKind::Insert(_)
            | EntityKind::Dimension(_)
            | EntityKind::MLine(_)
            | EntityKind::MLeader(_)
            | EntityKind::Hatch(_)
            | EntityKind::Spline(_)
            | EntityKind::Helix(_)
            | EntityKind::Region(_)
            | EntityKind::Viewport(_)
            | EntityKind::Image(_)
            | EntityKind::Wipeout(_)
            | EntityKind::Table(_)
            | EntityKind::Tolerance(_)
            | EntityKind::Unknown(_) => return Err(NotIn3d("it can't be turned out of the XY plane yet")),
            EntityKind::Solid3d(_) => {}
        }
        // The plane of a planar object, its thickness along the new normal.
        let (nn, turned) = settle(m.apply_normal(n));
        common.extrusion = nn;
        common.thickness *= if turned { -scale } else { scale };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::unreachable)]
    use super::*;
    use crate::{Arc, Circle, Line};

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).len() < 1e-9
    }

    #[test]
    fn a_line_turns_about_x_and_a_circle_stands_up() {
        let r = Mat4::rotation_about(Vec3::ZERO, Vec3::X, std::f64::consts::FRAC_PI_2).unwrap();
        let mut c = Common::default();
        let mut l = EntityKind::Line(Line { a: Vec3::new(0.0, 1.0, 0.0), b: Vec3::new(2.0, 1.0, 0.0) });
        l.transform3(&mut c, &r).unwrap();
        let EntityKind::Line(l) = l else { unreachable!() };
        assert!(close(l.a, Vec3::new(0.0, 0.0, 1.0)) && close(l.b, Vec3::new(2.0, 0.0, 1.0)));
        let mut c = Common { thickness: 3.0, ..Common::default() };
        let mut ci = EntityKind::Circle(Circle { center: Vec3::new(0.0, 5.0, 0.0), radius: 2.0 });
        ci.transform3(&mut c, &r).unwrap();
        assert!(close(c.extrusion, Vec3::new(0.0, -1.0, 0.0)), "{:?}", c.extrusion);
        assert!((c.thickness - 3.0).abs() < 1e-12);
    }

    #[test]
    fn an_arc_keeps_its_ends_where_the_transform_puts_them() {
        let r = Mat4::rotation_about(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0), 1.0).unwrap();
        let mut c = Common::default();
        let a0 = Arc { center: Vec3::new(3.0, 1.0, 2.0), radius: 2.0, start: 0.3, end: 2.0 };
        let mut a = EntityKind::Arc(a0.clone());
        a.transform3(&mut c, &r).unwrap();
        let EntityKind::Arc(a) = a else { unreachable!() };
        let m = Mat4::ocs(c.extrusion);
        let x = Vec3::new(m.m[0][0], m.m[1][0], m.m[2][0]);
        let y = Vec3::new(m.m[0][1], m.m[1][1], m.m[2][1]);
        let p = |t: f64| a.center + x * (a.radius * t.cos()) + y * (a.radius * t.sin());
        let want = |t: f64| r.apply(a0.center + Vec3::X * (a0.radius * t.cos()) + Vec3::Y * (a0.radius * t.sin()));
        assert!(close(p(a.start), want(a0.start)) && close(p(a.end), want(a0.end)));
    }

    #[test]
    fn a_level_transform_maps_heights_and_text_cant_tilt() {
        let lift = Mat4::translation(Vec3::new(1.0, 2.0, 3.0));
        let mut c = Common::default();
        let mut l = EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::X });
        l.transform3(&mut c, &lift).unwrap();
        let EntityKind::Line(l) = l else { unreachable!() };
        assert!(close(l.a, Vec3::new(1.0, 2.0, 3.0)));
        let tilt = Mat4::rotation_about(Vec3::ZERO, Vec3::X, 0.5).unwrap();
        let mut t = EntityKind::Region(crate::Region::default());
        assert!(t.transform3(&mut Common::default(), &tilt).is_err());
        let bad = Mat4 { m: [[f64::NAN; 4]; 4] };
        assert!(EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::X }).transform3(&mut Common::default(), &bad).is_err());
    }
}
