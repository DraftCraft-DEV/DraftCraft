//! AREATOTAL: the total area, perimeter and length of the selected objects, with how many were
//! measured. AutoCAD has no single command for it (people add it with plug-ins); AREA's Object and
//! Add area options total areas one pick at a time, this totals a whole selection at once.
//!
//! Closed objects (circles, closed polylines and splines, full ellipses, regions, hatches, 2D
//! solids) add their area and their perimeter; open curves (lines, arcs, open polylines and
//! splines, elliptical arcs) add only their length. Anything else is skipped and counted.

use std::collections::BTreeMap;

use draftcraft_doc::{EntityKind, Handle};
use draftcraft_geom::{Polyline, Vec2};
use serde_json::{Value, json};

use super::super::machines::{SelOutcome, SelectPhase};
use super::super::{bad, point_param, targets};
use super::{fmt_d, object_area};
use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

/// The command's id, as typed.
const CMD: &str = "areatotal";

/// AREATOTAL's JSON parameters.
pub(super) const PARAMS: &str = "{handles?: [...] (default: the current selection), at?: [x,y] (also place the totals there as text)} → \
     {count, measured, closed, open, area, perimeter (of the closed objects), length (of the open ones), totalLength, \
     skipped: {type: n}, items: [{handle, type, closed, area?, length}] (the first 1000), message, handle? (the text placed)}";

/// Objects listed one by one in the JSON result; the totals always cover all of them.
const MAX_ITEMS: usize = 1000;

/// What one object adds to the totals.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Measure {
    /// A closed object: its area and its perimeter.
    Closed { area: f64, perimeter: f64 },
    /// An open curve: its length.
    Open { length: f64 },
}

/// The totals of a selection.
#[derive(Default)]
pub(super) struct Totals {
    count: usize,
    closed: usize,
    open: usize,
    area: f64,
    perimeter: f64,
    length: f64,
    /// Objects skipped, by type (or by reason for ones that measure past f64's range).
    skipped: BTreeMap<String, usize>,
    items: Vec<Value>,
}

impl Totals {
    fn measured(&self) -> usize {
        self.closed.saturating_add(self.open)
    }
    fn total_length(&self) -> f64 {
        self.perimeter + self.length
    }
    fn skip(&mut self, why: &str) {
        let n = self.skipped.entry(why.to_string()).or_default();
        *n = n.saturating_add(1);
    }
}

/// Whether `a` and `b` are the same point, to a tolerance relative to the size of the drawing
/// around them.
fn same_point(a: Vec2, b: Vec2) -> bool {
    let scale = 1.0 + a.x.abs().max(a.y.abs()).max(b.x.abs()).max(b.y.abs());
    a.dist(b) <= scale * 1e-9
}

