//! CONFIGEXPORT and CONFIGIMPORT: the portable settings file (`docs/config-file.md`).
//!
//! One flat TOML file carries the settings a user sets up once and wants on every computer, or
//! wants to hand to a colleague: the Options profile (object snaps, grid and snap spacing,
//! selection, MEASUREINIT…), the drawing settings (units, linetype scale, current lineweight,
//! annotation scale…), the plot settings of the layouts (plot style table, lineweights) and the
//! user's command aliases. The desktop and web app add the interface's own preferences (theme,
//! colours, language) under `[interface]`; the engine alone hands that section back unapplied.
//!
//! AutoCAD keeps these in a user profile in the Windows registry (exported from Options ▸
//! Profiles as an `.arg` file) plus a `.pgp` alias file and a template; there is no command
//! that writes them all to one text file, so the names follow AutoCAD's `noun + IMPORT/EXPORT`
//! commands (PDFIMPORT, ATTEXT/ATTOUT…).
//!
//! The file is untrusted input: it is read only up to [`MAX_CONFIG_BYTES`], parsed by the
//! `toml` crate (which limits nesting), and every entry goes through the same checks as SETVAR,
//! OPTIONS, PAGESETUP and ALIASEDIT. Import applies what it can and lists every unknown or
//! invalid entry instead of stopping; a file that is not TOML at all is refused as a whole.

use serde_json::{Map, Value, json};

use super::{CommandSpec, always, bad};
use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

/// The largest settings file read (a real one is a few kilobytes).
pub const MAX_CONFIG_BYTES: usize = 256 * 1024;
/// The most entries read from one file, all sections together.
const MAX_ENTRIES: usize = 20_000;
/// The most problems listed.
const MAX_ISSUES: usize = 100;
/// The file format version this build writes and reads.
pub const VERSION: i64 = 1;
/// The name the command line and the file dialogs suggest.
pub const DEFAULT_FILE: &str = "draftcraft-settings.toml";

/// The drawing settings the `[drawing]` section carries, in the order the file lists them.
pub const DRAWING_VARS: &[&str] = &[
    "MEASUREMENT",
    "INSUNITS",
    "LUNITS",
    "LUPREC",
    "AUNITS",
    "AUPREC",
    "ANGBASE",
    "LTSCALE",
    "CELWEIGHT",
    "LWDEFAULT",
    "DIMSCALE",
    "TEXTSIZE",
    "CANNOSCALE",
];

/// The page setup entries of the `[plot]` section (PAGESETUP's names).
pub const PLOT_KEYS: &[&str] = &["plotStyleTable", "displayPlotStyles", "plotTransparency", "lineweights"];

/// The order `[interface]` lists the app's preferences in (the OPTIONS JSON form's names).
const INTERFACE_ORDER: &[&str] = &[
    "theme",
    "animations",
    "interfaceLanguage",
    "interfaceScale",
    "modelBackground",
    "paperBackground",
    "selectionColor",
    "snapMarkerSize",
    "systemCursor",
    "historyLines",
    "timeSensitiveRightClick",
    "saveFormat",
    "saveVersion",
    "SAVETIME",
    "ISAVEBAK",
];

/// The sections a file may have.
const SECTIONS: &[&str] = &["interface", "profile", "drawing", "plot", "aliases"];

/// The section only the app applies (the interface's own preferences).
pub const INTERFACE: &str = "interface";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("configexport", "Export Settings...", run_configexport)
            .menu(&["Tools", "Export Settings..."])
            .params("{file?: path of the .toml file to write (omitted: the text comes back as `text`), interface?: {name: value} the app adds} → {text, settings, file?}")
            .enabled(always)
            .noundo()
            .interactive(|_| Ok(Box::new(FileM { export: true }))),
        CommandSpec::new("configimport", "Import Settings...", run_configimport)
            .menu(&["Tools", "Import Settings..."])
            .params("{file?: path of a .toml settings file | text?: its contents | settings?: the same as JSON {profile?, drawing?, plot?, aliases?}, quiet?: no command-line report} → {applied: [section.name], issues: [text], interface?: the section only the app applies}")
            .enabled(always)
            .interactive(|_| Ok(Box::new(FileM { export: false }))),
    ]
}

