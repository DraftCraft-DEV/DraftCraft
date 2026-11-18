//! Workspaces (WORKSPACE, WSCURRENT, WSSAVE) and interface customization (CUI): a saved
//! workspace brings its layout back, the built-in one can't be changed, and the Quick Access
//! Toolbar and ribbon tabs follow the customization.

use draftcraft_engine::Session;
use draftcraft_ui_egui::{CadApp, Services};
use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

fn app() -> CadApp {
    let mut app = CadApp::new(Session::default(), Services::default());
    app.run("new", json!({})).unwrap();
    app.ui.start_tab = false;
    app
}

/// The built-in workspaces, then `more`.
fn listed(more: &[&str]) -> serde_json::Value {
    let mut v = vec!["2D Drafting"];
    if cfg!(feature = "solids") {
        v.push("3D Modeling");
    }
    v.extend_from_slice(more);
    json!(v)
}

#[test]
fn a_saved_workspace_restores_its_layout() {
    let mut app = app();
    app.ui.show_status_bar = false;
    app.ui.ribbon_minimized = true;
    let r = app.run("workspace", json!({"operation": "saveAs", "name": "Minimal"})).unwrap();
    assert_eq!(r["current"], "Minimal");
    assert_eq!(r["workspaces"], listed(&["Minimal"]));
    // The built-in workspace is the default layout.
    app.run("wscurrent", json!({"name": "2D Drafting"})).unwrap();
    assert!(app.ui.show_status_bar && !app.ui.ribbon_minimized);
    // The saved one brings its layout back.
    app.run("wscurrent", json!({"name": "minimal"})).unwrap();
    assert!(!app.ui.show_status_bar && app.ui.ribbon_minimized);
    assert_eq!(app.run("wscurrent", json!({})).unwrap()["current"], "Minimal");
}

#[test]
fn the_built_in_workspace_and_the_current_one_are_protected() {
    let mut app = app();
    assert!(app.run("workspace", json!({"operation": "saveAs", "name": "2D Drafting"})).is_err());
    assert!(app.run("workspace", json!({"operation": "delete", "name": "2D Drafting"})).is_err());
    app.run("wssave", json!({"name": "Mine"})).unwrap();
    assert!(app.run("workspace", json!({"operation": "delete", "name": "Mine"})).is_err(), "Mine is current");
    app.run("workspace", json!({"operation": "rename", "name": "Mine", "to": "Ours"})).unwrap();
    assert_eq!(app.ui.current_workspace, "Ours");
    app.run("wscurrent", json!({"name": "2D Drafting"})).unwrap();
    let r = app.run("workspace", json!({"operation": "delete", "name": "Ours"})).unwrap();
    assert_eq!(r["workspaces"], listed(&[]));
    assert!(app.run("wscurrent", json!({"name": "Nowhere"})).is_err());
    assert!(app.run("workspace", json!({"operation": "explode"})).is_err());
}

/// The 3D Modeling workspace swaps in AutoCAD's 3D ribbon: Home (with the modeling panels),
/// Solid and Visualize, then the shared tabs; 2D Drafting brings the 2D ribbon back.
#[cfg(feature = "solids")]
#[test]
fn the_3d_modeling_workspace_shows_the_3d_ribbon() {
    let mut app = app();
    assert!(app.run("workspace", json!({"operation": "delete", "name": "3D Modeling"})).is_err());
    assert!(app.run("workspace", json!({"operation": "saveAs", "name": "3d modeling"})).is_err());
    app.run("wscurrent", json!({"name": "3d modeling"})).unwrap();
    assert_eq!(app.ui.current_workspace, "3D Modeling");
    let mut h = Harness::builder().with_size(vec2(1600.0, 900.0)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    for tab in ["Home", "Solid", "Visualize", "Insert", "Annotate"] {
        assert!(h.query_by_role_and_label(Role::Tab, tab).is_some(), "{tab}");
    }
    assert!(h.query_all_by_label("Box").next().is_some(), "the Modeling panel is on Home");
    h.state_mut().run("wscurrent", json!({"name": "2D Drafting"})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_role_and_label(Role::Tab, "Solid").is_none());
    assert!(h.query_by_role_and_label(Role::Tab, "Visualize").is_none());
    assert!(h.query_by_role_and_label(Role::Tab, "Home").is_some());
}

#[test]
fn cui_customizes_the_quick_access_toolbar_and_the_tabs() {
    let mut app = app();
    let r = app.run("ui.cui", json!({"qat": ["new", "LINE", "circle"], "hiddenTabs": ["view", "Output"]})).unwrap();
    assert_eq!(r["qat"], json!(["new", "line", "circle"]));
    assert_eq!(app.ui.hidden_tabs, vec!["View".to_string(), "Output".to_string()]);
    assert!(app.run("ui.cui", json!({"qat": ["nosuchcommand"]})).is_err());
    assert!(app.run("ui.cui", json!({"hiddenTabs": ["Nowhere"]})).is_err());

    let mut h = Harness::builder().with_size(vec2(1400.0, 800.0)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    assert!(h.query_by_role_and_label(Role::Tab, "Output").is_none(), "Output is hidden");
    assert!(h.query_by_role_and_label(Role::Tab, "Insert").is_some());
    // The standard toolbar comes back.
    h.state_mut().run("ui.cui", json!({"qat": null, "hiddenTabs": []})).unwrap();
    h.run_steps(3);
    assert!(h.query_by_role_and_label(Role::Tab, "Output").is_some());
    assert_eq!(h.state().ui.qat, None);
}

#[test]
fn typed_cui_opens_the_dialog_and_never_from_json() {
    let mut app = app();
    app.cmdline("cui");
    assert_eq!(app.ui.dialog.as_deref(), Some("cui"));
    app.ui.dialog = None;
    app.run("workspace", json!({"operation": "list"})).unwrap();
    assert_eq!(app.ui.dialog, None);
}

#[test]
fn a_saved_workspace_named_3d_modeling_is_not_hidden() {
    // Review of PR 20: a "3D Modeling" saved by a DraftCraft built without solids was shadowed
    // by the built-in one and could not be reached or deleted.
    let mut app = app();
    let mut saved = draftcraft_ui_egui::workspaces::SavedWorkspace { name: "3D Modeling".into(), ..Default::default() };
    saved.show_status_bar = false;
    app.ui.workspaces.push(saved);
    // Listed once.
    let r = app.run("workspace", json!({"operation": "list"})).unwrap();
    let names: Vec<&str> = r["workspaces"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(names.iter().filter(|n| **n == "3D Modeling").count(), 1, "{names:?}");
    // Its layout comes back.
    app.run("wscurrent", json!({"name": "3D Modeling"})).unwrap();
    assert!(!app.ui.show_status_bar);
    // And it can be deleted (from another workspace): the built-in one, if any, stays.
    app.run("wscurrent", json!({"name": "2D Drafting"})).unwrap();
    let r = app.run("workspace", json!({"operation": "delete", "name": "3D Modeling"})).unwrap();
    assert_eq!(r["workspaces"], listed(&[]));
    if cfg!(feature = "solids") {
        app.run("wscurrent", json!({"name": "3D Modeling"})).unwrap();
        assert!(app.ui.show_status_bar, "the built-in layout");
        assert!(app.run("workspace", json!({"operation": "delete", "name": "2D Drafting"})).is_err());
    }
}
