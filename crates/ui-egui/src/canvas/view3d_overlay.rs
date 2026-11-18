//! The 3D view's navigation overlays, laid out as on Windows: the viewport controls at the top
//! left (`[-][SE Isometric][Realistic]`, each a menu) and the ViewCube at the top right, a cube
//! turned with the view whose faces, edges and corners are clickable (26 views), with the
//! compass ring under it and the Home button beside it. Everything is drawn in code and acts
//! through commands (`vpoint`, `vpoint.*`, `vscurrent.*`, `plan`, `ui.toggle.viewcube`).

use draftcraft_engine::view3d::{PRESET_VIEWS, View3d, preset_of};
use draftcraft_geom::Vec3;
use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, pos2, vec2};
use serde_json::{Value, json};

use crate::CadApp;
use crate::icons::Icon;
use crate::theme::Tokens;

/// The visual styles in the viewport controls' style menu: (command, label, option name).
const STYLES: [(&str, &str, &str); 9] = [
    ("vscurrent.2dwireframe", "2D Wireframe", "2dwireframe"),
    ("vscurrent.wireframe", "Wireframe", "wireframe"),
    ("vscurrent.hidden", "Hidden", "hidden"),
    ("vscurrent.realistic", "Realistic", "realistic"),
    ("vscurrent.conceptual", "Conceptual", "conceptual"),
    ("vscurrent.shaded", "Shaded", "shaded"),
    ("vscurrent.shadedwithedges", "Shaded with edges", "shadedwithedges"),
    ("vscurrent.shadesofgray", "Shades of Gray", "shadesofgray"),
    ("vscurrent.xray", "X-Ray", "xray"),
];

/// Half a cube edge on screen, in points.
const HALF: f32 = 30.0;
/// How far from a face's middle (in face units, its edges at ±1) an edge or corner spot starts.
const SPOT: f64 = 0.55;

/// The cube's six faces: outward normal, the face's own right and up directions, its label.
const FACES: [(Vec3, Vec3, Vec3, &str); 6] = [
    (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), "TOP"),
    (Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), "BOTTOM"),
    (Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), "FRONT"),
    (Vec3::new(0.0, 1.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), "BACK"),
    (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), "RIGHT"),
    (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), "LEFT"),
];

/// Where the ViewCube's parts are.
pub(crate) struct Cube {
    pub center: Pos2,
    x: Vec3,
    y: Vec3,
    dir: Vec3,
    pub home: Rect,
    pub bounds: Rect,
}

/// A spot on the cube: the face hit and the face coordinates of the point, each in [-1, 1].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spot {
    face: usize,
    a: f64,
    b: f64,
}

impl Cube {
    pub(crate) fn new(canvas: Rect, v: &View3d) -> Cube {
        let center = pos2(canvas.right() - 100.0, canvas.top() + 100.0);
        let (x, y) = v.axes();
        Cube {
            center,
            x,
            y,
            dir: v.dir,
            home: Rect::from_center_size(center + vec2(-62.0, -66.0), vec2(20.0, 20.0)),
            bounds: Rect::from_center_size(center, vec2(170.0, 170.0)),
        }
    }

    /// A world direction (or a point of the unit cube) on screen, relative to the centre.
    fn off(&self, w: Vec3) -> egui::Vec2 {
        vec2(w.dot(self.x) as f32, -(w.dot(self.y) as f32)) * HALF
    }

    fn screen(&self, w: Vec3) -> Pos2 {
        self.center + self.off(w)
    }