// ---------- the file ----------

/// A TOML value as JSON (the parser already limits how deep it nests).
fn json_of(v: &toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f).map_or(Value::Null, Value::Number),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(a) => Value::Array(a.iter().map(json_of).collect()),
        toml::Value::Table(t) => Value::Object(t.iter().map(|(k, v)| (k.clone(), json_of(v))).collect()),
    }
}

/// A JSON value as TOML; `None` for null (TOML has none: a missing entry keeps the setting).
fn toml_of(v: &Value) -> Option<toml::Value> {
    Some(match v {
        Value::Null => return None,
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => toml::Value::Integer(i),
            None => toml::Value::Float(n.as_f64().filter(|f| f.is_finite())?),
        },
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(a) => toml::Value::Array(a.iter().filter_map(toml_of).collect()),
        Value::Object(m) => toml::Value::Table(m.iter().filter_map(|(k, v)| Some((k.clone(), toml_of(v)?))).collect()),
    })
}

/// Parse a settings file's text into its sections, as JSON. Refused when it is too large or not
/// TOML; what is inside is checked when it is applied.
pub fn parse(text: &str) -> std::result::Result<Map<String, Value>, String> {
    if text.len() > MAX_CONFIG_BYTES {
        return Err(format!("the settings file is larger than {} KB", MAX_CONFIG_BYTES / 1024));
    }
    let table: toml::Table = text.trim_start_matches('\u{feff}').parse().map_err(|e: toml::de::Error| {
        let msg: String = e.message().chars().take(300).collect();
        match e.span().map(|r| line_of(text, r.start)) {
            Some(line) => format!("not a settings file (line {line}: {msg})"),
            None => format!("not a settings file ({msg})"),
        }
    })?;
    match json_of(&toml::Value::Table(table)) {
        Value::Object(m) => Ok(m),
        _ => Err("not a settings file".into()),
    }
}

/// The 1-based line of byte offset `at`.
fn line_of(text: &str, at: usize) -> usize {
    text.as_bytes().iter().take(at).filter(|b| **b == b'\n').count().saturating_add(1)
}

/// The file's text for `settings` (sections as built by [`snapshot`], the app's `interface` included).
pub fn render(settings: &Map<String, Value>) -> String {
    let mut out = String::from(
        "# DraftCraft settings (docs/config-file.md).\n# Import it with CONFIGIMPORT (Manage ▸ Customization ▸ Import Settings); entries left out keep their current value.\n\n",
    );
    out.push_str(&format!("version = {VERSION}\n"));
    let notes = [
        (INTERFACE, "The interface: theme, colours, language, size (the Options dialog)."),
        ("profile", "Drafting settings kept with the user profile (OPTIONS, SETVAR): snaps, grid, selection, MEASUREINIT."),
        ("drawing", "Applied to the current drawing: units, linetype scale, current lineweight, annotation scale."),
        ("plot", "Applied to every layout of the current drawing (PAGESETUP)."),
        ("aliases", "Command aliases: alias = \"command\" (ALIASEDIT)."),
    ];
    for (name, note) in notes {
        let Some(Value::Object(section)) = settings.get(name) else { continue };
        if section.is_empty() {
            continue;
        }
        // The order the Options dialog and the docs use; anything else after, as it came.
        let order: &[&str] = match name {
            INTERFACE => INTERFACE_ORDER,
            "profile" => super::options::PROFILE_VARS,
            "drawing" => DRAWING_VARS,
            "plot" => PLOT_KEYS,
            _ => &[],
        };
        let mut ordered: Vec<(String, Value)> = order.iter().filter_map(|k| Some(((*k).to_string(), section.get(*k)?.clone()))).collect();
        ordered.extend(section.iter().filter(|(k, _)| !order.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())));
        let mut body = toml::Table::new();
        for (k, v) in &ordered {
            if let Some(t) = toml_of(v) {
                body.insert(k.clone(), t);
            }
        }
        let mut one = toml::Table::new();
        let body = toml::Value::Table(body);
        one.insert(name.to_string(), body);
        out.push_str(&format!("\n# {note}\n{one}"));
    }
    out
}

