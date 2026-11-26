# Retained rendering

Design for drawing only what changed, in batches, without putting any of that in `lobo`.
Status: **implemented** in `renderer`, `lobo`, and `renderer-cpu` (`renderer-gpu` remains empty).
See [renderer.md](renderer.md) for the `Renderer` trait this builds on.

## Problem

The declarative layer is already retained: `Node`s persist, and a signal change rebuilds only
the nodes that read it. Drawing is not. `render_frame` lays out the whole tree and paints every
widget straight into the backend on every change. That is immediate mode behind a retained tree.

Goals:

1. **Redraw only the changed region.** A blinking caret must not repaint the screen.
2. **Not immediate mode.** `paint` must not talk to the device.
3. **Batch.** One submission per frame, not one call per primitive.
4. **Keep `lobo` declarative.** It only knows the `renderer` crate: no damage, batching or
   backend code, and no knowledge that `renderer-cpu` or `renderer-gpu` exist.
5. Existing widgets' `paint` stays unchanged.

## Crate responsibilities

```
lobo           declarative tree, signals, layout, focus, widgets
  │  depends on
  ▼
renderer       Renderer trait, geometry, paint types, DrawCmd, Recorder, Scene
  ▲  depend on                       ▲
  │                                  │
renderer-cpu  (vello_cpu)      renderer-gpu  (wgpu)
```

| Crate | Owns | Does not know |
|---|---|---|
| `renderer` | everything about *what to draw and when*: display lists, layers, diffing, damage, clipped batching, the frame protocol | `lobo`, widgets, signals, any concrete backend |
| `lobo` | *what the UI is*: build, layout, focus, the recording hook | damage, batching, pixels, backends |
| `renderer-cpu`, `renderer-gpu` | *how to put pixels down* | `lobo`, how damage is computed |

The application picks a backend and passes it to `lobo`'s run function through the `Renderer`
trait. `lobo` never names a backend type.

Backends come from crates.io (`vello_cpu`, `wgpu`). If a change to either is ever needed, use a
fork and `[patch.crates-io]` in the workspace manifest until it is released upstream; no git
submodule.

## The scene (in `renderer`)

`lobo` describes drawing as a tree of **layers**. A layer is the retained form of one widget's
own painting:

```rust
struct LayerId(u64);               // stable for the life of a node; lobo allocates it

scene.set(id, list);               // its own commands, with child / overlay slots
scene.remove(id);                  // widget unmounted
```

The scene stores, per layer, the last `list` and the union of its commands' bounds. It never
sees widgets.

### Display list

```rust
enum DrawCmd {
  Clear(Color),
  FillRect { rect, color, radius },
  StrokeRect { rect, border, radius },
  Line { from, to, color, width },
  FillCircle { center, radius, color },
  StrokeCircle { center, radius, border },
  Text { origin, text: Arc<str>, paint },
  PushClip(Rect),
  PopClip,
}

enum Item {
  Cmd(DrawCmd, Rect /* bounds */),
  Child(LayerId),                  // child's own list goes here
  Overlay(LayerId),                // child's list goes after the whole tree
}
type DisplayList = Vec<Item>;
```

`DrawCmd` mirrors the `Renderer` primitives, is `PartialEq`, and is cheap to clone. Coordinates
are absolute canvas pixels, as widgets already use. Each command carries `bounds`: the canvas
area it can change, cut by the clips active inside the same recording and grown 1 px for
anti-aliasing (text from `measure_text`). A fully clipped command is dropped.

A child is recorded on its own, so its bounds are not cut by a clip its parent pushed around
the `Child` slot (a scroll view). Damage can then reach past the visible area. That only
over-draws: the replay in `present` still applies the parent's clip.

A `Recorder` is a `Renderer` that builds a `DisplayList`. Text measuring and canvas size are
forwarded to the real backend, so widgets lay out and paint as they would against it.

The widget-level helpers (`draw_checkbox`, `draw_switch`, ...) are built from primitives, and
in the retained path a backend never sees them, so overriding them does nothing. They stop
being `Renderer` methods and become free functions in `lobo` (they are Material drawing, not
backend work). `renderer`'s trait keeps only the primitives, `size`, `measure_text`,
`snap_rect` and `draw_batch`.

### Frame

```rust
let mut scene = Scene::new();
let damage: Option<Rect> = scene.present(&mut backend);   // None: nothing changed, nothing sent
```

