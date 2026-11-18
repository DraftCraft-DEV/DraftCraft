//! Seeing model space in 3D: VPOINT, 3DORBIT, PLAN and VSCURRENT (docs/solids.md, S4). These
//! change the view, never the drawing.

use draftcraft_geom::Vec3;
use serde_json::{Value, json};

use super::super::{CommandSpec, bad, f64_or, str_param};
use super::point3;
use super::views::named_view;
use crate::view3d::{View3d, VisualStyle};
use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

const VIEWS: &str = "top, bottom, front, back, left, right, swiso, seiso, neiso or nwiso";

/// One preset view (View ▸ 3D Views ▸ Top … NW Isometric): VPOINT with that view, framed.
macro_rules! preset_view {
    ($id:literal, $label:literal, $name:literal) => {
        CommandSpec::new($id, $label, |s: &mut Session, p: &Value| {
            set_direction(
                s,
                "vpoint",
                named_view($name).map(|(d, _)| d).unwrap_or(Vec3::new(0.0, 0.0, -1.0)),
                p.get("fit").and_then(Value::as_bool).unwrap_or(true),
            )
        })
        .menu(&["View", "3D Views", $label])
        .params("{fit?: bool (default true)} → {direction, style}")
        .noundo()
    };
}

/// One visual style (View ▸ Visual Styles ▸ …): VSCURRENT with that style.
macro_rules! preset_style {
    ($id:literal, $label:literal, $name:literal) => {
        CommandSpec::new($id, $label, |s: &mut Session, _p: &Value| apply_style(s, $name))
            .menu(&["View", "Visual Styles", $label])
            .params("{} → {style}")
            .noundo()
    };
}

/// The preset views and visual styles, as the viewport controls and the ribbon offer them:
/// (command id, label).
pub const PRESET_VIEWS: [(&str, &str); 10] = [
    ("vpoint.top", "Top"),
    ("vpoint.bottom", "Bottom"),
    ("vpoint.left", "Left"),
    ("vpoint.right", "Right"),
    ("vpoint.front", "Front"),
    ("vpoint.back", "Back"),
    ("vpoint.swiso", "SW Isometric"),
    ("vpoint.seiso", "SE Isometric"),
    ("vpoint.neiso", "NE Isometric"),
    ("vpoint.nwiso", "NW Isometric"),
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        preset_view!("vpoint.top", "Top", "top"),
        preset_view!("vpoint.bottom", "Bottom", "bottom"),
        preset_view!("vpoint.left", "Left", "left"),
        preset_view!("vpoint.right", "Right", "right"),
        preset_view!("vpoint.front", "Front", "front"),
        preset_view!("vpoint.back", "Back", "back"),
        preset_view!("vpoint.swiso", "SW Isometric", "swiso"),
        preset_view!("vpoint.seiso", "SE Isometric", "seiso"),
        preset_view!("vpoint.neiso", "NE Isometric", "neiso"),
        preset_view!("vpoint.nwiso", "NW Isometric", "nwiso"),
        preset_style!("vscurrent.2dwireframe", "2D Wireframe", "2dwireframe"),
        preset_style!("vscurrent.wireframe", "Wireframe", "wireframe"),
        preset_style!("vscurrent.hidden", "Hidden", "hidden"),
        preset_style!("vscurrent.realistic", "Realistic", "realistic"),
        preset_style!("vscurrent.conceptual", "Conceptual", "conceptual"),
        preset_style!("vscurrent.shaded", "Shaded", "shaded"),
        preset_style!("vscurrent.shadedwithedges", "Shaded with edges", "shadedwithedges"),
        preset_style!("vscurrent.shadesofgray", "Shades of Gray", "shadesofgray"),
        preset_style!("vscurrent.xray", "X-Ray", "xray"),
        CommandSpec::new("vpoint", "Viewpoint", run_vpoint)
            .menu(&["View", "3D Views", "Viewpoint"])
            .params("{view?: \"top\"|\"bottom\"|\"front\"|\"back\"|\"left\"|\"right\"|\"swiso\"|\"seiso\"|\"neiso\"|\"nwiso\", point?: [x, y, z] (where the viewer is, looking toward the origin), fit?: bool (default true)} → {direction, style}")
            .noundo()
            .interactive(|_| Ok(Box::new(VpointM))),
        CommandSpec::new("3dorbit", "Orbit", run_orbit)
            .menu(&["View", "Orbit", "Constrained Orbit"])
            .alias(&["3do", "orbit"])
            .params("{azimuth?: degrees about Z, elevation?: degrees of tilt} → {direction, style}: turns the 3D view (from the plan view: starts at the top view); in the drawing area, drag to orbit")
            .noundo()
            .interactive(|_| Ok(Box::new(OrbitM))),
        CommandSpec::new("plan", "Plan View", run_plan)
            .menu(&["View", "3D Views", "Plan View"])
            .params("{} → {}: back to the plan view of model space")
            .noundo(),
        CommandSpec::new("vscurrent", "Visual Styles", run_vscurrent)
            .menu(&["View", "Visual Styles", "Visual Styles"])
            .alias(&["vs", "shademode"])
            .params("{style: \"2dwireframe\" (the plan view)|\"wireframe\"|\"hidden\"|\"realistic\"|\"conceptual\"|\"shaded\"|\"shadedwithedges\"|\"shadesofgray\"|\"xray\"} → {style}: from the plan view, a 3D style starts at the top view")
            .noundo()
            .interactive(|_| Ok(Box::new(VscurrentM))),
    ]
}

