//! Solid primitives: BOX, WEDGE, CYLINDER, CONE, SPHERE and TORUS.

use draftcraft_geom::{Vec2, Vec3};
use draftcraft_solid::{self as solid, Frame, Profile, Seg};
use serde_json::Value;

use super::super::machines::number;
use super::super::{CommandSpec, bad, f64_req, helpers};
use super::{added, other, placed, point3_req};
use crate::{Accept, EntityKind, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("box", "Box", run_box)
            .menu(&["Draw", "Modeling", "Box"])
            .params("{corner1: [x, y, z?], corner2: [x, y, z?], height? (from corner1's Z; default corner2's Z)} → {handle}")
            .interactive(|_| Ok(Box::new(PrimM::new(Kind::Box)))),
        CommandSpec::new("wedge", "Wedge", run_wedge)
            .menu(&["Draw", "Modeling", "Wedge"])
            .alias(&["we"])
            .params("{corner1: [x, y, z?], corner2: [x, y], height} → {handle}: the slope falls from corner1's side to corner2's along X")
            .interactive(|_| Ok(Box::new(PrimM::new(Kind::Wedge)))),
        CommandSpec::new("cylinder", "Cylinder", run_cylinder)
            .menu(&["Draw", "Modeling", "Cylinder"])
            .alias(&["cyl"])
            .params("{center: [x, y, z?], radius, height} → {handle}")
            .interactive(|_| Ok(Box::new(PrimM::new(Kind::Cylinder)))),
        CommandSpec::new("cone", "Cone", run_cone)
            .menu(&["Draw", "Modeling", "Cone"])
            .params("{center: [x, y, z?], radius, topRadius? (0: a pointed cone), height} → {handle}")
            .interactive(|_| Ok(Box::new(PrimM::new(Kind::Cone)))),
        CommandSpec::new("sphere", "Sphere", run_sphere)
            .menu(&["Draw", "Modeling", "Sphere"])
            .params("{center: [x, y, z?], radius} → {handle}")
            .interactive(|_| Ok(Box::new(PrimM::new(Kind::Sphere)))),
        CommandSpec::new("torus", "Torus", run_torus)
            .menu(&["Draw", "Modeling", "Torus"])
            .alias(&["tor"])
            .params("{center: [x, y, z?], radius, tubeRadius} → {handle}")
            .interactive(|_| Ok(Box::new(PrimM::new(Kind::Torus)))),
    ]
}

fn box_kind(a: Vec3, b: Vec3) -> Result<EntityKind> {
    placed(a, |o| solid::box_solid(a - o, b - o))
}

/// A wedge on the rectangle from `a` to `b` (in XY at `a`'s Z), `height` tall at `a`'s X side and
/// falling to nothing at `b`'s, as AutoCAD's WEDGE.
fn wedge_kind(a: Vec3, b: Vec2, height: f64) -> Result<EntityKind> {
    if !(height.is_finite() && height.abs() > 1e-9) {
        return Err(other("The height must not be 0."));
    }
    let (w, l) = (b.x - a.x, b.y - a.y);
    if w.abs() <= 1e-9 || l.abs() <= 1e-9 {
        return Err(other("The wedge needs a length and a width."));
    }
    placed(a, |_| {
        // The triangle in the XZ plane at Y 0, extruded along Y by the width.
        let frame = Frame { origin: Vec3::ZERO, x: Vec3::new(1.0, 0.0, 0.0), y: Vec3::new(0.0, 0.0, 1.0) };
        let (p0, p1, p2) = (Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(0.0, height));
        let tri = Profile { outer: vec![Seg::Line { a: p0, b: p1 }, Seg::Line { a: p1, b: p2 }, Seg::Line { a: p2, b: p0 }], holes: Vec::new() };
        // The plane's normal is X × Z = -Y: a positive width goes the other way.
        let mut bodies = solid::extrude(&frame, &tri, -l, 0.0)?;
        bodies.pop().ok_or_else(|| solid::SolidError("the wedge made no solid".into()))
    })
}

fn cylinder_kind(c: Vec3, r: f64, h: f64) -> Result<EntityKind> {
    placed(c, |_| {
        let (base, axis) = if h < 0.0 { (Vec3::new(0.0, 0.0, h), Vec3::Z) } else { (Vec3::ZERO, Vec3::Z) };
        solid::cylinder(base, axis, r, h.abs())
    })
}

