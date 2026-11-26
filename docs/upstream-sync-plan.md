# Upstream sync plan: DraftCraft and CADCraft

Status: plan, written 2026-11-26 from five read-only analyses. Nothing in it has been built or
compiled; every figure is an analyst's measurement or a regex scan unless it says "I reproduced".
Full reports (with per-commit tables) are in `docs/upstream-sync/analysis/`.

Upstream is `storytold/cadcraft` (MIT OR Apache-2.0, 0.21.0 at 2026-11-26, about 40 commits a day).
We have no shared git history with it: our history starts with a squashed import (`77c95f8`, 2026-11-07).
That rules out `git merge`; everything comes over as brand-rewritten snapshots merged per file, or as
hand ports.

## 0. Decisions the owner must make before any pull

These block work; each says what it blocks. Agents do not decide them.

| # | Question | Blocks |
|---|---|---|
| D1 | Upstream commit messages say behaviour was "measured in AutoCAD 2024" (accoreconsole, plots). `AGENTS.md` allows observing AutoCAD black-box. Is pulling commits whose behaviour came from such measurements acceptable? Affected: linetype dash layout (`a303ff43` family), solid-hatch DXF groups (`467ab20a`), many engine fixes. | the engine, render and io PRs |
| D2 | Commit `83a2b4c3` changes the ACI grey values and states no source. Pull it or not? | colour PR |
| D3 | 50 of our solid-model test fixtures were made with an AutoCAD educational licence. Do its terms allow keeping them in an open repository? | nothing upstream; a legal question about files we already ship |
| D4 | `craft-pdf` (already pinned in our `Cargo.toml`) pulls `hayro-interpret` and `hayro-cmap`: Foxit `.pfb` fonts, Adobe AFM metrics and CMaps. Our `NOTICE` and `ATTRIBUTION.md` never mention them, and `AGENTS.md` says no Adobe assets "ever". Rule on it: carve-out plus notice rows, or remove. | PDF work; an existing gap, independent of upstream |
| D5 | Upstream moved `dxf` and `dwg` into `craft-libs` (`craft_dxf::`, `craft_dwg::`). Options: (A) adopt `craft-dxf` and drop our `crates/dxf`, (B) alias it to ours (valid only if the API matches), (C) freeze that lane and port behaviour by hand. Recommended: C, because the DWG codec is our differentiator and `craft-dwg` is built on `acadrust` (MPL-2.0, whose authors say they cross-checked against LibreDWG, GPL). | the io PR |
| D6 | Upstream renamed "Express Tools" to "Additional Tools". We keep AutoCAD's names (we still ship "Tools > Express Tools" in 7 places). Confirm we keep ours. | UI PR |
| D7 | Upstream's main UI moved to an unmerged craft-libs `next` runtime at `b55abde2` (#3449, 2026-11-25). Recommended: sync to the last state before it (`414e5442`, 862 of the 915 commits) and defer the runtime port as its own decision. | every ui-egui PR |
| D8 | Mirrors: create org-owned mirrors of `storytold/craft-libs`, `storytold/egui` and `storytold/winit` (see section 2). | dependency PR |
| D9 | Contribute fixes back upstream? Any outward PR is a publishing action and needs a go-ahead each time. | section 8 |

## Decisions made by the owner (2026-11-26)

- **D5 decided: we do not take upstream's DWG or DXF reader or writer.** Our native codec stays. Their I/O fixes are
  only ported by hand, case by case, and nothing that needs `craft-dxf`, `craft-dwg` or `acadrust` is pulled.
- **D4 decided: remove the bundled standard PDF fonts.** Done; see `docs/clean-room-review/2026-11-26-bundled-pdf-fonts.md`.
  The Adobe CMaps and the width tables are still open for the partners.
- **Dynamic blocks first, then the ZWCAD parametric track** (section 3.5). Step 1 is done (section 9).
- **Only pull what improves ours and keeps us ahead.** UI items are taken only where they fit our ribbon.

## 1. Where we stand

Facts checked by me: version 2.2.3; 480 vs 482 registered commands, 458 shared; 3,840 vs 5,075 test
functions; 311k vs 327k lines of Rust; both open all 68 corpus drawings with no crash.

