//! Regions (REGION): a closed planar area bounded by loops of lines and arcs. Outer loops add
//! area, island loops (holes) take it away; loops don't cross.

use draftcraft_geom::{Bounds2, PolyVertex, Polyline, Segment, Vec2, point_in_polygon};
use serde::{Deserialize, Serialize};

use super::HatchLoop;

/// Most loops one region has (hostile files and edits).
pub const MAX_REGION_LOOPS: usize = 10_000;

/// Most polygon edges [`Region::renest`] tests, over all its point-in-loop tests.
pub const MAX_RENEST_EDGE_VISITS: usize = 50_000_000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Closed loops with bulges; `outer` false for islands.
    pub loops: Vec<HatchLoop>,
}

/// Area properties of a region (MASSPROP), in drawing units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RegionProps {
    pub area: f64,
    pub perimeter: f64,
    pub bounds: Bounds2,
    pub centroid: Vec2,
    /// ∫y² dA and ∫x² dA about the origin.
    pub ixx: f64,
    pub iyy: f64,
    /// ∫xy dA about the origin.
    pub ixy: f64,
    /// The same three about the centroid.
    pub centroid_moments: [f64; 3],
}

impl Region {
    /// A region bounded by one closed loop.
    pub fn from_loop(vertices: Vec<PolyVertex>) -> Region {
        Region { loops: vec![HatchLoop { vertices, outer: true, sources: Vec::new() }] }
    }

    pub fn polylines(&self) -> Vec<Polyline> {
        self.loops.iter().map(|l| Polyline { vertices: l.vertices.clone(), closed: true }).collect()
    }

    /// The boundary: every loop's lines and arcs.
    pub fn segments(&self) -> Vec<Segment> {
        self.polylines().iter().flat_map(Polyline::segments).collect()
    }

    /// Each loop as points, arcs within `tol`.
    pub fn tessellate(&self, tol: f64) -> Vec<Vec<Vec2>> {
        self.polylines().iter().map(|p| p.tessellate(tol)).collect()
    }

    pub fn bounds(&self) -> Bounds2 {
        self.polylines().iter().fold(Bounds2::EMPTY, |b, p| b.union(&p.bounds()))
    }

    /// Outer loops' area less the islands'.
    pub fn area(&self) -> f64 {
        self.loops.iter().map(|l| Polyline { vertices: l.vertices.clone(), closed: true }.area().abs() * if l.outer { 1.0 } else { -1.0 }).sum()
    }

    pub fn perimeter(&self) -> f64 {
        self.polylines().iter().map(Polyline::len).sum()
    }

    /// A chord tolerance fine enough for measuring: 1e-5 of the region's size.
    fn fine_tol(&self) -> f64 {
        let b = self.bounds();
        if b.is_empty() { 1e-6 } else { ((b.width() + b.height()) * 1e-5).max(1e-12) }
    }

    /// Whether `p` lies inside the region (odd parity over its loops).
    pub fn contains(&self, p: Vec2) -> bool {
        self.tessellate(self.fine_tol()).iter().filter(|l| point_in_polygon(l, p)).count() % 2 == 1
    }

    /// Set each loop's `outer` flag from how deeply other loops enclose it: an even depth is
    /// an outer loop, odd an island. `false`, with the flags left as they were, when that takes
    /// more than [`MAX_RENEST_EDGE_VISITS`] polygon edge tests.
    #[must_use]
    pub fn renest(&mut self) -> bool {
        let polys = self.tessellate(self.fine_tol());
        // A loop's probe can only be inside loops whose box holds it: test only those. A box with
        // a non-finite corner says nothing (an edge through a non-finite point never counts, so
        // such a polygon need not be closed), so its loop is always tested.
        let boxes: Vec<Option<Bounds2>> = polys
            .iter()
            .map(|l| {
                let b = Bounds2::from_points(l.iter().copied());
                (l.iter().all(|p| p.is_finite()) && b.min.is_finite() && b.max.is_finite()).then_some(b)
            })
            .collect();
        let mut visits = 0usize;
        let mut depth = Vec::with_capacity(polys.len());
        for (i, l) in polys.iter().enumerate() {
            let Some(p) = probe(l) else {
                depth.push(0);
                continue;
            };
            let mut d = 0;
            for (j, (o, b)) in polys.iter().zip(&boxes).enumerate() {
                if j == i || b.is_some_and(|b| !b.contains(p)) {
                    continue;
                }
                // Each loop against every other is loops x vertices: bounded so a region of many
                // large nested loops cannot hold the thread (CWE-407).
                visits = visits.saturating_add(o.len());
                if visits > MAX_RENEST_EDGE_VISITS {
                    return false;
                }
                if point_in_polygon(o, p) {
                    d += 1;
                }
            }
            depth.push(d);
        }
        for (l, d) in self.loops.iter_mut().zip(depth) {
            l.outer = d % 2 == 0;
        }
        true
    }

