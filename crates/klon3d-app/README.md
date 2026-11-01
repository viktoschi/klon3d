# klon3d-app – application window (stage 6)

Task: [#34](https://github.com/viktoschi/klon3d/issues/34) · claimed by `claude-app-1101` ·
branch `claude/vigilant-babbage-idsu9r` · base `ec76e25`

This file is the **handoff**: it says what is decided, what is done and which steps remain, so that any
session – Claude, another agent or a human – can continue without the chat history. Tick a step when it is
done and committed; update "Status" at the top when you stop.

## Status

**2026-11-01:** steps 1–3 done. `src/muster.rs` loads a pattern into plain data (outlines, ear-clipping
triangulation for the fill, layout, boundary edges, seam report with the numbers of `klon3d naehte`,
errors as `Err(String)`, reader panics caught). `src/ansicht2d.rs` has the view math as plain functions
(`Sicht`: world↔screen, fit, zoom about a point, pan; `treffer` point-in-polygon hit test; scale bar
1-2-5) and the egui painting (filled pieces, labels, seam edges coloured by `Kantenart`, selected seam,
scale bar). 18 unit tests green with `cargo +1.99 test --locked -p klon3d-app`. `src/main.rs` is still
empty. Next: step 4, `src/dateiwahl.rs`.

Note: egui's default fonts have no arrows (→ ← ↑ ↓), check marks or emoji; use `›`, `–`, `Δ`, `≤`,
`·`, `×`, `…` only (kittest panics on a missing glyph).

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
- [ ] 4. `dateiwahl.rs`
- [ ] 5. `app.rs` + `main.rs`: menus, toolbar, panels, status bar, shortcuts, theme with persistence
- [ ] 6. `tests/oberflaeche.rs` (kittest) green; `cargo test --locked -p klon3d-app`
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
# a real window: Xvfb + ImageMagick (both preinstalled in the cloud image used on 2026-11-01)
xvfb-run -s "-screen 0 1600x1000x24" sh -c 'target/debug/klon3d-app crates/klon3d-pattern/tests/muster/bluse3_v2.sm2d & sleep 8; import -window root bild.png'
```

**Main machine (Windows):** cargo path as in AGENTS.md; `cargo run --locked -p klon3d-app -- <muster.sm2d>`.

## Handing over

Stopping before the end: tick the steps above, write the status, commit and push, then leave the handoff
note on #34 in the format of CONTRIBUTING.md and release the task (`tools/task.sh release 34 <session>
"<note>"`, or by hand: a comment starting with `**Released** by \`<session>\``, label `ready` instead of
`in-progress`). The next session claims #34 again and continues on this branch.
