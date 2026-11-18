# 3D-dev-state.MD — DraftCraft 3D: where we are, how far to go, what to do next

**For:** Codie (coding agent). **From:** Chromie (Claude in Chrome) on behalf of Shalom (`withshalom`, WithShalom Studios).
**Written:** 2026-11-18, from a read-through of the DraftCraft-DEV GitHub org, the 3D pull requests, `handoff.md`,
and the SolidCore / SolidCraft repos.

> Read order for you: this file (orientation and status) → `AGENTS.md` in the repo (the rules; they outrank everything,
> including this file) → `docs/solids.md` (how the 3D code works) → `handoff.md` on branch `feat/solids-3d-finish`
> (the full to-do list; section 3 is the master list of what is missing). This file does not replace `handoff.md`.
> It adds the status review, the completeness assessment, the fork/upstream state, and a prioritised start.

---

## 0. TL;DR

- **Goal (owner's words, from the handoff):** DraftCraft's 3D works *like AutoCAD's* and is *perfectly compatible with
  AutoCAD*. What AutoCAD saves opens here as the same objects; what DraftCraft saves opens in AutoCAD as AutoCAD's own objects.
- **Where we are:** a sound **solid-modelling core** (primitives, extrude/revolve, booleans, slice, fillet/chamfer, shell),
  a real **ACIS file translator** (analytic solids verified in AutoCAD 2027 in 8 file/version combinations), and now (PR #23)
  3D transforms and a 3D view that draws legacy 3D objects where they really are. Everything 3D sits behind the cargo
  feature `solids`, except the 3D transform commands and 3D `LINE` points, which ship in release builds.
- **What is not there:** the *working environment* of 3D (UCS, dynamic UCS, 3D object snaps, 3D point input, sub-object
  selection, grips, history, gizmos, perspective), most modelling commands (SWEEP, LOFT, PRESSPULL, POLYSOLID, PYRAMID,
  most of SOLIDEDIT…), surfaces and meshes, section/drawing-view tools, materials/lights/render, and solids in the browser.
- **Biggest single blocker for file compatibility:** NURBS faces written by DraftCraft do not load in AutoCAD in any
  version (error 75004). Diagnosed, not yet fixed. Analytic solids are fine. See §6, task 1.
- **Completeness, by the handoff's own numbered items:** of 38 numbered items (A1–A14, D1–D5, E1–E8, G1–G5, I1–I6),
  **2 are essentially done, 6 are partly done, 30 have not been started.** Counts overweight tiny items, so read §5 for the
  qualitative picture. Honest summary: *a good 3D-solid tab inside a 2D drafter, not yet AutoCAD's 3D Modeling workspace.*

---

## 1. Project context

- **DraftCraft™** — open-source 2D drafting in pure Rust (egui UI, wgpu/CPU drawing), AutoCAD-style Windows ribbon, DWG/DXF
  read+write, MCP/JSON/CLI for agents. MIT OR Apache-2.0 for code. Name/logo are trademarks of WithShalom Studios (brand
  terms in `docs/brand/LICENSE-brand.txt`).
- **Lineage:** fork of **CADCraft** (ArtCraft team). WithShalom Studios did the DWG-fidelity work (first as *DWGCraft*);
  **SkryxDev** built the ribbon, CONIC arcs and AutoCAD-openable DWG (first as *RustDraft* 1.0/1.1). Not affiliated with
  Autodesk or the ArtCraft team.
- **GitHub:** org `DraftCraft-DEV`, repo `DraftCraft-DEV/DraftCraft` (current home). The older `withshalom/draftcraft-dev`
  README is an out-of-date snapshot; ignore it for scope.
- **People/agents:** `withshalom` = Shalom, the owner (merges PRs, runs AutoCAD 2027 checks). `SkryxDev` = co-maker/agent
  that authored most PRs. **Codie** = you.
- **3D stack:** `solidcore` (B-rep kernel, Rust port of NVIDIA SMLib, by `tangency-labs`) is the geometry engine.
  `SolidCraft` (also `tangency-labs`) is a parametric 3D CAD *application* on the same kernel; DraftCraft does **not**
  depend on it, but it is a useful reference (see §4.3).
- **Crate map (3D-relevant):** `crates/solid` (`draftcraft-solid`, L0 — the only crate allowed to name `solidcore*` and
  `cadmpeg*`) · `crates/doc` (entities incl. `Solid3d`, `transform3`) · `crates/render` (`wire3d.rs`) · `crates/engine`
  (commands in `cmd/solids/*`, `cmd/modify3d.rs`, `view3d.rs`) · `crates/io` (`solid_acis.rs`, DWG/DXF mapping, AutoCAD test
  drawings under `tests/data/acad-solids/`) · `crates/ui-egui` (3D view, ribbon `SOLID_TABS`, icons).

---

## 2. Pull requests (DraftCraft-DEV/DraftCraft, read 2026-11-18 ~08:25 PT)

22 PRs total: 3 open, 19 closed/merged.

| PR | Title | Author | State | Notes |
|---|---|---|---|---|
| #25 | CONFIGEXPORT / CONFIGIMPORT (portable TOML config) | SkryxDev | Open, 2/4 checks (2 pending) | Not 3D. 23 files, +1,250/−4. |
| #24 | AutoComplete learns the commands used most | SkryxDev | Open, 2/4 checks (2 pending) | Not 3D. 20 files, +604/−36. |
| **#23** | **3D: lumps, mirrors, objects in space, 3D transforms (NURBS writer diagnosed in handoff)** | SkryxDev | **Open, "Ready to merge", 2/2 checks pass** | Branch `feat/solids-3d-finish`, 58 files, 2 commits. See §3. |
| **#22** | **3D solids on solidcore (replaces #20)** | withshalom | **Merged** (merge commit `0c956e9`) | 21 commits, 108 files. See below. |
| #20 | 3D solids with solidcore, and a 3D Modeling workspace | SkryxDev | Draft, superseded (not in the open count) | CI failed in `gate-solids` (see #22). |
| #19, #18, #16, #15, #8 | DWG writer/reader work (own writer for every version; acadrust removed; solids/regions keep geometry in 2013/2018 data store) | SkryxDev | Closed/merged | DWG stack behind the 3D file work. |

**#22 in brief (merged).** Carried #20's 18 commits and fixed what #20 missed:
1. CI: the round-trip comparator compared region bulges with exact equality (0.5 vs 0.4999999999999999); regions now compare
   bulges with the comparator's existing angle tolerance (8 ε). That failure had stopped `nextest` early, so CI ran
   2,861 of 3,842 tests; all 348 round-trip tests pass with solids.
2. **AutoCAD's own ACIS is kept for unedited solids/bodies/regions** (`Foreign::acis`, keyed by handle). Written back only when
   the entity is unchanged, the handle is the same, the storage form is the same (SAT ≤2010 / SAB in data store ≥2013) and the
   target version ≥ source version. Otherwise DraftCraft's own translation is written.
3. **Solids and regions keep colour, true colour, linetype/scale, lineweight, visibility, transparency on save** (a bug in #20
   dropped them): `entity_props` → `dxf_write::property_groups`.
- #22's own verdict on solidcore: strong for primitives/extrude/revolve/booleans/fillet/chamfer/shell/slice and MASSPROP
  agreement; **weak**: a panic on wasm aborts the page; NURBS trims (no exact pcurve of a spline edge on a spline surface).
- Decision log in #22 worth knowing: where #20's and a parallel local fix did the same job, #20's was kept (`Session::all_or_nothing`,
  `Session::point_z`, ACIS-first saving); the interim mesh-in-XRECORD file form stays only as a rare fallback.

---

## 3. PR #23 review (the pending 3D PR)

**What it claims (from the PR body):** several lumps in one solid (A3); mirrored ACIS bodies read (A4, reading); the 3D view
draws non-solid objects where they are in space (A12); `LINE` takes 3D points; new commands 3DMOVE (3M), 3DROTATE (3R),
ROTATE3D, 3DSCALE, MIRROR3D, 3DALIGN (3AL), 3DARRAY (3A) — "also in release builds without solids"; plus **AutoCAD 2027's
verdict** on the NURBS writer and 43 new AutoCAD test drawings.

**What I checked** (sampled the raw source on branch `feat/solids-3d-finish`; I did **not** build, run, or read every diff line):

| Check | Result |
|---|---|
| File list | 58 files: 43 are AutoCAD test drawings + `README-finish.md`; the rest are code, i18n, docs, `ATTRIBUTION.md`, `CHANGELOG.md`, `handoff.md`. The +374,919 line count is almost certainly test-drawing text (inference from the file list). |
| `unwrap()`/`expect()`/`panic!`/`unreachable!`/`todo!` in non-test code | **0** in `transform3.rs`, `wire3d.rs`, `modify3d.rs`, `body.rs`, `acis/read.rs`. No `unsafe` in `transform3.rs`. |
| Resource caps | `modify3d.rs`: `MAX_ARRAY_COPIES = 100_000`, enforced on JSON, prompt and saturating-multiplication paths. `wire3d.rs`: `MAX_POINTS`. `acis/read.rs`: `MAX_DATA` (256 MiB), `MAX_FACES` (200k), `MAX_POLES` (1<<20), `MAX_DEGREE` (25), `MAX_HELIX_SPANS`, **`MAX_LUMPS` (10,000)**. |
| Mirror handling | `placed()` in `acis/read.rs`: determinant < −1e-300 → returns the mirror matrix for the kernel to apply (`Body::transform`); degenerate/non-finite determinants refused. `transformed()` (used by regions) still refuses mirrors, as the handoff says. |
| Tests added | `modify3d/tests.rs`: 6 (move/rotate/scale/mirror in space; circle turned up; text that can't tilt is left and reported; align by 1/2/3 points; array rows/cols/levels/polar; prompts with typed 3D points). Includes a "hostile sizes are refused" case. `wire3d.rs`: 4 test markers. `transform3.rs`: 3. `acis/tests.rs`: `a_solid_of_separate_pieces_reads_as_one_body`, plus existing `hostile_acis_is_an_error_not_a_panic`, `hostile_regions_never_crash`. CI shows 2/2 green. |

**Findings, in order of importance (for Shalom's decision and for your follow-up):**

1. **Merging #23 does not fix the NURBS-writer failure.** It *documents* it. Per the PR and handoff A1: AutoCAD's LOFT/SWEEP
   solids, once moved in DraftCraft and saved, don't load in AutoCAD in any of 8 version/format combinations (SAT: error
   `75004`/invalid ShapeManager solid; SAB: "ASM body missing"). Release builds are unaffected (solids is feature-gated), but
   anyone using `--features solids` can corrupt a curved AutoCAD solid by moving it and saving. This is task 1 in §6.
2. **Provenance/licence of the new test drawings needs the owner's call.** `README-finish.md` says the 43 files were made in
   *AutoCAD 2027 for Mac, educational version* ("VERSIONE DIDATTICA (NON COMMERCIALE)") and "may carry AutoCAD's educational
   stamp." They are committed into an MIT/Apache repo with a clean-room policy. The handoff's rule is that AutoCAD-saved
   files are fine as test data with an `ATTRIBUTION.md` row, but it does not address the educational-licence stamp. I'm not a
   lawyer; flag for Shalom before this ships widely. (Checking whether `ATTRIBUTION.md` rows exist for all 43 is something
   `cargo xtask assets` enforces, so CI passing implies rows exist — I did not read the rows.)
3. **Some 3D behaviour is not behind `solids`.** The seven 3D-transform commands and `LINE` with `x,y,z` ship in release
   builds. But `handoff.md` §C says CIRCLE, ARC, PLINE, COPY, MOVE… still **silently drop the Z** of a typed point, and many
   entity kinds can't tilt out of XY (`NotIn3d`: text, mtext, blocks, dimensions, hatches, splines, regions, images, …; the
   command reports how many were skipped). Inconsistent Z handling in a release build is a user-visible risk. Either gate it
   or finish "3D points in every command" soon.
4. **Verification gaps the PR itself admits:** no test yet against AutoCAD's `mirror-*` drawing (needs its MASSPROP values —
   they are in `README-finish.md`, see §8); no AutoCAD check of a DraftCraft UNION of disjoint solids; `wire3d` not compared
   against `legacy3d-*`; mirrored **regions** still refused; transparency lost in DWG 2000 (A1b).
