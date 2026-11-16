use draftcraft_doc::{Common, Drawing, EntityKind, Space};
use draftcraft_dxf::Version;
use draftcraft_geom::{Mat4, Vec3};
use draftcraft_solid::{Body, acis, box_solid, cylinder};

fn data(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/acad-solids").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The solids of model space: (volume, centroid) of each, in drawing order.
fn solids(d: &Drawing) -> Vec<(f64, Vec3)> {
    d.model
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Solid3d(so) => {
                let body = super::world_body(so).expect("body");
                let m = body.mass().expect("mass");
                Some((m.volume, m.centroid))
            }
            _ => None,
        })
        .collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
}

fn check(name: &str, got: &[(f64, Vec3)], want: &[(f64, [f64; 3])]) {
    assert_eq!(got.len(), want.len(), "{name}: {got:?}");
    for (i, ((v, c), (wv, wc))) in got.iter().zip(want).enumerate() {
        assert!(close(*v, *wv), "{name} #{i}: volume {v}, AutoCAD's MASSPROP {wv}");
        let d = (*c - Vec3::new(wc[0], wc[1], wc[2])).len();
        assert!(d <= 1e-3, "{name} #{i}: centroid {c:?}, AutoCAD's MASSPROP {wc:?}");
    }
}

/// AutoCAD's MASSPROP for the six primitives of `solids-basic` (the drawings' README).
const BASIC: [(f64, [f64; 3]); 6] = [
    (240000.0, [50.0, 30.0, 20.0]),
    (98174.7704, [200.0, 0.0, 25.0]),
    (56548.6678, [300.0, 0.0, 15.0]),
    (113097.3355, [400.0, 0.0, 30.0]),
    (78956.8352, [500.0, 0.0, 0.0]),
    (80000.0, [626.6667, 25.0, 13.3333]),
];

fn close4(got: &[(f64, Vec3)], want: &[(f64, [f64; 3])], name: &str) {
    // MASSPROP prints four decimals: compare to that.
    assert_eq!(got.len(), want.len(), "{name}: {got:?}");
    for (i, ((v, c), (wv, wc))) in got.iter().zip(want).enumerate() {
        assert!((v - wv).abs() <= 1e-4 + 1e-9 * wv.abs(), "{name} #{i}: volume {v}, AutoCAD's MASSPROP {wv}");
        assert!((*c - Vec3::new(wc[0], wc[1], wc[2])).len() <= 1e-3, "{name} #{i}: centroid {c:?}, AutoCAD's MASSPROP {wc:?}");
    }
}

