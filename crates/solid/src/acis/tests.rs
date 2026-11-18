use draftcraft_geom::Vec3;

use super::*;
use crate::body::{BoolOp, boolean, box_solid, cone, cylinder, sphere, torus};

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-9)
}

/// Written as SAT and as SAB, each read back: the same volume and centroid.
fn round_trip(name: &str, body: &Body) {
    let mass = body.mass().expect("mass");
    let sat = write_sat(body).unwrap_or_else(|e| panic!("{name}: write SAT: {e}"));
    for (form, data) in [("SAT", sat.clone().into_bytes()), ("scrambled SAT", scramble(&sat).into_bytes()), ("SAB", write_sab(body).expect("SAB"))] {
        let back = read(&data).unwrap_or_else(|e| panic!("{name}: read {form}: {e}\n{sat}"));
        assert_eq!(back.len(), 1, "{name} {form}");
        let m = back[0].mass().expect("mass back");
        assert!(close(m.volume, mass.volume, 1e-5), "{name} {form}: volume {} → {}\n{sat}", mass.volume, m.volume);
        assert!((m.centroid - mass.centroid).len() <= 1e-5 * body.size().max(1.0), "{name} {form}: centroid {:?} → {:?}", mass.centroid, m.centroid);
        assert_eq!(back[0].face_count(), body.face_count(), "{name} {form}: faces\n{sat}");
    }
}

#[test]
fn primitives_round_trip_through_acis() {
    let z = Vec3::new(0.0, 0.0, 1.0);
    round_trip("box", &box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 60.0, 40.0)).expect("box"));
    round_trip("cylinder", &cylinder(Vec3::new(200.0, 0.0, 0.0), z, 25.0, 50.0).expect("cylinder"));
    round_trip("cone", &cone(Vec3::new(300.0, 0.0, 0.0), z, 30.0, 0.0, 60.0).expect("cone"));
    round_trip("frustum", &cone(Vec3::new(300.0, 0.0, 0.0), z, 30.0, 10.0, 60.0).expect("frustum"));
    round_trip("sphere", &sphere(Vec3::new(400.0, 0.0, 30.0), 30.0).expect("sphere"));
    round_trip("torus", &torus(Vec3::new(500.0, 0.0, 0.0), 40.0, 10.0).expect("torus"));
}

#[test]
fn a_box_with_a_hole_round_trips_through_acis() {
    let b = box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 50.0)).expect("box");
    let c = cylinder(Vec3::new(50.0, 50.0, -10.0), Vec3::new(0.0, 0.0, 1.0), 20.0, 70.0).expect("cylinder");
    let holed = boolean(&b, &c, BoolOp::Subtract).expect("subtract").expect("a solid");
    round_trip("box with a hole", &holed);
}

#[test]
/// SAT is save format 400, as AutoCAD writes it in DWG 2000: AutoCAD 2027 discards a 700 solid
/// from a DWG 2000 to 2010 file (error 75003). Format 400 has no history fields after an entity's
/// attribute, and no parameters or convexity on an edge; the header counts the records.
fn sat_text_looks_like_acis_400() {
    let b = box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 3.0)).expect("box");
    let sat = write_sat(&b).expect("sat");
    let lines: Vec<&str> = sat.lines().collect();
    let records = lines.len() - 4;
    assert_eq!(lines.first().copied(), Some(format!("400 {records} 1 0 ").as_str()));
    assert!(lines.iter().any(|l| l.starts_with("body $-1 $1 $-1 $-1 #")), "{sat}");
    for edge in lines.iter().filter(|l| l.starts_with("edge ")) {
        let words: Vec<&str> = edge.split(' ').collect();
        assert_eq!(words.len(), 8, "edge $attrib $start $end $coedge $curve sense #: {edge}");
        assert!(!edge.contains("unknown"), "{edge}");
    }
    assert_eq!(lines.iter().filter(|l| l.starts_with("face ")).count(), 6);
    assert_eq!(lines.iter().filter(|l| l.starts_with("plane-surface ")).count(), 6);
    assert_eq!(lines.iter().filter(|l| l.starts_with("edge ")).count(), 12);
    assert_eq!(lines.iter().filter(|l| l.starts_with("vertex ")).count(), 8);
    assert_eq!(lines.last().copied(), Some("End-of-ACIS-data "));
    assert_eq!(scramble(&scramble(&sat)), sat);
}