// ---------- export ----------

/// The settings as the file holds them, by section (the app adds `interface`).
pub fn snapshot(s: &Session) -> Map<String, Value> {
    let mut m = Map::new();
    if let Value::Object(profile) = super::options::profile(s) {
        m.insert("profile".into(), Value::Object(profile));
    }
    if let Ok(d) = s.doc() {
        let mut drawing = Map::new();
        let units = crate::units::DrawingUnits::of(d);
        let h = &d.header;
        for name in DRAWING_VARS {
            let v = match *name {
                "MEASUREMENT" => json!(h.i64("MEASUREMENT", 0)),
                "INSUNITS" => json!(h.i64("INSUNITS", 0)),
                "LUNITS" => json!(units.lunits),
                "LUPREC" => json!(units.luprec),
                "AUNITS" => json!(units.aunits),
                "AUPREC" => json!(units.auprec),
                "LTSCALE" => json!(h.f64("LTSCALE", 1.0)),
                "CELWEIGHT" => json!(h.i64("CELWEIGHT", -1)),
                "TEXTSIZE" => json!(h.f64("TEXTSIZE", 0.2)),
                "CANNOSCALE" => json!(d.current_anno_scale().name),
                other => match crate::sysvars::get(s, other) {
                    Some(v) => v,
                    None => continue,
                },
            };
            drawing.insert((*name).to_string(), v);
        }
        m.insert("drawing".into(), Value::Object(drawing));
        // The current layout's page setup, else the first layout's.
        let current = s.space().layout().map(str::to_string);
        let mut layouts: Vec<_> = d.layouts.iter().collect();
        layouts.sort_by_key(|l| l.tab_order);
        let page = current.and_then(|n| d.layout(&n)).or_else(|| layouts.first().copied()).map(|l| &l.page);
        if let Some(p) = page {
            m.insert(
                "plot".into(),
                json!({
                    "plotStyleTable": if draftcraft_doc::no_plot_style_table(&p.plot_style_table) { "None" } else { p.plot_style_table.as_str() },
                    "displayPlotStyles": p.show_plot_styles,
                    "plotTransparency": p.plot_transparency,
                    "lineweights": p.lineweights,
                }),
            );
        }
    }
    let aliases: Map<String, Value> = super::file::user_aliases().into_iter().map(|(a, id)| (a.to_ascii_uppercase(), Value::String(id))).collect();
    if !aliases.is_empty() {
        m.insert("aliases".into(), Value::Object(aliases));
    }
    m
}

/// The `[interface]` section the engine can't make or apply itself, handed over by the app when
/// the commands ask for the file name at the command line (no file dialog, or FILEDIA off).
#[derive(Clone, Debug, Default)]
pub struct HostInterface {
    /// The app's interface settings, set before the command starts: CONFIGEXPORT writes them.
    pub current: Option<Map<String, Value>>,
    /// The `[interface]` section CONFIGIMPORT read, for the app to apply.
    pub imported: Option<Value>,
}

