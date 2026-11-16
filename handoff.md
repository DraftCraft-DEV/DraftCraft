# Handoff: finishing 3D in DraftCraft, everything still missing compared with AutoCAD

Rewritten 2026-11-15 for the agent that takes the work over. This file is the to-do list: everything that
DraftCraft's 3D still lacks compared with AutoCAD's 3D Modeling workspace, checked against the code at the
tip of `feat/solids-3d` (commit `afccf044` and this file). The goal set by the owner is that DraftCraft's 3D works
**like AutoCAD's and is perfectly compatible with AutoCAD**: what AutoCAD saves opens here as the same
objects, and what DraftCraft saves opens in AutoCAD as AutoCAD's own objects.

- **Branch:** `feat/solids-3d`. **PR:** https://github.com/SkryxDev/DraftCraft/pull/20. Commit and push to
  this branch; never to `main`, and never merge the PR (the owner merges).
- **Feature:** everything 3D is behind the cargo feature `solids`. Without it, the build, the tests and the
  wasm build must stay exactly as they are.
- **Read first:** `AGENTS.md` (the rules; they outrank this file), then `docs/solids.md` (how the 3D code
  works; keep it current as you go), then this file.

---

## 1. What already works (don't redo it)

Verified in the code and, where it says so, in AutoCAD 2027.

**Kernel and document**
- `crates/solid` (`draftcraft-solid`, L0) is the only crate that names the kernel (solidcore, pinned to
  `tangency-labs/solidcore` `ce5c6e9`) and the ACIS record reader (`cadmpeg-asm` 0.6.12).
  `cargo xtask layers` enforces this.
- `EntityKind::Solid3d` (`crates/doc/src/entity/solid3d.rs`): the exact body (solidcore `.smb`), a
  placement matrix, a display look, face/edge counts, volume, and `body_entity` (saved back as BODY).
  MOVE, ROTATE, SCALE and MIRROR change only the placement.

**Commands** (`crates/engine/src/cmd/solids/*`, all `CommandSpec`s with JSON form, interactive prompts and tests)
- BOX, WEDGE, CYLINDER, CONE, SPHERE, TORUS (`prims.rs`).
- EXTRUDE (with taper), REVOLVE (`sweep.rs`; despite the file name there is no SWEEP command).
- UNION, SUBTRACT, INTERSECT (the region commands, using the kernel when solids are selected).
- SLICE, FILLETEDGE, CHAMFEREDGE, SOLIDEDIT (**Shell only**), INTERFERE (`edit.rs`).
- MASSPROP for solids and regions (exact for regions with arcs).
- FLATSHOT, SECTION (`views.rs`).
- VPOINT (+ `vpoint.top` … `vpoint.nwiso`), 3DORBIT, PLAN, VSCURRENT (+ `vscurrent.*`) (`look.rs`).
- Multi-object commands are all-or-nothing (`Session::all_or_nothing`).

**3D view** (`crates/engine/src/view3d.rs`, `crates/ui-egui/src/canvas/view3d.rs`, `view3d_overlay.rs`)
- Orthographic only, painted on the CPU with egui's painter, one view per drawing (model space only).
- Nine visual styles: 2D Wireframe, Wireframe, Hidden, Realistic, Conceptual, Shaded, Shaded with Edges,
  Shades of Gray, X-Ray.
- ViewCube (26 spots, drag to orbit, compass, Home, right-click menu), navigation bar (zoom extents/in/out,
  orbit), viewport controls `[-][view][style]`.
- Picking: a click picks the front-most solid; edge/face picks for FILLETEDGE, CHAMFEREDGE and Shell go
  along the line of sight; a point prompt gives the point on the XY plane.

**Interface**
- *3D Modeling* workspace and ribbon (`SOLID_TABS` in `crates/ui-egui/src/ribbon.rs`): Home, Solid,
  Visualize tabs, icons drawn in `icons.rs`, Italian and Chinese labels. Workspace switch in the QAT,
  status bar and WSCURRENT.

