# Handoff: UCS on the ribbon

For SkryxDev, who designs DraftCraft's ribbon. Written 2026-11-26. This is design advice, not a finished design: the layout is yours.
The aim is that UCS sits where a Windows AutoCAD user looks for it, in the style of our Windows-style ribbon.

## Where we are

DraftCraft has no user coordinate systems yet. What exists today:

- The **UCS icon** at the canvas's lower left (`crates/ui-egui/src/viewcube.rs`, `ucs_icon`), and its click menu (`ucs_menu`), which offers only
  "World (WCS)" and "Hide UCS Icon". Its tooltip says user coordinate systems aren't available yet.
- **View tab ▸ Viewport Tools ▸ UCS Icon** (`ui.toggle.ucsicon`) in `crates/ui-egui/src/ribbon.rs`.
- UCS **records** are read from DXF and kept (`Ucs` in `crates/doc/src/tables.rs`); nothing uses them.

There is no `UCS` command. The engine work (the UCS command and its options, points and angles entered and shown in the current UCS,
ortho, snap, grid, polar and tracking following it, the icon following it) is planned as its own change. Upstream CADCraft's UCS
work is the reference for the engine behaviour, ported into our code. The ribbon is ours and is not taken from them.

## How AutoCAD on Windows presents UCS (to confirm by observation)

Clean-room: look at AutoCAD running with your own drawings. Don't take screenshots into the repository, and don't trace or copy
its icons. What follows is my reading of recent AutoCAD releases on Windows; please check each point in a current release before relying on it.

- **Coordinates panel.** UCS lives in a ribbon panel named **Coordinates**. In the 3D Modeling workspace it is on the **Home** tab
  (and the Visualize tab). In 2D Drafting & Annotation it belongs to the **View** tab; recent releases hide it there by default
  (right-click the ribbon, then Show Panels, then Coordinates).
- **Contents of the panel**, in roughly this arrangement:
  - a large **UCS** button that runs the command with all its options at the command line;
  - a drop-down list of **named UCSs**, with World first;
  - small buttons: **World**, **UCS Previous**, **Origin**, **Z-Axis Vector**, **3 Point**, **X**, **Y**, **Z** (rotate about an axis),
    **View**, **Object**, **Face**;
  - **UCS Icon Properties**, and a panel launcher (the small arrow in the panel title) that opens the UCS dialog (UCSMAN: named UCSs,
    orthographic UCSs, settings).
- **Other places it appears**, which users expect to work the same way:
  - the **UCS icon's** right-click and grip menu: World, Previous, named UCSs, origin and axis moves, icon settings;
  - the menu under the **ViewCube** (WCS, New UCS, the named UCSs);
  - the **command line**: `UCS`, `UCSICON`, `UCSMAN`, and the `UCS` options typed directly.

## Proposal for DraftCraft

### Placement

**View tab, a new "Coordinates" panel right after "Viewport Tools".** We are 2D-first, so I suggest showing it by default rather than hiding
it as AutoCAD does in its 2D workspace. If our ribbon later gets Show Panels, it can follow AutoCAD's default then.
Keep "UCS Icon" where it is in Viewport Tools.

### What goes in it (main branch, 2D)

| Ribbon item | Kind | Command it runs | Notes |
|---|---|---|---|
| UCS | Large button | `ucs` (interactive) | All options at the command line, as AutoCAD |
| Named UCS list | Drop-down | `ucs` with a name (restore) | World first, then saved UCSs; shows the current one |
| World | Small | `ucs.world` | |
| UCS Previous | Small | `ucs` option Previous | Up to the history AutoCAD keeps (10) |
| Origin | Small | `ucs.origin` (interactive: pick a point) | |
| Z (rotate about Z) | Small | `ucs.z` (interactive: angle) | In-plane rotation, 2D-safe |
| 3 Point | Small | `ucs.3point` (interactive) | On main, limited to frames in the drawing's plane; tilted frames are 3D |
| View | Small | `ucs.view` | Aligns X with the screen, follows the view twist |
| UCS Icon Properties | Small | `ucsicon` properties | |
| Panel launcher | Title arrow | `ucsman` (dialog) | Only when we have the dialog; otherwise leave it off |

### 3D branch only

Z-Axis Vector, X and Y rotation, Object (3D objects) and Face tilt the plane out of XY. They belong on the `3D` branch (AGENTS.md:
3D work goes there until it is finished). Leave room for them in the panel's layout so adding them later doesn't reshuffle it.

### Also update

- `ucs_menu` (the icon's menu): World, UCS Previous, the named UCSs, Origin, then the existing Hide UCS Icon.
- The ViewCube's WCS menu: WCS, New UCS, the named UCSs.

## Rules the design must follow (AGENTS.md)

- **Everything is a command.** Each button runs a registered command (`app.run` or `app.start`); nothing on the ribbon has logic of its own.
  Ribbon and menu clicks start the interactive prompt sequence; scripts and automation use the JSON form, which never opens a dialog.
- **Icons are drawn in code** in `crates/ui-egui/src/icons.rs`, original to us. No Autodesk icons, no traced or lookalike copies. Ideas: an
  axis tripod with a square origin (UCS), the tripod with a W (World), the tripod with a back arrow (Previous), a dot moving the tripod
  (Origin), three dots (3 Point), a curved arrow around the tripod (Z), the tripod inside a screen frame (View).
- **Colours come from `theme::Tokens`**, light and dark.
- **Strings:** every new label and tooltip gets an Italian row in `i18n/it.tsv`; menu entries also need `zh.tsv` (a test checks it), and
  please add `zh_tw.tsv` too. The other catalogs fall back to English until translated.
- **Enabled state:** buttons stay disabled until the engine command exists, through the command's `enabled` function.
- **CHANGELOG:** one line under `## Unreleased` when the panel ships.

## Acceptance

- At 1400×900 and at the narrowest supported window, the panel collapses in the same order as its neighbours and nothing is clipped. Check it with
  `ui.screenshot` over the control channel (`docs/control-protocol.md`).
- Each button runs its command; the command line shows the same prompts as typing `UCS`.
- `cargo xtask ci` passes, including the i18n checks.

## Questions for you

1. Show the Coordinates panel by default (my suggestion), or hide it as AutoCAD's 2D workspace does?
2. Large UCS button plus the named list as a split button, or as two separate controls?
3. Should the panel exist (disabled) before the engine work lands, or arrive with it?