/// What `kind` adds to the totals, `None` for an object with neither area nor length (text, block
/// references, dimensions...). Errors are objects that measure past f64's range.
fn measure(s: &Session, h: Handle, kind: &EntityKind) -> std::result::Result<Option<Measure>, &'static str> {
    let closed = |s: &Session| match object_area(s, h) {
        Ok((area, perimeter)) if area.is_finite() && perimeter.is_finite() => Ok(Some(Measure::Closed { area, perimeter })),
        Ok(_) => Err("unmeasurable"),
        Err(_) => Ok(None),
    };
    let open = |length: f64| if length.is_finite() { Ok(Some(Measure::Open { length })) } else { Err("unmeasurable") };
    match kind {
        EntityKind::Line(l) => open((l.b - l.a).len()),
        EntityKind::Arc(a) => open(a.radius.abs() * draftcraft_geom::ccw_sweep(a.start, a.end)),
        EntityKind::Circle(_) | EntityKind::Region(_) | EntityKind::Hatch(_) => closed(s),
        EntityKind::Ellipse(el) => {
            let arc = draftcraft_geom::Ellipse { center: el.center.xy(), major: el.major.xy(), ratio: el.ratio, start: el.start, end: el.end };
            if arc.is_full() { closed(s) } else { open(arc.length()) }
        }
        EntityKind::LwPolyline(pl) => {
            // Drawn back to its first point without being closed, it still bounds an area.
            let ends = pl.vertices.first().zip(pl.vertices.last()).is_some_and(|(a, b)| same_point(a.p, b.p));
            if pl.closed || (pl.vertices.len() >= 3 && ends) {
                closed(s)
            } else {
                open(Polyline { vertices: pl.vertices.clone(), closed: false }.len())
            }
        }
        EntityKind::Polyline3d(pl) => {
            let length: f64 = pl.points.windows(2).map(|w| w.get(1).zip(w.first()).map_or(0.0, |(b, a)| (*b - *a).len())).sum();
            let closing = if pl.closed { pl.points.last().zip(pl.points.first()).map_or(0.0, |(b, a)| (*a - *b).len()) } else { 0.0 };
            // A closed one lying flat bounds an area; one that leaves its plane is measured by length only.
            let z0 = pl.points.first().map_or(0.0, |p| p.z);
            let flat = pl.points.iter().all(|p| (p.z - z0).abs() <= 1e-9 * (1.0 + z0.abs()));
            if pl.closed && flat && pl.points.len() >= 3 {
                let xy: Vec<Vec2> = pl.points.iter().map(|p| p.xy()).collect();
                let (area, perimeter) = (draftcraft_geom::shoelace(&xy).abs(), length + closing);
                if area.is_finite() && perimeter.is_finite() { Ok(Some(Measure::Closed { area, perimeter })) } else { Err("unmeasurable") }
            } else {
                open(length + closing)
            }
        }
        EntityKind::Spline(sp) => {
            if !sp.is_valid() {
                return Ok(None);
            }
            let (a, b) = sp.domain();
            if sp.closed || same_point(sp.eval(a), sp.eval(b)) { closed(s) } else { open(sp.checked_length().unwrap_or(f64::INFINITY)) }
        }
        EntityKind::Solid(sd) | EntityKind::Trace(sd) => {
            // Corners in drawing order 1, 2, 4, 3; a triangle repeats its third corner.
            let [c0, c1, c2, c3] = sd.corners;
            let mut pts = vec![c0.xy(), c1.xy(), c3.xy(), c2.xy()];
            pts.dedup_by(|a, b| same_point(*a, *b));
            if pts.len() > 2 && pts.first().zip(pts.last()).is_some_and(|(a, b)| same_point(*a, *b)) {
                pts.pop();
            }
            let n = pts.len();
            let perimeter: f64 = (0..n).filter_map(|i| Some(pts.get(i)?.dist(*pts.get((i + 1) % n)?))).sum();
            let area = draftcraft_geom::shoelace(&pts).abs();
            if area.is_finite() && perimeter.is_finite() { Ok(Some(Measure::Closed { area, perimeter })) } else { Err("unmeasurable") }
        }
        _ => Ok(None),
    }
}

/// The totals of the objects `hs`. Handles that are not in the drawing are skipped as
/// "not found"; a total past f64's range is an error.
pub(super) fn totals(s: &Session, hs: &[Handle]) -> Result<Totals> {
    let d = s.doc()?;
    let mut t = Totals::default();
    for &h in hs {
        t.count = t.count.saturating_add(1);
        let Some(e) = d.entity(h) else {
            t.skip("not found");
            continue;
        };
        let ty = e.kind.type_name();
        let m = match measure(s, h, &e.kind) {
            Ok(Some(m)) => m,
            Ok(None) => {
                t.skip(ty);
                continue;
            }
            Err(why) => {
                t.skip(why);
                continue;
            }
        };
        let item = match m {
            Measure::Closed { area, perimeter } => {
                t.closed = t.closed.saturating_add(1);
                t.area += area;
                t.perimeter += perimeter;
                json!({ "handle": h.hex(), "type": ty, "closed": true, "area": area, "length": perimeter })
            }
            Measure::Open { length } => {
                t.open = t.open.saturating_add(1);
                t.length += length;
                json!({ "handle": h.hex(), "type": ty, "closed": false, "length": length })
            }
        };
        if t.items.len() < MAX_ITEMS {
            t.items.push(item);
        }
    }
    if !t.area.is_finite() {
        return Err(bad(CMD, "the total area is too large"));
    }
    if !t.perimeter.is_finite() || !t.length.is_finite() || !t.total_length().is_finite() {
        return Err(bad(CMD, "the total length is too large"));
    }
    Ok(t)
}

/// The report, one line each: what was measured, the area and perimeter of the closed objects,
/// the length of the open ones and what was skipped.
fn report(s: &Session, t: &Totals) -> String {
    let mut lines = vec![format!("{} object(s) measured: {} closed, {} open", t.measured(), t.closed, t.open)];
    if t.closed > 0 {
        lines.push(format!("Total area = {}, Total perimeter = {}", fmt_d(s, t.area), fmt_d(s, t.perimeter)));
    }
    if t.open > 0 {
        lines.push(format!("Total length of open objects = {}", fmt_d(s, t.length)));
    }
    if t.closed > 0 && t.open > 0 {
        lines.push(format!("Perimeter and length together = {}", fmt_d(s, t.total_length())));
    }
    if !t.skipped.is_empty() {
        let what: Vec<String> = t.skipped.iter().map(|(why, n)| format!("{n} {why}")).collect();
        lines.push(format!("Skipped, no area or length: {}", what.join(", ")));
    }
    lines.join("\n")
}

