#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use draftcraft_geom::{Mat4, Vec2, Vec3};

use crate::*;

fn rect(w: f64, h: f64) -> Profile {
    let p = [Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, h), Vec2::new(0.0, h)];
    Profile { outer: (0..4).map(|i| Seg::Line { a: p[i], b: p[(i + 1) % 4] }).collect(), holes: Vec::new() }
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1.0)
}

#[test]
fn spike_extrude_a_rectangle_tessellate_and_measure() {
    let b = extrude(&Frame::xy(0.0), &rect(10.0, 5.0), 2.0, 0.0).unwrap().pop().unwrap();
    let m = b.mass().unwrap();
    assert!(close(m.volume, 100.0, 1e-9), "volume {}", m.volume);
    assert!(close(m.area, 2.0 * (50.0 + 20.0 + 10.0), 1e-9), "area {}", m.area);
    let d = b.display(0.0).unwrap();
    assert!(d.triangles.len() >= 12);
    assert_eq!(d.wires.len(), 12, "a box has 12 edges");
    assert_eq!(b.face_count(), 6);
}

#[test]
fn primitives_have_their_volumes() {
    let pi = std::f64::consts::PI;
    let v = |b: Body| b.mass().unwrap().volume;
    assert!(close(v(box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 3.0, 4.0)).unwrap()), 24.0, 1e-9));
    // Curved bodies are measured from a fine tessellation: to about 1e-4.
    assert!(close(v(cylinder(Vec3::ZERO, Vec3::Z, 1.0, 2.0).unwrap()), 2.0 * pi, 1e-3));
    assert!(close(v(sphere(Vec3::ZERO, 2.0).unwrap()), 4.0 / 3.0 * pi * 8.0, 1e-3));
    assert!(close(v(torus(Vec3::ZERO, 5.0, 1.0).unwrap()), 2.0 * pi * pi * 5.0, 1e-3));
    assert!(close(v(cone(Vec3::ZERO, Vec3::Z, 3.0, 0.0, 4.0).unwrap()), pi * 9.0 * 4.0 / 3.0, 1e-3));
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    assert!(box_solid(Vec3::ZERO, Vec3::ZERO).is_err());
    assert!(box_solid(Vec3::ZERO, Vec3::new(f64::NAN, 1.0, 1.0)).is_err());
    assert!(cylinder(Vec3::ZERO, Vec3::ZERO, 1.0, 1.0).is_err());
    assert!(sphere(Vec3::ZERO, -1.0).is_err());
    assert!(torus(Vec3::ZERO, 1.0, 2.0).is_err());
    assert!(extrude(&Frame::xy(0.0), &Profile::default(), 1.0, 0.0).is_err());
    assert!(extrude(&Frame::xy(0.0), &rect(1.0, 1.0), 0.0, 0.0).is_err());
    assert!(Body::decode(b"SCBREP04garbage").is_err());
    assert!(Body::decode(b"DCSMB001garbage").is_err());
    assert!(Body::decode(b"DCSMB001").is_err());
    assert!(Body::decode(&[]).is_err());
}

#[test]
fn booleans_and_their_volumes() {
    let a = box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0)).unwrap();
    let b = box_solid(Vec3::new(5.0, 5.0, 5.0), Vec3::new(15.0, 15.0, 15.0)).unwrap();
    let v = |b: Option<Body>| b.unwrap().mass().unwrap().volume;
    assert!(close(v(boolean(&a, &b, BoolOp::Union).unwrap()), 2000.0 - 125.0, 1e-9));
    assert!(close(v(boolean(&a, &b, BoolOp::Subtract).unwrap()), 1000.0 - 125.0, 1e-9));
    assert!(close(v(boolean(&a, &b, BoolOp::Intersect).unwrap()), 125.0, 1e-9));
    let far = box_solid(Vec3::new(50.0, 0.0, 0.0), Vec3::new(51.0, 1.0, 1.0)).unwrap();
    assert!(boolean(&a, &far, BoolOp::Intersect).unwrap().is_none());
    assert!(a.meets(&b).unwrap());
    assert!(!a.meets(&far).unwrap());
}

