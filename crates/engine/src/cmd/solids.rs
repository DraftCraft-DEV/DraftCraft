//! 3D solids (cargo feature `solids`, docs/solids.md): the commands that make, combine, edit,
//! measure and draw solids, by AutoCAD's names.
//!
//! Every kernel call goes through `draftcraft-solid`. A solid's body is kept in its own
//! coordinates near the origin, with a placement saying where it sits in the drawing ("Units and
//! scale" in docs/solids.md), so parts far from the origin keep the kernel's tolerances.

use draftcraft_doc::{Entity, EntityKind, Handle, Solid3d, SolidLook};
use draftcraft_geom::{Mat4, Vec2, Vec3};
use draftcraft_solid::{self as solid, Body};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::{EngineError, Result, Session};

mod edit;
pub(crate) mod look;
mod prims;
mod sweep;
#[cfg(test)]
mod tests;
mod views;

pub(crate) use edit::{try_boolean, try_massprop, try_subtract};

/// Whether a solid is saved as AutoCAD's own 3DSOLID (its body written as ACIS); the others are
/// saved as meshes with the exact body attached (docs/solids.md, "The ACIS translator").
pub(crate) fn saves_as_acis(so: &Solid3d) -> bool {
    let Ok(body) = Body::decode(&so.body) else { return false };
    let body = if is_identity(&so.placement) { Ok(body) } else { body.transformed(&so.placement) };
    body.is_ok_and(|b| solid::acis::write_sat(&b).is_ok())
}

/// The solids commands.
pub fn specs() -> Vec<CommandSpec> {
    let mut v = prims::specs();
    v.extend(sweep::specs());
    v.extend(edit::specs());
    v.extend(views::specs());
    v.extend(look::specs());
    v
}

pub(crate) fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

/// A kernel error as the command's error.
pub(crate) fn kernel(e: solid::SolidError) -> EngineError {
    EngineError::Other(e.0)
}

/// A 3D point parameter: `[x, y]` (at Z 0) or `[x, y, z]`.
pub(crate) fn point3(v: &Value) -> Option<Vec3> {
    let a = v.as_array()?;
    let x = a.first()?.as_f64()?;
    let y = a.get(1)?.as_f64()?;
    let z = match a.get(2) {
        None | Some(Value::Null) => 0.0,
        Some(z) => z.as_f64()?,
    };
    let p = Vec3::new(x, y, z);
    p.is_finite().then_some(p)
}

pub(crate) fn point3_req(cmd: &str, p: &Value, key: &str) -> Result<Vec3> {
    p.get(key).and_then(point3).ok_or_else(|| super::bad(cmd, format!("`{key}` ([x, y] or [x, y, z]) is required")))
}

pub(crate) fn translation(o: Vec3) -> Mat4 {
    let mut m = Mat4::IDENTITY;
    m.m[0][3] = o.x;
    m.m[1][3] = o.y;
    m.m[2][3] = o.z;
    m
}

/// The inverse of an affine placement; `None` when it flattens.
pub(crate) fn inverse(m: &Mat4) -> Option<Mat4> {
    let a = &m.m;
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    if !(det.is_finite() && det.abs() > 1e-300) {
        return None;
    }
    let inv = 1.0 / det;
    let mut r = [[0.0; 4]; 4];
    r[0][0] = (a[1][1] * a[2][2] - a[1][2] * a[2][1]) * inv;
    r[0][1] = (a[0][2] * a[2][1] - a[0][1] * a[2][2]) * inv;
    r[0][2] = (a[0][1] * a[1][2] - a[0][2] * a[1][1]) * inv;
    r[1][0] = (a[1][2] * a[2][0] - a[1][0] * a[2][2]) * inv;
    r[1][1] = (a[0][0] * a[2][2] - a[0][2] * a[2][0]) * inv;
    r[1][2] = (a[0][2] * a[1][0] - a[0][0] * a[1][2]) * inv;
    r[2][0] = (a[1][0] * a[2][1] - a[1][1] * a[2][0]) * inv;
    r[2][1] = (a[0][1] * a[2][0] - a[0][0] * a[2][1]) * inv;
    r[2][2] = (a[0][0] * a[1][1] - a[0][1] * a[1][0]) * inv;
    for row in r.iter_mut().take(3) {
        row[3] = -(row[0] * a[0][3] + row[1] * a[1][3] + row[2] * a[2][3]);
    }
    r[3][3] = 1.0;
    let out = Mat4 { m: r };
    out.m.iter().flatten().all(|v| v.is_finite()).then_some(out)
}

