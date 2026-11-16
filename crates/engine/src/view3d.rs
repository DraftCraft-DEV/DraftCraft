//! The 3D view of model space (VPOINT, 3DORBIT, VSCURRENT; PLAN goes back to the plan view):
//! which way the model is seen, how it is framed, and the visual style it is drawn in
//! (docs/solids.md, S4).
//!
//! The view is orthographic. Points are seen on the view plane through the drawing's origin:
//! a point's place on that plane (`to_plane`) is in drawing units, and `center`/`height` frame it
//! on screen exactly as [`crate::View`] frames the plan view.

use draftcraft_doc::{Drawing, EntityKind, Space};
use draftcraft_geom::{Vec2, Vec3};

/// The preset views (View ▸ 3D Views) as (command id, label), and the one a direction is.
pub use crate::cmd::solids::look::{PRESET_VIEWS, preset_of};

/// How the 3D view draws solids (VSCURRENT). The plan view (AutoCAD's 2D Wireframe) is no 3D
/// view at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualStyle {
    /// Every edge.
    Wireframe,
    /// The edges a viewer sees; faces hide what is behind them.
    Hidden,
    /// Faces lit in the object's colour.
    Realistic,
    /// Faces in warm and cool tones of the object's colour, with their edges.
    Conceptual,
    /// Faces lit in the object's colour.
    Shaded,
    /// Faces lit in the object's colour, with their edges.
    ShadedWithEdges,
    /// Faces in shades of grey, with their edges.
    ShadesOfGray,
    /// See-through faces with their edges.
    XRay,
}

/// The styles and their names (VSCURRENT's options, lower case, without spaces).
const STYLES: [(VisualStyle, &str, &str); 8] = [
    (VisualStyle::Wireframe, "wireframe", "Wireframe"),
    (VisualStyle::Hidden, "hidden", "Hidden"),
    (VisualStyle::Realistic, "realistic", "Realistic"),
    (VisualStyle::Conceptual, "conceptual", "Conceptual"),
    (VisualStyle::Shaded, "shaded", "Shaded"),
    (VisualStyle::ShadedWithEdges, "shadedwithedges", "Shaded with edges"),
    (VisualStyle::ShadesOfGray, "shadesofgray", "Shades of Gray"),
    (VisualStyle::XRay, "xray", "X-Ray"),
];

impl VisualStyle {
    /// The style's name as VSCURRENT shows it.
    pub fn label(self) -> &'static str {
        STYLES.iter().find(|(s, ..)| *s == self).map_or("", |(_, _, l)| l)
    }
    /// The style's option name (lower case, no spaces).
    pub fn name(self) -> &'static str {
        STYLES.iter().find(|(s, ..)| *s == self).map_or("", |(_, n, _)| n)
    }
    /// The style named `s` (case, spaces, dashes and the AutoCAD "3d" prefix of 3D Wireframe
    /// ignored).
    pub fn parse(s: &str) -> Option<VisualStyle> {
        let key: String = s.chars().filter(|c| !matches!(c, ' ' | '-' | '_')).collect::<String>().to_ascii_lowercase();
        let key = key.strip_prefix("3d").unwrap_or(&key);
        STYLES.iter().find(|(_, n, _)| *n == key).map(|(s, ..)| *s)
    }
    /// The option names, for prompts and parameter docs.
    pub fn names() -> impl Iterator<Item = &'static str> {
        STYLES.iter().map(|(_, n, _)| *n)
    }
    /// Faces are filled (every style but Wireframe).
    pub fn fills(self) -> bool {
        self != VisualStyle::Wireframe
    }
    /// Faces are lit (shaded) rather than filled with the background.
    pub fn shades(self) -> bool {
        !matches!(self, VisualStyle::Wireframe | VisualStyle::Hidden)
    }
    /// Edges are drawn.
    pub fn edges(self) -> bool {
        !matches!(self, VisualStyle::Realistic | VisualStyle::Shaded)
    }
}

/// A 3D view of model space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View3d {
    /// The direction the viewer looks along (unit).
    pub dir: Vec3,
    /// The view plane's centre on screen, in its own coordinates ([`View3d::to_plane`]).
    pub center: Vec2,
    /// How much of the view plane the screen shows, top to bottom, in drawing units.
    pub height: f64,
    pub style: VisualStyle,
}

/// The steepest a view can look (degrees from the XY plane), so up stays defined.
const MAX_ELEVATION: f64 = 90.0;

impl View3d {
    /// A view looking along `dir`; `None` for a direction that is no direction.
    pub fn new(dir: Vec3, center: Vec2, height: f64, style: VisualStyle) -> Option<View3d> {
        let d = dir.normalized();
        let usable = d.is_finite() && d.len() > 0.5 && center.is_finite() && height.is_finite() && height > 1e-12;
        usable.then_some(View3d { dir: d, center, height, style })
    }

