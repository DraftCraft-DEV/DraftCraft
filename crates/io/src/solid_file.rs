//! How a DraftCraft 3D solid is kept in DXF and DWG files until DraftCraft writes ACIS
//! (docs/solids.md, "Interim file form"): a polyface mesh every CAD program can see, and in the
//! mesh's extension dictionary an XRECORD keyed `DRAFTCRAFT_SOLID` holding the exact body in
//! solidcore's exact `.smb` format (behind the 8-byte header `DCSMB001`), its placement and its counts. DraftCraft (built with solids)
//! reopens the mesh as the solid; other programs, and DraftCraft without solids, see the mesh.
//!
//! A mesh larger than a polyface's 16-bit counts is written as several meshes in a row; the first
//! carries the record, which says how many there are and holds a fingerprint of each (counts, face
//! indices, bounding box). The solid comes back only when every mesh still matches its
//! fingerprint: a mesh edited, moved or copied elsewhere (AutoCAD copies the record with it) stays
//! the mesh it now is, and no other mesh is ever taken for a part.

use std::collections::HashMap;

use draftcraft_doc::{PolyfaceMesh, Solid3d};
use draftcraft_geom::Vec3;

/// The extension dictionary key of the record.
pub(crate) const KEY: &str = "DRAFTCRAFT_SOLID";
/// The record's first group (1): what it is and its layout's version.
pub(crate) const MARK: &str = "DraftCraft solid 2";
/// Bytes per binary chunk (group 310), as AutoCAD writes them.
const CHUNK: usize = 127;
/// Most vertices or faces of one polyface mesh (16-bit counts).
const MAX_MESH: usize = 32_767;

/// The meshes AutoCAD sees, in drawing coordinates: shared corners merged, each triangle a face,
/// and the edges inside a face of the solid hidden (negative indices), so only the solid's own
/// edges show.
pub(crate) fn meshes(so: &Solid3d) -> Vec<PolyfaceMesh> {
    let look = &so.look;
    // Edges inside a face: shared by two triangles of that face.
    let key = |p: Vec3| (p.x.to_bits(), p.y.to_bits(), p.z.to_bits());
    let mut shared: HashMap<((u64, u64, u64), (u64, u64, u64), u32), u32> = HashMap::new();
    // A mirrored placement turns the triangles inside out: wind them back.
    let flip = so.mirrored();
    let tris: Vec<([Vec3; 3], u32)> = look
        .triangles
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let [a, b, c] = look.triangle(t)?;
            Some((if flip { [a, c, b] } else { [a, b, c] }, look.tri_face.get(i).copied().unwrap_or(u32::MAX)))
        })
        .collect();
    let edge = |a: Vec3, b: Vec3, f: u32| {
        let (ka, kb) = (key(a), key(b));
        if ka <= kb { (ka, kb, f) } else { (kb, ka, f) }
    };
    for (c, f) in &tris {
        for k in 0..3 {
            let (a, b) = (c[k], c[(k + 1) % 3]);
            *shared.entry(edge(a, b, *f)).or_insert(0) += 1;
        }
    }
    let mut out = Vec::new();
    let mut cur = PolyfaceMesh { vertices: Vec::new(), faces: Vec::new() };
    let mut index: HashMap<(u64, u64, u64), i32> = HashMap::new();
    for (c, f) in &tris {
        if cur.vertices.len() + 3 > MAX_MESH || cur.faces.len() + 1 > MAX_MESH {
            out.push(std::mem::replace(&mut cur, PolyfaceMesh { vertices: Vec::new(), faces: Vec::new() }));
            index.clear();
        }
        let mut face = [0i32; 4];
        for k in 0..3 {
            let p = c[k];
            let ix = *index.entry(key(p)).or_insert_with(|| {
                cur.vertices.push(so.world(p));
                i32::try_from(cur.vertices.len()).unwrap_or(i32::MAX)
            });
            let hidden = shared.get(&edge(p, c[(k + 1) % 3], *f)).is_some_and(|n| *n >= 2);
            face[k] = if hidden { -ix } else { ix };
        }
        cur.faces.push(face);
    }
    if !cur.faces.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

