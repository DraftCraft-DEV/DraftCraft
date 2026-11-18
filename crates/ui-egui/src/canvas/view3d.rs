//! The drawing area in a 3D view (VPOINT, 3DORBIT, VSCURRENT; docs/solids.md, S4).
//!
//! Model space is projected orthographically along the engine's [`View3d`] and painted with
//! egui's painter: 2D objects lie on the XY plane, solids are drawn from their look (edges and
//! triangle mesh). Wireframe draws every edge; the other styles fill faces back to front and draw
//! the edges and silhouettes a viewer sees, tested against the faces in front of them.
//!
//! Input here: the wheel zooms, the middle button pans, Shift + middle button orbits (and the
//! left button while 3DORBIT runs). A click picks the front-most solid (or a 2D object) for a
//! selection prompt or the selection, and a point on the XY plane for a point prompt.

use std::collections::HashMap;
use std::sync::Arc;

use draftcraft_color::Rgb;
use draftcraft_doc::{Drawing, EntityKind, EntityStore, Handle, Insert, MAX_BLOCK_DEPTH, Solid3d, Space};
use draftcraft_engine::Input;
use draftcraft_engine::view3d::{View3d, VisualStyle};
use draftcraft_geom::{Mat3, Mat4, Vec2, Vec3};
use draftcraft_render::Kind;
use draftcraft_render::wire3d::{Placement, Wire3d};
use egui::{Color32, Mesh, Pos2, Rect, Shape, Stroke, pos2};

use crate::CadApp;
use crate::theme::Tokens;

/// The most 2D segments, solid edge segments and triangles a scene keeps (the screen stays
/// responsive; past them the rest is left out).
const MAX_SEGMENTS: usize = 400_000;
const MAX_TRIANGLES: usize = 300_000;
/// Screen grid cell for the visibility test, in points.
const CELL: f32 = 24.0;
/// Degrees of orbit per point of mouse movement.
const ORBIT_PER_POINT: f64 = 0.4;

/// A triangle of a solid, in the drawing.
#[derive(Clone, Copy)]
struct Tri {
    p: [Vec3; 3],
    /// Outward unit normal (zero for a degenerate triangle).
    n: Vec3,
    solid: u32,
}

/// What a solid draws in: its colour, whether colour 7 follows the background.
#[derive(Clone, Copy)]
struct SolidInfo {
    handle: Handle,
    color: Rgb,
    aci7: bool,
}

/// A line segment in the drawing: from a 2D object (`solid == None`) or a solid's edge.
#[derive(Clone, Copy)]
struct Seg {
    a: Vec3,
    b: Vec3,
    color: Rgb,
    aci7: bool,
    solid: Option<u32>,
    handle: Handle,
}

/// Model space ready to project: built once per drawing revision.
pub(crate) struct Scene {
    key: (u64, u64),
    segs: Vec<Seg>,
    /// 2D fills (hatches, solids, wide polylines, outline text) on the XY plane.
    fills: Vec<([Vec3; 3], Rgb, bool)>,
    tris: Vec<Tri>,
    solids: Vec<SolidInfo>,
    /// Mesh edges inside one face of a solid, with the two triangles beside them: drawn as a
    /// silhouette when one triangle faces the viewer and the other does not.
    smooth: Vec<(Vec3, Vec3, u32, u32)>,
}

fn rgb(c: Color32) -> Rgb {
    Rgb(c.r(), c.g(), c.b())
}

fn display(c: Rgb, aci7: bool, bg: Rgb) -> Rgb {
    if !aci7 {
        return c;
    }
    let light = u32::from(bg.0) * 299 + u32::from(bg.1) * 587 + u32::from(bg.2) * 114 > 128_000;
    if light { Rgb(0, 0, 0) } else { Rgb(255, 255, 255) }
}

/// Most block references (array cells included) followed to find the solids inside blocks.
const MAX_INSERT_VISITS: usize = 100_000;

/// A block reference's 3D transform for one cell of its array: its plan transform, with its
/// insertion height and Z scale.
fn insert_matrix(ins: &Insert, base: Vec3, col: u32, row: u32) -> Mat4 {
    let (dx, dy) = (f64::from(col) * ins.col_spacing, f64::from(row) * ins.row_spacing);
    let (s, c) = ins.rotation.sin_cos();
    let at = ins.insert.xy() + Vec2::new(dx * c - dy * s, dx * s + dy * c);
    let t = Mat3::translate(at)
        .then_before(Mat3::rotate(ins.rotation))
        .then_before(Mat3::scale(ins.scale.x, ins.scale.y))
        .then_before(Mat3::translate(-base.xy()));
    let mut m = Mat4::from_mat3(&t);
    m.m[2][2] = ins.scale.z;
    m.m[2][3] = ins.insert.z - ins.scale.z * base.z;
    m
}

/// Whether block `name` holds 3D solids, itself or in the blocks it references.
fn block_has_solids(d: &Drawing, name: &str, depth: usize, memo: &mut HashMap<String, bool>) -> bool {
    let key = name.to_ascii_lowercase();
    if let Some(v) = memo.get(&key) {
        return *v;
    }
    // Cut a cycle short while it is being looked at.
    memo.insert(key.clone(), false);
    let Some(b) = d.block(name) else { return false };
    let has = depth < MAX_BLOCK_DEPTH
        && b.entities.iter().any(|e| match &e.kind {
            EntityKind::Solid3d(_) => true,
            EntityKind::Insert(i) => block_has_solids(d, &i.block, depth + 1, memo),
            _ => false,
        });
    memo.insert(key, has);
    has
}

/// The solids of `st` placed by `m`, each with the handle that selects it: its own at the top,
/// the outermost block reference's inside blocks (nested to [`MAX_BLOCK_DEPTH`]).
#[allow(clippy::too_many_arguments)]
fn collect_solids(
    d: &Drawing,
    st: &EntityStore,
    m: Option<Mat4>,
    top: Option<Handle>,
    depth: usize,
    visits: &mut usize,
    memo: &mut HashMap<String, bool>,
    out: &mut Vec<(Handle, Solid3d)>,
) {
    for e in st.iter() {
        match &e.kind {
            EntityKind::Solid3d(so) => {
                let mut so = so.clone();
                if let Some(m) = &m {
                    so.placement = m.mul(&so.placement);
                }
                out.push((top.unwrap_or(e.handle), so));
            }
            EntityKind::Insert(ins) if depth < MAX_BLOCK_DEPTH && block_has_solids(d, &ins.block, depth, memo) => {
                let Some(b) = d.block(&ins.block) else { continue };
                for row in 0..ins.rows.max(1) {
                    for col in 0..ins.cols.max(1) {
                        *visits += 1;
                        if *visits > MAX_INSERT_VISITS {
                            return;
                        }
                        let here = insert_matrix(ins, b.base, col, row);
                        let m = m.as_ref().map_or(here, |outer| outer.mul(&here));
                        collect_solids(d, &b.entities, Some(m), Some(top.unwrap_or(e.handle)), depth + 1, visits, memo, out);
                    }
                }
            }
            _ => {}
        }
    }
}