#[test]
fn hostile_acis_is_an_error_not_a_panic() {
    for data in
        [&b""[..], b"700 0 1 0", b"ACIS BinaryFile", b"garbage\n#\n", b"700 0 1 0 \n4 x 4 y 4 z \n1 1e-06 1e-10 \nbody $5 -1 $-1 $9 $-1 $-1 #\n"]
    {
        let _ = read(data);
    }
    let b = box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 3.0)).expect("box");
    let sab = write_sab(&b).expect("sab");
    for cut in [1, 10, 40, sab.len() / 2, sab.len() - 1] {
        let _ = read(&sab[..cut]);
    }
    let sat = write_sat(&b).expect("sat");
    for cut in [5, 30, sat.len() / 3, sat.len() / 2] {
        let _ = read(&sat.as_bytes()[..cut]);
    }
}

/// A cone's apex is a loop of its own, as AutoCAD writes it: one coedge on an edge with no curve
/// that starts and ends at the apex vertex. Without it AutoCAD reports the cone as an invalid
/// solid of volume 0 (seen in AutoCAD 2027: SOLIDEDIT Check, MASSPROP).
#[test]
fn a_cone_apex_is_a_point_edge_loop() {
    let z = Vec3::new(0.0, 0.0, 1.0);
    let sat = write_sat(&cone(Vec3::new(300.0, 0.0, 0.0), z, 30.0, 0.0, 60.0).expect("cone")).expect("sat");
    let lines: Vec<&str> = sat.lines().collect();
    assert_eq!(lines.iter().filter(|l| l.starts_with("loop ")).count(), 3, "base face, cone side and apex\n{sat}");
    // edge $attrib $start $end $coedge $curve sense #
    let point_edges: Vec<Vec<&str>> = lines
        .iter()
        .filter(|l| l.starts_with("edge "))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|f| f.get(5) == Some(&"$-1"))
        .collect();
    assert_eq!(point_edges.len(), 1, "one edge with no curve\n{sat}");
    assert_eq!(point_edges[0].get(2), point_edges[0].get(3), "starts and ends at the apex\n{sat}");
    assert_eq!(lines.iter().filter(|l| l.starts_with("ellipse-curve ")).count(), 1, "{sat}");
    assert_eq!(lines.iter().filter(|l| l.starts_with("vertex ")).count(), 2, "{sat}");
    // The frustum and the sphere gain no point edge.
    for body in [cone(Vec3::new(0.0, 0.0, 0.0), z, 30.0, 10.0, 60.0).expect("frustum"), sphere(Vec3::new(0.0, 0.0, 0.0), 30.0).expect("sphere")] {
        let sat = write_sat(&body).expect("sat");
        assert!(sat.lines().filter(|l| l.starts_with("edge ")).all(|l| l.split_whitespace().nth(5) != Some("$-1")), "{sat}");
    }
}

/// The area of loops read back: outer loops counter-clockwise add, holes clockwise take away.
fn signed_area(loops: &[Vec<draftcraft_geom::PolyVertex>]) -> f64 {
    loops.iter().map(|l| draftcraft_geom::Polyline { vertices: l.clone(), closed: true }.area()).sum()
}

