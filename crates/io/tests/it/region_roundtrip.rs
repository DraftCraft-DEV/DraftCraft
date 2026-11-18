//! Regions survive DXF save and reopen (#510). Without 3D solids, a region is written as closed
//! polylines (one per loop) that every DXF reader shows; `DRAFTCRAFT` xdata on the first lets
//! DraftCraft read the region back whole, and foreign REGIONs stay unknown objects. With them
//! (the `solids` feature), a region is AutoCAD's own REGION, its ACIS data written and read by the
//! kernel's translator. Hostile xdata doesn't panic.

use draftcraft_doc::{Common, Drawing, EntityKind, HatchLoop, Region, Space};
use draftcraft_dxf::{records, sections};
use draftcraft_geom::{PolyVertex, Vec2};

fn regions(d: &Drawing) -> Vec<Region> {
    d.model
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Region(r) => Some(r.clone()),
            _ => None,
        })
        .collect()
}

fn square(x: f64, y: f64, s: f64, outer: bool) -> HatchLoop {
    let v = |a: f64, b: f64| PolyVertex::new(Vec2::new(a, b));
    HatchLoop { vertices: vec![v(x, y), v(x + s, y), v(x + s, y + s), v(x, y + s)], outer, sources: Vec::new() }
}

/// A square with a round hole and a square island in it, and a second square piece.
fn islands() -> Region {
    let hole = HatchLoop {
        vertices: vec![PolyVertex::with_bulge(Vec2::new(3.0, 5.0), 1.0), PolyVertex::with_bulge(Vec2::new(7.0, 5.0), 1.0)],
        outer: false,
        sources: Vec::new(),
    };
    Region { loops: vec![square(0.0, 0.0, 10.0, true), hole, square(4.5, 4.5, 1.0, true), square(20.0, 0.0, 2.0, true)] }
}

#[cfg(feature = "solids")]
#[test]
fn region_with_islands_is_saved_as_an_autocad_region() {
    let mut d = Drawing::new_imperial();
    let r = islands();
    let h = d.add(&Space::Model, Common { layer: "Areas".into(), ..Common::default() }, EntityKind::Region(r.clone())).unwrap();
    let saved = draftcraft_io::write_dxf(&d);
    let tags = draftcraft_dxf::parse(saved.as_bytes()).unwrap();
    let ents = records(&sections(&tags).into_iter().find(|s| s.name == "ENTITIES").unwrap().tags);
    assert_eq!(ents.iter().filter(|(k, _)| k == "REGION").count(), 1);
    assert!(!ents.iter().any(|(k, _)| k == "LWPOLYLINE"));
    for back in [draftcraft_io::read_dxf(saved.as_bytes()).unwrap(), draftcraft_io::read_dxf(draftcraft_io::write_dxf(&d).as_bytes()).unwrap()] {
        let got = regions(&back);
        assert_eq!(got.len(), 1);
        assert_eq!(back.entity(h).unwrap().common.layer, "Areas");
        // The same loops (holes run clockwise in ACIS, so their order may turn round).
        assert_eq!(got[0].loops.len(), 4);
        assert_eq!(got[0].loops.iter().filter(|l| l.outer).count(), 3);
        assert!((got[0].area() - r.area()).abs() < 1e-9, "{} {}", got[0].area(), r.area());
        assert!((got[0].perimeter() - r.perimeter()).abs() < 1e-9);
    }
}

#[cfg(not(feature = "solids"))]
#[test]
fn region_with_islands_survives_save_and_reopen() {
    let mut d = Drawing::new_imperial();
    let r = islands();
    let h = d.add(&Space::Model, Common { layer: "Areas".into(), ..Common::default() }, EntityKind::Region(r.clone())).unwrap();
    let plain = d
        .add(
            &Space::Model,
            Common::default(),
            EntityKind::LwPolyline(draftcraft_doc::LwPolyline {
                vertices: square(50.0, 0.0, 1.0, true).vertices,
                closed: true,
                const_width: 0.0,
                elevation: 0.0,
                plinegen: false,
            }),
        )
        .unwrap();

    let saved = draftcraft_io::write_dxf(&d);
    let back = draftcraft_io::read_dxf(saved.as_bytes()).unwrap();
    assert_eq!(regions(&back), vec![r.clone()]);
    assert_eq!(back.model.len(), 2, "the loops come back as one region, the plain polyline stays");
    let e = back.entity(h).unwrap();
    assert_eq!(e.common.layer, "Areas");
    assert!(matches!(back.entity(plain).unwrap().kind, EntityKind::LwPolyline(_)));

    // Other readers see four closed polylines on the region's layer.
    let tags = draftcraft_dxf::parse(saved.as_bytes()).unwrap();
    let ents = records(&sections(&tags).into_iter().find(|s| s.name == "ENTITIES").unwrap().tags);
    let polys: Vec<_> = ents.iter().filter(|(k, _)| k == "LWPOLYLINE").collect();
    assert_eq!(polys.len(), 5);
    assert_eq!(polys.iter().filter(|(_, t)| t.iter().any(|g| g.code == 8 && g.str() == "Areas")).count(), 4);
    assert!(!ents.iter().any(|(k, _)| k == "REGION"));

    // Saving again is stable.
    let again = draftcraft_io::read_dxf(draftcraft_io::write_dxf(&back).as_bytes()).unwrap();
    assert_eq!(regions(&again), vec![r]);
}

#[test]
fn foreign_region_stays_unknown() {
    // ACIS data that reads as no region: kept as saved, with or without solids.
    let text = "0\nSECTION\n2\nENTITIES\n0\nREGION\n5\n2B\n100\nAcDbEntity\n8\n0\n100\nAcDbModelerGeometry\n70\n1\n1\nabc\n0\nENDSEC\n0\nEOF\n";
    let d = draftcraft_io::read_dxf(text.as_bytes()).unwrap();
    assert!(matches!(&d.model.iter().next().unwrap().kind, EntityKind::Unknown(u) if u.dxf_type == "REGION"));
}

#[test]
fn hostile_region_xdata_does_not_panic() {
    let mut t = String::from("0\nSECTION\n2\nENTITIES\n");
    t.push_str("0\nLWPOLYLINE\n5\n30\n8\n0\n90\n2\n70\n1\n10\n0\n20\n0\n10\n1\n20\n1\n1001\nDRAFTCRAFT\n1000\nREGION\n1002\n{\n");
    for _ in 0..12_000 {
        t.push_str("1000\nLOOP\n1070\n7\n1071\n-1\n1010\nnan\n1020\ninf\n1040\n1e308\n1010\n1\n1020\n2\n");
    }
    t.push_str("0\nLWPOLYLINE\n5\n31\n8\n0\n90\n1\n10\nnan\n1001\nDRAFTCRAFT\n1000\nREGION\n");
    t.push_str("0\nLWPOLYLINE\n5\n32\n8\n0\n90\n1\n10\n0\n1001\nDRAFTCRAFT\n1000\nREGIONPART\n");
    t.push_str("0\nENDSEC\n0\nEOF\n");
    let d = draftcraft_io::read_dxf(t.as_bytes()).unwrap();
    for r in regions(&d) {
        assert!(r.loops.len() <= draftcraft_doc::MAX_REGION_LOOPS);
        let _ = r.area();
        let _ = r.props();
    }
    let _ = draftcraft_io::write_dxf(&d);
    let _ = draftcraft_render::build(&d, &Space::Model, &draftcraft_render::Options::default());
}