/// CONFIGEXPORT `{file?, interface?}`.
fn run_configexport(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "configexport";
    let mut settings = snapshot(s);
    match p.get(INTERFACE) {
        None | Some(Value::Null) => {
            if let Some(ui) = s.config_interface.current.clone() {
                settings.insert(INTERFACE.into(), Value::Object(ui));
            }
        }
        Some(Value::Object(ui)) => {
            settings.insert(INTERFACE.into(), Value::Object(ui.clone()));
        }
        Some(_) => return Err(bad(cmd, "`interface` must be an object")),
    }
    let text = render(&settings);
    let mut out = json!({ "text": text, "settings": settings });
    match p.get("file") {
        None | Some(Value::Null) => {}
        Some(Value::String(path)) => {
            write_file(cmd, path.trim(), &text)?;
            s.echo(format!("Settings exported to {}.", path.trim()));
            out["file"] = json!(path.trim());
        }
        Some(_) => return Err(bad(cmd, "`file` must be a file path")),
    }
    Ok(out)
}

fn write_file(cmd: &str, path: &str, text: &str) -> Result<()> {
    if path.is_empty() {
        return Err(bad(cmd, "`file` is empty"));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::atomic_write::write_atomic_making_folders(std::path::Path::new(path), text.as_bytes()).map_err(|e| bad(cmd, format!("{path}: {e}")))
    }
    #[cfg(target_arch = "wasm32")]
    {
        super::exchange::deliver_file(cmd, path, text.as_bytes(), super::file::deliver())
    }
}

fn read_file(cmd: &str, path: &str) -> Result<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let bytes = crate::capped_read::read_capped(std::path::Path::new(path), MAX_CONFIG_BYTES).map_err(|e| bad(cmd, format!("{path}: {e}")))?;
        String::from_utf8(bytes).map_err(|_| bad(cmd, format!("{path}: not UTF-8 text")))
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err(bad(cmd, format!("{path}: files can't be read in the browser; pass the file's contents as `text`")))
    }
}

// ---------- import ----------

/// The sections CONFIGIMPORT's parameters name: `settings` (JSON), `text` or `file` (TOML).
pub fn load_params(cmd: &str, p: &Value) -> Result<Map<String, Value>> {
    if let Some(v) = p.get("settings").filter(|v| !v.is_null()) {
        return match v {
            Value::Object(m) => Ok(m.clone()),
            _ => Err(bad(cmd, "`settings` must be an object of sections")),
        };
    }
    let text = match (p.get("text"), p.get("file")) {
        (Some(Value::String(t)), _) => t.clone(),
        (Some(v), _) if !v.is_null() => return Err(bad(cmd, "`text` must be the file's contents")),
        (_, Some(Value::String(path))) => read_file(cmd, path.trim())?,
        (_, Some(v)) if !v.is_null() => return Err(bad(cmd, "`file` must be a file path")),
        _ => return Err(bad(cmd, "pass `file` (a .toml settings file), `text` or `settings`")),
    };
    parse(&text).map_err(|e| bad(cmd, e))
}

/// What an import did: the entries applied (`section.name`) and the problems.
#[derive(Default, Debug)]
pub struct Report {
    pub applied: Vec<String>,
    pub issues: Vec<String>,
    /// Problems not listed once [`MAX_ISSUES`] were.
    pub more: usize,
}

impl Report {
    pub fn issue(&mut self, msg: impl Into<String>) {
        if self.issues.len() < MAX_ISSUES {
            self.issues.push(msg.into());
        } else {
            self.more = self.more.saturating_add(1);
        }
    }
    /// The one-line summary the command line shows.
    pub fn summary(&self) -> String {
        let skipped = self.issues.len().saturating_add(self.more);
        match skipped {
            0 => format!("{} setting(s) imported.", self.applied.len()),
            n => format!("{} setting(s) imported, {n} skipped.", self.applied.len()),
        }
    }
    pub fn to_json(&self) -> Value {
        let mut issues = self.issues.clone();
        if self.more > 0 {
            issues.push(format!("… and {} more", self.more));
        }
        json!({ "applied": self.applied, "issues": issues, "message": self.summary() })
    }
}