/// `d` without the solids inside its blocks, for the flat part of the 3D view (those solids are
/// drawn in 3D); `None` when no block holds one.
fn without_block_solids(d: &Drawing) -> Option<Drawing> {
    let names: Vec<String> =
        d.blocks.iter().filter(|(_, b)| b.entities.iter().any(|e| matches!(e.kind, EntityKind::Solid3d(_)))).map(|(k, _)| k.clone()).collect();
    if names.is_empty() {
        return None;
    }
    let mut c = d.clone();
    for n in names {
        if let Some(b) = c.blocks.get_mut(&n) {
            let hs: Vec<Handle> = b.entities.iter().filter(|e| matches!(e.kind, EntityKind::Solid3d(_))).map(|e| e.handle).collect();
            let b = Arc::make_mut(b);
            for h in hs {
                b.entities.remove(h);
            }
        }
    }
    Some(c)
}

impl Scene {
    fn build(d: &Drawing, key: (u64, u64)) -> Scene {
        let flat = without_block_solids(d);
        let list = draftcraft_render::build(flat.as_ref().unwrap_or(d), &Space::Model, &draftcraft_render::Options::default());
        let mut colors: HashMap<Handle, (Rgb, bool)> = HashMap::new();
        for p in &list.prims {
            colors.entry(p.handle).or_insert((p.color, p.aci7));
        }
        let mut scene = Scene { key, segs: Vec::new(), fills: Vec::new(), tris: Vec::new(), solids: Vec::new(), smooth: Vec::new() };
        let mut solid_handles = std::collections::HashSet::new();
        let mut placed: Vec<(Handle, Solid3d)> = Vec::new();
        if let Some(st) = d.space(&Space::Model) {
            let (mut visits, mut memo) = (0usize, HashMap::new());
            collect_solids(d, st, None, None, 0, &mut visits, &mut memo, &mut placed);
        }
        {
            for (handle, so) in &placed {
                let handle = *handle;
                if d.space(&Space::Model).and_then(|st| st.get(handle)).is_some_and(|e| matches!(e.kind, EntityKind::Solid3d(_))) {
                    solid_handles.insert(handle);
                }
                let Ok(index) = u32::try_from(scene.solids.len()) else { break };
                let (color, aci7) = colors.get(&handle).copied().unwrap_or((Rgb(255, 255, 255), true));
                scene.solids.push(SolidInfo { handle, color, aci7 });
                for wire in &so.look.wires {
                    for w in wire.windows(2) {
                        if let [a, b] = w
                            && scene.segs.len() < MAX_SEGMENTS
                        {
                            scene.segs.push(Seg { a: so.world(*a), b: so.world(*b), color, aci7, solid: Some(index), handle });
                        }
                    }
                }
                // Weld corners by position, so triangles of one face find their neighbours even
                // when the mesh repeats corners.
                let mut edges: HashMap<([i64; 3], [i64; 3]), (u32, u32)> = HashMap::new();
                let quant = |p: Vec3| {
                    let q = |v: f64| (v * 1e7).round() as i64;
                    [q(p.x), q(p.y), q(p.z)]
                };
                for (t, face) in so.look.triangles.iter().zip(so.look.tri_face.iter().chain(std::iter::repeat(&u32::MAX))) {
                    if scene.tris.len() >= MAX_TRIANGLES {
                        break;
                    }
                    let Some(local) = so.look.triangle(t) else { continue };
                    // Wound to face out, a mirrored solid too.
                    let p = so.world_triangle(local);
                    let n = (p[1] - p[0]).cross(p[2] - p[0]).normalized();
                    let Ok(ti) = u32::try_from(scene.tris.len()) else { break };
                    scene.tris.push(Tri { p, n, solid: index });
                    for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                        let (a, b) = (quant(p[i]), quant(p[j]));
                        let k = if a < b { (a, b) } else { (b, a) };
                        let entry = edges.entry(k).or_insert((ti, *face));
                        if entry.0 != ti && entry.1 == *face {
                            scene.smooth.push((p[i], p[j], entry.0, ti));
                        }
                    }
                }
            }
        }
        // Objects off the XY plane: lifted to their height, or drawn from their own edges and
        // faces (a 3D face or a mesh hides what is behind it, like a solid's faces).
        let mut heights: HashMap<Handle, f64> = HashMap::new();
        let mut spatial: std::collections::HashSet<Handle> = std::collections::HashSet::new();
        if let Some(st) = d.space(&Space::Model) {
            let mut visits = 0usize;
            for e in st.iter() {
                if matches!(e.kind, EntityKind::Solid3d(_)) {
                    continue;
                }
                match place(d, e, 0, &mut visits) {
                    Placement::Flat => {}
                    Placement::Lifted(z) => {
                        heights.insert(e.handle, z);
                    }
                    Placement::Spatial(w) => {
                        spatial.insert(e.handle);
                        let (color, aci7) = colors.get(&e.handle).copied().unwrap_or((Rgb(255, 255, 255), true));
                        scene.add_wire(e.handle, &w, color, aci7);
                    }
                }
            }
        }
        for p in &list.prims {
            if solid_handles.contains(&p.handle) || spatial.contains(&p.handle) {
                continue;
            }
            let z = heights.get(&p.handle).copied().unwrap_or(0.0);
            let start = usize::try_from(p.start).unwrap_or(usize::MAX);
            let len = usize::try_from(p.len).unwrap_or(0);
            let end = start.saturating_add(len);
            match p.kind {
                Kind::Polyline => {
                    let Some(pts) = list.verts.get(start..end) else { continue };
                    for w in pts.windows(2) {
                        if let [a, b] = w
                            && scene.segs.len() < MAX_SEGMENTS
                        {
                            scene.segs.push(Seg { a: a.to3(z), b: b.to3(z), color: p.color, aci7: p.aci7, solid: None, handle: p.handle });
                        }
                    }
                }
                Kind::Tris => {
                    let Some(pts) = list.tris.get(start..end) else { continue };
                    for [a, b, cc] in pts.as_chunks::<3>().0 {
                        if scene.fills.len() < MAX_TRIANGLES {
                            scene.fills.push(([a.to3(z), b.to3(z), cc.to3(z)], p.color, p.aci7));
                        }
                    }
                }
                _ => {}
            }
        }
        scene
    }
}

