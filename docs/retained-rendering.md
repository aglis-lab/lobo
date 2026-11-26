# Retained rendering

Design for drawing only what changed, in batches. Status: **proposal**, nothing here is
finished. See [renderer.md](renderer.md) for the `Renderer` trait this builds on.

## Problem

The declarative layer is already retained: `Node`s persist, a signal change rebuilds only the
nodes that read it. Drawing is not. `render_frame` lays out the whole tree and paints every
widget, straight into the backend, on every change. That is immediate mode with a retained
tree in front of it.

Goals:

1. **Redraw only the changed region.** A blinking caret must not repaint the screen.
2. **Not immediate mode.** `paint` must not be what talks to the device.
3. **Batch.** One submission per frame, not one call per primitive.
4. Keep every existing widget's `paint` unchanged.

## Overview

```
signal change
   │ marks the node's observer dirty
   ▼
update ──▶ layout (only if needed) ──▶ walk ──▶ damage ──▶ batch ──▶ backend
 rebuild     tree top-down              re-record   merged     filtered,     begin_frame
 dirty nodes                            changed     rects      clipped       draw_batch
                                        nodes only                          end_frame
```

Three ideas:

- **Record, don't draw.** `paint` writes into a *display list* (`Vec<DrawCmd>`) through a
  `Recorder`, which implements `Renderer`. Widgets are unchanged.
- **Cache per node.** Each node keeps the list from its last paint and re-records only when
  something it depends on changed. Comparing the new list with the old one tells exactly
  whether pixels changed.
- **Compositor.** Owns the frame: finds damage, filters the cached lists to it, and submits one
  batch.

