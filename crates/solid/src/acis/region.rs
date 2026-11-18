//! Regions as AutoCAD keeps them: a REGION entity's ACIS data is a sheet body, flat faces with
//! no volume, each face double-sided (`double out`) and each edge used by one coedge. DraftCraft's
//! regions lie on the XY plane at z = 0 and are bounded by lines and arcs (polyline vertices with
//! bulges), so that is what is read and written here.
//!
//! - [`read_region`] gives every loop of a region's faces as closed polyline vertices; a face off
//!   the XY plane, or an edge that is neither a line nor a circular arc, is refused.
//! - [`region_records`] writes loops (each face's outer loop first, then its holes) as one lump
//!   per face, one shell, one plane face on `z = 0` with normal +Z: the outer loop counter-
//!   clockwise, the holes clockwise, every edge running the way its loop does.

use std::collections::HashMap;
use std::f64::consts::TAU;

use draftcraft_geom::{PolyVertex, Polyline, Vec2, bulge_to_arc, point_in_polygon};
use serde_json::Value;
use solidcore_step::geom::{Curve, P3, Surface};

use super::model::{Field, Record};
use super::read::{Geometry, by_id, decode, ids};
use super::write::{ACurve, AEdge, AFace, Model, Use, layout};
use crate::body::{Result, SolidError};

fn err(m: impl Into<String>) -> SolidError {
    SolidError(m.into())
}

/// Most loops read from or written for one region.
pub const MAX_REGION_LOOPS: usize = 10_000;
/// Most edges read from or written for one region.
const MAX_REGION_EDGES: usize = 1_000_000;
/// Most polygon edges tested while finding which outer loop holds each hole.
const MAX_NESTING_VISITS: usize = 50_000_000;

// ---------------------------------------------------------------------------------------------
// Reading

/// The loops of a REGION's ACIS data (SAT text, scrambled or not, or SAB): each a closed run of
/// polyline vertices on the XY plane, the bulge of each the arc to the next.
pub fn read_region(data: &[u8]) -> Result<Vec<Vec<PolyVertex>>> {
    let (v, mm) = decode(data)?;
    flat_loops(&v, mm)
}

/// The angle of `p` about a circle's centre, in its placement's frame.
fn angle(c: P3, x: P3, y: P3, p: P3) -> f64 {
    let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    dot(d, y).atan2(dot(d, x))
}

