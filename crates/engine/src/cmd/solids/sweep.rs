//! Solids from 2D profiles: EXTRUDE and REVOLVE. Closed polylines, circles and regions in the
//! XY plane (at their elevation) are the profiles; the profiles are erased, as AutoCAD does with
//! DELOBJ at its default.

use draftcraft_doc::{EntityKind, Handle, HatchLoop};
use draftcraft_geom::{PolyVertex, TAU, Vec2, Vec3, bulge_to_arc, point_in_polygon};
use draftcraft_solid::{self as solid, Frame, Profile, Seg};
use serde_json::{Value, json};

use super::super::machines::{SelOutcome, SelectPhase, number};
use super::super::{CommandSpec, bad, bool_or, f64_or, f64_req, point_param, targets};
use super::{add_like, other, solid_kind, translation};
use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("extrude", "Extrude", run_extrude)
            .menu(&["Draw", "Modeling", "Extrude"])
            .alias(&["ext"])
            .params("{handles?, height, taper? (degrees, leaning in), keepProfiles?: bool} → {handles} (closed polylines, circles and regions)")
            .interactive(|_| Ok(Box::new(SweepM::new(Mode::Extrude)))),
        CommandSpec::new("revolve", "Revolve", run_revolve)
            .menu(&["Draw", "Modeling", "Revolve"])
            .alias(&["rev"])
            .params("{handles?, axisStart: [x, y], axisEnd: [x, y], angle? (degrees, default 360), keepProfiles?: bool} → {handles}")
            .interactive(|_| Ok(Box::new(SweepM::new(Mode::Revolve)))),
    ]
}

/// Most profile vertices read from one object.
const MAX_VERTICES: usize = 100_000;

/// A closed boundary of bulged vertices as segments, relative to `o`.
fn loop_segs(vs: &[PolyVertex], o: Vec2) -> Result<Vec<Seg>> {
    if vs.len() < 2 || vs.len() > MAX_VERTICES {
        return Err(other("A profile needs 2 to 100000 vertices."));
    }
    let mut out = Vec::with_capacity(vs.len());
    for (i, v) in vs.iter().enumerate() {
        let next = vs.get((i + 1) % vs.len()).ok_or_else(|| other("profile vertex"))?;
        let (a, b) = (v.p - o, next.p - o);
        if a.dist(b) <= 1e-12 {
            continue;
        }
        match bulge_to_arc(a, b, v.bulge) {
            Some((arc, ccw)) => {
                let sweep = draftcraft_geom::ccw_sweep(arc.start, arc.end);
                out.push(if ccw {
                    Seg::Arc { center: arc.center, radius: arc.radius, start: arc.start, sweep }
                } else {
                    Seg::Arc { center: arc.center, radius: arc.radius, start: arc.end, sweep: -sweep }
                });
            }
            None => out.push(Seg::Line { a, b }),
        }
    }
    if out.len() < 2 && !matches!(out.first(), Some(Seg::Arc { .. })) {
        return Err(other("A profile needs an area."));
    }
    Ok(out)
}

/// A profile object: its regions (relative to `origin`), and its plane's height.
struct Source {
    profiles: Vec<Profile>,
    origin: Vec3,
}

/// The regions of a region entity: each outer loop with the holes inside it.
fn region_profiles(loops: &[HatchLoop], o: Vec2) -> Result<Vec<Profile>> {
    let outers: Vec<&HatchLoop> = loops.iter().filter(|l| l.outer).collect();
    let holes: Vec<&HatchLoop> = loops.iter().filter(|l| !l.outer).collect();
    let mut out = Vec::new();
    for ol in &outers {
        let poly: Vec<Vec2> = ol.vertices.iter().map(|v| v.p).collect();
        let mut p = Profile { outer: loop_segs(&ol.vertices, o)?, holes: Vec::new() };
        for h in &holes {
            if h.vertices.first().is_some_and(|v| point_in_polygon(&poly, v.p)) {
                p.holes.push(loop_segs(&h.vertices, o)?);
            }
        }
        out.push(p);
    }
    Ok(out)
}