    /// The faces turned toward the viewer.
    fn visible(&self) -> impl Iterator<Item = (usize, &'static (Vec3, Vec3, Vec3, &'static str))> + '_ {
        FACES.iter().enumerate().filter(|(_, (n, ..))| n.dot(self.dir) < -1e-6)
    }

    /// The spot under screen point `p`, if it is on a face turned toward the viewer.
    pub(crate) fn spot_at(&self, p: Pos2) -> Option<Spot> {
        for (i, (n, u, w, _)) in self.visible() {
            let (su, sw) = (self.off(*u), self.off(*w));
            let d = p - self.screen(*n);
            let det = f64::from(su.x * sw.y - su.y * sw.x);
            if det.abs() < 1e-6 {
                continue;
            }
            let a = f64::from(d.x * sw.y - d.y * sw.x) / det;
            let b = f64::from(su.x * d.y - su.y * d.x) / det;
            if a.abs() <= 1.0 && b.abs() <= 1.0 {
                return Some(Spot { face: i, a, b });
            }
        }
        None
    }

    /// Where the viewer stands for a spot (VPOINT's point): the face's normal, plus the edge or
    /// corner directions the spot is near.
    pub(crate) fn viewpoint(spot: Spot) -> Option<Vec3> {
        let (n, u, w, _) = FACES.get(spot.face)?;
        let pick = |t: f64, axis: Vec3| {
            if t > SPOT {
                axis
            } else if t < -SPOT {
                -axis
            } else {
                Vec3::ZERO
            }
        };
        Some(*n + pick(spot.a, *u) + pick(spot.b, *w))
    }

    /// The screen quad of a spot's area on its face.
    fn spot_quad(&self, spot: Spot) -> Option<[Pos2; 4]> {
        let (n, u, w, _) = FACES.get(spot.face)?;
        let range = |t: f64| {
            if t > SPOT {
                (SPOT, 1.0)
            } else if t < -SPOT {
                (-1.0, -SPOT)
            } else {
                (-SPOT, SPOT)
            }
        };
        let ((a0, a1), (b0, b1)) = (range(spot.a), range(spot.b));
        let at = |a: f64, b: f64| self.screen(*n + *u * a + *w * b);
        Some([at(a0, b0), at(a1, b0), at(a1, b1), at(a0, b1)])
    }
}

/// Paint the overlays of the 3D view and handle their clicks. `orbit_per_point` turns a drag on
/// the cube into degrees.
pub(super) fn show(app: &mut CadApp, ui: &mut egui::Ui, rect: Rect, v: &View3d, orbit_per_point: f64) {
    viewport_controls(app, ui, rect, v);
    if app.ui.show_viewcube {
        viewcube(app, ui, rect, v, orbit_per_point);
    }
    navigation_bar(app, ui, rect, v);
}

/// The navigation bar's buttons, top to bottom: (icon, what it does, tooltip).
const NAV: [(Icon, Nav, &str); 4] = [
    (Icon::ZoomExtents, Nav::Extents, "Zoom Extents"),
    (Icon::ZoomIn, Nav::In, "Zoom In"),
    (Icon::ZoomOut, Nav::Out, "Zoom Out"),
    (Icon::Orbit, Nav::Orbit, "Orbit"),
];

#[derive(Clone, Copy)]
enum Nav {
    Extents,
    In,
    Out,
    Orbit,
}

/// Where the navigation bar is: on the right, under the ViewCube.
fn nav_rect(rect: Rect) -> Rect {
    let size = 26.0;
    Rect::from_min_size(pos2(rect.right() - size - 8.0, rect.top() + 196.0), vec2(size, 10.0 + NAV.len() as f32 * size))
}

