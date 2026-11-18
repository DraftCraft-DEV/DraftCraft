//! AREATOTAL: the total area, perimeter and length of a selection; and AREA's JSON form totalling
//! several objects with some subtracted (Add area / Subtract area).

use std::f64::consts::PI;

use draftcraft_engine::Session;
use draftcraft_engine::doc::geom::{PolyVertex, Spline, Vec2, Vec3};
use draftcraft_engine::doc::{Common, EntityKind, Line, LwPolyline, Space};
use serde_json::{Value, json};

fn handle(v: &Value) -> String {
    v["handle"].as_str().or_else(|| v["handles"][0].as_str()).expect("a handle").to_string()
}

fn run(s: &mut Session, cmd: &str, p: Value) -> String {
    handle(&s.execute(cmd, &p).unwrap())
}

/// Add an entity straight to the document, for shapes the commands refuse to draw.
fn add(s: &mut Session, kind: EntityKind) -> String {
    s.doc_mut().unwrap().add(&Space::Model, Common::default(), kind).unwrap().hex()
}

fn total(s: &mut Session, hs: &[&str]) -> Value {
    s.execute("areatotal", &json!({ "handles": hs })).unwrap()
}

fn near(v: &Value, want: f64) {
    let got = v.as_f64().unwrap_or(f64::NAN);
    assert!((got - want).abs() < 1e-6 * (1.0 + want.abs()), "{v} != {want}");
}

fn has(log: &[String], text: &str) -> bool {
    log.iter().any(|l| l.contains(text))
}

#[test]
fn a_rectangle_and_a_circle_add_their_areas_and_perimeters() {
    let mut s = Session::new();
    let rect = run(&mut s, "rectang", json!({ "p1": [0, 0], "p2": [10, 5] }));
    let circle = run(&mut s, "circle", json!({ "center": [20, 0], "radius": 2 }));

    let r = total(&mut s, &[&rect]);
    near(&r["area"], 50.0);
    near(&r["perimeter"], 30.0);
    assert_eq!((r["count"].clone(), r["closed"].clone(), r["open"].clone()), (json!(1), json!(1), json!(0)), "{r}");

    let r = total(&mut s, &[&rect, &circle]);
    near(&r["area"], 50.0 + 4.0 * PI);
    near(&r["perimeter"], 30.0 + 4.0 * PI);
    near(&r["length"], 0.0);
    assert_eq!(r["measured"], 2, "{r}");
    let msg = r["message"].as_str().unwrap();
    assert!(msg.contains("Total area = 62.5664, Total perimeter = 42.5664"), "{msg}");
    // The AREA and PERIMETER system variables hold the totals, as after AREA.
    assert!((s.area - (50.0 + 4.0 * PI)).abs() < 1e-9);
    assert!((s.perimeter - (30.0 + 4.0 * PI)).abs() < 1e-9);
}

#[test]
fn a_polyline_with_an_arc_counts_the_arc() {
    let mut s = Session::new();
    // A 10 x 10 square whose right side is a half circle bulging out (bulge 1, radius 5).
    let v = |x: f64, y: f64, bulge: f64| PolyVertex { p: Vec2::new(x, y), bulge, start_width: 0.0, end_width: 0.0 };
    let pl = LwPolyline {
        vertices: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 1.0), v(10.0, 10.0, 0.0), v(0.0, 10.0, 0.0)],
        closed: true,
        const_width: 0.0,
        elevation: 0.0,
        plinegen: false,
    };
    let h = add(&mut s, EntityKind::LwPolyline(pl.clone()));
    let r = total(&mut s, &[&h]);
    near(&r["area"], 100.0 + 12.5 * PI);
    near(&r["perimeter"], 30.0 + 5.0 * PI);

    // Drawn back to its first point without the closed flag, it still bounds the same area.
    let mut open_but_closed = pl;
    open_but_closed.closed = false;
    open_but_closed.vertices.push(v(0.0, 0.0, 0.0));
    let h = add(&mut s, EntityKind::LwPolyline(open_but_closed));
    let r = total(&mut s, &[&h]);
    assert_eq!(r["closed"], 1, "{r}");
    near(&r["area"], 100.0 + 12.5 * PI);
}

#[test]
fn open_lines_and_arcs_add_only_their_length() {
    let mut s = Session::new();
    let a = run(&mut s, "line", json!({ "points": [[0, 0], [3, 4]] }));
    let b = run(&mut s, "line", json!({ "points": [[0, 0], [10, 0]] }));
    let arc = run(&mut s, "arc", json!({ "center": [0, 0], "radius": 2, "start": 0, "end": 90 }));
    let pline = run(&mut s, "pline", json!({ "points": [[0, 0], [5, 0], [5, 5]] }));
    let r = total(&mut s, &[&a, &b, &arc, &pline]);
    near(&r["area"], 0.0);
    near(&r["perimeter"], 0.0);
    near(&r["length"], 15.0 + PI + 10.0);
    assert_eq!((r["closed"].clone(), r["open"].clone()), (json!(0), json!(4)), "{r}");
    let msg = r["message"].as_str().unwrap();
    assert!(msg.contains("Total length of open objects = 28.1416"), "{msg}");
    assert!(!msg.contains("Total area"), "no closed object, no area line: {msg}");

    // With a closed object too: the perimeter and the length are also given together.
    let rect = run(&mut s, "rectang", json!({ "p1": [0, 0], "p2": [1, 1] }));
    let r = total(&mut s, &[&a, &rect]);
    near(&r["totalLength"], 9.0);
    assert!(r["message"].as_str().unwrap().contains("Perimeter and length together = 9.0000"), "{r}");
}