/// What object `kind` extrudes or revolves as; `None` for one that isn't a closed profile.
fn source(kind: &EntityKind, normal: Vec3) -> Option<Result<Source>> {
    // Profiles drawn in another plane than XY (an extrusion direction other than Z) are left out.
    if (normal - Vec3::Z).len() > 1e-9 {
        return None;
    }
    match kind {
        EntityKind::Circle(c) if c.radius > 0.0 => {
            let o = c.center;
            let outer = vec![Seg::Arc { center: Vec2::ZERO, radius: c.radius, start: 0.0, sweep: TAU }];
            Some(Ok(Source { profiles: vec![Profile { outer, holes: Vec::new() }], origin: o }))
        }
        EntityKind::LwPolyline(pl) if pl.closed || pl.vertices.first().zip(pl.vertices.last()).is_some_and(|(a, b)| a.p.near(b.p, 1e-12)) => {
            let o = pl.vertices.first().map(|v| v.p).unwrap_or(Vec2::ZERO);
            let mut vs = pl.vertices.clone();
            if vs.len() > 2 && vs.first().zip(vs.last()).is_some_and(|(a, b)| a.p.near(b.p, 1e-12)) {
                vs.pop();
            }
            Some(loop_segs(&vs, o).map(|outer| Source { profiles: vec![Profile { outer, holes: Vec::new() }], origin: o.to3(pl.elevation) }))
        }
        EntityKind::Region(r) => {
            let o = r.bounds().min;
            let o = if o.is_finite() { o } else { Vec2::ZERO };
            Some(region_profiles(&r.loops, o).map(|profiles| Source { profiles, origin: o.to3(0.0) }))
        }
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Extrude,
    Revolve,
}

/// What to make from each profile.
#[derive(Clone, Copy)]
enum Make {
    Extrude { height: f64, taper: f64 },
    Revolve { a: Vec2, b: Vec2, angle: f64 },
}

/// Make solids from the profiles among `hs`, all or none; erase the profiles unless `keep`.
fn make(s: &mut Session, hs: &[Handle], how: Make, keep: bool) -> Result<Value> {
    s.all_or_nothing(|s| make_all(s, hs, how, keep))
}

fn make_all(s: &mut Session, hs: &[Handle], how: Make, keep: bool) -> Result<Value> {
    let mut work = Vec::new();
    {
        let d = s.doc()?;
        for h in hs {
            let Some(e) = d.entity(*h) else { continue };
            if let Some(src) = source(&e.kind, e.common.extrusion) {
                work.push((*h, src?));
            }
        }
    }
    if work.is_empty() {
        return Err(other("Select closed polylines, circles or regions."));
    }
    let mut made = Vec::new();
    for (h, src) in work {
        let frame = Frame::xy(0.0);
        for prof in &src.profiles {
            let bodies = match how {
                Make::Extrude { height, taper } => solid::extrude(&frame, prof, height, taper),
                Make::Revolve { a, b, angle } => {
                    let (a, b) = (a - src.origin.xy(), b - src.origin.xy());
                    solid::revolve(&frame, prof, a, b - a, angle)
                }
            }
            .map_err(super::kernel)?;
            for body in bodies {
                let kind = solid_kind(&body, translation(src.origin))?;
                made.push(add_like(s, h, kind)?);
            }
        }
        if !keep {
            s.doc_mut()?.remove_entity(h);
        }
    }
    Ok(json!({ "handles": made.iter().map(|h| h.hex()).collect::<Vec<_>>() }))
}

fn run_extrude(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let height = f64_req("extrude", p, "height")?;
    let taper = f64_or(p, "taper", 0.0).to_radians();
    make(s, &hs, Make::Extrude { height, taper }, bool_or(p, "keepProfiles", false))
}

fn run_revolve(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let a = point_param(p, "axisStart").ok_or_else(|| bad("revolve", "`axisStart` ([x, y]) is required"))?;
    let b = point_param(p, "axisEnd").ok_or_else(|| bad("revolve", "`axisEnd` ([x, y]) is required"))?;
    if a.dist(b) <= 1e-12 {
        return Err(bad("revolve", "the axis needs two different points"));
    }
    let angle = f64_or(p, "angle", 360.0).to_radians();
    make(s, &hs, Make::Revolve { a, b, angle }, bool_or(p, "keepProfiles", false))
}

/// EXTRUDE and REVOLVE at the command line.
struct SweepM {
    mode: Mode,
    sel: SelectPhase,
    picked: Option<Vec<Handle>>,
    taper: f64,
    asking_taper: bool,
    axis_start: Option<Vec2>,
    axis_end: Option<Vec2>,
}

impl SweepM {
    fn new(mode: Mode) -> Self {
        SweepM { mode, sel: SelectPhase::default(), picked: None, taper: 0.0, asking_taper: false, axis_start: None, axis_end: None }
    }
    fn finish(&self, s: &mut Session, how: Make) -> Result<Step> {
        let hs = self.picked.clone().unwrap_or_default();
        make(s, &hs, how, false)?;
        s.set_selection(Vec::new());
        Ok(Step::Done)
    }
}

impl Interactive for SweepM {
    fn name(&self) -> &'static str {
        match self.mode {
            Mode::Extrude => "EXTRUDE",
            Mode::Revolve => "REVOLVE",
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
            let mut p = self.sel.prompt();
            p.message = match self.mode {
                Mode::Extrude => "Select objects to extrude".into(),
                Mode::Revolve => "Select objects to revolve".into(),
            };
            return p;
        }
        match self.mode {
            Mode::Extrude if self.asking_taper => Prompt::new("Specify angle of taper for extrusion", Accept::NUMBER).default("0"),
            Mode::Extrude => Prompt::new("Specify height of extrusion", Accept::POINT_OR_NUMBER).kw(&["Taper angle"]),
            Mode::Revolve => match (self.axis_start, self.axis_end) {
                (None, _) => Prompt::new("Specify axis start point or define axis by", Accept::POINT).kw(&["X", "Y"]),
                (Some(a), None) => Prompt::new("Specify axis endpoint", Accept::POINT).base(a),
                _ => Prompt::new("Specify angle of revolution", Accept::NUMBER).default("360"),
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
        match self.mode {
            Mode::Extrude => {
                if self.asking_taper {
                    self.asking_taper = false;
                    if let Input::Text(t) = &i {
                        let a = number(t).filter(|a| a.abs() < 90.0).ok_or_else(|| other("The taper angle must be between -90 and 90."))?;
                        self.taper = a.to_radians();
                    }
                    return Ok(Step::Continue);
                }
                match i {
                    Input::Keyword(_) => {
                        self.asking_taper = true;
                        Ok(Step::Continue)
                    }
                    Input::Text(t) => {
                        let h = number(&t).ok_or_else(|| other("Requires a distance."))?;
                        self.finish(s, Make::Extrude { height: h, taper: self.taper })
                    }
                    Input::Enter | Input::Cancel => Ok(Step::Cancel),
                    _ => Ok(Step::Continue),
                }
            }
            Mode::Revolve => match (self.axis_start, self.axis_end, i) {
                (None, _, Input::Point(p)) => {
                    self.axis_start = Some(p);
                    Ok(Step::Continue)
                }
                (None, _, Input::Keyword(k)) => {
                    self.axis_start = Some(Vec2::ZERO);
                    self.axis_end = Some(if k == "X" { Vec2::X } else { Vec2::Y });
                    Ok(Step::Continue)
                }
                (Some(a), None, Input::Point(p)) if !p.near(a, 1e-12) => {
                    self.axis_end = Some(p);
                    Ok(Step::Continue)
                }
                (Some(a), Some(b), Input::Text(t)) => {
                    let ang = number(&t).ok_or_else(|| other("Requires an angle."))?;
                    self.finish(s, Make::Revolve { a, b, angle: ang.to_radians() })
                }
                (Some(a), Some(b), Input::Enter) => self.finish(s, Make::Revolve { a, b, angle: TAU }),
                (_, _, Input::Cancel) => Ok(Step::Cancel),
                _ => Ok(Step::Continue),
            },
        }
    }
}
