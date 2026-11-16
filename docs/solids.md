# 3D solids with solidcore

This page has two parts:
- **Status**: what branch `feat/solids-3d` built, how to try it, measurements, decisions and open questions. It is kept current.
- **The advisory** that planned the work (written 2026-11-09), unchanged except for its status line. It planned the work on SolidCraft's kernel; the kernel is now solidcore (see Decisions), and the plan's stages, file form and ACIS work stand as written.

## Status

Updated 2026-11-15. Stages S0, S2, S3 and S6 are done, and S4 is done in part. S5 and S7 (the ACIS writer and reader) are done for solids bounded by planes, cylinders, cones, spheres and tori; free-form (NURBS) faces are read and written, not yet checked in AutoCAD (see The ACIS translator). S1 was not needed. Everything is behind the cargo feature `solids`. Without the feature, the build, the tests and the wasm check are exactly what they were.

The kernel is [solidcore](https://github.com/tangency-labs/solidcore) (Apache-2.0): a pure-Rust B-rep modeller, a port of NVIDIA's SMLib, with no `unsafe`, building for wasm32. It replaced SolidCraft's truck-based kernel on 2026-11-14, at the owner's request.

### Trying it

```bash
cargo run --release -p draftcraft --features solids -- --sample
```

The feature chain:
- `draftcraft-engine/solids` turns on `draftcraft-solid/kernel`.
- `draftcraft-io/solids` opens AutoCAD's 3DSOLIDs as solids, saves solids as 3DSOLIDs, and reopens the interim form. `draftcraft-engine/solids` turns it on.
- `draftcraft-ui-egui/solids` adds the 3D view and the ribbon tab.
- `draftcraft-mcp/solids`.
- The apps `draftcraft`, `draftcraft-cli` and `draftcraft-web` each have a `solids` feature that turns on the ones they use.

`cargo xtask ci --solids` runs the clippy and test steps with the feature on (see Measurements for the cost).

### Where it sits

| Crate | Layer | What it has |
|---|---|---|
| `draftcraft-solid` (new) | L0 | The only crate that names the kernel (`cargo xtask layers` enforces it). It holds the body type, primitives, extrude and revolve, booleans, slice, fillet, chamfer, shell, mass properties, the display look, exact hidden-line projection and sections. It converts between `draftcraft-geom` and the kernel's vectors here and nowhere else. It uses `solidcore-ops` (the modelling API), `solidcore-record` (profiles, planes, the portable B-rep record) and `solidcore-tess` (display meshes). Without its `kernel` feature it is empty. |
| `draftcraft-doc` | L1 | `EntityKind::Solid3d`, which holds: the exact body in solidcore's binary `.smb` format behind DraftCraft's 8-byte header `DCSMB001`; a placement matrix; the look (edges and a triangle mesh in the body's own coordinates); and the face count, edge count and volume. The document never calls the kernel. MOVE, ROTATE, SCALE and MIRROR change only the placement. 2D drawing, snapping and selection read the edges projected to XY. |
| `draftcraft-io` | L3 | The DXF and DWG form (below). |
| `draftcraft-engine` | L4 | The commands (`cmd/solids/*`) and the 3D view state (`view3d.rs`). |
| `draftcraft-ui-egui` | L5 | The *3D Modeling* ribbon tab, its icons (drawn in `icons.rs`), and the 3D drawing area (`canvas/view3d.rs`). |

Bodies are kept near their own origin. A solid is made around its base point and placed by its matrix. Booleans and other edits work in a frame at the first solid's origin. This keeps kernel tolerances relative to the part even at survey coordinates.

### Commands

| Command | Notes |
|---|---|
| BOX, WEDGE, CYLINDER, CONE, SPHERE, TORUS | JSON and interactive prompts. |
| EXTRUDE, REVOLVE | Profiles can be circles, closed polylines (with bulges) or regions. The profiles are erased unless `keepProfiles` is set. EXTRUDE takes a taper angle. |
| UNION, SUBTRACT, INTERSECT | These are the existing region commands. When solids are selected, they use the kernel. |
| SLICE | Cuts by a plane given as origin and normal, two points, or XY/YZ/ZX. Keeps both sides or the side with a given point. A point typed as `x,y,z` (or `@dx,dy,dz`) keeps its Z: the XY plane goes at that height, and the side point can be above or below it; a picked point is in the XY plane. A side point on the plane is refused. |
| FILLETEDGE, CHAMFEREDGE | Edges are picked by a point near them, along the current view: in plan the top edge of a box, seen from below the bottom one. |
| SOLIDEDIT | Only the Shell operation; it can remove faces, picked as the face nearest the viewer along the current view. Other operations report "not available yet". |
| INTERFERE | Reports whether solids interfere. It can create the interference solid. Solids whose boxes are apart are ruled out without the kernel; each body is read once; the interference is drawn only when it is created. |
| MASSPROP | For solids: mass at density 1, volume, surface area, bounding box, centroid, moments and products of inertia about the origin, radii of gyration, and moments about the centroid. |
| FLATSHOT | Hidden-line view as lines, arcs and circles (exact where the projection is a line or an arc). Hidden lines can be shown in the HIDDEN linetype. |
| SECTION | The cut through solids as regions. |
| VPOINT, 3DORBIT, PLAN, VSCURRENT | The 3D view (S4, below). Without the feature, 3DORBIT still says "not available yet". |

A command that works through several objects (EXTRUDE of several profiles, SLICE of several solids, FLATSHOT, UNION/SUBTRACT/INTERSECT of solids and regions) changes all of them or, when it fails, none (`Session::all_or_nothing`).

All of them are `CommandSpec`s with tests (`crates/engine/src/cmd/solids/tests.rs`). They are reachable from the command line, scripts, the CLI, the control channel and MCP. They sit on the ribbon's *3D Modeling* tab, which has four panels: Modeling, Solid Editing, Section and View. Their labels are translated into Italian and Chinese.

### The ACIS translator (S5 and S7)

AutoCAD keeps a 3D solid as a 3DSOLID entity whose shape is ACIS data: SAT text in the entity's groups 1 and 3 (scrambled: each printable character `c` stored as `159 - c`) up to AutoCAD 2010, and SAB in the file's data store (ACDSDATA, one `ASM_Data` record per entity, keyed by its handle) from AutoCAD 2013. DraftCraft reads and writes both.

- **Where:** `crates/solid/src/acis/` (`read`, `write_sat`, `write_sab`, `scramble`) does the ACIS work; `crates/io/src/solid_acis.rs` does the files (`adopt` on open, `for_saving` before the DXF and DWG writers).
- **How:** both directions go through STEP, which describes a B-rep the way ACIS does and which solidcore reads and writes (`solidcore-step`).
  - Reading, `cadmpeg-asm` (Apache-2.0, 0.6.12) frames and decodes the ACIS records, in any dialect AutoCAD writes (ACIS 400/700, ShapeManager 21500 and later; text or binary), into a neutral B-rep. `acis/read.rs` writes it as STEP topology, applies the body's transform, bounds whole spheres and tori with seams, and solidcore's STEP reader makes the body (pcurves, tolerances).
  - Writing, solidcore writes the body as STEP and `acis/write.rs` turns the STEP entities into ACIS records: body, lump, shell, face, loop, coedge, edge, vertex, point, `plane-`, `cone-` (cylinders too), `sphere-` and `torus-surface`, `straight-` and `ellipse-curve`; a B-spline face as `spline-surface { exactsur … }` with an `exppc` pcurve on each of its coedges, a B-spline edge as `intcurve-curve { exactcur … }`, laid out as AutoCAD 2000 writes its own LOFT and SWEEP. Faces that go all the way round lose their seams, as ACIS has them, and a cone's apex becomes a loop of one point edge (an edge with no curve that starts and ends at the tip): without it AutoCAD calls the cone an invalid solid with no volume. `acis/model.rs` encodes the records as SAT in save format 400 (the format AutoCAD writes in DWG and DXF 2000 files, which every AutoCAD reads: no history fields after an entity's attribute, no parameters or convexity on an edge, the header counting the records) or as SAB in save format 700. SAT was 700 at first: AutoCAD 2027 took it from DXF but discarded every such solid from a DWG 2000 or 2010 file (`Error Code Number is 75003`, `eCannotRestoreFromAcisFile`). In a DWG 2000 to 2010 file the SAT lines end with a line feed alone, as in AutoCAD's own files, and keep the `End-of-ACIS-data` line (AutoCAD's own SAT has none, its header counting the records; a SAT whose header gives no count needs it).
  - Units: AutoCAD takes ACIS coordinates as drawing units (the header says 1 mm per unit); so does DraftCraft, both ways.
