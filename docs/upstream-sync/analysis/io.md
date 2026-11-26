# File I/O: DraftCraft vs upstream CADCraft

Read-only analysis. Nothing in the repo was edited or built. Tests were run against the two existing release binaries.

Base: fork base 1b39fbba / fc157ea9 (2026-11-05). Our crates/io DXF files are line-identical to upstream at that point, apart from our renames and about 517 lines of our own changes. 220 non-merge commits have touched crates/io, crates/dxf and crates/dwg upstream since (list in `up-io-log.txt`, a per-commit "is it in our tree" check in `presence.txt`). Nearly all of them are absent from our tree.

Upstream's DXF tag layer moved to `craft-dxf` in storytold/craft-libs on 2026-11-11 (c736e488). At the base our `crates/dxf` equals upstream's old `crates/dxf` (diff = 2 small additions), so craft-libs patches port almost textually.

Evidence is in the scratchpad: `perfrun/` (callgrind profiles, synthetic DXF/DWG), `hostile/` (crafted inputs), `tools/` (harness scripts), `fuzz/`, `analysis/hostile-*.txt`, `analysis/fuzz-dwg-1500.txt`.

---

## 1. Why our DXF read is slower than upstream's

Measured with the release binaries.

| Measurement | Ours | Upstream |
|---|---|---|
| `perf 1000000`, DXF read | 12.1 s | 10.5 s |
| 100k-entity, 21.7 MB DXF, `info`, wall time | 0.80 s | 0.69 s |
| Same file, peak RSS | 414 MB | 263 MB |
| Callgrind, 15k entities, total instructions | 635 M | 547 M |
| Callgrind, `read` only | 567 M | 485 M |

The 82 M-instruction gap in `read` (about 15%) breaks down as follows.

- **`draftcraft_dxf::sections()` deep-copies every tag.** 29.4 M instructions, against 2.1 M for upstream's `section_slices`.
- **`draftcraft_dxf::records()` clones every tag of every record into its own `Vec`.** 45.6 M instructions, against about 23 M for upstream's `record_slices`. malloc/free accounts for most of it (31.7 M in `free` against 13.5 M).
- **Three copies of the tag list.** Parse, then sections, then records. That explains the 57% higher RSS.
- **`unescape_unicode(&s)` always allocates a copy of every string tag.** About 35 M, +7.6 M net in `parse`. Upstream's `unescape_owned` moves the `String` through when it has no backslash.
- **The remaining difference is noise-level.** `place_entities` is +6.6 M.

Tokenizer, `count_pairs` and the entity mapping are otherwise identical. The tokenizer and the base pass are not the problem.

**Portable (S, low risk).**
- Upstream commit 74aed007 (io: sections and records as views) changes 8 files, 43 lines.
- It needs `section_slices`/`record_slices`/`SectionRef` from craft-libs ebb40fa0c, about 40 lines in `crates/dxf`.
- `unescape_owned` and `Tag::str_ref` come from craft-libs ebb40fa0c, which ports cadcraft#1833.
- Expected result: about −78 M instructions, which would bring us level with upstream. Peak RSS should drop about 35%.

**Faster than upstream.**
- Our corpus open time is 0.27 s against 0.60 s.
  - DWG open: 0.113 s against 0.43 s for 30 files.
  - DXF open: 0.153 s against 0.166 s.
- A 100k-entity DWG reads in 0.61 s against 1.4–1.6 s, and uses 226 MB against 302 MB.
- DXF write is on par: 5.5 s against 5.9 s at 1M entities. craft-dxf's faster writer (64a8c7f74) gives no visible gain, so skip it.

**Hot spots both share.**
- `dxf_raw::read` is 16–19% of the read (102 M of 567 M). It runs `head()` twice per entity, with a `norm()` string allocation per handle. Caching or avoiding those strings is a cheap improvement beyond upstream (S–M).
- `Tag::str()` clones a String on every call.

**DWG write is about 2x slower than upstream (4.4 s against 2.0 s for 100k).** The cause is not the writer.
- In a 15k-entity callgrind run, the thumbnail preview (`dwg::write::preview` → tiny-skia raster) is 54% of `convert`.
- The encoder is 14%, and the verify read-back is 8%.
- A cheaper preview is the quick win (S). The verify read-back is by design.

---

## 2. Why dwg→dxf "loses" about 3.5x more

