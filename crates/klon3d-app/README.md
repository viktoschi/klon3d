# klon3d-app – application window (stage 6)

Task: [#34](https://github.com/viktoschi/klon3d/issues/34) · claimed by `claude-app-1101` ·
branch `claude/vigilant-babbage-idsu9r` · base `ec76e25`

This file is the **handoff**: it says what is decided, what is done and which steps remain, so that any
session – Claude, another agent or a human – can continue without the chat history. Tick a step when it is
done and committed; update "Status" at the top when you stop.

## Status

**2026-11-01:** step 7 (review) in progress. A review of the whole crate found 25 defects (numbered
below as in the review); they are fixed in this order, one commit per group, each commit builds and
`cargo +1.99 test --locked -p klon3d-app` is green after it. Done so far: **groups A (loading),
B (drawing and panels) and C (file dialog)**. Next: groups D–E below, then tick step 7; after that
steps 8–11.

Steps 1–6 (2026-11-01): the window runs: `cargo +1.99 run --locked -p klon3d-app --
crates/klon3d-pattern/tests/muster/bluse3_v2.sm2d` opens it with the blouse (checked under Xvfb with
lavapipe: 28 pieces, 24 seams, status bar `GPU: llvmpipe (…) (Vulkan)` and `größte Abweichung N07L
Δ -0.0737 cm` as `klon3d naehte` prints). Files:

* `src/muster.rs` – pattern as plain data (outlines, fill as non-overlapping triangles by the
  even-odd rule in horizontal bands, layout, boundary edges, circumference `None` when the boundary
  cannot be resolved, seam report, errors as `Ladefehler` – `Massdatei` for errors another measurement file can
  cure, `Sonst` otherwise –, reader panics caught).
* `src/ansicht2d.rs` – view math as plain functions (`Sicht`, `treffer`, scale bar) and the painting
  (fills, then the selected piece as a 6 px band, then the seam strokes, then labels); refits on
  size changes until the user pans or zooms.
* `src/dateiwahl.rs` – in-app file dialog (confirm button "Datei öffnen"/"Datei wählen", not
  "Öffnen", which is the toolbar button). Nothing touches the disk per frame: drives read once per
  dialog (Windows: `GetLogicalDrives`, no drive is opened), only visible rows laid out
  (`show_rows`); on Windows hidden/system entries are left out like dot files.
* `src/app.rs` – `App` (`eframe::App`): every command is an `Aktion` run at the end of the frame
  (`App::ausfuehren`); shortcuts (Strg+O, Strg+Q, F, L, Tab/Umschalt+Tab, Esc – letters and Tab not
  while a text field has focus or a dialog is open); loading (`App::laden` makes paths absolute; a
  pattern that fails for its measurement file waits in `App::wartend`, and "Maßdatei wählen…" – menu,
  error-window button or a dropped `.smis` – applies to it); drag and drop (one drop = one request,
  `App::dateien_ablegen`); error and help windows; theme and last folder saved under
  `klon3d.thema` / `klon3d.ordner`.
* `src/menue.rs` – menu bar and toolbar; later entries disabled with hover "folgt in einer späteren
  Aufgabe"; entries that need a pattern disabled without one.
* `src/panele.rs` – search field, measurement file in use ("Maße: …"), piece list, seam list,
  properties panel, status bar (message cut with "…", full text on hover, a click shows an error
  again). Side panels are capped at 36 % / 24 % of the window (`app::SEITE_ANTEIL`) and scroll in
  both directions.
* `src/main.rs` – `klon3d-app [muster.sm2d]`, wgpu renderer, title "klon3d", min size 800×500.
* `tests/oberflaeche.rs` – pieces and seams against an independent `klon3d-pattern` evaluation
  (Δ formatted like the CLI), list click and view click select a piece and fill the properties,
  theme toggle + save/restore through an in-memory `eframe::Storage`, all five menus with their
  disabled entries, shortcuts incl. "no letters while the search field has focus", file dialog,
  drag and drop (also pattern + measurement file in one drop, both orders), a pattern without its
  measurement file loads after choosing one (menu and error window), relative paths stored
  absolute, "–" instead of a circumference where `klon3d naehte` prints none (measurement file with
  half the values), at 800×500 the 2D view keeps ≥ 300 px, a long status message stays inside the
  window and reopens the error window, missing files give messages, all 10 `.sm2d` below `klon3d-pattern/tests/muster` are
  loaded (see finding 20 for what that test asserts).

`Cargo.lock` change for the kittest `eframe` feature: one dependency edge (`egui_kittest` → `eframe`).

### Review findings (step 7)

Severity in brackets. Tick when fixed and committed; "won't fix" needs a reason here.

**Group A – loading** (`app.rs`, `muster.rs`, `menue.rs`, `panele.rs`, `tests/oberflaeche.rs`) – done:

- [x] 1, 6 [medium/high] a pattern whose measurement file is missing or not named could not get one:
      the failed request is kept (`App::wartend`), menu entry enabled for it (hover "für <name>"),
      error window offers "Maßdatei wählen…", typed error `Ladefehler`; a plain text file is
      reported as "kein Seamly2D-Muster mit Teilen", not as a missing measurement file
- [x] 2, 7 [low/medium] dropped files handled one by one: one drop is one request
- [x] 5, 25 [low] measurement file in use never shown: status "… geladen mit <smis>: …", left panel
      "Maße: <smis> · vom Muster genannt / von Hand gewählt" (hover: full path)
- [x] 11 [low] last folder stored relative: `Ladeauftrag::absolut` in `App::laden`
- [x] 12 [low] Prüfen › Nahtlängen and Ansicht › Einpassen enabled without a pattern; the seam-list
      scroll flag is also cleared on load

**Group B – drawing and panels** (`muster.rs`, `ansicht2d.rs`, `panele.rs`, `app.rs`) – done:

- [x] 3 [low] fill triangulation overlapped for self-overlapping outlines (bolero sleeves): the fill
      is built by the even-odd rule in horizontal bands (breaks at vertex heights and edge
      crossings), `muster::fuellung`; unit test over every piece of every test pattern on a raster
      against `ansicht2d::innen` (the old ear clipping fails it on the bolero sleeves)
- [x] 4 [low] circumference of a piece whose boundary cannot be resolved was shown unmarked:
      `umfang_cm: Option<f64>`, "–" in the error colour with the reason on hover
- [x] 8 [medium] at 800×500 the 2D view shrank to ~20 px: panels capped (`SEITE_ANTEIL`), lists
      scroll in both directions; left panel default 460 so the seam grid fits at 1400 px
- [x] 9 [medium] dark mode: selection outline hidden under seam strokes: 6 px band after the
      fills and before the seams, lighter selected fill (unit test: contrast ≥ 1.8 against the
      normal fill; the band itself checked by eye under Xvfb, no test)
- [x] 15 [low] long status messages cut off: truncated with "…", full text on hover, a click
      reopens the error window

**Group C – file dialog** (`dateiwahl.rs`) – done:

- [x] 10, 17 [medium] drive letters probed every frame on Windows: read once per dialog
      (`Dateiwahl::laufwerke`) from `GetLogicalDrives` (raw `extern "system"` like `klon3d-view`,
      no new dependency); unit test of the bit mask everywhere, of the system drive on Windows
- [x] 13 [low] long folders laid out completely every frame: `show_rows` (unit test: 3000 files,
      fewer than 100 rows laid out, counted through AccessKit); "Filter: aus (alle Dateien)" and
      "keine Einträge" with "alle Dateien"
- [x] 14 [low] Windows hidden/system entries listed: skipped by file attribute (unit test with
      `attrib +h`, runs on Windows only)

Windows-only code (`cfg(windows)`) checked with `cargo +1.99 check --locked -p klon3d-app
--all-targets --target x86_64-pc-windows-msvc` (compiles, no warnings); not run on Windows yet –
the Windows CI job (finding 16) is the first place that runs it.

**Group D – Windows build, CI, manifest** (`main.rs`, `Cargo.toml` of the crate, `ci.yml`) – open:

- [ ] 16 [medium] Windows CI job does not build or test klon3d-app
- [ ] 18 [low] release build on Windows opens a console window
- [ ] 19 [low] `rust-version = "1.95"` lower than what builds the crate (1.99)

**Group E – test honesty** (`tests/oberflaeche.rs`, `muster.rs` tests) – open:

- [ ] 20 [medium] `jedes_pruefmuster_laedt_oder_meldet` accepts any outcome: assert every pattern
      loads and its largest deviation equals an independent `pruefe_naehte`
- [ ] 21 [low] precondition "Randkanten (22)" can never fail
- [ ] 22 [low] "clicking disabled Start changes nothing" can never fail: check the hover text instead
- [ ] 23 [low] drop test does not check why a file is refused
- [ ] 24 [low] `etikett_liegt_im_bogen` asserts only "not in the gap", not "inside the piece"

Notes: egui's default fonts have no arrows (→ ← ↑ ↓), check marks or emoji; use `›`, `–`, `Δ`, `≤`,
`·`, `×`, `…` only (kittest panics on a missing glyph). `egui::Id::new` is deprecated in 0.37
(`Id::unique`); panels are `egui::Panel::{top,bottom,left,right}(..).show(ui, ..)`. kittest's
harness calls `ctx.set_theme(Dark)` *after* the app is built, so the app applies its stored theme in
`ui()`, not only in `App::neu`. Dropping an `egui::FullOutput` with texture deltas panics in debug
builds (`out.textures_delta.clear()`). kittest finds `Label`s by their AccessKit *value*; grid rows are
siblings under one parent (see `zeile_nach` in the UI test). A click on a disabled menu entry closes
the menu (egui), nothing else.

## Goal (from #34)

First slice of stage 6 in PLAN.md: an `eframe` window with the `wgpu` renderer and the layout the user asked
for on 2026-10-20 – **dark design** (switchable to light, saved), **menus**, **buttons** – working on the
2D pattern. 3D view, simulation, material, OBJ/DXF export and the maps are later tasks; their menu entries
exist but are disabled.

## Decisions

| Point | Decision | Why |
|---|---|---|
| Framework | `eframe` 0.37 with `wgpu_no_default_features` + our own `wgpu` 30 dependency (same features as `klon3d-sim`) | egui-wgpu 0.37 uses wgpu 30, the same as the solver – later the simulation can share the device (ADR-018); no second wgpu in `Cargo.lock` |
| eframe features | `accesskit`, `default_fonts`, `persistence`, `wgpu_no_default_features`, `x11`, `wayland` | `persistence` saves the theme; `accesskit` lets tests find widgets by label |
| Rust | the crate says `rust-version = "1.95"` (eframe's minimum); build with **1.99** like the main machine | the workspace itself no longer builds with 1.97 (`geom.rs:779`, elided lifetime in a `const`) |
| File dialog | own small dialog inside egui (directory list, filter `.sm2d`/`.smis`/`.naehte`), plus drag and drop and a command-line argument | no `rfd` – on Linux it pulls GTK or a D-Bus portal stack |
| Tests | `egui_kittest` 0.37 (dev-dependency) drives the UI headless by labels; no GPU needed | screenshots only where a GPU adapter exists, otherwise skipped (AGENTS.md: tests without data skip) |
| Language | UI text German, identifiers German like the rest of the code, **comments English** (AGENTS.md) | |
| Not touched | `PLAN.md` (held by #33), `klon3d-view` (stays the Win32 diagnostic tool) | |

## Planned layout of the crate

| File | Content |
|---|---|
| `src/main.rs` | command line (`klon3d-app [muster.sm2d]`), `NativeOptions` (wgpu renderer, window size, app id), `run_native` |
| `src/lib.rs` | modules, so that tests can build the app |
| `src/app.rs` | `struct App` implementing `eframe::App` (`fn ui(&mut self, ui, frame)`; `fn save` for persistence): menu bar, toolbar, side panels, status bar, keyboard shortcuts, theme |
| `src/muster.rs` | loading without UI: `.sm2d` → `Auswertung` → piece outlines (`ev.stueck_umriss`), circumference (`ev.umriss_laenge`), boundary edges (`ev.teilrand(stueck, Kurvenquelle::Seamly)`), seam report (`naehte::lies_nahtliste` + `pruefe_naehte`, the same numbers as `klon3d naehte`); measurement file as the pattern names it (see `klon3d-view/src/main.rs`, `Blatt::laden`) or chosen by hand |
| `src/ansicht2d.rs` | 2D view: pieces laid out side by side (as `klon3d-view` `auslegen`), pan, zoom around the cursor, fit, hit test, selection highlight, scale bar |
| `src/dateiwahl.rs` | the in-app file dialog |
| `tests/oberflaeche.rs` | kittest: load `crates/klon3d-pattern/tests/muster/bluse3_v2.sm2d`, piece list, seam list Δ equals `pruefe_naehte`, selection fills the properties, theme toggles and survives save/restore, menus exist |
| `examples/bildschirmfoto.rs` | renders the window with kittest's wgpu renderer to PNG (dark and light); skips without an adapter |

Menu bar (from PLAN.md): **Datei** (Muster öffnen, Maßdatei, Nahtliste, Körper, Export OBJ/DXF, Beenden) ·
**Simulation** (Start, Pause, Einzelschritt, Zurücksetzen, Material) · **Ansicht** (2D-Muster, 3D, Netz,
Nähte, Dehnungs- und Abstandskarte, hell/dunkel) · **Prüfen** (Nahtlängen, Netzprüfung) · **Hilfe**.
Status bar: frames per second, GPU adapter and backend (`frame.wgpu_render_state()` → adapter info), largest
seam deviation, last message.

## Steps

- [x] 0. Issue #34 created, claimed, no overlap with #33 or open PRs; skeleton committed
- [x] 1. Build the skeleton: `cargo build --locked -p klon3d-app --all-targets` (see environment below)
- [x] 2. `muster.rs` with unit tests (Bluse3 v2: pieces, seams, Δ equal to `pruefe_naehte`)
- [x] 3. `ansicht2d.rs` (view transform with tests for fit/zoom/hit test)
- [x] 4. `dateiwahl.rs`
- [x] 5. `app.rs` + `main.rs`: menus, toolbar, panels, status bar, shortcuts, theme with persistence
- [x] 6. `tests/oberflaeche.rs` (kittest) green; `cargo test --locked -p klon3d-app`
- [ ] 7. Review of the whole crate (correctness, UI against PLAN.md, Windows portability, test honesty), fixes
- [ ] 8. Windows: `cargo check --locked -p klon3d-app --target x86_64-pc-windows-msvc` (cross check, no linker
      needed); add `cargo build --locked -p klon3d-app` to the Windows job in `.github/workflows/ci.yml`
- [ ] 9. Screenshots dark/light with the blouse loaded (environment below), attach to the PR
- [ ] 10. Full workspace check: `cargo build --locked --workspace --exclude klon3d-view --all-targets` and
      `cargo test --locked --workspace --exclude klon3d-view`
- [ ] 11. `git fetch origin && git rebase origin/main`, push, pull request with the template
      (`.github/pull_request_template.md`: Change / Evidence / Not checked / Limits and provenance),
      `Closes #34`; then on #34: label `review` instead of `in-progress` and a done comment
      (`tools/task.sh done 34 <session> <pr>` does both where `gh` is available)

## Environment

**Cloud container (Linux):**

```sh
rustup toolchain install 1.99 --profile minimal      # the default 1.97 is too old for the workspace
cargo +1.99 build --locked -p klon3d-app --all-targets
# software Vulkan (lavapipe) and EGL, for screenshots and GPU tests without a graphics card
apt-get update && apt-get install -y --no-install-recommends mesa-vulkan-drivers libegl1 libegl-mesa0
# a real window: Xvfb + ImageMagick (both preinstalled in the cloud image used on 2026-11-01);
# winit's X11 backend also needs libxkbcommon-x11 (not preinstalled there)
apt-get install -y --no-install-recommends libxkbcommon-x11-0
xvfb-run -s "-screen 0 1600x1000x24" sh -c 'target/debug/klon3d-app crates/klon3d-pattern/tests/muster/bluse3_v2.sm2d & sleep 8; import -window root bild.png'
```

**Main machine (Windows):** cargo path as in AGENTS.md; `cargo run --locked -p klon3d-app -- <muster.sm2d>`.

## Handing over

Stopping before the end: tick the steps above, write the status, commit and push, then leave the handoff
note on #34 in the format of CONTRIBUTING.md and release the task (`tools/task.sh release 34 <session>
"<note>"`, or by hand: a comment starting with `**Released** by \`<session>\``, label `ready` instead of
`in-progress`). The next session claims #34 again and continues on this branch.
