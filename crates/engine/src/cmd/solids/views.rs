//! Flat views of solids as ordinary 2D objects: FLATSHOT (a hidden-line view, lines and arcs)
//! and SECTION (the cut through solids as regions).

use draftcraft_doc::{Arc, EntityKind, Handle, HatchLoop, Line, Region};
use draftcraft_geom::{PolyVertex, TAU, Vec2, Vec3, point_in_polygon};
use draftcraft_solid::{self as solid, Body, Frame, Seen, Seg};
use serde_json::{Value, json};

use super::super::machines::{SelOutcome, SelectPhase};
use super::super::{CommandSpec, bad, bool_or, point_param, point_value, str_param, targets};
use super::edit::world_body;
use super::{other, point3, solids};
use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("flatshot", "Flatshot", run_flatshot)
            .menu(&["Draw", "Modeling", "Flatshot"])
            .params("{handles? (all solids), view?: \"top\"|\"bottom\"|\"front\"|\"back\"|\"left\"|\"right\"|\"swiso\"|\"seiso\"|\"neiso\"|\"nwiso\", direction?: [x, y, z] (looking along it), up?: [x, y, z], base?: [x, y], showHidden?: bool} → {handles}")
            .interactive(|_| Ok(Box::new(FlatshotM::default()))),
        CommandSpec::new("section", "Section", run_section)
            .menu(&["Draw", "Modeling", "Section"])
            .alias(&["sec"])
            .params("{handles?, origin: [x, y, z], normal: [x, y, z] | points: [p1, p2] (vertical plane) | plane: \"xy\"|\"yz\"|\"zx\" with origin, base?: [x, y] (where a cut not parallel to XY is laid out)} → {handles}")
            .interactive(|_| Ok(Box::new(SectionM::default()))),
    ]
}

/// The named views: looking direction and up.
pub(crate) fn named_view(name: &str) -> Option<(Vec3, Vec3)> {
    let v = |x, y, z| Vec3::new(x, y, z);
    Some(match name.to_ascii_lowercase().as_str() {
        "top" => (v(0.0, 0.0, -1.0), v(0.0, 1.0, 0.0)),
        "bottom" => (v(0.0, 0.0, 1.0), v(0.0, 1.0, 0.0)),
        "front" => (v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0)),
        "back" => (v(0.0, -1.0, 0.0), v(0.0, 0.0, 1.0)),
        "left" => (v(1.0, 0.0, 0.0), v(0.0, 0.0, 1.0)),
        "right" => (v(-1.0, 0.0, 0.0), v(0.0, 0.0, 1.0)),
        "swiso" => (v(1.0, 1.0, -1.0), v(0.0, 0.0, 1.0)),
        "seiso" => (v(-1.0, 1.0, -1.0), v(0.0, 0.0, 1.0)),
        "neiso" => (v(-1.0, -1.0, -1.0), v(0.0, 0.0, 1.0)),
        "nwiso" => (v(1.0, -1.0, -1.0), v(0.0, 0.0, 1.0)),
        _ => return None,
    })
}

/// The handles to view: those given, or every solid in the current space.
fn all_solids(s: &Session, p: &Value) -> Result<Vec<Handle>> {
    if p.get("handles").is_some() {
        return targets(s, p);
    }
    let d = s.doc()?;
    Ok(d.space(&s.space()).map(|st| st.iter().filter(|e| matches!(e.kind, EntityKind::Solid3d(_))).map(|e| e.handle).collect()).unwrap_or_default())
}

/// The linetype for hidden lines: HIDDEN, loaded from the library if the drawing lacks it.
fn hidden_linetype(s: &mut Session) -> Result<String> {
    let d = s.doc_mut()?;
    if d.linetype("HIDDEN").is_none()
        && let Some(lt) = draftcraft_doc::library::standard_linetypes_for(d).into_iter().find(|l| l.name.eq_ignore_ascii_case("HIDDEN"))
    {
        d.linetypes.push(lt);
    }
    Ok(if d.linetype("HIDDEN").is_some() { "HIDDEN".into() } else { String::new() })
}

/// A 2D segment as a drawing object, moved by `at`.
fn seg_kind(seg: &Seg, at: Vec2) -> EntityKind {
    match *seg {
        Seg::Line { a, b } => EntityKind::Line(Line { a: (a + at).to3(0.0), b: (b + at).to3(0.0) }),
        Seg::Arc { center, radius, start, sweep } => {
            let (s0, s1) = if sweep >= 0.0 { (start, start + sweep) } else { (start + sweep, start) };
            if (s1 - s0) >= TAU - 1e-12 {
                EntityKind::Circle(draftcraft_doc::Circle { center: (center + at).to3(0.0), radius })
            } else {
                EntityKind::Arc(Arc { center: (center + at).to3(0.0), radius, start: s0.rem_euclid(TAU), end: s1.rem_euclid(TAU) })
            }
        }
    }
}