5. **Thin test depth for 7 new commands.** Six engine tests cover seven commands. Reasonable for a first pass, but the
   handoff's own command checklist asks for JSON form, interactive form, bad input, all-or-nothing on several objects, and a
   hostile-input test *per command*. Worth auditing 3DROTATE/ROTATE3D/3DALIGN/MIRROR3D individually.
6. **`handoff.md` on `main` is stale** (it predates #23 and still says branch `feat/solids-3d`, PR #20). The accurate copy is
   on `feat/solids-3d-finish`. Merging #23 will update it.

**My read:** the code I sampled is clean on the project's own "never crash" rules and the caps are present. #23 looks safe to
merge from a code standpoint, pending Shalom's decision on finding 2. I haven't run it.

---

## 4. Upstream and forks

### 4.1 SolidCore (kernel) — `tangency-labs/solidcore`; Shalom's fork `withshalom/solidcore`
- **Scope:** Rust port of NVIDIA SMLib (from `usd-brep` v1.0.1, Apache-2.0). Safe Rust, no panics on bad input (goal),
  deterministic, no global state, builds for `wasm32-unknown-unknown`. Out of scope: OCCT translators, USD/Hydra, Python.
- **Progress page (tangency-labs.github.io/solidcore, generated on every push to `main`):** 99.3% of upstream lines ported
  (405,737 of 408,742); 282 units: **254 green, 27 partial, 1 blocked, 0 todo/claimed**; 9/9 primitives build and pass.
  Module %: everything 100% except `blend` 24/25 units (93%). Surface/surface-intersection corpus (119 cases): 78 PASS,
  21 INCOMPLETE, 10 WRONG TOPOLOGY, 9 DEFERRED, 1 ERROR (C++ reference: 81 PASS). 1,364 upstream issues logged while porting.
- **Read those numbers carefully (the site says so itself):** "green/partial" are *historical port-plan statuses, not current CI
  verdicts*; goldens prove an output is *unchanged*, not *correct*. API is experimental (0.0.x); the project says it stays
  0.0.x "until it is usable end to end." Weakest areas for us: blend (fillets on curved geometry), SSI, NURBS trims.
- **WASM caveat:** on `wasm32-unknown-unknown` a stack overflow corrupts memory (no guard page) and a panic aborts the page.
  Consumers should link with a larger stack (e.g. `-C link-arg=-zstack-size=8388608`).
- **What DraftCraft pins:** `tangency-labs/solidcore` rev **`ce5c6e9`** (commit dated 2026-11-14), *not* Shalom's fork.
- **Fork state:** `withshalom/solidcore` latest commit is **2026-10-29** (`e016b8cf3`); upstream `main` is at **2026-11-18**
  (`f3cb5f75f`). So the fork is **~3 weeks behind and does not contain the pinned `ce5c6e9`.** (I could not get a reliable
  commit count — GitHub's compare page returned "nothing to compare" for the SolidCore fork, which contradicts the dates, so
  I'm relying on the commit dates.) Because DraftCraft pins upstream directly, the fork is **not in the build path**; syncing it
  is housekeeping unless Shalom wants to land changes through his fork.
- **Planned kernel-side work from #22:** `solidcore-worker` crate (Web Worker host: protocol with `.smb` bodies, stateless
  executor, platform-neutral host that fails a dead worker's calls with `WorkerDied` and respawns it, wasm32 transport),
  ~1,100 lines, on branch `wasm-worker-host` in the authors' local solidcore checkout. **Written, not yet published**; belongs in a
  solidcore PR. DraftCraft-side use of it is not written.

### 4.2 SolidCraft (app) — `tangency-labs/solidcraft`; fork `withshalom/solidcraft`
- Parametric 3D CAD in pure Rust, desktop + web + CLI + MCP, on solidcore. Features (per its README): sketches with its own
  constraint solver, extrude/revolve/sweep/loft/fillet/chamfer/shell/draft/holes/threads/patterns/mirror/split/combine,
  parameters/configurations, assemblies, sheet metal, STEP/IGES/3MF/STL/OBJ/glTF I/O. 75 of 76 public reference parts match
  analytic/FreeCAD volumes, areas, centres of mass and bounding boxes. Licence MIT OR Apache-2.0; **brand/UI is licensed
  separately (LICENSE-BRAND)** — don't copy its icons, theme, name or UI kit.
- **Fork state:** the fork is **1 commit ahead** of upstream — a `github-actions[bot]` "Regenerate generated files and fold
  pending files [skip ci]" commit dated 2026-11-11, i.e. **no real custom work** — and **~1,270 commits behind** per GitHub's compare page
  (treat the exact number as approximate; upstream is very active). Latest fork commit 2026-11-11 vs upstream 2026-11-18.
- Shalom also has a private `solidsketch` repo ("a friendly, art-focused 3D modeler built on the SolidCraft CAD engine") and a
  fork `getcadcraft` (CadCraft: "2D drafting and 3D modeling… 3D tab for solids"). Not examined.

### 4.3 How Codie can use SolidCraft
SolidCraft already wraps solidcore's sweep, loft, thicken, draft, offset, fillet and STEP/STL I/O. Its feature code and tests
are a good *reference* for calling those kernel ops correctly (the handoff lists the available ops at `ce5c6e9`: `extrude`,
`revolve`, `sweep_placed`, `sweep_controlled`, `sweep_stations`, `sweep_two_rails`, `screw_sweep`, `loft`, `loft_rails`,
`loft_to_point`, `loft_blend`, `thicken`, `offset_faces`, `offset_sheet`, `remove_faces`, `draft`, `imprint`, `split_body`,
`split_body_by_sheet`, `split_faces`, `split_face_by_curves`, `stitch`, `extend_sheet`, `fillet_sheets`, `network`,
`transform`, `check`/`check_solid`/`validate`; `solidcore-step` reads/writes STEP). **Before borrowing anything, confirm with
`AGENTS.md` and Shalom** how code from another MIT/Apache project must be attributed in this repo, and never take SolidCraft's
UI/brand assets. Everything must still go through `draftcraft-solid`'s facade with its own tests.

---

## 5. Completeness vs the AutoCAD-3D goal

Yardstick: `handoff.md` §3 (checked against the code at the tip of the 3D work, updated 2026-11-17).

### 5.1 Scorecard by area

| Area | State | Detail |
|---|---|---|
| **Solid modelling commands that exist** | **Working** | BOX, WEDGE, CYLINDER, CONE, SPHERE, TORUS; EXTRUDE (taper), REVOLVE; UNION/SUBTRACT/INTERSECT; SLICE; FILLETEDGE; CHAMFEREDGE; SOLIDEDIT *Shell only*; INTERFERE; MASSPROP (solids + regions); FLATSHOT; SECTION; VPOINT/3DORBIT/PLAN/VSCURRENT. All with JSON form, prompts, tests. |
| **3D view** | Working, limited | Orthographic only, CPU-painted (egui painter), one view per drawing, model space only. 9 visual styles, ViewCube, nav bar. #23 adds correct placement of legacy 3D objects. 10,000-face target never measured. |
| **ACIS/DWG/DXF for solids** | Strong for analytic, **broken for moved NURBS** | AutoCAD → DraftCraft reads DXF 2000/2010/2018, DWG 2000/2013/2018 (MASSPROP matches to 4 decimals). DraftCraft → AutoCAD verified for box/cylinder/cone/sphere/torus/frustum/holed box/rotated box in 8 version/format combos (AUDIT clean, Check valid, MASSPROP equal). AutoCAD's own solids round-trip byte-for-byte when unedited (#22). Properties (colour etc.) survive except transparency in DWG 2000. |
| **3D transforms** | Done (once #23 merges) | 3DMOVE/3DROTATE/ROTATE3D/3DSCALE/MIRROR3D/3DALIGN/3DARRAY. Missing: gizmos, several keyword options, many entity kinds can't tilt, other commands drop Z. |
| **Missing modelling commands** | Not started | POLYSOLID, PYRAMID, SWEEP, LOFT, PRESSPULL, THICKEN, IMPRINT, OFFSETEDGE, XEDGES, CONVTOSOLID/SURFACE/MESH/NURBS, all SOLIDEDIT options except Shell, many options on existing commands. |
| **Sub-objects, grips, history, Properties** | Not started | No Ctrl+click face/edge/vertex picking; a solid's grip only translates it; no primitive history (A8) so no dimension grips or editable Box Length/Width/Height; Properties palette doesn't edit solids. |
| **3D working environment** | **Not started — biggest gap** | No UCS command (only the named-UCS table), no dynamic UCS (F6), no 3D object snaps (F4), point prompts give the XY-plane point only, no perspective, cameras, walk/fly, per-viewport 3D, 3D window/crossing selection. The handoff: "without these, nothing else in 3D feels like AutoCAD." |
| **Sections & documentation** | Minimal | FLATSHOT/SECTION exist. No SECTIONPLANE/LIVESECTION/GENERATESECTION, SOLVIEW/SOLDRAW/SOLPROF, VIEWBASE family, 3D plot styles. |
| **Visualisation** | Visual styles only | No visual-style manager/user styles, materials, lights/sun, RENDER, GPU path. |
| **Surfaces & meshes** | Not started | SURFACE/MESH entities are kept as foreign data, not drawn. No Surface/Mesh ribbon tabs. |
| **Exchange** | Not started | No STEP/IGES/STL in DraftCraft's `export`/`import` (dxf/dwg/svg/png/pdf only). |
| **Web build** | Blocked | `draftcraft-web` has no `solids`; wasm kernel panics abort the page. Plan: kernel in a Web Worker (see §4.1). |

### 5.2 Tally of the 38 numbered handoff items

| Group | Done | Partial | Not started |
|---|---|---|---|
| A (A1–A14, file/format) | A3 (lumps), A12 (legacy 3D drawn) — both with follow-ups | A1 (NURBS writer; quick fix pending), A4 (mirrors: solids read, regions not), A8 (history: only "kept if unedited"), A9 (face colours: only "kept if unedited") | A2, A5, A6, A7, A10, A11, A13, A14 |
| D (D1–D5, sub-objects/grips/Properties) | — | — | D1–D5 |
| E (E1–E8, working environment) | — | — | E1–E8 |
| G (G1–G5, visualisation) | — | — | G1–G5 |
| I (I1–I6, platform) | — | I3 (hostile tests exist, must extend), I5 (undo partly checked) | I1, I2, I4, I6 |
| **Total (38)** | **2** | **6** | **30** |

Plus unnumbered groups, not counted above: **B** (modelling commands that don't exist; none of the listed missing commands is started),
**C** (the 7 core 3D-transform commands are done once #23 merges; gizmos, options and unsupported entity kinds are open), **F** (all open),
**H** (all open).

> **Correction to what Chromie told Shalom in chat earlier:** I said "perhaps a fifth of the items are done." By this stricter
> tally it is closer to **2 of 38 done and 8 of 38 started** (~5% done, ~21% started). The "fifth" figure is only fair if you
> count partial work as done and weight by the importance of the solid core, which is genuinely the hardest part to build.

---

## 6. Recommended plan for Codie (follows `handoff.md` §4, with concrete first steps)

Always: gates green (`cargo xtask ci` **and** `cargo xtask ci --solids`, plus `cargo check --target wasm32-unknown-unknown -p
draftcraft-solid --features kernel`), update `docs/solids.md`, update `handoff.md` (move done items to its section 1).

**Task 1 — A1 quick fix: keep AutoCAD's ACIS under a body transform** (highest value, smallest change).
A solid read from AutoCAD and then only moved/rotated/mirrored/uniformly scaled has the same body and a new `placement`.
Today `unchanged()` in `crates/io/src/solid_acis.rs` then drops AutoCAD's ACIS and rewrites the geometry (hitting the NURBS bugs).
Instead keep AutoCAD's data and set the *body transform*:
- **SAT:** body is record 0 (`body $-1 $lump $wire $transform #`). AutoCAD writes
  `transform $-1 r11 r12 r13 r21 r22 r23 r31 r32 r33 tx ty tz scale rotate|no_rotate reflect|no_reflect no_shear #`; rows are the
  images of the axes (`p' = p·A·scale + t`). Compose with an existing transform or append a record and point the body at it.
  Bump the record count in the header line (`400 87 1 0`).
- **SAB:** cadmpeg frames records with byte offsets (`sab::Record { offset, len }`). Patch the body's transform `Ref`
  (tag 0x0c, i32) in place; insert the transform record before the end marker. The ignored test `dump_sab`
  (`DRAFTCRAFT_SAB_IN=mirror-2018.dxf DRAFTCRAFT_SAB_OUT=/tmp/x cargo test -p draftcraft-io --features solids --lib dump_sab -- --ignored`)
  dumps AutoCAD's SAB so you can copy the exact token layout.
- Read the result back in a test (volume/centroid moved), then ask Shalom to check the `dc-free-*` files in AutoCAD.

**Task 2 — A1 full: fix the NURBS writer for geometry DraftCraft actually changed.** Diagnosis (from the handoff, by comparing our
SAT against AutoCAD's for the same loft/sweep with a coedge/pcurve checker):
1. *Pcurves aren't on the edge's parameter.* ACIS needs `S(pcurve(t)) = edge_curve(t)` (reversed coedges use −t). Ours lie on
   the right circle at another angle (up to 0.01 rad / 0.47 units; AutoCAD's: 1.2e-4). Cause: the STEP pcurve is parametrised
   along the surface's u and `write.rs` maps it linearly to the ACIS edge parameter. **Fix:** reparametrise each pcurve on a
   spline face — sample the edge at its own parameter, 1-D Newton on `|S(P(s)) − E(t)|` for the pcurve parameter, refit a 2-D
   B-spline in t, and write the *real* deviation as the pcurve's fit tolerance (we write 0).
2. *Our surfaces aren't AutoCAD's.* Read from the 2018 DWG they come back as cubic patches with interior knots of multiplicity 3
   (C0). AutoCAD's own 2000 loft has simple knots (C2). ShapeManager may reject C0 knots inside a face; if (1) alone doesn't fix
   the loft, remove those knots (geometrically smooth) before writing.
3. *Seam and closed edges are written differently.* AutoCAD writes a periodic surface's seam as `intcurve-curve { parcur … }` with
   `pcurve $-1 1 $<intcurve> 0 0` / `pcurve $-1 -1 $<intcurve> -6.28… 0`, and closed circle edges' pcurves as `nubs 1 periodic`. We
   write an `exactcur` approximation with two free `exppc` pcurves `open` whose ends miss the vertex by 1.8e-4.
4. In SAB both fail, so once SAT works also check the SAB tail after a spline's discontinuity lists (`Tok::Sab` in
   `acis/model.rs`).
Test files come from the ignored test `write_files_for_autocad` in `crates/io/src/solid_acis/tests.rs` (`dc-free-*`). Source
solids: `acad-solids/solids-free-2018.dwg`. Each iteration needs Shalom to open files in AutoCAD 2027 (open, AUDIT,
SOLIDEDIT ▸ Body ▸ Check, MASSPROP vs DraftCraft's MASSPROP).

**Task 3 — A1b, A2, and close out A3/A4 with tests.**
- A1b: DWG 2000 has no transparency field; observe how AutoCAD itself saves transparency in DWG 2000 (likely xdata) and mirror that.
- A2: `modeler()` in `crates/io/src/dwg/entities.rs` reads only SAT before 2013 (`bs != 1` → not read). DWG 2004–2010 can hold binary
  SAB right after the entity header. Read it through the same path as the 2013+ data store, with size caps. Test drawings
  `sab-2004/2007/2010.dwg` already exist (values in §8).
- A3/A4: write tests against `lumps-*` and `mirror-*` using the AutoCAD MASSPROP values in §8; implement SOLIDEDIT Body ▸ seParate;
  mirrored *regions* (`transformed()` still refuses them).

**Task 4 — E1–E4: UCS, dynamic UCS, 3D object snaps, 3D point input** (and finish C: "3D points in every command").
Big engine change: every command must work in the current UCS (typed coordinates are UCS coordinates; 2D drawing commands draw in the
UCS XY plane; ELEV). Plan carefully and test widely. Until this lands, CIRCLE/ARC/PLINE/COPY/MOVE silently drop Z.

**Task 5 — A5/A6/A9/A10/A11 against the new AutoCAD drawings** (`procsurf`, `regions-ucs`, `facecolor`, `surfaces`, `mesh`,
`history`). Read blends/tapers/offset surfaces from their approximation caches as `read/legacy.rs` does for `loft_spl_sur`/`sweep_sur`.

**Task 6 — D1–D3 with A8:** sub-object selection, grips per primitive, Properties palette, primitive history (record type and
parameters on `Solid3d`; write/read AutoCAD's `ACSH_*` history objects). Cylinder/box/etc. grips depend on A8.

**Task 7 — B: modelling commands.** PRESSPULL, SWEEP, LOFT, POLYSOLID, PYRAMID, rest of SOLIDEDIT (Face: Extrude/Move/Rotate/Offset/
Taper/Delete/Copy/coLor; Edge: Copy/coLor; Body: Imprint/seParate/cLean/Check), THICKEN (needs A10), the CONVTO* family, missing options
on EXTRUDE/REVOLVE/SLICE/FILLETEDGE/CHAMFEREDGE. Wrap kernel ops in the `draftcraft-solid` facade first, with its own tests.

**Task 8 — A13 exchange:** STEP via `solidcore-step`, STL from the display mesh (binary + ASCII), IGES if possible. `export` in
`crates/engine/src/cmd/exchange.rs` handles dxf/dwg/svg/png/pdf only today.

**Task 9 (later) — E5–E8, F, G, H, I.** Perspective/cameras/walk/fly, per-viewport 3D, section objects, drawing views, visual-style
manager, materials, lights, render, **GPU path for the 3D view** (measure the 10,000-face target, also in the browser), surfaces and
meshes with Surface/Mesh ribbon tabs. **I1 (web):** build the DraftCraft side of the Web Worker host after solidcore publishes
`solidcore-worker`. **I2:** decide with Shalom when `solids` goes into release builds, then write CHANGELOG lines for 3D.

---

## 7. Rules that bite (from `handoff.md`; `AGENTS.md` outranks these)

- **Branching:** the handoff says commit and push to the PR's branch (`feat/solids-3d-finish`) and its PR; **never push to `main`,
  never merge a PR — the owner merges.** If #23 has merged by the time you start, ask Shalom which branch to use.
- **Feature gating:** all 3D stays behind cargo feature `solids`; without it the build, tests and wasm check must stay exactly as on `main`.
- **Gates before every commit:** `cargo xtask ci` and `cargo xtask ci --solids`; also the wasm check above.
- **Known acceptable test noise:** three macOS-only failures (`read_file_refuses_a_name_that_is_not_unicode`,
  `usage::non_unicode_output_names`, `thai_marks_stack_in_an_installed_font`); `mcp_connect::connect_to_closed_port_fails` can fail
  if the port is busy (passes alone); four AutoCAD DXFs (`solids-basic-2000/2010/2018.dxf`, `solids-edit-2000.dxf`) may show as
  modified from line endings only — don't commit them.
- **Never crash:** no `unwrap`/`expect`/`panic!`/`unreachable!`/indexing on input; checked arithmetic; cap loops and allocations;
  every crash fix gets a test. Kernel calls go through the facade (turns kernel errors into `SolidError`).
- **Clean-room:** observe AutoCAD only from the outside. Never read anything inside an Autodesk install, never commit AutoCAD
  screenshots, never copy its icons/materials/visual-style definitions/render presets/help text. AutoCAD-saved DWG/DXF are allowed as test
  data with a row in `ATTRIBUTION.md` (`cargo xtask assets` checks). File formats come from public specs (Autodesk DXF Reference,
  ODA DWG spec, Spatial's published ACIS layouts) and the files themselves.
- **No GPL code, not even reading it:** no LibreDWG, LibreCAD, QCAD, FreeCAD, SolveSpace, OpenCASCADE.
- **Layers:** only `draftcraft-solid` names `solidcore*` and `cadmpeg*`; nothing below L5 uses egui (`cargo xtask layers`).
- **New command checklist (every one):** `CommandSpec` in `crates/engine/src/cmd/solids/<file>.rs` (id = AutoCAD's name in lower case, label,
  menu path, aliases, `params` doc, JSON `run`, `interactive` prompt machine following AutoCAD's prompt sequence); programmatic calls never open
  dialogs; tests (JSON, interactive, bad input, all-or-nothing on several objects, hostile input); a ribbon place in `SOLID_TABS` with an
  icon drawn in code in `icons.rs`; Italian label in `i18n/it.tsv` (+ `zh.tsv` for menu entries); `docs/solids.md` updated; a CHANGELOG line
  only for changes release-build users see.
- **AutoCAD checks:** Shalom has AutoCAD 2027 and runs them on request. Give him files and what to compare. Generate check drawings with
  `DRAFTCRAFT_ACIS_OUT=dir cargo test -p draftcraft-io --features solids write_files_for_autocad -- --ignored`. In AutoCAD: open each file,
  note messages, AUDIT, SOLIDEDIT ▸ Body ▸ Check per solid, MASSPROP (volume, centroid, bounding box) vs DraftCraft's. For new object types, have
  him save the same object from AutoCAD as DWG/DXF 2000/2010/2018 into `crates/io/tests/data/acad-solids/` with a README of MASSPROP values and a test.

---

## 8. Reference data: AutoCAD 2027 test drawings (from `crates/io/tests/data/acad-solids/README-finish.md`)

Made 2026-11-17 in AutoCAD 2027 for Mac (educational build, Italian UI), `acadiso.dwt`, `INSUNITS` 4 (mm), `SOLIDHIST` as noted. Every drawing AUDIT-clean;
every 3DSOLID passed the body check. Volumes/centroids are AutoCAD MASSPROP at density 1. Files: `<name>-2018.dwg`, `-2018.dxf`, `-2000.dxf`
(`sab` also `-2010/-2007/-2004.dwg`).

| Drawing | Object | Volume / area | Centroid | Bounding box |
|---|---|---|---|---|
| sab | box 0,0,0–100,60,40 (2DB) | 240000.0000 | 50, 30, 20 | 0–100, 0–60, 0–40 |
| sab | cylinder r25 h50 at 200,0 (2DF) | 98174.7704 | 200, 0, 25 | 175–225, −25–25, 0–50 |
| sab | sphere r30 at 400,0,30 (2E3) | 113097.3355 | 400, 0, 30 | 370–430, −30–30, 0–60 |
| lumps | UNION of two apart 50-boxes, **one 3DSOLID, 2 lumps** (2DF) | 250000.0000 | 125, 25, 25 | 0–250, 0–50, 0–50 |
| mirror | mirrored wedge, MIRROR (2DB) | 75000.0000 | −33.3333, 25, 10 | −100–0, 0–50, 0–30 |
| mirror | mirrored cone apex down, MIRROR3D (2DF) | 56548.6678 | 200, 0, −15 | 170–230, −30–30, −60–0 |
| procsurf | cylinder + FILLETEDGE r5 top edge (2DB) | 168672.3642 | 0, 0, 29.8334 | −30–30, −30–30, 0–60 |
| procsurf | box + SOLIDEDIT Face Taper 10° (2E1) | 135536.3049 | 151.7266, 0, 19.5837 | 120–180, −30–30, 0–40 |
| procsurf | cylinder + SOLIDEDIT Face Offset 5 (2E5) | 230907.0600 | 300, 0, 30 | 265–335, −35–35, 0–60 |
| procsurf | circle r2 swept along 3-turn helix (2EB) | 4809.8419 | 450.1110, 2.0906, 25.1543 | 421.3123–481.9988, −27.0225–30.3532, −1.9889–61.9068 |
| regions-ucs | rectangle region in WCS XZ plane (2DA) | area 1500.0000, perimeter 160 | 25, 0, 15 | 0–50, 0, 0–30 |
| regions-ucs | ellipse region (2DC) | area 628.3185, perimeter 96.8845 | 120, 0 | 100–140, −10–10 |
| regions-ucs | closed-spline region (2DE) | area 2567.6843, perimeter 182.8775 | 219.1624, 24.8072 | 188.8559–251.3582, −2.4025–50.2120 |
| history | box with history (2DB), `SOLIDHIST` 1 | 240000.0000 | 50, 30, 20 | 0–100, 0–60, 0–40 |
| history | cylinder r20 h50 (2DF) | 62831.8531 | 200, 0, 25 | 180–220, −20–20, 0–50 |
| history | box − cylinder (2E3) | 211725.6661 | 350, 30, 20 | 300–400, 0–60, 0–40 |
| facecolor | box with red top face (2DB) | 240000.0000 | 50, 30, 20 | 0–100, 0–60, 0–40 |
| surfaces | PLANESURF 0,0–50,50 (2D9) | area 2500 | — | 0–50, 0–50, 0 |
| surfaces | EXTRUDED / LOFTED / REVOLVED / SWEPT surfaces | areas 2356.1945 / 1802.7756 / 3885.9355 / 1884.9556 | — | see README |
| mesh | box MESH smoothness 0 / smoothed copy smoothness 1 | area 15000.0000 / 12739.0350 | — | 0–50 / 100–150 etc. |

What AutoCAD shows for a history box (`history`, 2DB): Solid type = Box, Length 100, Width 60, Height 40, position 50,30,0 (centre of the base), `IsPrimitive` 1,
`RecordHistory` 1, `ShowHistory` 0, surface Area 24800 — the target for A8/D3. `legacy3d` holds a 3DFACE, 3D polyline, LINE, circle with thickness 30,
circle with extrusion 0,−1,0, 3×3 polygon mesh, and a 5-vertex polyface (square pyramid) — the target for A12 follow-ups.

---

## 9. Open questions for Shalom (decisions only the owner can make)

1. **Test-drawing provenance:** are the 43 educational-version AutoCAD files acceptable to commit into this MIT/Apache repo? (§3 finding 2.)
2. **Merge #23?** Code looks clean on a sample read (§3); the open item is question 1. Mergeable per GitHub, 2/2 checks green.
3. **Release gating:** the 3D transform commands and 3D `LINE` already ship in release builds while the rest of 3D is gated. Keep them
   ungated, gate them, or finish "3D points in every command" first? (When should `solids` enter release builds — handoff I2.)
4. **Branch for Codie's work** once #23 merges: new branch from `main`? (The handoff still says `feat/solids-3d-finish`.)
5. **Forks:** sync `withshalom/solidcore` (≈3 weeks behind upstream, missing the pinned `ce5c6e9`) and `withshalom/solidcraft` (≈1,270 commits behind), or leave them as bookmarks?
6. **Web:** approve the Web Worker approach for solids in the browser, and who publishes `solidcore-worker` upstream?

---

## 10. What this review did and did not verify

**Did:** read the DraftCraft-DEV repo README; the PR list; PR descriptions for #22, #23, #24, #25; the full `handoff.md` on `feat/solids-3d-finish`;
`README-finish.md`; sampled source on the #23 branch (`transform3.rs`, `wire3d.rs`, `modify3d.rs`, `modify3d/tests.rs`, `body.rs`, `acis/read.rs`,
`acis/tests.rs`) for panic-safety, caps and test names; SolidCore and SolidCraft READMEs; SolidCore's generated progress page; GitHub's commit pages
and compare pages for both forks and upstream.

**Did not:** build or run anything; read every diff line; open `AGENTS.md`, `ROADMAP.md`, `docs/solids.md`, `docs/gaps.md`, or `ATTRIBUTION.md`; open issues
(none listed on Shalom's dashboard); inspect `getcadcraft`, `solidsketch`, or the web build; verify fork commit counts beyond what GitHub's compare pages
displayed (the SolidCore compare page was unreliable — dates used instead). Pending checks on #24/#25 were still running when read.

**Sources (all on GitHub unless noted):** `DraftCraft-DEV/DraftCraft` (README, `pull/22`, `pull/23`, `pull/24`, `pull/25`, `pulls`, `handoff.md` and
`README-finish.md` on `feat/solids-3d-finish`); `withshalom/solidcore`, `withshalom/solidcraft`, `tangency-labs/solidcore`, `tangency-labs/solidcraft`
(commits and compare pages, commit `ce5c6e9`); `tangency-labs.github.io/solidcore` (progress site).

---

Signed,
**Chromie** — Claude in Chrome, on behalf of Shalom (WithShalom Studios)
2026-11-18
