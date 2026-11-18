use draftcraft_doc::{EntityKind, Solid3d};
use draftcraft_geom::Vec3;
use serde_json::{Value, json};

use crate::Session;

fn solid(s: &Session, h: &str) -> Solid3d {
    let d = s.doc().unwrap();
    let e = d.model.iter().find(|e| e.handle.hex() == h).unwrap_or_else(|| panic!("no entity {h}"));
    match &e.kind {
        EntityKind::Solid3d(so) => so.clone(),
        other => panic!("not a solid: {other:?}"),
    }
}

fn h(r: &Value) -> String {
    r["handle"].as_str().unwrap().to_string()
}

fn count(s: &Session, name: &str) -> usize {
    s.doc().unwrap().model.iter().filter(|e| e.kind.type_name() == name).count()
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

#[test]
fn primitives_by_json() {
    let mut s = Session::new();
    let b = s.execute("box", &json!({"corner1": [10, 10], "corner2": [12, 13], "height": 4})).unwrap();
    assert!(near(solid(&s, &h(&b)).volume, 24.0, 1e-9));
    let c = s.execute("cylinder", &json!({"center": [0, 0], "radius": 1, "height": 2})).unwrap();
    assert!(near(solid(&s, &h(&c)).volume, std::f64::consts::TAU, 1e-3));
    let sp = s.execute("sphere", &json!({"center": [5, 5, 5], "radius": 1})).unwrap();
    assert!(near(solid(&s, &h(&sp)).volume, 4.0 / 3.0 * std::f64::consts::PI, 1e-3));
    let w = s.execute("wedge", &json!({"corner1": [0, 0], "corner2": [2, 3], "height": 4})).unwrap();
    assert!(near(solid(&s, &h(&w)).volume, 12.0, 1e-9));
    let t = s.execute("torus", &json!({"center": [0, 0], "radius": 3, "tubeRadius": 1})).unwrap();
    assert!(near(solid(&s, &h(&t)).volume, 2.0 * std::f64::consts::PI.powi(2) * 3.0, 1e-3));
    let k = s.execute("cone", &json!({"center": [0, 0], "radius": 1, "height": 3})).unwrap();
    assert!(near(solid(&s, &h(&k)).volume, std::f64::consts::PI, 1e-3));
    // The box sits where it was drawn: its plan edges span 10..12 x 10..13.
    let so = solid(&s, &h(&b));
    let xs: Vec<f64> = so.plan_segments().iter().flat_map(|(a, b)| [a.x, b.x]).collect();
    assert!(xs.iter().all(|x| (9.999..=12.001).contains(x)), "{xs:?}");
}

#[test]
fn bad_primitive_input_is_an_error_not_a_crash() {
    let mut s = Session::new();
    for (id, p) in [
        ("box", json!({"corner1": [0, 0], "corner2": [0, 0], "height": 1})),
        ("box", json!({"corner1": [0, 0], "corner2": [1, 1], "height": f64::MAX})),
        ("cylinder", json!({"center": [0, 0], "radius": -1, "height": 2})),
        ("sphere", json!({"center": [0, 0], "radius": 0})),
        ("torus", json!({"center": [0, 0], "radius": 1, "tubeRadius": 2})),
        ("sphere", json!({"center": ["a", 0], "radius": 1})),
    ] {
        assert!(s.execute(id, &p).is_err(), "{id} {p}");
    }
    assert_eq!(count(&s, "3D Solid"), 0);
}

#[test]
fn box_at_the_command_line() {
    let mut s = Session::new();
    s.cmdline("box 0,0 2,3 4").unwrap();
    let d = s.doc().unwrap();
    let so = match &d.model.last().unwrap().kind {
        EntityKind::Solid3d(so) => so.clone(),
        other => panic!("{other:?}"),
    };
    assert!(near(so.volume, 24.0, 1e-9));
}

#[test]
fn extrude_and_revolve_profiles() {
    let mut s = Session::new();
    let pl = s.execute("rectang", &json!({"p1": [0, 0], "p2": [4, 2]})).unwrap();
    let r = s.execute("extrude", &json!({"handles": [h(&pl)], "height": 3})).unwrap();
    let hs = r["handles"].as_array().unwrap();
    assert_eq!(hs.len(), 1);
    assert!(near(solid(&s, hs[0].as_str().unwrap()).volume, 24.0, 1e-9));
    // The profile is gone (DELOBJ).
    assert_eq!(count(&s, "Polyline"), 0);

    let c = s.execute("circle", &json!({"center": [10, 0], "radius": 1})).unwrap();
    let r = s.execute("extrude", &json!({"handles": [h(&c)], "height": 2, "keepProfiles": true})).unwrap();
    let so = solid(&s, r["handles"][0].as_str().unwrap());
    assert!(near(so.volume, std::f64::consts::TAU, 1e-3));
    assert_eq!(count(&s, "Circle"), 1);

    // A unit square 2 from the Y axis, turned about it: a ring of volume 2π · 2.5 · 1.
    let sq = s.execute("rectang", &json!({"p1": [2, 0], "p2": [3, 1]})).unwrap();
    let r = s.execute("revolve", &json!({"handles": [h(&sq)], "axisStart": [0, 0], "axisEnd": [0, 1]})).unwrap();
    let so = solid(&s, r["handles"][0].as_str().unwrap());
    assert!(near(so.volume, std::f64::consts::TAU * 2.5, 1e-3), "{}", so.volume);

    // Open curves aren't profiles.
    s.execute("line", &json!({"points": [[0, 0], [1, 1]]})).unwrap();
    let l = s.doc().unwrap().model.last().unwrap().handle.hex();
    assert!(s.execute("extrude", &json!({"handles": [l], "height": 1})).is_err());
}

#[test]
fn booleans_through_the_region_commands() {
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap());
    let b = h(&s.execute("box", &json!({"corner1": [5, 5], "corner2": [15, 15], "height": 10})).unwrap());
    let r = s.execute("union", &json!({"handles": [a, b]})).unwrap();
    assert_eq!(h(&r), a);
    assert!(near(solid(&s, &a).volume, 1750.0, 1e-9));
    assert_eq!(count(&s, "3D Solid"), 1);

    let c = h(&s.execute("cylinder", &json!({"center": [2, 2], "radius": 1, "height": 10})).unwrap());
    s.execute("subtract", &json!({"from": [a], "remove": [c]})).unwrap();
    assert!(near(solid(&s, &a).volume, 1750.0 - 10.0 * std::f64::consts::PI, 1e-3));

    let d = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [1, 1], "height": 1})).unwrap());
    let e = h(&s.execute("box", &json!({"corner1": [5, 5], "corner2": [6, 6], "height": 1})).unwrap());
    let r = s.execute("intersect", &json!({"handles": [d, e]})).unwrap();
    assert!(r["handle"].is_null());
    assert!(s.doc().unwrap().model.iter().all(|x| x.handle.hex() != d && x.handle.hex() != e));
}

