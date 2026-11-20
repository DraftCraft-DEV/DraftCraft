//! The TraceDraft tab (`hosted`): a new drawing's tab bar reads Model | Layout1 | TraceDraft1 | +,
//! the tab shows the hosted page in the central area instead of the drawing, and Model or a layout
//! tab shows the drawing again.

use std::cell::Cell;
use std::rc::Rc;

use draftcraft_engine::Session;
use draftcraft_engine::doc::Space;
use draftcraft_ui_egui::hosted::{CentralView, HostedPage, TAB_LABEL};
use draftcraft_ui_egui::{CadApp, Services};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// What the stand-in page saw.
#[derive(Default)]
struct Seen {
    frames: Cell<u32>,
    saves: Cell<u32>,
    leave: Cell<bool>,
    /// The top of the area it was given, in its last frame.
    top: Cell<f32>,
}

/// A page in place of the editor: a label, and the calls it got.
struct Stub(Rc<Seen>);

impl HostedPage for Stub {
    fn ui(&mut self, ui: &mut egui::Ui) {
        self.0.frames.set(self.0.frames.get() + 1);
        self.0.top.set(ui.max_rect().top());
        ui.label("Stand-in page");
    }

    fn save(&mut self) {
        self.0.saves.set(self.0.saves.get() + 1);
    }

    fn wants_exit(&mut self) -> bool {
        self.0.leave.take()
    }
}

fn harness(page: Option<Rc<Seen>>) -> Harness<'static, CadApp> {
    harness_sized(page, 1400.0, 760.0)
}

fn harness_sized(page: Option<Rc<Seen>>, width: f32, height: f32) -> Harness<'static, CadApp> {
    let mut app = CadApp::new(Session::default(), Services::default());
    app.run("new", serde_json::json!({})).unwrap();
    app.ui.start_tab = false;
    app.ui.in_window_menu = false;
    app.integrated_titlebar = true;
    app.hosted = page.map(|seen| Box::new(Stub(seen)) as Box<dyn HostedPage>);
    let mut h = Harness::builder().with_size(egui::vec2(width, height)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(5);
    h
}

fn click_tab(h: &mut Harness<'_, CadApp>, label: &str) {
    h.get_by_role_and_label(Role::Tab, label).click();
    h.run_steps(3);
}

#[test]
fn a_new_drawing_has_model_layout1_and_tracedraft1_before_the_add_button() {
    let h = harness(Some(Rc::default()));
    assert_eq!(h.state().session.doc().unwrap().layouts.len(), 1, "a new drawing has one layout");
    assert!(h.query_by_role_and_label(Role::Tab, "Layout2").is_none());
    let [model, layout, tracedraft] = ["Model", "Layout1", TAB_LABEL].map(|name| h.get_by_role_and_label(Role::Tab, name).rect());
    assert!(model.right() <= layout.left() && layout.right() <= tracedraft.left(), "{model:?} {layout:?} {tracedraft:?}");
    assert!((tracedraft.center().y - model.center().y).abs() < 1.0, "one row");
    // "+" (a new layout) follows the TraceDraft1 tab on the same row.
    let plus =
        h.get_all_by_label("New layout").map(|n| n.rect()).find(|r| r.left() >= tracedraft.right() && (r.center().y - model.center().y).abs() < 8.0);
    assert!(plus.is_some(), "+ after {tracedraft:?}");
}