/// The preset view looking along `dir` (its command id and label), if it is one.
pub fn preset_of(dir: Vec3) -> Option<(&'static str, &'static str)> {
    let d = dir.normalized();
    PRESET_VIEWS
        .iter()
        .copied()
        .find(|(id, _)| id.strip_prefix("vpoint.").and_then(named_view).is_some_and(|(v, _)| (v.normalized() - d).len() < 1e-6))
}

/// The 3D view as JSON.
fn reply(v: &View3d) -> Value {
    json!({ "direction": [v.dir.x, v.dir.y, v.dir.z], "style": v.style.name() })
}

/// The screen's width over its height (for framing).
fn aspect(s: &Session) -> f64 {
    let (w, h) = s.viewport_px;
    if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 { w / h } else { 4.0 / 3.0 }
}

/// The current 3D view, or the top view framed like the plan view (`style` when given).
fn current_or_top(s: &Session, style: Option<VisualStyle>) -> Result<View3d> {
    if let Some(v) = s.view3d() {
        return Ok(View3d { style: style.unwrap_or(v.style), ..v });
    }
    let st = s.state()?;
    if st.space != draftcraft_doc::Space::Model || st.mspace.is_some() || st.block_edit.is_some() {
        return Err(bad("3D view", "3D views show model space: switch to the Model tab first"));
    }
    let pv = st.view();
    View3d::new(Vec3::new(0.0, 0.0, -1.0), pv.center, pv.height, style.unwrap_or(VisualStyle::Wireframe))
        .ok_or_else(|| bad("3D view", "the plan view can't be turned into a 3D view"))
}

/// Start a 3D view at the top view (for interactive 3DORBIT); nothing when there is one.
fn enter_3d(s: &mut Session) {
    if s.view3d().is_none()
        && let Ok(v) = current_or_top(s, None)
    {
        s.set_view3d(Some(v));
    }
}

fn run_vpoint(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = match (str_param(p, "view"), p.get("point")) {
        (Some(name), _) => named_view(name).map(|(d, _)| d).ok_or_else(|| bad("vpoint", format!("unknown view \"{name}\": use {VIEWS}")))?,
        (None, Some(v)) => -point3(v).ok_or_else(|| bad("vpoint", "point must be [x, y, z]"))?,
        (None, None) => return Err(bad("vpoint", "give a view name or a viewpoint")),
    };
    set_direction(s, "vpoint", dir, p.get("fit").and_then(Value::as_bool).unwrap_or(true))
}

/// Look along `dir`, keeping the style, framing the model when `fit`.
fn set_direction(s: &mut Session, cmd: &str, dir: Vec3, fit: bool) -> Result<Value> {
    let base = current_or_top(s, None)?;
    let v = View3d::new(dir, base.center, base.height, base.style).ok_or_else(|| bad(cmd, "the viewpoint can't be the origin"))?;
    let v = if fit { v.fitted(s.doc()?, aspect(s)) } else { v };
    s.set_view3d(Some(v));
    Ok(reply(&v))
}

fn run_orbit(s: &mut Session, p: &Value) -> Result<Value> {
    let az = f64_or(p, "azimuth", 0.0);
    let el = f64_or(p, "elevation", 0.0);
    if !(az.is_finite() && el.is_finite()) {
        return Err(bad("3dorbit", "azimuth and elevation must be finite angles"));
    }
    let v = current_or_top(s, None)?.orbited(az, el).ok_or_else(|| bad("3dorbit", "that orbit leaves no view"))?;
    s.set_view3d(Some(v));
    Ok(reply(&v))
}

