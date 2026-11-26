# hayro-interpret in DraftCraft

This is `hayro-interpret` 0.7.0 as vendored by storytold/craft-libs at revision
`92d392cff132eaf53e13afff77f38b5c11b73aa0` (`vendor/hayro-interpret`, with its PdfCraft
patches), licensed Apache-2.0 OR MIT (`LICENSE-APACHE`, `LICENSE-MIT`).

Changes made by DraftCraft (2026-11-26):

- The fourteen standard-font programs (`assets/Foxit*.pfb`, extracted from PDFium) and
  `assets/LICENSE_FOXIT` are removed, and so is `StandardFont::get_font_data`, the only code that
  read them. DraftCraft ships no bundled fonts besides its own stroke font.
- `InterpreterSettings::default()` no longer resolves standard fonts: its `font_resolver` returns
  `None`. A caller that wants them passes its own resolver. The `embed-fonts` feature stays declared
  (craft-pdf requests it) but embeds nothing.
- The `examples/` and `SOURCE.json` files are not vendored.

The CMYK ICC profile (`assets/CGATS001Compat-v2-micro.icc`, CC0-1.0) is unchanged and still needed.
The predefined CMaps still come from `hayro-cmap` (`embed-cmaps`): that is a separate question,
see `docs/clean-room-review/2026-11-26-bundled-pdf-fonts.md`.