#[test]
fn objects_without_area_or_length_are_skipped_and_counted() {
    let mut s = Session::new();
    let line = run(&mut s, "line", json!({ "points": [[0, 0], [1, 0]] }));
    let text = run(&mut s, "text", json!({ "at": [0, 0], "text": "hello", "height": 1 }));
    let point = run(&mut s, "point", json!({ "at": [5, 5] }));
    let r = total(&mut s, &[&line, &text, &point, "FFFFFF"]);
    assert_eq!(r["count"], 4, "{r}");
    assert_eq!(r["measured"], 1, "{r}");
    assert_eq!(r["skipped"], json!({ "Point": 1, "Text": 1, "not found": 1 }), "{r}");
    assert!(r["message"].as_str().unwrap().contains("Skipped, no area or length:"), "{r}");
}

#[test]
fn area_adds_several_objects_and_subtracts_others() {
    let mut s = Session::new();
    let square = run(&mut s, "rectang", json!({ "p1": [0, 0], "p2": [10, 10] }));
    let other = run(&mut s, "rectang", json!({ "p1": [20, 0], "p2": [22, 2] }));
    let hole = run(&mut s, "circle", json!({ "center": [5, 5], "radius": 1 }));
    let line = run(&mut s, "line", json!({ "points": [[0, 0], [1, 0]] }));

    let r = s.execute("area", &json!({ "handles": [square, other, line], "subtract": [hole] })).unwrap();
    near(&r["area"], 104.0 - PI);
    near(&r["perimeter"], 48.0);
    assert_eq!(r["count"], 3, "{r}");
    assert_eq!(r["skipped"], json!([line]), "the line has no area: {r}");
    assert!(r["message"].as_str().unwrap().starts_with("Total area = 100.8584"), "{r}");

    // One handle is still AREA's Object option.
    let r = s.execute("area", &json!({ "handle": square })).unwrap();
    near(&r["area"], 100.0);
    assert!(r.get("count").is_none(), "{r}");

    // Nothing with an area is an error, as for a single object.
    let err = s.execute("area", &json!({ "handles": [line] })).unwrap_err();
    assert!(err.to_string().contains("none of the objects has an area"), "{err}");
}

#[test]
fn an_empty_selection_is_refused_and_ends_the_typed_command() {
    let mut s = Session::new();
    let err = s.execute("areatotal", &json!({})).unwrap_err();
    assert!(err.to_string().contains("nothing is selected"), "{err}");
    let err = s.execute("areatotal", &json!({ "handles": [] })).unwrap_err();
    assert!(err.to_string().contains("nothing is selected"), "{err}");

    s.cmdline("areatotal").unwrap();
    assert!(s.running.is_some());
    s.cmdline("").unwrap();
    assert!(s.running.is_none(), "{}", s.prompt_text());
}

#[test]
fn typed_it_reports_the_totals_and_can_place_them_as_text() {
    let mut s = Session::new();
    for l in ["rectang 0,0 10,5", "circle 20,0 2", "line 30,0 33,4", ""] {
        s.cmdline(l).unwrap();
    }
    let before = s.doc().unwrap().entity_count();
    for l in ["areatotal", "all", ""] {
        s.cmdline(l).unwrap();
    }
    assert!(has(&s.log, "3 object(s) measured: 2 closed, 1 open"), "{:?}", s.log);
    assert!(has(&s.log, "Total area = 62.5664, Total perimeter = 42.5664"), "{:?}", s.log);
    assert!(has(&s.log, "Total length of open objects = 5.0000"), "{:?}", s.log);
    assert!(s.prompt_text().contains("place the totals as text"), "{}", s.prompt_text());

    // Words that are not a point or eXit ask again.
    s.cmdline("all").unwrap();
    assert!(has(&s.log, "Point or option keyword required."), "{:?}", s.log);
    assert!(s.prompt_text().contains("place the totals as text"), "{}", s.prompt_text());

    // Enter (eXit) ends it without adding anything.
    s.cmdline("").unwrap();
    assert!(s.running.is_none(), "{}", s.prompt_text());
    assert_eq!(s.doc().unwrap().entity_count(), before);

    // A point places the totals as one line of text.
    s.cmdline("areatotal").unwrap();
    // The objects picked last may still be selected (then they are measured at once).
    if s.prompt_text().contains("Select objects") {
        s.cmdline("all").unwrap();
        s.cmdline("").unwrap();
    }
    s.cmdline("0,20").unwrap();
    assert!(s.running.is_none(), "{}", s.prompt_text());
    let texts = s.execute("texts", &json!({ "contains": "Area =" })).unwrap();
    assert_eq!(texts["count"], 1, "{texts}");
    assert_eq!(texts["texts"][0]["text"], "Area = 62.5664, Perimeter = 42.5664, Length = 5.0000", "{texts}");
    // Placing the text is one undo step.
    s.execute("u", &json!({})).unwrap();
    assert_eq!(s.execute("texts", &json!({ "contains": "Area =" })).unwrap()["count"], 0);
}