/// Most objects one FLATSHOT or SECTION adds.
const MAX_OUTPUT: usize = 1_000_000;

/// FLATSHOT: the whole view or, on an error (too many lines), nothing.
fn flatshot(s: &mut Session, hs: &[Handle], dir: Vec3, up: Vec3, base: Vec2, hidden: bool) -> Result<Value> {
    s.all_or_nothing(|s| flatshot_lines(s, hs, dir, up, base, hidden))
}

fn flatshot_lines(s: &mut Session, hs: &[Handle], dir: Vec3, up: Vec3, base: Vec2, hidden: bool) -> Result<Value> {
    let sols = solids(s, hs)?;
    if sols.is_empty() {
        return Err(other("There are no 3D solids to draw."));
    }
    // All bodies relative to the first one's position, so the hidden-line code works near the
    // origin; the view is then moved back.
    let mut bodies: Vec<Body> = Vec::with_capacity(sols.len());
    let mut origin = None;
    for (_, so) in &sols {
        let (b, o) = world_body(so)?;
        let o0 = *origin.get_or_insert(o);
        let shift = o - o0;
        bodies.push(if shift.len() > 0.0 { b.transformed(&super::translation(shift)).map_err(super::kernel)? } else { b });
    }
    let o0 = origin.unwrap_or(Vec3::ZERO);
    let refs: Vec<&Body> = bodies.iter().collect();
    let view = solid::project(&refs, dir, up).map_err(super::kernel)?;
    // Where the view's origin lands: the first solid's position in view coordinates, then `base`.
    let x = dir.cross(up).normalized();
    let y = x.cross(dir.normalized());
    let at = Vec2::new(o0.dot(x), o0.dot(y)) + base;
    let ltype = if hidden { hidden_linetype(s)? } else { String::new() };
    let like = sols.first().map(|(h, _)| *h);
    let mut made = Vec::new();
    let mut approx = 0usize;
    for piece in &view.pieces {
        let is_hidden = !matches!(piece.seen, Seen::Visible);
        if is_hidden && !hidden {
            continue;
        }
        if !piece.exact {
            approx += 1;
        }
        for seg in &piece.segs {
            if made.len() >= MAX_OUTPUT {
                return Err(other("The view has too many lines."));
            }
            let h = s.add_entity(seg_kind(seg, at))?;
            let layer = like.and_then(|l| s.doc().ok()?.entity(l).map(|e| e.common.layer.clone()));
            let lt = ltype.clone();
            s.doc_mut()?.modify_entity(h, |e| {
                if let Some(l) = layer {
                    e.common.layer = l;
                }
                if is_hidden {
                    e.common.linetype = lt;
                }
            })?;
            made.push(h);
        }
    }
    let mut notes = Vec::new();
    if approx > 0 {
        notes.push(format!("{approx} edges where solids were combined are drawn as short straight segments."));
    }
    if view.unsupported_faces > 0 {
        notes.push(format!("{} faces are of a kind the hidden-line view doesn't handle exactly yet.", view.unsupported_faces));
    }
    Ok(json!({ "handles": made.iter().map(|h| h.hex()).collect::<Vec<_>>(), "message": notes.join(" ") }))
}

fn run_flatshot(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = all_solids(s, p)?;
    let (mut dir, mut up) = match str_param(p, "view") {
        Some(v) => named_view(v).ok_or_else(|| bad("flatshot", format!("unknown view `{v}`")))?,
        None => (Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0)),
    };
    if let Some(d) = p.get("direction").and_then(point3) {
        dir = d;
        if p.get("up").is_none() {
            up = if d.normalized().z.abs() > 0.99 { Vec3::Y } else { Vec3::Z };
        }
    }
    if let Some(u) = p.get("up").and_then(point3) {
        up = u;
    }
    let base = point_param(p, "base").unwrap_or(Vec2::ZERO);
    flatshot(s, &hs, dir, up, base, bool_or(p, "showHidden", false))
}

#[derive(Default)]
struct FlatshotM {
    view: Option<String>,
}

