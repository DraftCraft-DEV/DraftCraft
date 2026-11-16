//! Drawings from solids (FLATSHOT and SECTION).
//!
//! solidcore has no hidden-line module, so the view is made here: each B-rep edge is projected
//! exactly where it can be (a line, or a line or arc lying in a plane parallel to the paper,
//! read exactly by the kernel), the silhouettes of curved faces are found on the display mesh,
//! and every piece's visibility is decided by sampling it against the meshes' triangles. Pieces
//! drawn on top of a nearer line (a back edge behind a front edge) are left out, as a drafter
//! would. A cross-section slices the body and reads the cut face's edges exactly.

use draftcraft_geom::{Vec2, Vec3};
use solidcore_ops as ops;

use crate::body::{Body, Frame, Meshed, Result, Seg, SolidError, from_rec, from_seg2, unit};

fn err(m: impl Into<String>) -> SolidError {
    SolidError(m.into())
}

/// How a piece of a projected line is seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seen {
    Visible,
    Hidden,
    /// Could not be decided: reported, not guessed.
    Uncertain,
}

/// One piece of a projected line, in the view's 2D coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewPiece {
    pub seen: Seen,
    /// Lines and arcs where the projection is exact, short lines (chords) elsewhere.
    pub segs: Vec<Seg>,
    /// Whether this piece is a silhouette of a curved face rather than an edge.
    pub silhouette: bool,
    /// Whether the piece is exact (lines and arcs) rather than chords of a curve.
    pub exact: bool,
}

/// A projected view of solids: what FLATSHOT draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub pieces: Vec<ViewPiece>,
    /// Faces the mesher could not mesh: what they would hide is not known, so lines behind them
    /// may show as visible.
    pub unsupported_faces: usize,
}

/// Most bodies in one view.
pub const MAX_VIEW_BODIES: usize = 10_000;

/// Most visibility samples in one view, and on one line.
const MAX_SAMPLES: usize = 4_000_000;
const MAX_LINE_SAMPLES: usize = 4_000;

/// A line to draw: its 3D points and, where the projection is exact, its 2D segment.
struct Line {
    pts: Vec<Vec3>,
    exact: Option<Seg>,
    silhouette: bool,
}

/// A triangle seen on the paper: its 2D corners and their depths.
struct Tri {
    p: [Vec2; 3],
    d: [f64; 3],
}

/// A square grid over the paper, each cell listing what may overlap it.
struct Grid {
    lo: Vec2,
    cell: f64,
    n: usize,
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn new(lo: Vec2, hi: Vec2, items: usize) -> Grid {
        let side = (hi.x - lo.x).max(hi.y - lo.y).max(1e-9);
        let n = ((items as f64).sqrt().ceil() as usize).clamp(1, 256);
        Grid { lo, cell: side / n as f64, n, cells: vec![Vec::new(); n * n] }
    }
    fn index(&self, v: f64, o: f64) -> usize {
        let i = ((v - o) / self.cell).floor();
        if i.is_finite() && i > 0.0 { (i as usize).min(self.n - 1) } else { 0 }
    }
    fn insert(&mut self, id: u32, a: Vec2, b: Vec2) {
        let (x0, x1) = (self.index(a.x.min(b.x), self.lo.x), self.index(a.x.max(b.x), self.lo.x));
        let (y0, y1) = (self.index(a.y.min(b.y), self.lo.y), self.index(a.y.max(b.y), self.lo.y));
        for y in y0..=y1 {
            for x in x0..=x1 {
                if let Some(c) = self.cells.get_mut(y * self.n + x) {
                    c.push(id);
                }
            }
        }
    }
    fn at(&self, p: Vec2) -> &[u32] {
        let (x, y) = (self.index(p.x, self.lo.x), self.index(p.y, self.lo.y));
        self.cells.get(y * self.n + x).map_or(&[], Vec::as_slice)
    }
}

/// How one sample of a line is seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Seen(Seen),
    /// Under a nearer line, or along the view direction: not drawn.
    Gone,
}