Fork base (analysts agree on 2026-11-05): closest upstream commit `df659744`; `1b39fbba` and `fc157ea9` are within a file.
Use `df659744` as the base. Since then: 915 non-merge upstream commits (about 150 to 185 a week) against 120 of ours.

Churn by file (mechanics analyst, tree-level 3-way): 782 files changed upstream; 573 (73%) clean, 209 (27%) hot.
- Clean: constraints, color, mcp, fonts, render, geom, about 85% of engine, 106 of 163 ui-egui files. 479 files / 68.7k lines are portable as they stand; 84 reference `craft_dxf`/`craft_dwg`/`acadrust` and need adapting.
- Hot: io (29 conflicts), ui-egui (24: `cmdline.rs`, `docking.rs`, `lib.rs`, `main.rs`), `crates/dwg`, `crates/dxf`, root files, docs. Worst: `dwg/lib.rs`, `io/pdf_read.rs` (PDF import built on both sides), `engine/select.rs`, `geom/nurbs.rs`.
- Whole-tree 3-way merge: 106 conflicted files, 217 hunks; about 8 to 14 working days to catch up.
- Commit-by-commit replay does not work: of all 915 only 208 apply in order (23%). Windowed tree merges do (86% of files merge cleanly).
- Rebrand rules reproduce our existing rebrand exactly (823 files identical); every replay failure was a missing prerequisite commit, none was branding.

### Where we are ahead