#[test]
fn a_hole_through_a_plate() {
    let plate = box_solid(Vec3::ZERO, Vec3::new(20.0, 20.0, 5.0)).unwrap();
    let hole = cylinder(Vec3::new(10.0, 10.0, -1.0), Vec3::Z, 3.0, 7.0).unwrap();
    let r = boolean(&plate, &hole, BoolOp::Subtract).unwrap().unwrap();
    let want = 2000.0 - std::f64::consts::PI * 9.0 * 5.0;
    assert!(close(r.mass().unwrap().volume, want, 1e-4), "{}", r.mass().unwrap().volume);
}

#[test]
fn native_round_trip_is_exact() {
    let a = box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0)).unwrap();
    let h = cylinder(Vec3::new(5.0, 5.0, -1.0), Vec3::Z, 2.0, 12.0).unwrap();
    let r = boolean(&a, &h, BoolOp::Subtract).unwrap().unwrap();
    let bytes = r.encode().unwrap();
    let back = Body::decode(&bytes).unwrap();
    assert_eq!(back.encode().unwrap(), bytes, "decode then encode gives the same bytes");
    let (m1, m2) = (back.mass().unwrap(), r.mass().unwrap());
    assert!(close(m1.volume, m2.volume, 1e-9) && close(m1.area, m2.area, 1e-9), "{m1:?} {m2:?}");
}

#[test]
fn slice_fillet_chamfer() {
    let a = box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0)).unwrap();
    let (pos, neg) = slice(&a, Vec3::new(0.0, 0.0, 4.0), Vec3::Z).unwrap();
    assert!(close(pos.unwrap().mass().unwrap().volume, 600.0, 1e-9));
    assert!(close(neg.unwrap().mass().unwrap().volume, 400.0, 1e-9));
    let f = fillet(&a, &[Vec3::new(5.0, 0.0, 10.0)], 1.0).unwrap();
    let cut = 1.0 - std::f64::consts::PI / 4.0;
    assert!(close(f.mass().unwrap().volume, 1000.0 - cut * 10.0, 1e-4), "{}", f.mass().unwrap().volume);
    let c = chamfer(&a, &[Vec3::new(5.0, 0.0, 10.0)], 1.0).unwrap();
    assert!(close(c.mass().unwrap().volume, 1000.0 - 5.0, 1e-9));
    let closed = shell(&a, &[], 1.0).unwrap();
    assert!(close(closed.mass().unwrap().volume, 1000.0 - 512.0, 1e-9), "walls inside: {}", closed.mass().unwrap().volume);
    let s = shell(&a, &[Vec3::new(5.0, 5.0, 10.0)], 1.0).unwrap();
    assert!(close(s.mass().unwrap().volume, 1000.0 - 8.0 * 8.0 * 9.0, 1e-9), "{}", s.mass().unwrap().volume);
}

#[test]
fn revolve_a_rectangle_into_a_ring() {
    // A 1 x 2 rectangle from x = 4 to 5, turned about the plane's y axis: a tube.
    let p = Profile {
        outer: vec![
            Seg::Line { a: Vec2::new(4.0, 0.0), b: Vec2::new(5.0, 0.0) },
            Seg::Line { a: Vec2::new(5.0, 0.0), b: Vec2::new(5.0, 2.0) },
            Seg::Line { a: Vec2::new(5.0, 2.0), b: Vec2::new(4.0, 2.0) },
            Seg::Line { a: Vec2::new(4.0, 2.0), b: Vec2::new(4.0, 0.0) },
        ],
        holes: Vec::new(),
    };
    let frame = Frame { origin: Vec3::ZERO, x: Vec3::new(1.0, 0.0, 0.0), y: Vec3::Z };
    let b = revolve(&frame, &p, Vec2::new(0.0, 0.0), Vec2::new(0.0, 1.0), std::f64::consts::TAU).unwrap().pop().unwrap();
    let want = std::f64::consts::PI * (25.0 - 16.0) * 2.0;
    assert!(close(b.mass().unwrap().volume, want, 1e-3), "{}", b.mass().unwrap().volume);
}

