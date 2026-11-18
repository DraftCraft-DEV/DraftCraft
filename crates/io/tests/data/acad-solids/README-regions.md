# AutoCAD REGION test drawings (`regions-*`)

Made on 2026-11-15 in **AutoCAD 2027 for Mac** (Italian UI, educational licence: "VERSIONE DIDATTICA
(NON COMMERCIALE)"), used only as a black box, like the `solids-*` drawings in this folder. One new
drawing from the `acadiso.dwt` template (metric, `$MEASUREMENT` 1), `INSUNITS` set to 4 (mm), WCS, model
space, layer 0, everything on the XY plane at Z = 0. Wireframe 2D, plan view.

The same drawing was saved six times with `_SAVEAS`:

| File | Format | Version check |
|---|---|---|
| `regions-2018.dwg` | DWG 2018 | header `AC1032` |
| `regions-2013.dwg` | DWG 2013 | header `AC1027` |
| `regions-2000.dwg` | DWG 2000 | header `AC1015` |
| `regions-2018.dxf` | ASCII DXF 2018, 16 decimals | `$ACADVER` `AC1032`; ACIS data (SAB) in the `ACDSDATA` section |
| `regions-2010.dxf` | ASCII DXF 2010, 16 decimals | `$ACADVER` `AC1024`; SAT text in the REGION entities |
| `regions-2000.dxf` | ASCII DXF 2000, 16 decimals | `$ACADVER` `AC1015`; SAT text in the REGION entities |

Each file holds exactly **5 REGION entities** and nothing else in model space. AutoCAD deleted the source
polylines and circles (default `DELOBJ`). Handles (the same in all three DXF files), in creation order:
`2DA`, `2DC`, `2DE`, `2E2`, `2E4`.

## How each region was made

Commands went in through an AutoCAD script (`SCRIPT`, with `FILEDIA 0`, `DYNMODE 0`, `OSMODE 0`):

| # | Handle | Commands | Result |
|---|---|---|---|
| 1 | `2DA` | `_RECTANG 0,0 100,60`, `_REGION _L` | rectangle 100 × 60, corner at the origin |
| 2 | `2DC` | `_CIRCLE 200,30 30`, `_REGION _L` | disc, centre (200, 30), radius 30 |
| 3 | `2DE` | `_RECTANG 300,0 400,60`, `_REGION _L`; `_CIRCLE 350,30 15`, `_REGION _L`; `_SUBTRACT` (rectangle region) − (circle region) | plate 100 × 60 with a round hole r 15 at (350, 30): one region, outer loop + one hole |
| 4 | `2E2` | `_PLINE 500,0 600,0 600,60 _A 500,60 _L _C`, `_REGION _L` | slot: lines (500,0)→(600,0)→(600,60), a tangent half-circle arc (600,60)→(500,60) through (550,110) (centre (550,60), r 50), then a line back to (500,0) |
| 5 | `2E4` | `_RECTANG 700,0 720,20`, `_REGION _L`; `_RECTANG 740,0 760,20`, `_REGION _L`; `_UNION` of the two | one REGION made of two separate 20 × 20 squares |

## MASSPROP (as AutoCAD printed it)

Each region was measured on its own (`_MASSPROP`, written to a file). Values exactly as printed, four
decimals. "Bounding box" = "Casella di delimitazione", "Centroid" = "Baricentro".

| # | Region | Area | Perimeter | Bounding box X | Bounding box Y | Centroid X | Centroid Y |
|---|---|---|---|---|---|---|---|
| 1 | rectangle | 6000.0000 | 320.0000 | 0.0000 -- 100.0000 | 0.0000 -- 60.0000 | 50.0000 | 30.0000 |
| 2 | circle | 2827.4334 | 188.4956 | 170.0000 -- 230.0000 | 0.0000 -- 60.0000 | 200.0000 | 30.0000 |
| 3 | plate with hole | 5293.1417 | 414.2478 | 300.0000 -- 400.0000 | 0.0000 -- 60.0000 | 350.0000 | 30.0000 |
| 4 | slot (polyline with arc) | 9926.9908 | 377.0796 | 500.0000 -- 600.0000 | 0.0000 -- 110.0000 | 550.0000 | 50.2622 |
| 5 | two squares (union) | 800.0000 | 160.0000 | 700.0000 -- 760.0000 | 0.0000 -- 20.0000 | 730.0000 | 10.0000 |

Hand checks: 3 = 6000 − π·15² and 320 + 2π·15; 4 = 100·60 + π·50²/2 and 100 + 60 + 60 + π·50;
5 = 2 · 400 and 2 · 80 (the perimeter counts both loops).

### Rest of each report (moments about the WCS origin, as printed)

| # | Moments of inertia X / Y | Product of inertia XY | Radii of gyration X / Y | Principal moments about the centroid (I, J) and directions |
|---|---|---|---|---|
| 1 | 7200000.0000 / 20000000.0000 | -9000000.0000 | 34.6410 / 57.7350 | I 1800000.0000 along [1.0000 0.0000]; J 5000000.0000 along [0.0000 1.0000] |
| 2 | 3180862.5618 / 113733508.0416 | -16964600.3294 | 33.5410 / 200.5617 | I 636172.5124 along [0.9970 0.0777]; J 636172.5124 along [-0.0777 0.9970] |
| 3 | 6524066.7056 / 653370091.7034 | -55577987.3559 | 35.1077 / 351.3362 | I 1760239.2180 along [1.0000 0.0000]; J 4960239.2180 along [0.0000 1.0000] |
| 4 | 33791536.2018 / 3010369091.3993 | -274424030.2939 | 58.3439 / 550.6822 | I 8713052.3382 along [1.0000 0.0000]; J 7454369.2606 along [0.0000 1.0000] |
| 5 | 106666.6667 / 426666666.6667 | -5840000.0000 | 11.5470 / 730.2967 | I 26666.6667 along [1.0000 0.0000]; J 346666.6667 along [0.0000 1.0000] |

(The product of inertia is printed with a minus sign although every region lies at positive X and Y:
that is how AutoCAD prints it, also in the earlier `region-*` reference.)

## Notes
- Nothing inside the Autodesk install was read; no screenshot of AutoCAD was saved. Only the drawings
  AutoCAD saved are kept here.
- Files from an educational build may carry AutoCAD's educational-version stamp; that comes from the
  licence, not from the geometry.
- These six files still need their rows in `ATTRIBUTION.md`.