impl Interactive for FlatshotM {
    fn name(&self) -> &'static str {
        "FLATSHOT"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.view.is_none() {
            Prompt::new("Choose the view", Accept::TEXT)
                .kw(&["Top", "Front", "Right", "Left", "Back", "Bottom", "SWiso", "SEiso", "NEiso", "NWiso"])
                .default("Top")
        } else {
            Prompt::new("Specify insertion point", Accept::POINT)
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (&self.view, i) {
            (_, Input::Cancel) => Ok(Step::Cancel),
            (None, Input::Keyword(k) | Input::Text(k)) if named_view(&k).is_some() => {
                self.view = Some(k);
                Ok(Step::Continue)
            }
            (None, Input::Enter) => {
                self.view = Some("top".into());
                Ok(Step::Continue)
            }
            (Some(v), Input::Point(p)) => {
                let v = v.clone();
                let hs = all_solids(s, &json!({}))?;
                let (dir, up) = named_view(&v).unwrap_or((Vec3::new(0.0, 0.0, -1.0), Vec3::Y));
                let r = flatshot(s, &hs, dir, up, p, false)?;
                if let Some(m) = r.get("message").and_then(Value::as_str).filter(|m| !m.is_empty()) {
                    s.echo(m.to_string());
                }
                Ok(Step::Done)
            }
            _ => Ok(Step::Continue),
        }
    }
}

// ---- SECTION -------------------------------------------------------------------------------

/// A chain of segments as a closed loop of bulged vertices, moved by `at`.
fn chain_vertices(chain: &[Seg], at: Vec2) -> Vec<PolyVertex> {
    let mut out = Vec::with_capacity(chain.len() + 1);
    let v = |p: Vec2, bulge: f64| PolyVertex { p: p + at, bulge, start_width: 0.0, end_width: 0.0 };
    for seg in chain {
        match *seg {
            Seg::Line { a, .. } => out.push(v(a, 0.0)),
            Seg::Arc { center, radius, start, sweep } => {
                // Bulges stand for arcs under a full turn: split longer ones.
                let parts = (sweep.abs() / (TAU / 3.0)).ceil().max(1.0) as usize;
                let step = sweep / parts as f64;
                for k in 0..parts {
                    let a0 = start + step * k as f64;
                    out.push(v(center + Vec2::new(a0.cos(), a0.sin()) * radius, (step / 4.0).tan()));
                }
            }
        }
    }
    out
}

/// The regions bounded by `chains`: each loop not inside another is an outer boundary; loops
/// inside one other are its holes (deeper nesting starts a new region).
fn regions_from(chains: &[Vec<Seg>], at: Vec2) -> Vec<Region> {
    let loops: Vec<Vec<PolyVertex>> = chains.iter().map(|c| chain_vertices(c, at)).filter(|l| l.len() >= 2).collect();
    let polys: Vec<Vec<Vec2>> = loops.iter().map(|l| l.iter().map(|v| v.p).collect()).collect();
    let depth = |i: usize| -> usize {
        let Some(p) = polys.get(i).and_then(|l| l.first()) else { return 0 };
        polys.iter().enumerate().filter(|(j, q)| *j != i && point_in_polygon(q, *p)).count()
    };
    let depths: Vec<usize> = (0..loops.len()).map(depth).collect();
    let mut out = Vec::new();
    for (i, l) in loops.iter().enumerate() {
        if depths.get(i).is_none_or(|d| d % 2 == 1) {
            continue;
        }
        let mut r = Region { loops: vec![HatchLoop { vertices: l.clone(), outer: true, sources: Vec::new() }] };
        let outer = polys.get(i).cloned().unwrap_or_default();
        for (j, h) in loops.iter().enumerate() {
            let inside = polys.get(j).and_then(|q| q.first()).is_some_and(|p| point_in_polygon(&outer, *p));
            if j != i && inside && depths.get(j) == depths.get(i).map(|d| d + 1).as_ref() {
                r.loops.push(HatchLoop { vertices: h.clone(), outer: false, sources: Vec::new() });
            }
        }
        out.push(r);
    }
    out
}