**Files (ACIS translator, `crates/solid/src/acis/*`, `crates/io/src/solid_acis.rs`)**
- AutoCAD's 3DSOLIDs open as editable solids from DXF 2000/2010/2018 and DWG 2000/2013/2018: SAT 400, ASM
  text 21500, SAB in the 2013+ data store. Volume and centroid match AutoCAD's MASSPROP to four decimals
  (tests in `crates/io/src/solid_acis/tests.rs`, drawings in `crates/io/tests/data/acad-solids/`).
- AutoCAD's LOFT and SWEEP (also along a helix) open as solids (NURBS faces, ACIS pcurves, procedural
  surfaces read from their caches).
- DraftCraft's solids save as real 3DSOLIDs: SAT 400 for 2000–2010, SAB 700 in ACDSDATA for 2013+.
  **Checked in AutoCAD 2027** for analytic solids (box, cylinder, cone, sphere, torus, frustum, box with a
  hole, rotated box) in DWG and DXF 2000, 2010, 2013 and 2018: AUDIT clean, SOLIDEDIT Check valid,
  MASSPROP equal.
- REGION opens as a DraftCraft region and saves as an AutoCAD REGION in every version. **Checked in
  AutoCAD 2027** both ways (MASSPROP, AUDIT, EXTRUDE).
- BODY holding a solid opens as a solid and saves as a BODY again.
- Solids the ACIS writer can't describe are saved in the *interim form* (a polyface mesh plus the exact body
  in an XRECORD, `crates/io/src/solid_file.rs`), which DraftCraft reopens as the solid.
- DXF no longer writes `$CTABLESTYLE` in the header; AutoCAD's variables dictionary is written and read.

---

## 2. How to work

### Build, run, test
- Run: `cargo run --release -p draftcraft --features solids -- --sample --control PORT` (AGENTS.md explains
  the control channel and `ui.screenshot`; for UI work always look at a screenshot).
- **Gates before every commit:** `cargo xtask ci` **and** `cargo xtask ci --solids`. Also
  `cargo check --target wasm32-unknown-unknown -p draftcraft-solid --features kernel`.
- Three tests fail only on macOS and are accepted there: `read_file_refuses_a_name_that_is_not_unicode`,
  `usage::non_unicode_output_names`, `thai_marks_stack_in_an_installed_font`.
  `mcp_connect::connect_to_closed_port_fails` can fail when the port it picks happens to be in use; it
  passes alone.
- Four AutoCAD test DXFs (`acad-solids/solids-basic-2000/2010/2018.dxf`, `solids-edit-2000.dxf`) can show
  as modified in a checkout because of line endings only. Don't commit them.

### Adding a command (checklist, every one of them)
1. A `CommandSpec` in `crates/engine/src/cmd/solids/<file>.rs`: id = AutoCAD's command name in lower
   case, label, menu path, AutoCAD's aliases, `params` doc, JSON `run`, `interactive` prompt machine that
   follows AutoCAD's prompt sequence and options (observe AutoCAD; never copy its help text).
2. Programmatic calls never open dialogs; JSON runs with defaults.
3. Tests in `crates/engine/src/cmd/solids/tests.rs`: the JSON form, the interactive form, refusal of bad
   input, all-or-nothing on several objects, and a hostile-input test where input is a number or a file.
4. A place on the ribbon (`SOLID_TABS` in `ribbon.rs`), in the panel where AutoCAD on Windows puts it, with
   an icon drawn in code in `icons.rs`.
5. Italian label in `crates/ui-egui/src/i18n/it.tsv`; menu entries also in `zh.tsv`.
6. `docs/solids.md` updated. CHANGELOG: solids are not in release builds yet, so solids-only features have
   had no CHANGELOG lines; a change that users of release builds see (file reading/writing, 2D) gets one.