    /// Area, perimeter, bounding box, centroid and second moments (outer loops counted
    /// counter-clockwise, islands clockwise), exact: each edge's chord, and each arc's circular
    /// segment between its chord and the arc, as MASSPROP gives them.
    pub fn props(&self) -> RegionProps {
        // [area, ∫x, ∫y, ∫y², ∫x², ∫xy]
        let mut m = [0.0f64; 6];
        for l in &self.loops {
            let signed = Polyline { vertices: l.vertices.clone(), closed: true }.area();
            // Outer loops add, islands subtract, whatever way they run.
            let flip = if (signed >= 0.0) == l.outer { 1.0 } else { -1.0 };
            let n = l.vertices.len();
            for i in 0..n {
                let (Some(v), Some(w)) = (l.vertices.get(i), l.vertices.get((i + 1) % n)) else { continue };
                let (p, q) = (v.p, w.p);
                let c = (p.x * q.y - q.x * p.y) * flip;
                m[0] += c / 2.0;
                m[1] += (p.x + q.x) * c / 6.0;
                m[2] += (p.y + q.y) * c / 6.0;
                m[3] += (p.y * p.y + p.y * q.y + q.y * q.y) * c / 12.0;
                m[4] += (p.x * p.x + p.x * q.x + q.x * q.x) * c / 12.0;
                m[5] += (p.x * q.y + 2.0 * p.x * p.y + 2.0 * q.x * q.y + q.x * p.y) * c / 24.0;
                if let Some((arc, ccw)) = draftcraft_geom::bulge_to_arc(p, q, v.bulge) {
                    let s = segment(arc.center, arc.radius, arc.start, arc.sweep());
                    let k = if ccw { flip } else { -flip };
                    for (acc, x) in m.iter_mut().zip(s) {
                        *acc += k * x;
                    }
                }
            }
        }
        let [a, cx, cy, ixx, iyy, ixy] = m;
        let c = if a.abs() > 1e-300 { Vec2::new(cx / a, cy / a) } else { Vec2::ZERO };
        let centroid_moments = [ixx - a * c.y * c.y, iyy - a * c.x * c.x, ixy - a * c.x * c.y];
        RegionProps { area: self.area(), perimeter: self.perimeter(), bounds: self.bounds(), centroid: c, ixx, iyy, ixy, centroid_moments }
    }
}

/// The circular segment between an arc (centre, radius, counter-clockwise from angle `start`
/// through `sweep`) and its chord: [area, ∫x, ∫y, ∫y², ∫x², ∫xy] over it. Worked out in the
/// frame of the arc's middle (u from the centre towards it, v across) as the sector less the
/// triangle of the centre and the chord, then moved to the drawing's axes.
fn segment(center: Vec2, r: f64, start: f64, sweep: f64) -> [f64; 6] {
    let half = sweep / 2.0;
    let mid = start + half;
    let (e, f) = (Vec2::new(mid.cos(), mid.sin()), Vec2::new(-mid.sin(), mid.cos()));
    let (sa, ca) = half.sin_cos();
    let (r2, r4) = (r * r, r * r * r * r);
    // The centre-and-chord triangle's area (negative past a half turn: the segment then holds it).
    let t = r2 * sa * ca;
    let area = r2 * (sweep - sweep.sin()) / 2.0;
    let su = 2.0 / 3.0 * r * r2 * sa * sa * sa;
    let suu = r4 / 8.0 * (sweep + sweep.sin()) - t * r2 * ca * ca / 2.0;
    let svv = r4 / 8.0 * (sweep - sweep.sin()) - t * r2 * sa * sa / 6.0;
    let (x0, y0) = (center.x, center.y);
    [
        area,
        x0 * area + e.x * su,
        y0 * area + e.y * su,
        y0 * y0 * area + 2.0 * y0 * e.y * su + e.y * e.y * suu + f.y * f.y * svv,
        x0 * x0 * area + 2.0 * x0 * e.x * su + e.x * e.x * suu + f.x * f.x * svv,
        x0 * y0 * area + (x0 * e.y + y0 * e.x) * su + e.x * e.y * suu + f.x * f.y * svv,
    ]
}