/// A mesh's fingerprint as the record keeps it (group 300): vertex count, face count, a hash of
/// the faces' indices and the bounding box, of the faces a file keeps (the others are dropped on
/// write and on read).
pub(crate) fn fingerprint(m: &PolyfaceMesh) -> String {
    let (v, f, hash, lo, hi) = print_of(m);
    format!("{v} {f} {hash:016X} {} {} {} {} {} {}", lo.x, lo.y, lo.z, hi.x, hi.y, hi.z)
}

/// Whether a mesh still matches a fingerprint from a record: the same counts and faces, and the
/// same box up to the digits a file keeps.
#[cfg_attr(not(feature = "solids"), allow(dead_code))]
pub(crate) fn matches(print: &str, m: &PolyfaceMesh) -> bool {
    let w: Vec<&str> = print.split_whitespace().collect();
    if w.len() != 9 {
        return false;
    }
    let (v, f, hash, lo, hi) = print_of(m);
    let num = |i: usize| w.get(i).and_then(|t| t.parse::<f64>().ok()).filter(|x| x.is_finite());
    let same_ints = w.first().and_then(|t| t.parse::<usize>().ok()) == Some(v)
        && w.get(1).and_then(|t| t.parse::<usize>().ok()) == Some(f)
        && w.get(2).and_then(|t| u64::from_str_radix(t, 16).ok()) == Some(hash);
    if !same_ints {
        return false;
    }
    let got = [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z];
    got.iter().enumerate().all(|(i, g)| num(i + 3).is_some_and(|x| (x - g).abs() <= 1e-9 * (1.0 + x.abs().max(g.abs()))))
}