fn cone_kind(c: Vec3, r: f64, top: f64, h: f64) -> Result<EntityKind> {
    placed(c, |_| {
        let axis = if h < 0.0 { Vec3::new(0.0, 0.0, -1.0) } else { Vec3::Z };
        solid::cone(Vec3::ZERO, axis, r, top, h.abs())
    })
}

fn sphere_kind(c: Vec3, r: f64) -> Result<EntityKind> {
    placed(c, |_| solid::sphere(Vec3::ZERO, r))
}

fn torus_kind(c: Vec3, r: f64, tube: f64) -> Result<EntityKind> {
    placed(c, |_| solid::torus(Vec3::ZERO, r, tube))
}

fn run_box(s: &mut Session, p: &Value) -> Result<Value> {
    let a = point3_req("box", p, "corner1")?;
    let mut b = point3_req("box", p, "corner2")?;
    if let Some(h) = p.get("height") {
        let h = h.as_f64().filter(|h| h.is_finite()).ok_or_else(|| bad("box", "`height` must be a number"))?;
        b.z = a.z + h;
    }
    added(s.add_entity(box_kind(a, b)?)?)
}

fn run_wedge(s: &mut Session, p: &Value) -> Result<Value> {
    let a = point3_req("wedge", p, "corner1")?;
    let b = point3_req("wedge", p, "corner2")?;
    added(s.add_entity(wedge_kind(a, b.xy(), f64_req("wedge", p, "height")?)?)?)
}

fn run_cylinder(s: &mut Session, p: &Value) -> Result<Value> {
    let c = point3_req("cylinder", p, "center")?;
    added(s.add_entity(cylinder_kind(c, f64_req("cylinder", p, "radius")?, f64_req("cylinder", p, "height")?)?)?)
}

fn run_cone(s: &mut Session, p: &Value) -> Result<Value> {
    let c = point3_req("cone", p, "center")?;
    let top = super::super::f64_or(p, "topRadius", 0.0);
    added(s.add_entity(cone_kind(c, f64_req("cone", p, "radius")?, top, f64_req("cone", p, "height")?)?)?)
}

fn run_sphere(s: &mut Session, p: &Value) -> Result<Value> {
    let c = point3_req("sphere", p, "center")?;
    added(s.add_entity(sphere_kind(c, f64_req("sphere", p, "radius")?)?)?)
}

fn run_torus(s: &mut Session, p: &Value) -> Result<Value> {
    let c = point3_req("torus", p, "center")?;
    added(s.add_entity(torus_kind(c, f64_req("torus", p, "radius")?, f64_req("torus", p, "tubeRadius")?)?)?)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Box,
    Wedge,
    Cylinder,
    Cone,
    Sphere,
    Torus,
}

/// The prompt sequence of a primitive: a first point, then a size (a point or a number), then
/// a height or a second size.
struct PrimM {
    kind: Kind,
    first: Option<Vec2>,
    /// The second corner (BOX, WEDGE) or the radius (the others).
    second: Option<(Vec2, f64)>,
    /// The cone's top radius, when given.
    top: Option<f64>,
    asking_top: bool,
    diameter: bool,
}

impl PrimM {
    fn new(kind: Kind) -> Self {
        PrimM { kind, first: None, second: None, top: None, asking_top: false, diameter: false }
    }
    fn corners(&self) -> bool {
        matches!(self.kind, Kind::Box | Kind::Wedge)
    }
    fn finish(&self, s: &mut Session, third: f64) -> Result<Step> {
        let (Some(a), Some((b, r))) = (self.first, self.second) else { return Ok(Step::Cancel) };
        let a3 = a.to3(0.0);
        let kind = match self.kind {
            Kind::Box => box_kind(a3, b.to3(third))?,
            Kind::Wedge => wedge_kind(a3, b, third)?,
            Kind::Cylinder => cylinder_kind(a3, r, third)?,
            Kind::Cone => cone_kind(a3, r, self.top.unwrap_or(0.0), third)?,
            Kind::Torus => torus_kind(a3, r, third)?,
            Kind::Sphere => return Ok(Step::Cancel),
        };
        s.add_entity(kind)?;
        Ok(Step::Done)
    }
}

