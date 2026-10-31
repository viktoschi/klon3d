# Feature parity CLO → Rust

Yardstick: the **96 native methods** of the CLO API that ships with CLO
(`CLOAPIInterface.dll`, see finding 05) plus the
96 Python API methods from `MarvelousDesigner.py`
(finding 02).

**Status**

| Code | Meaning |
|---|---|
| `n/a` | not planned — deliberately excluded |
| `open` | planned, not started yet |
| `plan` | in the plan, order in `PLAN.md` |
| `partial` | partly available |
| `done` | verified |

---

## A. Session and files

| CLO function | CLO API | Rust equivalent | Status |
|---|---|---|---|
| New avatar object | `NewAvatar` | `klon3d-body` | open |
| New garment object | `NewGarment` | not needed — load a file | n/a |
| Open object | `OpenFile` | `klon3d-pattern` (`.sm2d`) | **plan** (stage 1) |
| Save | `SaveFile` | `klon3d-pattern`, writing | open |
| Project file | `SetSaveZprjFilePath` | not needed | n/a |
| Load measurement file | `LoadZfabFile` | `.smis` reader | **plan** (stage 2) |
| Simulation running? | `IsFinishProcess` | solver state | plan |
| Screen capture | `CaptureScreen` | viewer screenshot | open |

## B. Simulation

| CLO function | CLO API | Rust equivalent | Status |
|---|---|---|---|
| Start/stop simulation | `SimulationOn` / `SimulationOff` | `klon3d-sim` | **plan** (stage 4) |
| Simulation quality | `SetSimulationQuality` | solver profile | plan |
| Time step | `SetSimulationTimeStep` | solver profile | plan |
| CG iterations / residual | `SetSimulationCGIterationCount` / `CGResidual` | **replaced** by XPBD substeps | n/a (deliberate) |
| Nonlinear solver | `SetSimulationNonlinearSimulation` | XPBD is always nonlinear | n/a |
| Gravity | `SetSimulationGravity` | −9.81 m/s², profile | plan |
| Air damping | `SetSimulationAirDamping` | material profile | plan |
| Self-collision (V↔T, E↔E) | `SetSimulationSelfCollisionDetection_*` | proximity springs, intersection resolution, impulses as in CLO (finding 116), spatial hashing | **yes** (`--selbstkollision`, CPU) |
| Proximity distance / stiffness | `…_AvoidanceStiffness` | `selbst_steifigkeit` 0.001, `selbst_zusatzdicke` 2.5 mm (finding 116) | **yes** |
| Ground collision | `SetSimulationGround_Collision` | collider | plan |
| Avatar collision | `SetSimulationAvatarClothCollisionDetection_*` | body mesh | **plan** |
| Intersection resolution | `SetSimulationIntersectionResolution` | XPBD CCD | plan |
| Layer-based collision | `SetSimulationLayerBasedCollisionDetection` | layers in the material model | open |
| Temporary pinning | `CreateTemporaryPin` / `RemoveAllPin` | pin groups | open |
| Particle distance | `SetParticleDistance` | mesh density | **plan** (stage 3) |

### Wind

| CLO function | CLO API | Rust | Status |
|---|---|---|---|
| Wind on/off | `SetWindControllerWind_Activate` | wind field | plan |
| Wind type, strength, decay | `…Wind_Type` / `_Strength` / `_Decay` | noise field | plan |
| Frequency, turbulence | `…Wind_Frequency` / `_Turbulence` | noise field | plan |
| Wind direction | `…Translate_X/Y/Z` | vector | plan |

## C. Material

| CLO function | CLO API | Rust | Status |
|---|---|---|---|
| Load fabric file | `LoadZfabFile` | material profile from `.sm2d` variables | plan |
| Material thickness | — | profile (0.4 / 1.0 mm) | plan |
| Stretch / stiffness | `SetStrengthen` | profile | plan |
| Color swatch | `AddColorSwatch` | renderer | n/a |

## D. 3D and display

| CLO function | CLO API | Rust | Status |
|---|---|---|---|
| Front camera | `FrontCamera` | viewer camera | open |
| Gizmo manipulators | `3D_Gizmo_Scale_*` | viewer | n/a |
| Screenshot | `CaptureScreen` | viewer | open |

## E. Export

| CLO function | CLO API | Rust | Status |
|---|---|---|---|
| Scale | `SetScale` | unit conversion | **done** (formula engine) |
| Frame rate | `SetFps` | export parameter | open |
| Single/multi-object export | `SetSingleObject` | export parameter | open |
| Welding | `SetWeld` | export parameter | open |
| Keep thickness | `SetThin` | material profile | plan |
| Unified UV | `SetUnifiedUVMap` | texture bake | open |
| Texture bake | `SetTextureBake*` | renderer | n/a |
| Axes / mirroring | `SetAxisX/Y/Z`, `SetInvertX/Y/Z` | export parameter | open |
| Inner shape | `SetIncludeInnerShape` | volume constraint | plan |
| **glTF export** | `ExportGLTF` | `gltf` crate | open |
| **TechPack export** | `ExportTechPackToStream` | own exporter | open |
| OBJ | — | `klon3d-mesh` | plan |

### Inclusion flags (11)

`Pattern` `PipingPattern` `ZipperTeethPattern` `Avatar` `Trim` `Graphics`
`TopStitch` `ZipperObj` `ButtonHole` `ButtonHead` `OBJStitch`

CLO can export individual extra objects along with the garment. For the simulator they are
**not a goal** — they belong to documentation, not to simulation.
Status `n/a` throughout.

## F. Plugin interface

| CLO function | CLO API | Rust | Status |
|---|---|---|---|
| Get/set API binding | `Get*API` / `Set*API` | not needed | n/a |
| Set metadata | `SetAPIMetaData` | JSON export | open |
| Load plugin | `CloLibraryAPI_Plugin.dll` | not needed | n/a |

## G. Not adopted

| CLO area | Reason |
|---|---|
| **V-Ray rendering** | proprietary, cannot be rebuilt |
| FBX rigging / characters | the existing body is sufficient |
| Sewing with thread, embroidery | not needed for fit checking |
| Layout / nesting | plotting is already handled by Seamly2D |
| Drawing pattern sheets | Seamly2D does that |
| Live simulation while drafting | a design-tool feature, not verification |
| Multi-user / cloud / sharing | explicitly "local" |
| Capturing measurements with an app | the `.smis` file exists |
| Measurements directly from the Android app | outside the scope of the tool |

---

## Summary

| Area | CLO scope | Rust scope |
|---|---|---|
| Simulation | complete | core, XPBD instead of CG |
| Collision | 11 configurations | the 6 relevant for latex |
| Material | fabric files | material profiles |
| Export | 14 routes | OBJ, glTF, TechPack |
| 2D patterns | full CAD | reading + seam allowance check |
| Rendering | V-Ray | simple viewer |

**This is not a clone.** It is a tool for a clearly defined purpose:
**detect fit problems in latex patterns before the material is cut.**

The test case for this is fixed: test blouse v26 with its five
known faults. It decides whether the functionality is sufficient —
not the number of implemented rows.