#[test]
fn the_tracedraft_tab_shows_the_page_and_model_or_a_layout_shows_the_drawing() {
    let seen: Rc<Seen> = Rc::default();
    let mut h = harness(Some(Rc::clone(&seen)));
    assert_eq!(seen.frames.get(), 0, "hidden until its tab is clicked");
    assert!(h.query_by_label("Stand-in page").is_none());

    click_tab(&mut h, TAB_LABEL);
    assert_eq!(h.state().central, CentralView::TraceDraft);
    assert!(h.state().tracedraft_active());
    assert!(seen.frames.get() > 0);
    assert!(h.query_by_label("Stand-in page").is_some(), "the page fills the central area");
    assert_eq!(h.state().session.state().unwrap().space, Space::Model, "the drawing stays as it was");
    assert_eq!(h.get_by_role_and_label(Role::Tab, TAB_LABEL).accesskit_node().is_selected(), Some(true));
    assert_eq!(h.get_by_role_and_label(Role::Tab, "Model").accesskit_node().is_selected(), Some(false), "Model is not the current tab");

    // A layout tab: the drawing again, on that layout; the page backed up its work on the way out.
    click_tab(&mut h, "Layout1");
    assert_eq!(h.state().central, CentralView::Drawing);
    assert_eq!(h.state().session.state().unwrap().space, Space::Paper("Layout1".into()));
    assert!(h.query_by_label("Stand-in page").is_none());
    assert_eq!(seen.saves.get(), 1);

    // Model, from the page: model space.
    click_tab(&mut h, TAB_LABEL);
    assert!(h.state().tracedraft_active());
    click_tab(&mut h, "Model");
    assert_eq!(h.state().central, CentralView::Drawing);
    assert_eq!(h.state().session.state().unwrap().space, Space::Model);

    // Window ▸ TraceDraft, and the page's own Exit, which leaves it.
    h.state_mut().run("ui.tracedraft", serde_json::json!({})).unwrap();
    h.run_steps(3);
    assert!(h.state().tracedraft_active());
    seen.leave.set(true);
    h.run_steps(3);
    assert_eq!(h.state().central, CentralView::Drawing);
}

#[test]
fn without_a_hosted_page_there_is_no_tracedraft_tab() {
    let mut h = harness(None);
    assert!(h.query_by_role_and_label(Role::Tab, TAB_LABEL).is_none());
    assert!(h.state_mut().run("ui.tracedraft", serde_json::json!({})).is_err(), "not part of this build");
    assert!(!h.state().tracedraft_active());
    let window = draftcraft_ui_egui::menus::tree(h.state()).into_iter().find(|(name, _)| name == "Window").map(|(_, entries)| entries);
    let entry = window.unwrap().into_iter().find_map(|e| match e {
        draftcraft_ui_egui::menus::Entry::Item { id, enabled, .. } if id == "ui.tracedraft" => Some(enabled),
        _ => None,
    });
    assert_eq!(entry, Some(false), "Window ▸ TraceDraft is greyed out");
}

#[test]
fn the_tabs_stay_while_the_page_is_shown_with_the_status_bar_turned_off() {
    let mut h = harness(Some(Rc::default()));
    h.state_mut().ui.show_status_bar = false;
    h.run_steps(3);
    assert!(h.query_by_role_and_label(Role::Tab, "Model").is_none(), "the drawing follows the setting");
    h.state_mut().run("ui.tracedraft", serde_json::json!({})).unwrap();
    h.run_steps(3);
    assert!(h.state().tracedraft_active());
    // The tabs are the way back to the drawing.
    click_tab(&mut h, "Model");
    assert_eq!(h.state().central, CentralView::Drawing);
    assert!(h.query_by_role_and_label(Role::Tab, "Model").is_none(), "hidden again with the drawing");
}

#[test]
fn in_a_compact_window_the_tracedraft1_tab_keeps_its_place_before_the_add_button() {
    let mut h = harness_sized(Some(Rc::default()), 900.0, 560.0);
    for shown in [false, true] {
        if shown {
            click_tab(&mut h, TAB_LABEL);
            assert!(h.state().tracedraft_active());
        }
        let model = h.get_by_role_and_label(Role::Tab, "Model").rect();
        let tracedraft = h.get_by_role_and_label(Role::Tab, TAB_LABEL).rect();
        assert!(model.right() <= tracedraft.left() && (tracedraft.center().y - model.center().y).abs() < 1.0, "{model:?} {tracedraft:?}");
        assert!(tracedraft.right() <= 900.0, "inside the window: {tracedraft:?}");
        let plus = h
            .get_all_by_label("New layout")
            .map(|n| n.rect())
            .find(|r| r.left() >= tracedraft.right() && (r.center().y - model.center().y).abs() < 8.0);
        assert!(plus.is_some(), "+ after {tracedraft:?} (TraceDraft shown: {shown})");
    }
}

/// Press `key` with Ctrl (Cmd on macOS) and let go of it, a frame each. Returns whether the press's
/// frame asked to close the window.
fn press_command(h: &mut Harness<'_, CadApp>, key: egui::Key) -> bool {
    for pressed in [true, false] {
        h.event(egui::Event::Key { key, pressed, modifiers: egui::Modifiers::COMMAND, repeat: false, physical_key: None });
        h.step();
        if pressed {
            let commands = h.output().viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.commands.as_slice()).unwrap_or_default();
            if commands.iter().any(|c| matches!(c, egui::ViewportCommand::Close)) {
                return true;
            }
        }
    }
    false
}

