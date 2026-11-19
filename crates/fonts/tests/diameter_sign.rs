//! `%%c` (⌀, U+2300) in a TrueType text style: almost no TrueType font has U+2300, so it used to
//! come out as the font's missing-character box. It must draw the font's own Ø instead.

use std::sync::Arc;

use draftcraft_fonts::ttf;

/// A system font that has Ø but not ⌀ (most of them), or `None` where there is none.
fn a_face_without_the_diameter_sign() -> Option<ttf::Face> {
    let dirs = ["/usr/share/fonts", "/System/Library/Fonts", "/Library/Fonts", "C:\\Windows\\Fonts"];
    let mut stack: Vec<std::path::PathBuf> = dirs.iter().map(Into::into).collect();
    while let Some(p) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ttf"))
                && let Ok(bytes) = std::fs::read(&p)
                && let Some(face) = ttf::Face::new(Arc::new(bytes), 0)
                && face.has_glyph('Ø')
                && !face.has_glyph('\u{2300}')
            {
                return Some(face);
            }
        }
    }
    None
}

#[test]
fn the_diameter_sign_draws_as_the_fonts_own_o_slash_not_a_box() {
    let Some(face) = a_face_without_the_diameter_sign() else { return };
    let (Some(dia), Some(slash)) = (ttf::shape(&face, "%%c25", 2.5, 1.0, 0.0), ttf::shape(&face, "Ø25", 2.5, 1.0, 0.0)) else {
        panic!("the font shapes text");
    };
    assert_eq!(dia.glyphs, slash.glyphs, "⌀ must draw exactly like the font's Ø, not the missing-glyph box");
    assert!((dia.width - slash.width).abs() < 1e-9);
}
