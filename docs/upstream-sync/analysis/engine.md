# Upstream CADCraft vs DraftCraft: engine, commands, document model, geometry, constraints, colour

Scope: `crates/engine`, `crates/doc`, `crates/geom`, `crates/constraints`, `crates/color`. File I/O, rendering and UI are other analysts' areas; where an item crosses into them it is marked "cross-area".
Nothing in the repo was edited, committed or built. The only things run were the two prebuilt CLIs and git. Compile compatibility of any port is therefore UNVERIFIED.

Working files (all under `/tmp/claude-0/-home-user-DraftCraft/792b8165-94d2-5589-bbfd-8d15bce2a63e/scratchpad/analysis/`):
- `engine-commits.tsv`: all 391 upstream commits in the area since the base, with theme, whether the patch applies to our HEAD, and subject.
- `triage.out` (does the area part of each commit apply to HEAD), `mergesim.out` (per-commit `git merge-tree`), `fullmerge-hunks.txt` (whole-branch merge conflict hunks), `presence.out` (are a commit's added lines already in our tree), `patches/` (area part of every commit, with cadcraft renamed to draftcraft).
- `sim/` is a throwaway `--shared` clone used for `git merge-tree` so the real repo was not touched. `proof/` holds the test DXFs used for the A/B runs.

---

## 1. Fork base: better than 2569dbd5

Method: diff the area's `src/` trees of our import commit (77c95f86) against every upstream first-parent commit from 2026-10-30 to 2026-11-12 and count changed lines. Every file also differs by the cadcraft to draftcraft rename, which is a constant offset.

| upstream commit | date (-0700) | changed lines vs our import |
|---|---|---|
| 2569dbd5 (your estimate) | 11-04 20:20 | 2,866 |
| df659744 / fc157ea9 / 2611bc5f / 1b39fbba | 11-05 14:17 to 16:31 | 2,689 (minimum, a plateau) |
| 8e899eef (#2837 stack/s1) | 11-05 19:16 | 3,128 |

So the base is one of df659744, fc157ea9, 2611bc5f, 1b39fbba. They are identical for this area, so I use **1b39fbba** (merge of #2835, 2026-11-05 16:31). The #2831 (land/1031) changes are already in our tree. Stack s1 (#2837) and everything after is not.

- Commits after it: 888 non-merge, of which **391 touch this area** (`git log 1b39fbba..cadcraft/main --no-merges -- crates/engine crates/doc crates/geom crates/constraints crates/color`).
- Net change in the area: 272 files, +37.3k / -2.4k lines.
- 228 of the 391 touch only this area; 80 also touch io, 68 ui-egui, 19 render, 29 apps/mcp/dwg/dxf.
- A `--since=2026-11-04` date filter is the wrong tool: in this area it lists 263 commits, 5 of which are already in our base, and it **misses 133 commits** that carry older author dates but were merged after the base (the whole UCS series, most dynamic-block work, OLE2FRAME, the automation roots, BREAK/TRIM keep-handle). Use `1b39fbba..cadcraft/main`.
- Our tree already contains essentially none of the 391. Only 8 have 40% or more of their added lines present, all trivial. Our divergence from the base in this area is our own features plus the rename: geom +557/-6 (`bezier.rs`), constraints and color rename only, doc +931/-102, engine +9.7k/-2.2k.
- Our LISP is the base verbatim (12 non-rename differing lines), so LISP patches port cleanly.

### Whole-branch merge simulation (area only)
`git merge-tree --write-tree --merge-base=1b39fbba HEAD cadcraft/main` gives **105 conflict hunks in 62 files** in this area. The conflicts are concentrated in:
- `cmd/file.rs` (12 hunks) and `select.rs` (9), both of which we also changed.
- `lisp/drawing.rs`, `cmd/props.rs`, `cmd/layer.rs` (4 each).
- `snap.rs`, `lisp.rs` (3 each).
- `doc/lib.rs`, `dynblock/apply.rs` (2 each), `doc/extents.rs` (1).

Whole-repo, 267 files conflict, mostly io, ui, packaging and docs, which belong to the other analysts. Per-commit, 79 of 391 apply to HEAD on their own (Appendix A). The other 312 depend on earlier commits of their stack, so they should go in order, not as isolated cherry-picks.

---

## 2. A/B evidence (release binaries, run here)

| test | draftcraft-cli 2.2.3 | cadcraft-cli (upstream main) |
|---|---|---|
| DXF whose line and `$HANDSEED` are `FFFFFFFFFFFFFFFF`; add two LINEs; save; count | **1 entity (silent data loss)**. A normal handle gives 3. | 3 entities |
| 60,000 layers + 60,000 INSERTs of missing blocks, `info` | **32.9 s** (20k: 2.3 s, so quadratic) | 1.3 s (20k: 0.4 s) |
| `layer.set locked`, then `properties.set {color:1}` on an object on it | `{"changed":1}`, edits the locked object (also lets it be moved off the layer) | `{"changed":0,"locked":1,"message":"1 object was on a locked layer. Nothing was changed."}` |
| `break` on a LINE handle 100 | `{"handles":["101","102"]}`: original erased, handle lost | `{"handles":["100","101"]}`: keeps handle 100 (and groups, draw order, xdata) |
| `ucs` | unknown command | exists |

---

## 3. Verdict table

Verdicts: PULL = compatible as is or trivial port. ADAPT = worth it, needs rework for our architecture, branding or rules. SKIP. 3D = belongs on the `3D` branch.
Effort: S under a day, M a few days, L over a week. Risk is about regressions and conflicts with our own work.
"Pre-image matches" means the commit's patch applies to our HEAD, so we have the unfixed code.

### A. Never-crash, data loss and hostile input (do these first: AGENTS.md says this outranks feature work)

| # | item | upstream commits / files | verdict | why | effort | risk |
|---|---|---|---|---|---|---|
| A1 | Handle band: a file handle or `$HANDSEED` at the top of the range makes every new object get the same handle and replace the file's object | bcac20dd, 140437e7, a516ce11, 8ec1e9a0, 76420454. `doc/lib.rs` (`HANDLE_LIMIT`, `next_handle`, `reserved_handles`), `engine/audit.rs`, io `dxf_read`/`dxf_write*`, tests `io/tests/handle_exhaustion.rs`, `engine/tests/audit.rs` | **PULL** | Reproduced above (1 vs 3 entities). Our `new_handle` uses `saturating_add`, so the seed sticks at `u64::MAX`. Our DWG path (`io/dwg/carry.rs:518`, `dwg_read.rs:135`) calls `bump_handseed` too and needs the same `< HANDLE_LIMIT` guard. | M (S for doc/engine, rest is io/dwg) | low-med |
| A2 | Name lookups through an index (`NamedList` for layers, `NamedMap` for blocks) | 3532ddf6, 04c880e9 (key by folded name, not a hash), bc5749ec (ordered tags: hash keys could collide on web), a34ebd0b (attributes matched by tag in one lookup). New `doc/names.rs` (~500 lines), `doc/lib.rs`, `tables.rs` | **PULL** | Ours is `layers.iter().find(eq_ignore_ascii_case)` and a block scan on a miss (`doc/lib.rs:252,333`); quadratic: 33 s vs 1.3 s above. Take all four (the later ones fix the first). | M | low-med: `Drawing.layers` and `.blocks` change type, about 5 call sites (`d.layers = x.into()`, `.to_vec()`; ours add ui `layers.rs`, io/dwg writers). Merge conflict in `tables.rs` is small. |
| A3 | Tessellation and region budgets | 7804e94e (`Polyline::tessellate_within`, `MAX_TESSELLATED_POINTS`; also `render/hatch.rs`), 95fdf8d1 (`geom/region.rs`, `constraints/auto.rs`, `fonts/ttf.rs`) | **PULL** | Compact bulged vertices expand to gigabytes (CWE-400). Ours has no budget. Applies cleanly. | S | low |
| A4 | Capped reader for local files | 08437703 | **PULL** | We still use `std::fs::read` in `cmd/file/compare.rs:113`, `template.rs:77`, `exchange.rs:210`, `insert_file.rs:88`, `audit.rs:160`, `table/breaks.rs:245`; a FIFO or `/dev/zero` is buffered to EOF. Also apply the same to our own `cmd/file/transmit.rs:79`, which upstream does not have. | S | low |
| A5 | Bounds and indexes: DOF selection set, QSELECT colour text length, log-line escaping; arrays, DIVIDE, revcloud, path array, broken tables; MOCORO copies, array EXPLODE, COMPARE, attribute extraction; xref case variants, LISP OPEN/CLOSE, template names; hatch canonical key; atomic write keeps the file's group | bc9ad6b0, 2c25c2d7, 7e780548, 5a6aa9b6, 2410a1fd, b460a3b8 | **PULL** (2c25c2d7 and 7e780548 conflict slightly in `apply.rs`, `patharray.rs`, `extract.rs`, `compare.rs`, `mocoro.rs`) | Denial-of-service class from hostile files and params; our code is the pre-image for the ones that apply cleanly (bc9ad6b0, 5a6aa9b6, 2410a1fd, b460a3b8). | M total | low |
| A6 | LISP: a routine always yields, charged for its work; READ/EQUAL/SETVAR/ENTMAKE budgets; one-pass output flush; `append` cell cap | ed855587, f9036215, 348d767b, 44b0399a, ea2637ae, 36f5ad64, bf1c5146 | **PULL** | `(while t (length l))` over a long list never gives the UI a frame (hang), and nested callbacks multiply (10000^3). Our LISP is the base, so these apply in order. | M | low |
| A7 | Image data limits (decoded size, oversized data, argument typing, loader limit) | f998f2f6, f91d5ad3, 19fc1f20, 736f56d3, 1f56cef6, f833ab96, 7c580ec8 (`cmd/image.rs`) | **PULL** | Same class. Check against our `imageops.rs` and `tests/image_attach.rs`, which we changed. | S-M | low-med |
| A8 | **Automation roots**: `--automation-read-root/--automation-write-root` confine every path an agent's command reads or writes; every command must be classified | 46ee1a81, 0f33f430, f09c50d6, 09e4f907, 8a42e8a1, b19c3665, 290e578c, 8314fb87, 387a6e6b, d4c4c799, 9cea2f78, 61cfaab5, 4efecc2f, 776e8710, 43d7a192, e10109e2, 7d4f35f1, d670f5ad, 91966ccc. New `engine/file_access.rs` (781 lines) and `file_access/commands.rs` (568), hooks in ~25 files, `mcp/backend.rs`, CLI flags, `docs/mcp.md` | **ADAPT** | We have nothing like it: MCP, control channel and CLI can read and write any path. AGENTS.md says agents drive the app, so a sandbox is a real gap. Our 22 ours-only commands must be classified (the test `every_command_is_classified` will fail until they are); `attin`, `attout`, `etransmit`, `pdfattach`, `pdfclip`, `laytrans`, `sheetset`, `configexport`, `configimport` and `pointcloudattach` touch files. Hooks go in our own file readers too. | L | med: touches every file entry point; mcp and CLI parts belong to another analyst. |
| A9 | UTF-8 BOM in script files | 3d9a59c0, 892a7e2a | **PULL** | Trivial, applies cleanly. | S | none |
| A10 | `properties.set` leaves objects on locked layers out and says how many | 0dcdf8d7, 5787b363, e82dede2, 8b916467, c311ec6b, 7e386a2c; tests `properties_set_locked.rs` | **ADAPT** | Verified bug above (the Properties palette, Quick Properties and agents edit locked objects). Ours handles locks only inside `chprop`. Conflicts in `cmd/props.rs` with our `rho` handling. | S-M | low |

### B. Command behaviour fixes

| # | item | upstream commits / files | verdict | why | effort | risk |
|---|---|---|---|---|---|---|
| B1 | BREAK, BREAKATPOINT, TRIM keep the object's handle, groups, draw order and xdata (AutoCAD 2024 rule); closed and self-cutting polylines; Z kept; "a pick on a cut trims nothing" and about 25 follow-up rules; EXTEND end choice; JSON diagnostics | 156d117d, 4c875d94, 061ecd1c, 1b7cf50f, 3a9b653b, 45d84f10, 9c2b8a25, a70223e2, e35dfe65, f370a558, f6a97d27, 96a9fa61, 0cd36ad1, 63347dbf, 451caaac, 129dd86e, 276424ae, cf46d11e, 032a5dc3 (BREAKATPOINT at command line), 8db82ebf and more. `cmd/modify/trim.rs` (+442), `break_join.rs` (+583), `trimextend.rs` | **ADAPT** | Verified: ours erases and re-adds (handles 101,102). ~48 commits, +7.6k lines in the biggest theme; take as one ordered stack. Associative dimensions follow the piece. | L | med: whole-branch merge shows 1 hunk each in `trim.rs`, `trimextend.rs`, `break_join.rs`, `transform.rs`, 2 in `stretch.rs`, 1 in one test; small, but the stack has to go in order. |
| B2 | JOIN and PEDIT per AutoCAD 2024 measurements (whole objects by ends, circle question, same height, PEDIT convert keeps Z, source's space) | 3c9b4be3, 6c81995e, 19dcf744, e20f5ed5, 96e62b46, 6133dc60, 12b77423 | **ADAPT** | Part of the B1 stack (same files). Takes `join_on_source.rs`, `pedit_heights.rs`. | M | med |
| B3 | **EXTRIM** (new command): trim everything crossing one cutting edge | 14779e26, 07ae46cc, 2cdb5832, 1a4f8d38, 1a57b390, a0b2d45f, c7e6b10f. New `cmd/modify/extrim.rs` (719 lines) | **PULL** | New command we lack; bounded work, tests included. Needs B1 helpers and the `Interactive::highlight_all` trait hook. Needs ribbon slot, icon, it/zh strings, CHANGELOG line. | M | low-med |
| B4 | HATCHGENERATEBOUNDARY (new command) and its right-click entry | 0bd4d54f, f7c4c148, 6412e9c9, ad6d3d12, b39fbb. `cmd/hatch/edit.rs` | **PULL** | New command we lack. Tests included. | M | low |
| B5 | TRIM/EXTEND highlight only the piece a click changes; Fence/Crossing preview, capped | 09351655, bb53e4e7, e6a486b7, 3c7427d2. `Interactive::highlight`/`highlight_all` in `prompt.rs`/`guard.rs` | **ADAPT** | Needs ui canvas support (other analyst). New default-method trait hooks are guarded in `guard.rs` the same way. | M | low |
| B6 | Dimension fixes: honour DIMASSOC off; negative DIMGAP frame; DIMPOST on angular; DIMSPACE Auto uses text height; mirror keeps hatch direction | 57e32b0c, 800b0319, b17880ca, a8882187, bb36fab2, 0ef50935, 4f51d0c6 | **PULL** | 57e32b0c, 800b0319, b17880ca, a8882187 apply cleanly to HEAD (pre-image matches, so we have the bug); bb36fab2, 0ef50935, 4f51d0c6 (mirror keeps hatch direction, plus policy docs and tests) need a conflict check. | S each | low |
| B7 | Layer states in AutoCAD's own form (`ACAD_LAYERSTATES`), restore by mask, `-LAYER stAte`, `-RENAME` viewport configs | 9ed3607b, 50555a91, 38f51899, 5420dea6, 2cb1fcd0, f8be7d83, 38cab40e, bcb6d6ac, 685e3643, 10aa4e18. New `layer/state_prompts.rs` | **ADAPT** | Ours (`layerstate.*`) keeps states only in a private header record, so AutoCAD-saved states are invisible both ways (the upstream commit message makes the same case for the old CADCraft). The io half must be written against our own DXF/DWG writers. | M-L | med |
| B8 | INSERT/ATTDIA/ATTREQ with Edit Attributes dialog, Verify/Preset; "only a command a person started opens the dialog"; ATTDEF justify/style; -ATTEDIT one property at a time; constant attribute values in extents and drawing | 03328362, 5b00e362, ed012ca3, d3419d15, b244b59e, df5063f5, d759668f, 2017653a, d3ec8efe; afcb6aa7, 3e42b346, 28428b0c, e65132c7, f0c1b4ff; 22226a08, e1a048ba, feea659f; 4be3a25b, 7ee382fb, b9821b2b, 4f58d512 | **ADAPT** | Several commits state our "programmatic calls never open dialogs" rule as code and tests. The dialog is UI (ui analyst); engine side is `Interactive::attribute_entry`. Constant-value extents conflict with our `doc/extents.rs` work. | M-L | med |
| B9 | Page setup and layouts: paper sets paper units, inch/mm choice, explicit inch units with MEASUREMENT 1, unchanged plot scale kept, plot flags saved as read, model-space plots take paper units | 9a6396c0, 272e812e, 9c2915c2, 1df2b829, dab89a38, 3ee12a8c, cec00d45, 0b060ec9, a3e82dc9, 62e70432 and the plot-format commits | **ADAPT** | Round-trip fidelity of page setup; `pagesetup` gains `paperUnits`. Several apply cleanly (9a6396c0, 9c2915c2). Conflicts in `layout.rs`, `layout/plotstyle.rs`, `saveas.rs`. | M | low-med |
| B10 | Selection and grips honour layers frozen in the active viewport; hidden objects take no grips | e43c39c5 (28 files), ddfa833b | **ADAPT** | Frozen-in-viewport objects can still be picked and edited in ours. `select.rs` conflicts with our pick work. | M | med |
| B11 | Crossing selection tests a block by its geometry; blank MTEXT/MLEADER pickable; Tab cycles every snap candidate | 15d2a63b, afb53d53, d71f2b95; bc7f41d5, 89817444, f04ca514, 2ef9cf67, a0ce65ef, 2697456c, cf9f3a7e, 72f07e5c, 086bd8a8; b3a1e1af, 2bd91666; ce4b33ad (Last) | **ADAPT** | We independently made blocks pick on lines and words (24e7d4f5, `insert_text_outlines`) and added window preview; the two overlap in `select.rs`, so the crossing logic must be re-derived on our outlines. | M | med |
| B12 | ACI grey entries 8, 9, 250-254 | 83a2b4c3 (`color/lib.rs`) | **ADAPT** | Changes 8 from (128,128,128) to (65,65,65), 9 from (192,192,192) to (128,128,128), 250-254 to 51,80,105,130,190. ACI 8 and 9 are very common greys, so this is visible. **Provenance unstated**: confirm it was observed in the UI rather than copied from an Autodesk table, and record it. | S | low, but a visible colour change |
| B13 | User aliases persist across restarts; PGP import perf; non-ASCII alias keys | fd3a0b9e, c9082d97, cc1914d8, 4e1ebc79 (`file/pgp.rs`, ui `options.rs`) | **ADAPT** | Ours loses ALIASEDIT aliases on restart except through CONFIGEXPORT. Overlaps our `cmd/config.rs`. | S-M | low |
| B14 | LISP features: namedobjdict, dictsearch, dictnext, tblobjname, handent/entget of non-graphic objects; entlast sees own edits; HANDSEED live; getvar point lists; findfile folders; load-name confinement; block-entity entnext/entget (with cached index) | 11d66cc3, b23d1728, 9e022cf3, 4750d673, 2ccf5874, 77b30143, ec23a13e, 744e99ca, 853ca574. New `lisp/objects.rs` | **PULL** | Our LISP is base-exact, so these apply in order after A6. 11d66cc3 touches io `dxf_raw.rs`. | M | low-med |
| B15 | Small command fixes | 877ce875, 1a036390 (SPLINE CV), ad5ac326 (REGION asks), e5972c99 (MTEXT ortho), 1cfc7d8f (PLINE elevation), 6f5bf85d, b44cb368, 2c8a6edf (REVCLOUD), 0f94865f (RevcloudProps), 9aff1d6a (SETVAR dims saved), 650ae383 (BSAVEAS), 1b7cf50f (arc pick near end), 220d8dcb (polar input) | **PULL** | 877ce875, ad5ac326, e5972c99, 1cfc7d8f, 0f94865f, 9aff1d6a, 650ae383, 220d8dcb apply cleanly; the rest are marked in the TSV. | S each | low |
| B16 | geom performance with bit-identical results and test oracles | c2175377 (NURBS derivative), 98b47c19 (banded spline fit), 1f75a9fb (region pairs), 08ea23ec (region test budgets) | **PULL** | The first three apply cleanly; results are bit-identical and each ships a test-only oracle. | S | low |
| B17 | `constraints` | bc9ad6b0 (DOF set), 95fdf8d1 (candidate and island caps in `auto.rs`) | **PULL** | The only two upstream constraint commits; both are bounds. | S | low |

### C. New features

| # | item | upstream commits / files | verdict | why | effort | risk |
|---|---|---|---|---|---|---|
| C1 | **UCS**: UCS and UCSICON commands, `ucs.*`, points, angles, ortho/snap/grid/polar and tracking, DIMLINEAR, LISP `trans`/`getpoint`, `-PAN` and PAN in the UCS; DXF/DWG UCS records | bc7357ff, 9f833f3d, a0e023d7, e3aeff2a, 821b8ad1, 99b7dc80, 3f14431a, and ~28 more (Stack ucs, SBaksa). `engine/ucs.rs` (404), `cmd/ucs.rs` (706), `lisp/ucs_tests.rs`, tests `ucs.rs`, `ucs_drafting.rs`; ~5.2k lines in total | **ADAPT** | We have no UCS at all (`ucs` is unknown; only the DXF table exists). Core AutoCAD drafting feature. Upstream keeps the drawing in WCS and handles only XY-plane UCSs, tilted ones are stored and read in World, so this stays 2D and goes to **main**, not 3D (X, Y, ZAxis, Face say "not available yet"). Touches io, ui (icon, status bar, ViewCube) and render. | L | med: wide surface; needs ribbon, icon and i18n entries. |
| C2 | **Dynamic block references**: rotation, flip, scale, polar stretch, array actions; polar/XY/base point/alignment/lookup parameters; grips and Dynamic Input | ee069795, ebeb2638, 46acac72, b471e597, fc8caca7, fc82a2f9, 7f6033c0, 28d240ca and ~18 more, plus 2ez4gcx and Cody Vogt follow-ups (3f805561, c862f8ac, 522ed596, 05cd8ac0, 8c143f8f, e3599da8). `doc/dynblock/apply.rs` 1088 to 2300 lines, `dynblock.rs`, `grips.rs`, `reference.rs`, `cmd/blocks/dyngrip.rs` | **ADAPT** | We parse these kinds but `apply.rs` acts on far fewer. Our authoring code (`dynblock/author.rs`, `bauthor.rs`) only constructs `Linear` and `Point` parameters, so the `ParameterKind` field additions do not break it, but `bauthor.rs` should gain the new kinds. io test data `dynblock_*.txt` must be checked for provenance (other analyst). | L | med |
| C3 | Dimension Style Manager (engine half): `-dimstyle`, `dimstyle.overrides`, `dimstyle.saveas`, `dimoverride.clear`, `properties.describe`, large `properties.set` | a770fa5a (+5.5k lines: `dimstyle_cli.rs` 509, `dimstyles.rs`, `properties.rs` 806, `props/set.rs` 781) | **ADAPT** | We have the `dimstyle` command but no manager dialog or `-DIMSTYLE`. `properties.describe`/`set` make the Properties palette data-driven, in keeping with "everything is a command". Conflicts with our `rho` row and A10. The dialog itself is the UI analyst's. | L | med-high |
| C4 | Blocks palette and block libraries: `blockspalette`, `blocklibrary`, `blocklibimport`, `blocklibraries` | 65ed2fb7. `cmd/blocks/library.rs` (403), `doc/library/files.rs`, tests `blocklibrary.rs` | **ADAPT** | AutoCAD BLOCKSPALETTE parity, mostly UI. Library paths must be held to the roots (A8). | L | med |
| C5 | ARRAY: Key point option, `array_live` (live ARRAYRECT/ARRAYPOLAR dialog), refusal bounds | b600293c, bdc0c7b2, bd7d94c8, c6e80bea, 2ecf474d. `engine/array_live.rs` | **ADAPT** | Engine half is small; the dialog is UI. | M | low |
| C6 | QLEADER (new command) | 762b6426, 43207db6, 91966ccc. `cmd/annotate/qleader.rs` | **PULL** | New command we lack. | S-M | low |
| C7 | Realtime PAN and `-PAN` | fc67a5c2, a7808fdc, aab42ef9, 0191a5fa | **ADAPT** | Command side is small; the realtime drag is UI. Needs UCS (C1) for the "-PAN in the UCS" part. | S | low |
| C8 | MENUBAR and TOOLBAR commands | ad072e6c, 9c2ebc0e, 98240d82. `cmd/interface.rs` | **ADAPT / SKIP** | MENUBAR maps to our Windows-style menu bar (move it to `UI_COMMANDS` in `ui-egui/src/menus.rs`, since AGENTS puts UI-only commands there). TOOLBAR conflicts with "No macOS-style tool bar": skip. | S | low |
| C9 | PASTETABLE: paste Excel TSV as TABLE | 28ab81b3, ff88c83b. `cmd/table/paste.rs` | **ADAPT** | Needs `craft-csv` from craft-libs, which we can add (we already use craft-ui, craft-pdf, craft-window and craft-update from it). Low priority. | S-M | low |
| C10 | WBLOCK dialog and `wblockpick` | a974b202 | **ADAPT** | UI-heavy; engine half small. | M | low |
| C11 | Greek keyboard layout for commands, aliases and prompt keywords | 7064a685. `engine/keyboard.rs` (186), `session/cmdline.rs`, `interactive.rs`, `prompt.rs` | **PULL** | We already ship `el.tsv`; the engine part is self-contained. | M | low |
| C12 | Proxy graphics and kept-dimension blocks (draw ACAD_PROXY_ENTITY pictures, keep `*D` blocks) | 0829bfa0, 50a1f833, 6ed01dea and ~28 Walter Haslbeck / Tanish Manku commits, f297806c, 3ac368c0, a4b15ec5, 8c2852fd. `doc/entity/proxy.rs` (1,411 lines) | **ADAPT** (cross-area, io and render) | Decoder follows the ODA public DWG spec (allowed source). Real value for files from vertical products, but it is a big cross-layer feature that depends on our own DWG reader. | L | med |
| C13 | OLE2FRAME transform/pick/extent rules (20 commits, ~1k lines) | 0c486e33, 87f3c087, 9bd5c479 and others (SBaksa) | **SKIP for now** | Niche (we only pass the entity through); revisit if our DWG keeps OLE data. | M | low |
| C14 | ACIS save-loss reporting | f7b3b689, fd17a05b, 20386506, ea4a59b6 | **ADAPT** (cross-area) | AGENTS routes ACIS-in-DWG work to main when it works without the `solids` feature; check that each commit does. | S | low |
| C15 | PDFIMPORT built on craft-pdf's interpreter | d4fd0dd7, 13b956c1 | **SKIP** | We have our own `pdfimport` (`cmd/underlay.rs`, `io/pdf_read.rs`); two competing implementations. The io analyst decides. | | |
| C16 | craft-libs migrations: `craft-dxf` for DXF tags, `lin_pat` parsing, shared Base64/PNG | c736e488, 865ecbd0, 91b14c83, ca6c1003, f81c52b9 | **SKIP** until the io analyst decides about `crates/dxf` | Patches that use `craft_dxf::` (`cmd/file.rs` 9 places, `lisp/drawing.rs` 8, `library/files.rs` 4) need the API compared against our `crates/dxf` first. | | |
| C17 | Clippy-pedantic churn in geom/color | 2ac6d98d, ff2163dd, 64f70796, fefb9737, a84de561 | **SKIP** | Style only, and conflicts with our lint policy. | | |
| C18 | IO fidelity commits that arrived through the area | 9244ffda (open on saved view), 42503a8e, 94d9e354, a4d8cae5, 8e9dc8a6, a801739e, 0722afc5, b325046f, cef9ee4b, bf137e2c | **hand to io analyst** | Not engine logic. | | |

---

## 4. Where DraftCraft is ahead (evidence)

- **22 commands upstream lacks**: `adcenter`, `areatotal`, `attin`, `attout`, `baction`, `bparameter`, `conic`, `configexport`, `configimport`, `etransmit`, `geographiclocation`, `georemove`, `imageadjust`, `imageclip`, `laytrans`, `laywalk`, `pdfadjust`, `pdfattach`, `pdfclip`, `pointcloudattach`, `sheetset`, `ulayers`. In this area: `inquiry/totals.rs`, `blocks/attio.rs` (656-line diff), `cmd/config.rs` (645), `file/transmit.rs`, `layer/tools.rs`, `geo.rs`, `imageops.rs` (650), `underlay.rs` (432), `pointcloud.rs`, `sheetset.rs`, `blocks/designcenter.rs`, `draw2/conic.rs`. Each has its own tests (`area_total.rs`, `attout_attin.rs`, `config_file.rs`, `conic_rho.rs`, `pdf_underlay.rs`).
- **Dynamic block authoring**: `doc/dynblock/author.rs` (472), `cmd/blocks/bauthor.rs` (482), `tests/dynblock_authoring.rs`. Upstream only has the reference side.
- **Selection**: live window-selection preview (`Session::window_preview`) and block/dimension picking on drawn lines and words (`insert_text_outlines`; tests `pick_audit.rs` (ours only) and our extensions to `pick_text.rs`); upstream has neither (0 hits).
- **Geometry**: `geom/bezier.rs` (cubic Bézier paths for PDF/SVG writers and PDFIMPORT round trips).
- **Units**: `new` takes `units: in|mm|cm`; upstream takes only `metric`. Our drawing-unit handling is richer (`units.rs`, +72).
- **Properties**: conic Rho under `properties.set`.
- Crash-guard design is at **parity, not ahead**: `guard.rs` has the same design (upstream only added guarded wrappers for new trait methods), `panic_hunt.rs` is byte-identical apart from the rename, `capped_read.rs`, `finite.rs` are identical, the crate-root `deny(clippy::unwrap_used...)` attributes are identical. Upstream's panic-prevention is therefore not ahead of ours; where it is ahead is DoS and data-integrity hardening (section A).
- Tests: our area has 1,797 tests at the base, 1,891 now (+94); upstream has 2,522 (+725). We are behind on test count.

## 5. Where we are behind (concrete)

1. No UCS (command, icon, input in a UCS, LISP `trans`).
2. Dynamic blocks cannot rotate, flip, scale, array or polar-stretch references as AutoCAD does.
3. No agent filesystem sandbox (automation roots).
4. Silent data loss with a hostile top-of-range handle; quadratic open time (A1, A2).
5. BREAK/TRIM/JOIN/PEDIT lose handles and miss ~25 measured AutoCAD rules; no EXTRIM, no HATCHGENERATEBOUNDARY.
6. `properties.set` ignores layer locks.
7. Layer states are not AutoCAD's; page setup paper units; no Dimension Style Manager or `-DIMSTYLE`; no ATTDIA/Edit Attributes; aliases not persisted; LAYLOCKFADECTL absent; no block libraries palette; 631 fewer tests.

## 6. AGENTS.md check of the candidates

- **No panics / unsafe**: upstream area code has none outside tests. Added lines carry `#[allow(clippy::unwrap_used, ...)]` only in `file_access/commands.rs` and `lisp/ucs_tests.rs`, both test code. No `unsafe` added. The same crate-root lints apply to our tree, so a port that compiles passes them.
- **Programmatic calls never open dialogs**: upstream commits ed012ca3, d3419d15, b244b59e and df5063f5 add exactly this rule for INSERT's Edit Attributes dialog; pull them with B8. Dialog-bearing ports (C3, C5, C10) must keep the JSON form dialog-free.
- **Everything is a command / UI is thin**: UCS, EXTRIM, QLEADER, etc. are `CommandSpec`s with JSON `run`. MENUBAR and TOOLBAR are engine commands in upstream but are UI-only for us, so they go to `UI_COMMANDS`.
- **Layering**: all area changes stay in L0 to L4. The new `Interactive` hooks (`highlight`, `live_value`, `text_editor`, `attribute_entry`, `array_live`) are trait methods with no UI dependency.
- **Clean-room**: upstream is MIT or Apache, so reading it is fine. Flags:
  (a) Many commit messages say behaviour was "measured in AutoCAD 2024 (accoreconsole)". AGENTS allows running AutoCAD black-box, but the owner should confirm that driving `accoreconsole` is within "run it, look at its UI". The ported code and tests are upstream's own.
  (b) 83a2b4c3 changes ACI colour constants without a stated source (B12).
  (c) io test data `crates/io/tests/data/dynblock_*.txt` (added with the dynamic-block work) are AutoCAD-flavoured DXF; per AGENTS a kept drawing needs a row in `ATTRIBUTION.md`. I did not see one upstream.
  (d) No area commit adds fonts, hatch patterns, linetypes or icons. The proxy decoder cites the ODA public DWG spec.
- **Time via chrono**: nothing hand-rolls calendar conversion in the area diff.
- **Process**: every port needs a PR for owner review; user-visible changes need a `CHANGELOG.md` Unreleased line; new interface strings need `it.tsv` (menu entries also `zh.tsv`, and check `zh_tw.tsv`); new ribbon commands need a drawn icon in `icons.rs`; `cargo xtask ci` before commit. All of this is 2D work for **main**; nothing in the area belongs on `3D` except the UCS tilted-plane options, which upstream itself leaves "not available yet".

## 7. Blockers and integration hazards

1. **craft-libs split**: upstream replaced in-tree `cadcraft-dxf`/`dwg` with `craft-dxf`/`craft-dwg`. Area patches that mention `craft_dxf::` or `craft_mime`/`craft_csv` need an API comparison with our `crates/dxf`.
2. **Hot-spot files** where our own work collides: `select.rs`, `doc/extents.rs`, `cmd/props.rs` (our `rho`), `cmd/file.rs`, `cmd/layer.rs`, `lisp/drawing.rs`, `snap.rs`, `cmd/exchange.rs`.
3. **Doc model**: our `Image` has an `underlay` field; upstream struct literals need `underlay: None`. `Drawing.layers`/`blocks` change type under A2.
4. **Stack order**: 312 of 391 commits only apply after their stack predecessors. Port by theme in upstream order, not by isolated hash.
5. **Classification test** for automation roots will fail until our 22 extra commands are classified.
6. **Other analysts**: 80 area commits also change io, 68 ui, 19 render. Rows marked cross-area depend on their decisions.
7. No compile or test run was possible here.

## 8. Suggested order

1. A1, A2, A3, A4, A5, A6, A7 (hardening, mostly clean). B16, B17, A9.
2. A10, B6, B15 (small clean fixes). Appendix A is the list that applies cleanly.
3. B1/B2 stack, then B3, B4.
4. A8 (automation roots), B7, B9, B12 (after a provenance check), B13, B14.
5. C1 (UCS), C2 (dynamic blocks), C3 (dimension styles), B8, B10, B11.
6. C4 to C11 as UI capacity allows; C12 with the io analyst; skip C13, C15 to C17.

---

## Appendix A: 79 area commits whose patch applies to our HEAD on its own

Source: `triage.out` (`git apply --check` of the area part) plus `mergesim.out`. They apply as isolated patches to the area files; non-area parts of the same commit (io, ui, render) are not applied. Rows marked "(io/ui)" in the TSV need the matching work from the other analysts.

| commit | date | author | theme | subject |
|---|---|---|---|---|
| cc0e3c7a | 2026-10-13 | Cody Vogt | dynblock | Dynamic blocks: a stretch or move carries a multiline attribute's text along |
| d35ee3ae | 2026-10-13 | Cody Vogt | dynblock | Block reference grips: none on locked or invisible attributes; a moved multiline attribute moves its text |
| 0f94865f | 2026-10-19 | NetterSilver | command behaviour | Write RevcloudProps xdata on revision clouds |
| a801739e | 2026-10-19 | Oliver Hagmann | other | DXF/DWG save: kept render settings take the declared version's class-version form |
| b3a1e1af | 2026-10-19 | Oliver Hagmann | selection/snap/grips | Object snaps: Tab cycles every candidate snap under the cursor, not one per mode |
| 7f85a547 | 2026-10-20 | Cody Vogt | dynblock | Dynamic blocks: a value record takes its shape from its parameter |
| a0e023d7 | 2026-10-20 | SBaksa | ucs/view | Angles: shown and entered directions are measured in the current UCS |
| b17880ca | 2026-10-22 | Nicholas Gower | dimensions | dim: apply the style's DIMPOST to angular dimensions (#1653) |
| 800b0319 | 2026-10-24 | dotCooCoo | dimensions | Dimensions: a negative DIMGAP draws a frame around the dimension text |
| a3e82dc9 | 2026-10-24 | Nicholas Gower | layout/plot | plot: model-space plots take their units from the paper (#1828) |
| cc1914d8 | 2026-10-26 | WillPastor | command behaviour | Avoid repeated alias scans during PGP imports |
| 3e0de624 | 2026-10-27 | SBaksa | blocks/attributes | ATTDEF: reach justify through draw's re-export now that draw.rs is split |
| 1f75a9fb | 2026-10-28 | vangel76 | geom | geom: test region candidate pairs once, in their lowest shared grid cell |
| 5e8d4e7f | 2026-10-28 | Cody Vogt | selection/snap/grips | Snap: gather_in takes the object to skip, like its siblings' argument lists |
| 98b47c19 | 2026-10-28 | vangel76 | geom | perf(geom): limit the spline fit solver to the matrix band |
| c2175377 | 2026-10-28 | vangel76 | geom | perf(geom): make NURBS derivative allocation-free up to degree 10 |
| d0ab457f | 2026-10-28 | Aldo Zarate | command behaviour | Relative height codes kept when a text opens in the editor |
| 60b0f21e | 2026-10-30 | The old Dutch Guy | selection/snap/grips | feat(ui): show pickbox-only cursor at object selection prompts |
| cef9ee4b | 2026-10-30 | ffishman | other | fix(dwg): read kept objects as the file's version on every save, not just the first |
| a84de561 | 2026-10-31 | Cédric Legrand | constraints/color | refactor(color): satisfy Clippy pedantic lints |
| 0c486e33 | 2026-11-01 | SBaksa | ole2frame | Engine: refuse rotating, scaling or mirroring an OLE frame on main's transform paths (#2587) |
| 64f70796 | 2026-11-01 | Cédric Legrand | geom | test(geom): use assert_eq for NaN arc bound |
| c22700d3 | 2026-11-01 | SBaksa | layout/plot | Plot: refuse several layouts for PNG and SVG output (review on #2585) |
| 46ee1a81 | 2026-11-02 | Nicholas Gower | automation-roots | mcp: port the automation roots onto the clap CLI |
| 6c81995e | 2026-11-03 | 2ez4gcx | trim/break/join/pedit | fix(engine): JOIN onto the source object as AutoCAD 2024 does |
| cf46d11e | 2026-11-03 | 2ez4gcx | trim/break/join/pedit | BREAK and BREAKATPOINT: test the measured ends, closing side and undo |
| f6a97d27 | 2026-11-03 | 2ez4gcx | trim/break/join/pedit | EXTEND: a polyline extends the end on the pick's side of its middle |
| fefb9737 | 2026-11-03 | Cédric Legrand | geom | fix(geom): move NURBS lint expectations to velocity |
| 0bd4d54f | 2026-11-04 | okereke-dev | trim/break/join/pedit | HATCHGENERATEBOUNDARY: Generate Boundary in the right-click menu of a hatch (#2801) |
| 14779e26 | 2026-11-04 | okereke-dev | trim/break/join/pedit | EXTRIM: trim everything crossing one cutting edge on the side picked, with a live preview (#2802) |
| 762b6426 | 2026-11-04 | 2ez4gcx | command behaviour | QLEADER at the command line, as AutoCAD 2024 runs it with its default settings |
| 96a9fa61 | 2026-11-04 | Zaxaerith | trim/break/join/pedit | fix(engine): clarify JSON TRIM and EXTEND pick diagnostics (#2750) |
| ab71b353 | 2026-11-04 | 2ez4gcx | selection/snap/grips | OPTIONS: reset puts every profile variable back; a refused change leaves the drawing's snap and grid |
| f91d5ad3 | 2026-11-04 | wovadi | hardening/robustness | Images: refuse oversized data before decoding, and say what to do with a relative path in an unsaved drawing |
| 44b0399a | 2026-11-05 | mo | lisp | LISP: a step's list walking and callback steps count against the slice, so a routine always yields |
| c397ab1e | 2026-11-05 | SBaksa | ole2frame | OLE2FRAME: the frame can be picked by its edges (review on #2587) |
| 9a6396c0 | 2026-11-06 | austin62jake | layout/plot | fix(engine): Page Setup's paper sets the layout's paper units (#2836) |
| 38f51899 | 2026-11-07 | 2ez4gcx | layers/rename | -RENAME: viewport configurations, and AutoCAD's messages |
| 9ed3607b | 2026-11-07 | 2ez4gcx | layers/rename | DXF/DWG: layer states in AutoCAD's form, restored by mask and current layer |
| 57e32b0c | 2026-11-09 | Vincent Cloutier | dimensions | fix(engine): honor non-associative DIMASSOC mode |
| fb9a8331 | 2026-11-09 | Vincent Cloutier | hardening/robustness | test: handle macOS rejection of non-Unicode filenames |
| 08437703 | 2026-11-10 | Dylan M. Taylor | hardening/robustness | fix(engine): read local files for RECOVER, INSERT, IMPORT, COMPARE, NEW, DATALINK and STYLESMANAGER through the capped reader (#2687) |
| 36f5ad64 | 2026-11-10 | Dylan M. Taylor | lisp | fix(engine): move LISP output to the command history in one pass (#2714) |
| 7804e94e | 2026-11-10 | Dylan M. Taylor | geom | fix(geom): bound the points one polyline tessellation samples (#2723) |
| 95fdf8d1 | 2026-11-10 | Dylan M. Taylor | selection/snap/grips | fix: bound region grid registrations, island containment tests, snap_ends visits, inference duplicate searches and font name decoding (#2742) |
| bc9ad6b0 | 2026-11-10 | Dylan M. Taylor | selection/snap/grips | fix: index or bound four small lookups (DOF selection, xref palette, QSELECT colours, log lines) (#2728) |
| 152e3b56 | 2026-11-11 | cdwilson127 | command behaviour | MCP: find_text takes `query` as its filter; stylesmanager set replies with what changed (#3011) |
| 2410a1fd | 2026-11-11 | Dylan M. Taylor | command behaviour | hatch: one canonical key per source loop before tessellation (#3043) |
| 4750d673 | 2026-11-11 | cdwilson127 | lisp | LISP: getvar returns a point list for point system variables (#2988) |
| 5a6aa9b6 | 2026-11-11 | Dylan M. Taylor | hardening/robustness | fix: bound or confine seven name, path and file inputs (#2732) |
| 650ae383 | 2026-11-11 | V1dalTheAbidal | blocks/attributes | Block Editor: BSAVEAS keeps the source block's last saved definition |
| b460a3b8 | 2026-11-11 | Dylan M. Taylor | other | fix(engine): an atomic replacement keeps the old file's group, or drops the group bits (#2743) |
| ea2637ae | 2026-11-11 | Dylan M. Taylor | lisp | fix(lisp): charge READ text, EQUAL string bytes, SETVAR and ENTMAKE copies, and nested callbacks against one budget (#2741) |
| 1f56cef6 | 2026-11-13 | dotCooCoo | hardening/robustness | fix(engine): image loaders start a file only while its largest size still fits the loaded-image limit |
| ddfa833b | 2026-11-13 | Dimitrios Vidalis | selection/snap/grips | Grips: hidden objects paint no grips and take no grip hits (#3048) |
| f833ab96 | 2026-11-13 | dotCooCoo | hardening/robustness | Images: IMAGEATTACH refuses a wrongly typed at, scale, rotation or name |
| fa7a9772 | 2026-11-14 | FredApps | other | Naming: use Additional Tools for the utility menu (#2945) |
| 8db82ebf | 2026-11-15 | jayzed80 | trim/break/join/pedit | TRIM/EXTEND: Space enters an option at the mode, projection and edge prompts (#3092) |
| 9aff1d6a | 2026-11-15 | jayzed80 | dimensions | DXF/DWG: save dimension variables set with SETVAR (#3100) |
| ca6c1003 | 2026-11-15 | Dylan M. Taylor | layout/plot | Encode plot previews with shared PNG writer |
| 2ccf5874 | 2026-11-16 | Zinadin Zidan | lisp | LISP: findfile finds folders (#2985) (#3165) |
| f81c52b9 | 2026-11-16 | dotCooCoo | other | Dependencies: take craft-libs main at 3af2280ee3 |
| 05cd8ac0 | 2026-11-17 | 2ez4gcx | dynblock | Dynamic blocks: a stretch moves a wipeout's points inside its frame |
| 9c2915c2 | 2026-11-17 | jayzed80 | layout/plot | Page setup: choose inches or millimetres for a layout's paper units (#3249) |
| ad5ac326 | 2026-11-18 | austin62jake | selection/snap/grips | fix(engine): REGION typed with nothing selected asks for objects (#2447) |
| 22226a08 | 2026-11-19 | Alex Ortiz | blocks/attributes | -ATTEDIT: Position, Height, Angle and Style one at a time (#2152) |
| 865ecbd0 | 2026-11-19 | Dylan M. Taylor | other | doc: delegate .lin and .pat parsing to craft-dxf::lin_pat |
| 220d8dcb | 2026-11-20 | Tanish Manku | ucs/view | Typed polar input with a world prefix (@*d<a or *@d<a) keeps its distance and WCS direction for deferred tangent snaps |
| 2d95af7d | 2026-11-21 | mo | command behaviour | fix(ui-egui): Edit > Paste and PASTECLIP read the system clipboard |
| 3d9a59c0 | 2026-11-21 | Mex | command behaviour | fix(script): accept a leading UTF-8 byte-order mark |
| 892a7e2a | 2026-11-21 | Mex | other | Cover BOM-prefixed commands and first-line comments |
| 94d9e354 | 2026-11-22 | Jon Breen | layout/plot | Save: every layout keeps its plot settings, the Model layout included |
| a4d8cae5 | 2026-11-22 | Jon Breen | hardening/robustness | DXF and DWG save: a hatch keeps its boundary path flags, so islands stay holes in other programs |
| 1cfc7d8f | 2026-11-23 | ViktorBystrov72 | command behaviour | PLINE: keep the current drawing elevation |
| a8882187 | 2026-11-23 | cloutier | dimensions | fix(engine): use dimension text height for DIMSPACE Auto (#3429) |
| a8c50b09 | 2026-11-23 | skylar taylor-barrick | other | Regression tests for #618, #651 and #1336 (#1961) |
| e5972c99 | 2026-11-23 | Zaxaerith | command behaviour | fix(engine): ignore Ortho mode for MTEXT opposite corner (#3388) |
| 4f58d512 | 2026-11-24 | mo | blocks/attributes | fix(render): a block reference draws its constant attribute values (#3413) |
| 877ce875 | 2026-11-24 | bic98 | command behaviour | SPLINE: ask again when the CV degree is fractional |

