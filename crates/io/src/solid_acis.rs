//! 3D solids and regions as AutoCAD keeps them: a 3DSOLID, REGION or BODY entity whose shape is ACIS data (docs/solids.md,
//! "The ACIS translator"). Up to AutoCAD 2010 the entity holds it as SAT text in its groups 1
//! and 3, scrambled; from AutoCAD 2013 the file's data store holds it as SAB, keyed by the
//! entity's handle, and the entity holds a GUID.
//!
//! - Opening a file, every 3DSOLID whose ACIS data the kernel reads becomes a DraftCraft solid
//!   ([`adopt`]); one it can't read stays as AutoCAD saved it, and the drawing's read notes say
//!   how many and why.
//! - Saving, every DraftCraft solid the ACIS writer can describe is written as AutoCAD's own
//!   3DSOLID for the file's version ([`for_saving`]); one it can't (a body with free-form
//!   surfaces, not written as ACIS yet) keeps the interim form, a mesh with the exact body
//!   (`solid_file`).
//! - A solid or region read from a file and saved unchanged is written with AutoCAD's own
//!   description (kept in [`draftcraft_doc::foreign::AcisSource`]) when the file's version holds
//!   it as it is: same form, no older ACIS reader. Its history, face colours and attributes then
//!   survive the round trip; an edited, copied or down-saved one gets a new description.
//! - A REGION (a flat sheet of ACIS faces) opens as a DraftCraft region when its faces lie on
//!   the XY plane and its edges are lines and arcs, and a region is saved as a REGION. A BODY
//!   holding a solid opens as a solid that is saved as a BODY again.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

use draftcraft_doc::foreign::{AcisSource, data_store_owner, data_store_records};
use draftcraft_doc::{Drawing, Entity, EntityKind, EntityStore, Handle, HatchLoop, RawTag, Region, Solid3d, SolidLook, Unknown};
use draftcraft_dxf::Version;
use draftcraft_geom::Mat4;
use draftcraft_solid::{Body, acis};

/// Most solids converted in one drawing, either way (a file may hold any number).
const MAX_SOLIDS: usize = 100_000;
/// Most triangles kept to draw one solid read from a file.
const MAX_LOOK_TRIANGLES: usize = 200_000;
/// Most characters of a DXF group value.
const GROUP_CHARS: usize = 255;

fn tag(code: i32, value: impl Into<String>) -> RawTag {
    RawTag { code, value: value.into() }
}

/// The solid modeler entities: AutoCAD's 3DSOLID, REGION and BODY.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Modeler {
    Solid,
    Region,
    Body,
}

impl Modeler {
    fn of(u: &Unknown) -> Option<Modeler> {
        let t = u.dxf_type.trim();
        [("3DSOLID", Modeler::Solid), ("REGION", Modeler::Region), ("BODY", Modeler::Body)]
            .into_iter()
            .find_map(|(name, m)| t.eq_ignore_ascii_case(name).then_some(m))
    }

    /// "1 3D solid is", "2 regions are"…
    fn count(self, n: usize) -> String {
        let (one, many) = match self {
            Modeler::Solid => ("3D solid", "3D solids"),
            Modeler::Region => ("region", "regions"),
            Modeler::Body => ("body", "bodies"),
        };
        if n == 1 { format!("1 {one} is") } else { format!("{n} {many} are") }
    }
}

