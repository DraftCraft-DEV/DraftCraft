# Upstream sync mechanics: DraftCraft <- storytold/cadcraft

Scope: quantitative churn, a brand-rewrite pipeline that was built and tested, a sync process, risks.
Measured 2026-11-26 on `cadcraft/main` = ba344a89 against our HEAD e8aa0afa (Version 2.2.3).
No branch was committed or pushed. Throwaway branch `tmp/upstream-sync-sim` was created and deleted. No cargo build was run.
Dangling objects remain in the shared object store: vendor commits c4784c51 (base), 0962c060 (head), da21321c (pre-next) and merged tree f0744276. They are harmless and gc will drop them.
Scripts and raw data: `analysis/mechanics-tools/` and `analysis/mechanics-data/`.
Their git calls were written for the old worktree path. They now use `/home/user/DraftCraft`, set by `sed` in the working copy under `scratchpad/work`. The copies in `mechanics-tools/` still point at the removed worktree, so edit `WT`/`WT=` before reuse.

## 0. Findings

1. **Fork base.** The base is upstream first-parent **df659744** (PR #2818, 2026-11-05 15:04 +0100). fc157ea9 (11-05 14:17) is one file away. 2569dbd5 (11-04) is wrong: 9 fewer files match. See section 1.
2. **Upstream volume.** 915 non-merge commits (1480 with merges) since the base, about 130-185 per week.
   - In upstream, 782 files changed (+108.8k lines net) between base and head.
   - Of those, 573 files (73%) were changed only upstream (clean zone) and 209 (27%) on both sides (hot).
   - 106 files give a text conflict in a whole-tree 3-way merge, with 217 hunks. Cargo.lock is regenerated, not merged.
3. **Patches do not work for catch-up; tree-level 3-way merge does.**
   - Replaying all 915 commits as rewritten patches in order applies only 208 (23%).
   - A 48-commit independent test of clean-zone commits applies 28 (58%). All 20 failures are "file missing in ours" (a prerequisite upstream commit not pulled), none from branding.
   - A vendor snapshot plus `git merge-tree --merge-base` merges everything at once with 106 conflicting files out of 782 (13.6%).
4. **Rewrite rules.** The sed rules reproduce our own rebrand exactly: after rewriting upstream's base, 823 files are byte-identical to ours, and **0 are brand-only mismatches**. Real divergence is 234 files.
5. **Structural drift matters more than text drift.**
   - Upstream moved `crates/dxf` and `crates/dwg` into craft-libs: craft-dxf c736e488 (11-11), craft-dwg cutover fbb08380 (11-13).
   - It also moved SHX, SVG, MIME, CSV, image codecs, MCP and others into craft-libs.
   - Upstream head uses `craft_dxf` in 52 .rs files, `craft_dwg` in 20 and `acadrust` in 30. We use none of them.
   - 58 upstream commits reference the acadrust/craft-dwg bridge.
   - **acadrust is MPL-2.0 and we deliberately removed it** (7114f195).
6. **Unmerged craft-libs "next" runtime.**
   - b55abde2 (PR #3449, 11-25) repins every craft-libs and egui pin to the unmerged `next` rev 8fa4db64a9 / egui b9770908e: 35 pin edits, 110 ui-egui files, +3.1k lines.
   - 414e5442 is the last state before it and holds 862 of the 915 non-merge commits. After it there are 53 commits, of which only 1 touches ui-egui.
   - 414e5442 still uses egui **6d518fc7, identical to ours**, and craft-libs 453b0da782 (plus 219c1d0b for one crate).
   - Recommended target: sync to 414e5442 and defer the next-runtime port.

## 1. Fork base (refined)

Method: take each upstream first-parent commit, apply the brand rewrite to every blob, and count byte-identical files against our 77c95f8 import (crates, apps, xtask; dwg/dxf excluded; 979 files).

| upstream commit | identical | differ | upstream-only |
|---|---:|---:|---:|
| **df659744** 11-05 15:04 (PR #2818) | **856** | 123 | 14 |
| fc157ea9 11-05 14:17 (PR #2831, #1031 accents) | 855 | 124 | 16 |
| 6ca414c3 11-05 14:10 | 850 | 128 | 14 |
| 1b39fbba 11-05 16:31 (drops three egui patches) | 849 | 130 | 17 |
| 2569dbd5 11-04 20:20 (earlier guess) | 847 | 131 | 14 |

- Our `[patch]` table still lists `egui_system_font_provider`, `egui_demo_lib` and `egui_inspection`. Those are the three patches dropped by deb1468d, which landed at 1b39fbba. So our import predates 1b39fbba, and the base is df659744 or fc157ea9. This agrees with the other analysts' "11-05" finding. 1b39fbba is the first merge after the base.
- The base is mechanical: the sync tool uses df659744. The one-file difference from fc157ea9 does not change any table here.
- The craft-libs pin at the base (92d392cf) and the egui pin (6d518fc7) are exactly ours.
- At import, 123 of 979 in-scope files already differed (RustDraft work). Today 234 files differ over the whole tree.
- Our history has **four root commits**: 77c95f8 plus the grafted 141d1be5, 897a0429 and e340b275 (DWGCraft lineage, 1100+ files each). Log-based tools must exclude them (`--first-parent`, explicit excludes) or churn is inflated by about 450k lines per root.

## 2. Churn

### 2a. Log-based (commits, files, lines per area)

U = upstream df659744..cadcraft/main (915 non-merge). O = ours 77c95f8..HEAD (120 non-merge).
O excludes the rebrand commit cfad94f8 and the three grafted roots. Data, `.dxf`/`.dwg`/png/`tests/data` files and Cargo.lock are excluded from lines.
A commit counts once per area it touches.

| area | U commits | U files | U + | U - | O commits | O files | O + | O - |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ui-egui | 228 | 167 | 27730 | 2972 | 37 | 168 | 11848 | 4570 |
| engine (non-cmd) | 263 | 144 | 21439 | 1053 | 24 | 277 | 3961 | 1802 |
| io | 216 | 148 | 16230 | 2465 | 53 | 188 | 17972 | 5602 |
| engine/cmd | 229 | 96 | 11322 | 2275 | 23 | 168 | 9963 | 4839 |
| doc | 96 | 20 | 5819 | 1203 | 18 | 37 | 1473 | 644 |
| fonts | 64 | 27 | 3816 | 1909 | 2 | 18 | 163 | 110 |
| apps | 83 | 39 | 4600 | 334 | 14 | 55 | 897 | 519 |
| dwg | 16 | 5 | 1470 | 3322 | 22 | 59 | 23796 | 6260 |
| packaging | 7 | 31 | 3497 | 77 | 9 | 29 | 860 | 358 |
| root/other | 87 | 16 | 2303 | 589 | 52 | 16 | 1951 | 1844 |
| dxf | 5 | 11 | 418 | 2345 | 3 | 10 | 41 | 24 |
| render | 45 | 32 | 2170 | 343 | 6 | 55 | 817 | 735 |
| docs | 127 | 24 | 1155 | 675 | 36 | 18 | 1488 | 780 |
| geom | 10 | 14 | 1142 | 313 | 7 | 6 | 518 | 104 |
| ci(.github) | 12 | 9 | 310 | 107 | 10 | 5 | 568 | 184 |
| mcp | 8 | 6 | 260 | 72 | 3 | 10 | 60 | 60 |
| color | 2 | 1 | 53 | 26 | 1 | 1 | 2 | 2 |
| xtask | 8 | 3 | 48 | 13 | 10 | 7 | 301 | 144 |
| constraints | 2 | 2 | 15 | 7 | 1 | 9 | 45 | 45 |
| solid (3D, added then reverted on main) | 0 | 0 | 0 | 0 | 12 | 12 | 5710 | 5710 |

Notes on the table:
- Our 3D work (18921834..cd45bb3c) was reverted on main (ee840fe3, ee098cc1). It nets to zero but inflates our engine/cmd, io and ui rows.
- Upstream's `dwg` and `dxf` rows are deletions: the crates moved to craft-libs.
- Our DWG codec is why dwg/io lines are large on our side.

Upstream velocity, non-merge commits by ISO week with changed non-data lines: W45 185 / 19.6k, W46 135 / 20.4k, W47 181 / 27.6k, W48 (partial) 133 / 20.5k. Commit sizes: 327 are <=20 lines, 357 are <=100, 188 are <=500, 43 are >500. Only 39 commits (4%) touch nothing but CI, credits, roadmap, other-language i18n or nightly files, so there is little to auto-skip.

### 2b. Tree-level zones (3-way: rewritten upstream base / rewritten upstream head / our HEAD)

Brand noise is gone, so this is the real overlap.

| area | upstream files changed | clean (upstream-only) | hot (both sides) | of hot: text conflict | upstream lines | clean lines | hot lines | ours-only files |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ui-egui | 163 | 106 | 57 | 24 | 28342 | 19390 | 8952 | 20 |
| engine (non-cmd) | 141 | 119 | 22 | 4 | 21244 | 18036 | 3208 | 13 |
| io | 143 | 94 | 49 | 29 | 19914 | 13152 | 6762 | 109 |
| engine/cmd | 95 | 79 | 16 | 7 | 11424 | 8423 | 3001 | 16 |
| doc | 20 | 14 | 6 | 4 | 5872 | 4429 | 1443 | 4 |
| apps | 39 | 28 | 11 | 5 | 4568 | 3794 | 774 | 9 |
| fonts | 26 | 25 | 1 | 0 | 4028 | 3794 | 234 | 1 |
| packaging | 31 | 24 | 7 | 3 | 3478 | 3228 | 250 | 20 |
| render | 33 | 29 | 4 | 1 | 2364 | 2146 | 218 | 1 |
| dxf (crate removed upstream) | 10 | 9 | 1 | 1 | 1910 | 1083 | 827 | 0 |
| dwg (ours is BlackCAD) | 4 | 0 | 4 | 2 | 1741 | 0 | 1741 | 48 |
| root/other | 16 | 7 | 9 | 9 | 1168 | 346 | 822 | 6 |
| geom | 13 | 10 | 3 | 1 | 1085 | 748 | 337 | 1 |
| docs | 22 | 7 | 15 | 13 | 803 | 296 | 507 | 27 |
| ci(.github) | 8 | 5 | 3 | 3 | 353 | 230 | 123 | 9 |
| mcp | 6 | 6 | 0 | 0 | 324 | 324 | 0 | 0 |
| color | 1 | 1 | 0 | 0 | 79 | 79 | 0 | 0 |
| assets | 6 | 6 | 0 | 0 | 52 | 52 | 0 | 20 |
| xtask | 3 | 2 | 1 | 0 | 47 | 46 | 1 | 4 |
| constraints | 2 | 2 | 0 | 0 | 22 | 22 | 0 | 0 |
| **total (Cargo.lock excluded)** | **782** | **573 (73%)** | **209 (27%)** | **106** | **108818** | **79618 (73%)** | **29200** | **308** |

**Clean zones (upstream-only, drop-in):**
- constraints, color, mcp, fonts (25 of 26), render (29 of 33), geom (10 of 13).
- engine: 198 of 236 files, 85% of lines.
- ui-egui: 106 of 163, including most of the new panels.
- The 223 added and 340 modified files here are mostly tests (205 are test files).

**Hot spots** (files heavily changed on both sides):

| file | upstream lines | our lines vs base | text conflict |
|---|---:|---:|:--:|
| crates/dwg/src/lib.rs | 1377 | 1377 | yes (ours is a different codec) |
| crates/io/src/pdf_read.rs | 1064 | 758 | yes (PDFIMPORT built on both sides) |
| crates/ui-egui/src/cmdline.rs | 505 | 484 | yes |
| crates/engine/src/select.rs | 792 | 272 | yes |
| crates/io/src/tests.rs | 1862 | 241 | yes |
| crates/geom/src/nurbs.rs | 239 | 207 | yes (our CONIC/Rho work vs their NURBS perf) |
| crates/ui-egui/src/menus.rs | 385 | 204 | yes |
| crates/doc/src/extents.rs | 707 | 189 | yes |
| crates/ui-egui/src/options.rs | 229 | 183 | yes |
| crates/ui-egui/src/updates_ui.rs | 148 | 592 | yes |
| crates/io/src/dxf_read/{objects,entities}.rs | 196 / 131 | 168 / 156 | yes |
| apps/draftcraft/src/main.rs | 486 | 116 | yes (10 hunks) |
| crates/ui-egui/src/lib.rs | 458 | 121 | yes (11 hunks) |
| README / AGENTS / ROADMAP / NOTICE / ATTRIBUTION / docs/* | | | always manual |

- Of the 209 hot files, 39 have at least 50 lines changed on both sides; 34 of those conflict.
- 41 hot files have 10 lines or fewer changed upstream; 16 still conflict.

**Conflicts after a whole-tree merge:** 107 conflicted paths in total.
- 88 content, 18 modify/delete, 1 add/add.
- Cargo.lock accounts for 1 content conflict (14 hunks); the rest of the table excludes it.

| area | conflicted files | text hunks | other (modify/delete, add/add) |
|---|---:|---:|---:|
| io | 29 | 66 | 0 |
| ui-egui (+4 i18n tsv, 9 hunks) | 24 | 73 | 1 |
| root/other (Cargo.toml, README, AGENTS...) | 10 | 29 | 2 |
| apps | 5 | 13 | 1 |
| engine (non-cmd) | 4 | 12 | 0 |
| engine/cmd | 7 | 11 | 0 |
| packaging | 3 | 8 | 0 |
| docs | 13 | 7 | 9 |
| render | 1 | 5 | 0 |
| doc | 4 | 4 | 0 |
| ci / geom / dwg / dxf | 3 / 1 / 2 / 1 | 2 / 1 / 0 / 0 | 2 / 0 / 2 / 1 |

Excluding Cargo.lock, that is 106 files and 217 text hunks.

**Semantic taint of the 573 clean files** (clean text does not mean it compiles):
- 84 files (15%, 9.5k lines) reference new craft-libs crates or acadrust / the in-process DWG bridge.
  - 50 use craft_dxf, craft_dwg, craft_shx, craft_svg, craft_mime, craft_csv, craft_image_codecs, craft_font_catalog, craft_theme, craft_keymap and similar.
  - 48 mention acadrust or the bridge.
- 10 are upstream deletions (9 `crates/dxf` files plus 1 fonts file): we do not follow them.
- **479 files / 68.7k lines are portable as is.** Subtract about 50 files of nightly/CI/licence/icon-only material and about 430 code and test files remain.

**Conflict growth versus waiting.** Same ours-HEAD, upstream snapshot at successive dates:

| upstream snapshot | commits since base | files changed | conflicted files |
|---|---:|---:|---:|
| 11-08 (7a9ec9ba) | 144 | 131 | 30 |
| 11-12 (ca87680f) | 195 | 263 | 59 |
| 11-15 (7064a685) | 205 | 317 | 72 |
| 11-19 (a770fa5a) | 266 | 399 | 79 |
| 11-22 (a70b9b02) | 622 | 567 | 89 |
| 11-26 (ba344a89) | 915 | 782 | 106 |

Conflicts are bounded by the hot-file set (209 files today). New conflicted files grow by about 3 to 4 per day and keep growing as our side grows. The count of conflicted files does not shrink with frequent syncs, but each hunk gets smaller and fresher.

**Pre-next window.** Syncing to 414e5442 instead of head gives 101 conflicted files, versus 106 at head. The five extra conflicts at head come from the next-runtime port (4 ui-egui, 1 apps). The port itself, PR #3449 (b55abde2), touches 110 ui-egui files (+3.15k lines), 7 apps files, Cargo.toml (35 pin edits) and Cargo.lock.

## 3. Brand-rewrite pipeline and experiments

### 3a. Rules (`mechanics-tools/rw.py`)

Applied in order, case-sensitive:
- `ai.storyteller.cadcraft` -> `dev.skryx.draftcraft`
- `github.com/storytold/cadcraft` -> `github.com/DraftCraft-DEV/DraftCraft`
- `ghcr.io/storytold/cadcraft` -> `ghcr.io/draftcraft-dev/draftcraft`
- `storytold/cadcraft` -> `DraftCraft-DEV/DraftCraft`
- `CADCRAFT` -> `DRAFTCRAFT`
- `CADCraft` and `CadCraft` -> `DraftCraft`
- `Cadcraft` -> `Draftcraft`
- `cadcraft` -> `draftcraft`

Protected strings (never rewritten): `storytold/cadcraft-corpus`, `cadcraft-corpus`, and `CADCraft contributors`. The last is the upstream copyright holder, which NOTICE must keep.

- `craft-fonts`, `craft-libs`, `craft-dwg`, `craft-ui` and the other `craft-*` crate names never contain "cadcraft", so a substring rewrite cannot touch them.
- The CadCraft/Cadcraft rules were added after the rule-fit test found them in two test files.
- Case variants and reverse-DNS ids were the only surprises. Env vars (`CADCRAFT_*`), crate names, `cadcraft-cli`, `CADCraft.app`, `CADCraftData` and similar all fall out of the substring rules.
- Paths are rewritten too (`apps/cadcraft` becomes `apps/draftcraft`, `ai.storyteller.cadcraft.*` becomes `dev.skryx.draftcraft.*`).
- Never auto-merge LICENSE-*, NOTICE, ATTRIBUTION.md, README.md, AGENTS.md, CLAUDE.md, docs/brand/*, assets/app-icon/* or Cargo.lock. Handle them as manual lanes.

**Rule fit test.** Rewrite upstream's base tree and compare with our HEAD at the same path:
- 823 files identical.
- 0 brand-only mismatches (2 existed before the case rules).
- 234 real differences.
- 43 only upstream.
- 227 only ours.

### 3b. Per-commit patch experiments

Pipeline per commit: `git format-patch -1 --stdout --full-index --binary SHA -- . ':(exclude)Cargo.lock'`, then the rewrite, then `git apply`.
- Variant A: rewrite the patch text only.
- Variant B: A, plus re-point the `index` lines to blobs brand-rewritten with `git hash-object -w`, so `--3way` has a correct merge base.

**E1: 48 independent commits.** Selection: the 42 most recent clean-zone commits confined to geom/constraints/color/doc/fonts/render/engine/mcp, plus 6 geom/color ones. Each is checked against our HEAD with no state change.

| check | applies |
|---|---:|
| plain `git apply --check` | 8 / 48 |
| `--check --3way`, variant A | 25 / 48 |
| `--check --3way`, variant B | **28 / 48 (58%)** |

- Variant B gains 3 over A: commits whose files contain brand strings (4370e61c1, b713869ce, cec00d458).
- 17 need `--3way`: earlier upstream commits changed the same files, so the context has drifted.
- **All 20 failures are the same error**: "<file>: does not exist in index". The patch edits a file an earlier upstream commit created, for example `crates/doc/src/entity/proxy.rs`, `crates/engine/src/file_access/commands.rs`, `crates/fonts/src/ttf/dirs.rs`, `crates/engine/src/cmd/ucs.rs`.
- Zero failures come from branding. Zero from our own changes (these are clean-zone files).
- Results per commit: `mechanics-data/e1_results.tsv`.

**E2a: sequential replay of the 250 clean-class commits** (topo order, each rolled back on failure, no skipped-commit prerequisites):

| result | count |
|---|---:|
| applied (46 plain + 9 via 3-way) | 55 (22%) |
| file missing in ours | 126 (50%) |
| true content conflict | 68 (27%) |
| other (partial apply) | 1 |

**E2b: sequential replay of all 915 non-merge commits:**

| result | count |
|---|---:|
| applied (171 plain + 37 via 3-way) | **208 (22.7%)** |
| file missing in ours (prerequisite skipped) | 350 (38%) |
| true content conflict | 319 (35%) |
| other (a patch applies to some files, another fails) | 37 |
| binary patch skipped | 1 |

Skipping one commit breaks every later commit that depends on it, so cascades dominate. Per-commit classification (whether all touched files are clean) is also poor: only 250 commits (27%) are clean, 279 (30%) hot, 384 (42%) mixed, 2 lock-only. A single UI commit touching an i18n tsv or menus.rs makes a commit "mixed".

**Whole-tree 3-way (the mechanism to adopt).**
- Build brand-rewritten vendor snapshot commits V(base) and V(head) from `git ls-tree -r`: rewrite blobs and paths, `hash-object -w`, a private index via `update-index --index-info`, then `write-tree` and `commit-tree`. This takes about 6 s per snapshot and never touches refs or the worktree.
- Merge with `git merge-tree --write-tree --name-only --merge-base=V(base) HEAD V(head)`. Git needs no shared history for this.
- Result: 782 files in play; 106 conflict (13.6%), 217 text hunks.
- In file terms this is 6 times better than patches: 86% merged automatically versus 23% of commits applied.
- Three-way merge is per-file, so the result can be split by area into themed PRs without changing the merge.

### 3c. Catch-up estimate (non-hot-spot, one-off)

- Portable upstream-only changes: **479 files / 68.7k lines** (205 test files), reduced by about 50 CI/nightly/licence files to about 430 code and test files.
- Mechanically this is one `plan` command plus per-area overlay. The real cost is stabilising: compile fixes, tests, i18n rows, NOTICE/ATTRIBUTION, CHANGELOG.
- Deps lane first: re-pin craft-libs and decide on craft-dxf, craft-shx, craft-svg and the rest.
- Rough effort in working days (not compile-validated):

| piece | days |
|---|---:|
| tool and ledger | 1-2 |
| deps lane (single craft-libs rev; egui stays 6d518fc7 if the target is 414e5442) | 1-2 |
| clean overlay in about 6 layer-ordered PRs | 2-3 |
| hot hunks (217 in 88 files, io and ui-egui dominate) | 3-5 |
| i18n, ATTRIBUTION/NOTICE, CHANGELOG, gates | 1-2 |
| **total** | **about 8-14 days**, over about 8-10 PRs, bounded by owner review |

- Cost grows about 3-4 conflicted files per day of delay, so doing it early is cheaper.

## 4. Ongoing sync game plan

### 4a. Mechanism: vendor snapshots plus windowed 3-way merges

- Keep a remote `cadcraft` (fetch only; never push) and a local, never-merged branch `vendor/cadcraft`. Each commit on it is the brand-rewritten snapshot of one upstream SHA.
- It could be pushed to our org as a plain branch. Upstream is MIT OR Apache-2.0 and NOTICE keeps the copyright text. This preserves the merge base if upstream rewrites history or disappears.
- No `git merge`. For a window `last..new`, run `merge-tree --merge-base=V(last) HEAD V(new)`.
- **Per-area watermarks.** The ledger records a last-reviewed upstream SHA per area. The base for area X is V(watermark[X]). This works because the merge is per-file, and it lets areas land in separate PRs over several days.
- Land PRs in layer order: deps, then geom/color/doc, then constraints/fonts/render, then io, then engine, then ui-egui/mcp, then apps, then docs/packaging/CI.
- Individual commits go through `patch SHA...` only for an urgent single fix whose prerequisites are present.

### 4b. Files

- `docs/upstream-sync.md` (human):
  - fork base df659744, the target SHA, and the per-area watermark table;
  - pinned craft-libs, egui and winit revs and the upstream rev at each watermark;
  - the hash of upstream's LICENSE-*, the `license =` lines, and a NOTICE digest at each sync;
  - the decisions log;
  - a "contributed back" table.
- `docs/upstream-sync/rules.toml`:
  - rewrite rules and protected strings;
  - path classes: `manual` (NOTICE, ATTRIBUTION, README, AGENTS, CLAUDE, brand, icons, Cargo.lock), `skip` (nightly, CI workflows, credits, ROADMAP, other-language i18n), `deps`, `ours-wins` (crates/dwg, crates/dxf), `bridge` (acadrust, craft_dwg);
  - taint regexes;
  - deny list (`acadrust`, `craft-dwg`).
- `docs/upstream-sync/ledger.tsv`, with range rows (`cadcraft@old..new PULLED PR#`) plus **exception rows only**:
  - columns: `upstream_sha, date, area, class, disposition (PULLED|ADAPTED|SKIPPED|DEFERRED|CONTRIB), our_pr, reason`;
  - reason codes: dwg-bridge, craft-dxf-cutover, duplicate-feature, naming-policy, ci-infra, nightly, credits, i18n-other-locale, licence-gate, needs-3D, superseded-by-ours, next-runtime.
- A per-commit ledger for all 915 would not be reviewable; ranges plus exceptions is enough.

### 4c. `cargo xtask upstream-sync` (pure std plus serde_json, shelling out to git, as the rest of xtask does)

| subcommand | does |
|---|---|
| `fetch` | `git fetch cadcraft main` |
| `status` | reads the watermarks, checks the SHA is an ancestor of `cadcraft/main`, prints new commits by area and class (clean / hot / mixed / skip), and flags taint and 3D candidates |
| `list [--area ..] [--class ..] [--since ..]` | one commit per line with files, zone and taint |
| `vendor SHA` | builds and caches a snapshot in `vendor/cadcraft` (the rewrite plus plumbing in 3a) |
| `plan [--to SHA] [--area A]` | the merge-tree run; prints the conflict table; with `--area` builds that area's tree (merged paths for the area, HEAD elsewhere; deletions per rules) onto a `sync/<area>-<date>` branch, never main |
| `patch SHA..` | emits variant-B patches for hotfix cherry-picks |
| `gate RANGE` | the licence and clean-room gate (4f) |
| `ledger` | appends exception rows and moves watermarks |
| `check` | CI hook: ledger parses, watermark SHAs exist, trailers match the ledger, **no conflict markers anywhere**; not required to be current |

### 4d. Cadence

- **Weekly**, aligned with the existing weekly main -> `3D` merge. Upstream averages about 150-185 non-merge commits (about 20-27k lines) a week.
- A daily read-only `status` (also CI) catches security or crash fixes for the `patch` lane the same day.
- Weekly is the compromise: fortnightly doubles hunks per cycle, daily multiplies PRs over the same ~210 hot files.
- After the catch-up, a weekly window is about 150 commits, about 15-25 newly conflicting files at today's rates, 4-6 PRs.

### 4e. PR hygiene and attribution

- One themed PR per area per window, in layer order. Title: `Upstream sync: <area> cadcraft@old..new`.
- The body lists commit subjects by group, hot-file decisions, gate output, and the i18n, ATTRIBUTION, NOTICE and CHANGELOG lines added.
- The ledger and watermark move in the same PR, so "last reviewed" advances only on merge.
- Agents open the PR and never merge. `cargo xtask ci` must pass before every commit.
- Commit trailers:
  - `Upstream: storytold/cadcraft@<sha>` for one commit, or `Upstream-Range: storytold/cadcraft@<old>..<new>`;
  - `Upstream-PR: storytold/cadcraft#NNNN` where known;
  - then the usual `Co-Authored-By` and `Claude-Session` lines.
- **Do not rewrite** the copyright holder string. NOTICE already carries "ArtCraft Team and the CADCraft contributors", and it stays verbatim.
- Third-party NOTICE and ATTRIBUTION blocks that upstream adds when a port needs them are carried across verbatim. They apply only if we adopt the dependency (see risk 7).
- Every user-visible port gets a CHANGELOG `## Unreleased` line written in plain words. The tool drafts it from the subject; upstream has no CHANGELOG.
- AGENTS rules on our side that ports must satisfy:
  - no panics in non-test code;
  - an Italian and (for menu entries) Chinese string for every new UI string;
  - an ATTRIBUTION row for any new file asset;
  - a crash fix lands with a regression test.
- Upstream already ships it/zh updates (it.tsv +57 rows, zh.tsv +54) which merge row-wise, so the Italian load is mostly free. Upstream also added the locales ka, pl and ta, each of which needs an ATTRIBUTION row.

### 4f. Per-commit gates (`gate`)

1. **New dependencies.** Diff `Cargo.lock` names. The licence must be on a local allow-list; any new git source must be storytold/*. **Deny** `acadrust` and `craft-dwg` (MPL-2.0, deliberately removed). Flag IJG (jpeg-encoder: acknowledgement required), BSD-3 (exr), and unknowns for the owner.
2. **Assets.** Any added non-code file needs an ATTRIBUTION row (`cargo xtask assets` enforces it). Block `*.shx *.pat *.lin *.dwt *.cui* *.pgp *.ttf *.otf *.pfb` and any Autodesk, Adobe or Avid origin unless the owner approves.
3. **Words in added lines.** GPL, LGPL, AGPL, LibreDWG, LibreCAD, QCAD, FreeCAD, planegcs, SolveSpace, OpenCASCADE, Teigha, ObjectARX, "Copyright ... Autodesk". Scan of upstream since the base: zero hits other than their own AGENTS rule, a pointer to the public ODA DWG specification, and a parity-table mention of ObjectARX.
4. **Fonts and corpora.** Fonts only from craft-fonts (never a Cargo dependency); no test corpus committed.
5. **Naming policy.** Upstream renamed "Express Tools" to "Additional Tools" (docs/feature-names.md, #2945). Decide per ledger row; our UI keeps AutoCAD's names.
6. **Brand.** No DraftCraft or ArtCraft marks added to or removed from brand assets by a port (docs/brand and app icons are `manual`).

### 4g. The 3D branch

- Upstream is **2D only**: no solids kernel, no ACIS translator. Of 915 commits, none needs the `solids` feature or `3D`.
- Commits touching ACIS/SAT records in DXF/DWG (report_acis_dropped, the save-loss warning ea4a59b6, f7b3b689 "no empty modeler records") conflict semantically with our ACIS preservation. Put them in the ADAPT lane: land on main only if they work without the `solids` feature; otherwise target `3D`.
- Sync PRs land on main and flow into `3D` through the weekly merge. Keep them area-small so that merge stays tractable.
- The `3D` ref was not in this clone's refs (`git branch -a` shows main and one claude/ branch).

### 4h. Reverse direction: contributing back

What is generic in our ~120 commits:
- DXF/DWG fixes: 7fba4792 (current table style in the variables dictionary, not $CTABLESTYLE), 8318ca0c (arc length and jogged radius dimension types), d03de31b (region bulge round trip), the DWG SAT/ACIS line-feed fixes, a453ad03.
- A few TrueType and units fixes (c531302e).
- The DWG codec itself (MIT OR Apache-2.0, BlackCAD lineage, writes every version, no MPL).

It is a thin set. Most of our work is UI, packaging and the DWG codec.

Helps:
- Every fix upstream accepts disappears from the future conflict set.
- It earns goodwill and gets their review and CI on our fixes.
- Upstreaming the DWG codec could remove the MPL dependency for both projects.

Hurts or risks:
- Maintainer time, and the context mismatch: their io already uses craft-dxf/craft-dwg.
- Giving up the DWG codec as a differentiator.
- Brand: the DraftCraft name and marks are not covered by the code licence. A PR must carry no DraftCraft strings, icons or brand files. The reverse rewrite (`draftcraft` to `cadcraft`) is mechanical, and anything with a DraftCraft-only word must be stripped.
- Their rules: one issue per PR, no workflow changes in feature PRs, English strings only, tests required, and an agent must ask its operator before publishing.
- Any outward PR is a publishing action and needs the owner's go-ahead.

Recommendation: finish the catch-up first. Then send small generic fixes (DXF writer, hostile-input hardening) as separate PRs from an org fork, one per issue. Track them in the "contributed back" table. Treat the DWG codec as a separate strategic conversation for the owner, not a routine contribution.

## 5. Risks

1. **Structural drift into craft-libs.** craft-dxf (c736e488, 11-11), craft-dwg (fbb08380, 11-13) and "cad-shared-*" moves (11-15/16) take code out of upstream's crates.
   - Upstream's own AGENTS rule 9 says reusable parsers belong in craft-libs, so this will continue.
   - Textual merges of files that now say `craft_dxf::` against our `draftcraft_dxf` will auto-merge and then not compile.
   - Options: (A) adopt craft-dxf and drop our crates/dxf (its API and licence were not inspectable: craft-libs access is not enabled for this session); (B) alias `craft_dxf` to our crate in the rules, which is only valid if the API is identical; (C) freeze that lane and port behaviour by hand (ADAPTED).
   - Decide before the io PR.
2. **MPL-2.0.** Upstream still uses acadrust (MPL-2.0) through craft-dwg; NOTICE says "used unmodified as a separate library". We removed it (7114f195). Any craft-dwg adoption or Cargo.lock sync would reintroduce it. The gate denies it; 58 upstream commits reference the bridge and are SKIPPED or ADAPTED.
3. **Dependency mirroring and drift.** Our Cargo.toml and Cargo.lock mirror third-party git pins: storytold/craft-libs, storytold/egui and storytold/winit.
   - We have **three** craft-libs revs (92d392cf for most, c5a1cbfb for craft-update, 2e2312dd for craft-web-host), against upstream's own rule of a single rev merged on craft-libs main.
   - **Upstream's current pin 8fa4db64a9 is the `next` branch, not main.** b55abde2 / #3449 (11-25, 35 pin edits, 110 ui-egui files) and 1cff8cd6 ("Repin craft-libs to storytold next 8fa4db64a9 and egui to b9770908e") move everything to it. Taking those UI commits after b55abde2 forces all pins forward together.
   - 414e5442, the last state before it, still uses egui 6d518fc7 (ours) and craft-libs 453b0da782 (with 219c1d0b for one crate). Target it first and defer the next-runtime port as a separate decision.
   - Our builds depend on repositories we do not control. A force-push, deleted rev or rewritten `next` branch breaks `cargo build` for us even if our code is unchanged. Mitigate by mirroring every pinned rev into org-owned forks, pointing the git URLs at the mirror, and recording the revs in the ledger.
   - New workspace packages upstream: 122 added to Cargo.lock since base, including about 45 craft-libs crates, `vectorcraft-geom` and `vectorcraft-pathops` (git, a sibling app repo), and an xcap/pipewire/wayland screen-capture stack. Their licences are mostly unknown to us: only exr (BSD-3-Clause), jpeg-encoder ((MIT OR Apache-2.0) AND IJG) and zune-jpegxl (MIT OR Apache-2.0 OR Zlib) could be checked from the local registry cache.
4. **Relicensing.** None observed. LICENSE-MIT and LICENSE-APACHE are byte-identical at base and head, and the workspace `license = "MIT OR Apache-2.0"` is unchanged. The ledger records hashes of both and the licence lines at every sync, so a change fails `check`.
5. **GPL and Autodesk-derived material.** The word scan of every added line (excluding Cargo.lock) finds none. No Autodesk assets, SHX, PAT, LIN, DWT or CUI files and no new DXF/DWG fixtures were added; added non-code files are only nightly icons, a signing key and jpeg-encoder licence texts. Residual risk is unobservable provenance in AI-assisted code, which the per-commit gate and the owner's review cover.
6. **Policy drift.**
   - Upstream's AGENTS.md block is managed from storytold/craft-contrib and says "do not edit this copy"; ours is different. Never overwrite it, CLAUDE.md, NOTICE or README.
   - Renames such as Express Tools to Additional Tools, and English-only feature PRs with translations in craft-i18n, differ from ours.
7. **New third-party notice obligations, and a possible existing gap.**
   - Upstream added NOTICE/ATTRIBUTION blocks for IJG (jpeg-encoder), PDFium BSD-3 Foxit .pfb fonts and Adobe BSD-3 CMaps plus Core-14 metrics, because its PDF import enables craft-pdf `interpret`.
   - Our AGENTS says no Adobe fonts or artwork "ever". The owner must rule on these before we enable it.
   - **Check now:** our Cargo.toml already pins `hayro-interpret` and `hayro-cmap` (the same assets) and our NOTICE and ATTRIBUTION never mention PDFium, Foxit or Adobe. Both sides built PDF import independently: upstream's PDFIMPORT came from PR #2787, ours is 2.1.0. `crates/io/src/pdf_read.rs` conflicts heavily (1064 upstream lines, 758 ours). It is the only command id added on both sides.
8. **Velocity.** About 40 commits a day. A month's delay roughly doubles the number of files in play and the hunk count. Anything left in the hot set grows as both sides touch the same files.
9. **History oddities in our repo.** The four root commits break ancestry-based tooling; the sync design relies on SHAs and snapshots, not on ancestry between the repos.
10. **Unverified.** Nothing here was compiled. Taint counts are regex-based. craft-libs, craft-contrib and craft-i18n were not readable (no repository access in this session), so craft-dxf and craft-dwg API compatibility and the licence allow-list in craft-contrib's `standards/licences.md` are unverified.

## 6. Reproduction

All steps are read-only against the object store and write only vendor objects and scratch files.
- `mkvendor.py <sha>` builds the snapshot commit.
- `mt.sh VBASE OURS VNEW out` runs the merge simulation. `post_mt.sh` and `zones.py` produce the conflict and zone tables.
- `patchsim.py independent|sequential LIST OUT` runs the patch experiments. The sequential mode rolls back through `git read-tree --reset -u` and needs a throwaway branch.
- `rulefit.py` runs the rule fit test. `basefind4.py` runs the base search. `churn.py` and `matrix.py` give the log-based table. `growth.sh` gives the growth curve.
- Vendor commits used: base c4784c51, head 0962c060, pre-next da21321c.
