//! The portable settings file: CONFIGEXPORT writes the profile, drawing, plot and alias settings
//! to one TOML file and CONFIGIMPORT reads them back, applying what it can and listing the rest.
//! User aliases are process-wide, so the tests that touch them run one at a time.

use std::sync::Mutex;

use draftcraft_engine::Session;
use draftcraft_engine::cmd::config;
use draftcraft_engine::cmd::resolve_alias;
use serde_json::{Value, json};

#[path = "support/temp_dir.rs"]
mod temp_dir;

use temp_dir::TempDir;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn snapshot(s: &Session) -> Value {
    Value::Object(config::snapshot(s))
}

fn issues(r: &Value) -> Vec<String> {
    r["issues"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

/// A session whose settings are all away from the defaults.
fn customised() -> Session {
    let mut s = Session::new();
    s.execute("options", &json!({ "PICKBOX": 7, "OSMODE": 15, "GRIDUNIT": [10.0, 10.0], "SNAPUNIT": [5.0, 5.0], "LWDISPLAY": 1, "MEASUREINIT": 1 }))
        .unwrap();
    for (name, v) in [
        ("LUNITS", json!(4)),
        ("LUPREC", json!(2)),
        ("AUNITS", json!(1)),
        ("INSUNITS", json!(4)),
        ("LTSCALE", json!(25.0)),
        ("LWDEFAULT", json!(35)),
        ("DIMSCALE", json!(50.0)),
        ("TEXTSIZE", json!(2.5)),
    ] {
        s.execute("setvar", &json!({ "name": name, "value": v })).unwrap();
    }
    s.execute("lweight", &json!({ "lineweight": 0.5 })).unwrap();
    s.execute("aliasedit", &json!({ "set": { "zzcfgx": "circle" } })).unwrap();
    s
}

#[test]
fn export_then_import_gives_the_same_settings() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = customised();
    let dir = TempDir::new("config");
    let file = dir.path_str("mine.toml");
    let r = a.execute("configexport", &json!({ "file": file })).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(r["text"].as_str(), Some(text.as_str()));
    assert!(text.contains("[profile]") && text.contains("PICKBOX = 7") && text.contains("[drawing]") && text.contains("LUNITS = 4"), "{text}");
    assert!(text.contains("[aliases]") && text.contains("ZZCFGX = \"circle\""), "{text}");

    // Forget the alias, then read the file into a session with every default.
    a.execute("aliasedit", &json!({ "remove": ["zzcfgx"] })).unwrap();
    assert_eq!(resolve_alias("zzcfgx"), "zzcfgx");
    let mut b = Session::new();
    assert_ne!(snapshot(&b)["profile"], snapshot(&a)["profile"]);
    let r = b.execute("configimport", &json!({ "file": file })).unwrap();
    assert_eq!(issues(&r), Vec::<String>::new());
    assert_eq!(resolve_alias("zzcfgx"), "circle");
    assert_eq!(snapshot(&b), snapshot(&a));
    assert_eq!(b.settings.pickbox, 7.0);
    // The drawing change is one undo step.
    b.execute("u", &json!({})).unwrap();
    assert_eq!(snapshot(&b)["drawing"]["LUNITS"], json!(2));
    a.execute("aliasedit", &json!({ "remove": ["zzcfgx"] })).unwrap();
}

#[test]
fn the_plot_settings_go_to_every_layout() {
    let mut s = Session::new();
    let r = s
        .execute("configimport", &json!({ "text": "[plot]\nplotStyleTable = \"monochrome.ctb\"\nlineweights = false\ndisplayPlotStyles = true\n" }))
        .unwrap();
    assert_eq!(issues(&r), Vec::<String>::new());
    let d = s.doc().unwrap();
    assert!(!d.layouts.is_empty());
    for l in &d.layouts {
        assert_eq!(l.page.plot_style_table, "monochrome.ctb");
        assert!(!l.page.lineweights && l.page.show_plot_styles);
    }
    assert_eq!(snapshot(&s)["plot"]["plotStyleTable"], json!("monochrome.ctb"));
}

#[test]
fn unknown_and_invalid_entries_are_reported_and_the_rest_applied() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Session::new();
    let before = snapshot(&s);
    let text = r#"
version = 7
[profile]
PICKBOX = 9
OSMODE = "lots"
NOSUCHVAR = 1
SNAPUNIT = [-1, 0]
[drawing]
LUNITS = 99
LTSCALE = nan
CELWEIGHT = 17
INSUNITS = 6
TEXTSIZE = -2
CANNOSCALE = "no such scale"
[plot]
plotStyleTable = "nowhere.ctb"
lineweights = "yes"
[aliases]
zzcfgok = "line"
"two words" = "line"
zzcfgbad = "nosuchcommand"
zzcfgnum = 5
LINE = "circle"
[macros]
a = 1
"#;
    let r = s.execute("configimport", &json!({ "text": text })).unwrap();
    let applied: Vec<&str> = r["applied"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert_eq!(applied, ["profile.PICKBOX", "drawing.INSUNITS", "aliases.ZZCFGOK"]);
    let issues = issues(&r);
    for needle in [
        "version",
        "profile.OSMODE",
        "profile.NOSUCHVAR",
        "profile.SNAPUNIT",
        "drawing.LUNITS",
        "drawing.LTSCALE",
        "drawing.CELWEIGHT",
        "drawing.TEXTSIZE",
        "drawing.CANNOSCALE",
        "plot.plotStyleTable",
        "plot.lineweights",
        "aliases.two words",
        "aliases.zzcfgbad",
        "aliases.zzcfgnum",
        "aliases.LINE",
        "[macros]",
    ] {
        assert!(issues.iter().any(|i| i.starts_with(needle)), "no issue for {needle}: {issues:#?}");
    }
    assert_eq!(issues.len(), 16, "{issues:#?}");
    let after = snapshot(&s);
    assert_eq!(after["profile"]["PICKBOX"], json!(9.0));
    assert_eq!(after["profile"]["OSMODE"], before["profile"]["OSMODE"]);
    assert_eq!(after["profile"]["SNAPUNIT"], before["profile"]["SNAPUNIT"]);
    assert_eq!(after["drawing"]["LUNITS"], before["drawing"]["LUNITS"]);
    assert_eq!(after["drawing"]["LTSCALE"], before["drawing"]["LTSCALE"]);
    assert_eq!(after["drawing"]["INSUNITS"], json!(6));
    assert_eq!(resolve_alias("zzcfgok"), "line");
    assert_eq!(resolve_alias("line"), "line");
    s.execute("aliasedit", &json!({ "remove": ["zzcfgok"] })).unwrap();
}

#[test]
fn a_hostile_file_is_refused_cleanly() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Session::new();
    let before = snapshot(&s);
    let dir = TempDir::new("config-hostile");
    let deep = format!("[profile]\nPICKBOX = {}1{}\n", "[".repeat(100_000), "]".repeat(100_000));
    let cases: Vec<(&str, Value)> = vec![
        ("not toml", json!({ "text": "PICKBOX: 5\n<xml/>" })),
        ("too deep", json!({ "text": deep })),
        ("too large", json!({ "text": format!("# {}\n", "x".repeat(config::MAX_CONFIG_BYTES)) })),
        ("binary", json!({ "file": dir.file("bin.toml", [0u8, 159, 146, 150, 255, 0, 1]) })),
        ("too large file", json!({ "file": dir.file("big.toml", vec![b'#'; config::MAX_CONFIG_BYTES + 1]) })),
        ("missing file", json!({ "file": dir.path_str("nope.toml") })),
        ("a folder", json!({ "file": dir.path_str("") })),
        ("nothing", json!({})),
        ("wrong types", json!({ "text": 5 })),
        ("settings not an object", json!({ "settings": [1, 2] })),
    ];
    for (what, p) in cases {
        assert!(s.execute("configimport", &p).is_err(), "{what} was accepted");
    }
    // Sections of the wrong shape, absurd numbers and many keys: skipped, never applied.
    let mut many = String::from("[profile]\n");
    for i in 0..25_000 {
        many.push_str(&format!("a{i}=1\n"));
    }
    assert!(many.len() < config::MAX_CONFIG_BYTES);
    for text in ["profile = 5\ndrawing = [1]\naliases = \"x\"", many.as_str()] {
        let r = s.execute("configimport", &json!({ "text": text })).unwrap();
        assert!(!issues(&r).is_empty());
    }
    s.execute("configimport", &json!({ "text": "[profile]\nPICKBOX = 1e308\nMAXARRAY = -9223372036854775808\n" })).unwrap();
    let mut after = snapshot(&s);
    // PICKBOX = 1e308 is clamped by SETVAR as when typed; everything else is as it was.
    after["profile"]["PICKBOX"] = before["profile"]["PICKBOX"].clone();
    after["profile"]["MAXARRAY"] = before["profile"]["MAXARRAY"].clone();
    assert_eq!(after, before);
}