/// Every entity store of the drawing: model space, the layouts, the block definitions.
fn for_each_store(d: &mut Drawing, mut f: impl FnMut(&mut EntityStore)) {
    f(&mut d.model);
    for l in &mut d.layouts {
        f(&mut l.entities);
    }
    for b in d.blocks.values_mut() {
        if b.entities.iter().any(|e| matches!(e.kind, EntityKind::Unknown(_) | EntityKind::Solid3d(_) | EntityKind::Region(_))) {
            f(&mut Arc::make_mut(b).entities);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Opening

/// The SAT text an entity's groups hold: each group 1 a line, continued by groups 3, with DXF's
/// caret escape undone. Still scrambled as stored; the reader unscrambles it.
fn sat_text(tags: &[RawTag]) -> Option<Vec<u8>> {
    let own: Vec<&RawTag> = tags.iter().take_while(|t| t.code < 1000).collect();
    let from = own.iter().position(|t| t.code == 100 && t.value.trim() == "AcDbModelerGeometry").map_or(0, |i| i + 1);
    let mut lines: Vec<String> = Vec::new();
    for t in own.iter().skip(from) {
        match t.code {
            100 => break,
            1 => lines.push(t.value.replace("^ ", "^")),
            3 => match lines.last_mut() {
                Some(l) => l.push_str(&t.value.replace("^ ", "^")),
                None => lines.push(t.value.replace("^ ", "^")),
            },
            _ => {}
        }
    }
    if lines.is_empty() {
        return None;
    }
    let mut text = lines.join("\n");
    text.push('\n');
    Some(text.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect())
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim().as_bytes();
    if !s.len().is_multiple_of(2) {
        return None;
    }
    s.chunks(2).map(|p| std::str::from_utf8(p).ok().and_then(|p| u8::from_str_radix(p, 16).ok())).collect()
}

/// The SAB each data store record holds, by the entity it belongs to.
fn stored_sab(store: &[RawTag]) -> BTreeMap<Handle, Vec<u8>> {
    let mut out = BTreeMap::new();
    for rec in data_store_records(store) {
        let is_asm = rec.first().is_some_and(|t| t.value.trim().eq_ignore_ascii_case("ACDSRECORD"))
            && rec.iter().any(|t| t.code == 2 && t.value.trim().eq_ignore_ascii_case("ASM_Data"));
        let Some(h) = is_asm.then(|| data_store_owner(rec).and_then(|t| Handle::parse_hex(t.value.trim()))).flatten() else { continue };
        let mut data = Vec::new();
        let mut ok = true;
        for t in rec.iter().filter(|t| t.code == 310) {
            match unhex(&t.value) {
                Some(b) if data.len() + b.len() <= acis::MAX_DATA => data.extend(b),
                _ => ok = false,
            }
        }
        if ok && !data.is_empty() {
            out.entry(h).or_insert(data);
        }
    }
    out
}

/// A body as a DraftCraft solid where it is (no placement), with its look.
fn solid_of(body: &Body) -> Result<Solid3d, String> {
    let d = body.display(0.0).map_err(|e| e.0)?;
    if d.triangles.len() > MAX_LOOK_TRIANGLES {
        return Err("too detailed to draw".into());
    }
    Ok(Solid3d {
        body: Arc::new(body.encode().map_err(|e| e.0)?),
        placement: Mat4::IDENTITY,
        look: Arc::new(SolidLook { wires: d.wires, positions: d.positions, triangles: d.triangles, tri_face: d.tri_face }),
        faces: u32::try_from(body.face_count()).unwrap_or(u32::MAX),
        edges: u32::try_from(body.edge_count()).unwrap_or(u32::MAX),
        volume: body.mass().map(|m| m.volume).unwrap_or(0.0),
        body_entity: false,
    })
}

/// A modeler entity's ACIS data: its data store record's SAB, or its SAT text.
fn acis_data<'a>(u: &Unknown, sab: Option<&'a Vec<u8>>) -> Result<Cow<'a, [u8]>, String> {
    match (sab, sat_text(&u.tags)) {
        (Some(sab), _) => Ok(Cow::Borrowed(sab.as_slice())),
        (None, Some(sat)) => Ok(Cow::Owned(sat)),
        (None, None) => Err("it holds no ACIS data".into()),
    }
}

/// The region an AutoCAD REGION describes: its loops, outer and island as they nest.
fn region_of(u: &Unknown, sab: Option<&Vec<u8>>) -> Result<Region, String> {
    let loops = acis::read_region(&acis_data(u, sab)?).map_err(|e| e.0)?;
    let mut r = Region { loops: loops.into_iter().map(|vertices| HatchLoop { vertices, outer: true, sources: Vec::new() }).collect() };
    if !r.renest() {
        return Err("too many loops".into());
    }
    Ok(r)
}

/// The solid an AutoCAD 3DSOLID or BODY describes, from its SAT text or its data store record.
fn adopt_one(u: &Unknown, sab: Option<&Vec<u8>>) -> Result<Solid3d, String> {
    let data = acis_data(u, sab)?;
    let bodies = acis::read(&data).map_err(|e| e.0)?;
    match bodies.as_slice() {
        [one] => solid_of(one),
        [] => Err("it describes no solid".into()),
        _ => Err("it holds several separate solids (not read yet)".into()),
    }
}

/// Turn every AutoCAD 3DSOLID and BODY of `d` whose ACIS data the kernel reads into a DraftCraft
/// solid, and every REGION into a DraftCraft region.
/// A record of the data store that became a solid is dropped (saving writes the solid's own).
pub(crate) fn adopt(d: &mut Drawing) {
    let mut stored = stored_sab(&d.foreign.data_store);
    let acadver = d.header.get("ACADVER").and_then(draftcraft_doc::HVal::as_str).unwrap_or_default().to_string();
    let mut sources: Vec<(Handle, AcisSource)> = Vec::new();
    let mut adopted: Vec<Handle> = Vec::new();
    let mut failed: BTreeMap<(Modeler, String), usize> = BTreeMap::new();
    let mut n = 0usize;
    for_each_store(d, |st| {
        let hs: Vec<Handle> = st.iter().filter(|e| matches!(&e.kind, EntityKind::Unknown(u) if Modeler::of(u).is_some())).map(|e| e.handle).collect();
        for h in hs {
            if n >= MAX_SOLIDS {
                return;
            }
            n += 1;
            let Some(EntityKind::Unknown(u)) = st.get(h).map(|e| &e.kind) else { continue };
            let Some(kind) = Modeler::of(u) else { continue };
            let made = match kind {
                Modeler::Solid => adopt_one(u, stored.get(&h)).map(EntityKind::Solid3d),
                Modeler::Body => adopt_one(u, stored.get(&h)).map(|so| EntityKind::Solid3d(Solid3d { body_entity: true, ..so })),
                Modeler::Region => region_of(u, stored.get(&h)).map(EntityKind::Region),
            };
            match made {
                Ok(k) => {
                    // AutoCAD's own description moves to the source, for saving it back.
                    let read = k.clone();
                    let mut was = None;
                    let _ = st.modify(h, |e| was = Some(std::mem::replace(&mut e.kind, k)));
                    if let Some(EntityKind::Unknown(entity)) = was {
                        sources.push((h, AcisSource { acadver: acadver.clone(), entity, sab: stored.remove(&h), read }));
                    }
                    adopted.push(h);
                }
                Err(why) => *failed.entry((kind, why)).or_default() += 1,
            }
        }
    });
    if !adopted.is_empty() {
        let f = Arc::make_mut(&mut d.foreign);
        let owners: std::collections::BTreeSet<Handle> = adopted.iter().copied().collect();
        let mut kept = Vec::with_capacity(f.data_store.len());
        let first = f.data_store.iter().position(|t| t.code == 0).unwrap_or(f.data_store.len());
        kept.extend(f.data_store.iter().take(first).cloned());
        for rec in data_store_records(&f.data_store) {
            let owner = data_store_owner(rec).and_then(|t| Handle::parse_hex(t.value.trim()));
            let is_record = rec.first().is_some_and(|t| t.value.trim().eq_ignore_ascii_case("ACDSRECORD"));
            if !(is_record && owner.is_some_and(|h| owners.contains(&h))) {
                kept.extend(rec.iter().cloned());
            }
        }
        // Only the schemas are left: nothing to keep.
        if !kept.iter().any(|t| t.code == 0 && t.value.trim().eq_ignore_ascii_case("ACDSRECORD")) {
            kept.clear();
        }
        f.data_store = kept;
        f.acis.extend(sources);
    }
    for ((kind, why), count) in failed {
        let what = kind.count(count);
        d.read_notes.push(format!("{what} kept as AutoCAD saved it: its ACIS data can't be read yet ({why})."));
    }
}

// ---------------------------------------------------------------------------------------------
// Saving

/// The body of a solid where it sits in the drawing.
fn world_body(so: &Solid3d) -> Option<Body> {
    let body = Body::decode(&so.body).ok()?;
    let identity = so.placement.m.iter().flatten().zip(Mat4::IDENTITY.m.iter().flatten()).all(|(a, b)| (a - b).abs() <= 1e-15);
    if identity { Some(body) } else { body.transformed(&so.placement).ok() }
}

/// SAT text as the groups of an entity: each line a group 1, continued in groups 3 past 255
/// characters, a caret written as a caret and a space.
fn sat_groups(sat: &str, out: &mut Vec<RawTag>) {
    for line in sat.lines() {
        let chars: Vec<char> = line.replace('^', "^ ").chars().collect();
        if chars.is_empty() {
            continue;
        }
        let mut at = 0;
        let mut first = true;
        while at < chars.len() {
            // Never end a part between a caret and its space.
            let mut end = (at + GROUP_CHARS).min(chars.len());
            if end < chars.len() && chars.get(end - 1) == Some(&'^') {
                end -= 1;
            }
            out.push(tag(if first { 1 } else { 3 }, chars.get(at..end).unwrap_or(&[]).iter().collect::<String>()));
            first = false;
            at = end;
        }
    }
}

/// A GUID for the solid's entity, the same each time the same solid is saved.
fn guid(h: Handle, data: &[u8]) -> String {
    let mut a: u64 = 0xcbf2_9ce4_8422_2325 ^ h.0;
    let mut b: u64 = 0x8422_2325_cbf2_9ce4 ^ h.0.rotate_left(17);
    for &x in data {
        a = (a ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3);
        b = (b ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(5);
    }
    // Version 4, variant 1, as a random GUID.
    let c = (((a >> 16) & 0x0fff) | 0x4000) as u16;
    let v = ((b >> 48) & 0x3fff | 0x8000) as u16;
    format!("{{{:08x}-{:04x}-{c:04x}-{v:04x}-{:012x}}}", (a >> 32) as u32, (a & 0xffff) as u16, b & 0xffff_ffff_ffff)
}

/// The data store records of SAB data, laid out as AutoCAD writes them.
fn store_records(records: &[(Handle, Vec<u8>)], out: &mut Vec<RawTag>) {
    for (h, sab) in records {
        out.extend([
            tag(0, "ACDSRECORD"),
            tag(90, "0"),
            tag(2, "AcDbDs::ID"),
            tag(280, "10"),
            tag(320, h.hex()),
            tag(2, "ASM_Data"),
            tag(280, "15"),
            tag(94, sab.len().to_string()),
        ]);
        out.extend(sab.chunks(127).map(|c| tag(310, c.iter().map(|b| format!("{b:02X}")).collect::<String>())));
    }
}

/// The 3DSOLID entity for a solid in a file of `version`, and its SAB for the data store (2013
/// and later). `None` when the ACIS writer can't describe the body.
fn as_3dsolid(e: &Entity, so: &Solid3d, version: Version) -> Option<(Unknown, Option<Vec<u8>>)> {
    let body = world_body(so)?;
    let mut tags = vec![tag(100, "AcDbModelerGeometry")];
    let sab = if version >= Version::R2013 {
        let sab = acis::write_sab(&body).ok()?;
        tags.extend([tag(290, "1"), tag(2, guid(e.handle, &sab))]);
        Some(sab)
    } else {
        let sat = acis::write_sat(&body).ok()?;
        tags.push(tag(70, "1"));
        sat_groups(&acis::scramble(&sat), &mut tags);
        None
    };
    if version >= Version::R2007 && !so.body_entity {
        tags.extend([tag(100, "AcDb3dSolid"), tag(350, "0")]);
    }
    Some((Unknown { dxf_type: if so.body_entity { "BODY" } else { "3DSOLID" }.into(), tags }, sab))
}

/// The REGION entity for a region in a file of `version`, and its SAB for the data store (2013
/// and later). `None` when the region has no area to write (it keeps its polylines).
fn as_region(e: &Entity, r: &Region, version: Version) -> Option<(Unknown, Option<Vec<u8>>)> {
    let loops: Vec<(Vec<draftcraft_geom::PolyVertex>, bool)> = r.loops.iter().map(|l| (l.vertices.clone(), l.outer)).collect();
    let mut tags = vec![tag(100, "AcDbModelerGeometry")];
    let sab = if version >= Version::R2013 {
        let sab = acis::write_region_sab(&loops).ok()?;
        tags.extend([tag(290, "1"), tag(2, guid(e.handle, &sab))]);
        Some(sab)
    } else {
        let sat = acis::write_region_sat(&loops).ok()?;
        tags.push(tag(70, "1"));
        sat_groups(&acis::scramble(&sat), &mut tags);
        None
    };
    Some((Unknown { dxf_type: "REGION".into(), tags }, sab))
}

/// Whether entity `kind` is still what it was read as (`read`): the same body where it was, or
/// the same region.
fn unchanged(kind: &EntityKind, read: &EntityKind) -> bool {
    match (kind, read) {
        (EntityKind::Solid3d(a), EntityKind::Solid3d(b)) => a.body == b.body && a.placement == b.placement && a.body_entity == b.body_entity,
        (EntityKind::Region(a), EntityKind::Region(b)) => a == b,
        _ => false,
    }
}

/// The entity AutoCAD saved for `e`, and its SAB for the data store, when `e` is still as it was
/// read and a file of `version` holds that description as it is: in the same form (SAT in the
/// entity up to 2010, SAB in the data store from 2013) and in a version no older than the file it
/// came from (an older AutoCAD discards a newer ACIS description).
fn as_read(e: &Entity, src: &AcisSource, version: Version) -> Option<(Unknown, Option<Vec<u8>>)> {
    let from = Version::for_acadver(src.acadver.trim())?;
    let sab = version >= Version::R2013;
    if (from >= Version::R2013) != sab || src.sab.is_some() != sab || version < from || !unchanged(&e.kind, &src.read) {
        return None;
    }
    // Its modeler groups: the entity's properties are the drawing's now, its xdata the kept
    // foreign data's.
    let own: Vec<&RawTag> = src.entity.tags.iter().take_while(|t| t.code < 1000).collect();
    let from = own.iter().position(|t| t.code == 100 && t.value.trim() == "AcDbModelerGeometry")?;
    let mut u = Unknown { dxf_type: src.entity.dxf_type.clone(), tags: own.into_iter().skip(from).cloned().collect() };
    // From 2007 a 3D solid's groups end with its history (none known here), as `as_3dsolid` writes them.
    let solid = u.dxf_type.trim().eq_ignore_ascii_case("3DSOLID");
    if solid && version >= Version::R2007 && !u.tags.iter().any(|t| t.code == 100 && t.value.trim() == "AcDb3dSolid") {
        u.tags.extend([tag(100, "AcDb3dSolid"), tag(350, "0")]);
    }
    Some((u, src.sab.clone()))
}

/// `d` with every solid the ACIS writer can describe as AutoCAD's own 3DSOLID (or BODY) for a file
/// of `version`, and every region as a REGION; `d` itself when it has none.
pub(crate) fn for_saving(d: &Drawing, version: Version) -> Cow<'_, Drawing> {
    let has = |s: &EntityStore| s.iter().any(|e| matches!(e.kind, EntityKind::Solid3d(_) | EntityKind::Region(_)));
    if !has(&d.model) && !d.layouts.iter().any(|l| has(&l.entities)) && !d.blocks.values().any(|b| has(&b.entities)) {
        return Cow::Borrowed(d);
    }
    let mut d = d.clone();
    // AutoCAD's own descriptions, for the solids and regions still as they were read.
    let read = Arc::clone(&d.foreign);
    let mut sabs: Vec<(Handle, Vec<u8>)> = Vec::new();
    let mut n = 0usize;
    for_each_store(&mut d, |st| {
        let hs: Vec<Handle> = st.iter().filter(|e| matches!(e.kind, EntityKind::Solid3d(_) | EntityKind::Region(_))).map(|e| e.handle).collect();
        for h in hs {
            if n >= MAX_SOLIDS {
                return;
            }
            n += 1;
            let Some(made) = st.get(h).and_then(|e| {
                read.acis.get(&h).and_then(|src| as_read(e, src, version)).or_else(|| match &e.kind {
                    EntityKind::Solid3d(so) => as_3dsolid(e, so, version),
                    EntityKind::Region(r) => as_region(e, r, version),
                    _ => None,
                })
            }) else {
                continue;
            };
            let (mut u, sab) = made;
            // The entity's properties (colour, linetype, lineweight…) as every entity's: the
            // writer takes an AutoCAD entity's from its groups.
            let _ = st.modify(h, |e| {
                u.tags.splice(0..0, crate::dxf_write::property_groups(&e.common));
                e.kind = EntityKind::Unknown(u);
            });
            if let Some(sab) = sab {
                sabs.push((h, sab));
            }
        }
    });
    if !sabs.is_empty() {
        let f = Arc::make_mut(&mut d.foreign);
        if f.data_store.is_empty() {
            let records: Vec<(u64, &[u8])> = sabs.iter().map(|(h, s)| (h.0, s.as_slice())).collect();
            f.data_store = crate::dwg::data_store::groups(&draftcraft_dwg::datastore::solids(&records));
        } else {
            store_records(&sabs, &mut f.data_store);
        }
    }
    Cow::Owned(d)
}

#[cfg(test)]
mod tests;