- **Opening:** every 3DSOLID whose data reads becomes a solid (placement identity); its entity groups and its data store record move to `Foreign::acis` (an `AcisSource` by handle, with the solid as read). One that doesn't read is kept exactly as saved and written back, and the drawing's read notes say how many and why.
- **Saving:** a solid becomes a 3DSOLID for the file's version: SAT 400 in groups 1/3 with `70 1` for 2000 to 2010 (plus `AcDb3dSolid` / `350 0` from 2007), and `290 1`, a GUID and a data store record of SAB for 2013 and later. The existing DXF and DWG writers write it like any 3DSOLID read from a file; in DXF it carries `100 AcDbEntity` before its layer, without which AutoCAD refuses the whole file. A solid the writer can't describe (free-form faces) keeps the interim form below, and the save says how many.
- **Saving AutoCAD's own back:** a solid, body or region still as it was read (same body and placement, same region, same handle) is written with AutoCAD's description from `Foreign::acis`, not a new one, when the file holds it as it is: the same form (SAT up to 2010, SAB from 2013) and a version no older than the file it came from (an older AutoCAD discards a newer ACIS). Its ACIS version, history (the `350` handle to the kept history object), face colours and other attributes survive; an edited, copied or down-saved one gets DraftCraft's description. The entity's properties (colour, linetype, lineweight, linetype scale, visibility, transparency) are the drawing's either way (`dxf_write::property_groups`).
- **Checked against AutoCAD:** `crates/io/tests/data/acad-solids/` holds drawings made in AutoCAD 2027 (box, cylinder, cone, sphere, torus, wedge; SUBTRACT, FILLETEDGE, CHAMFEREDGE; LOFT, SWEEP) as DXF 2000/2010/2018 and DWG 2000/2013/2018. The tests (`crates/io/src/solid_acis/tests.rs`) open them and compare each solid's volume and centroid with AutoCAD's MASSPROP (four decimals): all match. Solids saved in every version reopen with the same mass properties, and AutoCAD's solids saved again by DraftCraft stay solids. The other way round was checked on 2026-11-15. AutoCAD 2027 opened DraftCraft's DWG and DXF files in every version (2000, 2010, 2013 and 2018), each holding a box, cylinder, cone, sphere, torus, frustum, box with a hole and turned box, and AutoCAD's own drawing saved again by DraftCraft as DWG 2018 and DXF 2000. The only message was AutoCAD's note on DWG files from other programs. AUDIT found nothing, SOLIDEDIT called every solid a valid ShapeManager solid, and MASSPROP's volume, centroid and bounding box matched to four decimals.
- **NURBS faces:** solidcore has no exact pcurve of a spline edge on a spline surface, so the reader hands it ACIS's own (`impl Trims for Kernel`), resampled onto the kernel's edge parameter where the two differ. What cadmpeg-asm leaves out is filled in from the records (`acis/read/legacy.rs`): SAT 400's `exppc` pcurves and `parcur` rails, and ASM text's procedural surfaces (`loft_spl_sur`, `sweep_sur`) and helices. SAB keeps save format 700 for NURBS faces too, the subtypes named in full and with the later formats' fields (`Tok::Sab` in `acis/model.rs`). Not checked in AutoCAD yet.
- **Regions and bodies:** a REGION (a sheet body: flat, double-sided faces, each edge used once) opens as a DraftCraft region when its faces lie on the XY plane and its edges are lines and arcs (`acis::read_region`), and a region is saved as a REGION (`acis::write_region_sat`, `write_region_sab`): one lump per outer loop and its holes, a plane face on z = 0, outer loops counter-clockwise about +Z and holes clockwise. A BODY holding a solid opens as a solid that is saved as a BODY again (`Solid3d::body_entity`). Other REGIONs and BODYs are kept as saved, with a read note. Checked on 2026-11-15: AutoCAD 2027's own regions (`acad-solids/regions-*`, DXF 2000/2010/2018 and DWG 2000/2013/2018: rectangle, circle, plate with a hole, slot with an arc, two squares in one region) open with area, perimeter and centroid equal to AutoCAD's MASSPROP (`autocad_regions_open_as_regions_in_every_format`); DraftCraft's regions saved as DXF and DWG 2000, 2010, 2013 and 2018 open in AutoCAD 2027 as REGIONs with the same MASSPROP, AUDIT finds nothing and EXTRUDE takes them. Region MASSPROP is exact on arcs (each arc's circular segment), equal to AutoCAD's to four decimals, moments included.
- **Not yet:** Several separate lumps in one 3DSOLID or BODY are kept as saved. Regions off the XY plane or with elliptical or spline edges are kept as saved. Mirrored ACIS transforms are refused (kept as saved).

