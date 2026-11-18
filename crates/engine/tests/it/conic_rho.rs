//! A conic arc's rho in Properties: `properties` reports it for a spline whose shape is a conic,
//! and `properties.set` with `rho` reshapes it (undoably), refusing a rho outside (0, 1).

use draftcraft_engine::Session;
use draftcraft_engine::doc::EntityKind;
use serde_json::{Value, json};

fn conic(s: &mut Session, rho: f64) -> String {
    let r = s.execute("conic", &json!({ "start": [0, 0], "apex": [5, 5], "end": [10, 0], "rho": rho })).unwrap();
    r["handle"].as_str().unwrap().to_string()
}

fn rho_of(s: &mut Session, h: &str) -> Option<f64> {
    let r = s.execute("properties", &json!({ "handles": [h] })).unwrap();
    r["objects"][0].get("rho").and_then(Value::as_f64)
}

fn weights(s: &Session) -> Vec<Vec<f64>> {
    s.doc().unwrap().model.iter().filter_map(|e| if let EntityKind::Spline(sp) = &e.kind { Some(sp.weights.clone()) } else { None }).collect()
}

#[test]
fn properties_show_a_conics_rho_and_nothing_for_other_splines() {
    let mut s = Session::new();
    let h = conic(&mut s, 0.3);
    assert!((rho_of(&mut s, &h).unwrap() - 0.3).abs() < 1e-12);
    let cubic = s.execute("spline", &json!({ "fit": [[0, 0], [3, 4], [6, 0], [9, 4]] })).unwrap();
    assert_eq!(rho_of(&mut s, cubic["handle"].as_str().unwrap()), None);
}

#[test]
fn setting_rho_reshapes_the_conic_and_undo_restores_it() {
    let mut s = Session::new();
    let h = conic(&mut s, 0.5);
    s.execute("properties.set", &json!({ "handles": [h], "rho": 0.8 })).unwrap();
    assert!((rho_of(&mut s, &h).unwrap() - 0.8).abs() < 1e-12);
    assert!((weights(&s)[0][1] - 4.0).abs() < 1e-9, "w = rho / (1 - rho)");
    s.execute("u", &json!({})).unwrap();
    assert!((rho_of(&mut s, &h).unwrap() - 0.5).abs() < 1e-12);
    assert_eq!(weights(&s), vec![vec![1.0, 1.0, 1.0]]);
}

#[test]
fn several_conics_take_one_rho_and_other_objects_are_left_alone() {
    let mut s = Session::new();
    let a = conic(&mut s, 0.2);
    let b = conic(&mut s, 0.7);
    s.execute("line", &json!({ "points": [[0, 0], [10, 0]] })).unwrap();
    let l = s.doc().unwrap().model.iter().last().unwrap().handle.hex();
    s.execute("properties.set", &json!({ "handles": [a, b, l], "rho": 0.6 })).unwrap();
    assert!((rho_of(&mut s, &a).unwrap() - 0.6).abs() < 1e-12);
    assert!((rho_of(&mut s, &b).unwrap() - 0.6).abs() < 1e-12);
    let d = s.doc().unwrap();
    assert!(d.model.iter().any(|e| matches!(&e.kind, EntityKind::Line(_))));
}

#[test]
fn a_bad_rho_is_refused_and_changes_nothing() {
    let mut s = Session::new();
    let h = conic(&mut s, 0.4);
    let before = weights(&s);
    for bad in [json!(0.0), json!(1.0), json!(-0.3), json!(1.5), json!("abc"), json!("0.3"), json!(f64::MAX), json!([0.3]), json!(true)] {
        let r = s.execute("properties.set", &json!({ "handles": [h], "rho": bad, "color": "red" }));
        assert!(r.is_err(), "{bad} accepted");
        assert_eq!(weights(&s), before, "{bad}");
    }
    // A rho for a selection with no conic arc in it is refused too.
    let cubic = s.execute("spline", &json!({ "fit": [[0, 0], [3, 4], [6, 0], [9, 4]] })).unwrap();
    assert!(s.execute("properties.set", &json!({ "handles": [cubic["handle"]], "rho": 0.5 })).is_err());
    assert!((rho_of(&mut s, &h).unwrap() - 0.4).abs() < 1e-12);
    let colors: Vec<_> = s.doc().unwrap().model.iter().map(|e| e.common.color).collect();
    assert!(colors.iter().all(|c| c.name() == "ByLayer"), "{colors:?}");
}