impl Scene {
    /// An object drawn from its own edges and faces. With faces it draws like a solid (shaded,
    /// hiding what is behind it); edges alone draw like 2D objects.
    fn add_wire(&mut self, handle: Handle, w: &Wire3d, color: Rgb, aci7: bool) {
        let index = if w.faces.is_empty() {
            None
        } else {
            let Ok(i) = u32::try_from(self.solids.len()) else { return };
            self.solids.push(SolidInfo { handle, color, aci7 });
            Some(i)
        };
        for line in &w.lines {
            for pair in line.windows(2) {
                if let [a, b] = pair
                    && self.segs.len() < MAX_SEGMENTS
                {
                    self.segs.push(Seg { a: *a, b: *b, color, aci7, solid: index, handle });
                }
            }
        }
        if let Some(solid) = index {
            for p in &w.faces {
                if self.tris.len() >= MAX_TRIANGLES {
                    break;
                }
                let n = (p[1] - p[0]).cross(p[2] - p[0]).normalized();
                self.tris.push(Tri { p: *p, n, solid });
            }
        }
    }
}

/// How an object of model space is drawn in 3D. A block reference is lifted to its height when
/// everything in its block lies on the block's XY plane, drawn in 3D when everything in it can
/// be, and left on the XY plane otherwise (its plan drawing keeps its text and fills).
fn place(d: &Drawing, e: &draftcraft_doc::Entity, depth: usize, visits: &mut usize) -> Placement {
    let EntityKind::Insert(ins) = &e.kind else { return draftcraft_render::wire3d::placement(e) };
    let Some((flat, wire)) = block_3d(d, ins, depth, visits) else { return Placement::Flat };
    if flat {
        let z = ins.insert.z;
        return if z.is_finite() && z != 0.0 && ins.attribs.is_empty() { Placement::Lifted(z) } else { Placement::Flat };
    }
    match wire {
        Some(w) if ins.attribs.is_empty() => Placement::Spatial(w),
        _ => Placement::Flat,
    }
}

/// A block reference's contents in 3D: whether all of it lies on its block's XY plane, and its
/// edges and faces placed in the drawing when every object in it can be drawn that way.
fn block_3d(d: &Drawing, ins: &Insert, depth: usize, visits: &mut usize) -> Option<(bool, Option<Wire3d>)> {
    let b = d.block(&ins.block)?;
    if depth >= MAX_BLOCK_DEPTH || !ins.scale.z.is_finite() {
        return None;
    }
    let mut flat = true;
    let mut parts: Option<Wire3d> = Some(Wire3d::default());
    for child in b.entities.iter() {
        *visits += 1;
        if *visits > MAX_INSERT_VISITS {
            return None;
        }
        let (child_flat, child_wire) = match &child.kind {
            // Solids in blocks are drawn with the solids; attribute definitions don't show.
            EntityKind::Solid3d(_) | EntityKind::AttDef(_) => continue,
            EntityKind::Insert(inner) => {
                let (f, w) = block_3d(d, inner, depth + 1, visits)?;
                (f && inner.insert.z == 0.0, w)
            }
            _ => {
                let f = matches!(draftcraft_render::wire3d::placement(child), Placement::Flat);
                (f, draftcraft_render::wire3d::wire(child))
            }
        };
        flat &= child_flat;
        parts = match (parts, child_wire) {
            (Some(mut all), Some(w)) => {
                all.lines.extend(w.lines);
                all.faces.extend(w.faces);
                Some(all)
            }
            _ => None,
        };
    }
    let cells = usize::try_from(ins.rows.max(1)).unwrap_or(usize::MAX).saturating_mul(usize::try_from(ins.cols.max(1)).unwrap_or(usize::MAX));
    *visits = visits.saturating_add(cells);
    if *visits > MAX_INSERT_VISITS {
        return None;
    }
    let placed = parts.map(|w| {
        let mut out = Wire3d::default();
        for row in 0..ins.rows.max(1) {
            for col in 0..ins.cols.max(1) {
                let m = insert_matrix(ins, b.base, col, row);
                out.lines.extend(w.lines.iter().map(|l| l.iter().map(|p| m.apply(*p)).collect()));
                out.faces.extend(w.faces.iter().map(|f| f.map(|p| m.apply(p))));
            }
        }
        out
    });
    Some((flat, placed))
}

/// The scene for the current drawing, rebuilt when it changes.
fn scene(app: &CadApp, ctx: &egui::Context) -> Option<Arc<Scene>> {
    let st = app.session.state().ok()?;
    let key = (st.uid, st.revision);
    let id = egui::Id::unique("draftcraft-scene3d");
    if let Some(s) = ctx.data(|d| d.get_temp::<Arc<Scene>>(id)).filter(|s| s.key == key) {
        return Some(s);
    }
    let s = Arc::new(Scene::build(&st.doc, key));
    ctx.data_mut(|d| d.insert_temp(id, s.clone()));
    Some(s)
}

/// Screen mapping of a 3D view in `rect`.
#[derive(Clone, Copy)]
struct Proj {
    x: Vec3,
    y: Vec3,
    dir: Vec3,
    center: Vec2,
    scale: f64,
    mid: Pos2,
}

impl Proj {
    fn new(v: &View3d, rect: Rect) -> Proj {
        let (x, y) = v.axes();
        Proj { x, y, dir: v.dir, center: v.center, scale: f64::from(rect.height()) / v.height.max(1e-12), mid: rect.center() }
    }
    fn screen(&self, p: Vec3) -> Pos2 {
        let q = Vec2::new(p.dot(self.x), p.dot(self.y)) - self.center;
        pos2(self.mid.x + (q.x * self.scale) as f32, self.mid.y - (q.y * self.scale) as f32)
    }
    fn depth(&self, p: Vec3) -> f64 {
        p.dot(self.dir)
    }
    /// The view-plane point at screen position `s`.
    fn plane(&self, s: Pos2) -> Vec2 {
        self.center + Vec2::new(f64::from(s.x - self.mid.x), f64::from(self.mid.y - s.y)) * (1.0 / self.scale)
    }
}

