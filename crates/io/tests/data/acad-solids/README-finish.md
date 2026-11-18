# AutoCAD test drawings for the rest of 3D (`sab`, `lumps`, `mirror`, `procsurf`, `regions-ucs`, `surfaces`, `mesh`, `history`, `facecolor`, `legacy3d`)

Made on 2026-11-17 in **AutoCAD 2027 for Mac**, used only as a black box. The UI was Italian and the build
was the educational version ("VERSIONE DIDATTICA (NON COMMERCIALE)"), so the files may carry AutoCAD's
educational stamp. That stamp comes from the licence, not from the geometry.

Each drawing was started with `_NEW` from the `acadiso.dwt` template (metric, `$MEASUREMENT` 1), with
`INSUNITS` 4 (mm), `OSMODE` 0, `DYNMODE` 0 and `DELOBJ` 3. Everything is in model space, on layer 0, with
ByLayer properties, except the face colour in `facecolor`. No layers, styles or blocks were added. The commands
went in through AutoCAD script files (`SCRIPT`, `FILEDIA` 0), using English command names with `_`.
AutoLISP `(entlast)` / `!var` picked the objects. After drawing, the script did the following:

1. Ran `ZOOM _E`.
2. Ran `AUDIT` (no fix), `SOLIDEDIT ▸ Body ▸ Check` on every 3DSOLID, `MASSPROP` on every 3DSOLID and REGION,
   `AREA ▸ Object` on everything else, and `LIST` on every object.
3. Saved the drawing with `_SAVEAS` as DWG 2018, then as ASCII DXF 2018 and DXF 2000 with 16 decimals
   (`_SAVEAS _DXF _V <ver> 16 <file>`). `sab` was also saved as DWG 2004, 2007 and 2010 first.
4. Closed the drawing without saving it again.

Every header was checked after saving:
- DWG 2004 = `AC1018`
- DWG 2007 = `AC1021`
- DWG 2010 = `AC1024`
- DWG 2018 = `AC1032`
- each DXF's `$ACADVER` matches its version: 2018 = `AC1032`, 2000 = `AC1015`.

`AUDIT` reported "Totale di 0 errori trovati" in every drawing. Every 3DSOLID passed the body check
("L'oggetto è un solido ShapeManager valido").

Handles are the ones AutoCAD gave in the drawing as made. The DWG and DXF files keep the same handles.
Volumes, centroids and bounding boxes are AutoCAD's MASSPROP output (density 1), copied as printed.
The bounding boxes are given as X min–max, Y min–max, Z min–max.

## Files

| Drawing | Files |
|---|---|
| sab | `sab-2018.dwg`, `sab-2010.dwg`, `sab-2007.dwg`, `sab-2004.dwg`, `sab-2018.dxf`, `sab-2000.dxf` |
| every other drawing | `<name>-2018.dwg`, `<name>-2018.dxf`, `<name>-2000.dxf` |

## 1. `sab`: three primitives (plan view, Wireframe 2D)

`BOX 0,0,0 100,60,40`, `CYLINDER 200,0,0 25 50` and `SPHERE 400,0,30 30`.

| Handle | Type | Volume | Centroid | Bounding box |
|---|---|---|---|---|
| 2DB | 3DSOLID (box) | 240000.0000 | 50, 30, 20 | 0–100, 0–60, 0–40 |
| 2DF | 3DSOLID (cylinder) | 98174.7704 | 200, 0, 25 | 175–225, −25–25, 0–50 |
| 2E3 | 3DSOLID (sphere) | 113097.3355 | 400, 0, 30 | 370–430, −30–30, 0–60 |

## 2. `lumps`: one solid made of two separate boxes

`BOX 0,0,0 50,50,50`, `BOX 200,0,0 250,50,50`, then `UNION _ALL`. LIST shows **one** 3DSOLID (handle 2DF).
The drawing holds nothing else.