`present` does:

1. **Diff.** For every layer set since the last frame, compare the new list with the old
   (`PartialEq`). Only if they differ is `old.bounds ∪ new.bounds` added to the damage. A
   rebuild that paints the same pixels costs nothing.
2. **Removed / unlinked layers.** `remove(id)` damages the layer's old bounds and drops it.
   A layer the parent's new list no longer links (closed menu, hidden page) damages its old
   bounds but **keeps its list**: its node is still mounted and won't re-record when it is
   linked again, so re-linking damages those bounds again and draws the kept list.
3. **Whole canvas** is damaged on the first frame and after a resize. A background colour
   change needs no special case: the root's `Clear` has the whole canvas as bounds, so the
   root list's diff damages everything.
4. If there is no damage, **stop**: nothing reaches the backend.
5. **Flatten** the layers from the root (expanding `Child`, then overlays in order), keep the
   commands whose bounds intersect the damage, and wrap them in `PushClip(damage)` /
   `PopClip`. `PushClip` / `PopClip` are always kept, whatever their bounds, so the clip stack
   stays balanced; an empty pair is removed. A `Clear` becomes `FillRect(damage)`.
6. Send them in one call: `backend.draw_batch(damage, &cmds)`.

Replaying every command that touches the damage (background, siblings below, siblings above),
clipped to it, gives the same pixels as a full redraw inside it, so a partial frame is exact.

### Damage

One rect: the union of everything that changed, grown outward to whole pixels. Two distant
changes (a clock and a caret) repaint the box around both. That over-draws, but it is one
rect, one clip and one batch. A list of rects is a change inside `present` only, if a real
backend shows the over-draw matters; nothing outside `renderer` would change.

### Backend contract (one defaulted `Renderer` method)

```rust
fn draw_batch(&mut self, damage: Rect, cmds: &[DrawCmd]) {
  for c in cmds { c.apply(self) }
}
```

- The surface **keeps its pixels between frames**; only `damage` is redrawn.
- `Clear` is never sent mid-frame (it ignores the clip); the background arrives as `FillRect`.
- The background must be opaque, or it would compound over the old pixels.
- The method returns when the frame is done, so a backend flushes / presents inside it.
- `renderer-cpu` can present only `damage`. `renderer-gpu` overrides it to build one vertex
  buffer (instanced SDF quads, glyph atlas) and uses the clip as a scissor.

## What `lobo` does

`lobo` keeps the layout and focus logic it has today and adds three small things.

### 1. Explicit repaint, no new observers

A node keeps its single observer. A signal it reads (in `build`, `layout` or `paint`) still
rebuilds it. That is slightly wasteful for a leaf that only reads a signal in `paint`, but the
display-list diff makes the *redraw* exact, so the cost is only a rebuild. Splitting observers
by phase can come later, if measured.

Signals are state the **user** interacts with. State the **system** owns (hover, press,
focus ring, caret blink, animation frame) is not a signal, so a widget asks for a repaint
explicitly:

```rust
// in Element::attach, keep the handle in node state
let repaint: Repaint = cx.repaint();
// later, from an event handler or timer, on the UI thread
repaint.mark();            // re-record this node on the next frame; no rebuild, no relayout
```

`Repaint` is a small `Clone` handle to one node. `mark` sets the node's paint-dirty flag and
wakes the run loop, the same wake-up a signal uses. A repaint that records the same list costs
nothing. There is no `mark_needs_layout`: layout-affecting state must be a signal or part of
the widget, so it goes through `build`.

### 2. Recording instead of painting

`Node` paints into a `Recorder` and hands the result to the scene. `paint_child(i)` records
`Item::Child(kid_id)`; `paint_child_on_top(i)` records `Item::Overlay(kid_id)`. A node is
re-recorded during the walk only when it must be:

- never recorded, or its widget / context was replaced or it was rebuilt
- `Repaint::mark` was called on it
- its laid-out `rect` changed
- the `focused` flag passed down changed
- the inherited text style changed
- a cached focus id's current focus state differs from when it was recorded

Otherwise the walk does nothing for that node: its layer in the scene is already current.
Cost is O(nodes) for the walk, plus recording only the changed nodes.

### 3. Layout only when needed