#[test]
fn autocad_primitives_open_as_solids_in_every_format() {
    for name in [
        "solids-basic-2000.dxf",
        "solids-basic-2010.dxf",
        "solids-basic-2018.dxf",
        "solids-basic-2000.dwg",
        "solids-basic-2013.dwg",
        "solids-basic-2018.dwg",
    ] {
        let d = crate::read(&data(name), name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(d.read_notes.iter().all(|n| !n.contains("3D solid")), "{name}: {:?}", d.read_notes);
        close4(&solids(&d), &BASIC, name);
        // The data store records became solids: none is left to write twice.
        assert!(d.foreign.data_store_entities().is_empty(), "{name}");
    }
}

#[test]
fn autocad_edited_solids_open_as_solids() {
    // SUBTRACT, FILLETEDGE, CHAMFEREDGE (the README's MASSPROP).
    let want = [(437168.1469, [50.0, 50.0, 25.0]), (495707.9633, [250.0, 50.0, 25.0]), (495166.6667, [450.0, 50.0, 24.7724])];
    for name in ["solids-edit-2000.dxf", "solids-edit-2018.dwg"] {
        let d = crate::read(&data(name), name).unwrap_or_else(|e| panic!("{name}: {e}"));
        close4(&solids(&d), &want, name);
    }
}

#[test]
fn autocad_free_form_solids_read() {
    // LOFT (a spline surface through a circle and a square) and SWEEP of a circle along a helix
    // (a procedural helix, closed circle edges): both read through ACIS's own pcurves.
    // DXF 2000 holds SAT 400 (`exactsur`, `exppc`, edges without their parameters), DXF 2010
    // SAT 700, DWG 2018 the data store's SAB.
    for name in ["solids-free-2000.dxf", "solids-free-2010.dxf", "solids-free-2018.dwg"] {
        free_form_read(name);
    }
}

fn free_form_read(name: &str) {
    let d = crate::read(&data(name), name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let read = solids(&d);
    let kept = d.model.iter().filter(|e| matches!(&e.kind, EntityKind::Unknown(u) if u.dxf_type == "3DSOLID")).count();
    assert_eq!((read.len(), kept), (2, 0), "{name}: {:?}", d.read_notes);
    // AutoCAD's MASSPROP of the two (acad-ref/mp-*.mpr): the loft 175685.1346 about z 36.2738,
    // the sweep 44855.4624 about (150, 0, 40). The helix is read as a spline through it, a few
    // parts in 10⁵ off.
    let near = |(v, c): (f64, Vec3), want: f64, at: Vec3| (v - want).abs() <= 1e-3 * want && (c - at).len() <= 0.05;
    let mut by_volume = read.clone();
    by_volume.sort_by(|a, b| a.0.total_cmp(&b.0));
    let [sweep, loft] = by_volume.as_slice() else { panic!("{name}: {read:?}") };
    assert!(near(*loft, 175_685.134_6, Vec3::new(0.0, 0.0, 36.2738)), "{name}: loft {loft:?}");
    // The side face whole, not its inner half: a seam's two uses a period apart.
    assert!(near(*sweep, 44_855.462_4, Vec3::new(150.0, 0.0, 40.0)), "{name}: sweep {sweep:?}");
}

/// A drawing with a box placed away from the origin and a box with a hole.
fn drawing() -> (Drawing, Vec<(f64, Vec3)>) {
    let mut d = Drawing::new_metric();
    let b = box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 60.0, 40.0)).expect("box");
    let mut so = super::solid_of(&b).expect("solid");
    let mut m = Mat4::IDENTITY;
    m.m[0][3] = 1000.0;
    m.m[1][3] = -50.0;
    so.placement = m;
    d.add(&Space::Model, Common::default(), EntityKind::Solid3d(so)).expect("add");
    let block = box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 50.0)).expect("box");
    let hole = cylinder(Vec3::new(50.0, 50.0, -10.0), Vec3::new(0.0, 0.0, 1.0), 20.0, 70.0).expect("cylinder");
    let holed: Body = draftcraft_solid::boolean(&block, &hole, draftcraft_solid::BoolOp::Subtract).expect("subtract").expect("a solid");
    d.add(&Space::Model, Common::default(), EntityKind::Solid3d(super::solid_of(&holed).expect("solid"))).expect("add");
    let want = solids(&d);
    (d, want)
}

/// Each 3DSOLID record of DXF text: its groups as (code, value) pairs.
fn solid_records(text: &str) -> Vec<Vec<(String, String)>> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out: Vec<Vec<(String, String)>> = Vec::new();
    let mut current: Option<Vec<(String, String)>> = None;
    for pair in lines.chunks(2) {
        let [code, value] = pair else { break };
        if *code == "0" {
            out.extend(current.take());
            current = (*value == "3DSOLID").then(Vec::new);
        } else if let Some(c) = current.as_mut() {
            c.push((code.to_string(), value.to_string()));
        }
    }
    out.extend(current);
    out
}

#[test]
fn solids_save_as_autocad_3dsolids_in_every_version() {
    let (d, want) = drawing();
    let want: Vec<(f64, [f64; 3])> = want.iter().map(|(v, c)| (*v, [c.x, c.y, c.z])).collect();
    for v in Version::ALL {
        let dxf = crate::write_with(&d, "s.dxf", &crate::SaveOptions { version: Some(v), binary: false }).expect("dxf");
        let text = String::from_utf8_lossy(&dxf).replace("\r\n", "\n");
        assert_eq!(text.matches("\n3DSOLID\n").count(), 2, "{v:?}");
        // No interim meshes: AutoCAD's own entity.
        assert!(!text.contains("DRAFTCRAFT_SOLID") && !text.contains("\nPOLYLINE\n"), "{v:?}");
        assert_eq!(text.contains("\nACDSDATA\n"), v >= Version::R2013, "{v:?}");
        // AutoCAD discards a DXF file whose 3DSOLID lacks the AcDbEntity marker before its layer.
        for groups in solid_records(&text) {
            let at = |g: (&str, &str)| groups.iter().position(|(c, x)| c == g.0 && (g.1.is_empty() || x == g.1));
            let (marker, layer, modeler) = (at(("100", "AcDbEntity")), at(("8", "")), at(("100", "AcDbModelerGeometry")));
            assert!(marker.is_some() && marker < layer && layer < modeler, "{v:?}: {groups:?}");
            // SAT is ACIS 400, as AutoCAD writes it in a 2000 file: AutoCAD 2027 discards a 700
            // solid from a DWG 2000 to 2010 file (error 75003). The DWG writer takes these groups.
            if v < Version::R2013 {
                let first = groups.iter().skip_while(|(c, _)| c != "70").find(|(c, _)| c == "1");
                let first = first.map(|(_, x)| draftcraft_solid::acis::scramble(&x.replace("^ ", "^")));
                assert!(first.as_deref().is_some_and(|l| l.starts_with("400 ")), "{v:?}: {first:?}");
            }
        }
        check(&format!("DXF {v:?}"), &solids(&crate::read(&dxf, "s.dxf").expect("read dxf")), &want);
        let (dwg, unwritten) = crate::write_dwg_own(&d, v).expect("dwg");
        assert!(unwritten.is_empty(), "{v:?}: {unwritten:?}");
        check(&format!("DWG {v:?}"), &solids(&crate::read(&dwg, "s.dwg").expect("read dwg")), &want);
    }
}