fn print_of(m: &PolyfaceMesh) -> (usize, usize, u64, Vec3, Vec3) {
    // FNV-1a over the kept faces' indices.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut f = 0usize;
    for face in m.faces.iter().filter(|q| m.face3d(q).is_some()) {
        f += 1;
        for b in face.iter().flat_map(|i| i.to_le_bytes()) {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in &m.vertices {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    if m.vertices.is_empty() {
        (lo, hi) = (Vec3::ZERO, Vec3::ZERO);
    }
    (m.vertices.len(), f, hash, lo, hi)
}

/// The record's groups after its cloning flag (280): mark, mesh count, counts, placement, body,
/// and each mesh's fingerprint.
pub(crate) fn record_groups(so: &Solid3d, parts: &[PolyfaceMesh]) -> Vec<(i32, Value)> {
    let mut g = vec![(1, Value::S(MARK.into())), (90, Value::I(i64::try_from(parts.len()).unwrap_or(1)))];
    g.push((91, Value::I(i64::from(so.faces))));
    g.push((92, Value::I(i64::from(so.edges))));
    g.push((140, Value::F(so.volume)));
    for row in &so.placement.m {
        for v in row {
            g.push((40, Value::F(*v)));
        }
    }
    g.push((93, Value::I(i64::try_from(so.body.len()).unwrap_or(0))));
    for c in so.body.chunks(CHUNK) {
        g.push((310, Value::S(c.iter().map(|b| format!("{b:02X}")).collect())));
    }
    for m in parts {
        g.push((300, Value::S(fingerprint(m))));
    }
    g
}

/// A group's value.
pub(crate) enum Value {
    S(String),
    I(i64),
    F(f64),
}

#[cfg(feature = "solids")]
pub(crate) use recover::recover;

#[cfg(feature = "solids")]
mod recover {
    use std::sync::Arc;

    use draftcraft_doc::foreign::Foreign;
    use draftcraft_doc::{Drawing, EntityKind, EntityStore, Handle, PolyfaceMesh, RawTag, Solid3d, SolidLook};
    use draftcraft_geom::Mat4;

    use super::{KEY, MARK};

    use draftcraft_solid::Body;

    /// Most triangles of an exact look worked out on open (larger ones keep the mesh's look).
    const MAX_LOOK_TRIANGLES: usize = 200_000;

    /// Most bytes of one solid's body read back (the writer has no such solids; a file may claim it).
    const MAX_BODY: usize = 256 << 20;

    /// What a record holds.
    struct Record {
        faces: u32,
        edges: u32,
        volume: f64,
        placement: Mat4,
        body: Vec<u8>,
        /// One fingerprint per mesh, in order.
        prints: Vec<String>,
    }

    fn parse(tags: &[RawTag]) -> Option<Record> {
        if !tags.iter().any(|t| t.code == 1 && t.value.trim() == MARK) {
            return None;
        }
        let int = |code: i32| tags.iter().find(|t| t.code == code).and_then(|t| t.value.trim().parse::<i64>().ok());
        let reals: Vec<f64> = tags.iter().filter(|t| t.code == 40).filter_map(|t| t.value.trim().parse::<f64>().ok()).collect();
        if reals.len() != 16 || reals.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let mut m = [[0.0; 4]; 4];
        for (i, v) in reals.iter().enumerate() {
            *m.get_mut(i / 4)?.get_mut(i % 4)? = *v;
        }
        let len = usize::try_from(int(93)?).ok().filter(|n| *n <= MAX_BODY)?;
        let mut body = Vec::with_capacity(len);
        for t in tags.iter().filter(|t| t.code == 310) {
            let s = t.value.trim().as_bytes();
            if s.len() % 2 != 0 {
                return None;
            }
            for pair in s.chunks(2) {
                let txt = std::str::from_utf8(pair).ok()?;
                body.push(u8::from_str_radix(txt, 16).ok()?);
                if body.len() > len {
                    return None;
                }
            }
        }
        if body.len() != len {
            return None;
        }
        let parts = usize::try_from(int(90).unwrap_or(1)).ok().filter(|n| (1..=10_000).contains(n))?;
        let volume = tags.iter().find(|t| t.code == 140).and_then(|t| t.value.trim().parse::<f64>().ok()).filter(|v| v.is_finite()).unwrap_or(0.0);
        let count = |code| u32::try_from(int(code).unwrap_or(0)).unwrap_or(0);
        let prints: Vec<String> = tags.iter().filter(|t| t.code == 300).map(|t| t.value.trim().to_string()).take(parts.saturating_add(1)).collect();
        if prints.len() != parts {
            return None;
        }
        Some(Record { faces: count(91), edges: count(92), volume, placement: Mat4 { m }, body, prints })
    }

    /// The record an entity's extension dictionary holds under [`KEY`]: the dictionary's and the
    /// record's handles, and the record.
    fn find(f: &Foreign, h: Handle) -> Option<(Handle, Handle, Record)> {
        let dh = f.entities.get(&h)?.xdict?;
        let dict = f.objects.iter().find(|o| o.handle == dh)?;
        let at = dict.tags.iter().position(|t| t.code == 3 && t.value.trim().eq_ignore_ascii_case(KEY))?;
        let xh = dict.tags.get(at + 1).filter(|t| matches!(t.code, 350 | 360)).and_then(|t| Handle::parse_hex(t.value.trim()))?;
        let rec = f.objects.iter().find(|o| o.handle == xh && o.kind.eq_ignore_ascii_case("XRECORD"))?;
        Some((dh, xh, parse(&rec.tags)?))
    }

    /// The inverse of an affine placement; `None` when it flattens.
    fn inverse(m: &Mat4) -> Option<Mat4> {
        let a = &m.m;
        let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
        if !(det.is_finite() && det.abs() > 1e-300) {
            return None;
        }
        let k = 1.0 / det;
        let mut r = [[0.0; 4]; 4];
        r[0][0] = (a[1][1] * a[2][2] - a[1][2] * a[2][1]) * k;
        r[0][1] = (a[0][2] * a[2][1] - a[0][1] * a[2][2]) * k;
        r[0][2] = (a[0][1] * a[1][2] - a[0][2] * a[1][1]) * k;
        r[1][0] = (a[1][2] * a[2][0] - a[1][0] * a[2][2]) * k;
        r[1][1] = (a[0][0] * a[2][2] - a[0][2] * a[2][0]) * k;
        r[1][2] = (a[0][2] * a[1][0] - a[0][0] * a[1][2]) * k;
        r[2][0] = (a[1][0] * a[2][1] - a[1][1] * a[2][0]) * k;
        r[2][1] = (a[0][1] * a[2][0] - a[0][0] * a[2][1]) * k;
        r[2][2] = (a[0][0] * a[1][1] - a[0][1] * a[1][0]) * k;
        for row in r.iter_mut().take(3) {
            row[3] = -(row[0] * a[0][3] + row[1] * a[1][3] + row[2] * a[2][3]);
        }
        r[3][3] = 1.0;
        let out = Mat4 { m: r };
        out.m.iter().flatten().all(|v| v.is_finite()).then_some(out)
    }

    /// A look from the meshes: the triangles, and the visible edges as the solid's edges. The
    /// engine works out the exact look again from the body when it has the kernel.
    fn look(meshes: &[&PolyfaceMesh], inv: &Mat4) -> SolidLook {
        let mut l = SolidLook::default();
        for m in meshes {
            let base = u32::try_from(l.positions.len()).unwrap_or(u32::MAX);
            l.positions.extend(m.vertices.iter().map(|p| inv.apply(*p)));
            let n = m.vertices.len();
            for (fi, f) in m.faces.iter().enumerate() {
                let ix: Vec<usize> =
                    f.iter().filter(|i| **i != 0).filter_map(|i| usize::try_from(i.unsigned_abs()).ok()).filter(|i| (1..=n).contains(i)).collect();
                if ix.len() < 3 {
                    continue;
                }
                let at = |i: usize| base.saturating_add(u32::try_from(i - 1).unwrap_or(0));
                for k in 1..ix.len() - 1 {
                    if let (Some(a), Some(b), Some(c)) = (ix.first(), ix.get(k), ix.get(k + 1)) {
                        l.triangles.push([at(*a), at(*b), at(*c)]);
                        l.tri_face.push(u32::try_from(fi).unwrap_or(u32::MAX));
                    }
                }
                let raw: Vec<i32> = f.iter().copied().filter(|i| *i != 0).collect();
                for k in 0..raw.len() {
                    let (Some(a), Some(b)) = (raw.get(k), raw.get((k + 1) % raw.len())) else { continue };
                    if *a < 0 {
                        continue;
                    }
                    let p = |i: i32| usize::try_from(i.unsigned_abs()).ok().and_then(|i| m.vertices.get(i.checked_sub(1)?)).map(|p| inv.apply(*p));
                    if let (Some(pa), Some(pb)) = (p(*a), p(*b)) {
                        l.wires.push(vec![pa, pb]);
                    }
                }
            }
        }
        l
    }

    /// The exact look of a body placed by `placement`, worked out from the body; `None` when the
    /// body doesn't decode or is too detailed to draw.
    fn exact_look(body: &[u8]) -> Option<SolidLook> {
        let d = Body::decode(body).ok()?.display(0.0).ok()?;
        (d.triangles.len() <= MAX_LOOK_TRIANGLES).then_some(SolidLook {
            wires: d.wires,
            positions: d.positions,
            triangles: d.triangles,
            tri_face: d.tri_face,
        })
    }

    fn recover_store(st: &mut EntityStore, f: &mut Foreign) {
        let order: Vec<Handle> = st.iter().map(|e| e.handle).collect();
        let mut gone: Vec<Handle> = Vec::new();
        for (i, h) in order.iter().enumerate() {
            if gone.contains(h) {
                continue;
            }
            let Some(e) = st.get(*h) else { continue };
            let EntityKind::PolyfaceMesh(first) = &e.kind else { continue };
            let Some((dh, xh, rec)) = find(f, *h) else { continue };
            let Some(inv) = inverse(&rec.placement) else { continue };
            // The mesh must still be the one written with the record: one edited, moved or copied
            // elsewhere stays a mesh.
            if !rec.prints.first().is_some_and(|p| super::matches(p, first)) {
                continue;
            }
            // The other meshes of the solid: each the mesh that matches its fingerprint, looked
            // for after the first and then anywhere, never one that is a part already or a
            // solid's own first mesh.
            let mut parts: Vec<Handle> = Vec::new();
            let mut complete = true;
            for print in rec.prints.iter().skip(1) {
                let after = order.iter().skip(i + 1).chain(order.iter().take(i));
                let found = after.copied().find(|c| {
                    c != h
                        && !gone.contains(c)
                        && !parts.contains(c)
                        && find(f, *c).is_none()
                        && matches!(st.get(*c).map(|e| &e.kind), Some(EntityKind::PolyfaceMesh(m)) if super::matches(print, m))
                });
                match found {
                    Some(c) => parts.push(c),
                    None => {
                        complete = false;
                        break;
                    }
                }
            }
            if !complete {
                continue;
            }
            let look = match exact_look(&rec.body) {
                Some(l) => l,
                None => {
                    let mut meshes: Vec<&PolyfaceMesh> = vec![first];
                    for p in &parts {
                        if let Some(EntityKind::PolyfaceMesh(m)) = st.get(*p).map(|e| &e.kind) {
                            meshes.push(m);
                        }
                    }
                    look(&meshes, &inv)
                }
            };
            let so = Solid3d {
                body: Arc::new(rec.body),
                placement: rec.placement,
                look: Arc::new(look),
                faces: rec.faces,
                edges: rec.edges,
                volume: rec.volume,
                body_entity: false,
            };
            if !so.placement_ok() {
                continue;
            }
            let _ = st.modify(*h, |e| e.kind = EntityKind::Solid3d(so));
            for p in parts {
                st.remove(p);
                gone.push(p);
            }
            // The record is the solid now: drop it, and the dictionary when nothing else is in it.
            f.objects.retain(|o| o.handle != xh);
            let mut empty = false;
            if let Some(d) = f.objects.iter_mut().find(|o| o.handle == dh) {
                let mut tags = Vec::with_capacity(d.tags.len());
                let mut it = d.tags.iter().peekable();
                while let Some(t) = it.next() {
                    if t.code == 3 && t.value.trim().eq_ignore_ascii_case(KEY) && it.peek().is_some_and(|n| matches!(n.code, 350 | 360)) {
                        it.next();
                        continue;
                    }
                    tags.push(t.clone());
                }
                empty = !tags.iter().any(|t| t.code == 3);
                d.tags = tags;
            }
            if empty {
                f.objects.retain(|o| o.handle != dh);
                if let Some(x) = f.entities.get_mut(h) {
                    x.xdict = None;
                    if x.is_empty() {
                        f.entities.remove(h);
                    }
                }
            }
        }
    }

    /// Turn the meshes DraftCraft saved for its solids back into solids, everywhere in the drawing.
    pub(crate) fn recover(d: &mut Drawing) {
        crate::solid_acis::adopt(d);
        if d.foreign.entities.is_empty() {
            return;
        }
        let mut f = (*d.foreign).clone();
        recover_store(&mut d.model, &mut f);
        for l in &mut d.layouts {
            recover_store(&mut l.entities, &mut f);
        }
        for b in d.blocks.values_mut() {
            if b.entities.iter().any(|e| matches!(e.kind, EntityKind::PolyfaceMesh(_)) && f.entities.contains_key(&e.handle)) {
                recover_store(&mut Arc::make_mut(b).entities, &mut f);
            }
        }
        d.foreign = Arc::new(f);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use draftcraft_doc::{Drawing, EntityKind, Solid3d, SolidLook};
    use draftcraft_geom::{Mat4, Vec3};

    /// A tetrahedron with a made-up body (the file form doesn't look inside it), placed far from
    /// the origin.
    fn tetra(body_len: usize) -> Solid3d {
        let positions = vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        let triangles = vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [0, 3, 2]];
        let wires = vec![vec![positions[0], positions[1]]];
        let look = SolidLook { wires, positions, triangles, tri_face: vec![0, 1, 2, 3] };
        let mut placement = Mat4::IDENTITY;
        placement.m[0][3] = 1.0e6;
        placement.m[1][3] = -2.5;
        placement.m[2][3] = 7.0;
        let body: Vec<u8> = (0..body_len).map(|i| (i * 31 % 256) as u8).collect();
        Solid3d { body: Arc::new(body), placement, look: Arc::new(look), faces: 4, edges: 6, volume: 1.0 / 6.0, body_entity: false }
    }

    fn drawing_with(so: Solid3d) -> Drawing {
        let mut d = Drawing::new_metric();
        d.add(&draftcraft_doc::Space::Model, draftcraft_doc::Common::default(), EntityKind::Solid3d(so)).unwrap();
        d
    }

    fn only_kind(d: &Drawing) -> Vec<EntityKind> {
        d.model.iter().map(|e| e.kind.clone()).collect()
    }

    #[test]
    fn the_mesh_hides_edges_inside_a_face() {
        let mut so = tetra(10);
        // Two triangles of one face: their shared edge is hidden.
        Arc::make_mut(&mut so.look).tri_face = vec![0, 0, 2, 3];
        let m = &super::meshes(&so)[0];
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.faces.len(), 4);
        let hidden = m.faces.iter().flatten().filter(|i| **i < 0).count();
        assert_eq!(hidden, 2, "{:?}", m.faces);
        assert!(m.vertices.iter().any(|p| (p.x - 1.0e6).abs() < 1e-9 && (p.z - 7.0).abs() < 1e-9));
    }

    #[test]
    fn a_mirrored_solid_s_mesh_faces_out() {
        // Review of PR 20: a mirrored solid's mesh was written inside out.
        for mirror in [false, true] {
            let mut so = tetra(10);
            if mirror {
                so.placement.m[0][0] = -1.0;
            }
            assert_eq!(so.mirrored(), mirror);
            let m = &super::meshes(&so)[0];
            let mid = m.vertices.iter().fold(Vec3::ZERO, |a, p| a + *p) * 0.25;
            for f in &m.faces {
                let c: Vec<Vec3> = f.iter().take(3).map(|i| m.vertices[i.unsigned_abs() as usize - 1]).collect();
                let n = (c[1] - c[0]).cross(c[2] - c[0]);
                let centre = (c[0] + c[1] + c[2]) * (1.0 / 3.0);
                assert!(n.dot(centre - mid) > 0.0, "mirror {mirror}: {f:?} faces in");
            }
        }
    }

    #[cfg(feature = "solids")]
    #[test]
    fn a_solid_comes_back_from_dxf_and_dwg() {
        let so = tetra(5000);
        let d = drawing_with(so.clone());
        let dxf = crate::write(&d, "solid.dxf").unwrap();
        let dwg = crate::write_dwg_own(&d, draftcraft_dxf::Version::R2018).unwrap().0;
        for (bytes, name) in [(dxf, "solid.dxf"), (dwg, "solid.dwg")] {
            let back = crate::read(&bytes, name).unwrap();
            let kinds = only_kind(&back);
            assert_eq!(kinds.len(), 1, "{name}: {kinds:?}");
            let EntityKind::Solid3d(b) = &kinds[0] else { panic!("{name}: {:?}", kinds[0]) };
            assert_eq!(b.body, so.body, "{name}");
            assert_eq!(b.placement, so.placement, "{name}");
            assert_eq!((b.faces, b.edges), (4, 6));
            assert_eq!(b.look.triangles.len(), 4);
            // The record is the solid now, not foreign data written twice.
            assert!(back.foreign.objects.iter().all(|o| !o.kind.eq_ignore_ascii_case("XRECORD")), "{name}");
            // And saving again keeps it.
            let again = crate::read(&crate::write(&back, "again.dxf").unwrap(), "again.dxf").unwrap();
            assert!(matches!(&only_kind(&again)[0], EntityKind::Solid3d(x) if x.body == so.body));
        }
    }

    #[cfg(feature = "solids")]
    #[test]
    fn a_large_solid_is_several_meshes_and_one_solid() {
        let mut so = tetra(100);
        // 40 000 separate triangles: more corners than one polyface holds.
        let mut look = SolidLook::default();
        for i in 0..40_000u32 {
            let x = f64::from(i);
            look.positions.extend([Vec3::new(x, 0.0, 0.0), Vec3::new(x + 0.5, 1.0, 0.0), Vec3::new(x, 0.0, 1.0)]);
            look.triangles.push([3 * i, 3 * i + 1, 3 * i + 2]);
            look.tri_face.push(i);
        }
        so.look = Arc::new(look);
        assert!(super::meshes(&so).len() >= 2);
        let back = crate::read(&crate::write(&drawing_with(so.clone()), "big.dxf").unwrap(), "big.dxf").unwrap();
        let kinds = only_kind(&back);
        assert_eq!(kinds.len(), 1);
        assert!(matches!(&kinds[0], EntityKind::Solid3d(x) if x.body == so.body && x.look.triangles.len() == 40_000));
    }

    #[cfg(not(feature = "solids"))]
    #[test]
    fn without_solids_the_mesh_keeps_its_record() {
        let so = tetra(300);
        let bytes = crate::write(&drawing_with(so), "solid.dxf").unwrap();
        let back = crate::read(&bytes, "solid.dxf").unwrap();
        assert!(matches!(&only_kind(&back)[0], EntityKind::PolyfaceMesh(_)));
        // Saved again, the record is still there for a DraftCraft with solids.
        let text = String::from_utf8_lossy(&crate::write(&back, "again.dxf").unwrap()).to_string();
        assert!(text.contains(super::KEY) && text.contains(super::MARK));
    }

    #[cfg(feature = "solids")]
    #[test]
    fn a_damaged_record_leaves_a_mesh_and_no_crash() {
        let text = String::from_utf8(crate::write(&drawing_with(tetra(300)), "solid.dxf").unwrap()).unwrap().replace("\r\n", "\n");
        let mark = super::MARK;
        for (i, bad) in [
            text.replacen("\n 93\n300\n", "\n 93\n999999999999\n", 1),
            text.replacen("\n 93\n300\n", "\n 93\n299\n", 1),
            // A placement group missing.
            text.replacen("\n 40\n1000000.0\n", "\n", 1),
            text.replacen("\n310\n", "\n310\nZZ", 1),
            text.replacen(mark, "Something else", 1),
        ]
        .iter()
        .enumerate()
        {
            assert_ne!(*bad, text, "case {i}");
            let back = crate::read(bad.as_bytes(), "bad.dxf").unwrap();
            assert!(matches!(&only_kind(&back)[0], EntityKind::PolyfaceMesh(_)), "case {i}");
        }
    }

    /// The large tetra solid of [`a_large_solid_is_several_meshes_and_one_solid`]: two meshes.
    #[cfg(feature = "solids")]
    fn large() -> Solid3d {
        let mut so = tetra(100);
        let mut look = SolidLook::default();
        for i in 0..40_000u32 {
            let x = f64::from(i);
            look.positions.extend([Vec3::new(x, 0.0, 0.0), Vec3::new(x + 0.5, 1.0, 0.0), Vec3::new(x, 0.0, 1.0)]);
            look.triangles.push([3 * i, 3 * i + 1, 3 * i + 2]);
            look.tri_face.push(i);
        }
        so.look = Arc::new(look);
        so
    }

    #[cfg(feature = "solids")]
    #[test]
    fn a_mesh_edited_elsewhere_stays_the_mesh() {
        // Review of PR 20: a stale record brought the old solid back over edits made in another
        // program. Moving the far corners of the mesh changes its box: it stays a mesh.
        let text = String::from_utf8(crate::write(&drawing_with(tetra(300)), "solid.dxf").unwrap()).unwrap().replace("\r\n", "\n");
        let edited = text.replace("\n1000001.0\n", "\n1000002.0\n");
        assert_ne!(edited, text);
        let back = crate::read(edited.as_bytes(), "edited.dxf").unwrap();
        let kinds = only_kind(&back);
        assert_eq!(kinds.len(), 1);
        let EntityKind::PolyfaceMesh(m) = &kinds[0] else { panic!("{:?}", kinds[0]) };
        assert!(m.vertices.iter().any(|p| (p.x - 1_000_002.0).abs() < 1e-9), "the edit is kept");
        // A mesh moved as a whole (as AutoCAD's COPY moves the copy and its record) as well.
        let moved = text.replace("\n1000000.0\n", "\n1000010.0\n").replace("\n1000001.0\n", "\n1000011.0\n");
        // Put the placement back: only the mesh moved.
        let moved = moved.replacen("\n 40\n1000010.0\n", "\n 40\n1000000.0\n", 1);
        let back = crate::read(moved.as_bytes(), "moved.dxf").unwrap();
        assert!(matches!(&only_kind(&back)[0], EntityKind::PolyfaceMesh(_)));
    }

    #[cfg(feature = "solids")]
    #[test]
    fn a_missing_part_takes_no_other_mesh() {
        // Review of PR 20: the parts were the meshes that followed the first, whatever they were,
        // and were deleted. With one part gone, no solid comes back and every mesh stays.
        let mut d = drawing_with(large());
        let other = draftcraft_doc::PolyfaceMesh {
            vertices: vec![Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), Vec3::new(0.0, 5.0, 0.0)],
            faces: vec![[1, 2, 3, 0]],
        };
        d.add(&draftcraft_doc::Space::Model, draftcraft_doc::Common::default(), EntityKind::PolyfaceMesh(other.clone())).unwrap();
        let text = String::from_utf8(crate::write(&d, "big.dxf").unwrap()).unwrap().replace("\r\n", "\n");
        // Whole, it is the solid and the other mesh.
        let back = crate::read(text.as_bytes(), "big.dxf").unwrap();
        let kinds = only_kind(&back);
        assert_eq!(kinds.len(), 2, "{}", kinds.len());
        assert!(matches!(&kinds[0], EntityKind::Solid3d(_)));
        assert!(matches!(&kinds[1], EntityKind::PolyfaceMesh(m) if m.vertices == other.vertices));
        // Cut the second POLYLINE (one of the solid's other meshes) out.
        let tag = "\n  0\nPOLYLINE\n";
        let starts: Vec<usize> = text.match_indices(tag).map(|(i, _)| i).collect();
        assert!(starts.len() >= 3, "{}", starts.len());
        let cut = text[..starts[1]].to_string() + &text[starts[2]..];
        let back = crate::read(cut.as_bytes(), "cut.dxf").unwrap();
        let kinds = only_kind(&back);
        assert_eq!(kinds.len(), starts.len() - 1, "every mesh left stays");
        assert!(kinds.iter().all(|k| matches!(k, EntityKind::PolyfaceMesh(_))), "no solid from part of it");
        assert!(matches!(kinds.last(), Some(EntityKind::PolyfaceMesh(m)) if m.vertices == other.vertices));
    }

    #[cfg(feature = "solids")]
    #[test]
    fn an_interim_solid_opens_with_the_look_of_its_body() {
        // Review of PR 20: opening a file re-tessellated every solid. The look is now worked out
        // only for the solids that come back from their meshes, as they are read.
        let body = draftcraft_solid::box_solid(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0)).unwrap();
        let exact = body.display(0.0).unwrap();
        let mut so = tetra(1);
        so.body = Arc::new(body.encode().unwrap());
        so.placement = Mat4::IDENTITY;
        let text = crate::dxf_write::write_interim(&drawing_with(so));
        let back = crate::read(text.as_bytes(), "box.dxf").unwrap();
        let EntityKind::Solid3d(b) = &only_kind(&back)[0] else { panic!("not a solid") };
        assert_eq!(b.look.triangles, exact.triangles);
        assert_eq!(b.look.wires.len(), exact.wires.len());
    }
}
