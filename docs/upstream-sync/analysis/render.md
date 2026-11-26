# Rendering, text/fonts, hatch, linetypes, plot/export, pick/select: DraftCraft vs upstream CADCraft

Scope: `crates/render`, `crates/fonts`, plot/PDF/SVG/PNG paths (`crates/io/src/{pdf,svg,plotter,raster,pdf_read}.rs`,
`crates/engine/src/cmd/{plotters,layout,underlay,sheetset}.rs`), pick/select and the spatial index
(`crates/engine/src/{select,spatial}.rs`), the display list. File I/O, engine commands and UI chrome are
other analysts' areas; items that cross into them are marked "(cross-area)".

Method. Upstream = ref `cadcraft/main`; fork base = `2569dbd5` (2026-11-04); range `2569dbd5..cadcraft/main` has 1,493
commits (925 non-merge), 107 non-merge ones touch `crates/render` or `crates/fonts`. Our `crates/render` and
`crates/fonts` are byte-identical to the base except for our own work (listed under "Where we are ahead"). Trees for
base, upstream HEAD and ours were exported with `cadcraft`->`draftcraft` renamed (scratchpad `base/`, `up/`, `ours/`) and
diffed per file. A three-way `git merge-file` of base -> upstream HEAD onto ours over all of `crates/render` and
`crates/fonts` gives **5 conflict hunks in total, all in `render/src/draw.rs`** (our `note_curve` calls wrap the same
`stroke(..)` lines that upstream turned into `stroke_as(.., Layout)`). The render merge is mechanical; the cost is in the
cross-crate prerequisites (below). Commit dates in the range can be older than the base because PR branches merged
later; always use the range, not `--since`.

Measurements were made with the existing release binaries (`draftcraft-cli`, `cadcraft-cli`), `valgrind --tool=callgrind`,
and small DXF files generated in the scratchpad (`.../scratchpad/rn/`, `.../scratchpad/cg/`). No repo file was touched.

---------------------------------------------------------------------------------------------------------------------

## 1. Why our fence select is 1.6-2.2x slower

**Cause: `text_quad` inside `hit_polylines`, run for every text object in the fence's bounding box.** Nothing in upstream
makes the fence faster; our code does more work per candidate.

Evidence (callgrind, `perf 20000`, only `select_fence` collected):

| | total Ir | `hit_polylines` | `text_quad` -> `build_entities` -> `place_text_entity` -> `fonts::place` |
|---|---|---|---|
| ours | 79.3 M | 60.6 M (76%) | **47.5 M (60% of the whole fence)** |
| upstream | 32.9 M | 14.3 M (43%) | none (`entity_bounds`) |

Everything outside text is the same in both (~32 M Ir); the entire 46 M gap is the text layout. Timing repeats this:
200k entities 116-129 ms vs 58-65 ms; 1M entities 580-600 ms vs 293-313 ms (both pick the same 260 / 634 objects).

How it happens:
- `select_fence_with` asks the index for everything inside the fence's *bounding box*. For the benchmark's 4-point random
  fence that box is 35% of the drawing (348,614 of 1,000,000 candidates), although the three legs can only touch ~3,851
  of them (1.1%, replicated with the benchmark's RNG, `scratchpad/rn/fence_est.py`). Both programs then run
  `hit_polylines` on every candidate.
- Upstream's `hit_polylines` takes text as `entity_bounds` (a cheap estimate). Ours (since 2.1.0 `31275fb6` "text picking"
  and `3dbac8d0` "Pick what objects draw", CHANGELOG 2.2.0 "Blocks and dimensions are picked on their lines and text")
  calls `text_quad`, which builds the text's display list at a fine tolerance and measures the ink, so 1 object in 5 in the
  benchmark pays a full font layout. Dimension/MLeader/Insert candidates pay `build_entities` in both programs.
- Secondary: `reach()` (our spatial-index box) clones a whole `DimStyle` (`with_overrides` always clones) and does a linear
  name lookup for every dimension, once at index build and again per fence/crossing candidate. Not visible in the
  benchmark (no dimensions), real in dimension-heavy drawings.

**Fix (ours, not portable from upstream; S).** In `crates/engine/src/select.rs::select_fence_with`, before
`hit_polylines`, skip a candidate unless some fence leg touches its reach box (the same trust level the crossing-window
path already places in `reach(..).intersects(bx)`):