impl Interactive for PrimM {
    fn name(&self) -> &'static str {
        match self.kind {
            Kind::Box => "BOX",
            Kind::Wedge => "WEDGE",
            Kind::Cylinder => "CYLINDER",
            Kind::Cone => "CONE",
            Kind::Sphere => "SPHERE",
            Kind::Torus => "TORUS",
        }
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.asking_top {
            return Prompt::new("Specify top radius", Accept::POINT_OR_NUMBER).base_opt(self.first).distance();
        }
        let radius = |what: &str| -> Prompt {
            let msg = if self.diameter { format!("Specify {what}diameter") } else { format!("Specify {what}radius") };
            let p = Prompt::new(msg, Accept::POINT_OR_NUMBER).base_opt(self.first).distance();
            if self.diameter { p } else { p.kw(&["Diameter"]) }
        };
        match (self.first, self.second) {
            (None, _) => match self.kind {
                Kind::Box | Kind::Wedge => Prompt::new("Specify first corner", Accept::POINT),
                Kind::Cylinder | Kind::Cone => Prompt::new("Specify center point of base", Accept::POINT),
                Kind::Sphere | Kind::Torus => Prompt::new("Specify center point", Accept::POINT),
            },
            (Some(a), None) => match self.kind {
                Kind::Box | Kind::Wedge => Prompt::new("Specify other corner", Accept::POINT).corner(a),
                Kind::Cylinder | Kind::Cone => radius("base "),
                Kind::Sphere | Kind::Torus => radius(""),
            },
            (Some(_), Some((b, _))) => match self.kind {
                Kind::Torus => {
                    let p = Prompt::new(if self.diameter { "Specify tube diameter" } else { "Specify tube radius" }, Accept::POINT_OR_NUMBER);
                    if self.diameter { p } else { p.kw(&["Diameter"]) }
                }
                Kind::Cone => Prompt::new("Specify height", Accept::POINT_OR_NUMBER).kw(&["Top radius"]).base_opt(self.first).distance(),
                _ => Prompt::new("Specify height", Accept::POINT_OR_NUMBER).base(if self.corners() { b } else { self.first.unwrap_or(b) }).distance(),
            },
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let value = |i: &Input, from: Option<Vec2>| -> Option<f64> {
            match i {
                Input::Text(t) => number(t),
                Input::Point(p) => from.map(|f| f.dist(*p)),
                _ => None,
            }
        };
        if let Input::Keyword(k) = &i {
            match k.as_str() {
                "Diameter" => self.diameter = true,
                "Top radius" => self.asking_top = true,
                _ => {}
            }
            return Ok(Step::Continue);
        }
        if matches!(i, Input::Enter | Input::Cancel) {
            return Ok(Step::Cancel);
        }
        if self.asking_top {
            self.top = value(&i, self.first).filter(|v| *v >= 0.0);
            self.asking_top = false;
            return Ok(Step::Continue);
        }
        let half = |v: f64, d: bool| if d { v / 2.0 } else { v };
        match (self.first, self.second) {
            (None, _) => {
                if let Input::Point(p) = i {
                    self.first = Some(p);
                }
                Ok(Step::Continue)
            }
            (Some(a), None) => {
                if self.corners() {
                    if let Input::Point(p) = i {
                        self.second = Some((p, 0.0));
                    }
                    return Ok(Step::Continue);
                }
                let Some(v) = value(&i, Some(a)).map(|v| half(v, self.diameter)) else { return Err(other("Requires a positive distance.")) };
                if !(v.is_finite() && v > 0.0) {
                    return Err(other("Requires a positive distance."));
                }
                self.diameter = false;
                self.second = Some((a, v));
                if self.kind == Kind::Sphere {
                    s.add_entity(sphere_kind(a.to3(0.0), v)?)?;
                    return Ok(Step::Done);
                }
                Ok(Step::Continue)
            }
            (Some(_), Some((b, _))) => {
                let from = if self.corners() { Some(b) } else { self.first };
                let Some(v) = value(&i, from).map(|v| if self.kind == Kind::Torus { half(v, self.diameter) } else { v }) else {
                    return Err(other("Requires a distance."));
                };
                self.finish(s, v)
            }
        }
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<EntityKind> {
        match (self.first, self.second) {
            (Some(a), None) if self.corners() => vec![helpers::lwpoly(helpers::rect_vertices(a, c), true)],
            (Some(a), None) => vec![helpers::circle(a, if self.diameter { a.dist(c) / 2.0 } else { a.dist(c) })],
            (Some(a), Some((b, _))) if self.corners() => vec![helpers::lwpoly(helpers::rect_vertices(a, b), true), helpers::line(b, c)],
            (Some(a), Some((_, r))) => vec![helpers::circle(a, r), helpers::line(a, c)],
            _ => Vec::new(),
        }
    }
}
