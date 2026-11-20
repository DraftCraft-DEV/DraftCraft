//! The TraceDraft tab: TraceDraft's vector editor (the `tracedraft` submodule's `tracedraw` library)
//! as the page `draftcraft_ui_egui` shows in the central area ([`draftcraft_ui_egui::hosted`]).
//! The interface crate knows only the [`HostedPage`] trait, so it keeps building for the web; the
//! editor itself is built here, for the desktop.

use draftcraft_ui_egui::hosted::HostedPage;

/// The folder under DraftCraft's settings folder where the editor keeps its preferences, session
/// markers and backups, apart from those of a TraceDraft run on its own.
const SETTINGS_SUBDIR: &str = "tracedraft";

/// The editor as DraftCraft's hosted page (a type of this crate, as the trait's is not ours to
/// implement for the editor's own type).
pub struct TraceDraftPage(tracedraw::Embedded);

impl TraceDraftPage {
    /// The page, cheap until it is first shown (the editor is built then). `config_dir` is
    /// DraftCraft's settings folder; without one the editor keeps its settings where TraceDraft
    /// does.
    pub fn new(config_dir: Option<&std::path::Path>) -> Self {
        TraceDraftPage(tracedraw::Embedded::new(tracedraw::EmbedOptions { config_dir: config_dir.map(|dir| dir.join(SETTINGS_SUBDIR)), open: None }))
    }
}

impl HostedPage for TraceDraftPage {
    fn ui(&mut self, ui: &mut egui::Ui) {
        self.0.ui(ui);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.0.raw_input_hook(ctx, raw);
    }

    fn add_fonts(&self, defs: &mut egui::FontDefinitions) {
        tracedraw::add_ui_fonts(defs);
    }

    fn save(&mut self) {
        self.0.save();
    }

    fn on_exit(&mut self) {
        self.0.on_exit();
    }

    fn wants_exit(&mut self) -> bool {
        self.0.wants_exit()
    }
}
