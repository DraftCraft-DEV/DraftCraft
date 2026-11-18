# DraftCraft roadmap

DraftCraft **2.2.0** is out for Windows (x64 and ARM64, installer or zip), macOS (Apple silicon and
Intel) and Linux (x86_64 and ARM64, AppImage or .deb), and in the browser. It is a young project: good
for 2D drafting, dimensioning and plotting, with DWG and DXF read and written by its own code, and still
missing parts of what a full CAD suite does. This page says what works today, what is known to be
missing and what comes next.

Something not listed here is broken, or missing something you need?
[Open an issue](https://github.com/DraftCraft-DEV/DraftCraft/issues/new) — or a pull request.

## What works today

| Area | What you get |
|---|---|
| Interface | Windows-style ribbon (10 tabs, split buttons, layer and property lists), application menu, Quick Access Toolbar, file tabs, floating command line, ViewCube and navigation bar, status bar toggles and drawing units, Properties and Layers palettes, What's New after every update, 18 languages |
| Command line | AutoCAD command names and aliases, prompts with keywords, AutoComplete that learns the commands you use most, `@dx,dy`, `@d<a`, `#x,y`, direct distance entry, transparent commands, scripts |
| Draw | LINE, PLINE (arcs, widths), CIRCLE, ARC, RECTANG, POLYGON, ELLIPSE, SPLINE, CONIC (exact conic arcs, saved as rational splines, with Rho editable in Properties), POINT, XLINE, RAY, DONUT, REVCLOUD, WIPEOUT, TEXT, MTEXT |
| Modify | ERASE, MOVE, COPY, ROTATE, SCALE, MIRROR, STRETCH, OFFSET, TRIM, EXTEND, FILLET, CHAMFER, BREAK, JOIN, EXPLODE, arrays, draw order, OVERKILL, grips |
| Precision and inquiry | Object snaps, polar tracking, ortho, grid snap, dynamic input; DIST, ID, LIST, AREA, MEASUREGEOM and AREATOTAL (total area, perimeter and length of a selection) |
| Annotation | DIM commands with dimension styles and associative dimensions, multileaders, tables, TrueType and SHX fonts, right-to-left text |
| Hatches and blocks | Pattern, solid and gradient fills; blocks, attributes, ATTOUT and ATTIN, the Block Editor and dynamic blocks, in-place editing, external references |
| Parametric | Geometric and dimensional constraints, AUTOCONSTRAIN, parameters with expressions |
| Layouts and output | Paper-space layouts, viewports, page setups, plot styles, PLOT and EXPORTPDF (arcs, circles and splines kept as true curves), SVG and PNG export |
| Files | DWG (AutoCAD 2000 to 2018) and DXF reading and writing with DraftCraft's own code, at a chosen file version; AUDIT and RECOVER; autosave and drawing recovery; PDF underlays and PDFIMPORT (PDF vectors to lines, arcs, circles, polylines, splines, hatches and text) |
| Settings | Export Settings and Import Settings (CONFIGEXPORT, CONFIGIMPORT): theme, Options, units, grid and snap, lineweights, plot style and aliases in one TOML file you can take to another computer |
| Automation | MCP server, JSON control channel, `draftcraft-cli`, AutoLISP, scripts and alias files |

## Known gaps

- **3D** — there is no 3D modelling in 2.x (no EXTRUDE, REVOLVE, solids or surfaces). It is being built
  on its own branch and will come with **DraftCraft 3.0**.
- **File round-trips** — some objects that DraftCraft doesn't model can be lost when a DWG or DXF is
  saved: from an AutoCAD DXF saved as DWG, subdivision meshes, surfaces and a solid's history. Keep a copy
  of drawings that come from other programs before saving over them.
- **Ribbon** — no slide-out panels yet; collapsed panels open a plain list of their commands.
- **Builds** — the macOS and Windows builds are not signed or notarized yet, so the system warns the first
  time you open them (the README says how to get past it).

## Next

1. Slide-out ribbon panels, so the ribbon behaves like Windows CAD in every state.
2. Keep every object through a DWG/DXF save.
3. Signed and notarized builds.
4. **DraftCraft 3.0: 3D modelling** — solids, surfaces, 3D commands and views.

Want something higher on this list? Say so in an [issue](https://github.com/DraftCraft-DEV/DraftCraft/issues)
or on [Discord](https://discord.gg/YgRK8Eww9x).