#[test]
fn without_a_drawing_the_drawing_sections_are_skipped() {
    let mut s = Session::new();
    s.execute("closeall", &json!({})).unwrap();
    assert!(s.docs.is_empty());
    let r = s.execute("configexport", &json!({})).unwrap();
    assert!(r["settings"].get("drawing").is_none());
    let r = s.execute("configimport", &json!({ "text": "[profile]\nPICKBOX = 4\n[drawing]\nLUNITS = 2\n[plot]\nlineweights = true\n" })).unwrap();
    assert_eq!(issues(&r).len(), 2, "{r}");
    assert_eq!(s.settings.pickbox, 4.0);
}

#[test]
fn the_interface_section_comes_back_for_the_app() {
    let mut s = Session::new();
    let r = s.execute("configimport", &json!({ "text": "[interface]\ntheme = \"dark\"\n" })).unwrap();
    assert_eq!(r["interface"], json!({ "theme": "dark" }));
    assert_eq!(issues(&r), Vec::<String>::new());
    let r = s.execute("configexport", &json!({ "interface": { "theme": "dark", "modelBackground": null } })).unwrap();
    let text = r["text"].as_str().unwrap();
    assert!(text.contains("[interface]\ntheme = \"dark\"\n"), "{text}");
    assert!(!text.contains("modelBackground"));
    assert!(s.execute("configexport", &json!({ "interface": "dark" })).is_err());
}

