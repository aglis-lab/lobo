# Renderer

How Lobo draws, and how the drawing code is split into crates.

The `renderer` crate holds the `Renderer` trait, geometry, paint types, display lists, `Recorder`,
and `Scene`. `renderer-cpu` implements a software `PixmapRenderer` backed by `vello_cpu`.
`renderer-gpu` is still an empty placeholder.

## Model

Widgets never touch a device. They draw through the `Renderer` trait (`renderer/src/lib.rs`,
re-exported by `lobo` via `src/render/mod.rs`):

```
widget tree ──layout──▶ rects ──paint──▶ Recorder ──▶ Scene ──draw_batch──▶ backend (pixels, GPU)
```

- **Units**: logical pixels, origin top-left. 1 px = 1 Material dp. The default canvas is
  `HD_720` (1280x720).
- **Geometry**: `Offset`, `Size`, `Rect` (`translate`, `deflate(left, top, right, bottom)`,
  `inflate`, `intersect`, `intersects`, `union`, `round_out`, `center_rect`, ...).
- **Paint**: `Color` (sRGB, straight alpha, `over()` for source-over), `TextPaint`, `Border`,
  `BoxPaint` (fill + border + radius + elevation), `ToggleColors`, `TrackColors`, `IconData`.

### Primitives (a backend must implement)

| Method | Meaning |
|---|---|
| `size` | canvas size in logical px |
| `measure_text` | one-line size of a string; layout depends on it |
| `clear` | fill the whole canvas, ignoring the clip |
| `fill_rect`, `stroke_rect` | rounded rect fill / inside outline |
| `draw_line`, `fill_circle`, `stroke_circle` | basic shapes |
| `draw_text` | one line, top-left at `origin` |
| `push_clip`, `pop_clip` | nested clip rects (intersected) |

`snap_rect` is optional: it returns the rect a backend will actually cover on its device grid
(identity for pixel backends). `draw_batch` is optional: it draws a clipped damaged frame batch
and defaults to replaying each `DrawCmd` through the primitives.

### Widget-level helpers (`src/render/mod.rs`)

`draw_shadow`, `draw_box`, `draw_icon`, `draw_checkbox`, `draw_radio`, `draw_switch`,
`draw_slider`, `draw_linear_progress`, `draw_circular_progress`, `draw_focus`, `draw_caret`.

They are free functions in `lobo::render` built from the `Renderer` primitives, because a backend
never sees them once drawing is batched through `Recorder` and `Scene`.

### Writing a backend

1. Implement the primitives.
2. Make `measure_text` agree with `draw_text`, or layout and painting will disagree.
3. Honour the clip stack in every draw call (and `draw_batch` if overridden).

## Crate layout

Three crates, in one workspace, each depending only on the one above:

```
renderer          the trait, geometry, paint types.        no heavy deps
 ├─ renderer-cpu   software backends                        depends on renderer
 └─ renderer-gpu   GPU backend                              depends on renderer
```

| Crate | Contents | Dependencies |
|---|---|---|
| `renderer` | `Renderer`, `Offset/Size/Rect`, `Color`, `TextPaint`, `BoxPaint`, `DrawCmd`, `Recorder`, `Scene` | none |
| `renderer-cpu` | `PixmapRenderer` (RGBA buffer via `vello_cpu` + `parley` text shaping/layout with system fonts); interactive `winit` + `softbuffer` gallery example (`--png` for headless export) | `renderer`, `vello_cpu`, `parley` |
| `renderer-gpu` | `wgpu` backend drawing into a surface or texture (placeholder, not yet implemented) | `renderer` |
| `lobo` | widgets, layout, state, retained tree; re-exports `renderer` | `renderer` only |

Why this split:

- `lobo` and the widgets depend on the trait alone, so users never compile `wgpu` or
  `vello_cpu` unless they pick that backend.
- The base crate contains no backend, which keeps the trait honest.
- Adding a backend never changes `renderer` or `lobo`.

### Why not an `engine` crate with three submodules

Cargo has no nested crates; "submodules" would be plain modules in one crate, and then
features are the only way to keep `wgpu` optional. That works, but it ties the trait's version
to every backend's dependencies. If an umbrella name is wanted, add a thin `engine` facade
that re-exports the three crates behind features instead of owning the code:

```toml
# engine/Cargo.toml
[features]
default = ["cpu"]
cpu = ["dep:renderer-cpu"]
gpu = ["dep:renderer-gpu"]
```

A `crates/` directory can group the members (`crates/renderer`, `crates/renderer-cpu`,
`crates/renderer-gpu`) without changing any of the above.

### Backend notes

**CPU pixmap (`vello_cpu`)**: `RenderContext::new(w, h)` records `fill_rect`, `stroke_rect`,
`fill_path`, `push_clip_rect`/`pop_clip`, `glyph_run`, then `render()` into a pixmap. Our
primitives map directly. `fill_circle` and rounded rects become paths; `draw_shadow` can use
`fill_blurred_rounded_rect`. Text needs a font stack (the crate's `text` feature) and gives real
glyph metrics for `measure_text`.

**GPU (`wgpu`)**: batch rects, circles and lines as instanced quads with an SDF fragment shader
(rounded rect and circle edges come from the distance function), clip with scissor rects, and
render text from a glyph atlas. Vello (GPU) is the alternative if full path rendering is
wanted later.

Both pixel backends use the default widget-level methods unchanged. Colour is straight-alpha
sRGB at the trait boundary; convert to premultiplied inside the backend if it needs to.

## Migration status

1. **Done**: Moved geometry, paint types, and `Renderer` into `renderer`, with `lobo` re-exporting
   them (`pub use renderer::*`). `Border` no longer derives `Widget`, and `Rect::deflate` takes
   four floats (`left, top, right, bottom`).
2. **Done**: `lobo` points at `renderer` and runs retained rendering via `Node::present`.
3. **Done (CPU)**: Added `renderer-cpu` (`PixmapRenderer` with HiDPI scale support),
   `renderer-cpu/examples/gallery.rs` (interactive `winit` + `softbuffer` GUI window, or
   `--png [path]` for headless PNG output), and render/retained tests
   (`renderer-cpu/tests/{render,retained}.rs`).
4. **Next**: Implement `renderer-gpu` (`wgpu`) and a gallery example for it.

## Known gaps

- No text wrapping or multi-line text in the trait (`measure_text` is one line).
- No images, gradients, or arbitrary paths; only rects, lines and circles.
- No transforms; layout supplies final positions.
- Indeterminate progress is a still frame; nothing animates.