### Rules that bite in this work
- **Never crash:** no `unwrap`/`expect`/`panic!`/`unreachable!`/indexing on input; checked arithmetic;
  cap loops and allocations (ACIS records, mesh sizes, lump/face counts); every crash fix with a test.
  Kernel calls go through the facade, which turns kernel errors into `SolidError`.
- **Clean-room:** AutoCAD is observed only from the outside. Never read anything inside an Autodesk
  install, never commit screenshots of AutoCAD, never copy its icons, materials, visual style definitions,
  render presets, help text. DWG/DXF files saved by AutoCAD are fine as test data, each with a row in
  `ATTRIBUTION.md` (`cargo xtask assets` checks it). File formats come from public specs (Autodesk's DXF
  Reference, ODA's DWG spec, Spatial's published ACIS record layouts) and from the files themselves.
- **No GPL code**, not even reading it: no LibreDWG, LibreCAD, QCAD, FreeCAD, SolveSpace, OpenCASCADE.
- **Layers:** only `draftcraft-solid` names `solidcore*` and `cadmpeg*`. Nothing below L5 uses egui.

### Checking against AutoCAD
The owner has AutoCAD 2027 and runs AutoCAD checks; ask them for one when a step below says "check in
AutoCAD". Give them files and what to compare:
- `DRAFTCRAFT_ACIS_OUT=dir cargo test -p draftcraft-io --features solids write_files_for_autocad -- --ignored`
  writes the check drawings (extend that test with what you add).
- In AutoCAD: open each file, note any message, AUDIT, `SOLIDEDIT` ▸ Body ▸ Check on each solid, MASSPROP
  (volume, centroid, bounding box) compared with DraftCraft's MASSPROP; for new commands, the same object
  made in AutoCAD saved as DWG/DXF 2000, 2010, 2018 as test data.
- The other direction: AutoCAD-made drawings of the new object type go into
  `crates/io/tests/data/acad-solids/` with their MASSPROP in a README, and a test that opens them.

---

## 3. Everything that is missing

Each item says what AutoCAD does, what DraftCraft has, what to build and what kernel support exists
(solidcore-ops at `ce5c6e9` has, among others: `extrude`, `revolve`, `sweep_placed`, `sweep_controlled`,
`sweep_stations`, `sweep_two_rails`, `screw_sweep`, `loft`, `loft_rails`, `loft_to_point`, `loft_blend`,
`thicken`, `offset_faces`, `offset_sheet`, `remove_faces`, `draft`, `imprint`, `split_body`,
`split_body_by_sheet`, `split_faces`, `split_face_by_curves`, `stitch`, `extend_sheet`, `fillet_sheets`,
`network`, `transform`, `check`/`check_solid`/`validate`; `solidcore-step` reads and writes STEP).
Wrap whatever you use in `draftcraft-solid`'s facade first, with its own tests.

### A. File compatibility (highest priority: the owner's first requirement)

**A1. Check the NURBS writer in AutoCAD.** DraftCraft writes NURBS faces (`spline-surface`, `intcurve-curve`,
`exppc` pcurves; `crates/solid/src/acis/write.rs`) as SAT 400 and SAB 700, but no file with a NURBS face
has been opened in AutoCAD yet. Take AutoCAD's LOFT/SWEEP from `acad-solids/solids-free-2018.dwg`, save it
from DraftCraft as DWG/DXF 2000, 2010, 2013, 2018, and check in AutoCAD (open, AUDIT, Check, MASSPROP).
The least certain part is the SAB tail after a spline surface's and curve's discontinuity lists
(`Tok::Sab` in `acis/model.rs`): if 2013/2018 fail while 2010 works, look there first. Also check solids
made by DraftCraft's own future SWEEP/LOFT the same way.