    /// The view plane's axes in the drawing: screen right and screen up. Up is the drawing's Z
    /// as the viewer sees it; looking straight down (or up) it is the drawing's Y, so the top
    /// view is the plan view.
    pub fn axes(&self) -> (Vec3, Vec3) {
        let d = self.dir;
        let mut up = Vec3::Z - d * d.dot(Vec3::Z);
        if up.len() < 1e-9 {
            up = Vec3::Y;
        }
        let x = d.cross(up).normalized();
        let y = x.cross(d).normalized();
        (x, y)
    }

    /// Where drawing point `p` is on the view plane, and how far along the view direction it
    /// is (larger is farther from the viewer).
    pub fn to_plane(&self, p: Vec3) -> (Vec2, f64) {
        let (x, y) = self.axes();
        (Vec2::new(p.dot(x), p.dot(y)), p.dot(self.dir))
    }

    /// The point of the drawing's XY plane seen at view-plane point `q`; `None` when the view
    /// looks along that plane (front, side views).
    pub fn on_xy(&self, q: Vec2) -> Option<Vec2> {
        let (x, y) = self.axes();
        if self.dir.z.abs() < 1e-6 {
            return None;
        }
        let p = x * q.x + y * q.y;
        let t = -p.z / self.dir.z;
        let w = p + self.dir * t;
        w.is_finite().then(|| w.xy())
    }

    /// The viewer's compass direction (degrees in XY from the X axis, toward the viewer) and
    /// elevation (degrees above the XY plane).
    pub fn angles(&self) -> (f64, f64) {
        let v = -self.dir;
        let el = v.z.clamp(-1.0, 1.0).asin().to_degrees();
        // Straight down or up: the plan view, seen from the south.
        let az = if v.x.hypot(v.y) < 1e-9 { -90.0 } else { v.y.atan2(v.x).to_degrees() };
        (az, el)
    }

    /// This view turned about the drawing's Z axis by `azimuth` degrees and tilted by
    /// `elevation` degrees (3DORBIT), keeping the framing. The elevation stops at straight
    /// down and straight up.
    pub fn orbited(&self, azimuth: f64, elevation: f64) -> Option<View3d> {
        if !(azimuth.is_finite() && elevation.is_finite()) {
            return None;
        }
        let (az, el) = self.angles();
        let az = (az + azimuth).to_radians();
        let el = (el + elevation).clamp(-MAX_ELEVATION, MAX_ELEVATION).to_radians();
        let v = Vec3::new(el.cos() * az.cos(), el.cos() * az.sin(), el.sin());
        // Keep the orbit's compass direction at the poles: a view straight down is seen from
        // that direction's side by `axes` only when it is not exactly vertical.
        let v = if el.cos() < 1e-9 { Vec3::new(1e-7 * az.cos(), 1e-7 * az.sin(), el.sin()) } else { v };
        View3d::new(-v, self.center, self.height, self.style)
    }

    /// The view moved by `by` (view-plane units): panning.
    pub fn panned(&self, by: Vec2) -> Option<View3d> {
        View3d::new(self.dir, self.center + by, self.height, self.style)
    }

    /// The view zoomed by `factor` (above 1 shows less) about view-plane point `at`.
    pub fn zoomed(&self, factor: f64, at: Vec2) -> Option<View3d> {
        if !(factor.is_finite() && factor > 0.0) {
            return None;
        }
        let height = self.height / factor;
        let center = at + (self.center - at) * (1.0 / factor);
        View3d::new(self.dir, center, height, self.style)
    }

    /// This view framed around the drawing's model space (with a margin), or kept when there
    /// is nothing to frame.
    pub fn fitted(&self, d: &Drawing, aspect: f64) -> View3d {
        let mut min = Vec2::new(f64::INFINITY, f64::INFINITY);
        let mut max = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut add = |p: Vec3| {
            let (q, _) = self.to_plane(p);
            if q.is_finite() {
                min = Vec2::new(min.x.min(q.x), min.y.min(q.y));
                max = Vec2::new(max.x.max(q.x), max.y.max(q.y));
            }
        };
        if let Some(st) = d.space(&Space::Model) {
            for e in st.iter() {
                match &e.kind {
                    EntityKind::Solid3d(so) => so.look.positions.iter().for_each(|p| add(so.world(*p))),
                    _ => {
                        let b = draftcraft_doc::extent_bounds(d, e);
                        if b.min.is_finite() && b.max.is_finite() && b.min.x <= b.max.x {
                            b.corners().iter().for_each(|c| add(c.to3(0.0)));
                        }
                    }
                }
            }
        }
        if !(min.x <= max.x && min.y <= max.y) {
            return *self;
        }
        let aspect = if aspect.is_finite() && aspect > 1e-6 { aspect } else { 1.0 };
        let w = (max.x - min.x).max(1e-9);
        let h = (max.y - min.y).max(1e-9);
        let height = h.max(w / aspect) * 1.1;
        View3d::new(self.dir, (min + max) * 0.5, height, self.style).unwrap_or(*self)
    }
}