| Handle | Type | Volume | Centroid | Bounding box |
|---|---|---|---|---|
| 2DF | 3DSOLID (2 lumps) | 250000.0000 | 125, 25, 25 | 0–250, 0–50, 0–50 |

## 3. `mirror`: mirrored solids (source erased)

- `WEDGE 0,0,0 100,50 30`, then `MIRROR _L` about the line 0,0 → 0,10, erasing the source (`_Y`).
- `CONE 200,0,0 30 60`, then `MIRROR3D _L` about the `_XY` plane through 0,0,0, erasing the source.

| Handle | Type | Volume | Centroid | Bounding box |
|---|---|---|---|---|
| 2DB | 3DSOLID (mirrored wedge) | 75000.0000 | −33.3333, 25, 10 | −100–0, 0–50, 0–30 |
| 2DF | 3DSOLID (mirrored cone, apex down) | 56548.6678 | 200, 0, −15 | 170–230, −30–30, −60–0 |

## 4. `procsurf`: solids with procedural faces (view SW iso)

| Handle | How | Volume | Centroid | Bounding box |
|---|---|---|---|---|
| 2DB | `CYLINDER 0,0,0 30 60`, then `FILLETEDGE _R 5` on the top circular edge (edge picked at 30,0,60) | 168672.3642 | 0, 0, 29.8334 | −30–30, −30–30, 0–60 |
| 2E1 | `BOX 120,-30,0 180,30,40`, then `SOLIDEDIT ▸ Face ▸ Taper` on the side face at x = 120 (picked at 120,0,20; base point 120,0,0, second point 120,0,40, angle 10) | 135536.3049 | 151.7266, 0, 19.5837 | 120–180, −30–30, 0–40 |
| 2E5 | `CYLINDER 300,0,0 30 60`, then `SOLIDEDIT ▸ Face ▸ Offset` 5 on the side face (picked at 278.7868,−21.2132,30) | 230907.0600 | 300, 0, 30 | 265–335, −35–35, 0–60 |
| 2EB | `HELIX 450,0,0`, base radius 30, top radius 10, `_T` 3 turns, height 60; `CIRCLE 600,0,0 2`; then `SWEEP _MO _SO` of the circle along the helix | 4809.8419 | 450.1110, 2.0906, 25.1543 | 421.3123–481.9988, −27.0225–30.3532, −1.9889–61.9068 |

Checks:
- The offset cylinder is r 35: π·35²·60 = 230907.06.
- The taper removes a wedge of 40·tan 10°·40/2·60 = 8464 from the 144000 box.

DELOBJ 3 deleted the helix and the circle, so the drawing holds just these 4 solids.

The first try made a SWEPTSURFACE instead of a solid. SWEEP had kept the surface mode from the `surfaces`
drawing made earlier in the same session. procsurf was then made again from scratch with an explicit
`_MO _SO`. The files here are that second version.

## 5. `regions-ucs`: regions off the XY plane and from curves

| Handle | How | Area | Perimeter | Centroid | Bounding box |
|---|---|---|---|---|---|
| 2DA | `UCS _X 90`, `RECTANG 0,0 50,30`, `REGION _L`, `UCS _W`: lies in the WCS XZ plane | 1500.0000 | 160.0000 | 25, 0, 15 | 0–50, 0–0, 0–30 |
| 2DC | `ELLIPSE 100,0 140,0 10` (axes 40 and 20), `REGION _L` | 628.3185 | 96.8845 | 120, 0 | 100–140, −10–10 |
| 2DE | `SPLINE 200,0 240,10 250,40 220,50 190,30 _C` (closed fit spline; at "Specificare tangente:" the default was taken with Enter), `REGION _L` | 2567.6843 | 182.8775 | 219.1624, 24.8072 | 188.8559–251.3582, −2.4025–50.2120 |

MASSPROP prints a 3D bounding box and centroid for 2DA because it isn't parallel to the current UCS
(the WCS). For the other two it prints them in 2D.

## 6. `surfaces`: one surface of each kind