/// Apply the sections the engine owns; the `interface` section is left to the caller.
pub fn apply(s: &mut Session, settings: &Map<String, Value>, r: &mut Report) {
    let mut budget = MAX_ENTRIES;
    match settings.get("version") {
        None => {}
        Some(v) => match v.as_i64() {
            Some(VERSION) => {}
            Some(n) if n > VERSION => r.issue(format!("version: the file is from a newer DraftCraft (version {n}); what this one knows is imported")),
            _ => r.issue("version: must be 1"),
        },
    }
    for name in settings.keys().filter(|k| *k != "version" && !SECTIONS.contains(&k.as_str())) {
        r.issue(format!("[{}]: unknown section; skipped", clip(name)));
    }
    // Always in the same order, whatever order the file has.
    for name in SECTIONS.iter().filter(|n| **n != INTERFACE) {
        let Some(section) = settings.get(*name) else { continue };
        let Value::Object(entries) = section else {
            r.issue(format!("{name}: must be a section ([{name}])"));
            continue;
        };
        let take = entries.len().min(budget);
        if take < entries.len() {
            r.issue(format!("[{name}]: more than {MAX_ENTRIES} entries in the file; the rest are skipped"));
        }
        budget -= take;
        let entries = entries.iter().take(take);
        match *name {
            "profile" => apply_profile(s, entries, r),
            "drawing" => apply_drawing(s, entries, r),
            "plot" => apply_plot(s, entries, r),
            "aliases" => apply_aliases(entries, r),
            _ => {}
        }
    }
}

/// A file's name or key cut short for a message.
fn clip(k: &str) -> String {
    let mut t: String = k.chars().filter(|c| !c.is_control()).take(64).collect();
    if k.chars().count() > 64 {
        t.push('…');
    }
    t
}

fn apply_profile<'a>(s: &mut Session, entries: impl Iterator<Item = (&'a String, &'a Value)>, r: &mut Report) {
    for (k, v) in entries {
        let name = k.trim().to_ascii_uppercase();
        if !super::options::is_profile_var(&name) {
            r.issue(format!("profile.{}: unknown setting; skipped", clip(k)));
            continue;
        }
        let before = s.settings.clone();
        match crate::sysvars::set(s, &name, v) {
            Ok(()) => r.applied.push(format!("profile.{name}")),
            Err(_) => {
                s.settings = before;
                r.issue(format!("profile.{name}: invalid value {}; skipped", clip(&v.to_string())));
            }
        }
    }
}

/// A finite real number greater than zero (and not absurdly large).
fn positive(v: &Value) -> Option<f64> {
    v.as_f64().filter(|f| f.is_finite() && *f > 0.0 && *f < 1e12)
}

fn apply_drawing<'a>(s: &mut Session, entries: impl Iterator<Item = (&'a String, &'a Value)>, r: &mut Report) {
    let entries: Vec<_> = entries.collect();
    if s.doc().is_err() {
        if !entries.is_empty() {
            r.issue("[drawing]: no drawing is open; skipped");
        }
        return;
    }
    let mut changed = false;
    for (k, v) in entries {
        let name = k.trim().to_ascii_uppercase();
        if !DRAWING_VARS.contains(&name.as_str()) {
            r.issue(format!("drawing.{}: unknown setting; skipped", clip(k)));
            continue;
        }
        match set_drawing_var(s, &name, v) {
            Ok(()) => {
                r.applied.push(format!("drawing.{name}"));
                changed = true;
            }
            Err(e) => r.issue(format!("drawing.{name}: {e}; skipped")),
        }
    }
    if changed {
        s.touch();
    }
}