#[test]
fn transforms_move_and_mirror() {
    let a = box_solid(Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0)).unwrap();
    let mut m = Mat4::IDENTITY;
    m.m[0][3] = 100.0;
    let moved = a.transformed(&m).unwrap().mass().unwrap();
    assert!(close(moved.min.x, 100.0, 1e-12) && close(moved.volume, 6.0, 1e-9));
    let mut mirror = Mat4::IDENTITY;
    mirror.m[0][0] = -1.0;
    let mm = a.transformed(&mirror).unwrap().mass().unwrap();
    assert!(close(mm.volume, 6.0, 1e-9) && close(mm.min.x, -1.0, 1e-12), "{mm:?}");
}

// ---- section 5's spike checks ----

/// Units and scale: the same part in inches (small numbers), metres-as-millimetres (large) and at
/// survey coordinates (far from the origin) gives the same answers, relative to its size.
#[test]
fn units_and_scale() {
    let part = |s: f64, at: Vec3| -> f64 {
        let a = box_solid(at, at + Vec3::new(10.0, 10.0, 10.0) * s).unwrap();
        let h = cylinder(at + Vec3::new(5.0, 5.0, -1.0) * s, Vec3::Z, 2.0 * s, 12.0 * s).unwrap();
        boolean(&a, &h, BoolOp::Subtract).unwrap().unwrap().mass().unwrap().volume / s.powi(3)
    };
    let want = 1000.0 - std::f64::consts::PI * 4.0 * 10.0;
    // Measured volumes come from a tessellation whose finest chord has a floor in absolute
    // units, so a very small part measures less closely (4e-4 at 1/100 scale): docs/solids.md.
    for (s, rel) in [(0.01, 1e-3), (1.0, 1e-4), (1000.0, 1e-4)] {
        let v = part(s, Vec3::ZERO);
        assert!(close(v, want, rel), "scale {s}: {v}");
    }
    // Survey coordinates: 5e5 from the origin, a 10-unit part.
    let v = part(1.0, Vec3::new(500_000.0, 4_000_000.0, 0.0) * 0.1);
    assert!(close(v, want, 1e-3), "far from the origin: {v}");
}

/// Determinism: the same operations give the same bytes on every run (undo and the tests compare
/// drawings).
#[test]
fn determinism() {
    let run = || {
        let a = box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0)).unwrap();
        let b = sphere(Vec3::new(10.0, 10.0, 10.0), 6.0).unwrap();
        let c = cylinder(Vec3::new(2.0, 2.0, -1.0), Vec3::Z, 1.5, 12.0).unwrap();
        let r = boolean(&a, &b, BoolOp::Subtract).unwrap().unwrap();
        boolean(&r, &c, BoolOp::Subtract).unwrap().unwrap().encode().unwrap()
    };
    let first = run();
    for _ in 0..3 {
        assert_eq!(run(), first);
    }
}

// ---- drawings ----

#[test]
fn top_view_of_a_box_with_a_hole() {
    let a = box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 5.0)).unwrap();
    let h = cylinder(Vec3::new(5.0, 5.0, -1.0), Vec3::Z, 2.0, 7.0).unwrap();
    let r = boolean(&a, &h, BoolOp::Subtract).unwrap().unwrap();
    let v = project(&[&r], Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0)).unwrap();
    assert_eq!(v.unsupported_faces, 0);
    let visible: Vec<&Seg> = v.pieces.iter().filter(|p| p.seen == Seen::Visible).flat_map(|p| &p.segs).collect();
    let len = |s: &&Seg| match s {
        Seg::Line { a, b } => a.dist(*b),
        Seg::Arc { radius, sweep, .. } => radius * sweep.abs(),
    };
    // The square outline's edges are exact lines; the hole's rim is a circle (exact when the
    // kernel keeps the intersection as a circle, chords otherwise).
    let exact: f64 = v.pieces.iter().filter(|p| p.seen == Seen::Visible && p.exact).flat_map(|p| &p.segs).map(|s| len(&s)).sum();
    assert!(exact >= 40.0 - 1e-9, "the square outline is exact: {exact}");
    let total: f64 = visible.iter().map(len).sum();
    assert!(close(total - 40.0, 2.0 * std::f64::consts::PI * 2.0, 1e-3), "the hole's circle is visible once: {}", total - 40.0);
    // Seen from the top, the bottom of the hole is hidden behind its top.
    assert!(v.pieces.iter().all(|p| p.seen != Seen::Uncertain));
}