/// The view of `bodies` looking along `direction`, with `up` pointing up on the paper. The view's
/// 2D coordinates are `(p · x, p · y)` with `x = direction × up` and `y = x × direction`.
pub fn project(bodies: &[&Body], direction: Vec3, up: Vec3) -> Result<View> {
    if bodies.len() > MAX_VIEW_BODIES {
        return Err(err("too many solids for one view"));
    }
    let dir = unit(direction).ok_or_else(|| err("the view needs a direction"))?;
    let x = unit(dir.cross(up)).ok_or_else(|| err("the view's up direction must not be along the view direction"))?;
    let y = x.cross(dir);
    let paper = Frame { origin: Vec3::ZERO, x, y };
    let plane = paper.plane()?;
    let to2 = |p: Vec3| Vec2::new(p.dot(x), p.dot(y));

    let mut lines: Vec<Line> = Vec::new();
    let mut tris: Vec<Tri> = Vec::new();
    let mut unsupported = 0usize;
    let mut size = 0.0f64;
    let mut chord = 0.0f64;
    for b in bodies {
        let Meshed { body, order, mesh, refused, chord: c } = b.meshed(b.size().max(1e-6) * 2e-4)?;
        unsupported = unsupported.saturating_add(refused);
        size = size.max(b.size());
        chord = chord.max(c);
        let all: Vec<usize> = (0..mesh.edges.len()).collect();
        let exact = ops::prism::planar_edges(&body, &order, &plane, &all).map(|e| e.pieces).unwrap_or_default();
        for (i, e) in mesh.edges.iter().enumerate() {
            // Seams (a closed face meeting itself) are not drawn: where one lies on a silhouette,
            // the silhouette is.
            if !mesh.edge_visible(i) || crate::body::is_seam(&mesh, i) || e.len() < 2 {
                continue;
            }
            let pts: Vec<Vec3> = e.iter().map(|p| from_rec(*p)).collect();
            let seg = match exact.get(i).and_then(|o| o.as_ref()).map(Vec::as_slice) {
                Some([ops::prism::PlanarPiece::Seg(s)]) => from_seg2(s),
                _ => None,
            };
            let seg = seg.or_else(|| match pts.as_slice() {
                [a, b] => Some(Seg::Line { a: to2(*a), b: to2(*b) }),
                _ => None,
            });
            lines.push(Line { pts, exact: seg, silhouette: false });
        }
        let pos: Vec<Vec3> = mesh.positions.iter().map(|p| from_rec(*p)).collect();
        let corner = |t: &[u32; 3]| -> Option<[Vec3; 3]> { Some([*pos.get(t[0] as usize)?, *pos.get(t[1] as usize)?, *pos.get(t[2] as usize)?]) };
        // Silhouettes: inside one face, the mesh edges between a triangle facing the viewer (or
        // edge on) and one facing away.
        let mut facing: std::collections::HashMap<([u64; 3], [u64; 3]), (i8, u32)> = std::collections::HashMap::new();
        let at = |i: u32| pos.get(i as usize).map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
        for (ti, t) in mesh.triangles.iter().enumerate() {
            let Some(c) = corner(t) else { continue };
            let face = mesh.tri_face.get(ti).copied().unwrap_or(u32::MAX);
            let n = (c[1] - c[0]).cross(c[2] - c[0]);
            let area = n.len();
            if area.is_finite() && area > 0.0 {
                let v = n.dot(dir) / area;
                let s: i8 = if v < -1e-9 {
                    1
                } else if v > 1e-9 {
                    -1
                } else {
                    0
                };
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let (Some(ka), Some(kb)) = (at(a), at(b)) else { continue };
                    let key = (ka.min(kb), ka.max(kb));
                    match facing.remove(&key) {
                        Some((s2, f2)) if f2 == face && s != s2 && s.min(s2) == -1 => {
                            if let (Some(p), Some(q)) = (pos.get(a as usize), pos.get(b as usize)) {
                                lines.push(Line { pts: vec![*p, *q], exact: None, silhouette: true });
                            }
                        }
                        Some(_) => {}
                        None => {
                            facing.insert(key, (s, face));
                        }
                    }
                }
            }
            let p = [to2(c[0]), to2(c[1]), to2(c[2])];
            let a2 = (p[1] - p[0]).cross(p[2] - p[0]).abs();
            if a2.is_finite() && a2 > 1e-12 * size.max(1.0).powi(2) {
                tris.push(Tri { p, d: [c[0].dot(dir), c[1].dot(dir), c[2].dot(dir)] });
            }
        }
    }
    merge_silhouettes(&mut lines);

    let size = size.max(1e-9);
    let eps = (chord * 2.0).max(size * 1e-7);
    let step = size / 400.0;

    // The paper's extent and the grids of triangles and of line pieces.
    let mut lo = Vec2::new(f64::INFINITY, f64::INFINITY);
    let mut hi = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for l in &lines {
        for p in &l.pts {
            let q = to2(*p);
            lo = Vec2::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = Vec2::new(hi.x.max(q.x), hi.y.max(q.y));
        }
    }
    if !(lo.x.is_finite() && hi.x.is_finite()) {
        return Ok(View { pieces: Vec::new(), unsupported_faces: unsupported });
    }
    let mut tri_grid = Grid::new(lo, hi, tris.len());
    for (i, t) in tris.iter().enumerate() {
        let a = Vec2::new(t.p[0].x.min(t.p[1].x).min(t.p[2].x), t.p[0].y.min(t.p[1].y).min(t.p[2].y));
        let b = Vec2::new(t.p[0].x.max(t.p[1].x).max(t.p[2].x), t.p[0].y.max(t.p[1].y).max(t.p[2].y));
        tri_grid.insert(u32::try_from(i).unwrap_or(u32::MAX), a, b);
    }
    // Every line piece, for the "drawn over a nearer line" test: (line, a, b, depth a, depth b).
    let mut pieces: Vec<(usize, Vec2, Vec2, f64, f64)> = Vec::new();
    for (li, l) in lines.iter().enumerate() {
        for w in l.pts.windows(2) {
            if let [a, b] = w {
                pieces.push((li, to2(*a), to2(*b), a.dot(dir), b.dot(dir)));
            }
        }
    }
    let mut seg_grid = Grid::new(lo, hi, pieces.len());
    for (i, p) in pieces.iter().enumerate() {
        seg_grid.insert(u32::try_from(i).unwrap_or(u32::MAX), p.1, p.2);
    }

    let hidden = |q: Vec2, depth: f64| -> bool {
        tri_grid.at(q).iter().any(|&i| {
            let Some(t) = tris.get(i as usize) else { return false };
            match inside(t, q, -eps * 1e-3) {
                Some(d) => d < depth - eps,
                None => false,
            }
        })
    };
    let covered = |line: usize, q: Vec2, depth: f64| -> bool {
        seg_grid.at(q).iter().any(|&i| {
            let Some(&(li, a, b, da, db)) = pieces.get(i as usize) else { return false };
            if li == line {
                return false;
            }
            let ab = b - a;
            let l2 = ab.dot(ab);
            let t = if l2 > 0.0 { ((q - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let foot = a + ab * t;
            foot.dist(q) <= eps && da + (db - da) * t < depth - eps
        })
    };

    let mut budget = MAX_SAMPLES;
    let mut out = View { pieces: Vec::new(), unsupported_faces: unsupported };
    for (li, l) in lines.iter().enumerate() {
        // Sample the line finely, each sample the middle of a short piece of it.
        let mut pts: Vec<Vec3> = Vec::new();
        for w in l.pts.windows(2) {
            if let [a, b] = w {
                let n = (((*b - *a).len() / step).ceil() as usize).clamp(1, MAX_LINE_SAMPLES);
                for k in 0..n {
                    pts.push(*a + (*b - *a) * (k as f64 / n as f64));
                }
            }
        }
        if let Some(last) = l.pts.last() {
            pts.push(*last);
        }
        if pts.len() > MAX_LINE_SAMPLES * 4 || pts.len() > budget {
            return Err(err("the view has too many lines to work out"));
        }
        budget -= pts.len();
        let classes: Vec<Class> = pts
            .windows(2)
            .map(|w| match w {
                [a, b] => {
                    let (qa, qb) = (to2(*a), to2(*b));
                    let m = (*a + *b) * 0.5;
                    let (q, d) = (to2(m), m.dot(dir));
                    // Lines seen end-on, and lines a nearer coincident line covers, are left out.
                    if (qa.dist(qb) <= eps * 1e-3 && l.pts.len() == 2) || covered(li, q, d) {
                        Class::Gone
                    } else if hidden(q, d) {
                        Class::Seen(Seen::Hidden)
                    } else {
                        Class::Seen(Seen::Visible)
                    }
                }
                _ => Class::Gone,
            })
            .collect();
        // Runs of one class become pieces.
        let mut start = 0usize;
        while start < classes.len() {
            let c = classes.get(start).copied().unwrap_or(Class::Gone);
            let mut end = start;
            while classes.get(end + 1) == Some(&c) {
                end += 1;
            }
            if let Class::Seen(seen) = c {
                let run: Vec<Vec2> = pts.get(start..=end + 1).unwrap_or(&[]).iter().map(|p| to2(*p)).collect();
                let whole = start == 0 && end + 1 == classes.len();
                if let Some(piece) = make_piece(l, &run, whole, seen, eps) {
                    out.pieces.push(piece);
                }
            }
            start = end + 1;
        }
    }
    Ok(out)
}

/// The depth of triangle `t` at `q` when `q` is inside it by more than `margin`.
fn inside(t: &Tri, q: Vec2, margin: f64) -> Option<f64> {
    let [a, b, c] = t.p;
    let area = (b - a).cross(c - a);
    if area.abs() <= 0.0 {
        return None;
    }
    // Signed distances to each side, positive inside.
    let side = |p: Vec2, r: Vec2| -> Option<f64> {
        let len = p.dist(r);
        (len > 0.0).then(|| (r - p).cross(q - p) / len * area.signum())
    };
    let (da, db, dc) = (side(b, c)?, side(c, a)?, side(a, b)?);
    if da <= margin || db <= margin || dc <= margin {
        return None;
    }
    let (wa, wb) = ((b - q).cross(c - q) / area, (c - q).cross(a - q) / area);
    let wc = 1.0 - wa - wb;
    Some(wa * t.d[0] + wb * t.d[1] + wc * t.d[2])
}

/// A run of a line as a piece: the exact segment cut to the run where there is one, chords
/// otherwise.
fn make_piece(l: &Line, run: &[Vec2], whole: bool, seen: Seen, eps: f64) -> Option<ViewPiece> {
    let (first, last) = (*run.first()?, *run.last()?);
    if run.len() < 2 || (run.len() == 2 && first.dist(last) <= eps * 1e-3) {
        return None;
    }
    let exact = l.exact.and_then(|s| if whole { Some(s) } else { cut(s, run) });
    let (segs, is_exact) = match exact {
        Some(s) => (vec![s], true),
        None => (chords(run, eps), false),
    };
    (!segs.is_empty()).then_some(ViewPiece { seen, segs, silhouette: l.silhouette, exact: is_exact })
}

/// `s` cut to the part `run` follows (its first and last points on it), or `None` when the run
/// doesn't lie along it.
fn cut(s: Seg, run: &[Vec2]) -> Option<Seg> {
    let (first, last, second) = (*run.first()?, *run.last()?, *run.get(1)?);
    match s {
        Seg::Line { a, b } => {
            let ab = b - a;
            let l2 = ab.dot(ab);
            if l2 <= 0.0 {
                return None;
            }
            let t = |p: Vec2| ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
            Some(Seg::Line { a: a + ab * t(first), b: a + ab * t(last) })
        }
        Seg::Arc { center, radius, start, sweep } => {
            let tau = std::f64::consts::TAU;
            let sign = sweep.signum();
            let u = |p: Vec2| ((p.y - center.y).atan2(p.x - center.x) - start) * sign;
            let u = |p: Vec2| u(p).rem_euclid(tau);
            let span = sweep.abs();
            // The run may go the other way round from the arc.
            let forward = u(second) >= u(first) || u(first) > span - 1e-9;
            let (s, e) = if forward { (u(first), u(last)) } else { (u(last), u(first)) };
            let s = if s >= span && s > tau - 1e-6 { 0.0 } else { s.min(span) };
            let mut e = e;
            if e <= s {
                e += tau;
            }
            let e = e.min(span);
            (e > s).then_some(Seg::Arc { center, radius, start: start + sign * s, sweep: sign * (e - s) })
        }
    }
}

/// A polyline as lines, consecutive pieces along one direction joined.
fn chords(run: &[Vec2], eps: f64) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    for w in run.windows(2) {
        let [a, b] = w else { continue };
        if a.dist(*b) <= eps * 1e-3 {
            continue;
        }
        if let Some(Seg::Line { a: pa, b: pb }) = out.last_mut() {
            let (u, v) = (*pb - *pa, *b - *a);
            if (*pb).dist(*a) <= eps * 1e-3 && u.cross(v).abs() <= 1e-9 * u.len() * v.len() && u.dot(v) > 0.0 {
                *pb = *b;
                continue;
            }
        }
        out.push(Seg::Line { a: *a, b: *b });
    }
    out
}

/// Silhouette pieces end to end along one line joined, so a cylinder's outline is one line.
fn merge_silhouettes(lines: &mut Vec<Line>) {
    let (sil, mut rest): (Vec<Line>, Vec<Line>) = std::mem::take(lines).into_iter().partition(|l| l.silhouette);
    let mut segs: Vec<(Vec3, Vec3)> = sil.iter().filter_map(|l| Some((*l.pts.first()?, *l.pts.last()?))).collect();
    let mut joined: Vec<Line> = Vec::new();
    while let Some((mut a, mut b)) = segs.pop() {
        loop {
            let d = b - a;
            let near = |p: Vec3, q: Vec3| (p - q).len() <= 1e-9 * (1.0 + d.len());
            let along = |p: Vec3, q: Vec3| {
                let e = q - p;
                d.cross(e).len() <= 1e-9 * d.len() * e.len()
            };
            let Some(i) = segs.iter().position(|(p, q)| (near(*p, b) || near(*q, b) || near(*p, a) || near(*q, a)) && along(*p, *q)) else {
                break;
            };
            let (p, q) = segs.swap_remove(i);
            if near(p, b) {
                b = q;
            } else if near(q, b) {
                b = p;
            } else if near(q, a) {
                a = p;
            } else {
                a = q;
            }
        }
        joined.push(Line { pts: vec![a, b], exact: None, silhouette: true });
    }
    rest.extend(joined);
    *lines = rest;
}

/// The cross-section of `body` by the plane `frame`: closed loops in the frame's coordinates,
/// outer boundaries and holes alike (empty when the plane misses the body).
pub fn section(body: &Body, frame: &Frame) -> Result<Vec<Vec<Seg>>> {
    let (part, _) = crate::body::slice(body, frame.origin, frame.normal())?;
    let Some(part) = part else { return Ok(Vec::new()) };
    let size = part.size().max(1e-9);
    let tol = size * 1e-6;
    let normal = frame.normal();
    let Meshed { body: work, order, mesh, .. } = part.meshed(size / 2000.0)?;
    let on_plane: Vec<usize> = mesh
        .edges
        .iter()
        .enumerate()
        .filter(|(_, pts)| !pts.is_empty() && pts.iter().all(|p| (from_rec(*p) - frame.origin).dot(normal).abs() <= tol * 10.0))
        .map(|(i, _)| i)
        .collect();
    let plane = frame.plane()?;
    let exact = ops::prism::planar_edges(&work, &order, &plane, &on_plane)?.pieces;
    let mut segs = Vec::new();
    for (k, &edge) in on_plane.iter().enumerate() {
        let read: Option<Vec<Seg>> = exact.get(k).and_then(|o| o.as_ref()).and_then(|ps| {
            ps.iter()
                .map(|p| match p {
                    ops::prism::PlanarPiece::Seg(s) => from_seg2(s),
                    ops::prism::PlanarPiece::Spline(_) => None,
                })
                .collect()
        });
        match read {
            Some(s) => segs.extend(s),
            // A curve that is no line or arc: its chords.
            None => {
                let pts: Vec<Vec2> = mesh.edges.get(edge).map(|e| e.iter().map(|p| frame.to_local(from_rec(*p))).collect()).unwrap_or_default();
                segs.extend(pts.windows(2).filter_map(|w| match w {
                    [a, b] if a.dist(*b) > 0.0 => Some(Seg::Line { a: *a, b: *b }),
                    _ => None,
                }));
            }
        }
    }
    chain(segs, tol * 100.0)
}

/// Most segments chained into loops.
const MAX_CHAIN: usize = 200_000;

/// Join segments end to start into closed loops.
fn chain(mut segs: Vec<Seg>, tol: f64) -> Result<Vec<Vec<Seg>>> {
    if segs.len() > MAX_CHAIN {
        return Err(err("section: too many edges"));
    }
    let mut loops = Vec::new();
    while let Some(first) = segs.pop() {
        let start = first.start();
        let mut lp = vec![first];
        let mut end = first.end();
        while end.dist(start) > tol {
            let next = segs.iter().position(|s| s.start().dist(end) <= tol || s.end().dist(end) <= tol);
            let Some(i) = next else { return Err(err("section: the cut's edges don't close")) };
            let s = segs.swap_remove(i);
            let s = if s.start().dist(end) <= tol { s } else { s.reversed() };
            end = s.end();
            lp.push(s);
        }
        loops.push(lp);
    }
    Ok(loops)
}