/// Set one `[drawing]` variable, checked as SETVAR and UNITS check it.
fn set_drawing_var(s: &mut Session, name: &str, v: &Value) -> std::result::Result<(), String> {
    let invalid = || format!("invalid value {}", clip(&v.to_string()));
    let int = |lo: i64, hi: i64| v.as_i64().filter(|i| (lo..=hi).contains(i));
    let d = s.doc_mut().map_err(|e| e.to_string())?;
    match name {
        "MEASUREMENT" => d.header.set_i64(name, int(0, 1).ok_or_else(invalid)?),
        "LUNITS" | "LUPREC" | "AUNITS" | "AUPREC" | "INSUNITS" => {
            let (_, lo, hi) = super::UNIT_RANGES.iter().find(|(k, ..)| k.eq_ignore_ascii_case(name)).copied().ok_or_else(invalid)?;
            let i = int(lo, hi).ok_or_else(invalid)?;
            // Dimension styles that showed the drawing's units follow them, as with UNITS.
            let before = crate::units::DrawingUnits::of(d);
            d.header.set_i64(name, i);
            crate::units::dim_styles_follow_units(d, before);
        }
        "LTSCALE" | "TEXTSIZE" => d.header.set_f64(name, positive(v).ok_or_else(invalid)?),
        "CELWEIGHT" => {
            let i = v.as_i64().filter(|i| (-3..=-1).contains(i) || crate::sysvars::LINEWEIGHTS.contains(i)).ok_or_else(invalid)?;
            d.header.set_i64(name, i);
        }
        // ANGBASE, LWDEFAULT, DIMSCALE and CANNOSCALE: SETVAR's own checks.
        _ => crate::sysvars::set(s, name, v).map_err(|_| invalid())?,
    }
    Ok(())
}

fn apply_plot<'a>(s: &mut Session, entries: impl Iterator<Item = (&'a String, &'a Value)>, r: &mut Report) {
    let entries: Vec<_> = entries.collect();
    let Ok(d) = s.doc() else {
        if !entries.is_empty() {
            r.issue("[plot]: no drawing is open; skipped");
        }
        return;
    };
    // Checked once, then set on every layout.
    let mut table: Option<String> = None;
    let mut flags: Vec<(&str, bool)> = Vec::new();
    for (k, v) in entries {
        let Some(key) = PLOT_KEYS.iter().find(|p| p.eq_ignore_ascii_case(k.trim())).copied() else {
            r.issue(format!("plot.{}: unknown setting; skipped", clip(k)));
            continue;
        };
        if key == "plotStyleTable" {
            let name = v.as_str().map(str::trim).filter(|t| t.chars().count() <= 260 && !t.chars().any(char::is_control));
            match name {
                Some(n) if draftcraft_doc::no_plot_style_table(n) => table = Some(String::new()),
                Some(n) if draftcraft_doc::plot_style_table(d, n).is_some() => table = Some(n.to_string()),
                Some(n) => r.issue(format!("plot.plotStyleTable: this drawing has no plot style table `{}`; skipped", clip(n))),
                None => r.issue("plot.plotStyleTable: must be a table name such as \"monochrome.ctb\" or \"None\"; skipped"),
            }
        } else {
            match v.as_bool() {
                Some(b) => flags.push((key, b)),
                None => r.issue(format!("plot.{key}: must be true or false; skipped")),
            }
        }
    }
    if table.is_none() && flags.is_empty() {
        return;
    }
    let Ok(d) = s.doc_mut() else { return };
    for l in &mut d.layouts {
        if let Some(t) = &table {
            l.page.plot_style_table.clone_from(t);
        }
        for (key, b) in &flags {
            match *key {
                "displayPlotStyles" => l.page.show_plot_styles = *b,
                "plotTransparency" => l.page.plot_transparency = *b,
                "lineweights" => l.page.lineweights = *b,
                _ => {}
            }
        }
    }
    if table.is_some() {
        r.applied.push("plot.plotStyleTable".into());
    }
    r.applied.extend(flags.iter().map(|(k, _)| format!("plot.{k}")));
    s.touch();
}