#[test]
fn the_json_form_places_text_where_asked() {
    let mut s = Session::new();
    let rect = run(&mut s, "rectang", json!({ "p1": [0, 0], "p2": [4, 3] }));
    let r = s.execute("areatotal", &json!({ "handles": [rect], "at": [0, 10] })).unwrap();
    let h = r["handle"].as_str().expect("a text handle").to_string();
    let texts = s.execute("texts", &json!({})).unwrap();
    assert_eq!(texts["texts"][0]["handle"], h, "{texts}");
    assert_eq!(texts["texts"][0]["text"], "Area = 12.0000, Perimeter = 14.0000", "{texts}");

    let err = s.execute("areatotal", &json!({ "handles": [rect], "at": "here" })).unwrap_err();
    assert!(err.to_string().contains("`at` must be a point"), "{err}");
    let t = run(&mut s, "text", json!({ "at": [0, 0], "text": "x", "height": 1 }));
    let err = s.execute("areatotal", &json!({ "handles": [t], "at": [0, 0] })).unwrap_err();
    assert!(err.to_string().contains("none of the objects"), "{err}");
}

#[test]
fn the_drawing_units_format_the_totals() {
    let mut s = Session::new();
    let rect = run(&mut s, "rectang", json!({ "p1": [0, 0], "p2": [10, 5] }));
    s.doc_mut().unwrap().header.set("LUPREC", draftcraft_engine::doc::HVal::Int(2));
    let r = total(&mut s, &[&rect]);
    assert!(r["message"].as_str().unwrap().contains("Total area = 50.00, Total perimeter = 30.00"), "{r}");
}

#[test]
fn hostile_and_degenerate_geometry_does_not_break_the_totals() {
    let mut s = Session::new();
    let big = 1e308;
    let huge = add(&mut s, EntityKind::Line(Line { a: Vec3::new(-big, 0.0, 0.0), b: Vec3::new(big, 0.0, 0.0) }));
    let nan = add(&mut s, EntityKind::Line(Line { a: Vec3::new(f64::NAN, 0.0, 0.0), b: Vec3::new(1.0, 0.0, 0.0) }));
    let zero = add(&mut s, EntityKind::Line(Line { a: Vec3::new(1.0, 1.0, 0.0), b: Vec3::new(1.0, 1.0, 0.0) }));
    let single = add(
        &mut s,
        EntityKind::LwPolyline(LwPolyline {
            vertices: vec![PolyVertex::new(Vec2::new(2.0, 2.0))],
            closed: true,
            const_width: 0.0,
            elevation: 0.0,
            plinegen: false,
        }),
    );
    let empty =
        add(&mut s, EntityKind::LwPolyline(LwPolyline { vertices: Vec::new(), closed: false, const_width: 0.0, elevation: 0.0, plinegen: false }));
    let bad_spline = add(
        &mut s,
        EntityKind::Spline(Spline {
            degree: 3,
            knots: vec![f64::NAN],
            control: vec![Vec2::new(0.0, 0.0)],
            weights: Vec::new(),
            fit: Vec::new(),
            closed: true,
            fit_opts: Default::default(),
        }),
    );

    let r = total(&mut s, &[&huge, &nan, &zero, &single, &empty, &bad_spline]);
    assert_eq!(r["count"], 6, "{r}");
    for k in ["area", "perimeter", "length", "totalLength"] {
        assert!(r[k].as_f64().is_some_and(f64::is_finite), "{k}: {r}");
    }
    assert_eq!(r["skipped"]["unmeasurable"], 2, "the line past f64's range and the NaN one: {r}");
    assert_eq!(r["skipped"]["Spline"], 1, "{r}");

    // Two lines that each fit add up past f64's range: an error, not infinity.
    let half = add(&mut s, EntityKind::Line(Line { a: Vec3::new(0.0, 0.0, 0.0), b: Vec3::new(1.5e308, 0.0, 0.0) }));
    let err = s.execute("areatotal", &json!({ "handles": [half, half] })).unwrap_err();
    assert!(err.to_string().contains("the total length is too large"), "{err}");
    // Typed, over everything (each object once): the ones past range are skipped, the rest totalled.
    for l in ["areatotal", "all", ""] {
        s.cmdline(l).unwrap();
    }
    assert!(has(&s.log, "Skipped, no area or length:"), "{:?}", s.log);
    s.cmdline("").unwrap();
    assert!(s.running.is_none(), "{}", s.prompt_text());
}