/// The totals as one line of text to place in the drawing.
fn label(s: &Session, t: &Totals) -> String {
    let mut parts = Vec::new();
    if t.closed > 0 {
        parts.push(format!("Area = {}", fmt_d(s, t.area)));
        parts.push(format!("Perimeter = {}", fmt_d(s, t.perimeter)));
    }
    if t.open > 0 {
        parts.push(format!("Length = {}", fmt_d(s, t.length)));
    }
    parts.join(", ")
}

/// Place the totals at `at` as a single-line text, in the current style and TEXTSIZE.
fn place(s: &mut Session, t: &Totals, at: Vec2) -> Result<Handle> {
    let height = s.doc()?.header.f64("TEXTSIZE", 0.2);
    let height = if height.is_finite() && height > 0.0 { height } else { 0.2 };
    let value = label(s, t);
    let k = super::super::draw::text::make_text(s, at, &value, height, 0.0, (draftcraft_doc::HAlign::Left, draftcraft_doc::VAlign::Baseline));
    s.add_entity(k)
}

/// Keep the totals in the AREA and PERIMETER system variables, as AREA does: the total area, and
/// the perimeter and length together.
fn remember(s: &mut Session, t: &Totals) {
    s.area = t.area;
    s.perimeter = t.total_length();
}

pub(super) fn run(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    if hs.is_empty() {
        return Err(bad(CMD, "nothing is selected: give `handles` or select objects first"));
    }
    let at = match p.get("at") {
        None | Some(Value::Null) => None,
        Some(_) => Some(point_param(p, "at").ok_or_else(|| bad(CMD, "`at` must be a point [x, y]"))?),
    };
    let t = totals(s, &hs)?;
    remember(s, &t);
    let mut out = json!({
        "count": t.count,
        "measured": t.measured(),
        "closed": t.closed,
        "open": t.open,
        "area": t.area,
        "perimeter": t.perimeter,
        "length": t.length,
        "totalLength": t.total_length(),
        "skipped": t.skipped,
        "items": t.items,
        "itemsTruncated": t.measured() > t.items.len(),
        "message": report(s, &t),
    });
    if let Some(at) = at {
        if t.measured() == 0 {
            return Err(bad(CMD, "none of the objects has an area or a length to place"));
        }
        let h = place(s, &t, at)?;
        if let Some(o) = out.as_object_mut() {
            o.insert("handle".into(), json!(h.hex()));
        }
    }
    Ok(out)
}

/// AREATOTAL at the command line: select objects (or use the ones already selected), read the
/// totals, then optionally place them as text.
#[derive(Default)]
pub(super) struct AreaTotalM {
    sel: SelectPhase,
    /// The totals measured, while asking where to place them.
    totals: Option<Totals>,
}

impl AreaTotalM {
    /// Measure and report `hs`; ends the command when nothing could be measured.
    fn measured(&mut self, s: &mut Session, hs: &[Handle]) -> Step {
        match totals(s, hs) {
            Ok(t) => {
                for l in report(s, &t).lines() {
                    s.echo(l.to_string());
                }
                remember(s, &t);
                if t.measured() == 0 {
                    return Step::Done;
                }
                self.totals = Some(t);
                Step::Continue
            }
            Err(e) => {
                s.echo(e.to_string());
                Step::Done
            }
        }
    }
}

impl Interactive for AreaTotalM {
    fn name(&self) -> &'static str {
        "AREATOTAL"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.sel = SelectPhase::begin(s);
        if self.sel.done {
            let hs = self.sel.picked.clone();
            return Ok(self.measured(s, &hs));
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.totals.is_some() {
            return Prompt::new("Specify point to place the totals as text", Accept::POINT).kw(&["eXit"]).default("eXit");
        }
        self.sel.prompt()
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if let Some(t) = &self.totals {
            match i {
                Input::Point(at) => {
                    if let Err(e) = place(s, t, at) {
                        s.echo(e.to_string());
                    }
                }
                Input::Text(_) => {
                    s.echo("Point or option keyword required.");
                    return Ok(Step::Continue);
                }
                _ => {}
            }
            return Ok(Step::Done);
        }
        match self.sel.feed(s, &i)? {
            SelOutcome::More => Ok(Step::Continue),
            SelOutcome::Empty => Ok(Step::Done),
            SelOutcome::Done(hs) => Ok(self.measured(s, &hs)),
        }
    }
}