#[test]
fn moved_solids_still_combine() {
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [2, 2], "height": 2})).unwrap());
    let b = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [2, 2], "height": 2})).unwrap());
    s.execute("move", &json!({"handles": [b], "from": [0, 0], "to": [1, 1]})).unwrap();
    s.execute("rotate", &json!({"handles": [a], "base": [1, 1], "angle": 90})).unwrap();
    s.execute("intersect", &json!({"handles": [a, b]})).unwrap();
    assert!(near(solid(&s, &a).volume, 2.0, 1e-9), "{}", solid(&s, &a).volume);
}

#[test]
fn massprop_of_a_solid() {
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [1, 2], "corner2": [3, 6], "height": 1})).unwrap());
    let r = s.execute("massprop", &json!({"handles": [a]})).unwrap();
    let m = &r["solids"][0];
    assert!(near(m["volume"].as_f64().unwrap(), 8.0, 1e-9));
    assert!(near(m["centroid"][0].as_f64().unwrap(), 2.0, 1e-9));
    assert!(near(m["centroid"][1].as_f64().unwrap(), 4.0, 1e-9));
    assert!(near(m["centroid"][2].as_f64().unwrap(), 0.5, 1e-9));
    // A 2 x 4 x 1 box: Ixx about its centroid is m (4² + 1²) / 12, about the origin that plus
    // m (cy² + cz²); Ixy about the origin is m cx cy.
    assert!(near(m["centroidMoments"][0].as_f64().unwrap(), 8.0 * 17.0 / 12.0, 1e-6), "{m}");
    assert!(near(m["momentsOfInertia"][0].as_f64().unwrap(), 8.0 * 17.0 / 12.0 + 8.0 * 16.25, 1e-6), "{m}");
    assert!(near(m["productsOfInertia"]["xy"].as_f64().unwrap(), 64.0, 1e-6), "{m}");
    assert!(r["message"].as_str().unwrap().contains("SOLIDS"));
    assert!(r["message"].as_str().unwrap().contains("Moments of inertia"));
}

