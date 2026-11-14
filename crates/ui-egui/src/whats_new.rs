//! What's New: the release notes of the versions this copy has just been updated to. They show
//! once, at the first start after an update, and again from Help ▸ What's New. The notes are the
//! repository's `CHANGELOG.md`, built into the program, so nothing is fetched to show them.
//!
//! The settings folder keeps the version that last ran (`last-version.txt`). A newer program
//! shows the releases after that version up to its own. A settings folder from a copy older than
//! this window (it has `workspace.json` but no version file) shows this version's notes. A fresh
//! install, the same version again or an older one shows nothing.

use std::path::Path;

use egui::{RichText, vec2};

use crate::CadApp;
use crate::theme::Tokens;

/// The release notes, newest first: `## X.Y.Z` sections (and `## Unreleased`, never shown).
pub const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// This build's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The file in the settings folder naming the version that last ran.
const SEEN_FILE: &str = "last-version.txt";

/// A file every copy that ran writes in the settings folder, including those older than this
/// window: with no version file next to it, an older copy ran here.
const USED_MARK: &str = "workspace.json";

/// One `## ` section of the changelog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release<'a> {
    /// `2.1.0` (the heading's first word; `Unreleased` for the work since the last release).
    pub version: &'a str,
    /// The Markdown under the heading.
    pub body: &'a str,
}

/// The window's state: the versions it lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WhatsNew {
    pub versions: Vec<String>,
}

/// The `## ` sections of `text`, in file order.
pub fn releases(text: &str) -> Vec<Release<'_>> {
    let mut out = Vec::new();
    let mut open: Option<(&str, usize)> = None;
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some((version, start)) = open.take() {
                out.push(Release { version, body: text.get(start..pos).unwrap_or("").trim() });
            }
            open = Some((heading.split_whitespace().next().unwrap_or(""), pos + line.len()));
        }
        pos += line.len();
    }
    if let Some((version, start)) = open {
        out.push(Release { version, body: text.get(start..).unwrap_or("").trim() });
    }
    out
}

/// `2.1.0` (or `v2.1.0`, `2.1.0-rc.1`) as numbers; `None` for anything else.
pub fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut nums = core.split('.').map(|n| n.parse::<u64>().ok());
    let version = (nums.next()??, nums.next()??, nums.next()??);
    nums.next().is_none().then_some(version)
}

/// The released versions in `text` newer than `after` (all when `None`) and not newer than
/// `current`, newest first as the file lists them.
pub fn versions_between(text: &str, after: Option<&str>, current: &str) -> Vec<String> {
    let Some(current) = parse(current) else { return Vec::new() };
    let after = after.and_then(parse);
    releases(text)
        .into_iter()
        .filter(|r| parse(r.version).is_some_and(|v| v <= current && after.is_none_or(|a| v > a)))
        .map(|r| r.version.to_string())
        .collect()
}

/// What the first start shows: `seen` is the version that last ran here (from the version file),
/// `used` whether a copy ran here before at all. `None` shows nothing.
pub fn on_update(text: &str, seen: Option<&str>, used: bool, current: &str) -> Option<WhatsNew> {
    let versions = match seen.map(parse) {
        Some(Some(seen)) if parse(current).is_some_and(|c| c > seen) => {
            versions_between(text, Some(&format!("{}.{}.{}", seen.0, seen.1, seen.2)), current)
        }
        // An older copy, from before the version file: this version's notes.
        None if used => versions_between(text, None, current).into_iter().filter(|v| v == current).collect(),
        _ => Vec::new(),
    };
    (!versions.is_empty()).then_some(WhatsNew { versions })
}

impl CadApp {
    /// At startup (the desktop host): show What's New when this copy was just updated, and note
    /// this version in the settings folder `dir` so it shows only once.
    pub fn enable_whats_new(&mut self, dir: &Path) {
        let file = dir.join(SEEN_FILE);
        let seen = std::fs::read_to_string(&file).ok().map(|s| s.trim().chars().take(64).collect::<String>());
        let used = seen.is_some() || dir.join(USED_MARK).exists();
        if let Some(w) = on_update(CHANGELOG, seen.as_deref(), used, VERSION) {
            self.whats_new = Some(w);
        }
        if seen.as_deref() != Some(VERSION)
            && let Err(e) = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&file, VERSION))
        {
            log::warn!("could not note the version in {}: {e}", file.display());
        }
    }
}

/// Help ▸ What's New: every release up to this one.
pub fn open_all(app: &mut CadApp) -> Vec<String> {
    let versions = versions_between(CHANGELOG, None, VERSION);
    app.whats_new = Some(WhatsNew { versions: versions.clone() });
    versions
}