Mostly a comparator artefact, plus a shared real gap. Some real losses are specific to ours.

Numbers from the two `report.json` files, over the 30 DWG files, dwg→dxf route:

| Category | Ours | Upstream |
|---|---|---|
| header lost | 3901 (154 distinct names) | 1104 (40 names) |
| object lost | 40 | 2 |
| class lost | 11 | 0 |
| extDict lost | 1 | 1 |

The header figure is 97% of our total.

- **What `round_trip` compares.** `compare(d, reopened)` compares the in-memory Drawing against the reopened one. It does not compare file census against file census.
- **Why our count is higher.** Our direct reader puts every header variable the DWG holds into `d.header`: all 197, as docs/dwg.md says. Upstream goes DWG → acadrust → DXF text → `d`, so `d.header` holds only what acadrust emitted. A variable that was never read cannot show up as lost.
- **Both DXF writers use a ~50-name whitelist.**
  - `HEADER_VARS` plus the DIM* vars come to about 115 variables written in total. AutoCAD's own DXF has about 250.
  - On AutoCAD's mi201 DWG→DXF, ours writes 112–117 header vars against upstream's 116. Both match AutoCAD's own DXF except for the same ~135 variables.
  - dxf→dxf loses 3973 (ours) against 3902 (upstream), with 135 shared names.
- **What the lost values are.** About 95% are defaults ($USERI1..5, $USERR1..5, $PUCS*, $MENU, $TILEMODE 1, $SPLINESEGS 8 and so on).
- **Real information loss on DXF save, shared with upstream.** Verified with a crafted header (`hostile/hdr_test.dxf`): $USERI1=42, $USERR1, $HYPERLINKBASE, $ISOLINES=12, $FACETRES, $PLIMMAX, $LIMCHECK, $TEXTQLTY, $SPLINESEGS, $LASTSAVEDBY, $TILEMODE and $PELLIPSE are all dropped by both tools.
- **Ours also loses the model-space UCS ($UCSNAME/ORG/XDIR/YDIR) in dxf→dxf.** Upstream keeps it (99b7dc80, be56597e, d7de5ccd).
- **Header values are mutated on DWG save.** Our DWG writer takes its header from the same DXF whitelist and fills missing variables with constants. dwg→dwg changes $ISOLINES 4→8 in 21 files, plus $PSOLHEIGHT, $PSOLWIDTH, $SKETCHINC and $DIMALTTD. Real bug, S–M.
  - Fix: one generic pass writing every `d.header` var with a code table. The DWG writer already has the table in `compose/sections.rs::header_variables()`.
- **Real object losses on ours.**
  - ACAD_EVALUATION_GRAPH: 38 objects plus the class, in 11 3D solids DWGs. These belong to solid history, which we do not read.
  - ACDSDATA in 1 file, an ACAD_TC xrecord, and an ACAD_TC extension dictionary.
- **Unread objects.**
  - We report "couldn't be read" on 19 files (ACSH_*, ACDBASSOC*, ACDBPERSSUBENTMANAGER, MESH ×2, 3DSOLID in 2004/2007/2010 binary SAB, VISUALSTYLE/MATERIAL in the R14 file, surfaces).
  - Upstream reports 1 file. acadrust drops equivalents silently.
  - Per-file entity counts agree exactly for all 68 files, except 13 `Proxy` entities upstream keeps raw (MESH ×2, 3DSOLID ×9, surfaces ×5 by file listing). Everything else we don't read is 3D, so 3D branch territory.
  - Our note is the more honest one.
- **Real 2D bug in the DWG reader.** "entity plot style ×54" in the two mi201-r2018-from-dxf DWGs: `plotstyle_flags==3` handle is read but not mapped (`dwg/entities.rs:123`). The DXF reader maps it via ACAD_PLOTSTYLENAME. S–M.
- **Fair summary.** For AutoCAD's mi201 DWGs, both tools produce DXF with the same record structure as AutoCAD's own DXF (differences are CELLSTYLEMAP, one DICTIONARY, one XRECORD).

---

## 3. Upstream I/O changes since base: verdicts

Verdict key: PULL = cherry-pick (clean port), ADAPT = port the idea or the I/O side onto our model, SKIP = not applicable. "Verified" means I reproduced the gap with the two binaries.

### 3a. Verified gaps in our tree

| Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|
| Solid or gradient HATCH writes groups 52/41/77; AutoCAD 2024 refuses the DXF ("Invalid or incomplete DXF input -- drawing discarded") | 467ab20a, `dxf_write/entity.rs`. Verified: our SOLID hatch has 52/41/77, upstream's has none | **PULL** | AutoCAD will not open any DXF we save that has a solid fill. Upstream measured it in AutoCAD 2024 | S | Low |
| Quadratic open with many root-dictionary objects | d6d16bd4. Verified: 15k 0.9 s, 30k 3.9 s, 60k 16.1 s, against 0.7 s upstream. Three passes: foreign-object key lookup, annotation scales per layout, CLASS count insertion | **PULL** | A 3 MB file hangs the app for minutes | S | Low |
| Entities silently dropped on duplicate or top-of-range handles | 44f02683, bcac20dd, 140437e7, 528e2698, a516ce11, 5eafcc93, 077c054d, 8ec1e9a0, e4f9b6aa, 8ab61bce, b1844542, 1dcb3042, aaaf6548 (#1276, #913). Verified: 2 of 4 lines kept across two ENTITIES sections with the same handles, 1 of 3 with $HANDSEED=FFFF…, no read note | **ADAPT** | Data loss with no warning. Touches doc `EntityStore`/`new_handle` | M | Med |
| Crafted 400k-element LTYPE | 7867366a. Verified: PNG export 8.7 s against 0.19 s | **PULL** | Render-time DoS | S | Low |
| Sections and records copied | 74aed007 + craft-dxf `section_slices`/`record_slices` | **PULL** | See section 1 | S | Low |
| String tags copied again | craft-libs ebb40fa0c (`unescape_owned`) | **PULL** | See section 1 | S | Low |
| MINSERT written as AcDbBlockReference (BricsCAD draws one copy) | 8e698902. Verified | **PULL** | Interop | S | Low |
| Mirrored objects (normal 0,0,−1) move on DXF save | 275f989a (+48bcb817, 6ec0f240). Verified: our circle thickness +2 (world flipped), LWPOLYLINE elevation +5 (should be −5) | **PULL** | Wrong geometry on save | S–M | Low |
| 2D LWPOLYLINE/HATCH/POLYLINE in a tilted plane read in raw OCS x,y | d19df49f, 756946c4, a49e4ff9, 90806a74, 9735b232, 41f9e394, ad596f1e, f4d4a6e4. Verified: normal (0,.6,.8) gives extents [0,0..30,10] ours against [−30,−8..0,0] upstream | **ADAPT** | Plan view differs from AutoCAD's | M | Med |
| SVG import: shape with 1e308 coordinates kept as ELLIPSE with ~1e307 numbers; sheared ellipse axis wrong | 50644fb0, 063cbef2, 51f38b4e, f24da7a8, 6a240a70, 16e0fa52, b1a34dc0, 6f862d7b, 766af02e. Verified. (Upstream later moved to craft-svg, c07b6a2f; these pre-date it and apply to our `svg_read.rs`) | **PULL** | Never-crash hygiene and correctness | S–M | Low |
| ASCII DXF with CR-only line ends fails ("bad group code") | craft-libs 04307380 (also: NaN/inf binary reals, integer clamping, hex normalisation, 254-digit hex split, trailing spaces on EOF, `\r` in codepage scan) | **ADAPT** | Port the lib.rs and codepage.rs diffs. The API is the same | S–M | Low |
| Binary hex chunks written up to 255 bytes; AutoCAD limit is 127 | craft-libs 5056c13ef (#1147) | **PULL** | Interop | S | Low |
| Binary DXF OOM abort; R12 1-byte-code binary; `parse_recovering` (open a damaged ASCII DXF up to the damage) | craft-libs e947d5d79, ebb40fa0c, 64a8c7f74 | **ADAPT** | Recovering a truncated file is a user feature | S–M | Low |
| 28 MB `\U+` string costs 178 MB (Vec<char>) | craft-libs `unescape_found` windowing | **PULL** | Allocation bound | S | Low |
| Hatch: `boundary_flags` kept so islands stay holes elsewhere; elliptic edge dedupe; points budget | a4d8cae5, 008d538e, 8d78cd09 | **ADAPT** | Doc model field plus writer | S–M | Low |
| Header not persisted: UCS, VIEWCTR/VIEWSIZE (open at the saved view), CTABLESTYLE; VPORT UCS groups 110–146 dropped | 9244ffda, 99b7dc80, be56597e, d7de5ccd, 026f7d90, d75bc7fa, 10db46d5, a06bb18b. Verified with AutoCAD mi201 DXF | **ADAPT** | I/O half is portable; the UCS command is engine work and we have no `ucs` command | M | Med |
| Layout/plot settings: LAYOUT groups 76/77/78 (shade plot) dropped; paper units; inherited units follow MEASUREMENT; model flag 1024; saved flags | 94d9e354, 371baf17, 3ee12a8c, dab89a38, 42503a8e, f84e458c, d4abea0d, 4ac1a6a0, 1df2b829, 8b19ba1f, ff0e4173, 661d0776, 0611bad3, a3e82dc9 | **ADAPT** | One set; port as a unit | M | Med |
| Layer states in AutoCAD's ACAD_LAYERSTATES form (we write only our DRAFTCRAFT_LAYERSTATES record, so AutoCAD lists none) | 9ed3607b, cd9f8b1d, b9d74f8f, bcb6d6ac, f474a27f, 45595d58; new `dxf_layer_states.rs` (227 lines) | **ADAPT** | AutoCAD interop. Engine side in `cmd/layer.rs` | M | Med |
| Dimension variables set with SETVAR saved; DIMASSOC kept; kept dimension *D block drawn and kept | 9aff1d6a, 52dfb93b, 392b0e16, f297806c, 3ac368c0, a4b15ec5, 29a22f79, 3165d273, d04d67f8 | **ADAPT** | Fidelity for dimensions from other programs. Render and doc work too | M–L | Med |

### 3b. Not verified, but worth adopting

| Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|
| Associative/kept-object bookkeeping | 10a493d2, 99a83201, 9f245800, 1363afb8, adf02f9f, baf24241, b325046f, 8e698902 | ADAPT | Incremental fixes on `dxf_raw.rs`. Also fixes dangling objects | M | Med |
| Proxy entities: decode and draw proxy graphics (ODA public spec, 1411-line `doc/entity/proxy.rs`); keep layers | 0829bfa0, a4aa6cfa, f33fa1ee, 72f0dddc, 80470c03, 73d1b65f | **ADAPT** | Custom-object drawings (Civil 3D, Architecture) show as blank for us. The decoder is from a public spec, so clean-room compatible | L | Med |
| Kept render settings, LAYER ext-dict xdata, per-IMAGE IMAGEDEF, sheet resolution, viewport id range | a801739e, 45595d58, 7189295b, 474df8ed, d6cc051d, 84ba9a80, 2872c051 | PULL as a batch | Small robustness fixes | S each | Low |
| PDF import clip, fill and text fixes; password | d4fd0dd7, 186e4654, fc1b63a7, 2236c148, 6a98857f, 2873092e, 3655ded6, 958ce153, a248c59f, 2aa669fa | SKIP / ADAPT low priority | Our `pdf_read.rs` is an independent implementation (not a descendant). Both import our test PDFs with the same entity counts. Ours has no clipping, no password. Port individual ideas only | M | Low |
| OLE2FRAME drawn and moved | 4caf26ee, 87f3c087, afb24ee6, 457627df | ADAPT | Engine plus io | S–M | Low |
| Other-OS and craft-libs refactors (shared raster codecs, craft-svg) | 2974d74b, caaa0e37, c07b6a2f, 281f86d6 | SKIP | No function gained; our `image` crate and `svg_read` work. Revisit if we adopt craft-svg for its SVG guards | — | — |

### 3c. DWG-bridge / acadrust-only: SKIP

We do not use acadrust, so none of these apply. Our DWG read and write are native.

2a9fc8f2, 54dfb2ee, 1a322faa, cef9ee4b, 33019677, 6ae7c156, bf137e2c, 0722afc5, d2052eb4, 31935157, 946af874, 3f9e45f7, a0dfe07f, 3c956c4c, 40ddd7ba, 5147aae2, fa4f997e, b551acec, cc74fe32, 6c5fdfdd, 2dc13678, and the tests that "opt in to the in-process bridge" (318d8d9d, 5d76e473, 3caad662, db66a8cd, 8152c9e7, d02dc3e9, d71fb54b, ec09c87c, 9ec57281).

### 3d. Outside my area

- Dynamic-block commits (fc82a2f9 … 3f805561) and the UCS/engine commits (bc7357ff, 821b8ad1, e3aeff2a) are doc/engine work.
- The `ucs*`, `qleader`, `extrim`, `blocklib*` and `dimstyle.overrides` commands are in upstream only.
- We have commands upstream lacks: `pdfattach`, `pdfclip`, `imageclip`, `etransmit`, `laytrans`, `attout`/`attin` and others.

---

## 4. DWG codecs compared

| Aspect | Ours (crates/dwg + io/src/dwg) | Upstream (craft-dwg over acadrust 0.6.6) |
|---|---|---|
| Architecture | Own reader decoding each object into the Drawing, own writer from the same records as the DXF writer. No DXF in the middle | DWG → acadrust → DXF text → our reader; save is R2000-form DXF → acadrust → DWG |
| Read versions | R13/R14 through 2018+ (AC1012–AC1032), including 2007 Reed-Solomon. Our corpus's R14 file reads | Same range by acadrust. R13 and R14 "untested", per upstream's own `docs/file-format-parity.md` |
| Write versions | 2000, 2004, 2007, 2010, 2013, 2018 by our own encoder | The same six codes; default at least 2004. The README line "write R2000" is stale |
| Size | About 34k lines (20.6k codec + 13.5k mapping) | craft-dwg 3.4k lines plus acadrust 224k lines (third-party) |
| WASM | Yes. Reads and writes DWG in the browser build | No. The bridge is not built for wasm; the web build says "save as DXF" |
| Hostile input | In-process, `Limits` 512 MB, every count bounded, byte-flip tests. 1500 fuzz mutants: 0 crashes, 0 hangs over 10 s. Rejects 953 | Child process with a 30 min timeout and no memory limit. The same 1500 mutants: 0 crashes. Rejects 731, i.e. more lenient |
| Speed | 100k entities: read 0.61 s, 226 MB. Write 4.4 s (preview render dominates) | read 1.4–1.6 s, 302 MB. Write 2.0 s |
| Cross-compatibility | Reads upstream-written DWGs (100000 entities, 0 notes) | Reads ours the same way |
| Fidelity evidence | Object-for-object bytes equal to AutoCAD's own DWG for tested types in all six versions. mi201 AutoCAD files from R14 to 2018 and their DXFs in the repo. Opened in AutoCAD 2027 with AUDIT 0 (our docs; I cannot verify) | Tests are synthetic. Parity doc estimates DWG ≈ 30% read, ≈ 15% write. Drops table xdata and custom-class objects on save |
| Fixtures | 66 real AutoCAD drawings, with ATTRIBUTION rows | None in-repo (a separate corpus repo) |
| Tests | 126 (dwg) + part of 821 (io) | 42 (craft-dwg) + part of 1117 (io) |
| Unknown or custom objects | Dropped with a read note (ACSH, ACDBASSOC, MESH, binary SAB, proxies) | Kept as raw records or `Proxy` (13 entities in our corpus), but lost on save when written at another version |
| Third-party code | None. Derived from BlackCAD (same owner, MIT OR Apache) and ODA's public spec | acadrust (MPL-2.0) |

**Ahead (ours):** native, wasm, 2.3–2.8x faster read with lower RSS, AutoCAD-verified byte fidelity, real fixtures, no third-party codec, honest "unread" accounting, ours writes DICTIONARYVAR variables that upstream does not.

**Behind (ours):** 3D history and associative objects (ACSH_*, ACDBASSOC*), MESH, binary SAB solids, proxy entities (no drawing of proxy graphics), strictness on damaged files (rejects a whole file when one compressed page fails; no skip-and-report mode), DWG write cost (preview render), entity plot style mapping, header variable round trip.

---

## 5. Hostile input and never-crash

Same two binaries, same inputs.

- **DXF fuzz:** 540 mutated DXFs (numbers, codes, drops, duplicates, huge counts, truncation). Identical outcomes on both (277 ok, 263 error), no crash or timeout.
- **DWG fuzz:** 1500 mutated DWGs (flips, truncation, 4-byte blasts, zeroing). No crash or hang on either.
- **Crafted files (`hostile/`):** declared counts of 2e9 (hatch edges, LWPOLYLINE, SPLINE, TABLE), recursive and exponential blocks, MINSERT 30000x30000, NaN/inf text, binary NaN, 50 MB unterminated binary strings, 90 MB of empty binary tags, 400k-vertex POLYLINE, 100k layers, 250k xdata groups, 900k attributes. None crashes or runs away on either side.

**Real gaps in ours:**

| Gap | Upstream fix | Priority |
|---|---|---|
| Quadratic root-dictionary open (16 s at 60k, 4x per doubling) | d6d16bd4 | High |
| Silent entity loss on duplicate or top-of-range handles | the #1276 series | High |
| 400k-element LTYPE: 8.7 s PNG export | 7867366a | Med |
| SVG: 1e308 shape accepted | 50644fb0 and the others | Med |
| CR-only DXF rejected | craft-libs 04307380 | Low |
| String escapes use 4 bytes/char: 178 MB for a 28 MB string | craft-libs `unescape_found` | Low |
| Binary-DXF tag vec grows with `push`; a 500 MB file of 3-byte tags can reach ~4 GB+ before the 100M-tag limit | craft-libs e947d5d79 | Low |
| Superlinear named UCS/VIEW/DIMSTYLE: 40k records take 6.7 s, 7.7 s and 12.3 s (upstream 3.6, 3.8 and 9.2) | Not fixed upstream | Low, shared |
| Hex values not sanitised in the ASCII writer | 04307380 | Low |
| 4 other upstream fixes (budgets for hatch curves, ids out of range, etc.) | 8d78cd09, 2872c051 | Low |

---

## 6. Licence flags

- **acadrust 0.6.6 is MPL-2.0** (Cargo.toml `license = "MPL-2.0"`, author Hakan AK; also stated in craft-libs `NOTICE` and `licenses/dwg/`). It is weak, file-level copyleft; used unmodified as a dependency it does not infect the MIT/Apache app.
  - Its `THIRD_PARTY_NOTICES` says the DWG code was ported from ACadSharp (MIT).
  - It also says LibreDWG (GPL) "is used as a secondary cross-check for selected field names and layouts", and the Reed-Solomon tests cite numeric values from LibreDWG's `reedsolomon.c`, with a claim that no routines were ported.
  - Our AGENTS.md forbids even reading GPL sources. Adopting craft-dwg would import that provenance risk and contradict our "our own code from the public spec" statement. **Do not adopt craft-dwg or acadrust.**
- **craft-libs (craft-dxf, craft-dwg bridge, craft-svg):** MIT OR Apache-2.0, Copyright "ArtCraft Team and the craft-libs contributors". Porting code is fine; keep the notice (our NOTICE already credits "ArtCraft Team and the CADCraft contributors").
- **Our own code:** no GPL/LGPL/AGPL references in `crates/` (grep for LibreDWG/QCAD/LibreCAD finds none). The docs say acadrust was consulted "for layout facts, each confirmed on a file; no code copied" and was used as a test oracle in the past. Several code comments say "as acadrust reads and writes it" (`io/src/dwg/entities.rs:602`, `dxf_read/entities.rs:426`, the LEADER annotation size, multiline style linetype index, diameter dimension point order, hatch elliptic angles). These are behaviours not confirmed against an AutoCAD-saved file. Low risk, but re-verify against AutoCAD files.
- **Fixtures:** the 50 `acad-solids` drawings are attributed as "drawn in AutoCAD 2027 for Mac (educational licence)". Educational licences commonly restrict commercial use of output. Worth a maintainer check that redistributing them under MIT/Apache is allowed.
- **BlackCAD provenance:** the codec comes from the owner's own repo (MIT OR Apache); I could not verify its clean-room history beyond the docs.

---

## 7. Ranked action list

1. PULL 467ab20a: solid hatch groups (AutoCAD refuses our DXF). S.
2. PULL d6d16bd4: quadratic open. S.
3. PULL 74aed007 plus craft-dxf slices and `unescape_owned`: closes the read gap and cuts about 35% peak RSS. S.
4. ADAPT the handle series: silent entity loss. M.
5. PULL 7867366a, SVG overflow/ellipse fixes, MINSERT marker, mirrored-object writer, 127-byte binary hex chunks, CR-only lines. S each.
6. ADAPT a generic header write (our own improvement), plus UCS/view/CTABLESTYLE. M. Fixes the dwg→dxf and dwg→dwg header "losses" at once.
7. ADAPT layout/plot settings, AutoCAD layer states, hatch boundary flags, tilted-plane reading. M each.
8. Map the DWG entity plot style. S–M.
9. Cheaper DWG preview. S.
10. Proxy graphics decoder. L.
11. Add a recover mode (skip an undecodable page and report). M.