#[test]
fn slice_keeps_both_or_one_side() {
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 2], "height": 2})).unwrap());
    let r = s.execute("slice", &json!({"handles": [a], "origin": [4, 0, 0], "normal": [1, 0, 0]})).unwrap();
    assert_eq!(r["handles"].as_array().unwrap().len(), 2);
    let total: f64 = r["handles"].as_array().unwrap().iter().map(|x| solid(&s, x.as_str().unwrap()).volume).sum();
    assert!(near(total, 40.0, 1e-9));
    let b = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 2], "height": 2})).unwrap());
    s.execute("slice", &json!({"handles": [b], "points": [[4, -1], [4, 5]], "keep": [9, 1, 1]})).unwrap();
    assert!(near(solid(&s, &b).volume, 24.0, 1e-9));
}

#[test]
fn slice_xy_takes_the_typed_height_and_side() {
    // Review of PR 20: the XY plane was always at Z=0 and the upper part always kept.
    let run = |side: &str| {
        let mut s = Session::new();
        let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap());
        s.set_selection(Vec::new());
        s.cmdline("slice").unwrap();
        s.cmdline("10,5").unwrap();
        s.cmdline("").unwrap();
        s.cmdline("xy").unwrap();
        s.cmdline("0,0,3").unwrap();
        s.cmdline(side).unwrap();
        (s.running.is_some(), solid(&s, &a).volume, s, a)
    };
    // Below the plane at Z=3: the lower part.
    let (_, v, ..) = run("1,1,1");
    assert!(near(v, 300.0, 1e-6), "{v}");
    // Above it.
    let (_, v, ..) = run("1,1,9");
    assert!(near(v, 700.0, 1e-6), "{v}");
    // A point picked in plan (Z=0) is below.
    let (_, v, ..) = run("1,1");
    assert!(near(v, 300.0, 1e-6), "{v}");
    // On the plane: asked again, nothing sliced yet.
    let (running, v, mut s, a) = run("2,2,3");
    assert!(running && near(v, 1000.0, 1e-6), "{v}");
    // `@` from there: 4 above it.
    s.cmdline("@0,0,4").unwrap();
    assert!(s.running.is_none());
    assert!(near(solid(&s, &a).volume, 700.0, 1e-6));
    // By JSON a side point on the plane is refused.
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap());
    assert!(s.execute("slice", &json!({"handles": [a], "origin": [0, 0, 3], "normal": [0, 0, 1], "keep": [5, 5, 3]})).is_err());
    assert!(near(solid(&s, &a).volume, 1000.0, 1e-9));
}

#[test]
fn edges_and_faces_are_picked_along_the_view() {
    // Review of PR 20: picking used the plan projection, so the top and bottom edges of a box
    // tied (the first won), faces were always the topmost, and 3D clicks went to Z=0.
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap());
    let hd = s.doc().unwrap().model.iter().find(|e| e.handle.hex() == a).unwrap().handle;
    let so = solid(&s, &a);
    let at = draftcraft_geom::Vec2::new(10.0, 5.0).to3(0.0);
    // From above, the top edge; from below, the bottom one.
    let (_, q) = super::pick_edge(&s, at, super::DOWN, &[]).unwrap().unwrap();
    assert!(near(q.z, 10.0, 1e-9), "{q:?}");
    let (_, q) = super::pick_edge(&s, at, Vec3::Z, &[hd]).unwrap().unwrap();
    assert!(near(q.z, 0.0, 1e-9), "{q:?}");
    let mid = draftcraft_geom::Vec2::new(5.0, 5.0).to3(0.0);
    assert!(near(super::pick_face(&so, mid, super::DOWN).unwrap().z, 10.0, 1e-9));
    assert!(near(super::pick_face(&so, mid, Vec3::Z).unwrap().z, 0.0, 1e-9));
    // Looking along +Y (the front view), the face at Y=0.
    let q = super::pick_face(&so, Vec3::new(5.0, -50.0, 5.0), Vec3::Y).unwrap();
    assert!(near(q.y, 0.0, 1e-9), "{q:?}");
    // In a view from below, a click on the edge fillets the bottom edge.
    let v = crate::view3d::View3d::new(Vec3::Z, draftcraft_geom::Vec2::new(5.0, 5.0), 40.0, crate::view3d::VisualStyle::Wireframe).unwrap();
    s.set_view3d(Some(v));
    let (at, dir) = super::sight(&s, draftcraft_geom::Vec2::new(10.0, 5.0));
    assert_eq!(dir, Vec3::Z);
    let (_, q) = super::pick_edge(&s, at, dir, &[]).unwrap().unwrap();
    assert!(near(q.z, 0.0, 1e-9), "{q:?}");
}