#[test]
fn an_autocad_solid_saved_again_stays_a_solid() {
    for (name, out) in [
        ("solids-basic-2018.dwg", "a.dwg"),
        ("solids-basic-2018.dwg", "a.dxf"),
        ("solids-basic-2000.dxf", "a.dxf"),
        ("solids-basic-2000.dxf", "a.dwg"),
    ] {
        let d = crate::read(&data(name), name).expect("read");
        for v in [Version::R2000, Version::R2010, Version::R2018] {
            let bytes = crate::write_with(&d, out, &crate::SaveOptions { version: Some(v), binary: false }).expect("write");
            let back = crate::read(&bytes, out).expect("read back");
            close4(&solids(&back), &BASIC, &format!("{name} → {out} {v:?}"));
            assert_eq!(back.foreign.data_store_entities().len(), 0, "{name} → {out} {v:?}");
        }
    }
}

#[test]
fn hostile_3dsolid_groups_are_kept_and_never_crash() {
    let (d, _) = drawing();
    let text = String::from_utf8(crate::write_with(&d, "s.dxf", &crate::SaveOptions { version: Some(Version::R2000), binary: false }).expect("dxf"))
        .expect("text")
        .replace("\r\n", "\n");
    // The first solid's SAT text with lines dropped, numbers garbled or cut short: each is a solid
    // that can't be read, kept as saved with a note, or a solid; never a crash.
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let first = lines.iter().position(|l| l == "AcDbModelerGeometry").expect("solid");
    // Groups are line pairs: a code, then its value.
    let sat: Vec<usize> =
        (first + 1..lines.len() - 1).step_by(2).take_while(|i| lines[*i].trim() != "0").filter(|i| lines[*i].trim() == "1").map(|i| i + 1).collect();
    assert!(sat.len() > 20);
    for (k, &at) in sat.iter().enumerate().skip(3).step_by(5) {
        let mut l = lines.clone();
        match k % 3 {
            0 => l[at] = String::new(),
            1 => {
                let garbled: String = l[at].chars().map(|c| if c.is_ascii_alphanumeric() { 'n' } else { c }).collect();
                l[at] = garbled;
            }
            _ => {
                let half = l[at].len() / 2;
                l[at].truncate(half);
            }
        }
        let back = crate::read(l.join("\n").as_bytes(), "bad.dxf").expect("the drawing still opens");
        let solids = back.model.iter().filter(|e| matches!(e.kind, EntityKind::Solid3d(_))).count();
        let kept = back.model.iter().filter(|e| matches!(&e.kind, EntityKind::Unknown(u) if u.dxf_type == "3DSOLID")).count();
        assert_eq!(solids + kept, 2, "line {at}");
        if kept > 0 {
            assert!(back.read_notes.iter().any(|n| n.contains("3D solid")), "line {at}: {:?}", back.read_notes);
        }
    }
}

