# DraftCraft changelog

What changed in each release, for the people who use DraftCraft. The app shows the new sections in
its What's New window at the first start after an update (and from Help › What's New), so this file
is built into every release: nothing is downloaded to show it.

How to write it (see AGENTS.md):
- Every user-visible change adds a line under `## Unreleased`, in plain words for users.
- One `- ` line per change; `### ` headings group them. Lines are not wrapped.
- Before a release, `cargo xtask version set X.Y.Z` moves the `## Unreleased` lines into `## X.Y.Z`.

## Unreleased

### Improved
- The Workspace button on the status bar opens a menu of the workspaces, as on Windows, with the current one checked.
- The workspace menus show 3D Modeling greyed out, with a note that 3D solids are not available in this build yet.

### Fixed
- AutoCAD no longer reports an unknown header variable $CTABLESTYLE when it opens a DXF file saved by DraftCraft: the current table style is saved where AutoCAD keeps it.
- MASSPROP gives a region's exact centroid and moments of inertia when it has arcs, the same as AutoCAD (they were measured on a fine polygon before).

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