#[test]
fn a_failing_command_leaves_nothing_half_done() {
    // Review of PR 20: SUBTRACT of solids and regions changed the solids, then failed on the
    // regions, and the drawing kept the half-done work.
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap());
    let b = h(&s.execute("box", &json!({"corner1": [5, 5], "corner2": [15, 15], "height": 10})).unwrap());
    let rect = s.execute("rectang", &json!({"p1": [30, 0], "p2": [40, 10]})).unwrap();
    let r = s.execute("region", &json!({"handles": [rect["handle"]]})).unwrap();
    let reg = r["handles"].get(0).cloned().unwrap_or_else(|| r["handle"].clone());
    let before = s.doc().unwrap().clone();
    // The region taken from itself fails after the solids were done.
    assert!(s.execute("subtract", &json!({"from": [a, reg], "remove": [b, reg]})).is_err());
    let after = s.doc().unwrap();
    assert!(near(solid(&s, &a).volume, 1000.0, 1e-9));
    assert!(near(solid(&s, &b).volume, 1000.0, 1e-9), "the removed solid is still there");
    assert_eq!(after.model.iter().count(), before.model.iter().count());
}

#[test]
fn fillet_chamfer_and_shell() {
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap());
    s.execute("filletedge", &json!({"handle": a, "edges": [[10, 5, 10]], "radius": 1})).unwrap();
    let v = solid(&s, &a).volume;
    assert!(near(v, 1000.0 - (1.0 - std::f64::consts::PI / 4.0) * 10.0, 1e-3), "{v}");
    let b = h(&s.execute("box", &json!({"corner1": [20, 0], "corner2": [30, 10], "height": 10})).unwrap());
    // An edge picked in plan: the nearest edge seen from above.
    s.execute("chamferedge", &json!({"handle": b, "edges": [[30, 5, 10]], "distance": 1})).unwrap();
    assert!(near(solid(&s, &b).volume, 995.0, 1e-6));
    let c = h(&s.execute("box", &json!({"corner1": [40, 0], "corner2": [50, 10], "height": 10})).unwrap());
    s.execute("solidedit", &json!({"handle": c, "operation": "shell", "thickness": 1, "remove": [[45, 5]]})).unwrap();
    // Open at the top: 1000 − 8·8·9.
    assert!(near(solid(&s, &c).volume, 1000.0 - 576.0, 1e-6), "{}", solid(&s, &c).volume);
}

#[test]
fn interfere_reports_and_creates() {
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [2, 2], "height": 2})).unwrap());
    let b = h(&s.execute("box", &json!({"corner1": [1, 1], "corner2": [3, 3], "height": 2})).unwrap());
    let r = s.execute("interfere", &json!({"first": [a, b]})).unwrap();
    assert_eq!(r["pairs"].as_array().unwrap().len(), 1);
    assert!(near(r["pairs"][0]["volume"].as_f64().unwrap(), 2.0, 1e-9));
    assert_eq!(count(&s, "3D Solid"), 2);
    s.execute("interfere", &json!({"first": [a], "second": [b], "create": true})).unwrap();
    assert_eq!(count(&s, "3D Solid"), 3);
}

#[test]
fn interfere_skips_solids_far_apart_and_scales_volumes() {
    // Review of PR 20: every pair went through the kernel. Boxes far apart are ruled out first;
    // a scaled solid's interference is in drawing units.
    let mut s = Session::new();
    let mut hs = Vec::new();
    for i in 0..30 {
        let x = f64::from(i) * 100.0;
        hs.push(h(&s.execute("box", &json!({"corner1": [x, 0], "corner2": [x + 2.0, 2], "height": 2})).unwrap()));
    }
    let r = s.execute("interfere", &json!({"first": hs})).unwrap();
    assert!(r["pairs"].as_array().unwrap().is_empty());
    let a = h(&s.execute("box", &json!({"corner1": [0, 50], "corner2": [2, 52], "height": 2})).unwrap());
    let b = h(&s.execute("box", &json!({"corner1": [1, 51], "corner2": [3, 53], "height": 2})).unwrap());
    s.execute("scale", &json!({"handles": [a, b], "base": [0, 50], "factor": 2})).unwrap();
    let r = s.execute("interfere", &json!({"first": [a, b]})).unwrap();
    // Twice as large each way: 2 × 2 × 2 × 2.
    assert!(near(r["pairs"][0]["volume"].as_f64().unwrap(), 16.0, 1e-9), "{r}");
}