**A2. DWG 2004–2010 solids stored as SAB.** `modeler()` in `crates/io/src/dwg/entities.rs` reads only SAT
text before 2013 (`bs != 1` → not read). AutoCAD 2004–2010 DWGs can hold binary SAB (modeler format 2)
right after the entity header up to the end marker. Read it into the same path as the 2013+ data store,
with caps on its size, and add AutoCAD-made DWG 2004/2007/2010 test drawings. Check whether DraftCraft
should also write SAB there (what AutoCAD writes when saving as 2004/2007/2010; observe its files).

**A3. A 3DSOLID or BODY with several separate lumps.** Refused today and kept as saved ("several separate
solids"). AutoCAD allows one 3DSOLID made of disjoint pieces (UNION of two apart boxes, SOLIDEDIT
Separate undoes it). Make the body type hold several lumps (check how solidcore's body/record represent
it), read and write them (the writer already lays out lumps for regions), and make UNION of disjoint solids
produce one 3DSOLID as AutoCAD does.

**A4. Mirrored ACIS transforms** (determinant < 0): refused in `crates/solid/src/acis/read.rs` (`a mirrored or
flattened body (not read yet)`). AutoCAD writes them after MIRROR/MIRROR3D. Apply the reflection by
reversing face senses, and on write, decide whether to bake mirrors into the geometry or write the
transform as AutoCAD does (observe its files).

**A5. ACIS geometry still refused on read:** conical helix (`apex_factor ≠ 0`); pcurves with
`wrapper_reversed`; other procedural curves and surfaces (offset curves/surfaces, intersection curves,
`off_int_cur`, `rb_blend` / blend surfaces of AutoCAD's FILLETEDGE on curved faces, `taper` surfaces of
SOLIDEDIT Taper, `skin_spl_sur`/`net_spl_sur` if they appear). Read each from its approximation cache as
`read/legacy.rs` does for `loft_spl_sur`/`sweep_sur`. Make AutoCAD drawings that contain each (fillet on a
cylinder edge, tapered face, offset face, conical helix sweep, PRESSPULL on a curved face) and test.

**A6. REGIONs not on the XY plane, or with ellipse/spline edges.** Kept as saved today. DraftCraft regions are
2D (XY, lines and arcs): either give `Region` an elevation/normal (OCS) and curved edges, or open such a
region as a planar sheet solid/surface. AutoCAD makes regions in any UCS.

**A7. BODY written by DraftCraft:** not checked in AutoCAD (AutoCAD has no command that makes one; use a
BODY read from a file and saved back). Also BODY that is a sheet or wire: kept as saved.

**A8. Solid history (`ACSH_*` objects, `AcDbShHistory`, SOLIDHIST, `350` link on the 3DSOLID).** Dropped
today (written as `350 0`). In AutoCAD a primitive keeps its type and dimensions (Properties shows Box
Length/Width/Height and editing them resizes it), and with SOLIDHIST=1 a boolean keeps its operands.
Minimum: record the primitive's type and parameters on DraftCraft solids (in `Solid3d`) and write/read
AutoCAD's history objects for primitives so Properties and grips work in both programs. Then boolean
history. Until then, at least keep foreign history objects linked to the solid if the body is unchanged.

**A9. Face/edge colours and materials.** ACIS attributes (colour per face from SOLIDEDIT Color Faces,
`persubent-acadSolidHistory-attrib`, material per face) and MATERIAL objects are not carried. Read them
into the body (per-face colour/material), draw them, write them back.

**A10. Surface entities.** PLANESURF, NURBSURF, EXTRUDEDSURFACE, LOFTEDSURFACE, SWEPTSURFACE,
REVOLVEDSURFACE (DXF `SURFACE` subclasses, ACIS sheet bodies plus their creation parameters). Need a
surface entity in `draftcraft-doc` (sheet body + placement + look + the associative parameters AutoCAD
stores), reading/writing in DXF/DWG all versions, drawing in the 3D view. Today they are kept as foreign
data and not drawn in 3D.

**A11. MESH (subdivision mesh).** The `MESH` entity (vertices, faces, edges, crease data, smoothness level)
is not modelled: read, draw (with its smoothing levels), write, in every version.

