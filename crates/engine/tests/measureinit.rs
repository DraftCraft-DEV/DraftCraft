//! MEASUREINIT: the units of a new drawing made without a template (issue #1182). It is a profile
//! setting, so a metric choice survives restarts; an explicit `metric` or template still wins.

use draftcraft_engine::Session;
use serde_json::json;

fn var(s: &mut Session, name: &str) -> serde_json::Value {
    s.execute("getvar", &json!({ "name": name })).unwrap()["value"].clone()
}

#[test]
fn imperial_by_default() {
    let mut s = Session::empty();
    assert_eq!(var(&mut s, "MEASUREINIT"), json!(0));
    s.execute("new", &json!({})).unwrap();
    assert_eq!(var(&mut s, "INSUNITS"), json!(1));
    assert_eq!(var(&mut s, "MEASUREMENT"), json!(0));
}

#[test]
fn new_follows_measureinit() {
    let mut s = Session::empty();
    s.execute("options", &json!({ "MEASUREINIT": 1 })).unwrap();
    s.execute("new", &json!({})).unwrap();
    assert_eq!(var(&mut s, "INSUNITS"), json!(4));
    assert_eq!(var(&mut s, "MEASUREMENT"), json!(1));
    assert_eq!(var(&mut s, "DIMSTYLE"), json!("ISO-25"));
    // An explicit choice still wins.
    s.execute("new", &json!({ "metric": false })).unwrap();
    assert_eq!(var(&mut s, "INSUNITS"), json!(1));
}

#[test]
fn only_zero_or_one() {
    let mut s = Session::empty();
    for bad in [json!(2), json!(-1), json!("metric")] {
        assert!(s.execute("options", &json!({ "MEASUREINIT": bad })).is_err(), "{bad}");
    }
    assert_eq!(var(&mut s, "MEASUREINIT"), json!(0));
}

#[test]
fn kept_in_the_profile() {
    let mut s = Session::empty();
    s.execute("options", &json!({ "MEASUREINIT": 1 })).unwrap();
    let saved = draftcraft_engine::cmd::options::profile(&s);
    assert_eq!(saved["MEASUREINIT"], json!(1));
    let mut restarted = Session::empty();
    draftcraft_engine::cmd::options::restore(&mut restarted, &saved);
    assert_eq!(var(&mut restarted, "MEASUREINIT"), json!(1));
}

/// #34: a new drawing in centimetres, chosen by NEW's `units` or remembered by MEASUREINITUNITS.
#[test]
fn centimetres_by_units_or_measureinitunits() {
    let mut s = Session::empty();
    s.execute("new", &json!({ "units": "cm" })).unwrap();
    assert_eq!(var(&mut s, "INSUNITS"), json!(5));
    assert_eq!(var(&mut s, "MEASUREMENT"), json!(1));
    assert_eq!(var(&mut s, "DIMSTYLE"), json!("ISO-25"));
    // The millimetre drawing's printed sizes, a tenth as large in centimetres.
    let d = s.doc().unwrap();
    assert_eq!(d.header.f64("TEXTSIZE", 0.0), 0.25);
    assert_eq!(d.header.f64("LTSCALE", 0.0), 0.1);
    assert_eq!(d.dim_style("ISO-25").unwrap().effective_scale(1.0), 0.1);
    assert!((s.docs[s.active].view().height - 29.7).abs() < 1e-9);
    // Inches and millimetres by name or INSUNITS code; `units` wins over `metric`.
    for (units, code) in [(json!("in"), 1), (json!("Millimeters"), 4), (json!(5), 5), (json!(" CM "), 5)] {
        s.execute("new", &json!({ "units": units, "metric": code == 1 })).unwrap();
        assert_eq!(var(&mut s, "INSUNITS"), json!(code), "{units}");
    }
    // Remembered for a plain New, and for `metric`, once metric drawings are in centimetres.
    assert_eq!(var(&mut s, "MEASUREINITUNITS"), json!(4));
    s.execute("options", &json!({ "MEASUREINIT": 1, "MEASUREINITUNITS": 5 })).unwrap();
    s.execute("new", &json!({})).unwrap();
    assert_eq!(var(&mut s, "INSUNITS"), json!(5));
    s.execute("new", &json!({ "metric": true })).unwrap();
    assert_eq!(var(&mut s, "INSUNITS"), json!(5));
    s.execute("new", &json!({ "metric": false })).unwrap();
    assert_eq!(var(&mut s, "INSUNITS"), json!(1));
    let saved = draftcraft_engine::cmd::options::profile(&s);
    assert_eq!(saved["MEASUREINITUNITS"], json!(5));
    let mut restarted = Session::empty();
    draftcraft_engine::cmd::options::restore(&mut restarted, &saved);
    restarted.execute("new", &json!({})).unwrap();
    assert_eq!(var(&mut restarted, "INSUNITS"), json!(5));
}

#[test]
fn units_that_are_not_inches_millimetres_or_centimetres_are_refused() {
    let mut s = Session::empty();
    for bad in [json!("m"), json!(2), json!(-1), json!(""), json!(true), json!(i64::MAX), json!(1.5)] {
        let before = s.docs.len();
        assert!(s.execute("new", &json!({ "units": bad })).is_err(), "{bad}");
        assert_eq!(s.docs.len(), before, "no drawing for {bad}");
    }
    for bad in [json!(1), json!(6), json!(0), json!("cm"), json!(-4), json!(f64::NAN.to_string())] {
        assert!(s.execute("options", &json!({ "MEASUREINITUNITS": bad })).is_err(), "{bad}");
    }
    assert_eq!(var(&mut s, "MEASUREINITUNITS"), json!(4));
}