impl crate::Session {
    /// The 3D view of the current drawing's model space; `None` in the plan view, in a layout
    /// and inside a layout viewport.
    pub fn view3d(&self) -> Option<View3d> {
        let st = self.state().ok()?;
        if st.space != Space::Model || st.block_edit.is_some() {
            return None;
        }
        st.view3d
    }
    /// Show model space in `v` (`None`: back to the plan view). Navigation, not an edit: no undo.
    pub fn set_view3d(&mut self, v: Option<View3d>) {
        if let Ok(st) = self.state_mut() {
            st.view3d = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec2, b: Vec2) -> bool {
        (a - b).len() < 1e-9
    }

    #[test]
    fn the_top_view_is_the_plan_view() {
        let v = View3d::new(Vec3::new(0.0, 0.0, -1.0), Vec2::ZERO, 10.0, VisualStyle::Hidden).unwrap();
        let (q, _) = v.to_plane(Vec3::new(3.0, 4.0, 7.0));
        assert!(close(q, Vec2::new(3.0, 4.0)), "{q:?}");
        assert!(close(v.on_xy(Vec2::new(3.0, 4.0)).unwrap(), Vec2::new(3.0, 4.0)));
    }

    #[test]
    fn an_iso_view_finds_the_xy_point_under_the_cursor() {
        let v = View3d::new(Vec3::new(-1.0, 1.0, -1.0), Vec2::ZERO, 10.0, VisualStyle::Shaded).unwrap();
        let p = Vec2::new(2.0, -5.0);
        let (q, _) = v.to_plane(p.to3(0.0));
        assert!(close(v.on_xy(q).unwrap(), p));
        // Z points up on screen.
        let (_, up) = v.axes();
        assert!(up.z > 0.5, "{up:?}");
        // Looking along the XY plane there is no point to find.
        let front = View3d::new(Vec3::Y, Vec2::ZERO, 10.0, VisualStyle::Shaded).unwrap();
        assert!(front.on_xy(Vec2::new(1.0, 1.0)).is_none());
    }

    #[test]
    fn orbiting_turns_and_tilts_and_stops_at_the_pole() {
        let top = View3d::new(Vec3::new(0.0, 0.0, -1.0), Vec2::ZERO, 10.0, VisualStyle::Wireframe).unwrap();
        let (az, el) = top.angles();
        assert!((az + 90.0).abs() < 1e-9 && (el - 90.0).abs() < 1e-9);
        let tilted = top.orbited(0.0, -45.0).unwrap();
        let (az, el) = tilted.angles();
        assert!((az + 90.0).abs() < 1e-6 && (el - 45.0).abs() < 1e-6, "{az} {el}");
        // Tilting from the south keeps Y pointing away on screen.
        let (_, up) = tilted.axes();
        assert!(up.y > 0.0 && up.z > 0.0, "{up:?}");
        let over = tilted.orbited(30.0, 500.0).unwrap();
        assert!((over.angles().1 - 90.0).abs() < 1e-3);
        assert!(top.orbited(f64::NAN, 0.0).is_none());
        assert!(View3d::new(Vec3::ZERO, Vec2::ZERO, 1.0, VisualStyle::Hidden).is_none());
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let v = View3d::new(Vec3::new(0.0, 0.0, -1.0), Vec2::new(5.0, 5.0), 10.0, VisualStyle::Hidden).unwrap();
        let at = Vec2::new(8.0, 2.0);
        let z = v.zoomed(2.0, at).unwrap();
        assert!((z.height - 5.0).abs() < 1e-12);
        // `at` sits at the same fraction of the screen.
        assert!(close((at - v.center) * (1.0 / v.height), (at - z.center) * (1.0 / z.height)));
        assert!(v.zoomed(0.0, at).is_none());
    }

    #[test]
    fn style_names_parse_like_vscurrent_options() {
        assert_eq!(VisualStyle::parse("Shaded with edges"), Some(VisualStyle::ShadedWithEdges));
        assert_eq!(VisualStyle::parse("3D Wireframe"), Some(VisualStyle::Wireframe));
        assert_eq!(VisualStyle::parse("X-Ray"), Some(VisualStyle::XRay));
        assert_eq!(VisualStyle::parse("nope"), None);
        for n in VisualStyle::names() {
            assert_eq!(VisualStyle::parse(n).map(VisualStyle::name), Some(n));
        }
    }
}
