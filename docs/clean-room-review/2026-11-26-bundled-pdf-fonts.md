# Review note: bundled PDF fonts removed (2026-11-26)

For review by the project's partners. The change is in the branch `claude/relaxed-franklin-vf134d`.
Written by the agent that made it; the owner asked for the fonts to be removed and for this note.

## What was found

DraftCraft's PDF reader (PDF underlays, PDFIMPORT, PDF raster) uses `craft-pdf`, which uses the
`hayro-interpret` crate. That crate embeds, through its `embed-fonts` feature, **14 Type 1 font
programs** (`assets/Foxit*.pfb`, about 270 KB): Sans, Serif and Fixed (regular, bold, italic, bold
italic each), Symbol and Dingbats. They stand in for the 14 standard PDF fonts (Helvetica, Times,
Courier, Symbol, ZapfDingbats) when a PDF does not embed them.

- **Origin:** extracted from PDFium (Foxit Software, "Copyright 2014 PDFium Authors", BSD-3-Clause;
  text kept in the old crate's `assets/LICENSE_FOXIT`).
- **Verified:** the bytes of all 14 were present in the released `draftcraft-cli` binary built from
  `main` (checked by searching the binary for 48 bytes from the middle of each file). They were
  not mentioned in `NOTICE` or `ATTRIBUTION.md`.
- **Why it was pulled in:** `craft-libs` (upstream's library repository) requests the feature for every
  user of `craft-pdf`; our `Cargo.toml` could not switch it off from outside.
- **Which rule:** `AGENTS.md` says DraftCraft contains no Autodesk, Adobe or Avid fonts and that any
  file asset needs an `ATTRIBUTION.md` row. These are Foxit's fonts, not Adobe's, and BSD-3 allows
  redistribution; whether they break the rule is a judgement. The owner decided to remove them.

## What changed

- `vendor/hayro-interpret/` is a copy of the crate as pinned (craft-libs `92d392cf`, which carries
  PdfCraft patches), **without** the 14 font files, their licence file and the one function that
  read them (`StandardFont::get_font_data`). `DRAFTCRAFT-CHANGES.md` in that directory lists every
  change. The root `Cargo.toml` patches `hayro-interpret` to this path.
- `crates/io/src/pdf_read/standard_fonts.rs`: when a PDF uses one of the standard fonts without
  embedding it, the reader now looks for an **installed** font through the font table (Arial or
  Helvetica, Times, Courier New, then Liberation or DejaVu where present). Nothing is bundled.
- Verified after the change: none of the 14 fonts' bytes remain in the rebuilt release CLI (0 of 14;
  the binary is 264 KB smaller), the 21 PDF tests in `draftcraft-io` pass, `cargo xtask layers` and
  `cargo xtask assets` pass.

## What users will notice

PDFs that embed their fonts (everything a CAD program plots) are unaffected. A PDF that uses a
standard font without embedding it now shows or imports that text only if a matching font is
installed on the computer; with none installed the text is not drawn (underlay) or not imported
(PDFIMPORT). The web build has no installed fonts, so such text is not drawn there.

## Left in place, for your decision (not fonts, so not touched)

1. **Predefined CMaps** (`hayro-cmap`, `embed-cmaps`, 251 KB, "Copyright 1990-2023 Adobe", BSD-3):
   Adobe's character-code tables, needed to read PDFs that use Japanese, Chinese or Korean fonts
   without embedding the mapping. Not a font, but an Adobe resource that is shipped in the binary and not in
   `NOTICE`. Options: keep and add the notice; or disable (CJK text in some PDFs would not decode).
2. **Width tables** for the 14 standard fonts (`src/font/generated/metrics.rs`, glyph name to
   advance width). One analyst described them as Adobe AFM metrics; the file has no header saying so,
   and I did not trace their origin. Numbers only, no outlines.
3. **CMYK ICC profile** (CC0-1.0): kept, and it now has an `ATTRIBUTION.md` row. krilla's sRGB and
   grey ICC profiles already had rows.
4. **`NOTICE`** still has no notice for PDFium/Foxit (needed only if the fonts come back) or for the
   Adobe CMaps (needed now). Add once decision 1 is made.
5. **Upstream CADCraft** ships the same fonts, with `NOTICE` rows. Pulling its PDF work later must not
   bring the `embed-fonts` setting back; the sync gate denies `*.pfb`.
6. **The 50 solid-model test drawings** made with an AutoCAD educational licence are a separate
   question (see `docs/upstream-sync-plan.md`, D3).
