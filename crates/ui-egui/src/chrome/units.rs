//! The status bar's drawing units: it shows the drawing's unit (INSUNITS, as "mm" or "in") and
//! its length format (LUNITS), and switches either through UNITS; its last entry opens the
//! Drawing Units dialog. Like AutoCAD's Units status control, but naming the unit too, so a
//! metric drawing and an imperial one can be told apart at a glance.

use egui::{Rect, Sense, pos2, vec2};
use serde_json::json;

use crate::CadApp;
use crate::theme::Tokens;

/// LUNITS values and their names, in the order the menu lists them.
pub const LENGTH_FORMATS: [(i64, &str); 5] = [(4, "Architectural"), (2, "Decimal"), (3, "Engineering"), (5, "Fractional"), (1, "Scientific")];

/// The units the menu offers directly (INSUNITS values), metric first; the rest are in the dialog.
pub const COMMON_UNITS: [i64; 6] = [4, 5, 6, 1, 2, 0];

/// The short name of an INSUNITS value, as the status bar shows it ("mm", "in"). Out-of-range
/// values read as unitless, as AutoCAD reads them.
pub fn unit_abbrev(insunits: i64) -> &'static str {
    match insunits {
        1 => "in",
        2 => "ft",
        3 => "mi",
        4 => "mm",
        5 => "cm",
        6 => "m",
        7 => "km",
        8 => "µin",
        9 => "mil",
        10 => "yd",
        11 => "Å",
        12 => "nm",
        13 => "µm",
        14 => "dm",
        15 => "dam",
        16 => "hm",
        17 => "Gm",
        18 => "AU",
        19 => "ly",
        20 => "pc",
        21 => "US ft",
        22 => "US in",
        23 => "US yd",
        24 => "US mi",
        _ => "Unitless",
    }
}

/// The full name of an INSUNITS value ("Millimeters"); out of range reads as unitless.
pub fn unit_name(insunits: i64) -> &'static str {
    usize::try_from(insunits).ok().and_then(|i| crate::dialogs::INSERTION_UNITS.get(i)).copied().unwrap_or("Unitless")
}

/// The status bar's label: the unit and the format, "mm · Decimal".
pub fn status_label(insunits: i64, lunits: i64) -> String {
    let unit = if insunits == 0 { crate::i18n::t("Unitless").to_string() } else { unit_abbrev(insunits).to_string() };
    format!("{unit} · {}", crate::i18n::t(format_name(lunits)))
}

/// The name of a LUNITS value ("Decimal" for one out of range, as AutoCAD reads it).
pub fn format_name(lunits: i64) -> &'static str {
    LENGTH_FORMATS.iter().find(|(v, _)| *v == lunits).map_or("Decimal", |(_, n)| n)
}

/// Draw the units control ending at `right`; returns its left edge, or `right` when there is no
/// drawing.
pub fn status(app: &mut CadApp, ui: &mut egui::Ui, right: f32, cy: f32) -> f32 {
    let Ok(d) = app.session.doc() else { return right };
    let lunits = d.header.i64("LUNITS", 2);
    let insunits = d.header.i64("INSUNITS", 0);
    let current = status_label(insunits, lunits);
    let t = Tokens::get();
    let p = ui.painter().clone();
    let g = p.layout_no_wrap(current, crate::theme::small(), t.text);
    let w = g.size().x.min(180.0) + 22.0;
    let r = Rect::from_min_size(pos2(right - w, cy - 9.0), vec2(w, 18.0));
    let resp = ui.interact(r, ui.scope_id().with("drawing_units"), Sense::click());
    p.rect_filled(r, 3.0, if resp.hovered() { t.control_hover } else { t.control.gamma_multiply(0.5) });
    p.with_clip_rect(r.shrink2(vec2(4.0, 0.0))).galley(pos2(r.left() + 6.0, r.center().y - g.size().y / 2.0), g, t.text);
    let (ax, ay) = (r.right() - 9.0, r.center().y);
    p.line_segment([pos2(ax - 3.0, ay - 1.5), pos2(ax, ay + 1.5)], egui::Stroke::new(1.2, t.text_dim));
    p.line_segment([pos2(ax, ay + 1.5), pos2(ax + 3.0, ay - 1.5)], egui::Stroke::new(1.2, t.text_dim));
    crate::icons::describe_control(ui, &resp, "Drawing units", None, false);
    let tip = crate::tf!("Drawing units: {unit}, {format}", unit = crate::i18n::t(unit_name(insunits)), format = crate::i18n::t(format_name(lunits)));
    let resp = resp.on_hover_text(tip);
    let mut pick: Option<i64> = None;
    let mut pick_unit: Option<i64> = None;
    let mut dialog = false;
    egui::Popup::from_toggle_button_response(&resp).width(190.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.label(egui::RichText::new(crate::tl!("Unit")).small().weak());
        for v in COMMON_UNITS {
            if ui.selectable_label(v == insunits, crate::i18n::t(unit_name(v))).clicked() {
                pick_unit = Some(v);
                ui.close();
            }
        }
        ui.separator();
        ui.label(egui::RichText::new(crate::tl!("Format")).small().weak());
        for (v, name) in LENGTH_FORMATS {
            if ui.selectable_label(v == lunits, crate::i18n::t(name)).clicked() {
                pick = Some(v);
                ui.close();
            }
        }
        ui.separator();
        if ui.selectable_label(false, crate::i18n::t("Units...")).clicked() {
            dialog = true;
            ui.close();
        }
    });
    let change = match (pick, pick_unit) {
        (Some(v), _) => Some(json!({ "lunits": v })),
        (None, Some(v)) => Some(json!({ "insunits": v })),
        (None, None) => None,
    };
    if let Some(params) = change {
        let res = app.run("units", params);
        app.canvas.list = None;
        if let Err(e) = res {
            app.status = Some((e, crate::now_ms()));
        }
    }
    if dialog {
        app.start("units");
    }
    r.left() - 6.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every format has a name, and the menu's pick reaches the drawing.
    /// The status bar names the unit, so a metric drawing and an imperial one look different.
    #[test]
    fn the_units_label_names_the_unit_and_the_format() {
        assert_eq!(status_label(4, 2), "mm · Decimal");
        assert_eq!(status_label(1, 4), "in · Architectural");
        assert_eq!(unit_name(4), "Millimeters");
        assert_eq!(unit_name(-3), "Unitless");
        assert_eq!(unit_abbrev(999), "Unitless");
        let mut app = CadApp::new(draftcraft_engine::Session::new(), crate::Services::default());
        for v in COMMON_UNITS {
            app.run("units", json!({ "insunits": v })).unwrap();
            assert_eq!(app.session.doc().unwrap().header.i64("INSUNITS", -1), v);
        }
    }

    #[test]
    fn the_units_control_names_and_sets_the_format() {
        assert_eq!(format_name(4), "Architectural");
        assert_eq!(format_name(1), "Scientific");
        assert_eq!(format_name(99), "Decimal");
        let mut app = CadApp::new(draftcraft_engine::Session::new(), crate::Services::default());
        for (v, _) in LENGTH_FORMATS {
            app.run("units", json!({ "lunits": v })).unwrap();
            assert_eq!(app.session.doc().unwrap().header.i64("LUNITS", 0), v);
        }
    }
}