```rust
let legs: Vec<(Vec2, Vec2)> = fence.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?))).collect();
...
for (e, known) in cands {
    if !selectable(d, e) { continue; }
    if !is_infinite(e) {
        let r = reach(d, e, bounds_of(d, e, known));
        if !r.intersects(&probe) { continue; }
        let r = r.expand(tol);
        if !legs.iter().any(|(a, b)| seg_hits_box(*a, *b, &r)) { continue; }   // or a slab test
    }
    /* existing hit_polylines + line_line loop */
}
```

This cuts the exact tests ~90x at 1M (348k -> 3.9k), so the fence should end up faster than upstream, not merely equal
(estimate only; I could not build). The same change applies to `select_polygon`'s fence-like paths and to crossing
windows over text-dense areas (`inside_text` calls `text_quad` per text candidate). Add the regression test
(`fence_selects_text_only_where_a_leg_crosses_its_box`) and a `perf` note. Also make `reach()` avoid the `DimStyle` clone
when `dm.overrides` is empty (or cache text height per style name).

Other pick numbers re-measured (4 cores): window select, osnap, pick are equal; first pick (incl. index build) is
equal within noise (673-696 ms vs 670-755 ms); our display-list build is ~5-20% faster at 4 threads (noisy on this 4-core box) and identical at 1
thread (`RAYON_NUM_THREADS=1`: 752-806 vs 768-776 ms; 4 threads at 500k: 297-325 vs 301-355 ms). The likely reason for
upstream's slower multi-thread build is its new shared atomic point budget (`Builder::charge` does a `fetch_add` on one
`AtomicUsize` for every primitive from every rayon thread). When we adopt the budget (item A2), batch the counter per
thread (flush every ~64k points) to keep our edge.

---------------------------------------------------------------------------------------------------------------------

## 2. Upstream changes since the base, with verdicts

Verdicts: PULL = take as is (rename `cadcraft`->`draftcraft`, add CHANGELOG line, regression test as AGENTS.md requires);
ADAPT = take with changes or only part; SKIP = we have it, it conflicts with our layering, or no gain.
Effort S (< half a day), M (1-3 days), L (a week+). Risk = regression/merge risk.

