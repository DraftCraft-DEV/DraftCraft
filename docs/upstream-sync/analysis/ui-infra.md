# DraftCraft <- CADCraft: UI, apps, agent interfaces, quality infrastructure

Scope: `crates/ui-egui`, `crates/mcp`, `apps/*`, `xtask`, `.github`, `packaging`, `docs`, `lint`, i18n, control
channel, web build, test and CI tooling. Read-only analysis of `/home/user/DraftCraft` (branch main) against
`cadcraft/main` (upstream full clone at `/home/user/storytold/cadcraft`). Nothing in the repo was changed.

Verdicts: **PULL** = take the upstream code (rename, merge, translate). **ADAPT** = rework to sit in our ribbon,
`theme::Tokens` and the `app.run` / `app.start` pattern. **SKIP** = conflicts with our design, we have it, or it
belongs to upstream's own infrastructure.
Effort: S = up to 1 day, M = 2 to 4 days, L = a week or more (one developer or agent, including tests, it/zh/zh_tw
strings and the CHANGELOG line). Risk = chance of regressions, merge pain or a policy problem (L / M / H).

---------------------------------------------------------------------------------------------------------------

## 0. Read this first

1. **Our real fork base is upstream `fc157ea9` (2026-11-05 "land/1031"), not `2569dbd5`.** I matched our import
   tree against 150 upstream first-parent commits (renamed blobs): the peak is `6ca414c3` / `df659744` /
   `fc157ea9`, all on 2026-11-05. Evidence by feature: `017cc9b1` (MCP render background/plot_style) and `0902a13b`
   (craft-web-host) are in our tree; `14779e26` (EXTRIM), `46ee1a81` (automation roots) and `6d744887` (token to
   127.0.0.1) are not. Since `fc157ea9`: **1,476 commits, 914 non-merge, 387 non-merge touch my
   areas** (66 parity/roadmap bookkeeping, 45 i18n, 41 review-fix "Stack"/CodeRabbit follow-ups, 15 CI/packaging/nightly,
   14 craft-libs repins; the rest is features and fixes).
2. **Upstream main changed its UI foundation on 2026-11-25** (`b55abde2` merges `minimal-port-next`: `8fe5682b`,
   `180ff447`, `68de3797`, `4f4a3e94`, six "Repin craft-libs to storytold next" commits). It now hosts the
   `craft-ui` runtime (new dock model, floating panels as OS windows, Tab/focus traversal, scale as RootEnv) on an
   unmerged craft-libs `next` branch plus egui fork `b9770908e`. **Only 32 of the 387 area commits (8%) come after
   that point.** The last main commit before it is **`414e5442`**; it still pins craft-libs main `453b0da782` and
   our own egui fork `6d518fc79`. Moving our `craft-ui/window/update` pins from `92d392cf` to `453b0da782` needed
   no `ui-egui` source changes upstream (`f81c52b9`, `e9638efc`, `ef12e7a9` touch only Cargo files and CLI tests).
   **Recommendation: pull from the `414e5442` snapshot and defer the runtime port.**
3. **Mechanical merge trial** (3-way, base `fc157ea9` renamed cadcraft->draftcraft, theirs `414e5442`, ours HEAD;
   `git merge-file`, per file, UI-area files only): 55 files upstream changed that we never touched (clean take),
   59 new upstream files, 54 files both sides changed of which 23 conflict, **84 conflict hunks in total**.
   Hot spots: `lib.rs` 12, `cmdline.rs` 11 (our AutoComplete learning vs their paste/IME work), `updates_ui.rs` 9
   (take ours: we have our own self-update), `apps/draftcraft/src/main.rs` 9, `docking.rs` 8, `canvas.rs` 4,
   `palettes.rs`/`options.rs`/`menus.rs`/`i18n/tests.rs` 3 each. Against full `HEAD` it is 99 hunks. Per-file list:
   `analysis/merge_try_prenext.txt` (and `merge_try.txt` for HEAD). **`ribbon.rs`, `ribbon/context.rs`,
   `window_frame.rs`, `workspaces.rs`, `brand.rs`, `self_update.rs`, `whats_new.rs`, `cmd_usage.rs` do not exist
   upstream, so nothing upstream can conflict with them; the risk is in the shared glue (`lib.rs`, `menus.rs`,
   `docking.rs`).**
4. **The panic guard is not a differentiator.** `crates/engine/src/guard.rs` and `tests/panic_hunt.rs` are
   byte-identical to the fork base; upstream has since added more guarded trait methods (`highlight`, `live_value`,
   `text_editor`, `attribute_entry`, `array_live`...: 345 lines vs our 267) and every pulled feature below adds
   methods we must guard too. See section 3 for what we are really ahead on.
5. **Ribbon button that is dead today:** `ribbon.rs:192/432` "Dimension Style" runs `dimstyle` with no parameters and
   the engine answers "`name` is required" (`engine/src/cmd/props.rs:909`); there is no Dimension Style Manager.
6. **Each pulled feature costs translations.** Union of English UI keys: ours 1,771, upstream 2,271 (about 500 new
   strings). Upstream's own Italian/Chinese catalogs cover only 56 / 51 of their new strings (their model is
   "English in the PR, translations in `storytold/craft-i18n`"); Swedish (719), Polish and Ukrainian (251) cover
   most. We must write the Italian + Chinese menu rows ourselves (AGENTS.md rule), and we can use sv/pl rows as the
   key list.

---------------------------------------------------------------------------------------------------------------

## 1. What upstream added since the base, verdict per item

