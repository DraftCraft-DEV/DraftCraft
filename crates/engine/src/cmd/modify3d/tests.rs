#![allow(clippy::unwrap_used, clippy::panic)]

use draftcraft_doc::{Arc, Circle, EntityKind, Handle, Line};
use draftcraft_geom::{Mat4, Vec3};
use serde_json::{Value, json};

use super::align;
use crate::Session;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).len() < 1e-9
}

fn line(s: &mut Session, a: [f64; 3], b: [f64; 3]) -> Handle {
    s.add_entity(EntityKind::Line(Line { a: Vec3::new(a[0], a[1], a[2]), b: Vec3::new(b[0], b[1], b[2]) })).unwrap()
}

fn ends(s: &Session, h: Handle) -> (Vec3, Vec3) {
    match &s.state().unwrap().doc.entity(h).unwrap().kind {
        EntityKind::Line(l) => (l.a, l.b),
        k => panic!("not a line: {k:?}"),
    }
}

fn hx(h: Handle) -> Value {
    json!([h.hex()])
}

#[test]
fn move_rotate_scale_and_mirror_in_space() {
    let mut s = Session::new();
    let h = line(&mut s, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    s.execute("3dmove", &json!({"handles": hx(h), "delta": [1, 2, 3]})).unwrap();
    assert_eq!(ends(&s, h), (Vec3::new(1.0, 2.0, 3.0), Vec3::new(11.0, 2.0, 3.0)));
    // A quarter turn about Y through (1,2,3): +X goes to −Z.
    s.execute("3drotate", &json!({"handles": hx(h), "base": [1, 2, 3], "axis": "y", "angle": 90})).unwrap();
    let (a, b) = ends(&s, h);
    assert!(close(a, Vec3::new(1.0, 2.0, 3.0)) && close(b, Vec3::new(1.0, 2.0, -7.0)), "{a:?} {b:?}");
    s.execute("3dscale", &json!({"handles": hx(h), "base": [1, 2, 3], "factor": 2})).unwrap();
    assert!(close(ends(&s, h).1, Vec3::new(1.0, 2.0, -17.0)));
    // MIRROR3D in the XY plane keeps the source and adds the mirror image.
    let r = s.execute("mirror3d", &json!({"handles": hx(h), "plane": "xy"})).unwrap();
    let copy = Handle::parse_hex(r["handles"][0].as_str().unwrap()).unwrap();
    assert!(close(ends(&s, copy).1, Vec3::new(1.0, 2.0, 17.0)));
    assert!(close(ends(&s, h).1, Vec3::new(1.0, 2.0, -17.0)));
    // ROTATE3D about the axis from (0,0,0) to (0,0,1).
    s.execute("rotate3d", &json!({"handles": hx(copy), "p1": [0, 0, 0], "p2": [0, 0, 1], "angle": 180})).unwrap();
    assert!(close(ends(&s, copy).0, Vec3::new(-1.0, -2.0, -3.0)));
}

#[test]
fn a_circle_turned_up_stands_in_its_new_plane() {
    let mut s = Session::new();
    let c = s.add_entity(EntityKind::Circle(Circle { center: Vec3::new(10.0, 0.0, 0.0), radius: 5.0 })).unwrap();
    let a = s.add_entity(EntityKind::Arc(Arc { center: Vec3::ZERO, radius: 5.0, start: 0.0, end: std::f64::consts::FRAC_PI_2 })).unwrap();
    s.execute("3drotate", &json!({"handles": [c.hex(), a.hex()], "base": [0, 0, 0], "axis": "x", "angle": 90})).unwrap();
    let st = s.state().unwrap();
    let e = st.doc.entity(c).unwrap();
    assert!(close(e.common.extrusion, Vec3::new(0.0, -1.0, 0.0)), "{:?}", e.common.extrusion);
    let EntityKind::Circle(ci) = &e.kind else { panic!() };
    assert!(close(ci.center, Vec3::new(10.0, 0.0, 0.0)));
    // The arc's ends are where the turn takes them: (5,0,0) stays, (0,5,0) goes to (0,0,5).
    let e = st.doc.entity(a).unwrap();
    let EntityKind::Arc(ar) = &e.kind else { panic!() };
    let m = Mat4::ocs(e.common.extrusion);
    let x = Vec3::new(m.m[0][0], m.m[1][0], m.m[2][0]);
    let y = Vec3::new(m.m[0][1], m.m[1][1], m.m[2][1]);
    let at = |t: f64| ar.center + x * (ar.radius * t.cos()) + y * (ar.radius * t.sin());
    assert!(close(at(ar.start), Vec3::new(5.0, 0.0, 0.0)), "{:?}", at(ar.start));
    assert!(close(at(ar.end), Vec3::new(0.0, 0.0, 5.0)), "{:?}", at(ar.end));
}

#[test]
fn text_that_cant_tilt_is_left_and_said() {
    let mut s = Session::new();
    s.cmdline("text 0,0 2 0 hello").unwrap();
    let t = s.state().unwrap().doc.model.iter().map(|e| e.handle).last().unwrap();
    let before = s.state().unwrap().doc.entity(t).unwrap().clone();
    let r = s.execute("3drotate", &json!({"handles": hx(t), "base": [0, 0, 0], "axis": "x", "angle": 30})).unwrap();
    assert_eq!(r["left"], 1);
    assert_eq!(*s.state().unwrap().doc.entity(t).unwrap(), before);
    // A turn about Z is a plan rotation: text takes it.
    let r = s.execute("3drotate", &json!({"handles": hx(t), "base": [0, 0, 0], "angle": 30})).unwrap();
    assert_eq!(r["left"], 0);
}

#[test]
fn align_by_one_two_and_three_points() {
    let p = |x: f64, y: f64, z: f64| Vec3::new(x, y, z);
    let m = align(&[p(1.0, 1.0, 1.0)], &[p(2.0, 3.0, 4.0)], false).unwrap();
    assert!(close(m.apply(p(1.0, 1.0, 1.0)), p(2.0, 3.0, 4.0)));
    let m = align(&[p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0)], &[p(5.0, 5.0, 5.0), p(5.0, 5.0, 7.0)], true).unwrap();
    assert!(close(m.apply(p(1.0, 0.0, 0.0)), p(5.0, 5.0, 7.0)));
    let src = [p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0)];
    let dst = [p(10.0, 0.0, 0.0), p(10.0, 0.0, 1.0), p(10.0, 1.0, 0.0)];
    let m = align(&src, &dst, false).unwrap();
    for (a, b) in src.iter().zip(&dst) {
        assert!(close(m.apply(*a), *b), "{:?} → {:?}", a, m.apply(*a));
    }
    assert!(m.det3() > 0.0, "3DALIGN never mirrors");
    assert!(align(&[p(0.0, 0.0, 0.0), p(0.0, 0.0, 0.0)], &[p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0)], false).is_err());
    assert!(align(&[], &[], false).is_err());
}