AutoCAD made these surfaces associative ("Associativo: Sì" in LIST). It kept their defining curves even
with DELOBJ 3, so the drawing holds 12 objects.

| Handle | Type | How | AREA (▸ Object) | Bounding box (LIST) |
|---|---|---|---|---|
| 2D9 | PLANESURFACE | `PLANESURF 0,0 50,50` | 2500.0000 (perimeter 200) | 0–50, 0–50, 0 |
| 2DE | NURBSURFACE | a copy of 2D9, then `CONVTONURBS _L` (2 × 2 control vertices, degree 1 × 1, not associative) | 2500.0000 (perimeter 200) | 0–50, 200–250, 0 |
| 2E0 | EXTRUDEDSURFACE | `ARC 100,0 125,25 150,0` (2DF), then `EXTRUDE _MO _SU` 30 | 2356.1945 | 100–150, 0–25, 0–30 |
| 2EF | LOFTEDSURFACE | `LINE 200,0,0 250,0,0` (2ED) and `LINE 200,30,20 250,30,20` (2EE), then `LOFT _MO _SU`, cross sections only | 1802.7756 | 200–250, 0–30, 0–20 |
| 2FB | REVOLVEDSURFACE | `LINE 310,0,0 320,0,40` (2FA), then `REVOLVE _MO _SU` about 300,0,0 → 300,0,10, 360° | 3885.9355 | 280–320, −20–20, 0–40 |
| 305 | SWEPTSURFACE | `ARC 400,0 410,10 420,0` (303) swept with `SWEEP _MO _SU` along `LINE 450,0,0 450,0,60` (304) | 1884.9556 | 440–460, −10–0, 0–60 |

The other objects are the defining curves: ARC 2DF, LINE 2ED, LINE 2EE, LINE 2FA, ARC 303 and LINE 304.

The NURBS surface sits at Y 200 because a script restart copied the plane surface twice. The stray
PLANESURFACE (2DC, at Y 100–150) was then erased in a reopened copy of `surfaces-2018.dwg`, and all three
`surfaces-*` files were saved again from that drawing. The files therefore come from a DWG 2018 that
AutoCAD had saved and reopened, not from the first session. The handles are unchanged.

## 7. `mesh`: subdivision meshes

`MESH _B 0,0,0 50,50,50`, then `COPY` of it to 100,0, then `MESHSMOOTHMORE` on the copy.

| Handle | Type | Smoothness (LIST) | AREA (▸ Object) | Bounding box |
|---|---|---|---|---|
| 2D9 | MESH (box) | 0 | 15000.0000 | 0–50, 0–50, 0–50 |
| 2DA | MESH (smoothed copy) | 1 | 12739.0350 | 100–150, 0–50, 0–50 |

## 8. `history`: solids with history (`SOLIDHIST` 1)

`SOLIDHIST 1`, then:
- `BOX 0,0,0 100,60,40`;
- `CYLINDER 200,0,0 20 50`;
- `BOX 300,0,0 400,60,40` minus `CYLINDER 350,30,0 15 40` (`SUBTRACT`).

| Handle | Type | Volume | Centroid | Bounding box |
|---|---|---|---|---|
| 2DB | 3DSOLID (box) | 240000.0000 | 50, 30, 20 | 0–100, 0–60, 0–40 |
| 2DF | 3DSOLID (cylinder) | 62831.8531 | 200, 0, 25 | 180–220, −20–20, 0–50 |
| 2E3 | 3DSOLID (box − cylinder) | 211725.6661 | 350, 30, 20 | 300–400, 0–60, 0–40 |

**What AutoCAD shows for the box (2DB).** These values come from LIST and from `dumpallproperties`, AutoCAD's
own dump of the object's properties: the same properties, with their Italian names, that feed the Properties
palette. The palette itself was not opened.
- LIST:
  - "Cronologia = Registra", "Mostra cronologia = No"
  - "Tipo solido = Parallelepipedo"
  - "Posizione X = 50 Y = 30 Z = 0"
  - "Lunghezza: 100", "larghezza: 60", "Altezza: 40", "Rotazione: 0"