1. **Native DWG codec** (`crates/dwg`, `crates/io/src/dwg`): no third-party codec, runs in WASM (upstream's cannot), reads 2.3 to 2.8 times faster with lower memory, reads and writes the dynamic-block graph natively, 66 real attributed drawings in tree against 1 upstream, 1,500 DWG and 540 DXF fuzz mutants with 0 crashes (upstream also 0). No MPL dependency.
2. **Windows-style ribbon and chrome**: `ribbon.rs` about 2,000 lines, navigation bar, workspaces, CUI. Upstream has no ribbon.
3. **Packaging and release**: Windows setup, `.rpm`, pacman, AppImage, DMG, install tests, self-update, CHANGELOG-driven What's New. Upstream has no CHANGELOG and no pacman.
4. **Commands upstream lacks (22)**: `adcenter areatotal attin attout baction bparameter conic configexport configimport etransmit geographiclocation georemove imageadjust imageclip laytrans laywalk pdfadjust pdfattach pdfclip pointcloudattach sheetset ulayers`. Also DesignCenter, Tool Palettes, sheet sets, PDF underlays.
5. **Dynamic-block authoring** (`doc/dynblock/author.rs`, `engine/.../bauthor.rs`): upstream has only the reference side.
6. **Plot output**: Béziers kept in plotted PDFs (12 `c` operators against 260 `l`, 1.3 KB against 3.0 KB); PDFIMPORT arc recovery; picking by text boxes.
7. **Localization**: Italian at 94%, Traditional Chinese (absent upstream).
8. **DWG fidelity on write**: dxf to dwg loses far fewer records than upstream on the shared corpus (30 against 688 on 2D, 458 against 2,572 on 3D).

Not a differentiator: crash safety. `guard.rs` and `panic_hunt.rs` are the same as upstream's at the base, and upstream has since added hardening and about 725 tests in the engine area against our 94. Our own reproduced crash is listed in 3.0.

### Where we are behind

Hardening and DoS bounds; handle handling; name lookups at scale; AutoCAD-style plot output and units; Latin accents in the stroke font; UCS (we have none); dynamic-block execution (about half the action coverage); Dimension Style Manager (our ribbon "Dimension Style" button errors with "name is required", analyst finding, not yet reproduced by me); Blocks palette; macOS Finder-open; Windows single instance; UI tests (465 against 770); quality infrastructure (section 6).

A correction to my earlier benchmark message: the round-trip tool reported our DWG to DXF loss as about 3.5 times worse. The I/O analyst finds 97% of that count is header variables (our DWG reader fills 197, our DXF writer keeps about 50; upstream's reader never held them). It is mostly a comparator artefact, with the real bugs listed in 3.0.

## 2. Repositories

Nothing needs a mirror to read upstream: it is public and agent sessions read it anonymously.
A mirror is for pinning and staging.

- **Source mirror**: use `withshalom/artcraft-cadcraft` (public fork, push access, currently at `ba344a89`; upstream is further ahead). Keep it fast-forward-only from upstream, with nothing of ours in it, and disable Actions on it (upstream's workflows would run there).
- **Public or private**: a fork of a public repo is public on GitHub. A private copy is a fresh private repo plus `git push --mirror` (no "Sync fork" button). MIT/Apache allow either as long as `LICENSE-*` and `NOTICE` travel with it. Public is simpler for agents: private repos are not readable anonymously and need the Claude GitHub App installed on them and a per-session attach.
- **Dependency mirrors (D8)**: our `Cargo.toml` pins `storytold/craft-libs` at three revs (`92d392cf`, `c5a1cbfb`, `2e2312dd`) and uses `storytold/egui` and `storytold/winit` forks. A force-push or deleted rev there breaks our builds with no change on our side. Mirror each pinned rev into org-owned forks, point the git URLs at them, record the revs in the ledger, and move to a single craft-libs rev as upstream's own rule asks.
- **Ledger and rewritten patches** live in the DraftCraft repo (`docs/upstream-sync/`), not in the mirror.
- **Vendor branch**: local `vendor/cadcraft`, never merged; each commit is the brand-rewritten snapshot of one upstream SHA. It may be pushed to our org as a plain branch to keep the merge base if upstream rewrites history.

## 3. Work, in order

Land each item as its own PR (agents never merge). Every crash or data-loss fix needs a regression test
(`crates/engine/tests/panic_hunt.rs`, `hostile_dxf_does_not_panic`). `cargo xtask ci` before every commit.
A user-visible change adds its CHANGELOG `## Unreleased` line.

### 3.0 Our own defects (no upstream needed to start; do first, "never crash" outranks features)

| Item | Evidence | Fix source |
|---|---|---|
| Display point budget: a 2 KB DXF array makes our CLI abort with an 8.6 GB allocation in `push_polyline` | analyst reproduction; upstream finishes in 1.5 GB | `fba309f8`, `80c6e074`; prerequisites `7804e94e`, `2410a1fd`, `a1574780`; batch the atomic counter per thread |
| Handle band: a DXF with a duplicate or top-of-range handle silently drops entities (saves 1 entity in ours, 3 upstream) | reproduced by two analysts | `bcac20dd`, `140437e7`, `a516ce11`, `8ec1e9a0`, `76420454`, `44f02683` |
| DoS bounds bundle and LISP always-yields | analyst | `7804e94e`, `95fdf8d1`, `08437703`, `bc9ad6b0`, `2c25c2d7`, `7e780548`, `5a6aa9b6`, `b460a3b8`; `ed855587`, `44b0399a`, `ea2637ae` |
| Layer and block name index: 60k layers plus 60k missing-block INSERTs takes 32.9 s (1.3 s upstream); display build 1.8 s against 9 ms at 30k layers | analyst reproduction | `3532ddf6`, `04c880e9`, `bc5749ec`, `a34ebd0b` (`names.rs`) |
| Root-dictionary open is quadratic: 60k entries 16 s (0.7 s upstream); 400k-element linetype makes PNG export 8.7 s | analyst | `d6d16bd4`, `7867366a` |
| `properties.set` ignores locked layers | reproduced by the engine analyst | `0dcdf8d7` |
| BREAK, TRIM, JOIN and PEDIT lose handles | engine analyst | `156d117d`, `4c875d94`, `061ecd1c`, `3c9b4be3`, `e20f5ed5` |
| DXF read is 13 to 25% slower (we deep-copy every tag; three copies of the tag list; 414 MB against 263 MB at 100k entities) | callgrind plus my 1M-entity runs | `74aed007`; craft-libs `ebb40fa0c` (`section_slices`, `record_slices`, `unescape_owned`) is read-only for us, port by hand |
| Fence select 1.6 to 2.2 times slower: `hit_polylines` calls `text_quad` (font layout) for every text in the fence box | callgrind: 79.3M against 32.9M instructions; `text_quad` alone is 47.5M, about the whole gap | ours only: skip a candidate unless a fence leg touches its `reach` box. Unmeasured until built |
| DWG to DWG save changes `$ISOLINES` 4 to 8 (writer shares the DXF whitelist); we drop model-space UCS, VIEWCTR/VIEWSIZE, CTABLESTYLE; DWG entity plot style read but not mapped (x54) | I/O analyst | ours only |
| Solid-hatch DXFs carry group codes 52, 41, 77 that upstream says AutoCAD 2024 refuses | upstream commit message (D1); not run by us | `467ab20a` |
| `plot x.png` writes a PDF; model-space plots use Letter instead of A4 for INSUNITS 4 with MEASUREMENT 0 | render analyst | `62e70432` family; `a3e82dc9` family (`paper_mm` becomes `Option<bool>`) |
| Dimension Style ribbon button errors | UI analyst | see 3.4 (needs the manager dialog) |

### 3.1 Sync infrastructure (before the catch-up PRs)

1. `docs/upstream-sync.md`: fork base `df659744`, target SHA, per-area watermark table, pinned craft-libs/egui/winit revs, hashes of upstream's `LICENSE-*`, the `license =` lines and a NOTICE digest at each sync, decisions log, "contributed back" table.
2. `docs/upstream-sync/rules.toml`: rewrite rules (`cadcraft` to `draftcraft`, `CADCraft` to `DraftCraft`, `CADCRAFT` to `DRAFTCRAFT`; **protected**: `craft-fonts`, `craft-dwg`, `craft-libs`, the copyright holder string in `NOTICE`), path classes (`manual`: NOTICE, ATTRIBUTION, README, AGENTS, CLAUDE, brand, icons, Cargo.lock; `skip`: nightly, CI workflows, credits, ROADMAP, other-language i18n; `deps`; `ours-wins`: `crates/dwg`, `crates/dxf`; `bridge`: acadrust, craft_dwg), taint regexes, and a deny list (`acadrust`, `craft-dwg`).
3. `docs/upstream-sync/ledger.tsv`: range rows (`cadcraft@old..new PULLED PR#`) plus exception rows only. Columns: `upstream_sha, date, area, class, disposition (PULLED|ADAPTED|SKIPPED|DEFERRED|CONTRIB), our_pr, reason`. Reason codes: dwg-bridge, craft-dxf-cutover, duplicate-feature, naming-policy, ci-infra, nightly, credits, i18n-other-locale, licence-gate, needs-3D, superseded-by-ours, next-runtime.
4. `cargo xtask upstream-sync` (std plus `serde_json`, shells out to git like the rest of `xtask`): `fetch`, `status`, `list`, `vendor SHA`, `plan [--to SHA] [--area A]` (runs `git merge-tree --merge-base=V(last) HEAD V(new)`; with `--area` builds a `sync/<area>-<date>` branch, never main), `patch SHA..` (urgent single fixes whose prerequisites are present), `gate RANGE`, `ledger`, `check` (CI hook: ledger parses, watermark SHAs exist, trailers match, no conflict markers; not required to be current).
5. Per-area watermarks, so areas land in separate PRs over several days.
6. Add `Upstream:`/`Upstream-Range:`/`Upstream-PR:` commit trailers; do not rewrite the copyright holder string in `NOTICE` ("ArtCraft Team and the CADCraft contributors").

### 3.2 Gate for every pulled range (`xtask upstream-sync gate`)

1. New dependencies: diff `Cargo.lock` names against a local licence allow-list; new git sources must be `storytold/*` or our mirrors. **Deny** `acadrust` and `craft-dwg`. Flag for the owner: IJG (`jpeg-encoder`, acknowledgement required), BSD-3 (`exr`), unknown. Upstream added 122 packages since the base (about 45 craft-libs crates, `vectorcraft-*`, a screen-capture stack); licences of most are unknown to us.
2. Assets: any added non-code file needs an `ATTRIBUTION.md` row (`cargo xtask assets`). Block `*.shx *.pat *.lin *.dwt *.cui* *.pgp *.ttf *.otf *.pfb` and any Autodesk, Adobe or Avid origin unless the owner approves.
3. Words in added lines: GPL, LGPL, AGPL, LibreDWG, LibreCAD, QCAD, FreeCAD, planegcs, SolveSpace, OpenCASCADE, Teigha, ObjectARX, "Copyright ... Autodesk". The mechanics analyst's scan of upstream since the base found none beyond their own rules and a pointer to the public ODA specification.
4. Fonts only from `craft-fonts`; no test corpus committed. The upstream dynamic-block test data (`crates/io/tests/data/dynblock_*.txt`) has no `ATTRIBUTION.md` row upstream; add one or regenerate it.
5. Naming policy (D6), brand (no DraftCraft or ArtCraft marks added or removed by a port; `docs/brand` and icons are `manual`).
6. Our rules the port must satisfy: no panics in non-test code, Italian string for every new UI string (menu entries also Chinese), CHANGELOG line, regression test for a crash fix.

### 3.3 Catch-up PRs, in layer order (target `414e5442`, D7; about 8 to 14 working days)

Each PR title: `Upstream sync: <area> cadcraft@old..new`; body lists commit subjects by group, hot-file decisions, gate output, and the i18n, ATTRIBUTION, NOTICE and CHANGELOG lines added. The ledger and watermark move in the same PR.

1. **deps**: mirrors (D8), single craft-libs rev, egui/winit pins, `cargo-deny`.
2. **geom/color/doc**: handle band, name index (`names.rs`), UCS model (`bc7357ff` plus about 34 more; UCS is 2D, so main), dynamic-block reference behaviours (3.5), layer states in AutoCAD's own form (`9ed3607b`, `50555a91`), page-setup paper units (`9a6396c0`, `9c2915c2`).
3. **constraints/fonts/render**: display point budget, plot output format and units, accents in the stroke font (`0de6ec87`), SHX glyph cache bounds (`5154f575`; ours allows about 13 GB; also `01c003e4`, `b4398791`, `bb91014e`, `82779b50`, `225407ca`; keep our `661e5e2d`, which upstream lost), wide-polyline mitre (`4370e61c`), locked-layer fade (`7f84dab9` family), constant attributes (`4f58d512`), AutoCAD-style dash layout (`a303ff43` family; D1), font discovery (`dirs.rs`), plus `cb96c182`, `98b47c19`/`c2175377`, `3b8531ff`/`75760178`.
4. **io**: only after D5. Quadratic open, linetype PNG, MINSERT marker (`8e698902`), mirrored-object writer (`275f989a`), SVG overflow/ellipse fixes, tilted-plane OCS reading, CR-only DXF, binary hex chunk size, layout/plot settings, UCS/view persistence, proxy graphics (large; 13 `Proxy` entities upstream keeps that we do not).
5. **engine**: DoS bounds and LISP yields, BREAK/TRIM/JOIN/PEDIT stack, `EXTRIM` (`14779e26`), `HATCHGENERATEBOUNDARY` (`0bd4d54f`), automation roots (agent filesystem sandbox, `46ee1a81` plus about 18 more; our 22 extra commands need classifying first), UCS commands, Dimension Style Manager engine side (`a770fa5a`), ATTDIA and Edit Attributes (`03328362`), `properties.describe`.
6. **ui-egui and mcp**: see 3.4.
7. **apps, docs, packaging, CI**: see section 6.

### 3.4 UI items to take into our ribbon (adapt: our theme tokens, `app.run`, ribbon placement, icons drawn in `icons.rs`)

Ranked by the UI analyst: macOS Finder-open Apple events (`7dc9e0ec`) and sheet dialogs (`2f8f1861`; our `docs/releasing.md` admits the gap); Windows single instance (`2ed5614e`) and portable ZIP (`8b6c204f`); Dimension Style Manager dialog plus `properties.describe` (`a770fa5a`, large); Blocks palette (`65ed2fb7`; ours is a flat button list); agent safety: loopback-only token (`6d744887`), control-parser hardening (`b52f0a57`, `a87ec8a5`), then opt-in automation roots (`46ee1a81`); canvas input fixes (`06e53f3e`, `fa4b6d13`, `2733c9d3`, about 1,300 test lines); in-place text and dimension editing (`9f2cd4ca`) and the INSERT attribute dialog (`03328362`); clipboard chain (`2d95af7d`) and Excel TSV paste to a table (`28ab81b3`); Live Array as a contextual ribbon tab (`2ecf474d`), WBLOCK dialog (`a974b202`), Keyboard Shortcuts editor (`2cbd67f9`); locales pl, ka, ta (`deff6cb0`, `6c729993`, `aa3d4635`; each needs an `ATTRIBUTION.md` row).

### 3.5 Dynamic blocks and parametric design (separate track, develops in tandem with ZWCAD import)

State (checked in code): both repos read the same parameter kinds (Linear, Polar, XY, Rotation, Alignment, Flip, Point, Visibility, Lookup, Base point). Upstream executes actions far more fully (`apply.rs` 2,300 lines against our 1,088: stretch, move, scale, rotate, flip, array, polar stretch, lookup, with grips and Dynamic Input). We alone author them (`bparameter`, `baction`; Linear and Point only today). Neither models constraints inside a block (grips and constraints are kept as "other"). Upstream has no authoring commands.

Plan:
1. Pull upstream's reference-side execution (engine analyst item C2: `ee069795`, `ebeb2638`, `46acac72`, `b471e597`, `fc8caca7`, `fc82a2f9`, `7f6033c0`, `28d240ca` and about 18 more, plus follow-ups). Large port; `apply.rs` conflicts; our authoring code does not break (it builds only Linear and Point) but `bauthor.rs` should gain the new parameter kinds.
2. Extend authoring to every parameter kind and action kind, round-tripped through our native DWG writer.
3. Parametric layer: build on `crates/constraints` (`geomconstraint`, `dimconstraint`, `autoconstrain`) so dimensional constraints drawn in the block editor become block parameters. New design work in neither repo.
4. ZWCAD import: I have not seen how ZWCAD stores its parametric blocks. Collect small DWG files, one feature each, saved by ZWCAD, and test against them (allowed by `AGENTS.md`; each file needs an `ATTRIBUTION.md` row). Never read anything inside a ZWCAD install.

### 3.6 Skip list

`TOOLBAR` and menubar commands (conflict with `AGENTS.md`: no tool bar); OLE2FRAME work for now; upstream `PDFIMPORT` (we have our own, 2.1.0; it is the only command id added on both sides, and `io/pdf_read.rs` conflicts heavily); nightly pipeline; `craft-ui` next runtime (D7); credits; parity docs; `craft-mcp`; the `craft-shx`, `craft-svg` and `craft-image-codecs` extractions; font guards and roots (need `file_access.rs`); every commit touching the `acadrust` or `craft-dwg` bridge (58 of them); UI commits that depend on `next`. Upstream is 2D only, so nothing needs the `3D` branch; the ACIS-related commits (`report_acis_dropped`, `ea4a59b6`, `f7b3b689`) conflict semantically with our ACIS preservation and go in the adapt lane, landing on main only if they work without the `solids` feature.

## 4. Cadence after the catch-up

Weekly windows, aligned with the weekly main to `3D` merge: about 150 commits, 15 to 25 newly conflicting files, 4 to 6 PRs. A read-only daily `status` (also CI) catches security and crash fixes for the `patch` lane. A month's delay roughly doubles the conflict set.

## 5. Risks

- craft-libs, craft-contrib and craft-i18n were not readable from the analysis sessions, so `craft-dxf`/`craft-dwg` API compatibility and upstream's licence allow-list are unverified.
- Files that mention `craft_dxf::` auto-merge and then fail to compile against our crates: expect compile-fix work in every io-adjacent PR. 84 files.
- Nothing was compiled; "applies cleanly" means the patch applies, not that the build passes.
- Upstream relicensing: none observed (`LICENSE-MIT`, `LICENSE-APACHE` identical at base and head); `check` fails if that changes.
- Provenance of AI-assisted upstream code is unobservable; the gate and owner review are the control.
- Our comments say "as acadrust reads it" in several places; review for clean-room wording.

## 6. Quality infrastructure to adopt

Restore the ast-grep workflow (our `lint/` rules exist but nothing runs them); dependabot; SHA-pinned actions plus Harden-Runner (we have 18 floating tags); stop relying on `storytold`'s `gate.yml@main`; nightly release dry run; run the `roundtrip` corpus harness nightly with `DRAFTCRAFT_CORPUS_STRICT=1` (our 68-file corpus plus any real-world files; keep `docs/` benchmark numbers from this session's `perf` runs as a baseline); a perf ratio check; `cargo-fuzz` targets and `proptest` (neither repo has them); `cargo-deny`; a ribbon smoke test that clicks every button and fails on "is required"; a display-names guard (`fa7a9772`).

## 7. How to start the next session

1. Read this file and `AGENTS.md`. Ask the owner for D1 to D9; do not guess.
2. Work on a branch from `main` (never `main` or `3D` directly). Build the release CLIs with `cargo build --release -p draftcraft-cli` (about 10 minutes on 4 cores; run only one build at a time).
3. Do 3.0 first, one PR per row, each with its regression test. These need no sync tooling and the first four are the highest value.
4. Then 3.1 (tooling, mirrors), then 3.3 in order. Verify each PR with `cargo xtask ci`.
5. Re-run the comparison from this session (`draftcraft-cli roundtrip`, `perf 1000000`) after 3.0 and after each catch-up PR and record the numbers in `docs/upstream-sync.md`.

Baseline numbers from 2026-11-26 (this machine, 4 cores): `perf 1000000`: DXF read 11.0 to 11.7 s (upstream 8.7 to 9.4 s), DXF write 5.4 to 5.8 s (5.2 to 5.6 s), build 819 ms (1,212 ms), fence select 667 ms (424 ms). Corpus round-trip: 68 of 68 open, 0 crashes, on both.

## 8. Contributing back (optional, D9)

Generic fixes of ours: `7fba4792` (current table style in the variables dictionary), `8318ca0c` (arc length and jogged radius dimension types), `d03de31b` (region bulge round trip), the DWG SAT/ACIS line-feed fixes, `a453ad03`, `c531302e`. A thin set; most of our work is UI, packaging and the DWG codec. Finish the catch-up first, then send small generic fixes one per issue from an org fork, with no DraftCraft strings or marks in them. Treat offering the DWG codec as a separate strategic conversation: it would remove the MPL dependency for both projects and also gives up a differentiator.

## 9. Progress log

### 2026-11-26: fonts removed, dynamic blocks pulled

- **Bundled fonts removed** (commit "Remove the bundled standard PDF fonts"): `vendor/hayro-interpret`, installed-font
  resolver in `crates/io/src/pdf_read/standard_fonts.rs`, none of the 14 Foxit fonts left in the release CLI.
- **Dynamic blocks pulled from upstream** (`Upstream-Range: storytold/cadcraft@df659744..414e5442`, files only; no history):
  `doc/src/dynblock.rs`, `dynblock/apply.rs` (1,088 to about 2,300 lines), `grips.rs`, `reference.rs`,
  `engine/src/blockedit.rs`, upstream's new dynamic-block tests and four hand-written DXF fixtures
  (rows added to `ATTRIBUTION.md`; upstream's own notes say they were written for the tests, not saved from a CAD
  program). Our files were identical to the fork base, so the merge was clean (0 conflicts in 7 files; the 8th, an io test
  file, had one conflict in a test that used `acadrust`, resolved by keeping our version).
  What this gives: rotation and flip parameters and actions, scale, polar stretch, array, base point and alignment
  parameters, lookups of angles, flips, XY distances and positions, as the reference side executes them.
  Our authoring code (`author.rs`, `bauthor.rs`, `bparameter`, `baction`) compiled unchanged.
- **Not pulled yet, and why:**
  - Grip dragging for the new parameter kinds (`engine/src/cmd/blocks/dyngrip.rs`, `ui-egui/src/dyngrips.rs`,
    `engine/tests/it/dynamic_grips.rs`). Upstream's version needs `TagKey` and `Block::attdefs_by_tag` (`a34ebd0b`),
    the live value hook of the prompt machine, `Prompt::angle`, and the frozen-layers-in-viewport work
    (`e43c39c5`: `Session::viewport_frozen`, `snap::nearest_with_direction`). Until those land, rotation, flip and
    alignment grips are read but not shown (`ui-egui/src/dyngrips.rs` filters them); their values change through
    Properties and `blocks.reference.set`.
  - The reader fix for tilted-plane polylines (#1886), which one upstream test depends on. We kept our version of
    `a_tilted_polyline_keeps_the_stored_vertex_list`; it will need the fix ported by hand into our reader.
- **Verified:** fmt, clippy, i18n, assets, layers and wasm pass; the full workspace test suite passes except
  `gpu_viewport_stencil` (no GPU adapter in this container; unrelated).
- **Next:** the ZWCAD parametric track (3.5, steps 2 to 4), starting with samples saved by ZWCAD; then the grip
  prerequisites above.