## Display list

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
```

These are exactly the `Renderer` primitives, derive `PartialEq`, and are cheap to clone.
Coordinates are absolute canvas pixels, as widgets already use.

The widget-level methods (`draw_checkbox`, `draw_switch`, ...) are defaults built from
primitives, so on a `Recorder` they are recorded as primitives. A backend therefore never sees
them and cannot override them in the retained path. That is the price of batching.

Each recorded command carries `bounds`: the canvas area it can change, cut by the clips active
inside the same recording, and grown 1 px for anti-aliasing. Text bounds come from
`measure_text`. A command whose bounds are empty (fully clipped) is dropped.

A node's recording is a list of **items**, in paint order:

```
Item::Cmd(cmd, bounds)
Item::Child { node: Weak<Node>, origin, focused }     // paint_child(i)
Item::Overlay { node: Weak<Node>, origin, focused }   // paint_child_on_top(i)
```

`paint_child` no longer paints; it records a `Child` marker. So a node's cache holds only its
own drawing, and a parent re-recording never re-records its children. Children are `Weak` so
unmounting still drops a node (and runs `dispose`) immediately.

## Node cache

```
cache {
  items, bounds,        // last recording and the area its commands cover
  rect, focused,        // inputs it was recorded with
  style,                // inherited text style at that time
  focus: [(id, was_focused)],
}
```

Observers are split by phase, so a signal only triggers the work that read it:

| Observer | Subscribed while | When dirty |
|---|---|---|
| build | `build`, `scope` | rebuild the node (as today), then relayout |
| layout | `layout` | relayout |
| paint | `paint` | re-record this node only |

Today one observer covers all three, so a signal read in `paint` (the text field's controller)
rebuilds the node. With a paint observer it only re-records.

A node is **re-recorded** on the walk when any of these hold:

- never recorded, or released (see below)
- its widget or context was replaced (`update_with`) or it was rebuilt
- its paint observer is dirty
- its `rect` differs from the cached one (it moved or resized)
- the `focused` flag passed down differs
- the inherited text style differs
- any cached focus id's current focus state differs from the recorded one

After re-recording, the new items are compared with the old (`PartialEq`). Only if they differ
is `old.bounds ∪ new.bounds` added to the damage. A rebuild that produces the same pixels
(the common case for a composite parent) costs no repaint.

### Released nodes

A node that stops being painted must have its pixels cleared:

- **Unmounted**: `Drop` adds its cached bounds to the damage.
- **No longer referenced by its parent's new recording** (a closed menu, a hidden page): the
  parent diff finds the dropped `Child` / `Overlay` links and calls `release()` on each: add the
  node's bounds to the damage, clear its cache, recurse into its own links.

## Layout

Layout stays top-down and whole-tree, but is skipped when nothing could have moved:

```
relayout = first frame || canvas size changed || any node rebuilt || any layout observer dirty
```

Layout writes `size` / `offset` only. It does not paint. Nodes whose rect changed notice it in
the walk and re-record. Partial (subtree) layout with relayout boundaries is a later
optimisation, not part of this design.

## Compositor

```rust
let mut compositor = Compositor::new();
let info = compositor.render(&root, &mut backend);   // FrameInfo { relaid_out, damage, commands }
```

`render` does, in order:

1. `root.update()`: rebuild dirty nodes.
2. Layout, if the condition above holds.
3. **Walk** from the root, then drain overlays (as `render_frame` does today). Each node is
   re-recorded if needed; then its focus ids are registered with `focus` (replayed from the
   cache for clean nodes, so Tab order stays complete) and its items are appended to the frame
   list, expanding `Child` markers recursively. The walk is O(nodes + commands) but only
   re-records changed nodes.
4. **Damage**: take everything added during the walk and by `Drop`. The whole canvas is damaged
   on the first frame, after a resize, or when the theme's surface colour changes.
5. If the damage is empty, **stop**: nothing is sent to the backend.
6. **Batch**: for each damage rect `D`, emit `PushClip(D)`, every command whose `bounds`
   intersect `D` (a `Clear` becomes `FillRect(D)`), then `PopClip`. Clip commands are always
   kept so the stack stays balanced; an empty `PushClip`/`PopClip` pair is removed.
7. Submit:

```
backend.begin_frame(&damage_rects);
backend.draw_batch(&batch);       // one call per frame
backend.end_frame();
```

Replaying *every* command that touches `D` (background, siblings below, siblings above), clipped
to `D`, gives the same pixels inside `D` as a full redraw, so partial updates are exact.

### Damage regions

`Damage` holds pixel-aligned rects (grown outward to whole pixels). Rects that overlap or touch
are merged; past 8 rects they collapse to their bounding box. This bounds batch size while
keeping two distant changes (a clock and a caret) as separate small regions.

## Renderer trait changes

Added with defaults, so existing backends keep compiling:

```rust
fn begin_frame(&mut self, damage: &[Rect]) {}
fn draw_batch(&mut self, cmds: &[DrawCmd]) { for c in cmds { c.apply(self) } }
fn end_frame(&mut self) {}
```

Contract for a backend:

- The surface **keeps its pixels between frames**; only damaged rects are redrawn.
- `Clear` is never sent mid-frame (it ignores the clip). The background arrives as `FillRect`.
- A CPU backend can use `damage` to upload or present only those rects. A GPU backend overrides
  `draw_batch` to build one vertex buffer (rects and circles as instanced SDF quads, text from
  a glyph atlas), using the `PushClip` rects as scissors.

The background must be opaque: translucent fills are re-blended over a freshly repainted
background, but a translucent background would compound over the old pixels.

## Focus

Focus registration happens in `paint` today, so a cached (not re-recorded) node would drop out
of the Tab order. The cache therefore stores the focus ids it registered and replays them in
walk order. The walk order matches today's paint order, so Tab order is unchanged. A change of
focus makes the affected nodes (those whose cached `was_focused` differs, and descendants whose
`focused` flag changed) re-record, so only the old and new focus rings repaint.

## Behaviour changes to be aware of

- `render_frame` is replaced by `Compositor::render`. `PaintCx::renderer` is now a recorder;
  `measure_text` and `size` still reach the real backend.
- A widget that draws based on state that is neither a `Signal` read in `paint` nor part of its
  widget/context/rect (for example a `Cell` mutated from an event handler) will not repaint.
  Such state must be a `Signal`, or the handler must call a new `PaintCx`-free hook,
  `mark_needs_paint` (open question below).
- Signals read in `layout` no longer rebuild the node, they relayout it.
- Overlay ordering and `focus::request_next` are unchanged.

## Testing

With a spying backend that records `begin_frame` / `draw_batch` / `end_frame`:

- first frame damages the whole canvas, one `draw_batch` call
- a second `render` with nothing changed sends nothing
- a signal read only in `paint` repaints that widget's rect, and `build` does not run again
- moving one widget in a `Column` damages the old and new rects, not the canvas
- a rebuild producing identical output sends nothing
- closing a menu clears the overlay's area
- unmounting a node damages its old bounds
- focus change repaints only the two focus rings; Tab order is unchanged
- the pixels of a partial frame equal a full redraw (replay both into a recording and compare
  the clipped results)
- `Damage` unit tests: pixel snapping, merge, collapse

## Implementation steps

1. `DrawCmd`, `Recorder`, `Damage`, `Rect::{union, touches, intersects}`, and the three
   `Renderer` hooks. (No behaviour change.)
2. Split `Node` observers into build / layout / paint; add the cache, `walk`, `release`, and
   damage on `Drop`.
3. `PaintCx` records `Child` / `Overlay` items and focus ids; remove `render_frame` and the
   overlay thread-local.
4. `Compositor` and `FrameInfo`.
5. Tests above; update `reconcile.rs` to use the compositor with a null backend.
6. Move the pieces that do not depend on `lobo` into the `renderer` crate (see
   [renderer.md](renderer.md)).

## Open questions

- **Repaint without a signal.** Do we want an explicit `mark_needs_paint()` for state that is
  not a `Signal`, or require signals? Signals are the existing convention.
- **Relayout boundaries.** Full-tree layout when anything changes is simple and fine for now;
  is subtree layout needed for large lists?
- **Animation.** A frame clock should mark animating nodes' paint observer dirty each tick; not
  designed here.
- **Max damage rects.** 8 is a guess; tune on a real backend.
