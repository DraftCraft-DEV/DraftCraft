# DraftCraft changelog

What changed in each release, for the people who use DraftCraft. The app shows the new sections in
its What's New window at the first start after an update (and from Help › What's New), so this file
is built into every release: nothing is downloaded to show it.

How to write it (see AGENTS.md):
- Every user-visible change adds a line under `## Unreleased`, in plain words for users.
- One `- ` line per change; `### ` headings group them. Lines are not wrapped.
- Before a release, `cargo xtask version set X.Y.Z` moves the `## Unreleased` lines into `## X.Y.Z`.

## Unreleased

### New
- Your settings in one file: Export Settings and Import Settings (Manage › Customization, or CONFIGEXPORT and CONFIGIMPORT) save the theme, Options, units, grid and snap, lineweights, plot style and command aliases to a TOML file and read it back on any computer.
- The command line's AutoComplete learns which commands you use: the ones you run most, and most recently, move up the list, so typing LI soon offers LINE first if you draw lines more than you list. The arrow keys move through the suggestions, and Reset Suggestion Order in the command line's right-click menu forgets what was learned.
- AREATOTAL (Home › Utilities › Total Area): select objects to get their total area and perimeter and the total length of the open ones, with how many were measured; the totals can be placed in the drawing as text. AREA also totals several objects at once from scripts and agents, with objects to subtract.
- The Properties palette shows the Rho of a selected conic arc (an ellipse, parabola or hyperbola piece drawn with CONIC, or any spline of that shape) under Geometry, and typing a new value between 0 and 1 reshapes it; with several conics selected it shows their common Rho.

### Fixed
- DWG: the materials AutoCAD 2027 puts in every drawing (ByLayer, ByBlock and Global) are kept when a DXF file from it is saved as DWG and when one of its DWG files is opened; before, they were left out.
- AutoCAD no longer reports an unknown header variable $CTABLESTYLE when it opens a DXF file saved by DraftCraft: the current table style is saved where AutoCAD keeps it.

## 3.0.0

### New
- DWG: DraftCraft reads and writes DWG with its own code, in every version from AutoCAD 2000 to 2018. The files it saves open in AutoCAD without recovery.
- DWG: drawings open much faster (100,000 layers in under a second) and keep far more: extension data, dynamic blocks, visual styles, materials, multileaders, tables, solids and regions.
- What's New: after an update, DraftCraft shows what changed in this window. Open it again from Help › What's New.
- ATTOUT and ATTIN (Express Tools): write block attribute values to a tab-delimited file, edit them in a spreadsheet, and read them back into the drawing.
- Drawing units on the status bar: see the length format at a glance, switch it, or open the Drawing Units dialog.
- Selecting with a window highlights the objects it will take while you drag.
- Installers: a Windows MSI and a Linux .deb that open .dxf and .dwg files, builds for Intel Macs and Linux ARM64, and a new DMG window on macOS.
- In the browser, DraftCraft can be installed as an app, and its menus sit in the top row as on Windows.

### Improved
- Arc length and jogged radius dimensions are saved as AutoCAD's own dimension types in DXF and DWG.
- Saving as DWG tells you about texts longer than a DWG text field can hold.
- Block Editor dynamic blocks keep their parameters and actions through a DWG save.
- Changing UNITS also updates the dimension styles that followed the drawing's units.
- Blocks and dimensions are picked on their lines and text, not anywhere inside their bounding box.
- The ribbon's layer list moves the selected objects to the layer you pick. Its bulb, sun and lock switch the layer, and the list stays open while you do.
- ARC: hold Ctrl (Cmd on macOS) to switch the direction, whichever way the arc is drawn.
- Command line: the history lines fade when the command line is not in use and step aside when the pointer is over them, so that part of the drawing can be edited.
- Command line: suggestions no longer cover the history window (F2), and a click on the drawing closes it.

## 2.1.0

### Picking near text
- Text at an angle is picked around its letters, turned with the text, not anywhere in a larger box around it. This applies to clicks, hover, crossing windows, TRIM/EXTEND edges and selection cycling.

### PDFs keep their curves
- PLOT and EXPORTPDF write arcs, circles, ellipses, splines and arc segments as true curves, not hundreds of short lines: smaller files that print smoother and edit properly in other programs.

### PDFIMPORT recognises geometry
- PDFIMPORT turns a PDF page into lines, arcs, circles, polylines with arc segments, splines, solid hatches and text, ready to save as DWG or DXF.
- A PDF written by DraftCraft, or by any CAD program that writes arcs as curves, comes back as the arcs, circles and polylines it was drawn with.