- Properties:
  - Tipo solido (`SolidType`) = **Parallelepipedo** (Box)
  - Lunghezza (`Length`) = **100**
  - Larghezza (`Width`) = **60**
  - Altezza (`Height`) = **40**
  - X/Y/Z posizione (`SolidPosition`) = 50, 30, 0, the centre of the base
  - Rotazione = 0
  - Registra cronologia (`RecordHistory`) = 1, Mostra cronologia (`ShowHistory`) = 0
  - `IsPrimitive` = 1
  - `Area` (surface area) = 24800

The cylinder (2DF) shows Tipo solido **Cilindro**, Raggio 20, Altezza 50 and `IsPrimitive` 1. The
subtracted box (2E3) has `IsPrimitive` 0 and no Tipo solido. Its history is recorded.

When the saved DWG 2018 was reopened, SOLIDHIST still read 1.

## 9. `facecolor`: one face coloured (view SW iso)

`BOX 0,0,0 100,60,40`, then `SOLIDEDIT ▸ Face ▸ Color` on the top face (picked at 50,30,40: "1 faccia
trovata"). On Mac, AutoCAD asked for the colour on the command line ("Nuovo colore
[Truecolor/CAtalogocolori] <DALAYER>:"), and **1** (red) was typed. The solid itself stays ByLayer.

| Handle | Type | Volume | Centroid | Bounding box |
|---|---|---|---|---|
| 2DB | 3DSOLID, top face (z = 40) red | 240000.0000 | 50, 30, 20 | 0–100, 0–60, 0–40 |

## 10. `legacy3d`: pre-ACIS 3D objects

| Handle | Type (LIST) | How | AREA / LIST |
|---|---|---|---|
| 2D9 | 3DFACE ("FACCIA 3D") | `3DFACE 0,0,0 50,0,0 50,50,20 0,50,20` | AREA: "L'oggetto selezionato non possiede un'area" |
| 2DA | POLYLINE, 3D polyline (`AcDb3dPolyline`) | `3DPOLY 0,0,0 50,0,30 50,50,0` | AREA 0: "I vertici della polilinea 3D avevano coordinate Z diverse." |
| 2DF | LINE | `LINE 0,0,0 0,0,50` | — |
| 2E0 | CIRCLE, thickness 30 (LIST: "Altezza = 30") | `CIRCLE 100,0 20`, then `CHPROP _L _T 30` | area 1256.6371, circumference 125.6637 |
| 2E1 | CIRCLE r 15, extrusion direction 0,−1,0 | `UCS _X 90`, `CIRCLE 200,0 15`, `UCS _W`: centre 200,0,0 WCS | LIST: area 706.8583, circumference 94.2478. AREA refused it: "L'oggetto non è parallelo all'UCS." |
| 2E2 | POLYLINE, 3 × 3 polygon mesh (`AcDbPolygonMesh`) | `3DMESH 3 3`, vertices 300,0,0 300,25,5 300,50,0 325,0,5 325,25,10 325,50,5 350,0,0 350,25,5 350,50,0 | AREA: no area |
| 2ED | POLYLINE, polyface mesh (`AcDbPolyFaceMesh`) | `PFACE`, vertices 400,0,0 450,0,0 450,50,0 400,50,0 425,25,30; faces 1 2 3 4, 1 2 5, 2 3 5, 3 4 5, 4 1 5 (a square pyramid) | AREA: no area |

## Notes on the session
- One script ran all the drawings one after another. A few steps needed a restart from the step that failed:
  - the spline tangent prompt;
  - an extra blank line after COPY;
  - an AREA helper that stalled on open arcs.

  None of these changed a saved drawing, except the two cases described above (procsurf made again, the
  extra plane surface in `surfaces` erased).
- No screenshot of AutoCAD was saved. Nothing in the Autodesk install was read.
