# Release handoff: main after the 3D split (2026-11-18)

**For:** SkryxDev. **From:** Sophie (Claude Code), for Shalom (`withshalom`).

> **Done (2026-11-18):** main was released as **2.2.0**, the `## 3.0.0` and `## Unreleased` notes merged
> into its section. The update check now names `DraftCraft-DEV/DraftCraft` (since the repository moved,
> 2.1.0's Update Now refused the new releases' files). On the next merge of main into `3D`, keep 3.0.0
> there (step 3 below).

## What to do

1. **Main is ready for release review, as a 2.x.x release without 3D.** Review it, pick the version number, and release it (steps below). Do not release main as 3.0.0: that number is kept for the 3D release.
2. **Commit all future 3D work to the `3D` branch**, never to main. 3D pull requests target `3D`.
3. **A backup of main from before the 3D work was moved is on the branch `backup-11-18-2026`** (`1506ce6`). Leave it as it is.

## What changed

The 3D work moved off main and lives on the `3D` branch until it is finished.

- **PR #26** took the 3D work off main: PR #22 (3D solids on solidcore), PR #23 (3D transforms, lumps, mirrors) and `3D-dev-state.MD`. It reverted them; main's history was not rewritten.
- **PR #27** put the 3D work back on `3D`. From that point, main merges into `3D` without deleting the 3D code.

**These stayed on main:**
- all the DWG work, ACIS in DWG included
- AutoComplete (#24), the Settings file (#25), AREATOTAL, conic Rho
- the `$CTABLESTYLE` fix (`afccf04`)
- the DWG 2000–2010 SAT-text fixes (from `c362986` and `2f3b3ef`)

## Branches

| Branch | What it is |
|---|---|
| `main` | Releasable as 2.x.x. No 3D code (`crates/solid`, the `solids` feature, the 3D commands and views are on `3D`). |
| `3D` | All 3D work, for the 3.0.0 release. Codie works here. |
| `backup-11-18-2026` | main as it was before the 3D work moved (`1506ce6`). Do not change it. |

The rules are in `AGENTS.md`, under "Branches: main is always releasable":
- **3D work goes to `3D`.** A change that needs the solid kernel goes to `3D`.
- **DWG and DXF work goes to main**, ACIS data included, when it works without the `solids` feature.
- **Merge main into `3D` at least weekly.** These merges are ordinary now. When 3D is finished, `3D` comes back to main in one pull request.

## Releasing main as 2.x.x

Main still says 3.0.0, so renumber before tagging:
- `Cargo.toml` says `version = "3.0.0"`.
- `CHANGELOG.md` has a `## 3.0.0` section (DWG writer, What's New, ATTOUT/ATTIN, installers, …). Under `## Unreleased` it also has the Settings file, AutoComplete, AREATOTAL, Rho and the `$CTABLESTYLE` fix.

Steps (2.2.0 is a suggestion; the number is Shalom's call):
1. Merge the `## 3.0.0` notes and the `## Unreleased` lines into one section for the new version. Run `cargo xtask version set 2.X.Y` and check `the_changelog_has_notes_for_this_version`.
2. Run `cargo xtask ci`, merge, then tag `v2.X.Y` on main. The tag builds every package (`docs/releasing.md`).
3. On the next merge of main into `3D`, resolve the version line and the changelog headings by hand: `3D` keeps 3.0.0 for itself.

## Check before the release: DXF → DWG drops some objects

Saving an AutoCAD DXF as DWG does not write:
- MATERIAL
- MESH
- the surfaces
- the solids' history and associative objects

3DSOLID, REGION and BODY are kept. The full list, how it was measured and where the drawings are is in `docs/dwg-handoff.md`, under "What an AutoCAD DXF loses when saved as DWG". This was already on main before the split.

The changelog's DWG line says opening keeps materials, and that holds for DWG → DWG. Make sure the release notes don't suggest that a DXF saved as DWG keeps them, or fix the writer (`crates/io/src/dwg/write/`).

## Small things

- With Rust 1.97, clippy flags `manual_range_contains` in `crates/engine/tests/it/arc_ctrl_direction.rs:119`. CI's toolchain passes it today; it is a one-line fix when CI moves to a newer Rust.
- `gpu_viewport_stencil` needs a GPU adapter, so it fails on machines without one (it passes on CI).
