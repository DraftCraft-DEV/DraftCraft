# DWG support for 3D objects: handoff

Written 2026-11-18 for Codie. It covers the 3D objects DraftCraft's DWG codec still can't carry: what
happens to them now, where the work goes, how the MATERIAL fix was found (the same method works for
these), and an order to take them in.

## Where things stand

`draftcraft-cli roundtrip` in strict mode (`DRAFTCRAFT_CORPUS_STRICT=1`) on AutoCAD 2027's test
drawings (`crates/io/tests/data/acad-solids/`) reported 29 findings, all "lost on save, not written",
all for DXF → DWG. MATERIAL (3 per file, in every file) was one of them. It is fixed on `main`
([DraftCraft-DEV/DraftCraft#29](https://github.com/DraftCraft-DEV/DraftCraft/pull/29), a draft until the owner merges it). That leaves 22 findings, in these types:

| Types | Where | Per file |
|---|---|---|
| `ACDBPERSSUBENTMANAGER`, `ACDBASSOCPERSSUBENTMANAGER` | every drawing with a solid, 2000 and 2018 DXF | 1 each |
| `ACSH_HISTORY_CLASS` and the nodes `ACSH_BOX_CLASS`, `ACSH_CYLINDER_CLASS`, `ACSH_CONE_CLASS`, `ACSH_SPHERE_CLASS`, `ACSH_TORUS_CLASS`, `ACSH_WEDGE_CLASS`, `ACSH_BOOLEAN_CLASS`, `ACSH_LOFT_CLASS`, `ACSH_SWEEP_CLASS` | 2010 and 2018 DXF; AutoCAD's DXF 2000 declares their classes but leaves the objects out, while its DWG 2000 keeps them | 1 history per solid, 1 node per primitive or operation |
| `MESH` | `mesh` | 2 |
| `SURFACE` (2000), `NURBSURFACE` (2018), `PLANESURFACE`, `EXTRUDEDSURFACE`, `LOFTEDSURFACE`, `REVOLVEDSURFACE`, `SWEPTSURFACE` | `surfaces` | 1 each |
| `ACDBASSOCNETWORK`, `ACDBASSOCACTION`, `ACDBASSOCDEPENDENCY`, `ACDBASSOCEDGEACTIONPARAM`, `ACDBASSOCPATHACTIONPARAM`, `ACDBASSOC*SURFACEACTIONBODY` | `surfaces` | 2, 4, 10, 6, 8, 1 of each body |

Things to know before starting:

- **None of these crash anything.** The writer counts each record it can't encode, leaves it out and
  writes a valid file. `left_out` and `empty_extension_dictionaries` in `crates/io/src/dwg/write/mod.rs`
  prune them first, and `cx.known` sets any reference to them to 0 (a 3DSOLID's history, group 350, for
  example). Every saved file reopened in the strict run.
- **DWG → DWG loses them too, on open rather than on save.** The DWG reader can't decode them, so
  opening AutoCAD's DWG reports "N of M object records couldn't be read and were left out". For
  example, `sab-2018.dwg` drops `ACSH_HISTORY_CLASS × 3`, `ACDBASSOCPERSSUBENTMANAGER`,
  `ACDBPERSSUBENTMANAGER`, `ACSH_BOX_CLASS`, `ACSH_CYLINDER_CLASS` and `ACSH_SPHERE_CLASS`, and
  `mesh-2018.dwg` drops `MESH × 2`. The roundtrip's DWG → DWG route shows no "lost on save" only because
  these records never reach the writer. So each type needs both a decoder (DWG → groups) and an encoder
  (groups → DWG).
- **On `main` the 3DSOLIDs of `sab-2004.dwg`, `sab-2007.dwg` and `sab-2010.dwg` aren't read** either
  (`3DSOLID × 3` in the same note). Check whether this branch already reads them.

## Where the work goes

The owner decided on 2026-11-18 that DWG work goes on `main`, which is also AGENTS.md's rule: DWG and DXF
work goes to `main` when it works without the `solids` feature. Carrying these records through DWG
(decoding a record into its DXF groups and encoding the groups back) needs no solid kernel, so it goes
to `main`. Work that does need the kernel stays here: evaluating a history, rebuilding a solid from
its history, the commands.

The same pull request puts the `acad-solids` drawings on `main` too: byte-identical copies, with their
rows in `ATTRIBUTION.md` placed exactly as here, so merging `main` into `3D` sees the same change on
both sides.

## Reproduce

```bash
DRAFTCRAFT_CORPUS_STRICT=1 cargo run -p draftcraft-cli -- roundtrip crates/io/tests/data/acad-solids --out <report dir>
```

Read `<report dir>/report.md`: the per-file notes on opening show what the reader left out, and each
route shows what was lost, added or changed.

## The method that worked for MATERIAL

Each drawing's DWG and DXF were saved from the same AutoCAD session, so they keep the same handles: the
DXF tells you what each DWG field means. Only AutoCAD's files, the public DXF Reference and the ODA's
public DWG specification were used (clean-room, AGENTS.md).

1. **Dump the record's bits.** Add a temporary `eprintln!` where the codec starts the type's own fields.
   For style objects that's `crates/dwg/src/read/styles.rs` (`decode`); for other objects,
   `crates/dwg/src/read/nongraphic.rs`; for entities, `crates/dwg/src/read/entities.rs`. Print the data
   stream from there to its end (`s.dat.clone()`, `s.dat.end()`), then run
   `draftcraft-cli info file.dwg`. Revert the print afterwards.
2. **Account for every bit.** Write a scratch parser (outside the repo) using the bit codes (B, BS, BL,
   BD, RC, TV, …) and work out the layout until every record of the type, in every version you have
   (2000, 2004, 2007, 2010, 2013, 2018), parses with no bits left over. Strings sit in the text stream
   from 2007 on and inline before that, so the 2000 and 2004 files show where the strings go.
3. **Know AutoCAD 2027's DXF rule.** It writes a group only when its value differs from AutoCAD's
   default. In MATERIAL, a map's source was 1 (an image file, the default, so no group 72), a matrix was
   the identity (no group 43 at all), and the channels were 127 (no group 94). The older AutoCAD that
   saved `mi201` wrote source 0, so its DXF has `72 0`; the codec only knew that form. When encoding,
   supply the default for any group the DXF leaves out. When decoding, leave the defaults out, so the
   output matches AutoCAD's DXF.
4. **Encode only what you understand.** Write fixed defaults for the fields you don't model, and return
   `Err` for any group or value you can't place: the record is then left out and counted, never guessed.
   No panics: `get()`, checked conversions, capped counts.
5. **Test it three ways, as `main`'s MATERIAL fix does.**
   - A codec unit test with AutoCAD's exact bits as the expected output (`styles.rs`, `BYLAYER_2018`).
   - AutoCAD's DWG reads to the groups its DXF of the same version gives.
   - AutoCAD's DXF saved as a DWG of every version reads back the same.

   Check that the new tests fail without the fix (`crates/io/tests/it/style_objects_dwg.rs`).

## Suggested order

1. **`ACDBPERSSUBENTMANAGER` and `ACDBASSOCPERSSUBENTMANAGER`.** One of each in every drawing with a
   solid, and small (in `sab-2018.dxf`: groups 90 only, plus a 290). Both are owned by the named-object
   dictionary (330 = C). This is the most value for the least work: with them, 9 of the 22 findings (every
   2000 DXF but `surfaces`) go away.
2. **Solid history: `ACSH_HISTORY_CLASS` and the `ACSH_*_CLASS` nodes.** A history (`AcDbShHistory`:
   90, 91, 360 → its `ACAD_EVALUATION_GRAPH`, 92, 280, 281) belongs to a 3DSOLID. Each node
   (`AcDbEvalExpr`, `AcDbShHistoryNode` with a 4×3 transform in 40–55, colour 62, material 347;
   `AcDbShPrimitive`, then the primitive's own subclass, such as `AcDbShBox` 40/41/42 = length, width,
   height) belongs to the graph. When they're left out, the solid keeps its shape but AutoCAD loses
   its history and primitive grips. Write the graph and its nodes together. Check in AutoCAD that the
   file opens without recovery, because a graph whose nodes are missing may not.
3. **`MESH`** (`AcDbSubDMesh`). The DXF gives it whole: vertices, faces, edges, creases. It's an entity
   with no ACIS data, so it's the most self-contained of the entities. Add it to
   `crates/io/src/dwg/write/entities.rs` and to the reader.
4. **The surfaces.** They're modeler entities like 3DSOLID: reuse `solid::modeler`
   (`crates/io/src/dwg/write/solid.rs`) for the ACIS part (SAT text up to 2010, the data store from
   2013), then add `AcDbSurface` (its isolines) and each type's own subclass (the extrusion, sweep,
   revolution or loft parameters and transforms; the NURBS surface's data). `surfaces-2000.dxf` stores
   the NURBS surface as a plain `SURFACE`, `surfaces-2018.dxf` as `NURBSURFACE`. `procsurf` has the
   procedural surfaces' history (`ACSH_SWEEP_CLASS`).
5. **The associative network (`ACDBASSOC*`).** The largest piece, found only in `surfaces`. Without it
   the surfaces stay but lose their associativity, so do it last.

## Related gaps on `main` (not 3D, found along the way)

- **The DWG reader drops the extended data of the objects it keeps.** `object_tags` in
  `crates/io/src/dwg/carry.rs` builds an object's DXF groups without its extended data. For example,
  the Global material's `ACAD` data (`1070 -1, 3, 0, 1000 "", 1071 0, 1070 0`) is in AutoCAD's DWG,
  and in DraftCraft's DWG of the same drawing, but not in the drawing either one opens to.
- **Materials get new handles on DXF → DXF and DXF → DWG** (in the baseline too), so the header's
  `$CMATERIAL` points at nothing and is lost.