fn section(s: &mut Session, hs: &[Handle], origin: Vec3, normal: Vec3, base: Option<Vec2>) -> Result<Value> {
    let sols = solids(s, hs)?;
    if sols.is_empty() {
        return Err(other("Select 3D solids to section."));
    }
    let n = normal.normalized();
    if !(n.is_finite() && n.len() > 0.5) {
        return Err(other("The section plane isn't valid."));
    }
    // A cut parallel to XY lies in the drawing where it is; any other is laid out flat, its own
    // X axis along the drawing's, at `base` (default: right of the solids).
    let flat = n.z.abs() > 1.0 - 1e-12;
    let mut made = Vec::new();
    let mut default_base = None;
    for (h, so) in &sols {
        let (body, o) = world_body(so)?;
        let frame = if flat { Frame::xy(origin.z - o.z) } else { Frame::from_normal(origin - o, n).map_err(super::kernel)? };
        let chains = solid::section(&body, &frame).map_err(super::kernel)?;
        let at = if flat {
            o.xy()
        } else {
            match base.or(default_base) {
                Some(b) => b,
                None => {
                    let d = s.doc()?;
                    let ext = d.extents(&s.space());
                    let b = if ext.is_empty() { Vec2::ZERO } else { Vec2::new(ext.max.x + ext.width() * 0.25, ext.min.y) };
                    default_base = Some(b);
                    b
                }
            }
        };
        for r in regions_from(&chains, at) {
            if made.len() >= MAX_OUTPUT {
                return Err(other("The section has too many regions."));
            }
            made.push(super::add_like(s, *h, EntityKind::Region(r))?);
        }
    }
    if made.is_empty() {
        return Err(other("The plane doesn't cut the selected solids."));
    }
    Ok(json!({ "handles": made.iter().map(|h| h.hex()).collect::<Vec<_>>() }))
}

fn run_section(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let (origin, normal) = if let Some(pts) = p.get("points").and_then(Value::as_array) {
        let a = pts.first().and_then(point_value).ok_or_else(|| bad("section", "`points` needs two points"))?;
        let b = pts.get(1).and_then(point_value).ok_or_else(|| bad("section", "`points` needs two points"))?;
        let d = b - a;
        if d.len() <= 1e-12 {
            return Err(bad("section", "the two points must differ"));
        }
        (a.to3(0.0), Vec3::new(-d.y, d.x, 0.0))
    } else {
        let origin = p.get("origin").and_then(point3).unwrap_or(Vec3::ZERO);
        let normal = match str_param(p, "plane") {
            Some(pl) => axis_normal(pl).ok_or_else(|| bad("section", format!("unknown plane `{pl}`")))?,
            None => super::point3_req("section", p, "normal")?,
        };
        (origin, normal)
    };
    section(s, &hs, origin, normal, point_param(p, "base"))
}

fn axis_normal(name: &str) -> Option<Vec3> {
    Some(match name.to_ascii_lowercase().as_str() {
        "xy" => Vec3::Z,
        "yz" => Vec3::X,
        "zx" => Vec3::Y,
        _ => return None,
    })
}

#[derive(Default)]
struct SectionM {
    sel: SelectPhase,
    picked: Option<Vec<Handle>>,
    axis: Option<Vec3>,
    first: Option<Vec2>,
    plane: Option<(Vec3, Vec3)>,
}

impl Interactive for SectionM {
    fn name(&self) -> &'static str {
        "SECTION"
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
        match (self.plane, self.axis, self.first) {
            (Some(_), ..) => Prompt::new("Specify base point for the section", Accept::POINT),
            (None, Some(_), _) => Prompt::new("Specify a point on the plane", Accept::POINT).default("0,0,0"),
            (None, None, Some(a)) => Prompt::new("Specify second point on plane", Accept::POINT).base(a),
            (None, None, None) => Prompt::new("Specify first point on Section plane", Accept::POINT).kw(&["XY", "YZ", "ZX"]),
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
        let hs = self.picked.clone().unwrap_or_default();
        if let Some((o, n)) = self.plane {
            let Input::Point(b) = i else { return Ok(Step::Continue) };
            section(s, &hs, o, n, Some(b))?;
            s.set_selection(Vec::new());
            return Ok(Step::Done);
        }
        let plane = if let Some(n) = self.axis {
            let o = match i {
                // A typed `x,y,z` puts the plane at that height.
                Input::Point(p) => p.to3(s.point_z.unwrap_or(0.0)),
                Input::Enter => Vec3::ZERO,
                _ => return Ok(Step::Continue),
            };
            (o, n)
        } else {
            match (self.first, i) {
                (None, Input::Keyword(k)) => {
                    self.axis = axis_normal(&k);
                    return Ok(Step::Continue);
                }
                (None, Input::Point(p)) => {
                    self.first = Some(p);
                    return Ok(Step::Continue);
                }
                (Some(a), Input::Point(b)) if !a.near(b, 1e-12) => {
                    let d = b - a;
                    (a.to3(0.0), Vec3::new(-d.y, d.x, 0.0))
                }
                _ => return Ok(Step::Continue),
            }
        };
        // A cut parallel to XY needs no base point.
        if plane.1.normalized().z.abs() > 1.0 - 1e-12 {
            section(s, &hs, plane.0, plane.1, None)?;
            s.set_selection(Vec::new());
            return Ok(Step::Done);
        }
        self.plane = Some(plane);
        Ok(Step::Continue)
    }
}