pub(crate) fn det3(m: &Mat4) -> f64 {
    let a = &m.m;
    a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
}

/// Most triangles kept to draw one solid (and to save it as a mesh AutoCAD can see).
pub(crate) const MAX_LOOK_TRIANGLES: usize = 200_000;

/// The entity for `body` (in its own coordinates) placed by `placement`.
pub(crate) fn solid_kind(body: &Body, placement: Mat4) -> Result<EntityKind> {
    let bytes = body.encode().map_err(kernel)?;
    let d = body.display(0.0).map_err(kernel)?;
    if d.triangles.len() > MAX_LOOK_TRIANGLES {
        return Err(other("The solid is too detailed to draw."));
    }
    let volume = body.mass().map(|m| m.volume).unwrap_or(0.0) * det3(&placement).abs();
    let look = SolidLook { wires: d.wires, positions: d.positions, triangles: d.triangles, tri_face: d.tri_face };
    Ok(EntityKind::Solid3d(Solid3d {
        body: std::sync::Arc::new(bytes),
        placement,
        look: std::sync::Arc::new(look),
        faces: u32::try_from(body.face_count()).unwrap_or(u32::MAX),
        edges: u32::try_from(body.edge_count()).unwrap_or(u32::MAX),
        volume,
        body_entity: false,
    }))
}

/// A body made in drawing coordinates, kept relative to `origin` (so the placement is a move to
/// it): `make` receives `origin` and builds the body around the drawing origin.
pub(crate) fn placed(origin: Vec3, make: impl FnOnce(Vec3) -> solid::Result<Body>) -> Result<EntityKind> {
    let body = make(origin).map_err(kernel)?;
    solid_kind(&body, translation(origin))
}

/// The solid's body moved into the coordinates of `frame` (a placement): `inverse(frame) *
/// placement` applied to it.
pub(crate) fn body_in(sol: &Solid3d, frame: &Mat4) -> Result<Body> {
    let body = Body::decode(&sol.body).map_err(|e| other(format!("This solid's data can't be read: {e}")))?;
    let inv = inverse(frame).ok_or_else(|| other("The solid's placement is not valid."))?;
    let m = inv.mul(&sol.placement);
    if is_identity(&m) { Ok(body) } else { body.transformed(&m).map_err(kernel) }
}

fn is_identity(m: &Mat4) -> bool {
    m.m.iter().flatten().zip(Mat4::IDENTITY.m.iter().flatten()).all(|(a, b)| (a - b).abs() <= 1e-15)
}

/// The solids among `hs`, with their handles.
pub(crate) fn solids(s: &Session, hs: &[Handle]) -> Result<Vec<(Handle, Solid3d)>> {
    let d = s.doc()?;
    Ok(hs
        .iter()
        .filter_map(|h| match &d.entity(*h)?.kind {
            EntityKind::Solid3d(so) => Some((*h, so.clone())),
            _ => None,
        })
        .collect())
}

/// Replace entity `h`'s geometry, keeping its layer and other properties.
pub(crate) fn replace(s: &mut Session, h: Handle, kind: EntityKind) -> Result<()> {
    s.doc_mut()?.modify_entity(h, |e| e.kind = kind)?;
    Ok(())
}

/// Add `kind` with the properties of entity `like` (layer, colour, …).
pub(crate) fn add_like(s: &mut Session, like: Handle, kind: EntityKind) -> Result<Handle> {
    let common = s.doc()?.entity(like).map(|e| e.common.clone());
    let h = s.add_entity(kind)?;
    if let Some(c) = common {
        s.doc_mut()?.modify_entity(h, |e: &mut Entity| e.common = c)?;
    }
    Ok(h)
}

pub(crate) fn added(h: Handle) -> Result<Value> {
    Ok(json!({ "handle": h.hex() }))
}

/// Looking straight down: the line of sight of the plan view.
pub(crate) const DOWN: Vec3 = Vec3 { x: 0.0, y: 0.0, z: -1.0 };