#[test]
fn front_view_has_hidden_lines() {
    let a = box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 5.0)).unwrap();
    let h = cylinder(Vec3::new(5.0, 5.0, -1.0), Vec3::Z, 2.0, 7.0).unwrap();
    let r = boolean(&a, &h, BoolOp::Subtract).unwrap().unwrap();
    let v = project(&[&r], Vec3::new(0.0, 1.0, 0.0), Vec3::Z).unwrap();
    let hidden: f64 = v
        .pieces
        .iter()
        .filter(|p| p.seen == Seen::Hidden)
        .flat_map(|p| &p.segs)
        .filter_map(|s| if let Seg::Line { a, b } = s { Some(a.dist(*b)) } else { None })
        .sum();
    assert!(close(hidden, 10.0, 1e-6), "the hole's two sides, 5 tall each, are hidden: {hidden}");
}

#[test]
fn section_of_a_plate_with_a_hole() {
    let a = box_solid(Vec3::ZERO, Vec3::new(20.0, 10.0, 4.0)).unwrap();
    let h = cylinder(Vec3::new(10.0, 5.0, -1.0), Vec3::Z, 2.0, 6.0).unwrap();
    let r = boolean(&a, &h, BoolOp::Subtract).unwrap().unwrap();
    let loops = section(&r, &Frame::xy(2.0)).unwrap();
    assert_eq!(loops.len(), 2, "the outline and the hole");
    let len = |l: &Vec<Seg>| -> f64 {
        l.iter()
            .map(|s| match s {
                Seg::Line { a, b } => a.dist(*b),
                Seg::Arc { radius, sweep, .. } => radius * sweep.abs(),
            })
            .sum()
    };
    let mut lens: Vec<f64> = loops.iter().map(len).collect();
    lens.sort_by(f64::total_cmp);
    assert!(close(lens[0], 4.0 * std::f64::consts::PI, 1e-6), "{lens:?}");
    assert!(close(lens[1], 60.0, 1e-9), "{lens:?}");
    // A plane that misses the part: nothing.
    assert!(section(&r, &Frame::xy(50.0)).unwrap().is_empty());
}

#[test]
fn cylinder_seams_are_not_drawn() {
    let c = cylinder(Vec3::ZERO, Vec3::Z, 2.0, 5.0).unwrap();
    // Two circles; the side's seam line is left out.
    assert_eq!(c.display(0.0).unwrap().wires.len(), 2);
    let v = project(&[&c], Vec3::new(0.0, 1.0, 0.0), Vec3::Z).unwrap();
    let sil = v.pieces.iter().filter(|p| p.silhouette && p.seen == Seen::Visible).count();
    assert_eq!(sil, 2, "a cylinder seen from the front shows its two outline lines: {:?}", v.pieces);
}

#[test]
fn tilted_cylinder_and_inertia() {
    let c = cylinder(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 0.0, 0.0), 1.0, 4.0).unwrap();
    let m = c.mass().unwrap();
    assert!(close(m.min.x, 1.0, 1e-6) && close(m.max.x, 5.0, 1e-6), "{m:?}");
    assert!(close(m.centroid.x, 3.0, 1e-6) && close(m.centroid.y, 2.0, 1e-6));
    // About its own axis (X here): m r^2 / 2 with m = pi r^2 h.
    let ix = std::f64::consts::PI * 4.0 / 2.0;
    assert!(close(m.moments.x, ix, 1e-4), "{m:?}");
}