fn apply_aliases<'a>(entries: impl Iterator<Item = (&'a String, &'a Value)>, r: &mut Report) {
    for (alias, target) in entries {
        let Some(t) = target.as_str() else {
            r.issue(format!("aliases.{}: the command must be a name in quotes; skipped", clip(alias)));
            continue;
        };
        match super::file::add_user_alias(alias, t) {
            Ok(_) => r.applied.push(format!("aliases.{}", clip(alias).to_ascii_uppercase())),
            Err(e) => r.issue(format!("aliases.{}: {e}; skipped", clip(alias))),
        }
    }
}

/// CONFIGIMPORT `{file? | text? | settings?}`.
fn run_configimport(s: &mut Session, p: &Value) -> Result<Value> {
    let settings = load_params("configimport", p)?;
    let mut r = Report::default();
    apply(s, &settings, &mut r);
    let mut out = r.to_json();
    if let Some(ui) = settings.get(INTERFACE) {
        // Only the app applies it; it comes back for the app (or the caller) to use.
        out[INTERFACE] = ui.clone();
    }
    if !super::bool_or(p, "quiet", false) {
        s.echo(format!("CONFIGIMPORT: {}", r.summary()));
        for i in &r.issues {
            s.echo(format!("  {i}"));
        }
    }
    Ok(out)
}

// ---------- at the command line ----------

/// CONFIGEXPORT and CONFIGIMPORT typed where there is no file dialog: the file name.
struct FileM {
    export: bool,
}

/// A typed file name without the quotes around it.
fn unquoted(t: &str) -> &str {
    let t = t.trim();
    t.strip_prefix('"').and_then(|u| u.strip_suffix('"')).unwrap_or(t)
}

impl Interactive for FileM {
    fn name(&self) -> &'static str {
        if self.export { "CONFIGEXPORT" } else { "CONFIGIMPORT" }
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        let q = if self.export { "Enter settings file name to write" } else { "Enter settings file name to read" };
        Prompt::new(q, Accept::TEXT).default(DEFAULT_FILE)
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let file = match &i {
            Input::Text(t) if !unquoted(t).is_empty() => unquoted(t).to_string(),
            Input::Text(_) | Input::Enter => DEFAULT_FILE.to_string(),
            _ => return Ok(Step::Continue),
        };
        let p = json!({ "file": file });
        if self.export {
            run_configexport(s, &p)?;
        } else {
            let r = run_configimport(s, &p)?;
            // The app applies it when the command ends ([`HostInterface::imported`]).
            s.config_interface.imported = r.get(INTERFACE).cloned();
        }
        Ok(Step::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_and_json_carry_the_same_values() {
        let m = parse("version = 1\n[profile]\nPICKBOX = 5\nSNAPUNIT = [0.5, 2.0]\n[aliases]\nZZ = \"line\"\n").unwrap();
        assert_eq!(m["profile"]["PICKBOX"], json!(5));
        assert_eq!(m["profile"]["SNAPUNIT"], json!([0.5, 2.0]));
        let again = parse(&render(&m)).unwrap();
        assert_eq!(again["profile"], m["profile"]);
        assert_eq!(again["aliases"], m["aliases"]);
        // Null has no TOML form: it is left out (a missing entry keeps the setting).
        assert_eq!(toml_of(&json!({"a": null, "b": 1})), toml_of(&json!({"b": 1})));
    }

    #[test]
    fn what_is_not_toml_is_refused_with_its_line() {
        let e = parse("[profile]\nPICKBOX = = 5\n").unwrap_err();
        assert!(e.contains("line 2"), "{e}");
        assert!(parse("\u{0}\u{1}\u{2}garbage").is_err());
        assert!(parse(&"x".repeat(MAX_CONFIG_BYTES + 1)).is_err());
        // Nesting deeper than the parser allows is an error, not a stack overflow.
        let deep = format!("a = {}1{}", "[".repeat(100_000), "]".repeat(100_000));
        assert!(parse(&deep).is_err());
        let deep_tables = format!("a = {}1{}", "{b = ".repeat(50_000), "}".repeat(50_000));
        assert!(parse(&deep_tables).is_err());
    }
}