```
relayout = first frame || canvas size changed || any node rebuilt (a dirty observer rebuilds)
```

Layout only writes `size` / `offset`. Nodes whose rect changed notice it in the walk and
re-record. Subtree layout with relayout boundaries is a later optimisation.

### Focus

Focus registration happens in `paint` today, so a node that is not re-recorded would drop out
of the Tab order. `lobo` caches the focus ids a node registered and replays them in walk order
(same as today's paint order), so Tab order is unchanged. A focus change makes the nodes whose
cached state differs re-record, so only the old and new focus rings repaint. Focus never enters
the scene; it is `lobo` state.

### Lifecycle

- A node's `LayerId` is allocated at mount. The scene belongs to the run loop, so a node
  cannot reach it from `Drop`. Instead `Drop` pushes the id onto a removed-ids list in the
  tree's root state (the root is passed to the walk, not a global), and the walk calls
  `scene.remove(id)` for each before `present`. An unmounted widget's pixels are cleared on
  the next frame and `dispose` still runs immediately.
- Child links in a list refer to ids, not to `Rc<Node>`, so the scene never keeps a node alive.

## Entry point

```rust
lobo::run(app, &mut backend)         // backend: &mut dyn Renderer
```

Each loop turn: dispatch input, `root.update()`, `root.present(&mut scene, backend)` (which
relayouts if needed, walks and re-records changed nodes into the scene, and calls
`scene.present(backend)`). `lobo` calls `present`; everything inside `Scene` lives in `renderer`.

## Behaviour changes to be aware of

- `render_frame` is removed (replaced by `Node::present(&mut scene, &mut backend)`);
  `PaintCx::renderer` is a `Recorder<'a>`. `measure_text` and `size` still reach the real backend.
- A widget that draws from state that is neither a `Signal`, nor part of its widget / context /
  rect (for example a `Cell` mutated in an event handler) will not repaint until it calls
  `Repaint::mark`.
- Overlay ordering and `focus::request_next` are unchanged.

## Testing

In `renderer` (no `lobo`, with a spy backend that records each `draw_batch`):

- first frame damages the whole canvas, one `draw_batch` call
- `present` with no layer changes sends nothing
- an identical list set again sends nothing
- a changed layer damages `old ∪ new` bounds only
- removing a layer, or unlinking a child, damages its old bounds; re-linking it damages them again
- overlays draw after the tree
- a partial frame equals a full redraw inside the damaged rect. Comparing command lists can't
  show this; it needs pixels, so it is a `renderer-cpu` test (render both into a buffer and
  compare inside the damage)
- damage is grown to whole pixels

In `lobo` (with a null / spying backend):

- `Repaint::mark` re-records that node only: `build` and `layout` do not run again
- a rebuild that paints the same pixels sends nothing
- moving one widget in a `Column` damages its old and new rects, not the canvas
- closing a menu clears the overlay area
- Tab order and focus rings survive caching

## Implementation status

1. **Done**: `renderer` owns geometry, paint types, `Renderer`, `DrawCmd`, `Recorder`, `Scene`, and
   `draw_batch`, with spy-backend tests in `renderer/tests/scene.rs`. Widget-level `draw_*` helpers
   live as free functions in `lobo` (`src/render/mod.rs`), and `Rect::deflate` takes four floats.
2. **Done**: `lobo` records into `Scene` via `Node::present`, provides `Repaint` on `NodeCx`, caches
   focus registrations, and skips relayout when clean (`tests/retained.rs`).
3. **Done**: `lobo::run(app, backend)` drives the retained loop.
4. **Done (CPU)**: `renderer-cpu` implements `PixmapRenderer` via `vello_cpu`, with pixel-exact
   partial-vs-full redraw tests (`renderer-cpu/tests/retained.rs`), render tests
   (`renderer-cpu/tests/render.rs`), and `renderer-cpu/examples/gallery.rs`. `renderer-gpu` (`wgpu`)
   remains next.

## Open questions

- **Input source.** `run` reads keys from the terminal today (`src/term.rs`, crossterm). With a
  windowed backend the window owns the event loop and the events. Who drives the loop, and how
  input reaches `lobo` without `lobo` naming the backend, is not designed here.
- **Relayout boundaries.** Is full-tree layout acceptable for large lists?
- **Animation.** A frame clock calls `Repaint::mark` on animating nodes each tick; not designed here.