**A12. Legacy 3D entities in the 3D view.** The 3D view draws every non-solid object flattened onto Z = 0
(`crates/ui-egui/src/canvas/view3d.rs`, `to3(0.0)`): 3DFACE, polyface and polygon meshes, 3D polylines,
lines and points with Z, elevation, thickness (extruded 2D objects), OCS of arcs/circles/polylines in
other planes, blocks inserted with Z are all wrong in 3D. Draw them at their real 3D positions (and fill
3DFACE/meshes as faces with hidden-line removal), exactly as AutoCAD's 3D views show them.

**A13. STEP, IGES, STL.** IMPORT and EXPORT of STEP (AP203/AP214) through `solidcore-step` (wrap it in
the facade); IGES if possible; STLOUT and 3DPRINT from the display mesh (binary and ASCII STL). `export` in
`crates/engine/src/cmd/exchange.rs` handles dxf/dwg/svg/png/pdf only today.

**A14. Round-trip report.** The `roundtrip` comparator (`crates/io/src/roundtrip*`) should report solids,
regions and bodies that stay foreign (kept as saved) as such, so the gaps above show in corpus runs.

### B. Solid modelling commands that don't exist

All need the checklist of section 2.

- **POLYSOLID** (wall-like solid along lines/arcs; Height, Width, Justify, Object option on existing 2D).
- **PYRAMID** (sides, inscribed/circumscribed, top radius, axis endpoint) — add to Primitive panel next to BOX…
- **HELIX** exists as a 2D/3D curve entity; make sure it is usable as a SWEEP path.
- **SWEEP** (profiles along a path: Alignment, Base point, Scale, Twist, Bank; profiles can be open → surface).
  Kernel: `sweep_placed` / `sweep_controlled`, `screw_sweep` for helices.
- **LOFT** (cross sections, Guides, Path, Cross sections only with ruled/smooth/normal/draft settings; open
  sections → surface). Kernel: `loft`, `loft_rails`, `loft_to_point`, `loft_blend`.
- **PRESSPULL** (click inside a bounded area or on a face; extrude/cut by dragging or typing; multiple).
  Needs face picking (D1) and the "bounded area" detection that BOUNDARY/HATCH already do in 2D.
- **THICKEN** (surface → solid). Kernel: `thicken`. Needs surfaces (A10).
- **EXTRUDE** missing options: Direction, Path, Mode (solid/surface), extrude of faces; extrude of open
  profiles into surfaces.
- **REVOLVE** missing options: object as axis, X/Y/Z axis, start angle, Reverse, Mode surface.
- **INTERSECT/UNION/SUBTRACT:** union of disjoint solids into one (A3); boolean with surfaces.
- **SLICE** missing options: Surface (slice by a surface), Object (by a circle/arc/2D polyline plane),
  View, Zaxis, 3points as AutoCAD names them.
- **FILLETEDGE / CHAMFEREDGE** options: Chain, Loop, Radius/Distance by expression, multiple radii; the
  preview while choosing.
- **SOLIDEDIT, everything except Shell** (only Shell exists; others say "not available yet"):
  - Face: Extrude (`offset_faces`/`extrude`), Move, Rotate, Offset (`offset_faces`), Taper (`draft`),
    Delete (`remove_faces`), Copy (to a region/body/surface), coLor.
  - Edge: Copy (to lines/arcs/splines), coLor.
  - Body: Imprint (`imprint`), seParate solids (A3), Shell (exists), cLean, Check (`check_solid`).
  - Undo inside the command.