#[test]
fn regions_round_trip_through_acis() {
    use draftcraft_geom::{PolyVertex, Vec2};
    let p = |x: f64, y: f64, b: f64| PolyVertex::with_bulge(Vec2::new(x, y), b);
    // A 100 × 60 plate (drawn clockwise: written counter-clockwise) with a round end, a circular
    // hole of radius 10 and a square hole; and a separate disc of radius 5.
    let plate = vec![p(0.0, 0.0, 0.0), p(0.0, 60.0, 0.0), p(100.0, 60.0, -1.0), p(100.0, 0.0, 0.0)];
    let circle = vec![p(30.0, 30.0, 1.0), p(50.0, 30.0, 1.0)];
    let square = vec![p(60.0, 20.0, 0.0), p(70.0, 20.0, 0.0), p(70.0, 30.0, 0.0), p(60.0, 30.0, 0.0)];
    let disc = vec![p(200.0, 0.0, 1.0), p(210.0, 0.0, 1.0)];
    let loops = vec![(plate, true), (circle, false), (square, false), (disc, true)];
    let pi = std::f64::consts::PI;
    let want = 100.0 * 60.0 + pi * 30.0 * 30.0 / 2.0 - pi * 100.0 - 100.0 + pi * 25.0;
    let sat = write_region_sat(&loops).expect("SAT");
    assert!(sat.starts_with("400 ") && sat.contains("double out"), "{sat}");
    // One lump a face: the plate and the disc.
    assert_eq!(sat.lines().filter(|l| l.starts_with("lump ")).count(), 2, "{sat}");
    for (form, data) in
        [("SAT", sat.clone().into_bytes()), ("scrambled SAT", scramble(&sat).into_bytes()), ("SAB", write_region_sab(&loops).expect("SAB"))]
    {
        let back = read_region(&data).unwrap_or_else(|e| panic!("{form}: {e}\n{sat}"));
        assert_eq!(back.len(), 4, "{form}: {back:?}");
        let got = signed_area(&back);
        assert!(close(got, want, 1e-9), "{form}: area {got}, want {want}\n{sat}");
        // The bounds of every loop are what was written.
        let xs: Vec<f64> = back.iter().flatten().map(|v| v.p.x).collect();
        assert!(xs.iter().all(|x| (-1e-9..=210.0 + 1e-9).contains(x)), "{form}: {xs:?}");
    }
    // A region isn't a solid, and a solid isn't a region.
    assert!(read(sat.as_bytes()).is_err());
    let cube = write_sat(&box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0)).expect("box")).expect("SAT");
    assert!(read_region(cube.as_bytes()).is_err());
}

#[test]
fn hostile_regions_never_crash() {
    use draftcraft_geom::{PolyVertex, Vec2};
    let p = |x: f64, y: f64, b: f64| PolyVertex::with_bulge(Vec2::new(x, y), b);
    // Nothing to write: no loops, degenerate loops, not-a-number corners.
    assert!(write_region_sat(&[]).is_err());
    assert!(write_region_sat(&[(vec![p(0.0, 0.0, 0.0)], true)]).is_err());
    assert!(write_region_sat(&[(vec![p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0)], true)]).is_err());
    assert!(write_region_sat(&[(vec![p(f64::NAN, 0.0, 0.0), p(1.0, f64::INFINITY, 0.0), p(0.0, 1.0, f64::NAN)], true)]).is_err());
    // A hole alone has no face to go in.
    assert!(write_region_sat(&[(vec![p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0)], false)]).is_err());
    // Garbled data: an error, never a panic.
    let sat = write_region_sat(&[(vec![p(0.0, 0.0, 0.0), p(10.0, 0.0, 0.5), p(10.0, 10.0, 0.0), p(0.0, 10.0, 0.0)], true)]).expect("SAT");
    for cut in (0..sat.len()).step_by(37) {
        let _ = read_region(sat.get(..cut).unwrap_or("").as_bytes());
        let garbled: String = sat.chars().enumerate().map(|(i, c)| if i >= cut && c.is_ascii_digit() { '9' } else { c }).collect();
        let _ = read_region(garbled.as_bytes());
    }
    assert!(read_region(&[]).is_err());
}