/// Front-facing solid triangles binned by screen cell, for "is this point hidden?".
struct Occluders {
    min: Pos2,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
    screen: Vec<[Pos2; 3]>,
    depth: Vec<[f64; 3]>,
    eps: f64,
}

impl Occluders {
    fn new(scene: &Scene, pr: &Proj, rect: Rect, eps: f64) -> Occluders {
        let cols = ((rect.width() / CELL).ceil().max(1.0) as usize).min(512);
        let rows = ((rect.height() / CELL).ceil().max(1.0) as usize).min(512);
        let mut o = Occluders { min: rect.min, cols, rows, cells: vec![Vec::new(); cols * rows], screen: Vec::new(), depth: Vec::new(), eps };
        for t in &scene.tris {
            let s = t.p.map(|p| pr.screen(p));
            let d = t.p.map(|p| pr.depth(p));
            let Ok(i) = u32::try_from(o.screen.len()) else { break };
            o.screen.push(s);
            o.depth.push(d);
            if t.n.dot(pr.dir) >= 0.0 {
                continue; // facing away: a closed solid's front faces hide what it hides
            }
            let (lo, hi) =
                (s.iter().fold(Pos2::new(f32::MAX, f32::MAX), |a, p| a.min(*p)), s.iter().fold(Pos2::new(f32::MIN, f32::MIN), |a, p| a.max(*p)));
            let Some((c0, r0, c1, r1)) = o.cells_of(lo, hi) else { continue };
            for r in r0..=r1 {
                for c in c0..=c1 {
                    if let Some(cell) = o.cells.get_mut(r * cols + c) {
                        cell.push(i);
                    }
                }
            }
        }
        o
    }
    /// The cell of screen point `p`; `None` outside the grid (off the view) or for a point that
    /// is no point.
    fn cell(&self, p: Pos2) -> Option<(usize, usize)> {
        if !(p.x.is_finite() && p.y.is_finite()) {
            return None;
        }
        let (c, r) = ((p.x - self.min.x) / CELL, (p.y - self.min.y) / CELL);
        if c < 0.0 || r < 0.0 || c >= self.cols as f32 || r >= self.rows as f32 {
            return None;
        }
        Some((c.floor() as usize, r.floor() as usize))
    }
    /// The cells a screen box from `lo` to `hi` covers, cut to the grid; `None` when it lies
    /// wholly off the view (such a triangle hides nothing on screen).
    fn cells_of(&self, lo: Pos2, hi: Pos2) -> Option<(usize, usize, usize, usize)> {
        if !(lo.x.is_finite() && lo.y.is_finite() && hi.x.is_finite() && hi.y.is_finite()) {
            return None;
        }
        let (c0, r0) = ((lo.x - self.min.x) / CELL, (lo.y - self.min.y) / CELL);
        let (c1, r1) = ((hi.x - self.min.x) / CELL, (hi.y - self.min.y) / CELL);
        let (cols, rows) = (self.cols as f32, self.rows as f32);
        if c1 < 0.0 || r1 < 0.0 || c0 >= cols || r0 >= rows {
            return None;
        }
        let at = |v: f32, n: f32| v.floor().clamp(0.0, n - 1.0) as usize;
        Some((at(c0, cols), at(r0, rows), at(c1, cols), at(r1, rows)))
    }
    /// The depth of the nearest front-facing triangle covering screen point `p`, and which.
    fn nearest(&self, p: Pos2) -> Option<(f64, u32)> {
        let (c, r) = self.cell(p)?;
        let mut best: Option<(f64, u32)> = None;
        for &i in self.cells.get(r * self.cols + c)? {
            let (Some(s), Some(d)) = (self.screen.get(i as usize), self.depth.get(i as usize)) else { continue };
            if let Some(z) = depth_in(s, d, p)
                && best.is_none_or(|(b, _)| z < b)
            {
                best = Some((z, i));
            }
        }
        best
    }
    /// Screen point `p` at depth `z` is behind a face.
    fn hidden(&self, p: Pos2, z: f64) -> bool {
        self.nearest(p).is_some_and(|(near, _)| near < z - self.eps)
    }
}

/// The depth at `p` of the screen triangle `s` with corner depths `d`, if `p` is inside it.
fn depth_in(s: &[Pos2; 3], d: &[f64; 3], p: Pos2) -> Option<f64> {
    let [a, b, c] = *s;
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < 1e-9 {
        return None;
    }
    let w0 = ((b.x - p.x) * (c.y - p.y) - (b.y - p.y) * (c.x - p.x)) / area;
    let w1 = ((c.x - p.x) * (a.y - p.y) - (c.y - p.y) * (a.x - p.x)) / area;
    let w2 = 1.0 - w0 - w1;
    let tol = -1e-4;
    (w0 >= tol && w1 >= tol && w2 >= tol).then(|| f64::from(w0) * d[0] + f64::from(w1) * d[1] + f64::from(w2) * d[2])
}

/// A face's colour in `style`, lit from over the viewer's left shoulder.
fn shade(style: VisualStyle, base: Rgb, n: Vec3, pr: &Proj) -> Color32 {
    let light = (pr.dir * -0.75 + pr.y * 0.5 - pr.x * 0.35).normalized();
    let k = n.dot(light).max(0.0);
    let mix = |c: u8, f: f64| (f64::from(c) * f).clamp(0.0, 255.0) as u8;
    match style {
        VisualStyle::ShadesOfGray => {
            let g = (60.0 + 170.0 * k) as u8;
            Color32::from_rgb(g, g, g)
        }
        VisualStyle::Conceptual => {
            // Cool to warm, tinted by the object's colour.
            let t = (1.0 + n.dot(light)) * 0.5;
            let lerp = |cool: f64, warm: f64, c: u8| ((cool + (warm - cool) * t) * 0.6 + f64::from(c) * 0.4).clamp(0.0, 255.0) as u8;
            Color32::from_rgb(lerp(70.0, 235.0, base.0), lerp(90.0, 215.0, base.1), lerp(160.0, 150.0, base.2))
        }
        VisualStyle::XRay => {
            let f = 0.35 + 0.65 * k;
            Color32::from_rgba_unmultiplied(mix(base.0, f), mix(base.1, f), mix(base.2, f), 90)
        }
        _ => {
            let f = 0.3 + 0.7 * k;
            Color32::from_rgb(mix(base.0, f), mix(base.1, f), mix(base.2, f))
        }
    }
}