#[test]
fn flatshot_draws_lines_and_hidden_lines() {
    let mut s = Session::new();
    s.execute("box", &json!({"corner1": [0, 0], "corner2": [4, 2], "height": 3})).unwrap();
    let r = s.execute("flatshot", &json!({"view": "front", "base": [100, 0]})).unwrap();
    let n = r["handles"].as_array().unwrap().len();
    assert_eq!(n, 4, "a box seen from the front is a rectangle");
    let r = s.execute("flatshot", &json!({"view": "top"})).unwrap();
    assert_eq!(r["handles"].as_array().unwrap().len(), 4);
    let hole = s.execute("cylinder", &json!({"center": [2, 1], "radius": 0.5, "height": 3})).unwrap();
    let r = s.execute("flatshot", &json!({"handles": [h(&hole)], "view": "front", "showHidden": true})).unwrap();
    assert!(!r["handles"].as_array().unwrap().is_empty());
}

#[test]
fn section_makes_regions() {
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 4], "height": 6})).unwrap());
    let c = h(&s.execute("cylinder", &json!({"center": [5, 2], "radius": 1, "height": 6})).unwrap());
    s.execute("subtract", &json!({"from": [a], "remove": [c]})).unwrap();
    let r = s.execute("section", &json!({"handles": [a], "plane": "xy", "origin": [0, 0, 3]})).unwrap();
    let hs = r["handles"].as_array().unwrap();
    assert_eq!(hs.len(), 1);
    let m = s.execute("massprop", &json!({"handles": [hs[0]]})).unwrap();
    let area = m["regions"][0]["area"].as_f64().unwrap();
    assert!(near(area, 40.0 - std::f64::consts::PI, 1e-2), "{area}");
    // A vertical cut through the middle of the hole: two 4 x 6 strips... less the hole: 10x6 − 2x6.
    let r = s.execute("section", &json!({"handles": [a], "points": [[0, 2], [10, 2]], "base": [50, 0]})).unwrap();
    let total: f64 = r["handles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| s.execute("massprop", &json!({"handles": [x]})).unwrap()["regions"][0]["area"].as_f64().unwrap())
        .sum();
    assert!(near(total, 48.0, 1e-2), "{total}");
    // A plane that misses: an error.
    assert!(s.execute("section", &json!({"handles": [a], "plane": "xy", "origin": [0, 0, 30]})).is_err());
}

#[test]
fn move_rotate_mirror_scale_keep_the_body() {
    let mut s = Session::new();
    let a = h(&s.execute("box", &json!({"corner1": [0, 0], "corner2": [1, 2], "height": 3})).unwrap());
    let before = solid(&s, &a).body.clone();
    s.execute("move", &json!({"handles": [a], "from": [0, 0], "to": [100, 50]})).unwrap();
    s.execute("mirror", &json!({"handles": [a], "p1": [0, 0], "p2": [0, 1], "erase": true})).unwrap();
    let after = solid(&s, &a);
    assert_eq!(after.body, before);
    assert!(near(after.volume, 6.0, 1e-9));
    let w = after.world(Vec3::new(0.5, 1.0, 0.0));
    assert!(near(w.x, -100.5, 1e-9) && near(w.y, 51.0, 1e-9), "{w:?}");
}

