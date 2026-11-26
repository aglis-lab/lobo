# Renderer

How Lobo draws, and how the drawing code is split into crates.

The terminal backend has been removed; there is currently no backend that can draw.
Today everything lives in `src/render/` inside the `lobo` crate. The empty `renderer/` crate is
the start of the split described in [Crate layout](#crate-layout-proposed).

## Model

Widgets never touch a device. They draw through the `Renderer` trait (`src/render/mod.rs`):

```
widget tree ──layout──▶ rects ──paint──▶ &mut dyn Renderer ──▶ backend (pixels, GPU)
```

- **Units**: logical pixels, origin top-left. 1 px = 1 Material dp. The default canvas is
  `HD_720` (1280x720).
- **Geometry**: `Offset`, `Size`, `Rect` (`translate`, `deflate`, `inflate`, `intersect`,
  `center_rect`, ...).
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
(identity for pixel backends).

### Widget-level methods (defaulted)

`draw_shadow`, `draw_box`, `draw_icon`, `draw_checkbox`, `draw_radio`, `draw_switch`,
`draw_slider`, `draw_linear_progress`, `draw_circular_progress`, `draw_focus`, `draw_caret`.

They are built from the primitives, so a pixel backend gets correct Material widgets for free.
A backend can override them where the default looks wrong on its device.

### Writing a backend

1. Implement the primitives.
2. Make `measure_text` agree with `draw_text`, or layout and painting will disagree.
3. Honour the clip stack in every draw call.
4. Override a widget-level method only where the default looks wrong on your device.

## Crate layout (proposed)

Three crates, in one workspace, each depending only on the one above:

```
renderer          the trait, geometry, paint types.        no heavy deps
 ├─ renderer-cpu   software backends                        depends on renderer
 └─ renderer-gpu   GPU backend                              depends on renderer
```

| Crate | Contents | Dependencies |
|---|---|---|
| `renderer` | `Renderer`, `Offset/Size/Rect`, `Color`, `TextPaint`, `BoxPaint`, ... | none |
| `renderer-cpu` | `pixmap` (RGBA buffer via [`vello_cpu`](https://github.com/linebender/vello)) | `renderer`, optional `vello_cpu` |
| `renderer-gpu` | `wgpu` backend drawing into a surface or texture | `renderer`, `wgpu` |
| `lobo` | widgets, layout, state; re-exports `renderer` | `renderer` only |

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

## Migration steps

1. Move geometry, paint types and the `Renderer` trait from `src/render/mod.rs` into `renderer`.
   `lobo` re-exports them (`pub use renderer::*`) so existing imports keep working.
   Note `Border` derives `Widget` and `Rect::deflate` takes `lobo::Padding`: both must move
   out of the trait's path (own a `Padding` in `renderer`, or make `deflate` take four floats).
2. Point `lobo` at `renderer`.
3. Add `renderer-cpu` (`pixmap`), then `renderer-gpu`, each with a gallery-style example.
   The gallery example and the render tests were deleted with the terminal backend; bring
   them back on the first pixel backend.

## Known gaps

- No text wrapping or multi-line text in the trait (`measure_text` is one line).
- No images, gradients, or arbitrary paths; only rects, lines and circles.
- No transforms; layout supplies final positions.
- Indeterminate progress is a still frame; nothing animates.