/// The window, while it is open.
pub fn show(app: &mut CadApp, ctx: &egui::Context) {
    let Some(w) = &app.whats_new else { return };
    let shown: Vec<Release> = releases(CHANGELOG).into_iter().filter(|r| w.versions.iter().any(|v| v == r.version)).collect();
    let t = Tokens::get();
    let mut open = true;
    let mut close = false;
    egui::Window::new(crate::tl!("What's New in DraftCraft"))
        .id(egui::Id::unique("dc_whats_new"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, 100.0))
        .show(ctx, |ui| {
            ui.set_width(480.0);
            egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                if shown.is_empty() {
                    ui.label(RichText::new(crate::tl!("There are no release notes for this version.")).color(t.text_dim));
                }
                for (i, r) in shown.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(6.0);
                        ui.separator();
                    }
                    ui.label(RichText::new(crate::tf!("DraftCraft {version}", version = r.version)).strong().size(16.0));
                    body(ui, r.body, &t);
                }
            });
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new(crate::tl!("Close")).strong())).clicked() {
                    close = true;
                }
            });
        });
    if !open || close {
        app.whats_new = None;
    }
}

/// A section's Markdown: `### ` headings, `- ` items, paragraphs; `**` and backticks dropped.
fn body(ui: &mut egui::Ui, text: &str, t: &Tokens) {
    let plain = |s: &str| s.replace("**", "").replace('`', "");
    let mut para = String::new();
    let flush = |ui: &mut egui::Ui, para: &mut String| {
        if !para.is_empty() {
            ui.label(RichText::new(plain(para)).color(t.text));
            para.clear();
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(h) = line.strip_prefix("### ") {
            flush(ui, &mut para);
            ui.add_space(6.0);
            ui.label(RichText::new(plain(h)).strong().color(t.text));
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            flush(ui, &mut para);
            ui.label(RichText::new(format!("•  {}", plain(item))).color(t.text_dim));
        } else if line.is_empty() {
            flush(ui, &mut para);
        } else {
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(line);
        }
    }
    flush(ui, &mut para);
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTES: &str = "# Changelog\nintro\n\n## Unreleased\n- next\n\n## 2.1.2\n- two\n\n## 2.1.1 — late\n- one\n\n## 2.1.0\n- zero\n";

    /// The release being built has its notes: `cargo xtask version set` names the Unreleased
    /// section after the new version, and this fails a release that forgot them.
    #[test]
    fn the_changelog_has_notes_for_this_version() {
        let mine = releases(CHANGELOG).into_iter().find(|r| r.version == VERSION);
        assert!(mine.is_some_and(|r| !r.body.is_empty()), "CHANGELOG.md has no `## {VERSION}` section with notes");
    }

    #[test]
    fn sections_and_versions_parse() {
        let r = releases(NOTES);
        assert_eq!(r.iter().map(|r| r.version).collect::<Vec<_>>(), ["Unreleased", "2.1.2", "2.1.1", "2.1.0"]);
        assert_eq!(r.get(2).map(|r| r.body), Some("- one"));
        assert_eq!(parse("v2.10.3-rc.1"), Some((2, 10, 3)));
        assert_eq!(parse("2.1"), None);
        assert_eq!(parse("Unreleased"), None);
        assert_eq!(versions_between(NOTES, Some("2.1.0"), "2.1.2"), ["2.1.2", "2.1.1"]);
        assert_eq!(versions_between(NOTES, None, "2.1.1"), ["2.1.1", "2.1.0"]);
    }

    /// Once after an update, with every release skipped over; never on a fresh install, the same
    /// version or a downgrade.
    #[test]
    fn what_shows_after_an_update() {
        let v = |w: Option<WhatsNew>| w.map(|w| w.versions);
        assert_eq!(v(on_update(NOTES, Some("2.1.0"), true, "2.1.2")), Some(vec!["2.1.2".into(), "2.1.1".into()]));
        assert_eq!(v(on_update(NOTES, Some("2.1.2"), true, "2.1.2")), None);
        assert_eq!(v(on_update(NOTES, Some("2.1.2"), true, "2.1.1")), None);
        assert_eq!(v(on_update(NOTES, None, false, "2.1.2")), None, "fresh install");
        assert_eq!(v(on_update(NOTES, None, true, "2.1.2")), Some(vec!["2.1.2".into()]), "a copy older than the version file");
        assert_eq!(v(on_update(NOTES, Some("garbage"), true, "2.1.2")), None);
    }

    /// The settings folder remembers the version, so the window shows at one start only.
    #[test]
    fn it_shows_once() {
        let dir = std::env::temp_dir().join(format!("draftcraft-whats-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SEEN_FILE), "0.0.1").unwrap();
        let mut app = CadApp::new(draftcraft_engine::Session::new(), crate::Services::default());
        app.enable_whats_new(&dir);
        assert!(app.whats_new.as_ref().is_some_and(|w| w.versions.iter().any(|v| v == VERSION)));
        assert_eq!(std::fs::read_to_string(dir.join(SEEN_FILE)).unwrap(), VERSION);
        let mut again = CadApp::new(draftcraft_engine::Session::new(), crate::Services::default());
        again.enable_whats_new(&dir);
        assert!(again.whats_new.is_none());
        // Help ▸ What's New opens it any time; Close shuts it.
        assert!(again.run("ui.whatsnew", serde_json::json!({})).is_ok());
        assert!(again.whats_new.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
