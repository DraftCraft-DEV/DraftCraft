//! The fonts a PDF's standard fonts (Helvetica, Times, Courier, Symbol, ZapfDingbats) are drawn
//! with when the PDF does not embed them.
//!
//! DraftCraft bundles no font for this: it uses a font installed on the computer, found through
//! the font table (`draftcraft_fonts::ttf::find`, by file name), the way text styles do. With
//! none installed the text of such a PDF is not drawn (underlay) or not imported (PDFIMPORT);
//! PDFs that embed their fonts, as the ones CAD programs plot do, are not affected.

use std::sync::Arc;

use craft_pdf::interpret::InterpreterSettings;
use craft_pdf::interpret::font::{FontData, FontQuery, StandardFont};

/// File names (as the font table spells them, no extension) of installed fonts that stand in for
/// `font`, best first: the Windows and macOS files, then the common free families.
fn candidates(font: StandardFont) -> &'static [&'static str] {
    use StandardFont::{
        Courier, CourierBold, CourierBoldOblique, CourierOblique, Helvetica, HelveticaBold, HelveticaBoldOblique, HelveticaOblique, Symbol,
        TimesBold, TimesBoldItalic, TimesItalic, TimesRoman, ZapfDingBats,
    };
    match font {
        Helvetica => &["arial", "helvetica", "liberationsans-regular", "dejavusans"],
        HelveticaBold => &["arialbd", "arial bold", "liberationsans-bold", "dejavusans-bold"],
        HelveticaOblique => &["ariali", "arial italic", "liberationsans-italic", "dejavusans-oblique"],
        HelveticaBoldOblique => &["arialbi", "arial bold italic", "liberationsans-bolditalic", "dejavusans-boldoblique"],
        TimesRoman => &["times", "times new roman", "liberationserif-regular", "dejavuserif"],
        TimesBold => &["timesbd", "times new roman bold", "liberationserif-bold", "dejavuserif-bold"],
        TimesItalic => &["timesi", "times new roman italic", "liberationserif-italic", "dejavuserif-italic"],
        TimesBoldItalic => &["timesbi", "times new roman bold italic", "liberationserif-bolditalic", "dejavuserif-bolditalic"],
        Courier => &["cour", "courier new", "liberationmono-regular", "dejavusansmono"],
        CourierBold => &["courbd", "courier new bold", "liberationmono-bold", "dejavusansmono-bold"],
        CourierOblique => &["couri", "courier new italic", "liberationmono-italic", "dejavusansmono-oblique"],
        CourierBoldOblique => &["courbi", "courier new bold italic", "liberationmono-bolditalic", "dejavusansmono-boldoblique"],
        Symbol => &["symbol", "dejavusans"],
        ZapfDingBats => &["dejavusans"],
    }
}

/// An installed font for `font`, if there is one.
pub(super) fn resolve(font: StandardFont) -> Option<(FontData, u32)> {
    candidates(font).iter().find_map(|name| {
        let face = draftcraft_fonts::ttf::find(name)?;
        let data: FontData = face.bytes;
        Some((data, face.index))
    })
}

/// Interpreter settings that draw a PDF's non-embedded standard fonts with installed fonts.
pub(super) fn settings() -> InterpreterSettings {
    InterpreterSettings {
        font_resolver: Arc::new(|query| match query {
            FontQuery::Standard(s) => resolve(*s),
            FontQuery::Fallback(f) => resolve(f.pick_standard_font()),
        }),
        ..InterpreterSettings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_standard_font_has_candidates_and_no_font_is_bundled() {
        use StandardFont::{
            Courier, CourierBold, CourierBoldOblique, CourierOblique, Helvetica, HelveticaBold, HelveticaBoldOblique, HelveticaOblique, Symbol,
            TimesBold, TimesBoldItalic, TimesItalic, TimesRoman, ZapfDingBats,
        };
        for f in [
            Helvetica,
            HelveticaBold,
            HelveticaOblique,
            HelveticaBoldOblique,
            Courier,
            CourierBold,
            CourierOblique,
            CourierBoldOblique,
            TimesRoman,
            TimesBold,
            TimesItalic,
            TimesBoldItalic,
            Symbol,
            ZapfDingBats,
        ] {
            assert!(!candidates(f).is_empty(), "{f:?}");
        }
        // The crate's own default resolver finds nothing: there is nothing bundled to find.
        let none = InterpreterSettings::default();
        assert!((none.font_resolver)(&FontQuery::Standard(Helvetica)).is_none());
    }

    #[test]
    fn an_installed_font_is_found_when_there_is_one() {
        // Whether a font is found depends on the computer; when it is, it must parse.
        if let Some((data, index)) = resolve(StandardFont::Helvetica) {
            let bytes: &[u8] = (*data).as_ref();
            assert!(draftcraft_fonts::ttf::Face::new(Arc::new(bytes.to_vec()), index).is_some());
        }
    }
}