/// Draw the 3D view and handle its input. Returns false (and does nothing) when the drawing
/// area shows the plan view.
pub(super) fn show(app: &mut CadApp, ui: &mut egui::Ui, rect: Rect, resp: &egui::Response) -> bool {
    let Some(mut v) = app.session.view3d() else { return false };
    let t = Tokens::get();
    let painter = ui.painter_at(rect);

    // ---- input ----
    let (hover, scroll, zoom, mods, middle, primary_down, delta) = ui.input(|i| {
        (
            i.pointer.hover_pos(),
            i.smooth_scroll_delta,
            i.zoom_delta(),
            i.modifiers,
            i.pointer.middle_down(),
            i.pointer.primary_down(),
            i.pointer.delta(),
        )
    });
    let inside = hover.is_some_and(|p| rect.contains(p) && !super::view3d_overlay::covers(app, rect, &v, p));
    let orbiting_cmd = app.session.running.as_ref().is_some_and(|r| r.id == "3dorbit");
    let pr = Proj::new(&v, rect);
    if inside {
        let steps = f64::from(scroll.y) / 50.0;
        let factor = 1.2f64.powf(steps) * f64::from(zoom);
        if (factor - 1.0).abs() > 1e-6
            && let Some(at) = hover
            && let Some(z) = v.zoomed(factor, pr.plane(at))
        {
            v = z;
        }
        let drag = egui::Vec2::new(delta.x, delta.y);
        if drag != egui::Vec2::ZERO {
            if (middle && mods.shift) || (orbiting_cmd && primary_down && resp.dragged()) {
                if let Some(o) = v.orbited(-f64::from(drag.x) * ORBIT_PER_POINT, f64::from(drag.y) * ORBIT_PER_POINT) {
                    v = o;
                }
            } else if middle && let Some(p) = v.panned(Vec2::new(-f64::from(drag.x), f64::from(drag.y)) * (1.0 / pr.scale)) {
                v = p;
            }
        }
    }
    if app.session.view3d() != Some(v) {
        app.session.set_view3d(Some(v));
        ui.ctx().request_repaint();
    }
    let pr = Proj::new(&v, rect);

    // ---- paint ----
    painter.rect_filled(rect, 0.0, t.canvas);
    let Some(scene) = scene(app, ui.ctx()) else { return true };
    let selected: std::collections::HashSet<Handle> = app.session.state().map(|s| s.selection.iter().copied().collect()).unwrap_or_default();
    let highlight = Color32::from_rgb(0x4d, 0xa3, 0xff);
    let key = PaintKey { scene: scene.key, view: v, rect, selection: selection_key(&selected), bg: t.canvas };
    let painted = painted(ui.ctx(), &scene, &key, &pr, &selected, highlight);
    painter.extend(painted.shapes.iter().cloned());
    draw_axes(&painter, &pr, rect, t.text);
    super::view3d_overlay::show(app, ui, rect, &v, ORBIT_PER_POINT);

    // ---- clicks ----
    if inside
        && resp.clicked()
        && !orbiting_cmd
        && let Some(at) = hover
    {
        click(app, &scene, painted.occ.as_deref(), &pr, &v, at);
    }
    true
}

/// What a repaint of the 3D view draws, kept until the drawing, the view, the canvas or the
/// selection changes: sorting the faces, binning them for hidden lines and cutting every edge
/// into visible pieces is done once, not on every frame.
#[derive(Clone, PartialEq)]
struct PaintKey {
    scene: (u64, u64),
    view: View3d,
    rect: Rect,
    selection: u64,
    bg: Color32,
}

#[derive(Clone)]
struct Painted {
    key: PaintKey,
    shapes: Arc<Vec<Shape>>,
    occ: Option<Arc<Occluders>>,
}

fn selection_key(selected: &std::collections::HashSet<Handle>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hs: Vec<&Handle> = selected.iter().collect();
    hs.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    hs.hash(&mut h);
    h.finish()
}

fn painted(
    ctx: &egui::Context,
    scene: &Scene,
    key: &PaintKey,
    pr: &Proj,
    selected: &std::collections::HashSet<Handle>,
    highlight: Color32,
) -> Painted {
    let id = egui::Id::unique("draftcraft-view3d-painted");
    if let Some(p) = ctx.data(|d| d.get_temp::<Painted>(id)).filter(|p| p.key == *key) {
        return p;
    }
    let (shapes, occ) = paint(scene, &key.view, pr, key.rect, selected, highlight, key.bg);
    let p = Painted { key: key.clone(), shapes: Arc::new(shapes), occ: occ.map(Arc::new) };
    ctx.data_mut(|d| d.insert_temp(id, p.clone()));
    p
}