fn flat_loops(v: &Value, mm: f64) -> Result<Vec<Vec<PolyVertex>>> {
    let unplaced = Geometry::new(mm);
    let (faces, loops, coedges, edges, vertices, points, surfaces, curves, shells, regions) = (
        by_id(v, "faces"),
        by_id(v, "loops"),
        by_id(v, "coedges"),
        by_id(v, "edges"),
        by_id(v, "vertices"),
        by_id(v, "points"),
        by_id(v, "surfaces"),
        by_id(v, "curves"),
        by_id(v, "shells"),
        by_id(v, "regions"),
    );
    let procedural: HashMap<&str, &Value> = HashMap::new();
    let mut out: Vec<Vec<PolyVertex>> = Vec::new();
    let mut edge_count = 0usize;
    let bodies = v.get("bodies").and_then(Value::as_array).cloned().unwrap_or_default();
    for body in &bodies {
        let geometry = unplaced.transformed(body.get("transform"))?;
        let at = |key: Option<&Value>| -> Option<P3> {
            key.and_then(Value::as_str)
                .and_then(|k| vertices.get(k))
                .and_then(|x| x.get("point"))
                .and_then(Value::as_str)
                .and_then(|p| points.get(p))
                .and_then(|p| geometry.point(p, "position"))
        };
        for region in ids(body.get("regions")) {
            let Some(region) = regions.get(region) else { continue };
            for shell in ids(region.get("shells")) {
                let Some(shell) = shells.get(shell) else { continue };
                for member in shell.get("members").and_then(Value::as_array).into_iter().flatten() {
                    if member.get("kind").and_then(Value::as_str) != Some("face") {
                        continue;
                    }
                    let Some(face) = member.get("id").and_then(Value::as_str).and_then(|f| faces.get(f)) else { continue };
                    let surface =
                        face.get("surface").and_then(Value::as_str).and_then(|s| surfaces.get(s)).ok_or_else(|| err("a face without its surface"))?;
                    let Surface::Plane { position } = geometry.surface(surface)? else {
                        return Err(err("a region face that is not flat"));
                    };
                    let n = position.axis;
                    if !(n[0].abs() <= 1e-9 && n[1].abs() <= 1e-9) {
                        return Err(err("a region off the XY plane (not read yet)"));
                    }
                    for lp in ids(face.get("loops").and_then(|l| l.get("loops"))) {
                        let Some(lp) = loops.get(lp) else { continue };
                        let mut run: Vec<PolyVertex> = Vec::new();
                        for co in ids(lp.get("boundary").and_then(|b| b.get("coedges"))) {
                            edge_count += 1;
                            if edge_count > MAX_REGION_EDGES {
                                return Err(err("the region has too many edges"));
                            }
                            let Some(co) = coedges.get(co) else { continue };
                            let e =
                                co.get("edge").and_then(Value::as_str).and_then(|e| edges.get(e)).ok_or_else(|| err("a coedge without its edge"))?;
                            let (Some(pa), Some(pz)) = (at(e.get("start")), at(e.get("end"))) else {
                                return Err(err("an edge without its vertices"));
                            };
                            let size = 1.0_f64.max(pa.iter().chain(pz.iter()).fold(0.0, |m, x| m.max(x.abs())));
                            if pa[2].abs() > 1e-9 * size || pz[2].abs() > 1e-9 * size {
                                return Err(err("a region off the XY plane (not read yet)"));
                            }
                            let forward = co.get("sense").and_then(Value::as_str) != Some("reversed");
                            let closed = e.get("start") == e.get("end");
                            let carrier = e.get("carrier").and_then(|c| c.get("curve")).and_then(Value::as_str);
                            let c = match carrier.and_then(|c| curves.get(c)) {
                                Some(c) => geometry.curve(c, &procedural)?,
                                None => None,
                            };
                            let from = if forward { pa } else { pz };
                            let circle = match &c {
                                None => continue,
                                Some(Curve::Line { .. }) => None,
                                Some(Curve::Circle { position, radius }) => Some((position, *radius)),
                                Some(Curve::Ellipse { position, semi_axis_1, semi_axis_2 })
                                    if (semi_axis_1 - semi_axis_2).abs() <= 1e-12 * semi_axis_1.abs().max(1.0) =>
                                {
                                    Some((position, *semi_axis_1))
                                }
                                Some(_) => return Err(err("a region with an elliptical or spline edge (not read yet)")),
                            };
                            let Some((p, r)) = circle else {
                                run.push(PolyVertex::new(Vec2::new(from[0], from[1])));
                                continue;
                            };
                            if !(r.is_finite() && r > 0.0) {
                                return Err(err("an arc without size"));
                            }
                            // The edge runs along its circle, counter-clockwise about its axis,
                            // from its start to its end: a whole turn when they are one vertex.
                            let (x, a) = (p.ref_direction, p.axis);
                            let y = [a[1] * x[2] - a[2] * x[1], a[2] * x[0] - a[0] * x[2], a[0] * x[1] - a[1] * x[0]];
                            let (t0, t1) = (angle(p.origin, x, y, pa), angle(p.origin, x, y, pz));
                            let mut sweep = (t1 - t0).rem_euclid(TAU);
                            if closed || sweep <= 1e-12 {
                                sweep = TAU;
                            }
                            // Seen from +Z, counter-clockwise when the axis and the use agree.
                            let ccw = (a[2] > 0.0) == forward;
                            let sign = if ccw { 1.0 } else { -1.0 };
                            if sweep >= TAU - 1e-9 {
                                // A whole circle: two half turns.
                                let mid = [2.0 * p.origin[0] - from[0], 2.0 * p.origin[1] - from[1]];
                                run.push(PolyVertex::with_bulge(Vec2::new(from[0], from[1]), sign));
                                run.push(PolyVertex::with_bulge(Vec2::new(mid[0], mid[1]), sign));
                            } else {
                                run.push(PolyVertex::with_bulge(Vec2::new(from[0], from[1]), sign * (sweep / 4.0).tan()));
                            }
                        }
                        if run.len() >= 2 {
                            if out.len() >= MAX_REGION_LOOPS {
                                return Err(err("the region has too many loops"));
                            }
                            out.push(run);
                        }
                    }
                }
            }
        }
    }
    if out.is_empty() {
        return Err(err("the ACIS data holds no region"));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Writing

/// The loop `l` running counter-clockwise (`ccw`) or clockwise, its vertices with no edge of
/// zero length.
fn oriented(l: &[PolyVertex], ccw: bool) -> Vec<PolyVertex> {
    let mut v: Vec<PolyVertex> = Vec::with_capacity(l.len());
    for p in l {
        if !(p.p.is_finite() && p.bulge.is_finite()) {
            continue;
        }
        match v.last() {
            Some(last) if last.p.near(p.p, 1e-12 * (1.0 + p.p.x.abs() + p.p.y.abs())) => {}
            _ => v.push(PolyVertex::with_bulge(p.p, p.bulge)),
        }
    }
    while v.len() > 1 && v.first().zip(v.last()).is_some_and(|(a, b)| a.p.near(b.p, 1e-12 * (1.0 + a.p.x.abs() + a.p.y.abs()))) {
        v.pop();
    }
    let area = Polyline { vertices: v.clone(), closed: true }.area();
    if (area > 0.0) != ccw {
        // Backwards: the vertices in the other order, each edge's bulge negated and moved to its
        // new start.
        let n = v.len();
        let mut r: Vec<PolyVertex> = Vec::with_capacity(n);
        for k in 0..n {
            let (Some(p), Some(e)) = (v.get(n - 1 - k), v.get((2 * n - 2 - k) % n)) else { continue };
            r.push(PolyVertex::with_bulge(p.p, -e.bulge));
        }
        v = r;
    }
    v
}

/// Each outer loop with the holes it holds: a hole goes to the smallest outer loop around it.
fn faces_of(loops: &[(Vec<PolyVertex>, bool)]) -> Result<Vec<Vec<Vec<PolyVertex>>>> {
    let outer: Vec<&Vec<PolyVertex>> = loops.iter().filter(|(_, o)| *o).map(|(l, _)| l).collect();
    let size = loops.iter().flat_map(|(l, _)| l.iter()).fold(0.0_f64, |m, v| m.max(v.p.x.abs()).max(v.p.y.abs()));
    let tol = (size * 1e-6).max(1e-9);
    let rings: Vec<Vec<Vec2>> = outer.iter().map(|l| Polyline { vertices: (*l).clone(), closed: true }.tessellate(tol)).collect();
    let areas: Vec<f64> = outer.iter().map(|l| Polyline { vertices: (*l).clone(), closed: true }.area().abs()).collect();
    let mut faces: Vec<Vec<Vec<PolyVertex>>> = outer.iter().map(|l| vec![oriented(l, true)]).collect();
    let mut visits = 0usize;
    for (l, _) in loops.iter().filter(|(_, o)| !*o) {
        let Some(probe) = l.first().map(|v| v.p) else { continue };
        let mut best: Option<usize> = None;
        for (i, ring) in rings.iter().enumerate() {
            visits = visits.saturating_add(ring.len());
            if visits > MAX_NESTING_VISITS {
                return Err(err("the region is too complex to write"));
            }
            let smaller = |b: usize| areas.get(i).zip(areas.get(b)).is_some_and(|(a, b)| a < b);
            if point_in_polygon(ring, probe) && best.is_none_or(smaller) {
                best = Some(i);
            }
        }
        if let Some(f) = best.and_then(|b| faces.get_mut(b)) {
            f.push(oriented(l, false));
        }
    }
    Ok(faces)
}

/// The records of a region whose loops are `loops` (each its vertices and whether it is an outer
/// loop), on the XY plane at z = 0.
pub(super) fn region_records(loops: &[(Vec<PolyVertex>, bool)]) -> Result<Vec<Record>> {
    if loops.len() > MAX_REGION_LOOPS || loops.iter().map(|(l, _)| l.len()).sum::<usize>() > MAX_REGION_EDGES {
        return Err(err("the region is too large to write"));
    }
    let mut m = Model::default();
    for face in faces_of(loops)? {
        let mut face_loops: Vec<Vec<Use>> = Vec::new();
        for l in face {
            let n = l.len();
            if n < 2 || (Polyline { vertices: l.clone(), closed: true }).area().abs() <= 0.0 {
                continue;
            }
            let first_point = m.points.len();
            m.points.extend(l.iter().map(|v| [v.p.x, v.p.y, 0.0]));
            let mut uses = Vec::with_capacity(n);
            for (k, v) in l.iter().enumerate() {
                let Some(w) = l.get((k + 1) % n) else { continue };
                let curve = match bulge_to_arc(v.p, w.p, v.bulge) {
                    Some((arc, ccw)) => {
                        let major = [(v.p.x - arc.center.x), (v.p.y - arc.center.y), 0.0];
                        ACurve::Ellipse {
                            center: [arc.center.x, arc.center.y, 0.0],
                            normal: [0.0, 0.0, if ccw { 1.0 } else { -1.0 }],
                            major,
                            ratio: 1.0,
                        }
                    }
                    None => {
                        let d = w.p - v.p;
                        let len = d.len();
                        if !(len.is_finite() && len > 0.0) {
                            continue;
                        }
                        ACurve::Straight { root: [v.p.x, v.p.y, 0.0], dir: [d.x / len, d.y / len, 0.0] }
                    }
                };
                m.edges.push(AEdge { start: first_point + k, end: first_point + (k + 1) % n, curve: Some(curve), pcurves: Vec::new() });
                uses.push(Use { edge: m.edges.len() - 1, reversed: false, pcurve: None });
            }
            if !uses.is_empty() {
                face_loops.push(uses);
            }
        }
        if face_loops.is_empty() {
            continue;
        }
        let plane = Record {
            name: "plane-surface",
            fields: vec![
                Field::Pos([0.0; 3]),
                Field::Vector([0.0, 0.0, 1.0]),
                Field::Vector([1.0, 0.0, 0.0]),
                Field::ForwardV,
                Field::Infinite,
                Field::Infinite,
                Field::Infinite,
                Field::Infinite,
            ],
        };
        m.lumps.push(vec![vec![AFace { surface: plane, reversed: false, loops: face_loops, spline: false, double: true }]]);
    }
    if m.lumps.is_empty() {
        return Err(err("the region has no area to write"));
    }
    layout(&m)
}