#[test]
fn views_turn_and_go_back_to_plan_without_touching_the_drawing() {
    let mut s = Session::new();
    s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap();
    let rev = s.state().unwrap().revision;
    assert!(s.view3d().is_none());
    let r = s.execute("vpoint", &json!({"view": "seiso"})).unwrap();
    assert_eq!(r["style"], "wireframe");
    let v = s.view3d().unwrap();
    // South-east, above: looking toward -x, +y, -z.
    assert!(v.dir.x < 0.0 && v.dir.y > 0.0 && v.dir.z < 0.0, "{:?}", v.dir);
    // Framed around the box (its far corners fit on screen).
    let (q, _) = v.to_plane(Vec3::new(10.0, 10.0, 10.0));
    assert!((q - v.center).len() < v.height, "{q:?} {v:?}");
    // VPOINT x,y,z is where the viewer stands.
    s.execute("vpoint", &json!({"point": [0, -1, 0]})).unwrap();
    assert!((s.view3d().unwrap().dir - Vec3::Y).len() < 1e-12);
    s.execute("vscurrent", &json!({"style": "Shaded with edges"})).unwrap();
    assert_eq!(s.view3d().unwrap().style, crate::view3d::VisualStyle::ShadedWithEdges);
    s.execute("3dorbit", &json!({"azimuth": 90})).unwrap();
    let d = s.view3d().unwrap().dir;
    assert!((d - Vec3::new(-1.0, 0.0, 0.0)).len() < 1e-9, "{d:?}");
    s.execute("plan", &json!({})).unwrap();
    assert!(s.view3d().is_none());
    // From the plan view a style starts at the top view; 2D Wireframe goes back to plan.
    s.execute("vscurrent", &json!({"style": "hidden"})).unwrap();
    assert!((s.view3d().unwrap().dir - Vec3::new(0.0, 0.0, -1.0)).len() < 1e-12);
    s.execute("vscurrent", &json!({"style": "2dwireframe"})).unwrap();
    assert!(s.view3d().is_none());
    assert_eq!(s.state().unwrap().revision, rev, "views never change the drawing");
    assert_eq!(s.state().unwrap().undo.len(), 1, "only the box is undoable");
    for bad in [json!({"view": "sideways"}), json!({"point": [0, 0, 0]}), json!({}), json!({"point": "x"})] {
        assert!(s.execute("vpoint", &bad).is_err(), "{bad}");
    }
    assert!(s.execute("vscurrent", &json!({"style": "sketchy-ish"})).is_err());
    // Huge angles still leave a usable view.
    s.execute("3dorbit", &json!({"azimuth": 1e308, "elevation": -1e308})).unwrap();
    assert!(s.view3d().unwrap().dir.is_finite());
}

/// The preset view and style commands behind the ribbon and the viewport controls.
#[test]
fn preset_views_and_styles_are_commands_and_name_the_view() {
    use crate::view3d::{PRESET_VIEWS, VisualStyle, preset_of};
    let mut s = Session::new();
    s.execute("box", &json!({"corner1": [0, 0], "corner2": [10, 10], "height": 10})).unwrap();
    for (id, label) in PRESET_VIEWS {
        s.execute(id, &json!({})).unwrap();
        assert_eq!(preset_of(s.view3d().unwrap().dir), Some((id, label)), "{id}");
    }
    s.execute("vpoint.front", &json!({})).unwrap();
    assert!((s.view3d().unwrap().dir - Vec3::Y).len() < 1e-12, "front looks toward +y");
    // A view between the presets is a custom one.
    s.execute("3dorbit", &json!({"azimuth": 10})).unwrap();
    assert_eq!(preset_of(s.view3d().unwrap().dir), None);
    for (id, style) in
        [("vscurrent.realistic", VisualStyle::Realistic), ("vscurrent.xray", VisualStyle::XRay), ("vscurrent.hidden", VisualStyle::Hidden)]
    {
        s.execute(id, &json!({})).unwrap();
        assert_eq!(s.view3d().unwrap().style, style, "{id}");
    }
    s.execute("vscurrent.2dwireframe", &json!({})).unwrap();
    assert!(s.view3d().is_none(), "2D Wireframe is the plan view");
    assert_eq!(s.state().unwrap().undo.len(), 1, "only the box is undoable");
}

#[test]
fn interactive_orbit_starts_a_3d_view_and_vpoint_takes_typed_points() {
    let mut s = Session::new();
    s.cmdline("3dorbit").unwrap();
    assert!(s.view3d().is_some(), "3DORBIT turns the plan view into the top view");
    s.cmdline("").unwrap();
    assert!(s.running.is_none());
    s.cmdline("vpoint").unwrap();
    s.cmdline("1,1,1").unwrap();
    let d = s.view3d().unwrap().dir;
    assert!(d.x < 0.0 && d.y < 0.0 && d.z < 0.0, "{d:?}");
    s.cmdline("vs").unwrap();
    s.cmdline("conceptual").unwrap();
    assert_eq!(s.view3d().unwrap().style, crate::view3d::VisualStyle::Conceptual);
}