fn run_plan(s: &mut Session, _p: &Value) -> Result<Value> {
    s.set_view3d(None);
    Ok(json!({}))
}

fn run_vscurrent(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "style").ok_or_else(|| bad("vscurrent", "style is required"))?;
    apply_style(s, name)
}

fn apply_style(s: &mut Session, name: &str) -> Result<Value> {
    let key: String = name.chars().filter(|c| !matches!(c, ' ' | '-' | '_')).collect::<String>().to_ascii_lowercase();
    if matches!(key.as_str(), "2dwireframe" | "2d" | "2") {
        s.set_view3d(None);
        return Ok(json!({ "style": "2dwireframe" }));
    }
    let style = VisualStyle::parse(&key).ok_or_else(|| bad("vscurrent", format!("unknown visual style \"{name}\"")))?;
    let v = current_or_top(s, Some(style))?;
    s.set_view3d(Some(v));
    Ok(reply(&v))
}

// ---- interactive ---------------------------------------------------------------------------

struct VpointM;

impl Interactive for VpointM {
    fn name(&self) -> &'static str {
        "VPOINT"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        Prompt::new("Specify a view point or choose a view", Accept::TEXT)
            .kw(&["Top", "Bottom", "Front", "Back", "Left", "Right", "SWiso", "SEiso", "NEiso", "NWiso"])
            .default("SEiso")
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let r = match i {
            Input::Cancel => return Ok(Step::Cancel),
            Input::Enter => run_vpoint(s, &json!({ "view": "seiso" })),
            Input::Keyword(k) => run_vpoint(s, &json!({ "view": k })),
            Input::Text(t) => match parse_xyz(&t) {
                Some(pt) => run_vpoint(s, &json!({ "point": pt })),
                None => run_vpoint(s, &json!({ "view": t })),
            },
            _ => return Ok(Step::Continue),
        };
        match r {
            Ok(_) => Ok(Step::Done),
            Err(e) => {
                s.echo(e.to_string());
                Ok(Step::Continue)
            }
        }
    }
}

/// "x,y,z" typed at the prompt.
fn parse_xyz(t: &str) -> Option<[f64; 3]> {
    let v: Vec<f64> = t.split(',').map(|c| c.trim().parse::<f64>()).collect::<std::result::Result<_, _>>().ok()?;
    match v.as_slice() {
        [x, y, z] if x.is_finite() && y.is_finite() && z.is_finite() => Some([*x, *y, *z]),
        _ => None,
    }
}

/// Interactive 3DORBIT: the drawing area turns the view while the mouse drags (the UI calls
/// [`Session::set_view3d`]); the prompt only ends the command.
struct OrbitM;

impl Interactive for OrbitM {
    fn name(&self) -> &'static str {
        "3DORBIT"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        enter_3d(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        Prompt::new("Drag in the drawing area to orbit; press Enter or Esc to exit", Accept::TEXT).kw(&["Plan"])
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Cancel | Input::Enter => Ok(Step::Done),
            Input::Keyword(k) | Input::Text(k) if k.eq_ignore_ascii_case("plan") => {
                s.set_view3d(None);
                Ok(Step::Done)
            }
            _ => Ok(Step::Continue),
        }
    }
}

struct VscurrentM;

impl Interactive for VscurrentM {
    fn name(&self) -> &'static str {
        "VSCURRENT"
    }
    fn prompt(&self, s: &Session) -> Prompt {
        let current = s.view3d().map_or("2dWireframe", |v| v.style.label());
        Prompt::new(format!("Enter an option (current: {current})"), Accept::TEXT)
            .kw(&["2dwireframe", "Wireframe", "Hidden", "Realistic", "Conceptual", "Shaded", "shadedwithEdges", "shadesofGray", "Xray"])
            .default("Wireframe")
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let name = match i {
            Input::Cancel => return Ok(Step::Cancel),
            Input::Enter => "wireframe".to_string(),
            Input::Keyword(k) | Input::Text(k) => k,
            _ => return Ok(Step::Continue),
        };
        match apply_style(s, &name) {
            Ok(_) => Ok(Step::Done),
            Err(e) => {
                s.echo(e.to_string());
                Ok(Step::Continue)
            }
        }
    }
}