### A. Crash, hang and memory hardening ("never crash" ranks above features)

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| A1 | Layer and block lookups by name through an index (ours scans `Vec<Layer>` with `eq_ignore_ascii_case` for every entity) | `3532ddf6`, `04c880e9`, `bc5749ec` (fixes): `crates/doc/src/names.rs` (new, 484 lines, `NamedList`/`NamedMap` via `DerefMut`), `doc/src/{lib,tables}.rs`, 4 call sites. | **PULL** (cross-area: doc) | Quadratic in layers x entities. Measured on a 2.9 MB DXF with 30,000 layers and 30,000 lines (`scratchpad/rn/layers30000.dxf`, `perf FILE`): display-list build **1,700-1,930 ms ours vs 8-10 ms upstream (~200x)**; open 3,084 ms vs 281 ms. Affects display list (`resolve_common`), pick/select (`selectable` -> `is_visible`), plots. Our 2.2.0 notes advertise 100,000-layer DWGs opening in under a second; the redraw would then take minutes. | M | Med: `Drawing.layers` changes type, every `layers.push/retain/sort` goes through `DerefMut` (compatible by design). Use the final `04c880e9`/`bc5749ec` version (folded-name key, ordered map; the first version hashed). |
| A2 | Point budget for a display list (`MAX_DISPLAY_POINTS` 64 M; `DisplayList::truncated`) | `fba309f8` (F-06b: `render/src/{lib,display,draw,pipeline,viewports}.rs`), `80c6e074` (F-06a: wide polylines `fill.rs`, `wide.rs`), prerequisites `7804e94e` (`geom`: `MAX_TESSELLATED_POINTS`, `Polyline::tessellate_within`), `a1574780`, `2410a1fd` (hatch) | **PULL + ADAPT** | `MAX_BLOCK_EXPANSION` counts objects, not points. A 2 KB DXF (1000x1000 `INSERT` array of a 500-vertex polyline, `scratchpad/rn/bomb2.dxf`) makes `draftcraft-cli convert` die with `memory allocation of 8589934592 bytes failed` in `Builder::push_polyline` under a 6 GB cap (SIGABRT, max RSS 4.2 GB); upstream finishes in 14 s at 1.56 GB (the build stops at the budget). In the app that abort loses the user's drawing. ADAPT: batch the shared counter per thread (see section 1) so the multi-thread build keeps its edge. | M | Med: textual conflicts only in `draw.rs`, `lib.rs`, `viewports.rs` (our `DCurve` plumbing). `Builder` gains 2 fields; all `Builder { .. }` literals change. |
| A3 | Hatch hostile-input bounds: canonical loop key before tessellation; charge boundary transform and scanline sort; swatch limits; linetype label point cap | `2410a1fd`, `a1574780` (applies cleanly), `7804e94e`, `2c25c2d7` (table part) | **PULL** | Bounded work for crafted hatches/linetypes; same ethos as AGENTS.md "cap input-sized loops". | S-M | Low |
| A4 | Table layout budget: parts counted together, outline fallback, merge cut by run lookup | `2c25c2d7` (`render/src/table.rs`, +22 test lines) | **PULL** | Broken tables built a quadratic merge walk and repeated header cells per part. | S | Low |
| A5 | SHX glyph cache bounded by points as well as glyph count | `5154f575` (`fonts/src/shx/mod.rs`), `09d2e068` (a glyph outlined by two threads is counted once) | **PULL** | Ours: `MAX_CACHED_GLYPHS = 16_384` x `MAX_POINTS = 50_000` per glyph is up to ~13 GB of cache from one hostile font. Upstream's `MAX_CACHED_POINTS = 1<<20` survived into craft-shx. | S | Low |
| A6 | SHX parser/compiler small hardening: doubled minus is an error, name terminator searched in 1,024 bytes, repeated shape number by set, name > 1,024 bytes refused, extended bigfont subshape held to coordinate bound | `01c003e4` (applies cleanly), `b4398791`, `bb91014e`, `82779b50`, `225407ca` (`fonts/src/shx/{compile,parse,program}.rs`) | **PULL** (apply non-test hunks by hand; the patches fail on `tests.rs` context only) | Each is a tiny bound with a test. NOTE: `661e5e2d` (glyph 8,192 points / 32,768 steps instead of 50,000 / 200,000) is in upstream main but **was lost** when SHX was extracted to craft-shx (craft-shx still has 50,000/200,000); worth keeping in ours. | S | Low |
| A7 | Per-line retained-point allowance for MTEXT layout and stroke-font fallback outlines | `cb96c182` (applies cleanly; `fonts/src/{lib,mtext,mtext/para,ttf}.rs`) | **PULL** | A string repeating one detailed fallback glyph copied its outline per occurrence. | S | Low |
| A8 | PDF hyperlink annotations capped file-wide (`MAX_FILE_ANNOTATIONS`), long targets dropped before copy | `7e780548` (applies cleanly; `io/src/pdf.rs`) | **PULL** | Per-page copy of the link list multiplied by the sheet count. | S | Low |
| A9 | Build-script hardening: font manifest confined, regular file, 1 MiB cap | `5a6aa9b6` (`fonts/build.rs`) | **PULL** | `/dev/zero` or a FIFO as `manifest.txt` could hang or fill memory. Note `build.rs` already bars Adobe families (`BARRED_FAMILIES`). | S | Low |
| A10 | Font guard series: font search asks a per-folder guard before opening a file (roots confinement, symlink targets judged by destination) | `387a6e6b`, `4efecc2f`, `8cfb9cfa`, `d4c4c799`, `9cea2f78`, `d4c1ae0e`, `6dfcbf29`, `de6b841d`, `b42d54a3`, `8cbade6e` (`fonts/src/shx/table.rs`, `tests/shx_guard*.rs`) | **SKIP for now** | Needs upstream's automation-roots framework (`engine/src/file_access.rs`, `mcp` roots), which we do not have. Adopt together with it (engine/MCP analyst) | M | n/a |
| A11 | Spline fit solver limited to the band; NURBS derivative allocation-free | `98b47c19`, `c2175377` (`geom/src/{spline,nurbs}.rs`; results bit-identical) | **PULL** (cross-area: geom) | Faster spline tessellation in display lists; verified against old oracle in the commits. | S | Low |