/// The line of sight through a point given at a prompt: that point (in the XY plane, or at the
/// typed Z) and the current view's direction, straight down in plan. In a 3D view a click is
/// the point of the XY plane seen there, so the line passes through what was clicked.
pub(crate) fn sight(s: &Session, p: Vec2) -> (Vec3, Vec3) {
    let dir = s.view3d().map_or(DOWN, |v| v.dir);
    (p.to3(s.point_z.unwrap_or(0.0)), dir)
}

/// Screen right and up for a view along `dir` (as [`crate::view3d::View3d::axes`]).
fn view_axes(dir: Vec3) -> (Vec3, Vec3) {
    let mut up = Vec3::Z - dir * dir.dot(Vec3::Z);
    if up.len() < 1e-9 {
        up = Vec3::Y;
    }
    let x = dir.cross(up).normalized();
    (x, x.cross(dir).normalized())
}

/// The solid edge seen nearest the line of sight through `at` along `dir` (an edge in front
/// winning over one behind it at the same place), among `hs` (all solids when empty): the
/// solid, and the 3D point of the edge seen there, in drawing coordinates.
pub(crate) fn pick_edge(s: &Session, at: Vec3, dir: Vec3, hs: &[Handle]) -> Result<Option<(Handle, Vec3)>> {
    let d = s.doc()?;
    let dir = dir.normalized();
    let (x, y) = view_axes(dir);
    let on = |q: Vec3| Vec2::new(q.dot(x), q.dot(y));
    let p = on(at);
    // (distance on screen, depth, solid, point)
    let mut best: Option<(f64, f64, Handle, Vec3)> = None;
    let Some(store) = d.space(&s.space()) else { return Ok(None) };
    for e in store.iter() {
        let EntityKind::Solid3d(so) = &e.kind else { continue };
        if !hs.is_empty() && !hs.contains(&e.handle) {
            continue;
        }
        for w in so.world_wires() {
            for seg in w.windows(2) {
                let (Some(a), Some(b)) = (seg.first(), seg.get(1)) else { continue };
                let (a2, b2) = (on(*a), on(*b));
                let ab = b2 - a2;
                let t = if ab.dot(ab) > 0.0 { ((p - a2).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0) } else { 0.0 };
                let dist = (a2 + ab * t).dist(p);
                let q = *a + (*b - *a) * t;
                let depth = q.dot(dir);
                if !(dist.is_finite() && depth.is_finite()) {
                    continue;
                }
                let better = best.is_none_or(|(bd, bz, ..)| {
                    let tol = 1e-7 * (1.0 + bd.abs());
                    dist < bd - tol || (dist <= bd + tol && depth < bz)
                });
                if better {
                    best = Some((dist, depth, e.handle, q));
                }
            }
        }
    }
    Ok(best.map(|(_, _, h, q)| (h, q)))
}

/// The point where the line of sight through `at` along `dir` first meets solid `so`'s faces
/// (the face nearest the viewer), in drawing coordinates.
pub(crate) fn pick_face(so: &Solid3d, at: Vec3, dir: Vec3) -> Option<Vec3> {
    let dir = dir.normalized();
    let (x, y) = view_axes(dir);
    let on = |q: Vec3| Vec2::new(q.dot(x), q.dot(y));
    let p = on(at);
    let mut best: Option<(f64, Vec3)> = None;
    for t in &so.look.triangles {
        let Some(local) = so.look.triangle(t) else { continue };
        let [a, b, c] = local.map(|q| so.world(q));
        let (a2, b2, c2) = (on(a), on(b), on(c));
        let den = (b2 - a2).cross(c2 - a2);
        if den.abs() < 1e-300 {
            continue;
        }
        let u = (p - a2).cross(c2 - a2) / den;
        let v = (b2 - a2).cross(p - a2) / den;
        if u < -1e-9 || v < -1e-9 || u + v > 1.0 + 1e-9 {
            continue;
        }
        let q = a + (b - a) * u + (c - a) * v;
        let depth = q.dot(dir);
        if depth.is_finite() && best.is_none_or(|(bz, _)| depth < bz) {
            best = Some((depth, q));
        }
    }
    best.map(|(_, q)| q)
}
