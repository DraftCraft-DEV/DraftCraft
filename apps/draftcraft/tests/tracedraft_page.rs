//! The TraceDraft tab's editor (`src/tracedraft.rs`, the `tracedraw` library) as DraftCraft hosts
//! it: built and drawn in the central area of a DraftCraft-like window, leaving the window to
//! DraftCraft. A test binary of its own: the editor starts system tools (the printer list) while it
//! comes up, and on Windows a child process started then holds the sockets other tests have open.

/// One frame of a host window: a bottom bar of its own and the editor in the central area, as
/// `CadApp::draw` lays them out. The viewport commands the frame sent are returned.
fn frame(ctx: &egui::Context, editor: &mut tracedraw::Embedded) -> Vec<egui::ViewportCommand> {
    let mut input =
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() };
    editor.raw_input_hook(ctx, &mut input);
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::bottom("cc_status").show(ui, |ui| {
            ui.label("Model");
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| editor.ui(ui));
    });
    out.textures_delta.clear();
    out.viewport_output.remove(&egui::ViewportId::ROOT).map(|v| v.commands).unwrap_or_default()
}

/// The whole editor is built and draws in DraftCraft's central area, leaves the window's title,
/// closing and frame theme to DraftCraft, and puts DraftCraft's style back after every frame
/// (DraftCraft's dialogs drawn later in the frame keep their look).
#[test]
fn the_editor_draws_in_the_central_area_and_leaves_the_window_to_draftcraft() {
    // The editor draws on the window's thread, which has 8 MB (build.rs); so does this one.
    let run = std::thread::Builder::new().stack_size(8 << 20).spawn(|| {
        tracedraw::prepare_process();
        // As `TraceDraftPage::new` does with DraftCraft's settings folder.
        let dir = std::env::temp_dir().join(format!("draftcraft-tracedraft-test-{}", std::process::id()));
        let mut editor = tracedraw::Embedded::new(tracedraw::EmbedOptions { config_dir: Some(dir.join("tracedraft")), open: None });
        let ctx = egui::Context::default();
        let mut defs = egui::FontDefinitions::default();
        tracedraw::add_ui_fonts(&mut defs);
        assert!(defs.families.contains_key(&egui::FontFamily::Name("ui-bold".into())), "the editor's headings have their face");
        ctx.set_fonts(defs);
        ctx.all_styles_mut(|s| s.animation_time = 0.25);
        // egui itself keeps the window's theme in step with the host's preference; off here, so
        // that a theme command can only come from the editor.
        ctx.options_mut(|o| o.sync_window_theme = false);
        let host = ctx.global_style();
        let started = std::time::Instant::now();
        let mut commands = Vec::new();
        let mut built_frames = 0;
        while built_frames < 3 {
            commands.extend(frame(&ctx, &mut editor));
            assert_eq!(*ctx.global_style(), *host, "DraftCraft's style is back after the frame");
            if editor.is_built() {
                built_frames += 1;
            } else {
                // The editor waits for the system font list, read in the background.
                assert!(started.elapsed() < std::time::Duration::from_secs(300), "the editor was never built");
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        for c in &commands {
            assert!(
                !matches!(
                    c,
                    egui::ViewportCommand::Title(_)
                        | egui::ViewportCommand::Close
                        | egui::ViewportCommand::CancelClose
                        | egui::ViewportCommand::SetTheme(_)
                ),
                "the editor sent {c:?}"
            );
        }
        assert!(!editor.wants_exit());
        assert!(!editor.has_unsaved(), "a new drawing has nothing to save");
        editor.save();
        editor.on_exit();
        let _ = std::fs::remove_dir_all(&dir);
    });
    if let Err(panic) = run.unwrap().join() {
        std::panic::resume_unwind(panic);
    }
}
