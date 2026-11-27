//! `properties.set`, which the Properties palette, Quick Properties and CHPROP edit through, leaves
//! objects on locked layers alone as CHPROP, MATCHPROP and MOVE do, and says how many it left out.

use draftcraft_engine::Session;
use draftcraft_engine::doc::Handle;
use draftcraft_engine::doc::color::Color;
use serde_json::{Value, json};

fn handle(v: &Value) -> Handle {
    Handle::parse_hex(v.get("handle").unwrap_or(&v["handles"][0]).as_str().unwrap()).unwrap()
}

fn line(s: &mut Session, y: f64) -> Handle {
    handle(&s.execute("line", &json!({ "points": [[0, y], [50, y]] })).unwrap())
}

/// Line a on layer 0 and lines b and c on the locked layer L1; their handles.
fn locked_drawing() -> (Session, [Handle; 3]) {
    let mut s = Session::new();
    let hs = [line(&mut s, 0.0), line(&mut s, 20.0), line(&mut s, 40.0)];
    s.execute("properties.set", &json!({ "handles": [hs[1].hex(), hs[2].hex()], "layer": "L1" })).unwrap();
    s.execute("layer.set", &json!({ "name": "L1", "locked": true })).unwrap();
    (s, hs)
}

fn counts(r: &Value) -> (Option<u64>, Option<u64>) {
    (r["changed"].as_u64(), r["locked"].as_u64())
}

fn colors(s: &Session) -> Vec<Color> {
    s.doc().unwrap().model.iter().map(|e| e.common.color).collect()
}

fn layers(s: &Session) -> Vec<String> {
    s.doc().unwrap().model.iter().map(|e| e.common.layer.clone()).collect()
}

#[test]
fn objects_on_locked_layers_are_left_out() {
    let (mut s, [a, b, c]) = locked_drawing();
    let r = s.execute("properties.set", &json!({ "handles": [a.hex()], "color": 2 })).unwrap();
    assert_eq!(counts(&r), (Some(1), Some(0)), "{r}");
    assert!(r.get("message").is_none(), "{r}");
    let r = s.execute("properties.set", &json!({ "handles": [a.hex(), b.hex()], "color": 3 })).unwrap();
    assert_eq!(counts(&r), (Some(1), Some(1)), "{r}");
    assert_eq!(r["message"], "1 object was on a locked layer.");
    assert_eq!(colors(&s), [Color::Index(3), Color::ByLayer, Color::ByLayer]);
    // The selection when no handles are given; a new layer leaves them on theirs.
    s.set_selection(vec![a, b, c]);
    let r = s.execute("properties.set", &json!({ "layer": "L2" })).unwrap();
    assert_eq!(counts(&r), (Some(1), Some(2)), "{r}");
    assert_eq!(r["message"], "2 objects were on a locked layer.");
    assert_eq!(layers(&s), ["L2", "L1", "L1"]);
}

#[test]
fn with_every_object_locked_nothing_changes() {
    let (mut s, [_, b, c]) = locked_drawing();
    let undo = s.state().unwrap().undo.len();
    let r = s.execute("properties.set", &json!({ "handles": [b.hex(), c.hex()], "layer": "NEW", "color": 4 })).unwrap();
    assert_eq!(counts(&r), (Some(0), Some(2)), "{r}");
    assert_eq!(r["message"], "2 objects were on a locked layer. Nothing was changed.");
    assert_eq!(colors(&s), [Color::ByLayer; 3]);
    assert!(s.doc().unwrap().layer("NEW").is_none(), "no layer is made");
    assert_eq!(s.state().unwrap().undo.len(), undo, "no undo step");
    // A bad value is still refused.
    assert!(s.execute("properties.set", &json!({ "handles": [b.hex()], "color": "nonsense" })).is_err());
    // CHPROP's JSON form is properties.set.
    let r = s.execute("chprop", &json!({ "handles": [b.hex()], "color": 4 })).unwrap();
    assert_eq!(counts(&r), (Some(0), Some(1)), "{r}");
    assert_eq!(r["message"], "1 object was on a locked layer. Nothing was changed.");
}