#[test]
fn array_in_rows_columns_levels_and_round_an_axis() {
    let mut s = Session::new();
    let h = line(&mut s, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let r = s
        .execute("3darray", &json!({"handles": hx(h), "type": "rectangular", "rows": 2, "columns": 3, "levels": 2, "rowSpacing": 10, "columnSpacing": 20, "levelSpacing": 30}))
        .unwrap();
    assert_eq!(r["handles"].as_array().unwrap().len(), 11);
    let tops: Vec<Vec3> =
        s.state().unwrap().doc.model.iter().filter_map(|e| if let EntityKind::Line(l) = &e.kind { Some(l.a) } else { None }).collect();
    assert!(tops.contains(&Vec3::new(40.0, 10.0, 30.0)));
    let r = s.execute("3darray", &json!({"handles": hx(h), "type": "polar", "count": 4, "p1": [0, 0, 0], "p2": [1, 0, 0]})).unwrap();
    assert_eq!(r["handles"].as_array().unwrap().len(), 3);
    // Hostile sizes are refused before anything is made.
    assert!(s.execute("3darray", &json!({"handles": hx(h), "rows": 100000, "columns": 100000})).is_err());
    assert!(s.execute("3darray", &json!({"handles": hx(h), "type": "polar", "count": 4, "p1": [0, 0, 0], "p2": [0, 0, 0]})).is_err());
}

#[test]
fn prompts_take_typed_points_in_space() {
    let mut s = Session::new();
    let h = line(&mut s, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    s.set_selection(vec![h]);
    s.cmdline("3dmove 0,0,0 5,0,7").unwrap();
    assert_eq!(ends(&s, h).0, Vec3::new(5.0, 0.0, 7.0));
    s.set_selection(vec![h]);
    s.cmdline("3drotate 5,0,7 y 90").unwrap();
    assert!(close(ends(&s, h).1, Vec3::new(5.0, 0.0, -3.0)), "{:?}", ends(&s, h));
    s.set_selection(vec![h]);
    s.cmdline("mirror3d yz 0,0,0 y").unwrap();
    assert!(close(ends(&s, h).0, Vec3::new(-5.0, 0.0, 7.0)), "{:?}", ends(&s, h));
    s.set_selection(vec![h]);
    s.cmdline("3dalign -5,0,7 c 0,0,0").unwrap();
    assert!(close(ends(&s, h).0, Vec3::ZERO), "{:?}", ends(&s, h));
    s.set_selection(vec![h]);
    s.cmdline("3darray r 1 2 1 4").unwrap();
    let n = s.state().unwrap().doc.model.iter().filter(|e| matches!(e.kind, EntityKind::Line(_))).count();
    assert_eq!(n, 2);
}