/// The shapes of the 3D view `v` of `scene`, and the faces binned for hidden lines and picking
/// (styles with faces).
fn paint(
    scene: &Scene,
    v: &View3d,
    pr: &Proj,
    rect: Rect,
    selected: &std::collections::HashSet<Handle>,
    highlight: Color32,
    canvas: Color32,
) -> (Vec<Shape>, Option<Occluders>) {
    let bg = rgb(canvas);
    let mut out: Vec<Shape> = Vec::new();
    let size = scene_size(scene);
    let occ = v.style.fills().then(|| Occluders::new(scene, pr, rect, size * 1e-6 + 1e-9));

    // 2D fills lie on the XY plane under the solids: drawn first.
    let mut mesh = Mesh::default();
    for (p, c, aci7) in &scene.fills {
        let c = display(*c, *aci7, bg);
        add_tri(&mut mesh, p.map(|q| pr.screen(q)), Color32::from_rgb(c.0, c.1, c.2));
    }
    out.push(Shape::mesh(mesh));

    if v.style.fills() {
        // Faces back to front.
        let mut order: Vec<(f64, usize)> = scene
            .tris
            .iter()
            .enumerate()
            .filter(|(_, t)| v.style == VisualStyle::XRay || t.n.dot(pr.dir) < 0.0)
            .map(|(i, t)| (-(pr.depth(t.p[0]) + pr.depth(t.p[1]) + pr.depth(t.p[2])), i))
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut mesh = Mesh::default();
        for (_, i) in order {
            let Some(tri) = scene.tris.get(i) else { continue };
            let info = scene.solids.get(tri.solid as usize);
            let base = info.map_or(Rgb(200, 200, 200), |s| display(s.color, s.aci7, bg));
            let base = if v.style.shades() { base } else { bg };
            let col = if v.style.shades() { shade(v.style, base, tri.n, pr) } else { Color32::from_rgb(bg.0, bg.1, bg.2) };
            add_tri(&mut mesh, tri.p.map(|q| pr.screen(q)), col);
            if mesh.vertices.len() > 60_000 {
                out.push(Shape::mesh(std::mem::take(&mut mesh)));
            }
        }
        out.push(Shape::mesh(mesh));
    }

    // Edges, 2D objects and silhouettes, the hidden parts left out.
    let mut shapes = Vec::new();
    let edge_color = |s: &Seg| -> Color32 {
        if selected.contains(&s.handle) {
            return highlight;
        }
        let c = display(s.color, s.aci7, bg);
        if s.solid.is_some() && v.style.shades() && v.style != VisualStyle::XRay {
            // Edges over lit faces: darker than the face.
            Color32::from_rgb(c.0 / 3, c.1 / 3, c.2 / 3)
        } else {
            Color32::from_rgb(c.0, c.1, c.2)
        }
    };
    for s in &scene.segs {
        if s.solid.is_some() && !v.style.edges() && !selected.contains(&s.handle) {
            continue;
        }
        visible_pieces(pr, occ.as_ref(), s.a, s.b, Stroke::new(1.0, edge_color(s)), &mut shapes);
    }
    if v.style.fills() && v.style.edges() {
        for (a, b, t1, t2) in &scene.smooth {
            let (Some(x), Some(y)) = (scene.tris.get(*t1 as usize), scene.tris.get(*t2 as usize)) else { continue };
            if (x.n.dot(pr.dir) < 0.0) == (y.n.dot(pr.dir) < 0.0) {
                continue;
            }
            let Some(info) = scene.solids.get(x.solid as usize) else { continue };
            let seg = Seg { a: *a, b: *b, color: info.color, aci7: info.aci7, solid: Some(x.solid), handle: info.handle };
            visible_pieces(pr, occ.as_ref(), *a, *b, Stroke::new(1.0, edge_color(&seg)), &mut shapes);
        }
    } else if v.style == VisualStyle::Wireframe {
        // Wireframe: curved faces show their silhouettes too.
        for (a, b, t1, t2) in &scene.smooth {
            let (Some(x), Some(y)) = (scene.tris.get(*t1 as usize), scene.tris.get(*t2 as usize)) else { continue };
            if (x.n.dot(pr.dir) < 0.0) != (y.n.dot(pr.dir) < 0.0) {
                let info = scene.solids.get(x.solid as usize);
                let c = info.map_or(Rgb(255, 255, 255), |s| display(s.color, s.aci7, bg));
                let c = if info.is_some_and(|s| selected.contains(&s.handle)) { highlight } else { Color32::from_rgb(c.0, c.1, c.2) };
                shapes.push(Shape::line_segment([pr.screen(*a), pr.screen(*b)], Stroke::new(1.0, c)));
            }
        }
    }
    out.extend(shapes);
    (out, occ)
}

fn add_tri(mesh: &mut Mesh, p: [Pos2; 3], c: Color32) {
    if p.iter().any(|q| !(q.x.is_finite() && q.y.is_finite())) {
        return;
    }
    let Ok(base) = u32::try_from(mesh.vertices.len()) else { return };
    for q in p {
        mesh.colored_vertex(q, c);
    }
    mesh.add_triangle(base, base + 1, base + 2);
}

/// How big the scene is (for depth tolerances).
fn scene_size(scene: &Scene) -> f64 {
    let mut m = 0.0f64;
    for t in &scene.tris {
        for p in t.p {
            m = m.max(p.x.abs()).max(p.y.abs()).max(p.z.abs());
        }
    }
    if m.is_finite() && m > 0.0 { m } else { 1.0 }
}

/// Segment `a`–`b` in pieces about four points long, keeping those not behind a face.
fn visible_pieces(pr: &Proj, occ: Option<&Occluders>, a: Vec3, b: Vec3, stroke: Stroke, out: &mut Vec<Shape>) {
    let (sa, sb) = (pr.screen(a), pr.screen(b));
    if !(sa.x.is_finite() && sa.y.is_finite() && sb.x.is_finite() && sb.y.is_finite()) {
        return;
    }
    let Some(occ) = occ else {
        out.push(Shape::line_segment([sa, sb], stroke));
        return;
    };
    let len = sa.distance(sb);
    let n = ((len / 4.0).ceil() as usize).clamp(1, 64);
    let mut run: Option<Pos2> = None;
    let mut prev = sa;
    for k in 0..n {
        let t1 = (k + 1) as f64 / n as f64;
        let tm = (k as f64 + 0.5) / n as f64;
        let mid = a + (b - a) * tm;
        let end = if k + 1 == n { sb } else { pr.screen(a + (b - a) * t1) };
        let seen = !occ.hidden(pr.screen(mid), pr.depth(mid));
        match (seen, run) {
            (true, None) => run = Some(prev),
            (false, Some(start)) => {
                out.push(Shape::line_segment([start, prev], stroke));
                run = None;
            }
            _ => {}
        }
        prev = end;
    }
    if let Some(start) = run {
        out.push(Shape::line_segment([start, prev], stroke));
    }
}

/// The drawing's X, Y and Z axes in the lower left corner.
fn draw_axes(p: &egui::Painter, pr: &Proj, rect: Rect, text: Color32) {
    let o = rect.left_bottom() + egui::vec2(36.0, -36.0);
    for (axis, name, col) in [
        (Vec3::X, "X", Color32::from_rgb(0xe0, 0x50, 0x50)),
        (Vec3::Y, "Y", Color32::from_rgb(0x50, 0xc0, 0x60)),
        (Vec3::Z, "Z", Color32::from_rgb(0x50, 0x80, 0xe0)),
    ] {
        let d = egui::vec2(axis.dot(pr.x) as f32, -(axis.dot(pr.y) as f32)) * 24.0;
        p.line_segment([o, o + d], Stroke::new(2.0, col));
        p.text(o + d * 1.25, egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(10.0), text);
    }
}