#[test]
fn typed_at_the_command_line_it_asks_for_the_file_name() {
    let mut s = Session::new();
    let dir = TempDir::new("config-typed");
    let file = dir.path_str("typed.toml");
    s.cmdline("CONFIGEXPORT").unwrap();
    s.cmdline(&format!("\"{file}\"")).unwrap();
    assert!(s.running.is_none());
    assert!(std::fs::read_to_string(&file).unwrap().contains("[profile]"));
    s.cmdline("CONFIGIMPORT").unwrap();
    s.cmdline(&file).unwrap();
    assert!(s.running.is_none());
}

#[test]
fn the_example_in_the_docs_imports_cleanly() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let doc = include_str!("../../../docs/config-file.md");
    let example = doc.split("```toml\n").nth(1).and_then(|rest| rest.split("```").next()).unwrap();
    let mut s = Session::new();
    let r = s.execute("configimport", &json!({ "text": example })).unwrap();
    assert_eq!(issues(&r), Vec::<String>::new());
    assert_eq!(r["interface"]["theme"], json!("dark"));
    assert_eq!(snapshot(&s)["drawing"]["CANNOSCALE"], json!("1:50"));
    assert_eq!(resolve_alias("rr"), "rectang");
    s.execute("aliasedit", &json!({ "remove": ["c", "rr"] })).unwrap();
}