/// The navigation bar on the right of the 3D view, as in the plan view: zoom to the extents, in
/// and out, and orbit (middle-button drag pans).
fn navigation_bar(app: &mut CadApp, ui: &mut egui::Ui, rect: Rect, v: &View3d) {
    let size = 26.0;
    let bar = nav_rect(rect);
    if bar.bottom() > rect.bottom() - 60.0 {
        return;
    }
    let p = ui.painter_at(rect);
    p.rect_filled(bar, 3.0, Color32::from_rgba_unmultiplied(0x48, 0x50, 0x5c, 200));
    p.hline(bar.x_range().shrink(6.0), bar.top() + 5.0, Stroke::new(1.0, Color32::from_rgb(0x7a, 0x82, 0x8e)));
    let mut clicked = None;
    for (i, (icon, nav, tip)) in NAV.iter().enumerate() {
        let br = Rect::from_min_size(pos2(bar.left(), bar.top() + 10.0 + i as f32 * size), vec2(size, size));
        let resp = ui.interact(br, ui.scope_id().with(("navbar3d", i)), Sense::click());
        if resp.hovered() {
            p.rect_filled(br.shrink(1.0), 2.0, Color32::from_rgb(0x5c, 0x67, 0x7c));
        }
        crate::icons::paint(&p, br.shrink(5.0), *icon, false);
        crate::icons::describe_control(ui, &resp, tip, None, false);
        if resp.on_hover_text(crate::i18n::t(tip)).clicked() {
            clicked = Some(*nav);
        }
    }
    match clicked {
        // VPOINT from where the viewer stands frames the drawing.
        Some(Nav::Extents) => {
            let at = -v.dir;
            let _ = app.run("vpoint", json!({ "point": [at.x, at.y, at.z] }));
        }
        Some(Nav::In | Nav::Out) => {
            let factor = if matches!(clicked, Some(Nav::In)) { 1.5 } else { 1.0 / 1.5 };
            if let Some(z) = v.zoomed(factor, v.center) {
                app.session.set_view3d(Some(z));
                ui.ctx().request_repaint();
            }
        }
        Some(Nav::Orbit) => app.start("3dorbit"),
        None => {}
    }
}

/// The strip the viewport controls take at the top left.
fn controls_strip(rect: Rect) -> Rect {
    Rect::from_min_size(rect.left_top(), vec2(300.0, 26.0))
}

/// Whether screen point `p` is on one of the overlays (so the drawing area leaves it alone).
pub(super) fn covers(app: &CadApp, rect: Rect, v: &View3d, p: Pos2) -> bool {
    controls_strip(rect).contains(p) || nav_rect(rect).contains(p) || (app.ui.show_viewcube && Cube::new(rect, v).bounds.contains(p))
}