/// egui acts on its own window keys at the end of the frame, after the page saw them. While the
/// page is shown Ctrl+Q (a command of the editor's there) does not close DraftCraft and Ctrl+= or
/// Ctrl+- does not zoom the whole window; with the drawing back they are DraftCraft's again. A
/// keyboard zoom made this session outlives a visit to the page.
#[test]
fn ctrl_q_and_the_keyboard_zoom_are_the_pages_while_it_is_shown() {
    let mut h = harness(Some(Rc::default()));
    let window_keys = |h: &Harness<'_, CadApp>| h.ctx.options(|o| !o.quit_shortcuts.is_empty() && o.zoom_with_keyboard);
    assert!(window_keys(&h));
    click_tab(&mut h, TAB_LABEL);
    assert!(h.state().tracedraft_active());
    let zoom = h.ctx.zoom_factor();
    assert!(!press_command(&mut h, egui::Key::Q), "Ctrl+Q on the page closed the window");
    for key in [egui::Key::Equals, egui::Key::Plus, egui::Key::Minus] {
        press_command(&mut h, key);
        assert_eq!(h.ctx.zoom_factor(), zoom, "{key:?} on the page zoomed the window");
    }
    assert!(h.state().tracedraft_active(), "still on the page");

    click_tab(&mut h, "Model");
    assert!(window_keys(&h), "DraftCraft's window keys are back with the drawing");
    assert!(press_command(&mut h, egui::Key::Q), "Ctrl+Q closes DraftCraft again");

    // egui's keyboard zoom in the drawing (as Ctrl+= leaves it) stays after the page was shown.
    h.ctx.set_zoom_factor(1.25);
    h.run_steps(2);
    click_tab(&mut h, TAB_LABEL);
    click_tab(&mut h, "Model");
    assert!((h.ctx.zoom_factor() - 1.25).abs() < 1e-6, "zoom {}", h.ctx.zoom_factor());
}

/// On macOS the ribbon's top row is the window's title bar: the window buttons sit on its left and
/// it moves the window. It stays while the page is shown, with the file tabs and the page below
/// it; where the system draws the title bar, the page starts at the top.
#[test]
fn the_title_row_stays_above_the_page_where_it_is_the_window_title_bar() {
    // The application bar's height (ribbon.rs).
    const TITLE_ROW: f32 = 32.0;
    let seen: Rc<Seen> = Rc::default();
    let mut h = harness(Some(Rc::clone(&seen)));
    let visual_dir = std::env::var("CRAFT_UI_VISUAL_DIR").ok();
    if let Some(directory) = &visual_dir {
        // The ribbon's View tab, with TraceDraft in its Interface panel.
        h.state_mut().ui.ribbon_tab = "View".into();
        h.run_steps(3);
        std::fs::create_dir_all(directory).unwrap();
        h.render().unwrap().save(format!("{directory}/tracedraft-view-tab.png")).unwrap();
    }
    click_tab(&mut h, TAB_LABEL);
    assert!(h.state().tracedraft_active());
    if let Some(directory) = &visual_dir {
        h.render().unwrap().save(format!("{directory}/tracedraft-page-title-row.png")).unwrap();
    }
    // Where the system draws the title bar, and without the file tabs, the page starts at the
    // top of the window's content.
    h.state_mut().integrated_titlebar = false;
    h.state_mut().ui.show_file_tabs = false;
    h.run_steps(3);
    let top = seen.top.get();
    // The title row, then the page.
    h.state_mut().integrated_titlebar = true;
    h.run_steps(3);
    assert!((seen.top.get() - (top + TITLE_ROW)).abs() < 0.5, "the page is below the title row: {} (top {top})", seen.top.get());
    // The title row, the file tabs, then the page.
    h.state_mut().ui.show_file_tabs = true;
    h.run_steps(3);
    let start = h.get_by_role_and_label(Role::Tab, "Start").rect();
    assert!((start.top() - (top + TITLE_ROW)).abs() < 0.5, "the file tabs are below the title row: {start:?} (top {top})");
    assert!(seen.top.get() >= start.bottom() - 0.5, "the page is below the file tabs: {}", seen.top.get());
    assert!(h.state().tracedraft_active());
}
