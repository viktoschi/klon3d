# Latex material profile for CLO (2026-10-18)

For the cross-check in CLO Standalone (earlier version). The values in part 2 rest on the same assumption as our own
solver (E 1.2 MPa, density 0.95 g/cm³). They have not been measured yet. Weight and tensile stiffness of latex sheeting are not in the knowledge
database (a search on 2026-10-18 found no matching hit). The numbers are therefore estimates.

## What the two forum posts say

- **"Trying to create a latex material type":** A user built 0.6 mm latex himself. The fabric gave way and slid
  down to the floor. The replies:
  - CLO computes with warp/weft fabrics, whereas latex is a uniform sheet. There is no ready-made preset,
    so you have to measure it yourself.
  - One person measures for the **Emulator** with a kitchen scale and a luggage scale as force gauge. For woven
    fabrics this is said to be 90–95% accurate.
  - CLO support recommends loading fabrics from the CLO-SET store, or taking a fabric with a similar drape and only
    switching its appearance to "Plastic".
- **"How to express latex?":** Appearance only. The starting fabric was `Trim_Full_Grain_Leather` from the library,
  Fabric Type "Plastic", stronger reflection, plus the lighting. Neither post gives any numbers.

For fit, the leather route does not help: leather is much stiffer than latex and barely stretches.

## 1. The right way in CLO: Emulator with your own measurements

CLO has the **Emulator mode** (changelog: Emulator/Fabric Kit, page for bias values, force in kgf,
category "None"). The Emulator guides you through the tests and computes a zfab from them, including nonlinear
stiffness. Instead of the expensive Fabric Kit, household tools will do. Use the latex sheet you cut from, i.e. the
same thickness and color.

| Measurement | How to do it | Expected at 0.25 / 0.40 / 0.60 mm |
|---|---|---|
| Thickness | caliper, 5 spots, average | nominal value |
| Weight | weigh a 10 × 10 cm piece, ×100 = g/m² | 238 / 380 / 570 g/m² |
| Stretch weft, warp, bias | strip 5 cm wide, 2 marks 20 cm apart. One end fixed, the luggage scale on the other. Read the force at 10%, 50% and 100% stretch (mark distance 22, 30, 40 cm) | 10%: 0.14 / 0.22 / 0.34 kgf<br>50%: 0.54 / 0.86 / 1.29 kgf<br>100%: 0.89 / 1.43 / 2.14 kgf |
| Bending | push a strip 2.5 cm wide flat over a table edge until the tip hangs down at 41.5° (Peirce). Measure the overhang | ≈ 1.9 / 2.6 / 3.5 cm |

Notes:
- Latex is almost the same in all directions. If warp and weft differ by more than 10%, the cause is usually the
  measurement (sheet slipped, stretched unevenly).
- The Emulator pages specify the exact strip size and force levels. If they differ from the table, the pages
  win. The expected values can be converted for any other strip size.
- Talc or silicone on the sheet distorts the friction, but not the stretch.
- This measurement also helps our own solver. Its assumption E 1.2 MPa then becomes a measured value instead of an estimate.

## 2. Without measurements: starting values in the Property Editor

Starting fabric: `Knit_Cotton_Jersey` or `Silk_Knit_Jersey` (soft and stretchy), not leather. Then in the Property Editor:

| Field | Setting for latex | Reason |
|---|---|---|
| Stretch-Weft / Stretch-Warp | set **equal**, jersey value as a start | a sheet has no grain direction |
| Shear | **raise** to roughly the same value as Stretch | A sheet resists shear (G = E/3). Woven and knit fabrics are soft here, which is why latex otherwise hangs skewed |
| Bending-Weft / -Warp | low, both equal; check against the bending test above (2.6 cm overhang at 0.4 mm) | latex drapes softly |
| Buckling Ratio / Stiffness | like Bending, no special buckling | no threads that could buckle |
| Internal Damping | medium | rubber springs back but should not jitter |
| Density | 380 g/m² (0.4 mm), 238 (0.25), 570 (0.6) | 0.95 g/cm³ × thickness |
| Thickness (Collision/Render) | sheet thickness, e.g. 0.4 mm | |
| Friction | high | latex grips the skin. With lubricant, rather lower. Compute both (as in the solver) |
| Shrinkage | 100% (off) | the reduction is already built into the pattern |

How CLO's slider values convert to N/m is not publicly documented. So use these values only as a
starting point and tune them in CLO against the bending test and a hanging strip (strip 5 × 20 cm,
fixed at one end, pull with 0.86 kgf → it should get 50% longer). It only becomes reliable with part 1.

## 3. Simulation

- Preset **Nonlinear** (`Preset\Simulation_Property\Nonlinear.smp`). Latex gets softer under stretch than a
  linear fabric, as the curve above shows (10% → 0.22 kgf, 100% → only 1.43 instead of 2.2 kgf).
- Fine particle distance, 5 mm or less at tight spots (armhole, cups).
- Why the fabric in the forum slid to the floor: Stretch was set far too soft. On top of that, friction is not
  enough at negative ease. In CLO a skin-tight garment stays on only through tension around the body. If stretching
  is too cheap, gravity pulls it down.

## 4. Appearance (independent of fit)

- Material Type **Plastic**, as CLO suggests
- strong reflection, low roughness (high-gloss polish), or higher for unpolished latex
- no woven normal map, single-color texture
- The HDR lighting creates the gloss; without good lights latex looks dull