### B. Rendering fidelity (what is drawn)

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| B1 | Dashes laid out as AutoCAD plots them: open runs centred (first dash starts at the run's start, last ends at its end), runs shorter than one period continuous, closed runs stretched to `round(L/period)` periods from an anchor (circles from angle 0, closed PLINEGEN from the second vertex), gap-first patterns, plots keep dashes past 50,000 periods | `a303ff43`, `c319cd7a`, `6d3f6375`, `e6f2ea04`, `dc07a103`, `6c2d3798`, `3e8f468f`, `b91fa882`, `d955a3cc`; `render/src/{linetype,linetype_text,wide,draw}.rs`, `render/tests/it/{psltscale,wide_pline_linetype,linetype_text}.rs` | **PULL** (see hazard H1) | Our `linetype.rs` still starts every run with a full dash (its own comment says "AutoCAD centres"). Every dashed line, circle and linetype text position changes, so this is the most visible fidelity change; table-driven tests (33 gap-first rows). Conflicts only where `note_curve` wraps `stroke`. | M | Med: changes every dashed output; CHANGELOG "Improved"; update our expectations in tests (e.g. `dashes_split_line`). |
| B2 | Wide polylines: mitred corners with bevel at the miter limit, tapered edges meet along their own edges | `4370e61c`, `7210d1d4` (`render/src/fill.rs`, `tests`) | **PULL** | Ours leaves notches at corners of wide polylines (#3443). | S | Low |
| B3 | Locked-layer fade (LAYLOCKFADECTL), on screen only; ByBlock children inherit the reference's own transparency | `7f84dab9`, `17408bdb`, `15dcfcfb`, `16290fd8`, `b48c82c1`, `be0f2f70` (`render/src/{props,draw,blocks,pipeline,display}.rs`, `Options::on_screen`, canvas sets it) | **PULL** | We have no locked-layer fade (grep `LAYLOCKFADECTL`: none). AutoCAD users expect it. | M | Low-Med: `resolve` tuple grows; UI canvas must set `on_screen`. |
| B4 | A block reference draws its constant attribute values (ATTMODE respected; one lookup per definition) | `4f58d512` (applies cleanly to render), `a34ebd0b`, `bc5749ec` (`doc`: `TagKey`, `Insert::attribs_by_tag`) | **PULL** | Ours never draws constant `ATTDEF` text inside blocks (`blocks.rs`: "Constant/visible attribute definitions inside blocks are not drawn; attribs are"); title blocks and symbols lose text. | S-M | Low (needs the doc helper) |
| B5 | Proxy graphics of `ACAD_PROXY_ENTITY` and OLE frame outline | `0829bfa0` (applies to render), `4caf26ee`, `457627df`, `ad609fab`, `a4aa6cfa`, `6ed01dea`; `doc/src/entity/proxy.rs`, `io` readers/writers | **ADAPT** (cross-area: doc, io) | Real DWGs from vertical AutoCAD products (Architecture, Civil) carry proxies; we show nothing. Render half is small; the doc/io half is the work. | L | Med |
| B6 | DIMGAP frame (negative DIMGAP), frame over DIMTFILL, DIMPOST on angular dimensions | `800b0319`, `23d5918a`, `b17880ca` (applies cleanly), `dc59a8e3`, `39011d25` (tests) | **PULL** | Small dimension-variable fidelity fixes. | S | Low |
| B7 | Text level-of-detail regression tests (#651) | `a8c50b09` (tests only) | **SKIP** | Tests only; our `text.rs` already has the logic. Optional to copy for coverage. | S | none |
| B8 | Antialiasing, lineweights, dark/light canvas, plot-style tables, plotter configs, library of hatch patterns/linetypes | `raster.rs` (only PNG encode and `DARK_BACKGROUND` constant), `doc/src/{plotstyle,plotter,library}.rs` unchanged | **SKIP** | Unchanged upstream since base; we have the same and `DARK_BACKGROUND` already. | n/a | n/a |

### C. Plotting, page setup, export

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| C1 | `plot` takes the output format from the path extension; refuses an extension that names no format, a device/`output` that disagrees, several layouts for PNG/SVG; reply carries `format` | `62e70432`, `09b5fb93`, `c22700d3`, `12ce6688`, `0e871873`, `d6555e5b` (`engine/src/cmd/{plotters,layout}.rs`, `engine/tests/plot_format.rs`) | **PULL** (cross-area: engine) | Verified bug in ours: `run --metric --cmd 'plot {"path":"x.png"}'` writes a **PDF** (`%PDF-1.7`) named `x.png`; upstream writes a real PNG. | S-M | Low (`layout.rs` hunk context differs) |
| C2 | Model-space plots take sheet and paper units from the paper/length unit; `PageSetup.paper_mm` becomes `Option<bool>` (explicit mm or inch) | `a3e82dc9`, `1df2b829`, `8b19ba1f`, `9c2915c2`, `9a6396c0`, `272e812e`, `0b060ec9`, `cec00d45`; `render/src/paper.rs` (`metric_length_unit`, `model_page`, `apply_model_paper_units`), `io/src/pdf.rs`, `doc/src/tables.rs`, `io/src/dxf_read/objects.rs`, `engine/src/cmd/layout.rs`, `ui-egui/src/managers.rs` | **PULL** (cross-area: doc, io, engine, ui) | Verified: a drawing with `INSUNITS` 4 (mm) and `MEASUREMENT` 0 (what UNITS leaves and AutoCAD writes for many metric files) plots from model space onto **Letter** (`MediaBox 0 0 792 612`) in ours and **A4** (`841.89 x 595.28`) upstream. Matters for an Italian/European user base. | M | Med: type change of a doc field ripples through DXF/DWG read/write and the Page Setup dialog. |
| C3 | SVG export written through `craft_svg::SvgWriter` | `c07b6a2f` (`io/src/svg.rs`) | **SKIP** | Refactor; no output change. Ours is the hand-built string it replaced. Pulling it adds `craft-svg`. | n/a | n/a |
| C4 | PNG/preview/screenshot encoding through `craft-image-codecs` (`PngRows`, no second full-frame copy) | `281f86d6`, `072ec450`, `caaa0e37`, `2974d74b` | **SKIP** (maybe later for memory) | Pulls `jpeg-encoder` with an IJG attribution duty (see H4). | M | Med |
| C5 | PDF text layer: Thai sara am order, shaped-cluster characters in typed order, ligature keeps every character | `5e01c3bb`, `1fa0a8b1`, `e0987094` (applies cleanly), `fonts/src/complex.rs` | **PULL** | Correct copy/search text from PDFs for Thai/Indic; only matters for those scripts but cheap. | S | Low |
| C6 | PDFIMPORT: nonzero/even-odd fill tessellation with islands, rectangular clip, white-fill drop, read notes, ToUnicode glyph decoding, vertex budgets | upstream `io/src/pdf_read.rs` (1,064 lines) + `pdf_read/fill.rs` (603), from PR #2787 | **ADAPT** (cross-area: io) | We have our own `pdf_read.rs` (758 lines) that recovers arcs, circles and splines but ignores fill rule and clipping (`push_clip_path` is empty; `fill()` marks only the first loop `outer`). Take upstream's fill-rule tessellation (`fill.rs`, uses lyon, already a workspace dependency) and rectangular clip; keep our arc recovery. | L | Med |
| C7 | Page setup, plotter manager, plot style manager "automation roots" checks | `0f33f430`, `09e4f907`, `46ee1a81`, `2423f45e`, `b4c76bb7`, `39a2e4da` | **SKIP for now** | Depends on `file_access.rs` (engine/MCP analyst). `stylesmanager set` replying with only the changed style is small and independent: PULL then. | S | Low |

### D. Text and fonts

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| D1 | Latin accents in the single-stroke drafting font (À-Ö, Ù-ý, ÿ, Ÿ, Ç/ç, Ñ/ñ): letter body + our own mark | `0de6ec87` (`fonts/src/stroke.rs` +55 lines, `fonts/tests/latin_accents.rs`) | **PULL** | Ours has no accented glyph: `TEXT "caffè perché città"` in the default style falls back to a filled system-font outline (SVG 23,745 bytes with a `<path>`) where upstream draws strokes (6,113 bytes, no `<path>`). On the web build (no system fonts) the fallback is a missing-glyph box. Italian locale ships (`it.tsv`) so à è é ì ò ù matter. Original geometry, no asset issue. | S | Low |
| D2 | Fallback font order: Unifont last, FreeSerif before Unifont | `3b8531ff`, `75760178` (`fonts/src/{lib,ttf}.rs`) | **PULL** | CJK/Thai/Indic fallbacks drew boxes or stacked marks on NixOS-like font sets. | S | Low |
| D3 | Font discovery: XDG dirs, fontconfig `<dir>` entries (bounded `<include>`), Flatpak host fonts; each directory read once by resolved path; user font folders and Hebrew SHX list | `bbd4eddd`, `dc2acd9b`, `e0f33d7d`, `2c979eb1`, `d12c87ca`, `b121eb15`; `fonts/src/ttf/dirs.rs` (new 502 lines), `ttf.rs` | **ADAPT** | We ship .deb/.rpm/pacman packages (CHANGELOG 2.2.2) and have none of this (our `ttf.rs` is the base's). Directories only; no Autodesk path is searched (checked). Take `dirs.rs` as a unit. | M | Low-Med |
| D4 | Cap height pinned for metric-compatible substitutes (`CADCRAFT_CAP_HEIGHT`), fallback chain | `d03af7d7`, `722aeb10`, `410c55aa`, `4d1d85d2` | **ADAPT** | Rename the env var to `DRAFTCRAFT_CAP_HEIGHT` and update `docs/cli.md`. Small. | S | Low |
| D5 | MTEXT: per-column boxes (`column_bounds`), malformed `\U+`/`\M+`/unknown escape shown as typed (#2952) | `f28011ec`, `007a24ef` | **PULL** | Fidelity; the first is only useful once MTEXT background masks exist (we have none); the second is a plain bug fix. | S | Low |
| D6 | Georgian and Tamil faces embedded in the web build; Tamil interface | `93e4ff10`, `aa3d4635`, `fonts/build.rs`, `craft_fonts.rs` | **ADAPT** | Only if we add those locales; check that the craft-fonts families are not Adobe-origin (build bars `Source*`/`Noto * CJK` only). +0.5 MB wasm. | S | Low |
| D7 | Extraction of SHX reader/glyph interpreter/compiler, MTEXT lexer and escapes, font embedding classification, `.lin`/`.pat` readers into shared crates (`craft-shx`, `craft-font-catalog`, `craft-dxf::lin_pat`) | `fd11b479`, `78bcdce7`, `305f4522`, `5b170664`, `db707170`, `173cdf6e`, `a009f7d0` | **SKIP** | Pure refactors; we have the same code locally (`subset.rs` already blocks restricted-licence fonts). Adopting makes us follow storytold's pinned craft-libs revision. | n/a | n/a |

---------------------------------------------------------------------------------------------------------------------

## 3. Licensing and clean-room hazards (must not be pulled verbatim)

| # | Hazard | Where | Assessment | Clean alternative |
|---|---|---|---|---|
| H1 | **Linetype layout "measured on AutoCAD 2024 plots"** (DWG To PDF at a fixed scale, "every stroke read back") with a 33-row table of expected dash ranges | `a303ff43` and followers: `render/src/linetype.rs` module notes + tests | Behavioural observation of plots of the contributor's own drawings, which AGENTS.md allows ("observed black-box"). No AutoCAD file, font, screenshot or linetype definition is committed (patterns used are `[2,-1]` etc.). Low-medium: the provenance is a claim in a comment. | Before pulling, have the owner (who owns AutoCAD 2027, see `ATTRIBUTION.md` acad-solids rows) re-plot 6-8 of the table's cases from their own drawings and confirm; keep wording "observed" and drop any AutoCAD-internal terms. Do not commit the PDFs. |
| H2 | **Our build already embeds Adobe- and Foxit-derived data, undocumented**: `craft-pdf` with `raster` -> `hayro-interpret` (14 Foxit `.pfb` standard-14 substitute fonts, BSD-3-Clause; `CGATS001Compat` ICC, CC0; Core-14 AFM metric tables derived from Adobe's) and `hayro-cmap` (`cmaps.brotli`, Adobe CMap resources, BSD-3-Clause). `Cargo.lock` has `hayro 0.7.1`, `hayro-interpret 0.7.0`, `hayro-cmap 0.1.0`. Our `ATTRIBUTION.md`/`NOTICE` list only the four krilla ICC files. | `Cargo.lock`, `crates/io/Cargo.toml` (`craft-pdf` features `write`,`raster`), PDF underlays / PDFIMPORT | AGENTS.md says no Adobe fonts "ever". CMaps and AFM metrics are encoding/metric tables (BSD-3 resources Adobe publishes), the Foxit fonts are Foxit's, not Adobe's, but the policy text does not carve this out and BSD-3 requires notice in binary distributions. Upstream records these rows with SHA-256 in `ATTRIBUTION.md` "PDF import" and `NOTICE`. | Add the rows (copy upstream's "PDF import" table, hashes included) and the BSD-3 notices to `ATTRIBUTION.md` and `NOTICE`; add one sentence to AGENTS.md saying dependency-embedded BSD-3 CMap/metric data is accepted, or (if the owner refuses) gate PDF underlays off. `cargo xtask assets` should list dependency-embedded files the way it does the krilla ICCs. |
| H3 | Upstream dependency on `craft-shx` etc. | Cargo.toml | `craft-shx` is "extracted from CADCraft", no font assets, signature strings `AutoCAD-86 shapes 1.0` are required by the file format. Upstream searches only user folders, `*_SHXPATH`, fontconfig dirs: grep finds no Autodesk install path in `crates/fonts`. No hazard. | n/a |
| H4 | `craft-image-codecs` brings `jpeg-encoder 0.6.1` ((MIT OR Apache-2.0) AND IJG) | upstream `NOTICE` +310 lines, `licenses/jpeg-encoder-0.6.1/`, `ATTRIBUTION.md` | Not a clean-room problem, but IJG requires an acknowledgement in documentation and the text in `NOTICE`. | If we ever adopt it, copy the `licenses/` folder, the NOTICE block and the ATTRIBUTION row; otherwise SKIP (C4). |
| H5 | Georgian/Tamil/Thai faces come from the optional `craft-fonts` input | `fonts/build.rs` | `BARRED_FAMILIES` only lists Adobe `Source*` and `Noto * CJK`. Adding `Geor`/`Taml` to the web build embeds whatever the manifest lists. | Check the craft-fonts manifest rows for those scripts (expected Noto Georgian/Tamil, OFL) before enabling; keep the barred list test in `craft_fonts.rs`. |
| H6 | Upstream commit text citing AutoCAD behaviour ("AutoCAD 2024, measured") is widespread outside my area (layer states, JOIN, BREAK) | many engine commits | Same black-box rule; no assets. For render/fonts only H1 applies. | Same as H1: keep as observation, never paste Autodesk help text (upstream links to `help.autodesk.com` pages in a few comments; links only, no copied text). |

No upstream commit in `crates/render`, `crates/fonts`, `doc/src/library*` adds a `.shx`, `.pat`, `.lin`, `.ctb`, font, icon or
image file (checked with `git diff --name-status 2569dbd5 cadcraft/main`: the only added non-code files are nightly app
icons, packaging scripts, i18n `.tsv` for pl/ka/ta, `jpeg-encoder` licences and DXF-dump test text files for dynamic blocks).
`doc/src/plotstyle.rs`, `plotter.rs`, `library.rs` are byte-identical to the base upstream, so hatch patterns/linetypes remain
"our own definitions".

---------------------------------------------------------------------------------------------------------------------

## 4. Where we are ahead, where behind

**Ahead (evidence):**
1. **PDF underlays** (PDFATTACH, ATTACH of a PDF, rasterised with craft-pdf `raster`, kept in `Drawing::rasters`, not in the file): `crates/engine/src/cmd/underlay.rs`, `crates/io/src/{pdf_read,dxf_underlay}.rs`, `crates/engine/tests/pdf_underlay.rs`. Upstream has no underlay code at all (`git grep -i underlay cadcraft/main -- crates` finds only an exchange-command string and locale rows).
2. **Vector curves in plotted PDFs**: `DisplayList::curves` (`DCurve`, Bézier paths, `render/src/{display,lib,viewports}.rs`) and `io/src/pdf.rs::curve`. Test: two circles + an ellipse -> ours 1,277 bytes with 12 `c` operators, upstream 2,986 bytes with 260 `l` operators (`scratchpad/rn/shp-*.pdf`). Upstream plots every arc as a polygon.
3. **Sheet Set Manager** (`engine/src/cmd/sheetset.rs`, `.dcss` JSON, publish one PDF per sheet). Upstream: none (only locale strings).
4. **PDFIMPORT arc/circle/spline recovery** (`ImportedShape`/`ShapeClass`, cubic -> arc detection `cubic_arc`, exact Bézier splines). Upstream flattens non-arc curves.
5. **Picking by drawn geometry and text boxes** for dimensions, multileaders, blocks, text (`hit_polylines`, `annotation_text_boxes`, `reach` so the last letter picks). Upstream hits dimensions and multileaders on their lines plus the outline of their bounds box (no text boxes); it is catching up for blocks (`15d2a63b`, `d71f2b95`, `afb53d53`, `94b11748`, #3445).
6. **Diameter sign in TrueType text** (`look_alikes`: ⌀ -> Ø -> ∅, `fonts/tests/diameter_sign.rs`, 2.2.1); upstream has no equivalent.
7. Display list build is ~5-20% faster at 4 threads and equal at 1 thread (noisy; no shared atomic budget yet; see section 1).
8. Plot styles, plotter configs, page setup, sheet/viewport rendering: **parity** (identical code); both use their own JSON plot-style format, neither reads real `.ctb/.stb` binaries.

**Behind:**
1. Display-list memory abort on a 2 KB DXF (A2), quadratic layer lookup (A1, ~200x), unbounded SHX glyph cache (A5): the three "never crash/never hang" gaps.
2. Plot output: format by extension (C1), metric model-space sheet and units (C2).
3. Accented letters in the stroke font (D1); font discovery on Linux (D3).
4. Fidelity: dash layout (B1), wide-polyline corners (B2), locked-layer fade (B3), constant attributes (B4), proxy graphics (B5).
5. PDFIMPORT fill rules and clip (C6).

---------------------------------------------------------------------------------------------------------------------

## 5. Suggested order and prerequisites

1. Ours only, S: fence leg prefilter + `reach` DimStyle clone (section 1). Add ATTRIBUTION/NOTICE rows (H2).
2. S pulls with no cross-area prerequisites: D1, A5, A6 hunks, A7, A8, A9, B2, B6, C5, D2.
3. A1 (doc name index) then A2 (with geom `7804e94e` first, then hatch `2410a1fd`/`a1574780`, then `80c6e074`, `fba309f8`) with per-thread counter batching.
4. C1, then C2 (doc `paper_mm` type change; coordinate with io/doc analysts).
5. B1 (after H1 re-measurement), B3, B4, B5.
6. D3, D4, C6 as time allows. Revisit A10/C7 when the roots framework is adopted.

Each pull needs: CHANGELOG line under `## Unreleased` (user-visible items), regression test (AGENTS.md "Prove it"), the `cadcraft`->`draftcraft`
rename, `cargo xtask ci`, and a PR (never direct to main).

## 6. Reproduction commands (all read-only)

- Fence profile: `valgrind --tool=callgrind --toggle-collect='draftcraft_engine::select::select_fence' draftcraft-cli perf 20000` (upstream: `cadcraft_engine::select::select_fence_in`); `callgrind_annotate --inclusive=yes`.
- Candidate counts: `python3 -I scratchpad/rn/fence_est.py 1000000`.
- Layers: `python3 -I scratchpad/rn/mk3.py 30000; <cli> perf layers30000.dxf`.
- Memory abort: `python3 -I scratchpad/rn/mk2.py; (ulimit -v 6000000; <cli> convert bomb2.dxf out.png)`.
- Plot format: `<cli> run --metric --script 'LINE 0,0 100,100\n' --cmd 'plot {"path":".../x.png"}'` then inspect magic bytes.
- Plot sheet: `<cli> run --script 'MEASUREMENT\n0\nINSUNITS\n4\nLINE 0,0 100,100\n' --cmd 'plot {"path":".../x.pdf"}'` then read `/MediaBox`.
- PDF curves: `<cli> run --script 'CIRCLE 0,0 50\nCIRCLE 20,20 30\nELLIPSE 0,0 60,0 0.4\n' --export x.pdf`, count `c`/`l` operators after inflating.