/// A click at screen point `at`: a pick for a selection prompt or the selection, a point on the
/// XY plane for a point prompt.
fn click(app: &mut CadApp, scene: &Scene, occ: Option<&Occluders>, pr: &Proj, v: &View3d, at: Pos2) {
    let hit = pick(scene, occ, pr, at);
    let prompt = app.session.current_prompt();
    let result = match prompt {
        Some(p) if p.accept.select => match hit {
            Some(h) => app.session.input(Input::Pick(vec![h])),
            None => Ok(()),
        },
        Some(p) if p.accept.point => match v.on_xy(pr.plane(at)) {
            Some(w) => app.session.input(Input::Point(w)),
            None => {
                app.session.echo("This view looks along the XY plane: type the point, or pick it in a view from above (VPOINT, PLAN)".to_string());
                Ok(())
            }
        },
        Some(_) => Ok(()),
        None => {
            match hit {
                Some(h) => {
                    let mut sel = app.session.state().map(|s| s.selection.clone()).unwrap_or_default();
                    if let Some(i) = sel.iter().position(|x| *x == h) {
                        sel.remove(i);
                    } else {
                        sel.push(h);
                    }
                    app.session.set_selection(sel);
                }
                None => app.session.set_selection(Vec::new()),
            }
            Ok(())
        }
    };
    if let Err(e) = result {
        app.session.echo(e.to_string());
    }
}