- **IMPRINT** (as its own command), **OFFSETEDGE** (offset a face's edges as a 2D object on that face),
  **XEDGES** (wireframe geometry from a solid's edges).
- **CONVTOSOLID, CONVTOSURFACE, CONVTOMESH, CONVTONURBS** (polylines/circles with thickness, meshes,
  regions, surfaces).
- **INTERFERE**: the AutoCAD dialog's options (nested selection, check pairs within a set, zoom to pair).
- **MASSPROP**: write the results to a `.mpr` file option.

### C. 3D transforms

Nothing exists; MOVE/ROTATE/SCALE/MIRROR work in 2D only (they already change a solid's placement).
- **3DMOVE, 3DROTATE, 3DSCALE** with the move/rotate/scale gizmos (drawn in code), axis/plane constraint,
  gizmo relocation; gizmo shown when a solid is selected in a 3D visual style (DEFAULTGIZMO).
- **MIRROR3D** (Object, Last, Zaxis, View, XY/YZ/ZX, 3points) — needs mirrored placements to save
  correctly (A4).
- **ROTATE3D** (axis by object, last, view, X/Y/Z, 2 points).
- **3DARRAY** (rectangular with levels, polar about an axis) and **ARRAYRECT/ARRAYPOLAR/ARRAYPATH** with
  Levels and Z spacing for 3D objects.
- **3DALIGN** (3 source points to 3 destination points); **ALIGN** in 3D (it exists, check it with solids
  and Z).
- **COPY/MOVE/STRETCH with 3D points**: typed `x,y,z` and picked points on faces/edges.

### D. Working with parts of solids

- **D1. Sub-object selection:** Ctrl+click picks a face, edge or vertex (and history sub-objects);
  selection filters (no filter / vertex / edge / face / history) on the ribbon; highlight of the hovered
  sub-object; Ctrl+drag window selection of sub-objects.
- **D2. Grips on solids:** today a solid's grip only translates it (`crates/engine/src/grips.rs`). AutoCAD
  shows grips per primitive (box: corner and face-centre arrows that change length/width/height; cylinder:
  radius and height; cone: base radius, top radius, height; sphere: radius; torus; wedge; pyramid; extrusion
  height and taper) and grips on sub-objects (move/rotate/scale a face, edge or vertex). Needs primitive
  history (A8).
- **D3. Properties palette for solids:** solid type, history (Record/Show), dimensions of the primitive
  (editable), volume (read only), material, and for faces/edges when sub-objects are selected.
- **D4. Sub-object editing with the gizmos:** move/rotate/scale a face, edge or vertex (C + D1).
- **D5. LIST / ID / DIST in 3D:** LIST on a solid shows what AutoCAD shows (type, layer, bounding box,
  history); DIST and ID report Z and work with 3D snaps.

### E. Working in 3D (the drawing environment)

- **E1. The UCS command does not exist** (only the named UCS table, `d.ucss`, is kept). AutoCAD's UCS:
  World, Previous, Origin, Face, Object, View, X/Y/Z rotation, Z axis, 3 points, Named (save, restore,
  delete), the UCS icon in 3D, UCSMAN, PLAN of the current UCS, the UCS grips on the icon. Every command
  must work in the current UCS (2D drawing commands draw in the UCS's XY plane; typed coordinates are UCS
  coordinates; ELEV/elevation). This is a big change in the engine: plan it carefully and test it widely.
- **E2. Dynamic UCS (DUCS, F6):** while a drawing command runs, the UCS follows the planar face under the
  cursor.
- **E3. 3D object snaps (3DOSNAP, F4):** zvertex, zmidpoint, zcenter (of a face), zknot, zperpendicular,
  znearest; and the 2D snaps (endpoint, midpoint, center, quadrant…) working on solid edges and on 3D
  objects in the 3D view, with the snap markers drawn there; object snap tracking in 3D.
- **E4. Point input in the 3D view:** today a point prompt gives the point on the XY plane only. Use the
  current UCS plane, faces under the cursor (with DUCS), snaps, and typed `x,y,z`/`@dx,dy,dz`/cylindrical
  and spherical coordinates.
- **E5. Perspective projection** (PERSPECTIVE variable, ViewCube menu Parallel/Perspective), field of view
  / lens length; **CAMERA** objects (create, list, view through, camera glyphs); **3DWALK / 3DFLY**,
  **3DDISTANCE, 3DSWIVEL, 3DZOOM, 3DPAN**, continuous orbit (3DCORBIT), free orbit (3DFORBIT), orbit
  about the selection or a pivot, **3DCLIP** (front/back clipping), **DVIEW** in 3D (it exists, check it),
  **VIEW** saving/restoring 3D views (named views with direction, visual style, background).
- **E6. Several viewports in 3D:** a 3D view, visual style and UCS per tiled model viewport (VPORTS
  exists but the 3D view belongs to model space only), and per layout viewport (VPOINT/visual style inside
  a paper-space viewport, plotting it).
- **E7. Selection in the 3D view:** window and crossing selection (rectangle and lasso) in 3D, cycling
  through overlapping objects, selection of 2D objects at their real Z.
- **E8. Commands in non-plan views:** every 2D command (LINE, CIRCLE, MOVE, COPY, ROTATE, TRIM…) used
  while the 3D view is on works in the current UCS with picks in 3D, as in AutoCAD.

### F. Sections and drawings from the model

- **SECTIONPLANE** (section object: plane, slice, boundary, volume states; jogs), **LIVESECTION**
  (live cut display), **GENERATESECTION / SECTIONPLANETOBLOCK** (2D/3D section blocks with hatching and
  settings), **SECTIONSPINNER** etc. Save the SECTIONOBJECT entity and its settings objects in DXF/DWG.
- **SOLVIEW, SOLDRAW, SOLPROF** (layout viewports with hidden/visible profile layers).
- **VIEWBASE, VIEWPROJ, VIEWSECTION, VIEWDETAIL, VIEWEDIT, VIEWUPDATE** (model documentation: drawing
  views in layouts that update when the model changes; their objects in DXF/DWG).
- **FLATSHOT** dialog options (foreground/obscured lines colour and linetype, insert as block vs. export
  to file, tangential edges). Today: lines/arcs/circles with optional HIDDEN linetype.
- **HIDE** and shade plot: plotting a 3D viewport "As displayed", "Wireframe", "Hidden", "Conceptual",
  "Realistic", "Rendered" from a layout (Output ▸ Plot and PUBLISH, PDF and raster outputs).

### G. Visualization

- **G1. Visual style details:** the styles exist; missing are VISUALSTYLES (the manager palette with face,
  edge, lighting, environment settings), user-defined styles saved in the drawing (`VISUALSTYLE` objects in
  DXF/DWG, read and written), Sketchy, edge jitter/overhang, silhouette width, isolines on curved faces,
  facet smoothness (FACETRES/VIEWRES equivalents), backgrounds (solid, gradient, image, sun & sky).
- **G2. Materials:** MATERIALS browser and editor, attaching materials to objects, faces and layers
  (MATERIALATTACH, MATERIALASSIGN), material mapping (MATERIALMAP), `MATERIAL` objects in DXF/DWG. Our own
  material library only (never Autodesk's).
- **G3. Lights and sun:** POINTLIGHT, SPOTLIGHT, DISTANTLIGHT, WEBLIGHT, default lighting, SUNPROPERTIES,
  GEOGRAPHICLOCATION, light glyphs, `LIGHT` entities and `SUN` object in files.
- **G4. RENDER:** render window, render presets (our own), render to file, shadows in the viewport.
- **G5. A GPU path** for the 3D view (wgpu exists in the egui stack): the CPU painter will not handle large
  models. The advisory's acceptance target (10,000 faces smooth, also in the browser) has never been
  measured; measure and meet it.

### H. Surface and mesh modelling (the Surface and Mesh ribbon tabs are absent)

- **Surfaces:** PLANESURF, SURFNETWORK (`network`), SURFBLEND, SURFPATCH, SURFOFFSET (`offset_sheet`),
  SURFFILLET (`fillet_sheets`), SURFEXTEND (`extend_sheet`), SURFTRIM, SURFUNTRIM, SURFSCULPT (`stitch` +
  solid), SURFEXTRACTCURVE, NURBS surface control-vertex editing (CVSHOW, CVHIDE, CVADD, CVREMOVE,
  CVREBUILD, 3DEDITBAR), SURFACEASSOCIATIVITY, SURFACEMODELINGMODE. Needs A10.
- **Mesh:** MESH primitives (box, cone, cylinder, pyramid, sphere, wedge, torus), MESHSMOOTH,
  MESHSMOOTHMORE/LESS, MESHREFINE, MESHCREASE/UNCREASE, MESHSPLIT, MESHEXTRUDE, MESHMERGE, MESHCAP,
  MESHCOLLAPSE, MESHSPIN. Needs A11.
- **Legacy surfaces:** 3DFACE, 3DMESH, PFACE, RULESURF, TABSURF, REVSURF, EDGESURF, EDGE (visibility),
  SURFTAB1/SURFTAB2. The entities 3DFACE and polyface mesh exist in the document; the commands don't.
- **Ribbon:** add the Surface and Mesh tabs, and the panels left out of the 3D Modeling workspace
  (Coordinates, Model Viewports, Selection with sub-object filters, Lights, Materials, Render, Camera) as
  their commands land. Each with drawn icons and translations.

### I. Platform and quality

- **I1. Solids in the browser:** `draftcraft-web` has no `solids` feature because on wasm a kernel panic
  aborts the app (`catch_unwind` doesn't work there). Find or get a panic-free path in solidcore (or
  validate inputs so the kernel's panicking paths are unreachable, fuzz-tested), then turn `solids` on
  for the web build and pass `cargo xtask wasm`.
- **I2. Releases:** decide with the owner when `solids` goes into release builds (`release.yml`), then add
  CHANGELOG lines for the 3D features.
- **I3. Robustness:** fuzz the ACIS reader with hostile SAT/SAB (there are `hostile_*` tests; extend them
  to every new record kind), and every new command with degenerate input (zero size, coincident points,
  self-intersecting profiles, huge counts).
- **I4. Performance:** opening drawings with thousands of solids, booleans on large bodies, the hidden-line
  FLATSHOT on many solids; profile and set limits that report instead of hanging.
- **I5. Undo:** every 3D command and view change behaves with UNDO as AutoCAD's does (view changes are not
  undone today, as in AutoCAD; check 3D edits group correctly).
- **I6. MCP / control channel:** expose the 3D view (direction, style) and solid info through MCP so agents
  can drive and check 3D work (`crates/mcp`), and document it.

---

## 4. Suggested order

1. **A1** (NURBS check in AutoCAD), **A2** (DWG 2004–2010 SAB), **A4** (mirrors), **A3** (lumps): file
   compatibility first, as the owner asked.
2. **A12** (legacy 3D objects drawn at their real Z) and **E1–E4** (UCS, DUCS, 3D snaps, 3D point input):
   without these, nothing else in 3D feels like AutoCAD.
3. **C** (3DMOVE, 3DROTATE, 3DSCALE, MIRROR3D, 3DALIGN, 3DARRAY).
4. **D1–D3** (sub-objects, grips, Properties) with **A8** (primitive history).
5. **B**: PRESSPULL, SWEEP, LOFT, POLYSOLID, PYRAMID, the rest of SOLIDEDIT, then the remaining options.
6. **A13** STEP/STL, **A5**/**A6**/**A9** remaining ACIS cases.
7. **E5–E8** (perspective, cameras, viewports), **F** (section objects, drawing views), **G**
   (visual style manager, materials, lights, render, GPU), **A10/A11/H** (surfaces, meshes), **I**.

After each step: gates green, `docs/solids.md` updated, this file updated (move done items to section 1),
commit and push to `feat/solids-3d`.