### Saving (S3, the interim form)

Solids the ACIS writer can't describe yet, and every solid in a build without the `solids` feature in `draftcraft-io`, are saved in this form. In DXF and DWG, such a solid is written as two things:

1. **A polyface mesh** (POLYLINE) that any program shows. The edges between triangles inside one face are invisible. A mesh past 32767 vertices or faces is split into several meshes.
2. **An extension dictionary on the first mesh**, with the entry `DRAFTCRAFT_SOLID` pointing to an XRECORD. The XRECORD groups are:

   | Group | Holds |
   |---|---|
   | 1 | `DraftCraft solid 2` |
   | 90 | Mesh count |
   | 91, 92 | Face count, edge count |
   | 140 | Volume |
   | 16 × 40 | Placement, row-major |
   | 93 | Body length |
   | 310 | The body, in chunks of 127 bytes |
   | 300 | One per mesh, in order: its fingerprint (vertex count, face count, a hash of the face indices, bounding box) |

On open, `draftcraft-io` with `solids` finds the marked meshes and turns them back into the solid. It drops the extra meshes, the record and the dictionary, and draws the solid from its body.

- **Only unchanged meshes come back:** the first mesh and each other mesh must match their fingerprints. A mesh edited or moved in another program, or a copy (AutoCAD's COPY copies the record with the mesh), stays the mesh it is now. The other meshes are found by fingerprint, never by position: a mesh missing means no solid, and no other mesh is taken or deleted (tested).

- **Without the feature:** the meshes stay meshes and the record is kept as foreign data, so a save writes it back.
- **A damaged record:** the mesh is kept and nothing crashes (tested).
- **The save tells the user:** it says on the command line (and in `solidsAsMeshes`) how many solids other programs see as meshes: those not saved as 3DSOLIDs.
- **Open time:** only the solids that come back from their meshes are drawn from their bodies on open (a look past 200,000 triangles keeps the mesh's). Solids read from ACIS get their look as they are read; nothing is drawn twice.

S1 (native DWG items) was not needed for this. The DWG writer already writes POLYLINE, DICTIONARY and XRECORD with binary 310 groups. The direct DWG reader keeps the extension dictionaries and objects, and DWG 2013/2018 solids from other programs already keep their data-store geometry (`e284f21e`).

### Drawings from solids (S6)

- **FLATSHOT.** solidcore has no hidden-line module, so `draftcraft-solid` makes the view (`crates/solid/src/views.rs`):
  - Each B-rep edge is projected exactly where it can be: a straight edge always, and a line or arc that lies in a plane parallel to the paper, read exactly by the kernel (`prism::planar_edges`). Other curves (a circle seen at an angle) are drawn as short chords.
  - The silhouettes of curved faces are found on the display mesh: the mesh edges, inside one face, between a triangle facing the viewer and one facing away. They are marked inexact.
  - Each line is sampled finely and every sample tested against the triangles of all the solids' meshes in front of it. Runs of visible and hidden samples become pieces, and an exact edge keeps its exact form cut to each run.
  - A piece lying on top of a nearer line (a back edge behind a front edge) is left out, as a drafter would draw it. Cylinder seams are not drawn.
  - Faces the mesher could not mesh are counted and reported.
- **SECTION** slices the solid and outlines the cut face from its exact edges.
  - The output is regions.
  - A cut parallel to XY stays where it is; other cuts are laid flat at a base point.

### 3D view (S4, in part)

The engine keeps a 3D view per drawing: a direction, a framing and a visual style.
- **VPOINT** takes named views (top … nwiso) or a viewpoint x,y,z.
- **3DORBIT** turns the view by azimuth and elevation. Interactively, you drag with the left button; Shift plus the middle button orbits at any time.
- **PLAN** goes back to the plan view.
- **VSCURRENT** picks the style: 2D Wireframe (the plan view), Wireframe, Hidden, Realistic, Conceptual, Shaded, Shaded with edges, Shades of Gray, X-Ray.

These change the view, never the drawing, and are not undone.

The drawing area paints the 3D view on the CPU with egui's painter:
- **Projection:** orthographic.
- **What it draws:** 2D objects lie on the XY plane; solids are drawn from their looks, those inside blocks too (placed by each reference, nested to `MAX_BLOCK_DEPTH`, and selected through the outermost reference). A mirrored solid's triangles are wound back so their outsides stay outside (`Solid3d::world_triangle`); the meshes a file keeps are wound the same way.
- **Faces:** filled back to front.
- **Edges and silhouettes:** cut into short pieces, each tested against the faces in front of it on a screen grid. So the Hidden and shaded styles show only the edges a viewer sees, including the silhouettes of curved faces. Faces off the view are left out of the grid.
- **Repaints:** what was painted is kept until the drawing, the view, the canvas or the selection changes, so a repaint with nothing changed sorts and cuts nothing again.
- **Navigation:** the wheel zooms about the cursor and the middle button pans.
- **Clicks:**
  - At a selection prompt, a click picks the front-most solid, or a 2D object within 5 points.
  - With no command running, a click toggles the selection.
  - At a point prompt, a click gives the point on the XY plane under the cursor. A view looking along that plane asks the user to type the point instead. Commands that pick an edge or a face (FILLETEDGE, CHAMFEREDGE, SOLIDEDIT Shell) take the line of sight through that point along the view, so they get what was clicked.

### The 3D Modeling workspace

The interface follows the structure of AutoCAD's 3D Modeling workspace on Windows. Only the structure is taken from it: the names of tabs, panels and commands and where they sit. Every icon is drawn in code (`crates/ui-egui/src/icons.rs`); no artwork, icons or help text come from AutoCAD.

- **Workspaces.** With `solids` on, there are two built-in workspaces, *2D Drafting* and *3D Modeling*. You pick one from the workspace switch in the Quick Access Toolbar, from the Workspace Switching button in the status bar, or with WSCURRENT. Neither can be renamed, deleted or overwritten (`crates/ui-egui/src/workspaces.rs`).
- **Ribbon in 3D Modeling** (`SOLID_TABS` in `crates/ui-egui/src/ribbon.rs`). Then come the shared tabs (Insert, Annotate, View, Manage, Output and so on).
  - **Home:** Modeling, Solid Editing, Draw, Modify, Section, View (visual style, preset views, orbit, plan) and Layers.
  - **Solid:** Primitive, Solid, Boolean, Solid Editing and Section.
  - **Visualize:** Views (the ten preset views and the plan view) and Visual Styles.
  
  In 2D Drafting the ribbon is the 2D one, as before.
- **Preset commands.** The ribbon and the viewport controls run one command per preset (`crates/engine/src/cmd/solids/look.rs`), so scripts, the command line and MCP reach them too.
  - `vpoint.top`, `vpoint.bottom`, `vpoint.left`, `vpoint.right`, `vpoint.front`, `vpoint.back`, `vpoint.swiso`, `vpoint.seiso`, `vpoint.neiso`, `vpoint.nwiso`.
  - `vscurrent.2dwireframe` … `vscurrent.xray`.
- **Viewport controls.** At the top left of the 3D view, `[-][SE Isometric][Realistic]` (`crates/ui-egui/src/canvas/view3d_overlay.rs`). Each bracket is a menu:
  - `[-]` shows or hides the ViewCube and goes to the plan view.
  - The view menu lists the ten presets, and shows *Custom View* when the view is between them.
  - The style menu lists the nine visual styles.
- **ViewCube.** At the top right, a cube drawn in code that turns with the view.
  - Its faces carry the labels TOP, FRONT, RIGHT and so on.
  - Each face is cut into nine spots, which gives 26 views (6 faces, 12 edges, 8 corners). A click on a spot runs VPOINT from there.
  - Dragging the cube orbits the view.
  - The compass ring (N, S, E, W) lies on the XY plane under the cube.
  - The Home button goes to SE Isometric.
  - The right-click menu has Home, Plan View and Hide ViewCube.
  - The cube follows the ViewCube setting (`ui.toggle.viewcube`), the same setting as in the plan view.
- **Navigation bar.** On the right, under the ViewCube: Zoom Extents, Zoom In, Zoom Out and Orbit. Panning is the middle button, as in the plan view.
- **Left out, for lack of commands.** These panels from AutoCAD are not there: Surface, Mesh, Coordinates, Model Viewports, Selection, Lights, Materials and Render. Pyramid, Sweep, Loft and Presspull are missing as well. The 3D view has no UCS of its own and belongs to model space, not to a viewport.

Not done yet:
- A view per tiled viewport and per layout viewport (the 3D view is model space's only).
- Object snaps and grips in the 3D view.
- Sub-object selection (Ctrl+click on a face, edge or vertex). FILLETEDGE, CHAMFEREDGE and SOLIDEDIT Shell already pick along the line of sight of the current view.
- UCS on a face.
- A GPU path.
- The "smooth at 10,000 faces in the browser" acceptance, which has not been measured.

### S0 spike: measurements and the three checks

Measured on an Apple M4 (10 cores).

| Measure | Without `solids` | With `solids` |
|---|---|---|
| Cold build of the kernel chain (`draftcraft-solid` with `kernel`, which includes solidcore; dev profile with dependencies at opt-level 2) | — | 2 min 21 s (141 s wall, 889 s CPU) |
| Release binary `draftcraft` (macOS arm64) | 61.4 MB | 105.8 MB (+44.4 MB) |
| `draftcraft-web` wasm (release, `wasm32-unknown-unknown`, before wasm-opt) | 53.8 MB | 105.3 MB (+51.5 MB) |
| `cargo xtask ci` | as before | `cargo xtask ci --solids` adds clippy and the tests with the feature on, plus a wasm32 check of the kernel |

The three checks from the advisory's section 5:

1. **Units and scale.** `crates/solid/src/tests.rs::units_and_scale` runs the same part at three scales and far from the origin, and the answers agree relative to the part's size:
   - at 1/100 scale, within 1e-3;
   - at 1 and at 1000, within 1e-4;
   - at survey coordinates, within 1e-3.

   Each body is held near its own origin with a placement, as the advisory expected.
2. **Panics in the browser.** solidcore runs every call inside its session, which turns a panic into an error with `catch_unwind`. On `wasm32-unknown-unknown` a panic aborts, so that guard (and DraftCraft's own `guard.rs`) does nothing there. A kernel panic in the browser ends the app, where on desktop it would become an error. **So the web build keeps `solids` off** (open question 1). The kernel builds for wasm: see the wasm row above.
3. **Determinism.** `crates/solid/src/tests.rs::determinism` repeats the same operations and gets byte-identical bodies. Undo and the tests compare drawings, so this matters. `Body::encode` writes the form a decoded body writes, so a solid opened and saved again keeps its bytes (`native_round_trip_is_exact`).

### Decisions

- **The kernel: solidcore**, the crates `solidcore-ops`, `solidcore-record` and `solidcore-tess` as git dependencies pinned to `tangency-labs/solidcore` `ce5c6e9` (Apache-2.0). This is the owner's decision of 2026-11-14 and replaces SolidCraft's truck-based kernel (`withshalom/solidcraft` `d11d268`), which the branch used first. The facade's API did not change, so the document model, the file form, the engine and the UI are the same with either kernel.
  - What solidcore brings: primitives, extrude (with draft), revolve, exact booleans, fillets and chamfers (`blend_ordered`), shell, slicing, exact mass properties with the inertia tensor (so MASSPROP now reports moments), point classification, contact tests, a native `.smb` format and a display mesher.
  - What it doesn't have: a hidden-line module. FLATSHOT's projection is DraftCraft's own (see S6).
  - Its positive shell thickness grows the body outward; the facade passes it negated, so a shell's walls go inside as in AutoCAD.
- **The ACIS record layer** (owner's call 3):
  - **cadmpeg-asm** (Apache-2.0, v0.6.12) reads ACIS: it frames every dialect and decodes the records. It is a dependency of `draftcraft-solid` under `kernel`, like solidcore (`cargo xtask layers` keeps both kernel families, `solidcore*` and `cadmpeg*`, inside that crate).
  - Writing is DraftCraft's own (`crates/solid/src/acis/write.rs`, `model.rs`): SAT save format 400 and SAB save format 700 from solidcore's STEP entities, from Spatial's published record layouts as cadmpeg-asm's grammar reads them back (the tests read every written file back through it).
  - **acadrust** (MPL-2.0) is out. DraftCraft no longer depends on it: the DWG writer replaced it.
- **The interim DWG/DXF form** is mesh plus exact body (owner's call 4), as described under Saving. The body bytes are solidcore's `.smb`; the XRECORD layout is unchanged.

### Open questions

1. **Panics on wasm.** How does solidcore's own browser build survive a kernel panic when `catch_unwind` can't catch it? Is there a panic-free mode, or a plan for one? Until then the web build keeps `solids` off.
2. **API stability.** `tangency-labs/solidcore` is a read-only mirror of `tangency-labs/solidcraft/solidcore`. How often does its API change, and is there a release to pin instead of a commit?
3. **Shell limits.** Which bodies is SOLIDEDIT shell expected to handle (curved faces, several removed faces)? What should a user see when it can't?

### Next steps

The full list of what is still missing compared with AutoCAD, with the order to do it in, is `handoff.md` at the repository root.


1. ACIS: check NURBS faces in AutoCAD (both ways are written); several lumps per 3DSOLID, REGION and BODY.
2. S4 remainder: a view per viewport, snaps in 3D, face and edge picking, a GPU path, and measuring the 10,000-face acceptance.
3. PRESSPULL, SWEEP, LOFT, THICKEN; STEP export.

---

## The advisory

### Solid modelling in DraftCraft with SolidCraft's kernel: advisory

Written 2026-11-09. Status: **S0, S2, S3 and S6 done; S4 in part, on solidcore instead of SolidCraft's kernel (see Status above).** Checked against `withshalom/solidcraft` at `d11d268` (v0.27.0) and `withshalom/draftcraft-app` at `3d1cf5e` (v2.0.2). Read with `claude/PLAN-ONE-MODEL.md` and `claude/DWG-READER-ROADMAP.md`; this plan adds to them and changes neither.


#### 1. The question: does DWG fidelity come first?

No. Three separate layers are involved, and only one of them is new work.

| Layer | What it is | State today |
|---|---|---|
| A. DWG/DXF container | The 3DSOLID record, and where its geometry blob is kept in the file | Carried through untouched on the DXF and acadrust path. Planned for the native reader (roadmap phase 3, item 10). |
| B. ACIS content | The solid itself: faces, edges and surfaces written in ACIS SAT (text) or SAB (binary) inside that blob | **Not understood by anything in DraftCraft or SolidCraft.** This is the gap. |
| C. Kernel | Code that builds and edits solids | SolidCraft has it. It speaks its own format and STEP, not ACIS. |

So the accurate version of your two readings:

- **Layer A is fidelity work you already planned.** It makes DraftCraft *keep* solids that are in a file. It does not make them editable, and the kernel isn't needed for it.
- **Layer B is the new development.** A translator between ACIS data and the kernel's bodies. Without it, a solid made in DraftCraft can't be saved as a real DWG solid, and a solid from a file can't be edited.
- **The DWG code needs only a small extension for 3D** (section 7). The large piece, the ACIS translator, sits beside the DWG code, not inside it.
- **The kernel doesn't have to wait for either.** It can come in first, with solids saved in ways that don't need ACIS (section 8, stage S3).

One correction to what I said earlier: the ACIS writer doesn't strictly need the native DWG writer. Today's save path (model → DXF → acadrust) already writes the data-store records back, so a new solid's ACIS blob could travel that way too.

#### 2. What the repos show

##### SolidCraft

- **`crates/kernel`** (about 95k lines, 906 tests) is the whole solid-modelling boundary. It depends only on `solidcraft-geom` (about 26k lines) and the truck crates. No truck type appears in its public API.
- **Operations:** `extrude`, `revolve`, `sweep`, `loft`, `box_solid`, `cylinder`, `sphere`, `torus`, `boolean`, `fillet`, `chamfer`, `shell`, `draft`, `split_by_plane`, `thicken`, `transform`, `measure_mass`, plus surfaces (`extrude_sheet`, `stitch`, `trim`, `offset_surface`).
- **Formats:** STEP in and out, IGES in and out, and an exact native binary body format (`encode_native_body` / `decode_native_body`, magic `SCBREP04`).
- **No ACIS.** `docs/step-import.md` says `.sat`/`.sab` files are not read yet. cadmpeg has a `sat` codec that SolidCraft deliberately leaves switched off.
- **Drawing support in `geom`:** `hlr` (exact hidden-line removal), `section` and `drawing_view`. These work on the portable B-rep, not on truck types.
- **Patched truck.** `vendor/` holds truck-geometry, truck-shapeops, truck-stepio, truck-meshalgo and matext4cgmath with local fixes, wired in by `[patch.crates-io]`.
- **Safety:** every call into truck runs under `guard`, which turns a panic into an error. Rayon on desktop, serial on wasm. A browser app already ships.
- **Licence:** MIT OR Apache-2.0; truck is Apache-2.0. Same as DraftCraft.
- **Versions:** Rust 1.95, edition 2024 and wgpu 30 in both repos. egui is 0.36 in SolidCraft and 0.37 (a patched fork) in DraftCraft.

##### DraftCraft

- **No solids yet.** `ROADMAP.md` says there is no 3D modelling, and lists "the first 3D commands" as a next step.
- **Solids are opaque.** 3DSOLID, BODY and foreign REGION are kept as `EntityKind::Unknown`. Their geometry in 2013+ files lives in the data-store section (`ACDSDATA`), which `doc::foreign::data_store` keeps as read and writes back (`io/tests/it/acis_data_store.rs`).
- **The native DWG codec names the types only.** `crates/dwg/src/read/object.rs` names REGION and 3DSOLID. I found no code in `crates/dwg` that decodes their record or reads the data-store section, so today that section comes through the acadrust path.
- **Regions are DraftCraft's own.** `Region` is a set of 2D loops. UNION, SUBTRACT, INTERSECT and MASSPROP already exist for regions. On save, a region is written as closed polylines with `DRAFTCRAFT` xdata, because `dxf_region.rs` treats ACIS as undocumented. That is a useful precedent (stage S3).
- **Interaction is 2D.** Entities decompose into `Prim`s built on `Vec2`. Snapping, selection, trimming and rendering all read those. Some entities hold 3D points (`Polyline3d`, `Face3d`, `PolyfaceMesh`, `Helix`).
- **Rules that bear on this:** crate layering (`cargo xtask layers`: L0 `geom`, `dxf` → L1 `doc` → L2 `render` → L3 `io` → L4 `engine` → L5 `ui-egui`), no panics, `unsafe_code = "forbid"`, and from `PLAN-ONE-MODEL.md`: no phase may make open slower, and no new runtime dependency in the open/save path.

#### 3. Recommendation

1. **Use SolidCraft's `kernel` and `geom` crates, and nothing above them.** SolidCraft's `doc` and `engine` are a parametric feature timeline. Bringing them in would be a second model, against the one-model rule.
2. **Keep every solid's original ACIS bytes, and write them back unchanged unless the solid was edited.** Fidelity then never depends on how good the translator is.
3. **Decode lazily.** Opening a file never calls the kernel. A solid is translated only when it is edited or needs a display the file didn't supply.
4. **Build the ACIS translator in tiers, writer first.** Writing only has to cover what DraftCraft's own commands can produce. Reading has to cope with anything AutoCAD wrote.
5. **Ship the drafting payoff early.** Hidden-line and section views from solids, as real 2D entities, is where a drafting product gains most from a kernel.

Alternatives considered:

| Option | Why not |
|---|---|
| Use truck directly | Loses SolidCraft's fixes, guards, fillets and 906 tests. |
| OpenCascade | C++ and LGPL: breaks pure Rust, forbid-unsafe and the browser build. |
| Write a kernel in BlackCAD's `cad_core` | Years of work that already exists under a compatible licence. |

#### 4. Where it sits in DraftCraft

| Crate | Layer | Holds |
|---|---|---|
| `draftcraft-acis` (new) | L0 | ACIS record streams: SAT text and SAB binary, read and write. No kernel, no `doc`. The same role `draftcraft-dxf` has for DXF. |
| `draftcraft-solid` (new) | L0 | A thin facade over `solidcraft-kernel`: the body type, the operations DraftCraft uses, tessellation, edge extraction, and the mapping between ACIS records and bodies. Converts between `draftcraft-geom` and `solidcraft-geom` vectors here and nowhere else. |
| `draftcraft-doc` | L1 | A new `EntityKind::Solid3d` (the name `Solid` is taken by the 2D SOLID entity), and later `Surface`. |
| `draftcraft-render` | L2 | Edges and silhouettes as `Prim`s; later a shaded mesh. |
| `draftcraft-io` | L3 | DXF and DWG mapping for the 3DSOLID record and its data-store record. |
| `draftcraft-engine` | L4 | Commands. |
| `draftcraft-ui-egui` | L5 | View direction, orbit, visual styles. |

**Why a facade.** Only `draftcraft-solid` names the kernel. The kernel can then be pinned, patched or replaced without touching `doc` or the engine, and `xtask layers` needs one new entry.

**The entity.** `Solid3d` holds:
- the original ACIS bytes and their form (SAT or SAB), if it came from a file;
- the decoded body (`Arc`, filled on demand), so undo snapshots stay cheap;
- an `edited` flag: clear means the original bytes are written back exactly;
- the display wires the file supplied. The DWG record carries wireframe and silhouette data (ODA spec 18.1.39), so an unedited solid can be drawn without decoding it.

#### 5. Bringing the kernel in

- **Pin a commit of your fork**, `withshalom/solidcraft`, as a git dependency. Same gate as `withshalom/cadcraft`: you decide which upstream changes enter the fork.
- **Repeat the truck patches in DraftCraft's root `Cargo.toml`.** Cargo ignores a dependency's own `[patch]` section; the root manifest already notes this for craft-pdf. The patch entries point at the vendored crates inside the solidcraft repo. Confirm this resolves in the spike.
- **Don't take SolidCraft's `gpu` or `ui-egui` crates.** They are on egui 0.36 and SolidCraft vendors its own egui-wgpu. Both repos use wgpu 30, so shaders and techniques can be adapted.
- **Put it behind a cargo feature** (`solids`) until the cost is measured: build time, binary size, wasm size.
- **Attribution:** add truck and SolidCraft to `ATTRIBUTION.md` and the third-party licence list.

Things to verify in the spike:

- **Units and scale.** SolidCraft's drawing code speaks in millimetres. DraftCraft drawings are in whatever units the file uses, sometimes far from the origin. Check how kernel tolerances behave on an inch drawing, a metre drawing, and one at survey coordinates. Likely answer: hold each body near its own origin, with a placement transform.
- **Panics in the browser.** `guard` relies on catching a panic. Confirm how SolidCraft's web build handles that, and that DraftCraft's wasm profile matches.
- **Determinism.** The vendored truck fixes include making boolean results come out in a stable order. Confirm results are stable across runs, since undo and tests compare drawings.

#### 6. The ACIS translator

##### What is publicly known

- **SAT is documented.** Spatial published the save-file format for ACIS 4.0 and 7.0. SAB holds the same data elements in binary.
- **AutoCAD's dialect moved on.** Autodesk's ShapeManager grew out of ACIS. The ODA notes that from version 20800 AutoCAD uses modified SAT and SAB, that Spatial versions above 700 are incompatible with DWG, and that data downgraded to version 700 is usable.
- **One body per entity.** The ODA notes a DWG entity holds a single body.
- **The container side is in the ODA spec** already in this Project (18.1.39): version 1 is SAT text with a simple character substitution; version 2 is SAT or SAB inline; SAB ends with a fixed end marker.

So **version 700 is the writer's target**: documented, and accepted by DWG readers. The reader must accept newer dialects too.

##### Existing Rust code to evaluate (not yet reviewed)

- **acadrust** (MPL-2.0, already a dependency, used unmodified). Its docs describe SAT and SAB parsing and writing, and builders for box, wedge, pyramid, cylinder, cone, sphere and torus. Per your rule, MPL code may be *used* but never copied into MIT/Apache files.
- **cadmpeg-asm.** Its docs describe parsing and writing ShapeManager record streams. Confirm its licence before use; the cadmpeg crates SolidCraft uses are Apache-2.0.
- **cadmpeg's `sat` codec**, which SolidCraft leaves off. Worth asking upstream why.
- **Never read GPL sources** (LibreDWG) for this. That rule stands.

Either library could supply the record layer, leaving DraftCraft to write only the mapping to kernel bodies. Decide after reading them (owner's call 3).

##### Writer tiers (kernel body → ACIS 700)

| Tier | Covers | Unlocks |
|---|---|---|
| W1 | Planar faces, straight edges | BOX, WEDGE, EXTRUDE of straight-sided profiles, booleans between them. Also real REGION entities, replacing the polyline stand-in. |
| W2 | Cylinder, cone, sphere, torus; circular and elliptical edges | CYLINDER, holes, REVOLVE, EXTRUDE of arcs, straight fillets and chamfers |
| W3 | NURBS surfaces and curves | LOFT, SWEEP, variable fillets, anything free-form |

**The main technical risk is here.** truck's booleans produce intersection curves as approximations (SolidCraft's `vendor/README.md` says so). ACIS expects edges to lie on their faces within a tight tolerance. For W1 and W2 the writer should **recompute each edge exactly from its two faces** (plane and plane give a line; plane and cylinder give a circle or ellipse) and not export the approximated curve. The kernel already classifies faces and edges (`face_surface`, `edge_circle`, `planar_face`), which gives the writer what it needs. W3 has no such shortcut and needs testing against AutoCAD.

**Acceptance per tier** (your black-box check): AutoCAD and TrueView open the file with no recovery prompt, and AutoCAD's MASSPROP volume and centroid match the kernel's `measure_mass`.

##### Reader tiers (ACIS → kernel body)

| Tier | Covers |
|---|---|
| R1 | Planar bodies |
| R2 | Analytic surfaces |
| R3 | Spline surfaces, and procedural ones (blends, offsets, sweeps) |

- **R3 is the long tail.** A possible shortcut, to verify: procedural surfaces often carry a NURBS approximation alongside their definition, which could be read in their place.
- **Fallback rule.** A body that can't be translated exactly stays opaque: shown, moved, copied, saved byte for byte, but not edited. The user is told why. Nothing is guessed.
- **Round-trip test.** Read, write, read again, and compare volume, area and topology counts.

#### 7. What the DWG and DXF code needs

Small next to section 6, and mostly already on the roadmap:

- [ ] **Native reader:** decode the REGION/3DSOLID/BODY record (ODA 18.1.39): the inline ACIS data for older versions, the wireframe and silhouette data, and the history handle.
- [ ] **Native reader:** read the data-store section for 2013+ files, where the geometry lives. I found no reader for it in `crates/dwg`.
- [ ] **Native writer:** write the record and its data-store record for 2018.
- [ ] **New solids:** allocate a handle, write a data-store record bound to it, and extend the schema records if the file had none.
- [ ] **Class-defined 3D objects carried through:** SURFACE and its kinds, MESH, solid-history objects. These belong with roadmap phase 3, item 12.
- [ ] **DXF:** the same record on the DXF side. Pre-2013 DXF holds SAT text in the entity; 2013+ uses the data store (`sat_text_dropped` already reports that case).

#### 8. Stages

Sizes are relative: S, M, L, XL.

| # | Stage | Size | Done when |
|---|---|---|---|
| S0 | **Spike.** Branch, add the kernel behind a feature, extrude a rectangle, tessellate, print volume. | S | Builds on desktop and wasm; build time, binary and wasm size recorded; section 5's three checks answered. |
| S1 | **Carry solids natively.** Section 7's reader and writer items. No kernel. | M | A file with solids opens through the native reader and saves with every ACIS blob byte-identical. |
| S2 | **Model and wireframe.** `Solid3d` in `doc`; edges drawn as `Prim`s projected to the current view, from the file's wires or the kernel's edges. | M | Solids show, select, snap, move and copy in the existing 2D pipeline; open time on the 200k-entity benchmark unchanged. |
| S3 | **Create and edit, saved without ACIS.** First commands (below). Solids saved exactly in DraftCraft's own format with `encode_native_body`; STEP export. In DWG/DXF: a mesh AutoCAD can see, plus the exact body in an extension-dictionary record so DraftCraft reopens it as a solid. The save-loss notice says so. | L | Make, edit, save, reopen in DraftCraft: identical body. AutoCAD shows the mesh. |
| S4 | **3D view.** View direction per viewport, orbit, hidden and shaded styles, face and edge picking, UCS on a face. | L | Orbit and shade a 10,000-face model smoothly on desktop and in the browser. |
| S5 | **ACIS writer** W1 → W2 → W3. Replaces the S3 mesh tier by tier. | XL | Per tier, the AutoCAD acceptance in section 6. |
| S6 | **Drawings from solids.** FLATSHOT-style and section views through `hlr` and `section`, output as ordinary lines, arcs and hatches. | M | A projected view of a test part matches hand-checked geometry; uncertain edges are reported, not guessed. |
| S7 | **ACIS reader** R1 → R2 → R3. Existing solids become editable. | XL | Per tier, the round-trip test; untranslatable bodies stay opaque. |

**First commands (S3), by AutoCAD name:**

| Command | Kernel call |
|---|---|
| BOX, CYLINDER, SPHERE, TORUS | `box_solid`, `cylinder`, `sphere`, `torus` |
| EXTRUDE, PRESSPULL | `extrude`, `extrude_tapered` (from closed polylines, circles, regions) |
| REVOLVE | `revolve` |
| UNION, SUBTRACT, INTERSECT | `boolean` (extending the region commands that exist) |
| SLICE | `split_by_plane` |
| FILLETEDGE, CHAMFEREDGE | `fillet`, `chamfer` |
| SOLIDEDIT shell | `shell` |
| MASSPROP, INTERFERE | `measure_mass`, `meets` |
| Later: SWEEP, LOFT, THICKEN | `sweep`, `loft`, `thicken` |

Each keeps DraftCraft's command front half (names, aliases, prompts, ribbon) and adds only a back half.

**Order against the DWG roadmap:**

- **S0** any time. It is isolated and answers the cost questions.
- **S1** is roadmap phase 3 items 10 and 12 done properly. It belongs inside the reader work, not after it.
- **S2 onward** after the native reader takes over (roadmap phase 8), so the entity lands in a settled model.
- **S5** can use today's DXF → acadrust save path. It does not have to wait for the native writer.
- **S6** needs only S2 and S3. It can ship before any ACIS work.

#### 9. Risks

| Risk | Effect | Answer |
|---|---|---|
| AutoCAD rejects written ACIS | No DWG solids from DraftCraft | Tiers; exact edges for W1/W2; S3's mesh-plus-exact-body stays as the fallback |
| Kernel fails on a boolean or fillet | Command fails | `guard` returns an error; the command reports it and changes nothing |
| Kernel tolerances at odd units or large coordinates | Wrong or failed results | Spike check; local frame per body |
| Binary and wasm size, build time | Slower builds, bigger download | Measure in S0; cargo feature |
| Slower open | Breaks a standing rule | Lazy decode; benchmark gate at S2 |
| Upstream SolidCraft changes the kernel API | Pin breaks | Facade crate; pinned fork |
| ShapeManager dialects the reader doesn't know | Some solids not editable | Opaque fallback; never rewritten |
| Scope: this is a second product's worth of work | DWG fidelity slips | S0 and S1 only until the reader switch-over |

#### 10. Owner's calls

1. **Does SkryxDev agree?** This adds a large dependency and a model entity to the shared repo. Options: behind a feature in the shared repo, or on your fork until S3 works.
2. **Kernel as a git dependency on your fork** (recommended, like craft-libs) or copied into the DraftCraft repo.
3. **ACIS record layer:** acadrust's, cadmpeg-asm's, or our own `draftcraft-acis`. Decide after reviewing the first two.
4. **S3's interim DWG form:** mesh plus exact body (recommended), or refuse to save solids to DWG until S5.
5. **Is 3D in scope for DraftCraft's identity** as a drafting product, and how far: solids for drafting (through S6), or a full modeller (S7 and free-form)?

#### Sources

- `withshalom/solidcraft`: `crates/kernel/src/lib.rs`, `crates/geom/src/hlr/mod.rs`, `vendor/README.md`, `docs/step-import.md`, `ROADMAP.md`
- `withshalom/draftcraft-app`: `crates/doc/src/entity.rs`, `crates/doc/src/foreign.rs`, `crates/io/src/dxf_region.rs`, `crates/io/tests/it/acis_data_store.rs`, `crates/dwg/src/read/object.rs`, `AGENTS.md`, `ROADMAP.md`
- Open Design Specification for .dwg files, 18.1.39 (in this Project)
- [ODA: Native and modified .sat and .sab file versions](https://opendesign.com/node/1285)
- [SAT Save File Format (Spatial, via Paul Bourke)](https://paulbourke.net/dataformats/sat)
- [acadrust `entities::acis`](https://docs.rs/crate/acadrust/0.3.4/source/src/entities/acis/mod.rs)
- [cadmpeg-asm](https://docs.rs/cadmpeg-asm)
- [CAD Exchanger: ACIS format overview](https://cadexchanger.com/blog/3d-formats-overview-acis/)