/// Writes the drawings AutoCAD is checked against into `DRAFTCRAFT_ACIS_OUT` (run by hand:
/// `DRAFTCRAFT_ACIS_OUT=dir cargo test -p draftcraft-io --features solids write_files_for_autocad -- --ignored`).
#[test]
#[ignore = "writes files for a check in AutoCAD"]
fn write_files_for_autocad() {
    let Ok(dir) = std::env::var("DRAFTCRAFT_ACIS_OUT") else { return };
    let z = Vec3::new(0.0, 0.0, 1.0);
    let mut d = Drawing::new_metric();
    let mut add = |body: Body, placement: Mat4| {
        let mut so = super::solid_of(&body).expect("solid");
        so.placement = placement;
        d.add(&Space::Model, Common::default(), EntityKind::Solid3d(so)).expect("add");
    };
    add(box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 60.0, 40.0)).expect("box"), Mat4::IDENTITY);
    add(cylinder(Vec3::new(200.0, 0.0, 0.0), z, 25.0, 50.0).expect("cylinder"), Mat4::IDENTITY);
    add(draftcraft_solid::cone(Vec3::new(300.0, 0.0, 0.0), z, 30.0, 0.0, 60.0).expect("cone"), Mat4::IDENTITY);
    add(draftcraft_solid::sphere(Vec3::new(400.0, 0.0, 30.0), 30.0).expect("sphere"), Mat4::IDENTITY);
    add(draftcraft_solid::torus(Vec3::new(500.0, 0.0, 0.0), 40.0, 10.0).expect("torus"), Mat4::IDENTITY);
    add(draftcraft_solid::cone(Vec3::new(600.0, 0.0, 0.0), z, 30.0, 10.0, 60.0).expect("frustum"), Mat4::IDENTITY);
    let block = box_solid(Vec3::new(0.0, 200.0, 0.0), Vec3::new(100.0, 300.0, 50.0)).expect("box");
    let hole = cylinder(Vec3::new(50.0, 250.0, -10.0), z, 20.0, 70.0).expect("cylinder");
    add(draftcraft_solid::boolean(&block, &hole, draftcraft_solid::BoolOp::Subtract).expect("subtract").expect("solid"), Mat4::IDENTITY);
    // A 100 × 60 × 40 box turned 30° about Z and moved to (300, 250, 0).
    let (s, c) = (30f64.to_radians().sin(), 30f64.to_radians().cos());
    let turned = Mat4 { m: [[c, -s, 0.0, 300.0], [s, c, 0.0, 250.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };
    add(box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 60.0, 40.0)).expect("box"), turned);
    for (v, y) in [(Version::R2000, "2000"), (Version::R2010, "2010"), (Version::R2013, "2013"), (Version::R2018, "2018")] {
        let dxf = crate::write_with(&d, "s.dxf", &crate::SaveOptions { version: Some(v), binary: false }).expect("dxf");
        std::fs::write(format!("{dir}/dc-solids-{y}.dxf"), dxf).expect("write");
        let (dwg, _) = crate::write_dwg_own(&d, v).expect("dwg");
        std::fs::write(format!("{dir}/dc-solids-{y}.dwg"), dwg).expect("write");
    }
    // AutoCAD's own drawing, opened and saved again by DraftCraft.
    let acad = crate::read(&data("solids-basic-2018.dwg"), "a.dwg").expect("read");
    std::fs::write(format!("{dir}/acad-resaved-2018.dwg"), crate::write_dwg_own(&acad, Version::R2018).expect("dwg").0).expect("write");
    std::fs::write(
        format!("{dir}/acad-resaved-2000.dxf"),
        crate::write_with(&acad, "a.dxf", &crate::SaveOptions { version: Some(Version::R2000), binary: false }).expect("dxf"),
    )
    .expect("write");
    for (i, (v, c)) in solids(&d).iter().enumerate() {
        println!("#{} volume {v:.4} centroid {:.4}, {:.4}, {:.4}", i + 1, c.x, c.y, c.z);
    }
    // Regions: the plate with its hole and round end, and the disc.
    let (r, areas) = regions_drawing();
    for (v, y) in [(Version::R2000, "2000"), (Version::R2010, "2010"), (Version::R2013, "2013"), (Version::R2018, "2018")] {
        let dxf = crate::write_with(&r, "r.dxf", &crate::SaveOptions { version: Some(v), binary: false }).expect("dxf");
        std::fs::write(format!("{dir}/dc-regions-{y}.dxf"), dxf).expect("write");
        std::fs::write(format!("{dir}/dc-regions-{y}.dwg"), crate::write_dwg_own(&r, v).expect("dwg").0).expect("write");
    }
    for (i, a) in areas.iter().enumerate() {
        println!("region #{} area {a:.4}", i + 1);
    }
}