/// `[-][view][style]` at the top left, each bracket a menu.
fn viewport_controls(app: &mut CadApp, ui: &mut egui::Ui, rect: Rect, v: &View3d) {
    let t = Tokens::get();
    let font = crate::theme::small();
    let current = preset_of(v.dir);
    let view = current.map_or_else(|| crate::tl!("Custom View").to_string(), |(_, l)| crate::i18n::t(l).to_string());
    let style = crate::i18n::t(v.style.label()).to_string();
    let parts = ["-".to_string(), view, style];
    let p = ui.painter_at(rect);
    let mut x = rect.left() + 8.0;
    let mut action: Option<(&'static str, Value)> = None;
    let cube_on = app.ui.show_viewcube;
    for (i, label) in parts.iter().enumerate() {
        let text = format!("[{label}]");
        let galley = p.layout_no_wrap(text.clone(), font.clone(), t.canvas_ink);
        let r = Rect::from_min_size(pos2(x, rect.top() + 6.0), galley.size() + vec2(2.0, 2.0));
        x = r.right() + 2.0;
        let resp = ui.interact(r, ui.scope_id().with(("vp_controls", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let ink = if resp.hovered() { t.accent } else { t.text_dim };
        p.text(r.min + vec2(1.0, 1.0), egui::Align2::LEFT_TOP, text, font.clone(), ink);
        egui::Popup::menu(&resp).show(|ui| {
            let mut chosen: Option<(&'static str, Value)> = None;
            match i {
                0 => {
                    let mut on = cube_on;
                    if ui.checkbox(&mut on, crate::tl!("ViewCube")).clicked() {
                        chosen = Some(("ui.toggle.viewcube", json!({ "on": on })));
                    }
                    ui.separator();
                    if ui.button(crate::tl!("Plan View")).clicked() {
                        chosen = Some(("plan", json!({})));
                    }
                }
                1 => {
                    for (n, (id, label)) in PRESET_VIEWS.iter().enumerate() {
                        if n == 6 {
                            ui.separator();
                        }
                        if ui.radio(current.is_some_and(|(c, _)| c == *id), crate::i18n::t(label)).clicked() {
                            chosen = Some((id, json!({})));
                        }
                    }
                    ui.separator();
                    if ui.button(crate::tl!("Plan View")).clicked() {
                        chosen = Some(("plan", json!({})));
                    }
                }
                _ => {
                    for (id, label, name) in STYLES {
                        if ui.radio(v.style.name() == name, crate::i18n::t(label)).clicked() {
                            chosen = Some((id, json!({})));
                        }
                    }
                }
            }
            if chosen.is_some() {
                action = chosen;
                ui.close();
            }
        });
    }
    if let Some((id, params)) = action {
        let _ = app.run(id, params);
    }
}

fn viewcube(app: &mut CadApp, ui: &mut egui::Ui, rect: Rect, v: &View3d, orbit_per_point: f64) {
    let cube = Cube::new(rect, v);
    let resp = ui.interact(cube.bounds, ui.scope_id().with("viewcube3d"), Sense::click_and_drag());
    let hover = resp.hover_pos();
    let on_home = hover.is_some_and(|p| cube.home.contains(p));
    let spot = if on_home { None } else { hover.and_then(|p| cube.spot_at(p)) };
    let p = ui.painter_at(cube.bounds);

    // The compass ring on the XY plane under the cube, and its letters.
    let ring: Vec<Pos2> = (0..=64)
        .map(|k| {
            let a = f64::from(k) / 64.0 * std::f64::consts::TAU;
            cube.screen(Vec3::new(1.9 * a.cos(), 1.9 * a.sin(), -1.0))
        })
        .collect();
    p.add(Shape::line(ring, Stroke::new(7.0, Color32::from_rgb(0x48, 0x50, 0x5c))));
    let lc = Color32::from_rgb(0xc8, 0xcc, 0xd2);
    for (w, l) in
        [(Vec3::new(0.0, 1.9, -1.0), "N"), (Vec3::new(0.0, -1.9, -1.0), "S"), (Vec3::new(1.9, 0.0, -1.0), "E"), (Vec3::new(-1.9, 0.0, -1.0), "W")]
    {
        // A letter behind the cube stays hidden.
        if cube.spot_at(cube.screen(w)).is_none() {
            p.text(cube.screen(w), egui::Align2::CENTER_CENTER, crate::i18n::t(l), egui::FontId::proportional(12.0), lc);
        }
    }

    // The faces turned toward the viewer, with their labels.
    let face_fill = Color32::from_rgb(0xd6, 0xd9, 0xdd);
    let edge = Stroke::new(1.0, Color32::from_rgb(0x6c, 0x70, 0x76));
    for (_, (n, u, w, label)) in cube.visible() {
        let quad = [cube.screen(*n - *u - *w), cube.screen(*n + *u - *w), cube.screen(*n + *u + *w), cube.screen(*n - *u + *w)];
        // Lit from above and the south-west, so the faces read apart from any side.
        let lit = (0.78 + 0.16 * n.z - 0.06 * n.x - 0.04 * n.y) as f32;
        let fill =
            Color32::from_rgb((f32::from(face_fill.r()) * lit) as u8, (f32::from(face_fill.g()) * lit) as u8, (f32::from(face_fill.b()) * lit) as u8);
        p.add(Shape::convex_polygon(quad.to_vec(), fill, edge));
        if -n.dot(v.dir) > 0.25 {
            p.text(
                cube.screen(*n),
                egui::Align2::CENTER_CENTER,
                crate::i18n::t(label),
                egui::FontId::proportional(9.0),
                Color32::from_rgb(0x40, 0x44, 0x4a),
            );
        }
    }
    if let Some(quad) = spot.and_then(|s| cube.spot_quad(s)) {
        p.add(Shape::convex_polygon(quad.to_vec(), Color32::from_rgba_unmultiplied(0x4d, 0xa3, 0xff, 110), Stroke::NONE));
    }
    crate::viewcube::paint_home(&p, cube.home, on_home);

    let tip = if on_home { Some("Home") } else { spot.map(|_| "Click to view from here; drag to orbit") };
    let resp = match tip {
        Some(tip) => resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(crate::i18n::t(tip)),
        None => resp,
    };
    let mut action: Option<(&'static str, Value)> = None;
    if resp.clicked() {
        let at = resp.interact_pointer_pos();
        if at.is_some_and(|q| cube.home.contains(q)) {
            action = Some(("vpoint.seiso", json!({})));
        } else if let Some(vp) = at.and_then(|q| cube.spot_at(q)).and_then(Cube::viewpoint) {
            action = Some(("vpoint", json!({ "point": [vp.x, vp.y, vp.z] })));
        }
    }
    if resp.dragged() {
        let d = resp.drag_delta();
        if let Some(o) = v.orbited(-f64::from(d.x) * orbit_per_point, f64::from(d.y) * orbit_per_point) {
            app.session.set_view3d(Some(o));
            ui.ctx().request_repaint();
        }
    }
    resp.context_menu(|ui| {
        if ui.button(crate::tl!("Home")).clicked() {
            action = Some(("vpoint.seiso", json!({})));
            ui.close();
        }
        if ui.button(crate::tl!("Plan View")).clicked() {
            action = Some(("plan", json!({})));
            ui.close();
        }
        if ui.button(crate::tl!("Hide ViewCube")).clicked() {
            action = Some(("ui.toggle.viewcube", json!({ "on": false })));
            ui.close();
        }
    });
    if let Some((id, params)) = action {
        let _ = app.run(id, params);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use draftcraft_engine::view3d::VisualStyle;
    use draftcraft_geom::Vec2;

    fn iso() -> View3d {
        View3d::new(Vec3::new(-1.0, 1.0, -1.0), Vec2::new(0.0, 0.0), 10.0, VisualStyle::Realistic).unwrap()
    }

    #[test]
    fn the_cube_turns_with_the_view_and_its_spots_name_26_views() {
        let canvas = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0));
        let cube = Cube::new(canvas, &iso());
        // From the SE iso view: top, front and right faces show.
        let shown: Vec<&str> = cube.visible().map(|(_, f)| f.3).collect();
        assert_eq!(shown, ["TOP", "FRONT", "RIGHT"]);
        // The middle of the top face is the top view; its corner toward the viewer is SE iso.
        let top = cube.spot_at(cube.screen(Vec3::new(0.0, 0.0, 1.0))).unwrap();
        assert_eq!(Cube::viewpoint(top), Some(Vec3::new(0.0, 0.0, 1.0)));
        let corner = cube.spot_at(cube.screen(Vec3::new(0.9, -0.9, 1.0))).unwrap();
        assert_eq!(Cube::viewpoint(corner), Some(Vec3::new(1.0, -1.0, 1.0)));
        // An edge spot: between the front and top faces.
        let edge = cube.spot_at(cube.screen(Vec3::new(0.0, -0.9, 1.0))).unwrap();
        assert_eq!(Cube::viewpoint(edge), Some(Vec3::new(0.0, -1.0, 1.0)));
        // Far from the cube: nothing.
        assert!(cube.spot_at(pos2(10.0, 700.0)).is_none());
    }

    #[test]
    fn every_spot_is_a_view_vpoint_takes() {
        let mut views = std::collections::HashSet::new();
        for face in 0..6 {
            for a in [-0.9, 0.0, 0.9] {
                for b in [-0.9, 0.0, 0.9] {
                    let p = Cube::viewpoint(Spot { face, a, b }).unwrap();
                    assert!(p.len() > 0.5);
                    views.insert([p.x as i32, p.y as i32, p.z as i32]);
                }
            }
        }
        assert_eq!(views.len(), 26, "6 faces, 12 edges and 8 corners");
    }
}
