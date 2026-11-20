//! A page the desktop app hosts in the central area beside the drawing: TraceDraft, the vector
//! editor, which the bottom tab bar shows as a tab after the layouts (Model | Layout1 | … |
//! TraceDraft1 | +). This crate knows the page only through [`HostedPage`], so it stays free of
//! the editor's code and keeps building for the web; the desktop app (`apps/draftcraft`) gives
//! [`crate::CadApp::hosted`] the real editor.

/// The name of the page's tab in the bottom tab bar: the product's name, numbered as layouts are
/// (a name, so not translated).
pub const TAB_LABEL: &str = "TraceDraft1";

/// A page drawn in the central area instead of the drawing while [`crate::CadApp::central`] is
/// [`CentralView::TraceDraft`]. It owns everything inside the area it is given (its own menus,
/// panels and canvas); DraftCraft keeps the window, the title bar and the bottom tab bar.
pub trait HostedPage {
    /// Draw one frame of the page into `ui`, which it fills.
    fn ui(&mut self, ui: &mut egui::Ui);
    /// The frame's input before it starts, on the frames the page is shown.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw: &mut egui::RawInput) {}
    /// Add the faces the page draws with to the interface fonts (egui keeps one set of fonts for
    /// the whole window, so DraftCraft installs them together, [`crate::theme::install_fonts_with`]).
    fn add_fonts(&self, _defs: &mut egui::FontDefinitions) {}
    /// Keep the page's unsaved work at once (backups): DraftCraft calls it when the page is left
    /// for the drawing, so nothing waits on the page's own timer while it is hidden.
    fn save(&mut self) {}
    /// The app closes.
    fn on_exit(&mut self) {}
    /// True once when the page asked to be left (its own File ▸ Exit); DraftCraft shows the
    /// drawing again.
    fn wants_exit(&mut self) -> bool {
        false
    }
}

/// What the central area shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CentralView {
    /// The drawing (model space or a layout) or the Start page.
    #[default]
    Drawing,
    /// The hosted TraceDraft page ([`crate::CadApp::hosted`]).
    TraceDraft,
}

impl crate::CadApp {
    /// Whether the central area shows the hosted TraceDraft page.
    pub fn tracedraft_active(&self) -> bool {
        self.central == CentralView::TraceDraft && self.hosted.is_some()
    }

    /// Show `view` in the central area. Leaving TraceDraft backs up its unsaved work and lets
    /// DraftCraft put its own theme, animation and interface size back into egui, in case the
    /// page changed them.
    pub fn set_central(&mut self, view: CentralView) {
        if self.central == view {
            return;
        }
        if self.central == CentralView::TraceDraft {
            if let Some(page) = self.hosted.as_mut() {
                page.save();
            }
            self.forget_shown_style();
        }
        self.central = view;
    }

    /// The `ui.tracedraft` command: show the hosted page, or say it is not part of this build.
    pub(crate) fn show_tracedraft(&mut self) -> Result<serde_json::Value, String> {
        if self.hosted.is_none() {
            return Err(crate::tl!("TraceDraft is not available in this build").to_string());
        }
        self.set_central(CentralView::TraceDraft);
        Ok(serde_json::Value::Null)
    }
}