/// A point of a loop to test its nesting with: the middle of its first non-degenerate edge.
fn probe(l: &[Vec2]) -> Option<Vec2> {
    let n = l.len();
    (0..n).find_map(|i| {
        let (a, b) = (*l.get(i)?, *l.get((i + 1) % n)?);
        (a.dist(b) > 1e-12).then(|| (a + b) * 0.5).filter(|m| m.is_finite())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(c: f64, half: f64, n: usize, outer: bool) -> HatchLoop {
        // `n` vertices along the bottom edge, then the other three corners.
        let mut vertices: Vec<PolyVertex> =
            (0..n).map(|k| PolyVertex::new(Vec2::new(c - half + 2.0 * half * k as f64 / n as f64, c - half))).collect();
        vertices.extend([Vec2::new(c + half, c - half), Vec2::new(c + half, c + half), Vec2::new(c - half, c + half)].map(PolyVertex::new));
        HatchLoop { vertices, outer, sources: Vec::new() }
    }

    #[test]
    fn renest_sets_depth_parity_and_skips_loops_far_away() {
        // Three nested squares, and one beside them.
        let mut r =
            Region { loops: vec![square(0.0, 3.0, 1, false), square(0.0, 2.0, 1, false), square(0.0, 1.0, 1, false), square(10.0, 1.0, 1, false)] };
        assert!(r.renest());
        assert_eq!(r.loops.iter().map(|l| l.outer).collect::<Vec<_>>(), [true, false, true, true]);
    }

    #[test]
    fn renest_of_many_large_nested_loops_stops_at_its_budget() {
        // Concentric loops of many vertices each: each probe is inside the box of every larger
        // loop (about loops² / 2 pairs), so each of those tests is a full polygon scan.
        let loops = 300;
        let n = 3 * MAX_RENEST_EDGE_VISITS / (loops * loops);
        let mut r = Region { loops: (0..loops).map(|i| square(0.0, 1.0 + i as f64, n, false)).collect() };
        let before = r.clone();
        assert!(!r.renest());
        assert_eq!(r, before, "flags left as they were");
    }

    #[test]
    fn mass_properties_are_exact_on_arcs() {
        // AutoCAD 2027's MASSPROP of the same regions (crates/io/tests/data/acad-solids/README-regions.md):
        // area, centroid, moments of inertia about X and Y, four decimals.
        let p = |x: f64, y: f64, b: f64| PolyVertex::with_bulge(Vec2::new(x, y), b);
        let slot = Region::from_loop(vec![p(500.0, 0.0, 0.0), p(600.0, 0.0, 0.0), p(600.0, 60.0, 1.0), p(500.0, 60.0, 0.0)]);
        let disc = Region::from_loop(vec![p(170.0, 30.0, 1.0), p(230.0, 30.0, 1.0)]);
        let plate = Region {
            loops: vec![
                HatchLoop {
                    vertices: vec![p(300.0, 0.0, 0.0), p(400.0, 0.0, 0.0), p(400.0, 60.0, 0.0), p(300.0, 60.0, 0.0)],
                    outer: true,
                    sources: Vec::new(),
                },
                // The hole runs counter-clockwise too: its flag says it takes area away.
                HatchLoop { vertices: vec![p(335.0, 30.0, 1.0), p(365.0, 30.0, 1.0)], outer: false, sources: Vec::new() },
            ],
        };
        for (r, area, c, ixx, iyy) in [
            (slot, 9926.9908, [550.0, 50.2622], 33791536.2018, 3010369091.3993),
            (disc, 2827.4334, [200.0, 30.0], 3180862.5618, 113733508.0416),
            (plate, 5293.1417, [350.0, 30.0], 6524066.7056, 653370091.7034),
        ] {
            let m = r.props();
            let near = |a: f64, b: f64| (a - b).abs() <= 1e-4 + 1e-12 * b.abs();
            assert!(near(m.area, area) && near(m.centroid.x, c[0]) && near(m.centroid.y, c[1]), "{m:?}");
            assert!(near(m.ixx, ixx) && near(m.iyy, iyy), "{m:?}");
        }
    }
}