/// The object under screen point `at`: the front-most solid face, else the nearest 2D object or
/// solid edge within a few points.
fn pick(scene: &Scene, occ: Option<&Occluders>, pr: &Proj, at: Pos2) -> Option<Handle> {
    let face = match occ {
        Some(o) => o.nearest(at).and_then(|(_, i)| scene.tris.get(i as usize)).map(|t| t.solid),
        None => {
            // Wireframe: any face under the cursor, nearest first.
            let mut best: Option<(f64, u32)> = None;
            for t in &scene.tris {
                let s = t.p.map(|p| pr.screen(p));
                let d = t.p.map(|p| pr.depth(p));
                if let Some(z) = depth_in(&s, &d, at)
                    && best.is_none_or(|(b, _)| z < b)
                {
                    best = Some((z, t.solid));
                }
            }
            best.map(|(_, s)| s)
        }
    };
    if let Some(s) = face.and_then(|i| scene.solids.get(i as usize)) {
        return Some(s.handle);
    }
    let mut best: Option<(f32, Handle)> = None;
    for s in &scene.segs {
        let d = dist_to_segment(at, pr.screen(s.a), pr.screen(s.b));
        if d <= 5.0 && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, s.handle));
        }
    }
    best.map(|(_, h)| h)
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_sq();
    if l2.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_inside_a_screen_triangle_gets_its_depth() {
        let s = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(0.0, 10.0)];
        let d = [1.0, 2.0, 3.0];
        let z = depth_in(&s, &d, pos2(0.0, 0.0)).unwrap();
        assert!((z - 1.0).abs() < 1e-6);
        assert!(depth_in(&s, &d, pos2(9.0, 9.0)).is_none());
        let flat = [pos2(0.0, 0.0), pos2(1.0, 1.0), pos2(2.0, 2.0)];
        assert!(depth_in(&flat, &d, pos2(1.0, 1.0)).is_none());
    }

    #[test]
    fn colour_7_follows_the_background() {
        assert_eq!(display(Rgb(1, 2, 3), true, Rgb(0, 0, 0)), Rgb(255, 255, 255));
        assert_eq!(display(Rgb(1, 2, 3), true, Rgb(255, 255, 255)), Rgb(0, 0, 0));
        assert_eq!(display(Rgb(1, 2, 3), false, Rgb(0, 0, 0)), Rgb(1, 2, 3));
    }

    #[test]
    fn a_box_hides_its_back_edges_in_the_hidden_style() {
        let mut s = draftcraft_engine::Session::new();
        s.execute("box", &serde_json::json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap();
        let st = s.state().unwrap();
        let scene = Scene::build(&st.doc, (st.uid, st.revision));
        assert!(!scene.tris.is_empty() && !scene.segs.is_empty());
        let v = View3d::new(Vec3::new(-1.0, 1.0, -1.0), Vec2::new(0.0, 5.0), 40.0, VisualStyle::Hidden).unwrap();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let v = v.fitted(&st.doc, 800.0 / 600.0);
        let pr = Proj::new(&v, rect);
        let occ = Occluders::new(&scene, &pr, rect, 1e-5);
        // The corner farthest from a south-east viewer above is (0, 10, 0): hidden. The nearest,
        // (10, 0, 10), is seen.
        let far = Vec3::new(0.0, 10.0, 0.0);
        let near = Vec3::new(10.0, 0.0, 10.0);
        assert!(occ.hidden(pr.screen(far), pr.depth(far)));
        assert!(!occ.hidden(pr.screen(near), pr.depth(near)));
        // Clicking the box's middle picks it.
        let mid = pr.screen(Vec3::new(5.0, 5.0, 5.0));
        assert!(pick(&scene, Some(&occ), &pr, mid).is_some());
        assert!(pick(&scene, Some(&occ), &pr, pos2(1.0, 1.0)).is_none());
    }

    /// Every triangle of the scene faces away from `mid`.
    fn faces_out(scene: &Scene, mid: Vec3) -> bool {
        scene.tris.iter().all(|t| t.n.dot((t.p[0] + t.p[1] + t.p[2]) * (1.0 / 3.0) - mid) > 0.0)
    }

    #[test]
    fn a_mirrored_solid_is_not_drawn_inside_out() {
        // Review of PR 20: the triangles of a mirrored solid faced in.
        let mut s = draftcraft_engine::Session::new();
        let r = s.execute("box", &serde_json::json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap();
        s.execute("mirror", &serde_json::json!({"handles": [r["handle"].clone()], "p1": [0, 0], "p2": [0, 1], "erase": true})).unwrap();
        let st = s.state().unwrap();
        let scene = Scene::build(&st.doc, (st.uid, st.revision));
        assert!(!scene.tris.is_empty());
        assert!(faces_out(&scene, Vec3::new(-5.0, 5.0, 5.0)));
    }

    #[test]
    fn a_solid_in_a_block_is_drawn_in_3d() {
        // Review of PR 20: solids inside blocks were drawn flat in the 3D view.
        let mut s = draftcraft_engine::Session::new();
        let r = s.execute("box", &serde_json::json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap();
        s.execute("block", &serde_json::json!({"name": "CUBE", "base": [0, 0], "handles": [r["handle"].clone()], "keep": "delete"})).unwrap();
        s.execute("insert", &serde_json::json!({"name": "CUBE", "at": [100, 0], "scale": [1, -1]})).unwrap();
        let st = s.state().unwrap();
        let scene = Scene::build(&st.doc, (st.uid, st.revision));
        assert_eq!(scene.solids.len(), 1);
        assert!(!scene.tris.is_empty());
        // Where the reference puts it, mirrored by its Y scale (its Z scale is its X one), with its height.
        assert!(scene.tris.iter().flat_map(|t| t.p).all(|p| (100.0 - 1e-9..=110.0 + 1e-9).contains(&p.x) && (-10.0 - 1e-9..=1e-9).contains(&p.y)));
        assert!(scene.tris.iter().flat_map(|t| t.p).any(|p| (p.z - 10.0).abs() < 1e-9));
        assert!(faces_out(&scene, Vec3::new(105.0, -5.0, 5.0)));
        // Its edges are 3D edges of the solid, not its plan drawn flat.
        assert!(scene.segs.iter().all(|g| g.solid.is_some()), "flat segments of the block's solid");
    }

    #[test]
    fn objects_off_the_xy_plane_are_drawn_where_they_are() {
        // Handoff A12: 3D objects were drawn flattened onto Z = 0.
        let mut s = draftcraft_engine::Session::new();
        s.execute("line", &serde_json::json!({"points": [[0, 0, 0], [0, 0, 50]]})).unwrap();
        let st = s.state().unwrap();
        let mut d: Drawing = (*st.doc).clone();
        d.add(
            &Space::Model,
            draftcraft_doc::Common::default(),
            EntityKind::Circle(draftcraft_doc::Circle { center: Vec3::new(100.0, 0.0, 12.0), radius: 5.0 }),
        )
        .unwrap();
        let face = draftcraft_doc::Face3d {
            corners: [Vec3::new(0.0, 0.0, 0.0), Vec3::new(50.0, 0.0, 0.0), Vec3::new(50.0, 50.0, 20.0), Vec3::new(0.0, 50.0, 20.0)],
            hidden_edges: 0,
        };
        d.add(&Space::Model, draftcraft_doc::Common::default(), EntityKind::Face3d(face)).unwrap();
        let thick = draftcraft_doc::Common { thickness: 30.0, ..draftcraft_doc::Common::default() };
        d.add(&Space::Model, thick, EntityKind::Circle(draftcraft_doc::Circle { center: Vec3::new(200.0, 0.0, 0.0), radius: 20.0 })).unwrap();
        let scene = Scene::build(&d, (1, 1));
        let top = |pred: &dyn Fn(&Seg) -> bool| scene.segs.iter().filter(|g| pred(g)).fold(f64::MIN, |m, g| m.max(g.a.z).max(g.b.z));
        // The vertical line stands up, the circle sits at its height, the extruded circle has
        // its top 30 up.
        assert!((top(&|g| g.a.x.abs() < 1e-9 && g.b.x.abs() < 1e-9 && g.a.y.abs() < 1e-9) - 50.0).abs() < 1e-9);
        assert!(scene.segs.iter().filter(|g| g.a.x > 90.0 && g.a.x < 110.0).all(|g| (g.a.z - 12.0).abs() < 1e-9));
        assert!((top(&|g| g.a.x > 170.0) - 30.0).abs() < 1e-9);
        // The 3D face and the extruded circle hide what is behind them, like solids.
        assert_eq!(scene.solids.len(), 2);
        assert!(scene.tris.iter().flat_map(|t| t.p).any(|p| (p.z - 20.0).abs() < 1e-9));
    }

    #[test]
    fn faces_off_the_view_hide_nothing_at_its_border() {
        // Review of PR 20: triangles off screen were clamped into the border cells, where they
        // hid edges they are nowhere near.
        let mut s = draftcraft_engine::Session::new();
        s.execute("box", &serde_json::json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap();
        let st = s.state().unwrap();
        let scene = Scene::build(&st.doc, (st.uid, st.revision));
        let rect = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        // Looking down at a spot far to the right of the box: the box is off the left edge.
        let v = View3d::new(Vec3::new(0.0, 0.0, -1.0), Vec2::new(1000.0, 5.0), 40.0, VisualStyle::Hidden).unwrap();
        let pr = Proj::new(&v, rect);
        assert!(pr.screen(Vec3::new(10.0, 5.0, 10.0)).x < 0.0);
        let occ = Occluders::new(&scene, &pr, rect, 1e-5);
        assert!(occ.cells.iter().all(Vec::is_empty));
        assert!(occ.cell(pos2(-5.0, 300.0)).is_none() && occ.cell(pos2(10_000.0, 300.0)).is_none());
        assert!(!occ.hidden(pos2(1.0, 300.0), 1e9));
    }

    #[test]
    fn a_repaint_reuses_what_was_painted() {
        // Review of PR 20: every repaint sorted the faces and cut the edges again.
        let mut s = draftcraft_engine::Session::new();
        s.execute("box", &serde_json::json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap();
        let st = s.state().unwrap();
        let scene = Scene::build(&st.doc, (st.uid, st.revision));
        let rect = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let v = View3d::new(Vec3::new(-1.0, 1.0, -1.0), Vec2::new(0.0, 5.0), 40.0, VisualStyle::Conceptual).unwrap();
        let pr = Proj::new(&v, rect);
        let none = std::collections::HashSet::new();
        let key = PaintKey { scene: scene.key, view: v, rect, selection: selection_key(&none), bg: Color32::BLACK };
        let ctx = egui::Context::default();
        let a = painted(&ctx, &scene, &key, &pr, &none, Color32::WHITE);
        let b = painted(&ctx, &scene, &key, &pr, &none, Color32::WHITE);
        assert!(Arc::ptr_eq(&a.shapes, &b.shapes));
        assert!(a.occ.is_some());
        // Another view paints again.
        let key2 = PaintKey { view: v.orbited(10.0, 0.0).unwrap(), ..key };
        let c = painted(&ctx, &scene, &key2, &pr, &none, Color32::WHITE);
        assert!(!Arc::ptr_eq(&a.shapes, &c.shapes));
    }
}
