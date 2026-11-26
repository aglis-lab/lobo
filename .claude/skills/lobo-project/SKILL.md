---
name: lobo-project
description: >-
  Project map and working rules for the lobo repository: crate layout, how the
  declarative tree, signals, layout and rendering fit together, commands to
  build and test, code style, and where the design docs are. Use at the start of
  any task in this repo, before touching widgets, the tree, signals, layout, the
  renderer crates or docs.
metadata:
  author: project-maintainers
  category: Engineering
  version: "1.1.0"
---

# Lobo project guide

Lobo is a Rust, Flutter-style UI library: declarative widgets, signals for state, a retained
tree, Material widgets. Read this first, then `keep-it-simple` before designing anything,
and `review` when asked to review a diff, doc or plan.

## Status (check it, it moves)

- `renderer` holds the `Renderer` trait, geometry, paint types, `DrawCmd`, `Recorder`, and `Scene`.
- Retained rendering (display lists, damage, one batch per frame via `Node::present`) is
  implemented in `renderer` and `lobo`; `lobo::run(app, backend)` takes `&mut dyn Renderer`.
- `renderer-cpu` implements `PixmapRenderer` backed by `vello_cpu` (with HiDPI scaling, a
  built-in 5×7 glyph renderer, and an interactive `winit` + `softbuffer` `gallery` example;
  pass `--png` for headless export). `renderer-gpu` is still an empty placeholder.
- Confirm with `git log`, `git status` and the docs before assuming any of the above.

## Map

| Path | What |
| --- | --- |
| `src/widget.rs` | `Widget`, the `Element` trait, `Text`/`Container`/`Column`, `Padding` (data), `InputEvent`, `global_events` |
| `src/basic.rs` | layout widgets (Flutter's `widgets` library): rows, stacks, alignment, flex, icons, lists |
| `src/theme.rs` | Material 3 theme: colour scheme, type scale, inherited text style |
| `src/tree.rs` | `Node`: retained tree, rebuild on dirty signal, reconcile children in place, `Repaint`, `present` |
| `src/signal.rs` | `Signal`, `Observer`, `track`; thread-local, single-threaded |
| `src/layout.rs` | `Constraints`, `LayoutCx`, `PaintCx` (with `Recorder`), `focus` (Tab order) |
| `src/render/mod.rs` | re-exports `renderer::*` and defines widget-level `draw_*` free functions |
| `src/context.rs` | `Context`, `run(app, backend)` loop, key dispatch |
| `src/task.rs`, `src/provider.rs` | async tasks on the UI thread (Tokio), `Provider` |
| `src/term.rs` | raw terminal input (crossterm), not drawing |
| `src/material/` | Material widgets by group (actions, selection, navigation, ...) |
| `lobo-macros/` | `ui!`, `#[derive(Widget)]`, `#[derive(Model)]` |
| `renderer/` | `Renderer` trait, geometry, paint types, `DrawCmd`, `Recorder`, `Scene` |
| `renderer-cpu/` | `PixmapRenderer` software RGBA backend via `vello_cpu`, plus interactive `gallery` GUI example |
| `renderer-gpu/` | GPU backend crate, currently empty |
| `docs/` | `renderer.md`, `retained-rendering.md`, `material-widgets.md`, `tickets.md` |

## How it works, in five lines

1. A widget is a struct with `#[derive(Widget)]` plus `impl Element` (`build`, optionally
   `layout`, `paint`, `attach`, `dispose`, `scope`, `parent_data`).
2. `ui!(Container { child: Text { text: "hi" } })` builds the tree of widget values.
3. A `Node` mounts a widget, calls `build`, and reads signals under an `Observer`.
4. A signal `set` that changes a value marks its readers dirty and wakes the loop; only those
   nodes rebuild, and children are matched by key, else by position and type.
5. Layout is Flutter-style (`Constraints` down, `Size` up, parent places children); `paint`
   draws through `&mut dyn Renderer` in logical pixels (1 px = 1 dp).

State the **user** interacts with is a `Signal`. Keep widget code single-threaded; background
work goes through the task API (`run_async`, `spawn`, `Latest`/`Ticket`, see `docs/tickets.md`).

## Commands

```
cargo fmt
cargo clippy --workspace --all-targets
cargo test --workspace
```

Run these for what you touched. Report real output; if a check fails, say so.

## Style

- `rustfmt.toml` rules: 2-space indent, `max_width = 100`, `use_small_heuristics = "Max"`.
  Run `cargo fmt`; do not hand-format. A `ui!(...)` call with a line over 100 columns is
  silently skipped by rustfmt, so keep those lines short.
- Edition 2024. `pub(crate)` by default; widen only for a real consumer.
- Comments explain why, not what. Doc comments (`///`) on public items, one or two lines.
- Docs in `docs/` are plain Markdown, short, with the decision first.
- **File size (Rust).** Each `.rs` file stays under 1,000 lines. Count with `wc -l` (blank
  lines and comments included). At 1,000, split it: move one related group of items into a
  sibling file (`src/tree.rs` declares `mod reconcile;`, body in `src/tree/reconcile.rs`) or a
  new module directory. Split along an existing seam, one job per file; never `part1.rs` /
  `part2.rs`.

## Testing

- Integration tests live in `tests/*.rs` and use only the public API (`use lobo::*;`).
- `global_events`, focus and signals are **thread-local**: call `global_events.clear()` at the
  start of a test that registers listeners, and don't share state across tests.
- No backend exists, so layout and focus tests use a small no-op `Renderer` (see
  `tests/reconcile.rs`). When a backend exists, prefer a spy `Renderer` that records calls.
- Test behaviour a library user sees. A test with more setup than assertions is a smell.
- **Unit tests stay under 500 lines.** An inline `#[cfg(test)] mod tests` that reaches 500
  moves to its own file: `src/tree.rs` keeps `#[cfg(test)] mod tests;` and the body goes in
  `src/tree/tests.rs`.
- **Test files stay under 1,000 lines.** At 1,000, split by behaviour into several files:
  `src/tree/tests.rs` declares `mod keys; mod layout;`, with bodies in
  `src/tree/tests/keys.rs` and `src/tree/tests/layout.rs`. The same 1,000-line limit applies
  to `tests/*.rs`; split those into more top-level files, since each one is its own crate.

## Rules for AI work here

- Read the file you are changing and its neighbours first; match their names and patterns.
- Keep `lobo` declarative: no backend names, damage tracking or batching in it.
- Ask before deleting a feature, a test or a doc, or before moving code between crates.
  Destructive scope questions are cheap; wrong deletions are not.
- Never publish or commit unless asked. Never put the maintainer's email or other personal
  data in files, commits or external requests.
- Licence is MIT (`LICENSE`); manifests say `license = "MIT"`. Keep them in sync.
- When a design question comes up, check `docs/` before inventing; update the doc when a
  decision changes, in the same change.
- Finish by saying what changed, where, how it was verified, and what you chose not to build.