### A. UI features, dialogs, palettes

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| A1 | **Blocks palette**: preview gallery, block libraries, name filter, repeat placement, hostile-input work budgets | `65ed2fb7` (#2122); `ui-egui/src/blocks_palette.rs` (1,735 l), `tests/blocks_palette.rs` (540), engine `cmd/blocks/library.rs` (378), `insert_file.rs` (+487), `tests/blocklibrary.rs` (308); commands `blockspalette`, `blocklibraries`, `blocklibrary`, `blocklibimport` | ADAPT | Ours is a flat button window (`dialogs.rs:396 fn blocks`). Our ribbon already has the entry points (`ribbon.rs:123,359,563` `ui.dialog.blocks`); re-point them at the palette (a new `Panel` in `docking.rs`) and let an Insert-panel gallery reuse its preview cache. Engine half overlaps `insert_file.rs` (engine analyst). | M | M |
| A2 | **Dimension Style Manager**, `-DIMSTYLE`, `dimstyle.overrides`, dimension shortcut menu | `a770fa5a` (#1460) + `3ec1e1b2`; `managers/dimstyle.rs` (818), `tests/it/dimstyle_dialog.rs` (320), `dimension_menu.rs`; engine `cmd/dimstyle_cli.rs` (509), `dimstyles.rs`, `props/set.rs` (781), `properties.rs` (806) | ADAPT | Fixes the dead ribbon button (point 5). Hook it to Annotate > Dimensions (button + style drop-down "Manage...") and Format menu; keep `app.run("dimstyle", params)` for the JSON form. Largest single item. | L | M |
| A3 | **Properties palette from `properties.describe` / `properties.set`** (text, mtext, dimension rows; fixed-height styles; several objects of one type) | part of `a770fa5a`; `palettes/described.rs` (226), `tests/it/properties_described.rs` (197), `palettes.rs` (+820 incl. refactor), engine `properties.rs` | ADAPT | Engine-driven rows: the same JSON an MCP agent can read, and a source for ribbon contextual panels (our ribbon already has property lists, `ribbon.rs` tests). `palettes.rs` merge: 3 hunks. | M (with A2) | M |
| A4 | **In-place editing** of TEXT, attribute definitions and dimension text on double-click; TEXTEDIT window; inline dimension-constraint editor | `9f2cd4ca`, `44e0e431`, `7ecf836d`, `afcf5d34`, `4683023d`, `6b707c89`, `df26fc51`, `44950708`; `b23eaa59`, `34777543`, `1d28c3ef`; `line_editor.rs`, `textdlg.rs`, `tests/it/text_edit_in_place.rs` (231), `dimcon_inline_edit.rs` (342); engine `prompt.rs` `TextEditor` | PULL | Behaviour on the canvas, no chrome; our `text_double_click.rs` test is an untouched clean take. Needs `guard.rs` additions. | M | L |
| A5 | **Edit Attributes dialog** for INSERT (ATTREQ/ATTDIA, Verify, Preset) with the "only a person-started command opens it" rules | `03328362` (F17), `5b00e362`, `ed012ca3`, `21cd2844`, `d3419d15`, `b244b59e`, `df5063f5`, `d759668f`; `attentry.rs`, `tests/it/insert_attrib_dialog.rs` (463), engine `AttribEntry` | PULL | We carry the ATTDIA/ATTREQ sysvars and ATTEDIT but no entry dialog (`attentry` absent). Insert panel behaves like AutoCAD with it. | M | L |
| A6 | **Live Array dialog** for ARRAYRECT / ARRAYPOLAR | `2ecf474d`; `arraydlg.rs` (361), `arraydlg/tests.rs` (450), engine `array_live.rs`, `cmd/array.rs` (+158), `guard.rs` (+33), `tests/it/array_live.rs` (441) | ADAPT | AutoCAD Windows does this as an "Array Creation" contextual ribbon tab. Reuse the engine `ArrayLive` state and render the fields in `ribbon/context.rs`, not in a floating window. | M | M |
| A7 | **Write Block dialog** + `WBLOCKPICK` | `a974b202` (#1778); `wblock.rs` (471), `tests/wblock_dialog.rs` (182) | PULL | Typed/ribbon WBLOCK currently needs a `path` param (engine `cmd/blocks/wblock.rs`); dialog fixes Insert > Block Definition > Write Block. | S-M | L |
| A8 | **Keyboard Shortcuts editor** (any command, user wins, conflict rules, saved in prefs) | `2cbd67f9`; `keys.rs` (699), `tests/it/shortcuts_checked.rs`; `6b767073`, `b7d5ed34` (Options reset keeps shortcuts and folders) | ADAPT | Our CUI (`workspaces.rs`) edits the Quick Access Toolbar, ribbon tabs and workspaces but not keys. Put the dialog in Manage > Customization and as a CUI page. | M | L |
| A9 | **Alias editor**: persisted in the profile, import a `.pgp`, an alias of a UI command opens its dialog | `fd3a0b9e`, `ff6b0e37`, `168f4a69`, `a13cbdc8`; `aliasdlg.rs` (+348), `tests/alias_persistence.rs` (177) | PULL | `aliasdlg.rs` untouched by us: clean take. | S | L |
| A10 | **Canvas input robustness**: clicks count at button-down while moving, press-first picks, slow click makes a grip hot, first corner survives pointer drift, realtime PAN per tile, pickbox-only cursor, no grips on hidden or viewport-frozen objects, double-click hatch opens Hatch Edit | `06e53f3e`, `d75e2a0a`, `2733c9d3`, `fa4b6d13`, `d1d2e578`, `816bd959`, `cc812a55`, `60b0f21e`, `ddfa833b`, `e43c39c5`; `canvas/press.rs` (172), `canvas.rs` (+501), tests `moving_clicks`, `press_first_clicks`, `slow_clicks`, `offset_clicks`, `realtime_nav_release`, `viewport_frozen_grips`, `hidden_object_grips` (about 1,300 test lines) | PULL | Pure canvas; `canvas/*` are clean takes, `canvas.rs` 4 hunks. Nothing here touches the ribbon. | M | L |
| A11 | **TRIM/EXTEND previews** (only the piece a click changes, Fence/Crossing previews) and EXTRIM | `09351655`, `af33469e`, `bb53e4e7`, `3c7427d2`, `14779e26`, `c7e6b10f`, `8db82ebf`; guard methods `highlight`, `highlight_all` | PULL | UI half is canvas highlight bands; engine half goes with the engine analyst's list. | M | L |
| A12 | **Dynamic Input**: Tab wraps to the first box, angle box for rotation grips | `bda2f3db` (#3201); `dyninput.rs` clean, `canvas/dyn_boxes.rs` clean | PULL | Small, clean. | S | L |
| A13 | **Clipboard**: Edit > Paste / PASTECLIP read the system clipboard, a held paste is tied to its request and cancelled by Esc, macOS menu offers Paste, Excel TSV pastes as a TABLE | `2d95af7d`, `a9165037`, `f581688c`, `f24468f2`, `e2eedc19`, `8bc67313`, `57c1d36f`, `fe5b66d1`, `dc143302`, `08b68e8c`, `28ab81b3`, `ff88c83b`; `clipboard.rs` (+247, clean), new dep `craft-csv` | PULL | Real user-visible bug class (paste across drawings). `cmdline.rs` conflicts (11 hunks) with our AutoComplete learning (`cmd_usage.rs`). | M | M |
| A14 | **Right-Click Customization** (Default / Edit / Command modes, click duration) | `811bd3cb` (F20), `80bbcb98`, `d49ea7e5`, `9787da70`; `options.rs`, `options/dialog.rs`, `tests/it/options_right_click.rs` (237) | PULL | AutoCAD Options parity, no ribbon contact. `options.rs` 3 hunks. | S-M | L |
| A15 | Options: reset keeps user's shortcuts and remembered folders; Drafting Settings scrolls on a short screen | `6b767073`, `b7d5ed34`, `92b718f9` (#2441); `tests/it/drafting_settings_scroll.rs` | PULL | Small fixes. | S | L |
| A16 | **Page Setup**: inch/mm paper units, explicit inches when MEASUREMENT=1 | `9c2915c2`, `1df2b829`, `33cb596e` (#3249); `tests/it/page_setup_paper_units.rs` | PULL | `managers.rs` 1 hunk. | S | L |
| A17 | Open/close/save flows: typed `OPEN` in the background, reuse existing tab when reopening a path, Save As suggests DXF after the DWG content-loss warning, pre-2013 DXF solids warning | `c73b1487`, `9b05dbed` (`tests/reopen_drawing.rs`), `fba67a1c`, `ea4a59b6`; `save_loss.rs`, `opening.rs`, `closing.rs` (clean) | PULL | Fewer lost-work surprises, fits "never lose work". | S | L |
| A18 | Shortcut menus: Generate Boundary on a hatch, dimension menu, menu ordering test against the reference menu tree | `0bd4d54f`, `a770fa5a` (part), `e8e76d2e`; `context_menu.rs` (+196, clean) | PULL | `context_menu.rs` untouched by us. The ordering test (`menus.rs`, 3 hunks) keeps our title-bar menus in AutoCAD order. | S | L |
| A19 | **MENUBAR / TOOLBAR** typed commands (hidden bars come back) | `ad072e6c` (#1341), `9c2ebc0e`, `4fadc54d`; engine `cmd/interface.rs`, `tests/menubar_toolbar.rs` | ADAPT | We only have the UI toggle `ui.toggle.menubar`. Same engine command pattern gives **RIBBON / RIBBONCLOSE** (our ribbon minimise state) and a way back when the bars are hidden. | S | L |
| A20 | MTEXT editor and interface show drawing scripts (Thai, Hebrew...) instead of boxes | `0e0344be`, `5c00b665`, `9c53d527`, `c4c632d1`; `drawing_fonts.rs` (131), `mtext_editor.rs` (+244, 1 hunk) | PULL | Needs `fonts::ttf::fallback_for` (fonts analyst). | M | M |
| A21 | UCS icon turns with the view twist, UCSICON origin toggle, grid in UCS | `b86caba6`, `d75bc7fa`, `9f833f3d`; `viewcube.rs` (clean), `menus.rs` `ui.toggle.ucsicon.origin` | PULL (with engine UCS) | Goes with the engine UCS work. | S | L |
| A22 | Interface scale keys Ctrl+= / Ctrl+- / Ctrl+0 | `138f4479` (post-runtime; test uses `framework::install_headless`) | ADAPT | Re-implement over our Options `interface_scale` (0.5 to 3.0); skip the runtime. | S | L |
| A23 | Tab/keyboard traversal between palettes, document tabs and the drawing; Enter/Space keep shortcut menus open | `9e5889b7`, `849e6fd5`, `aac7ae1c`, `e425c71c`, `9d214d6e`; `framework.rs` | SKIP now | All of it rides the craft-ui `next` runtime. Revisit with A24. | L | H |
| A24 | **craft-ui `next` runtime hosting**, dock model, floating panels as OS windows | `8fe5682b`, `180ff447`, `68de3797`, `4f4a3e94`, `d6593157`, `c3f69126`, `d26d9ca0`, `239f8a32`, `09944da2`, `1cff8cd6`, `f4c30180`, `b8442489`, `433e51ab`, `f99ae695`, `c9286726`; `framework.rs` (254), `os_window.rs`, `docking.rs` (+607) | SKIP (revisit when craft-libs `next` lands on its main) | Unmerged upstream branch, new egui fork rev, rewrites `docking.rs` and `lib.rs` which our ribbon, workspaces and CUI sit on. Do the 355 earlier commits first. | L | H |
| A25 | About credits tables, contributor JSON, `build.rs` credit baking | `ef1b55dc`, `c4d9e6ac`, `7a9ec9ba`; `credits.rs`, `contributors/`, `docs/contributors.md` | SKIP | We dropped it after the import (README/NOTICE credit upstream instead). Carries ArtCraft brand files (`docs/brand/artcraft-*`): AGENTS forbids other brand marks. | - | - |
| A26 | Upstream updater dialogs (installing/ready/failed, install at exit) | `3a9aebc2`, `20b4472f`, `3074f189`, `f1b76fab`; `updates_ui.rs` (9 hunks vs our 600 changed lines) | SKIP | We ship `self_update.rs` (757 l) that also installs the Windows setup. Keep ours; steal only the "reports failed" wording if missing. | - | - |

### B. Apps and platform shell

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| B1 | **macOS Finder open** (kAEOpenDocuments), Dock drops, `open -a`, quit as close request; DWG/DXF document types | `7dc9e0ec`, `2a9c14e9`, `5f44be5b`; `apps/*/src/apple_events.rs` (211); crate `fmv-macos-events =0.1.0` (safe wrapper, we keep `unsafe_code = forbid`) | PULL | `docs/releasing.md` of ours says "on macOS a file opened from Finder starts DraftCraft without loading it": we ship a DMG with that bug. | S-M | L (check the crate's licence per AGENTS; macOS-only test) |
| B2 | macOS file dialogs as sheets (winit aborts when a modal panel is resized) | `2f8f1861` (#1534); `sheet.rs` (51), `rfd` parent handle, `pollster`, winit `rwh_06` | PULL | Crash fix; "never crash" outranks features. | S-M | M |
| B3 | **Windows single instance**: files opened while running become tabs (AF_UNIX socket per sign-in, `--new-window`) | `2ed5614e` (#2561); `single_instance.rs` (431); crate `uds_windows`; tests `ed294b3f`, `67cc9a28`, `3caad662` | PULL | Our setup registers `.dxf/.dwg` with Open With, so every Explorer double-click spawns a window today. | M | M (Win10 1803+, short socket paths) |
| B4 | **Windows portable ZIP**: `portable.txt` beside the exe, settings in `DraftCraftData` | `8b6c204f` (#731), `8c7bfb3e`, `2c8b8f43`, `ab214cf2`; `portable.rs` (100), `env.rs`, `package.ps1` (+2) | PULL | We ship `draftcraft-windows-x64.zip`; CONFIGEXPORT is not the same thing. | S | L |
| B5 | `DRAFTCRAFT_STENCIL=0` for drivers that show a black window | `11cbaf36`; `env.rs`, `main.rs` | PULL | Cheap support escape hatch. | S | L |
| B6 | Log lines cannot forge records (control chars escaped, continuation lines indented) | `bc9ad6b0`; `logging.rs` (+25, clean) | PULL | Hostile text from files reaches logs. | S | L |
| B7 | Nightly channel, launch profile, profile root, process identity, build flavor, co-installable Linux nightly | `7e95cfc8`, `29baf193`, `cad62242`; `launch_profile.rs`, `profile_root.rs` (607), `nightly_session.rs`, `build_channel.rs`, `build_flavor.rs`, `process_identity.rs`, `packaging/nightly-*.py`, `packaging/macos/nightly-*.sh` | SKIP | storytold's signed-nightly pipeline (craft-common-actions pins, `nightly-signer` branch). Our side-by-side install with CADCraft was solved by upgrade codes (#35). | L | M |
| B8 | CLI polish: help flush errors, roundtrip progress on the supplied stderr, undrawn-entity note on image export, PDF as input | `dbef5a6b`, `22c03def`, `cf6046cc`, `924a37ab`, `3d9b5f61`, `a8d05d68`; `apps/*-cli/src/{output,error,run,lib,args}.rs` | PULL | `output`, `error`, `run` are clean takes; `args` and `lib` merge without conflicts. | S | L |

### C. MCP, control channel, CLI

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| C1 | **Automation roots**: `draftcraft-cli mcp --automation-read-root DIR --automation-write-root DIR` confine every file a headless agent can read or write; unclassified commands are refused; a "walk the registry" test fails when a new command is not classified | `46ee1a81`, `09e4f907`, `8a42e8a1`, `8314fb87`, `290e578c`, `1a64ca23`, `f09c50d6`, `0f33f430`, `43d7a192`, `387a6e6b`, `d4c4c799`, `8cfb9cfa`, `8b5503e8`, `f097a688`, `e10109e2`, `8cbade6e`; engine `file_access.rs` + `file_access/commands.rs`; tests `apps/*-cli/tests/mcp_roots.rs` (404), `mcp_roots_walk.rs` (749) | ADAPT | Strongest agent-safety item (hostile prompt writes `/etc` or `~/.ssh`). Headless only (running app is not confined: upstream issue #1585). Our extra commands (CONFIGEXPORT/IMPORT, ETRANSMIT, ATTOUT/ATTIN, PDF underlays, LAYTRANS, AREATOTAL...) all need classification. Make it opt-in first. | L | M |
| C2 | Control token goes only to `127.0.0.1:PORT` and is read only from a private file | `6d744887` (#2718); `mcp/src/{backend,control_token}.rs` (clean) | PULL | CWE-522 fix, 3 files, with test. | S | L |
| C3 | Control request parser hardening: bounded id, params decoded after authentication, unknown keys skipped, bounded parsing | `b52f0a57`, `2fffb31d`, `b284371c`, `a87ec8a5`, `7e780548` (control part); `control_server.rs` (+130), `control.rs` (+222) | PULL | Unauthenticated-peer memory abuse; `control_server.rs` is untouched on our side. | S | L |
| C4 | MCP `find_text` accepts `query`; `stylesmanager set` replies with what changed | `152e3b56`, `39a2e4da` | PULL | One-liners. | S | L |
| C5 | MCP protocol through shared `craft-mcp` / `craft-jsonrpc` | `8a4e2754`, `a20a46f7` | SKIP | No user benefit; welds our stdio server to craft-libs revisions. | M | M |
| C6 | `render` background / plot_style | `017cc9b1`, `04a279cf` | already ours | In `docs/mcp.md` and `engine/render_look.rs`. | - | - |
| C7 | Page command API `window.<app>` for the web build, `ui.screenshot` PNG inline on wasm | `85b35fc5`, `14a6cc3c`, `07b6b8c5`; `apps/*-web/src/api.rs` (61), crate `craft-web-api`; `control.rs` `PendingReply::try_reply` | ADAPT | Name the global `window.draftcraft`; pins at `a231e517`. Our web already has host mode, PWA manifest and `sw.js`, which upstream lacks. | S-M | L |
| C8 | **New control/MCP methods upstream added since base: none** (only parser/limits changes); we add `ui.modifiers`. | `docs/control-protocol.md` diff | n/a | Nothing to port protocol-wise. | - | - |

### D. i18n

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| D1 | **Add Polish, Georgian, Tamil** (ours: 18 locales; upstream 20) | `deff6cb0` (#218), `08dfe1ed`; `6c729993`, `93e4ff10`, `096e2062`, `4b532f6c`; `aa3d4635`, `23abe2ed`; `i18n.rs`, `theme.rs` (`tamil_font`), `crates/fonts/{build.rs,src/craft_fonts.rs}`, `i18n/{pl,ka,ta}.tsv` | PULL | pl 2,199 rows (1,428 apply to our key set), ka 1,473, ta 1,467 of 1,771. ka needs the craft-fonts `Geor` face, ta an installed Tamil font or `Taml`; each file needs an `ATTRIBUTION.md` row (`cargo xtask assets`). | M | L |
| D2 | Catalog corrections for existing locales | `7064a685` (Greek layout typing + 46 rows), `c17cbc9c`, `4e482cf6`, `53cebb90`, `17da6ab3`, `7b8ca639`, `2dd428dc`, `8725cc48`, `32d5bd85` | PULL (merge tool) | Rows that fill our gaps: el +47, sv +21, uk +17, zh +3, others +1 to +2. A 20-line script over the `.tsv` files. | S | L |
| D3 | i18n tests (+182 l): provenance, plural forms, Arabic words test, Thai in both families | `i18n/tests.rs`, `4683023d`, `5c00b665` | PULL | `i18n/tests.rs` 3 hunks vs our zh_tw tests. | S | L |
| D4 | Translate about 500 new English strings for the pulled features | upstream only has it 56 / zh 51 of them; sv 719, pl, uk 251 | (our job) | Needed for A1-A8 etc. before each PR; AI-assisted per our catalog header. | M | L |

### E. CI, packaging, xtask, test tooling

| # | Item | Upstream commits / files | Verdict | Why | Effort | Risk |
|---|---|---|---|---|---|---|
| E1 | **ast-grep workflow** | `.github/workflows/ast-grep.yml` | PULL | We carry the rules and their rule-tests (`lint/ast-grep/`, identical to upstream) and `docs/cli.md` says "CI runs it", but our `.github/workflows/` has no such workflow (only ci, packages, release, web-deploy, web-sync). | S | L |
| E2 | Dependabot (cargo + actions, grouped, daily) | `.github/dependabot.yml` | ADAPT | Drop their egui/eframe `ignore` block (we pin forks). | S | L |
| E3 | Harden-Runner on every job; SHA-pinned actions | `ecb362aa` (#3058) | ADAPT | Ours: 18 `uses:` on floating tags (`checkout@v4`, `rust-cache@v2`, ...) and `craft-common-actions/gate.yml@main`; only 3 SHA-pinned (in `web-deploy.yml`). | S | L |
| E4 | Packaging lint (workflow syntax, shellcheck, desktop-file, AppStream, Flatpak, WiX policy job) | `packaging-lint.yml` | ADAPT | We have `.shellcheckrc`, `check-install-policy.ps1`, `xtask/src/packaging.rs` tests but no lint job. | S-M | L |
| E5 | Scheduled **release dry run** on main | `2ef7c3be`, `25bfb600` | ADAPT | Add a nightly unsigned `packages.yml` run: finds release-only breakage before tag day. | S | L |
| E6 | Windows ARM64 install test on a real ARM runner | `windows-arm64.yml` | ADAPT | We cross-compile arm64 setup/zip but never run them (`packages.yml` installs x64 only). | M | L |
| E7 | FreeBSD VM, riscv64 tarball, Nix flake, git-sources check, SonarCloud, house-rules, nightly-sign, CodeRabbit | `freebsd.yml`, `nix.yml`, `sonarcloud.yml`, `house-rules.yml`, `nightly*.yml`, `.coderabbit.yaml` | SKIP (git-sources idea: ADAPT) | We do not ship those targets / rely on those org services. Keep only the idea of a check that git deps come from allowed owners. | S | L |
| E8 | **Display-name guard** (`tests/display_names.rs`, 215 l: scans string literals for banned product names) and the rename Express Tools -> Additional Tools | `fa7a9772` (#2945); `docs/feature-names.md` | PULL | We still ship `Tools > Express Tools` (7 occurrences in 6 engine files, e.g. `cmd/modify2.rs:68`, `cmd/txt2mtxt.rs:21`) and a CHANGELOG line using it. Brand hygiene is a stated non-negotiable. Needs menu-catalog and zh/it row updates. | S | L |
| E9 | Test rules from upstream AGENTS (assert relations not values, hostile-input sweep per crate, one `tests/it/main.rs` per crate, a11y checks) | AGENTS.md "Tests" rules 27-32, 21 | ADAPT (doc only) | We already follow most; add the sweep/relations rule to our AGENTS. | S | L |
| E10 | xtask | `xtask/src/*` unchanged since base except `stats.rs`, `layers.rs` (-1 l) and a new `tests/dwg_dependency_targets.rs` (DWG via craft-libs: n/a) | SKIP | Nothing to take; ours has more (`packaging.rs`, `version set` with CHANGELOG). | - | - |

### F. Docs

| # | Item | Verdict | Why | Effort |
|---|---|---|---|---|
| F1 | `docs/{ui-parity,gaps,target-app-parity,geometry-parity,file-format-parity,hardware-parity,roadmap,localization-parity,parity-checklist}.md` (66 bookkeeping commits) | SKIP (mine `gaps.md` for backlog ideas only) | Agent-written scoring built on AutoCAD-for-Mac observations; `docs/ui-parity.md` names UI gaps upstream still has (lasso selection, grip hover menus, navigation bar, workspaces); we already have the last two, so it is a weak backlog source for us. Our `xtask parity` writes `docs/parity-checklist.md`, which is not committed. | S |
| F2 | `docs/pdf-import.md`, `docs/mcp.md`, `docs/localization.md`, `docs/control-protocol.md`, `packaging/web/README.md` updates | PULL with the matching features | Our `docs/mcp.md` has duplicated `shapes` and `query_entities` rows to clean while merging. | S |
| F3 | `docs/nightly-release-preparation.md`, `docs/egui-037-migration-plan.md`, `docs/contributors.md`, `docs/brand/artcraft-*` | SKIP | Upstream-specific; brand marks must not enter our repo. | - |

---------------------------------------------------------------------------------------------------------------

## 2. Non-ribbon upstream UX worth keeping inside our ribbon

The idea: take the engine-side contract and the canvas behaviour from upstream, host the controls in our ribbon.

| Upstream idea | Where it sits in our ribbon / chrome | How |
|---|---|---|
| Blocks palette (previews, libraries, repeat placement) | Insert > Block panel button "Blocks Palette" (`ribbon.rs:123`) and Home > Block drop-down; later a recent-blocks gallery | Palette stays a docked `Panel`; gallery reads the same preview cache and starts INSERT through `app.start("insert")` |
| `properties.describe` rows | Properties palette and ribbon contextual tabs (Text Editor, Dimension) | One JSON source for palette, ribbon drop-downs and MCP `execute properties.describe` |
| Dimension Style Manager | Annotate > Dimensions: style drop-down + "Manage..." and the panel's expand arrow | Dialog opened via `route()` in `managers.rs`; JSON form stays dialog-free |
| Live Array | **Array Creation** contextual tab (`ribbon/context.rs`) | Engine `ArrayLive` state drives the tab fields, preview stays on canvas |
| Write Block, Edit Attributes | Insert > Block Definition panel (Write Block), INSERT flow | Dialogs, hide-for-pick pattern from `HATCHPICK` / `WBLOCKPICK` |
| Keyboard Shortcuts editor | Manage > Customization panel and the CUI dialog | One more CUI page next to QAT / tabs / workspaces |
| MENUBAR / TOOLBAR typed commands | New **RIBBON / RIBBONCLOSE / RIBBONSTATE** commands, same `cmd/interface.rs` pattern | The way back when the ribbon or title-bar menu is hidden; scriptable |
| Right-Click Customization, Options reset keeps shortcuts | Options dialog (already ours) | Logic only |
| In-place TEXT/dimension editing | Annotate > Text; contextual "Text Editor" tab for MTEXT is ours already | Same prompt-answering design (`TEXTEDIT`) so undo is one step |
| TRIM/EXTEND previews, press-first clicks, hot-grip slow click | Modify panel buttons just work better | No UI work |
| Interface-scale keys | View > Interface panel / Options | Over our `interface_scale` |
| Reference-menu ordering test | Title-bar menu bar | Keeps our Windows-style menu order faithful |

Smoke test worth writing for the ribbon (ours has 10 ribbon tests, one of which is `every_ribbon_command_exists`): click
every ribbon button id with no parameters and fail on "is required" / "not available yet" echoes. That finds the
Dimension Style class of dead buttons mechanically.

---------------------------------------------------------------------------------------------------------------

## 3. Where we are ahead, where we are behind (with evidence)

### Ahead
1. **Ribbon and Windows-style chrome.** `ribbon.rs` 2,033 lines + `ribbon/context.rs` 309, 298 `Icon::` uses, `icons.rs`
   1,425 lines (upstream 1,201, no ribbon at all: menu bar + Tool Sets strip), `window_frame.rs` (menu in the title bar),
   `workspaces.rs` + CUI (`tests/it/workspaces_cui.rs`), the canvas navigation bar (`canvas/widgets.rs:91`; upstream's
   `docs/ui-parity.md` lists workspaces and the navigation bar as missing for them), Start page with units, `chrome/units.rs` status-bar units.
2. **Distribution and release process.** Ours: `apps/draftcraft-setup` (Windows `.exe` wrapping the WiX MSI, updates
   itself), `.rpm` and pacman packages (upstream has neither: `pacman` appears in 0 upstream packaging files),
   AppImage via `appimage.sh`, DMG layout generator, `packages.yml` (386 lines) that installs and removes the
   `.deb`, `.rpm` (Fedora container), Arch package (Arch container), Windows setup (including update over 2.2.2) and checks
   the macOS app on every packaging PR; in-app **Update Now** (`self_update.rs`, 757 l); in-app **What's New** from
   `CHANGELOG.md` with `cargo xtask version set` and the `the_changelog_has_notes_for_this_version` test (upstream has no
   CHANGELOG at all); `docs/releasing.md` 128 lines + `release-handoff.md`.
3. **Native DWG codec and real test drawings.** `crates/dwg` 20.7k lines, 126 tests, CHANGELOG 2.2.0 "reads and
   writes DWG with its own code, 2000 to 2018". Upstream reads DWG through a child worker (`dwg_worker.rs`, acadrust /
   craft-dwg). In-tree: 66 AutoCAD-saved DWG/DXF drawings (`io/tests/data/{acad-solids,mi201}`), each with an
   `ATTRIBUTION.md` row, against 1 example upstream; upstream's own `docs/gaps.md` lists "no real-file test corpus"
   as its #2 alpha blocker. We also carry `dwg_bench`.
4. **Translation depth where it counts for us.** Share of our own key set translated: it 94%, zh 92%, **zh_tw 92% (upstream has no
   Traditional Chinese)**, average over 18 locales 76%. Upstream: it 62%, zh 66%, average over 20 locales 64% (pl 96%, sv 97%
   are its best); each repo measured against its own key set. Enforced by AGENTS (Italian per string, Chinese for menus). Counterpoint: upstream has pl, ka, ta that we lack (D1).
5. **Features upstream does not have:** DesignCenter, Tool Palettes, Sheet Set Manager (`palettes/designcenter.rs`,
   `toolpalettes.rs`, `sheetset.rs`), LAYTRANS / LAYWALK / geolocation (`managers/layertools.rs`), eTransmit, point
   clouds, PDF underlays with contextual ribbon tabs, ATTOUT/ATTIN, AREATOTAL, CONFIGEXPORT/CONFIGIMPORT, AutoComplete
   that learns (`cmd_usage.rs`), saved workspaces. None of these command ids exists in upstream's engine.
6. **Licence and brand hygiene.** `LICENSE-MIT`/`NOTICE` keep upstream's copyright; README fork disclosure and
   "not affiliated" lines (README 50-54); `docs/brand/{LICENSE-brand.txt,trademark-assignment.md}`;
   `cargo xtask assets` plus 151-row `ATTRIBUTION.md`; no ArtCraft marks (upstream's `docs/brand/artcraft-*`
   stay out); legacy-upgrade-code tests so DraftCraft never replaces CADCraft (`xtask/src/packaging.rs`).
   Gaps in this area: E8 ("Express Tools"), and `web-deploy.yml` still names the `withshalom/draftcraft-app` repository
   though the trademark moved to DraftCraft Engineering Ventures (88be1d30).

Not an advantage (checked): **panic guard** (identical base; upstream 345 vs our 267 lines of guard, same `panic_hunt.rs`, 134 vs 126
matches of `hostile` in test code; our edge is only 17 vs 14 crate roots carrying `deny(clippy::unwrap_used,...)`).
**Docs volume:** upstream 4,436 lines in 24 files, ours 1,843 in 15; ours wins on DWG internals (618 lines), theirs on parity bookkeeping.

### Behind
| Area | Ours | Upstream | Evidence |
|---|---|---|---|
| UI tests | 465 | 770 | `#[test]` count in `crates/ui-egui`; apps 142 vs 224; whole repo about 3,870 vs 5,110 |
| macOS Finder open / Windows single instance / portable zip | missing | present | B1, B3, B4; our own `docs/releasing.md` admits the macOS gap |
| Blocks palette, Dimension Style Manager, Write Block, Edit Attributes, Array dialog, in-place text, Keyboard Shortcuts | missing | present | A1-A8 |
| Agent safety | no roots, token sent by hostname, lenient control parser | roots, loopback-only token, bounded parser | C1-C3 |
| Languages | 18 | 20 (pl, ka, ta) | D1 |
| CI breadth | gate, packages, release, web | + ast-grep, packaging-lint, windows-arm64, freebsd, nix, nightly dry run, Harden-Runner, dependabot, SonarCloud coverage | E1-E7 |
| Release targets | MSI/setup x64 + arm64, deb, rpm, pacman, AppImage, DMG (x64, arm64) | + Flatpak bundle, FreeBSD, riscv64, Windows x86 MSI, web zip, AppImage zsync | `docs/releasing.md` of both |
| Shared-action dependency | `ci.yml` calls `storytold/craft-common-actions/.../gate.yml@main` | same | Both float on a third party's `@main`; ours has no way to see what the gate runs |

---------------------------------------------------------------------------------------------------------------

## 4. Quality infrastructure: comparison and what to adopt

| Topic | Ours | Upstream | Gap / recommendation |
|---|---|---|---|
| Gate | `cargo xtask ci`: fmt, clippy -D warnings, nextest, i18n, assets, layers, wasm (same as upstream; xtask `main.rs` unchanged since base) | same, through shared gate | Same. Add Windows and macOS test legs (nightly): single-instance, Apple events, sheets, portable code is `cfg`-gated and Linux CI cannot compile it. |
| Real-file corpus runner | `draftcraft-cli roundtrip`, `cargo xtask corpus`, `DRAFTCRAFT_CORPUS_DIR`, `DRAFTCRAFT_CORPUS_STRICT=1` (`crates/io/src/roundtrip/tests.rs:488`) | identical (renamed), plus `CADCRAFT_CORPUS_REPORT`; corpus in `storytold/cadcraft-corpus`, not in CI either | We already have the harness and 66 in-tree drawings. Missing: a corpus repo and a scheduled job. Create `draftcraft-corpus` (files with attribution), run `cargo xtask corpus --strict` nightly, upload `report.md`, fail on a regression in the count, not on zero. Also copy upstream's `CADCRAFT_CORPUS_REPORT` env. |
| Hostile-input tests | 126 `hostile` matches in test code, `panic_hunt.rs` (every command with adversarial params, per-command deadline), `hostile_dxf_does_not_panic` | 134, same files | Parity. Adopt AGENTS rule 29: a crash fix adds its input to the crate's sweep. Add a sweep for `control_server` lines and the new dialogs' parse paths. |
| Fuzzing | none | none | Add `fuzz/` (cargo-fuzz, outside the workspace) with targets: DXF reader, `draftcraft-dwg` reader, MTEXT parser, LISP reader, control request line parser, units/point parser. Seed from the 66 in-tree files and the `hostile_*` inputs. Nightly job, 10 minutes per target; crashes become regression tests. Effort M, risk L. |
| Property tests | `proptest` only in `xtask stats` (no dependency in any `Cargo.toml`) | same | Add proptest to `geom` (intersections, offsets, bounds), `units` parsing, DXF write->read invariants and undo/redo identity. Effort M. |
| Golden-image tests | none (`egui_kittest` used in 42 files for behaviour only) | none (59 files) | Snapshot the ribbon (our signature UI) in light/dark and each tab with `egui_kittest` snapshots, Linux only, tolerance set; also gives the `ui.screenshot` + a11y-tree check upstream AGENTS rule 21 asks for. Effort M, risk M (font drift). |
| Perf | `draftcraft-cli perf N|FILE`, `dwg_bench`, `export_bench`; no gate | `perf`, `export_bench`; no gate | Add `cargo xtask perf`: run `perf 200000` and `dwg_bench` on the CI runner, compare ratios inside one run (open time vs entity count, zoom 1x vs 4x) rather than absolute milliseconds (AGENTS 28: no timings). Nightly, non-blocking first. Effort S-M. |
| Supply chain | no `deny.toml`, no audit; git deps from `storytold/*` | shared `git-sources` check (`bf581c23`) | Add `cargo deny` (licence allow-list per AGENTS, bans unknown git sources) and `cargo audit` on schedule. Effort S. |
| Coverage / mutation | none | SonarCloud (org service) | `cargo llvm-cov` nightly summary artifact; `cargo mutants` monthly on `geom`/`doc`/`units`. Effort S-M. |
| Static rules | `lint/ast-grep` rules + rule-tests, **no workflow** | workflow present | E1. |
| CI hardening | 18 floating action tags, no Harden-Runner, no dependabot | SHA pins, Harden-Runner, dependabot (cargo + actions) | E2, E3. Also vendor or SHA-pin the shared gate (`ci.yml`). |
| Packaging checks | install/uninstall of deb, rpm, pacman, setup on PR | + lint job, ARM64 install, nightly dry run, FreeBSD/riscv64 smoke | E4-E6. |
| Brand/display guard | none | `display_names.rs` | E8. |

---------------------------------------------------------------------------------------------------------------

## 5. Suggested order (all PRs to main; nothing here is 3D)

0. **Hours:** E1 ast-grep workflow, E2 dependabot, E3 pin + Harden-Runner, E8 display-name guard (+ rename), C2, C3, C4, B5, B6, A9, A17, A12, A15, A16 (clean takes; one PR each or one small "clean takes" PR with the CHANGELOG lines).
1. **Platform gaps users hit daily:** B1 + B2 (macOS), B3 (Windows), B4 (portable).
2. **Repin craft-libs to main `453b0da782`** (Cargo-only; add `craft-csv`, `craft-web-api` when needed), then A10, A11, A13, A14 (canvas, previews, clipboard, right-click).
3. **Feature PRs, ribbon-hosted:** A2+A3 (Dimension Style Manager + describe), A1 (Blocks palette), A4 + A5 (in-place text, attributes), A6 (Array contextual tab), A7, A8, A19.
4. **i18n:** D1 (pl, ka, ta), D2, D3, D4 in the same PRs as the features.
5. **Agent safety:** C1 behind a flag, C7, E5/E6 CI items, quality additions from section 4.
6. **Defer:** A23/A24/A22 until craft-libs `next` is on its main and we choose to follow it.

Mechanics that worked in the trial: rename-normalise upstream (`cadcraft->draftcraft`, `CADCraft->DraftCraft`, `CADCRAFT->DRAFTCRAFT`,
`apps/cadcraft*` dirs), then `git merge-file -p ours base theirs` per file with base = `fc157ea9`, theirs = `414e5442`
(trees extracted to `analysis/base_fc` and `analysis/up_prenext`). Commit messages should name the upstream hash
("From storytold/cadcraft@<hash>"); keep the MIT/Apache notices; each user-visible change needs its `## Unreleased`
CHANGELOG line, it.tsv rows and, for menus, zh.tsv and zh_tw.tsv rows; every new `.tsv`, fixture or asset needs an `ATTRIBUTION.md` row.
New engine trait methods (`array_live`, `highlight`, `text_editor`, `attribute_entry`) need the `guard.rs` wrappers, or the "never crash" guard has a hole.

---------------------------------------------------------------------------------------------------------------

## 6. Files kept in the analysis folder

| File | What |
|---|---|
| `ui-infra.md` | this report |
| `ui_commits_fc.txt` | the 387 non-merge upstream commits in my areas since `fc157ea9` (hash, date, subject) |
| `merge_try_prenext.txt`, `merge_try.txt` | per-file 3-way merge trial (snapshot `414e5442`, and full upstream HEAD) |
| `file_divergence.txt` | per file: lines changed upstream vs lines changed by us since the base |
| `fork_scan.txt` | blob-match scan that located the fork base |
| `base_fc/`, `up_prenext/`, `up_all/` | rename-normalised trees (`crates`, `apps`, `xtask`) of `fc157ea9`, `414e5442`, upstream HEAD |

(Other analysts' folders in the same directory are untouched.)