#[test]
fn autocad_free_form_solids_write_back() {
    // The loft's and the sweep's spline faces written as SAT 400 and as SAB (exactsur, exactcur and
    // exppc, as AutoCAD writes them in DWG 2000), then read again: the same solids.
    let name = "solids-free-2018.dwg";
    let d = crate::read(&data(name), name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut n = 0;
    for e in d.model.iter() {
        let EntityKind::Solid3d(so) = &e.kind else { continue };
        let body = super::world_body(so).expect("body");
        let want = body.mass().expect("mass");
        let sat = acis::write_sat(&body).unwrap_or_else(|e| panic!("{name}: SAT: {}", e.0));
        if let Some(dir) = std::env::var_os("DRAFTCRAFT_ACIS_DUMP") {
            let _ = std::fs::write(std::path::Path::new(&dir).join(format!("free-{n}.sat")), &sat);
        }
        let sab = acis::write_sab(&body).unwrap_or_else(|e| panic!("{name}: SAB: {}", e.0));
        if let Some(dir) = std::env::var_os("DRAFTCRAFT_ACIS_DUMP") {
            let _ = std::fs::write(std::path::Path::new(&dir).join(format!("free-{n}.sab")), &sab);
        }
        for (form, bytes) in [("SAT", sat.into_bytes()), ("SAB", sab)] {
            let back = acis::read(&bytes).unwrap_or_else(|e| panic!("{name} #{n} {form}: {}", e.0));
            let [b] = back.as_slice() else { panic!("{name} #{n} {form}: {} bodies", back.len()) };
            let got = b.mass().expect("mass");
            assert!((got.volume - want.volume).abs() <= 1e-6 * want.volume, "{name} #{n} {form}: {} vs {}", got.volume, want.volume);
        }
        n += 1;
    }
    assert_eq!(n, 2);
}

/// A 100 × 60 plate with a hole of radius 15 and a round end, and a disc of radius 5 apart.
fn regions_drawing() -> (Drawing, Vec<f64>) {
    use draftcraft_doc::{HatchLoop, Region};
    use draftcraft_geom::{PolyVertex, Vec2};
    let p = |x: f64, y: f64, b: f64| PolyVertex::with_bulge(Vec2::new(x, y), b);
    let lp = |v: Vec<PolyVertex>, outer: bool| HatchLoop { vertices: v, outer, sources: Vec::new() };
    let mut d = Drawing::new_metric();
    let plate = Region {
        loops: vec![
            lp(vec![p(0.0, 0.0, 0.0), p(100.0, 0.0, 1.0), p(100.0, 60.0, 0.0), p(0.0, 60.0, 0.0)], true),
            lp(vec![p(35.0, 30.0, 1.0), p(65.0, 30.0, 1.0)], false),
        ],
    };
    let disc = Region { loops: vec![lp(vec![p(200.0, 0.0, 1.0), p(210.0, 0.0, 1.0)], true)] };
    let mut want = Vec::new();
    for r in [plate, disc] {
        want.push(r.area());
        d.add(&Space::Model, Common::default(), EntityKind::Region(r)).expect("add");
    }
    (d, want)
}

fn region_areas(d: &Drawing) -> Vec<f64> {
    d.model.iter().filter_map(|e| if let EntityKind::Region(r) = &e.kind { Some(r.area()) } else { None }).collect()
}

#[test]
fn regions_save_as_autocad_regions_in_every_version() {
    let (d, want) = regions_drawing();
    for v in Version::ALL {
        let dxf = crate::write_with(&d, "r.dxf", &crate::SaveOptions { version: Some(v), binary: false }).expect("dxf");
        let text = String::from_utf8_lossy(&dxf).replace("\r\n", "\n");
        assert_eq!(text.matches("\nREGION\n").count(), 2, "{v:?}");
        // AutoCAD's own entity, not the polylines DraftCraft writes without the ACIS writer.
        assert!(!text.contains("REGIONPART") && !text.contains("\nLWPOLYLINE\n"), "{v:?}");
        assert_eq!(text.contains("\nACDSDATA\n"), v >= Version::R2013, "{v:?}");
        let back = crate::read(&dxf, "r.dxf").expect("read dxf");
        assert!(back.read_notes.iter().all(|n| !n.contains("region")), "{v:?}: {:?}", back.read_notes);
        let got = region_areas(&back);
        assert_eq!(got.len(), 2, "DXF {v:?}");
        assert!(got.iter().zip(&want).all(|(a, b)| close(*a, *b)), "DXF {v:?}: {got:?} {want:?}");
        assert_eq!(back.foreign.data_store_entities().len(), 0, "{v:?}");
        let (dwg, unwritten) = crate::write_dwg_own(&d, v).expect("dwg");
        assert!(unwritten.is_empty(), "{v:?}: {unwritten:?}");
        let got = region_areas(&crate::read(&dwg, "r.dwg").expect("read dwg"));
        assert!(got.len() == 2 && got.iter().zip(&want).all(|(a, b)| close(*a, *b)), "DWG {v:?}: {got:?} {want:?}");
    }
}

#[test]
fn a_body_is_saved_as_a_body_again() {
    let mut d = Drawing::new_metric();
    let b = box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 20.0, 30.0)).expect("box");
    let so = draftcraft_doc::Solid3d { body_entity: true, ..super::solid_of(&b).expect("solid") };
    d.add(&Space::Model, Common::default(), EntityKind::Solid3d(so)).expect("add");
    for v in [Version::R2000, Version::R2010, Version::R2018] {
        let dxf = crate::write_with(&d, "b.dxf", &crate::SaveOptions { version: Some(v), binary: false }).expect("dxf");
        let text = String::from_utf8_lossy(&dxf).replace("\r\n", "\n");
        assert!(text.contains("\nBODY\n") && !text.contains("\n3DSOLID\n") && !text.contains("AcDb3dSolid"), "{v:?}");
        let (dwg, _) = crate::write_dwg_own(&d, v).expect("dwg");
        for (bytes, name) in [(dxf, "b.dxf"), (dwg, "b.dwg")] {
            let back = crate::read(&bytes, name).expect("read");
            let bodies: Vec<&draftcraft_doc::Solid3d> =
                back.model.iter().filter_map(|e| if let EntityKind::Solid3d(so) = &e.kind { Some(so) } else { None }).collect();
            assert!(bodies.len() == 1 && bodies[0].body_entity, "{name} {v:?}");
            assert!(close(bodies[0].volume, 6000.0), "{name} {v:?}: {}", bodies[0].volume);
        }
    }
}

#[test]
fn hostile_region_groups_are_kept_and_never_crash() {
    let (d, _) = regions_drawing();
    let text = String::from_utf8(crate::write_with(&d, "r.dxf", &crate::SaveOptions { version: Some(Version::R2000), binary: false }).expect("dxf"))
        .expect("text")
        .replace("\r\n", "\n");
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let first = lines.iter().position(|l| l == "AcDbModelerGeometry").expect("region");
    let sat: Vec<usize> =
        (first + 1..lines.len() - 1).step_by(2).take_while(|i| lines[*i].trim() != "0").filter(|i| lines[*i].trim() == "1").map(|i| i + 1).collect();
    assert!(sat.len() > 10);
    for (k, &at) in sat.iter().enumerate().skip(3) {
        let mut l = lines.clone();
        match k % 3 {
            0 => l[at] = String::new(),
            1 => l[at] = l[at].chars().map(|c| if c.is_ascii_alphanumeric() { 'n' } else { c }).collect(),
            _ => {
                let half = l[at].len() / 2;
                l[at].truncate(half);
            }
        }
        let back = crate::read(l.join("\n").as_bytes(), "bad.dxf").expect("the drawing still opens");
        let regions = back.model.iter().filter(|e| matches!(e.kind, EntityKind::Region(_))).count();
        let kept = back.model.iter().filter(|e| matches!(&e.kind, EntityKind::Unknown(u) if u.dxf_type == "REGION")).count();
        assert_eq!(regions + kept, 2, "line {at}");
        if kept > 0 {
            assert!(back.read_notes.iter().any(|n| n.contains("region")), "line {at}: {:?}", back.read_notes);
        }
    }
}

/// AutoCAD's MASSPROP for the five regions of `regions-*` (README-regions.md): area, perimeter,
/// centroid.
const REGIONS: [(f64, f64, [f64; 2]); 5] = [
    (6000.0, 320.0, [50.0, 30.0]),
    (2827.4334, 188.4956, [200.0, 30.0]),
    (5293.1417, 414.2478, [350.0, 30.0]),
    (9926.9908, 377.0796, [550.0, 50.2622]),
    (800.0, 160.0, [730.0, 10.0]),
];

#[test]
fn autocad_regions_open_as_regions_in_every_format() {
    for name in ["regions-2000.dxf", "regions-2010.dxf", "regions-2018.dxf", "regions-2000.dwg", "regions-2013.dwg", "regions-2018.dwg"] {
        let d = crate::read(&data(name), name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(d.read_notes.iter().all(|n| !n.contains("region")), "{name}: {:?}", d.read_notes);
        let props: Vec<draftcraft_doc::RegionProps> =
            d.model.iter().filter_map(|e| if let EntityKind::Region(r) = &e.kind { Some(r.props()) } else { None }).collect();
        assert_eq!(props.len(), REGIONS.len(), "{name}");
        for (i, (p, (area, perimeter, c))) in props.iter().zip(REGIONS).enumerate() {
            // MASSPROP prints four decimals.
            assert!((p.area - area).abs() <= 1e-4 + 1e-9 * area, "{name} #{i}: area {}, AutoCAD's {area}", p.area);
            assert!((p.perimeter - perimeter).abs() <= 1e-4 + 1e-9 * perimeter, "{name} #{i}: perimeter {}, AutoCAD's {perimeter}", p.perimeter);
            assert!(
                (p.centroid.x - c[0]).abs() <= 1e-4 && (p.centroid.y - c[1]).abs() <= 1e-4,
                "{name} #{i}: centroid {:?}, AutoCAD's {c:?}",
                p.centroid
            );
        }
        assert!(d.foreign.data_store_entities().is_empty(), "{name}");
        // Saved again, they stay regions with the same areas.
        for v in [Version::R2000, Version::R2018] {
            for out in ["a.dxf", "a.dwg"] {
                let bytes = crate::write_with(&d, out, &crate::SaveOptions { version: Some(v), binary: false }).expect("write");
                let back = crate::read(&bytes, out).expect("read back");
                let got = region_areas(&back);
                assert!(
                    got.len() == 5 && got.iter().zip(REGIONS).all(|(a, (w, ..))| (a - w).abs() <= 1e-4 + 1e-9 * w),
                    "{name} → {out} {v:?}: {got:?}"
                );
            }
        }
    }
}

/// The ACIS description a kept solid or region holds: its SAT text (unscrambled) or its SAB.
fn description(src: &draftcraft_doc::foreign::AcisSource) -> (Option<String>, Option<Vec<u8>>) {
    let sat = super::sat_text(&src.entity.tags).map(|t| acis::scramble(&t.iter().map(|&b| char::from(b)).collect::<String>()));
    (sat, src.sab.clone())
}

/// The ACIS version a description is in: the first number of its SAT header, or of SAB's.
fn acis_version(src: &draftcraft_doc::foreign::AcisSource) -> Option<String> {
    let (sat, _) = description(src);
    sat.and_then(|s| s.split_whitespace().next().map(str::to_string))
}

#[test]
fn an_unedited_autocad_solid_or_region_saves_as_autocad_wrote_it() {
    for (name, out, v) in [
        ("solids-basic-2000.dxf", "a.dxf", Version::R2000),
        ("solids-basic-2000.dwg", "a.dwg", Version::R2000),
        ("solids-basic-2010.dxf", "a.dxf", Version::R2010),
        ("solids-free-2010.dxf", "a.dxf", Version::R2010),
        ("solids-basic-2013.dwg", "a.dwg", Version::R2013),
        ("solids-basic-2018.dxf", "a.dxf", Version::R2018),
        ("solids-basic-2018.dwg", "a.dwg", Version::R2018),
        ("solids-edit-2018.dwg", "a.dwg", Version::R2018),
        ("solids-free-2018.dwg", "a.dwg", Version::R2018),
        ("regions-2000.dxf", "a.dxf", Version::R2000),
        ("regions-2010.dxf", "a.dxf", Version::R2010),
        ("regions-2018.dwg", "a.dwg", Version::R2018),
        // A newer version of the same form reads the older description.
        ("solids-basic-2000.dxf", "a.dxf", Version::R2010),
        ("solids-basic-2013.dwg", "a.dwg", Version::R2018),
    ] {
        let at = format!("{name} → {out} {v:?}");
        let d = crate::read(&data(name), name).expect("read");
        assert!(!d.foreign.acis.is_empty(), "{at}");
        let bytes = crate::write_with(&d, out, &crate::SaveOptions { version: Some(v), binary: false }).expect("write");
        let back = crate::read(&bytes, out).expect("read back");
        for (h, src) in &d.foreign.acis {
            let again = back.foreign.acis.get(h).unwrap_or_else(|| panic!("{at}: {h:?} not read back as a solid or region"));
            assert_eq!(description(again), description(src), "{at}: {h:?}");
            if crate::save_version(&d) == v {
                assert_eq!(modeler_groups(again), modeler_groups(src), "{at}: {h:?}");
            }
        }
        assert!(back.read_notes.iter().all(|n| !n.contains("ACIS")), "{at}: {:?}", back.read_notes);
    }
}

#[test]
fn an_edited_copied_or_down_saved_solid_gets_a_new_description() {
    let name = "solids-basic-2010.dxf";
    let mut d = crate::read(&data(name), name).expect("read");
    let first: Vec<draftcraft_doc::Handle> = d.model.iter().filter(|e| matches!(e.kind, EntityKind::Solid3d(_))).map(|e| e.handle).collect();
    let [moved, copied, ..] = first[..] else { panic!("{name}: {first:?}") };
    assert!(d.foreign.acis.values().all(|s| acis_version(s).as_deref() != Some("400")), "AutoCAD 2010 writes a newer ACIS than 400");
    d.model.modify(moved, |e| {
        if let EntityKind::Solid3d(so) = &mut e.kind {
            so.transform(&draftcraft_geom::Mat3::translate(draftcraft_geom::Vec2::new(5.0, 0.0)));
        }
    });
    let copy = d.model.get(copied).map(|e| e.kind.clone()).expect("solid");
    let copy = d.add(&Space::Model, Common::default(), copy).expect("copy");
    let bytes = crate::write_with(&d, "a.dxf", &crate::SaveOptions { version: Some(Version::R2010), binary: false }).expect("write");
    let back = crate::read(&bytes, "a.dxf").expect("read back");
    for (h, src) in &back.foreign.acis {
        let new = *h == moved || *h == copy;
        assert_eq!(acis_version(src).as_deref() == Some("400"), new, "{h:?}");
        if !new {
            assert_eq!(description(src), description(&d.foreign.acis[h]), "{h:?}");
        }
    }
    let mut want = BASIC.to_vec();
    want[0].1[0] += 5.0;
    want.push(BASIC[1]);
    close4(&solids(&back), &want, "moved and copied");
    // A 2018 description is newer than a 2010 file holds: written anew.
    let d = crate::read(&data("solids-basic-2018.dwg"), "a.dwg").expect("read");
    let bytes = crate::write_with(&d, "a.dxf", &crate::SaveOptions { version: Some(Version::R2010), binary: false }).expect("write");
    let back = crate::read(&bytes, "a.dxf").expect("read back");
    assert!(!back.foreign.acis.is_empty() && back.foreign.acis.values().all(|s| acis_version(s).as_deref() == Some("400")));
}

/// A modeler entity's own groups as kept: from its `AcDbModelerGeometry` marker, without xdata;
/// a handle (its history, 350) as whether it names an object (saving renumbers kept objects).
fn modeler_groups(src: &draftcraft_doc::foreign::AcisSource) -> Vec<draftcraft_doc::RawTag> {
    let own = src.entity.tags.iter().take_while(|t| t.code < 1000);
    own.skip_while(|t| !(t.code == 100 && t.value.trim() == "AcDbModelerGeometry"))
        .map(|t| match t.code {
            330..=369 => draftcraft_doc::RawTag { code: t.code, value: (t.value.trim() != "0").to_string() },
            _ => t.clone(),
        })
        .collect()
}

#[test]
fn solids_and_regions_keep_their_properties() {
    use draftcraft_doc::color::Color;
    let props = Common {
        color: Color::Index(1),
        lineweight: draftcraft_doc::Lineweight::Mm100(50),
        transparency: draftcraft_doc::Transparency::Percent(40),
        ltscale: 2.0,
        ..Common::default()
    };
    let check = |what: &str, d: &Drawing, want: &Common| {
        for v in [Version::R2000, Version::R2010, Version::R2018] {
            let dxf = crate::write_with(d, "p.dxf", &crate::SaveOptions { version: Some(v), binary: false }).expect("dxf");
            let (dwg, _) = crate::write_dwg_own(d, v).expect("dwg");
            for (bytes, name) in [(dxf, "p.dxf"), (dwg, "p.dwg")] {
                let back = crate::read(&bytes, name).expect("read");
                let got: Vec<&Common> =
                    back.model.iter().filter(|e| matches!(e.kind, EntityKind::Solid3d(_) | EntityKind::Region(_))).map(|e| &e.common).collect();
                assert!(!got.is_empty(), "{what} {name} {v:?}");
                for c in got {
                    // DWG has transparency from 2004.
                    let t = if name.ends_with(".dwg") && v < Version::R2004 { draftcraft_doc::Transparency::ByLayer } else { want.transparency };
                    let same = (c.color, c.lineweight, c.transparency, c.ltscale) == (want.color, want.lineweight, t, want.ltscale);
                    assert!(same, "{what} {name} {v:?}: {c:?}");
                }
            }
        }
    };
    // Made in DraftCraft.
    let (mut d, _) = drawing();
    let (r, _) = regions_drawing();
    for e in r.model.iter() {
        d.add(&Space::Model, e.common.clone(), e.kind.clone()).expect("add");
    }
    let hs: Vec<draftcraft_doc::Handle> = d.model.iter().map(|e| e.handle).collect();
    for h in hs {
        d.model.modify(h, |e| e.common = Common { layer: e.common.layer.clone(), ..props.clone() });
    }
    check("new", &d, &props);
    // Read from AutoCAD, given new properties: its description is still AutoCAD's.
    for name in ["solids-basic-2018.dwg", "regions-2000.dxf"] {
        let mut d = crate::read(&data(name), name).expect("read");
        let hs: Vec<draftcraft_doc::Handle> = d.foreign.acis.keys().copied().collect();
        for h in hs {
            d.model.modify(h, |e| e.common = Common { layer: e.common.layer.clone(), ..props.clone() });
        }
        check(name, &d, &props);
        let v = crate::save_version(&d);
        let out = if name.ends_with(".dwg") { "a.dwg" } else { "a.dxf" };
        let back =
            crate::read(&crate::write_with(&d, out, &crate::SaveOptions { version: Some(v), binary: false }).expect("write"), out).expect("read");
        for (h, src) in &d.foreign.acis {
            assert_eq!(back.foreign.acis.get(h).map(modeler_groups), Some(modeler_groups(src)), "{name}: {h:?}");
        }
    }
}
