//! Retained tree: one `Node` per mounted widget. A node caches what its
//! widget built, and rebuilds only when a signal it read has changed.
//! Each frame, changed subtrees are laid out and re-recorded into a [`Scene`].
//!
//! When a node rebuilds, its new child widgets are matched against its current
//! child nodes (by key, else by position and type). A matched node is updated in
//! place: it keeps its state, its focus and its own children, which are matched
//! the same way. Only unmatched widgets are mounted, and only unmatched nodes are
//! unmounted.

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt::Write;
use std::rc::Rc;

use crate::layout::{Constraints, InteractionState, LayoutCx, PaintCx, focus, node_state};
use crate::pointer::{self, PointerRegion};
use crate::render::{LayerId, Offset, Recorder, Rect, Renderer, Scene, Size};
use crate::signal::{Observer, track, wake};
use crate::task::with_owner;
use crate::theme::{TextStyle, ThemeData};
use crate::{Context, Widget, global_events};

const BACKDROP_LAYER: LayerId = LayerId(0);

pub(crate) struct TreeState {
  next_layer: u64,
  pub(crate) removed_layers: Vec<LayerId>,
  pub(crate) layout_dirty: bool,
  pub(crate) last_size: Option<Size>,
}

impl TreeState {
  fn new() -> Self {
    TreeState { next_layer: 1, removed_layers: Vec::new(), layout_dirty: true, last_size: None }
  }

  fn alloc_layer(&mut self) -> LayerId {
    let id = LayerId(self.next_layer);
    self.next_layer += 1;
    id
  }
}

/// Handle for requesting a repaint of a node without rebuilding or laying out.
#[derive(Clone)]
pub struct Repaint {
  dirty: Rc<Cell<bool>>,
}

impl Repaint {
  /// Marks this node's display list dirty and wakes the run loop.
  pub fn mark(&self) {
    self.dirty.set(true);
    wake();
  }
}

#[derive(Clone)]
pub(crate) enum WalkEvent {
  Focus {
    id: String,
    was_focused: bool,
    was_hovered: bool,
    was_pressed: bool,
    activate_on_click: bool,
  },
  Pointer(PointerRegion),
  Child(usize),
  Overlay(usize),
}

struct Recorded {
  rect: Rect,
  interaction: InteractionState,
  style: TextStyle,
  events: Vec<WalkEvent>,
}

pub struct Node {
  widget: RefCell<Widget>,
  /// The context this node's widget is built in (set by its parent).
  context: RefCell<Context>,
  /// Replaced on every rebuild, which drops stale subscriptions.
  /// Signals read in `build`, `layout` and `paint` all subscribe it.
  observer: RefCell<Rc<Observer>>,
  composite: Cell<bool>,
  kids: RefCell<Vec<Rc<Node>>>,
  /// Laid-out size, and position relative to the parent's top-left.
  pub(crate) size: Cell<Size>,
  pub(crate) offset: Cell<Offset>,
  /// Text style in effect here (inherited), from the last layout.
  style: RefCell<TextStyle>,
  /// Widget state kept for the node's whole life: across frames, rebuilds and
  /// in-place updates (`LayoutCx::state`, `NodeCx::state`), e.g. a list's scroll offset.
  state: RefCell<Option<Rc<dyn Any>>>,
  pub(crate) layer_id: LayerId,
  tree: Rc<RefCell<TreeState>>,
  paint_dirty: Rc<Cell<bool>>,
  recorded: RefCell<Option<Recorded>>,
}

/// What `Element::attach`, `dispose` and `scope` get: the node's context, state, and repaint handle.
pub struct NodeCx<'a> {
  context: &'a Context,
  state: &'a RefCell<Option<Rc<dyn Any>>>,
  repaint: &'a Rc<Cell<bool>>,
}

impl NodeCx<'_> {
  /// The context the widget is built in (from its parent).
  pub fn context(&self) -> &Context {
    self.context
  }

  /// The node's state (same value as `LayoutCx::state` / `PaintCx::state`); created
  /// with `Default` on first use and dropped when the node is unmounted.
  pub fn state<T: Default + 'static>(&self) -> Rc<RefCell<T>> {
    node_state(self.state)
  }

  /// Returns a handle that schedules a repaint of this node on `mark()`.
  pub fn repaint(&self) -> Repaint {
    Repaint { dirty: self.repaint.clone() }
  }
}

impl Node {
  pub fn mount(widget: Widget, context: Context) -> Rc<Node> {
    let tree = Rc::new(RefCell::new(TreeState::new()));
    Node::mount_with(widget, context, tree)
  }

  fn mount_with(widget: Widget, context: Context, tree: Rc<RefCell<TreeState>>) -> Rc<Node> {
    let layer_id = tree.borrow_mut().alloc_layer();
    let node = Rc::new(Node {
      widget: RefCell::new(widget),
      context: RefCell::new(context),
      observer: RefCell::new(Observer::new()),
      composite: Cell::new(false),
      kids: RefCell::new(Vec::new()),
      size: Cell::new(Size::ZERO),
      offset: Cell::new(Offset::ZERO),
      style: RefCell::new(TextStyle::default()),
      state: RefCell::new(None),
      layer_id,
      tree,
      paint_dirty: Rc::new(Cell::new(true)),
      recorded: RefCell::new(None),
    });
    node.attach();
    node.rebuild();
    node
  }

  /// The widget currently on this node.
  pub fn widget(&self) -> Widget {
    self.widget.borrow().clone()
  }

  /// The context this node's widget is built in.
  pub fn context(&self) -> Context {
    self.context.borrow().clone()
  }

  fn attach(&self) {
    let (widget, context) = (self.widget(), self.context());
    with_owner(context.task_owner(), || {
      widget.element.attach(&NodeCx {
        context: &context,
        state: &self.state,
        repaint: &self.paint_dirty,
      })
    });
  }

  /// Puts `widget` (same type as the current one) on this node, built in `context`,
  /// and rebuilds the node, unless both are the very same as before.
  fn update_with(&self, widget: Widget, context: Context) {
    let old_widget = self.widget.borrow().clone();
    let same_ctx = self.context.borrow().same(&context);
    if same_ctx && Rc::ptr_eq(&old_widget.element, &widget.element) {
      // Same widget value in the same context builds the same thing: keep the subtree.
      // If a signal it read changed, the `update` walk rebuilds it.
      return;
    }
    if same_ctx
      && let Some(old_any) = old_widget.element.as_any()
      && !widget.element.should_repaint(old_any)
    {
      // Widget explicitly declared its output unchanged (e.g. CustomPainter::should_repaint == false).
      *self.widget.borrow_mut() = widget;
      return;
    }
    *self.widget.borrow_mut() = widget;
    *self.context.borrow_mut() = context;
    self.attach();
    self.rebuild();
  }

  fn rebuild(&self) {
    self.paint_dirty.set(true);
    self.tree.borrow_mut().layout_dirty = true;

    let observer = Observer::new();
    *self.observer.borrow_mut() = observer.clone();

    let (widget, context) = (self.widget(), self.context());
    // Tasks started while building belong to the nearest `Provider` above.
    let (built, scope, children) = with_owner(context.task_owner(), || {
      let (built, scope) = track(&observer, || {
        let built = widget.element.build(&context);
        let scope = widget.element.scope(&NodeCx {
          context: &context,
          state: &self.state,
          repaint: &self.paint_dirty,
        });
        (built, scope)
      });
      let children = if built.is_none() { widget.element.children() } else { Vec::new() };
      (built, scope, children)
    });
    self.composite.set(built.is_some());
    let widgets = match built {
      Some(w) => vec![w],
      None => children,
    };
    let child_context = scope.unwrap_or(context);
    // Outside `track`: children subscribe themselves, not us.
    self.reconcile(widgets, child_context);
  }

  /// Matches `widgets` against the current children: a keyed widget takes the old
  /// child with the same key and type, an unkeyed one the unkeyed old child of the same
  /// type at the same index. Matched children are updated, the rest mounted, and old
  /// children nobody took are unmounted (before the new ones are mounted).
  fn reconcile(&self, widgets: Vec<Widget>, context: Context) {
    let mut old: Vec<Option<Rc<Node>>> =
      std::mem::take(&mut *self.kids.borrow_mut()).into_iter().map(Some).collect();
    let mut keyed: HashMap<(TypeId, u64), usize> = HashMap::new();
    for (i, kid) in old.iter().enumerate() {
      let w = kid.as_ref().unwrap().widget.borrow();
      if let Some(key) = w.key {
        keyed.entry((w.type_id, key)).or_insert(i);
      }
    }
    let matched: Vec<Option<Rc<Node>>> = widgets
      .iter()
      .enumerate()
      .map(|(i, w)| match w.key {
        Some(key) => keyed.remove(&(w.type_id, key)).and_then(|j| old[j].take()),
        None => {
          let fits = old.get(i).and_then(Option::as_ref).is_some_and(|kid| {
            let k = kid.widget.borrow();
            k.key.is_none() && k.type_id == w.type_id
          });
          if fits { old[i].take() } else { None }
        }
      })
      .collect();
    drop(old);

    let kids = widgets
      .into_iter()
      .zip(matched)
      .map(|(w, kid)| match kid {
        Some(kid) => {
          kid.update_with(w, context.clone());
          kid
        }
        None => Node::mount_with(w, context.clone(), self.tree.clone()),
      })
      .collect();
    *self.kids.borrow_mut() = kids;
  }

  /// Advances active declarative animations and rebuilds nodes whose signals changed;
  /// clean nodes are only walked. Returns whether anything was rebuilt.
  pub fn update(&self) -> bool {
    crate::anim::tick_animations();
    self.update_inner()
  }

  fn update_inner(&self) -> bool {
    let mut rebuilt = false;
    if self.observer.borrow().is_dirty() {
      self.rebuild();
      rebuilt = true;
    }
    // Children kept by the rebuild (or untouched by it) may be dirty themselves.
    let kids = self.kids.borrow().clone();
    for kid in kids {
      rebuilt |= kid.update_inner();
    }
    rebuilt
  }

  /// Size and position from the last layout, on the canvas, given the parent's origin.
  pub fn rect_in(&self, parent_origin: Offset) -> Rect {
    Rect::from_origin(parent_origin + self.offset.get(), self.size.get())
  }

  /// Lays this node out within `constraints`; returns (and stores) its size.
  /// `inherited` is the text style in effect here.
  pub fn layout(
    &self,
    constraints: Constraints,
    renderer: &dyn Renderer,
    inherited: &TextStyle,
  ) -> Size {
    let widget = self.widget();
    let theme = ThemeData::of(&self.context.borrow());
    let kids = self.kids.borrow().clone();
    // A child the widget doesn't place sits at the top-left.
    for kid in &kids {
      kid.offset.set(Offset::ZERO);
    }
    let observer = self.observer.borrow().clone();
    let size = track(&observer, || {
      let mut cx = LayoutCx {
        kids: &kids,
        renderer,
        theme: &theme,
        style: inherited,
        child_style: inherited.clone(),
        state: &self.state,
      };
      widget.element.layout(&mut cx, constraints)
    });
    let size = constraints.constrain(size);
    self.size.set(size);
    *self.style.borrow_mut() = inherited.clone();
    size
  }

  /// Lays out if needed, re-records changed nodes into `scene`, and submits the damaged batch.
  /// Returns the damaged rect, or `None` when nothing changed.
  pub fn present(&self, scene: &mut Scene, backend: &mut dyn Renderer) -> Option<Rect> {
    self.present_inner(scene, backend, true)
  }

  /// Like [`Node::present`], without touching the global keyboard focus ring.
  /// Used for standalone overlay trees (e.g. `PerfLayer`) rendered to a separate renderer.
  #[cfg(debug_assertions)]
  pub(crate) fn present_isolated(
    &self,
    scene: &mut Scene,
    backend: &mut dyn Renderer,
  ) -> Option<Rect> {
    self.present_inner(scene, backend, false)
  }

  fn present_inner(
    &self,
    scene: &mut Scene,
    backend: &mut dyn Renderer,
    manage_focus: bool,
  ) -> Option<Rect> {
    let removed = std::mem::take(&mut self.tree.borrow_mut().removed_layers);
    for id in removed {
      scene.remove(id);
    }

    let size = backend.size();
    let relayout = {
      let mut tree = self.tree.borrow_mut();
      let need = tree.layout_dirty || tree.last_size != Some(size);
      tree.layout_dirty = false;
      tree.last_size = Some(size);
      need
    };
    if relayout {
      self.layout(Constraints::tight(size), backend, &TextStyle::default());
    }

    let mut backdrop = Recorder::new(backend);
    backdrop.clear(ThemeData::of(&self.context.borrow()).color_scheme.surface);
    backdrop.child(self.layer_id);
    scene.set_root(BACKDROP_LAYER);
    scene.set(BACKDROP_LAYER, backdrop.finish());

    if manage_focus {
      focus::begin_frame();
      pointer::begin_frame();
    }
    let mut overlays = VecDeque::new();
    self.walk_paint(Offset::ZERO, InteractionState::default(), scene, backend, &mut overlays);
    while let Some((node, origin, interaction)) = overlays.pop_front() {
      node.walk_paint(origin, interaction, scene, backend, &mut overlays);
    }
    if manage_focus {
      focus::end_frame();
    }

    scene.present(backend)
  }

  /// Like [`Node::present`], also returning per-phase [`FrameTimings`](crate::FrameTimings).
  #[cfg(debug_assertions)]
  pub fn present_timed(
    &self,
    scene: &mut Scene,
    backend: &mut dyn Renderer,
  ) -> (Option<Rect>, crate::FrameTimings) {
    let removed = std::mem::take(&mut self.tree.borrow_mut().removed_layers);
    for id in removed {
      scene.remove(id);
    }

    let size = backend.size();
    let relayout = {
      let mut tree = self.tree.borrow_mut();
      let need = tree.layout_dirty || tree.last_size != Some(size);
      tree.layout_dirty = false;
      tree.last_size = Some(size);
      need
    };
    let t_layout = std::time::Instant::now();
    if relayout {
      self.layout(Constraints::tight(size), backend, &TextStyle::default());
    }
    let layout_ms = t_layout.elapsed().as_secs_f32() * 1000.0;

    let t_paint = std::time::Instant::now();
    let mut backdrop = Recorder::new(backend);
    backdrop.clear(ThemeData::of(&self.context.borrow()).color_scheme.surface);
    backdrop.child(self.layer_id);
    scene.set_root(BACKDROP_LAYER);
    scene.set(BACKDROP_LAYER, backdrop.finish());

    focus::begin_frame();
    pointer::begin_frame();
    let mut overlays = VecDeque::new();
    self.walk_paint(Offset::ZERO, InteractionState::default(), scene, backend, &mut overlays);
    while let Some((node, origin, interaction)) = overlays.pop_front() {
      node.walk_paint(origin, interaction, scene, backend, &mut overlays);
    }
    focus::end_frame();
    let paint_ms = t_paint.elapsed().as_secs_f32() * 1000.0;

    let st = scene.present_timed(backend);
    let timings = crate::FrameTimings {
      update_ms: 0.0,
      layout_ms,
      paint_ms,
      scene_ms: st.diff_and_flatten_ms,
      raster_ms: st.raster_ms,
      present_ms: 0.0,
      cmd_count: st.cmd_count,
    };
    (st.damage, timings)
  }

  /// Walks this node for retained painting, re-recording its layer only when invalidated.
  pub(crate) fn walk_paint(
    &self,
    parent_origin: Offset,
    interaction: InteractionState,
    scene: &mut Scene,
    backend: &dyn Renderer,
    overlays: &mut VecDeque<(Rc<Node>, Offset, InteractionState)>,
  ) {
    let rect = self.rect_in(parent_origin);
    let style = self.style.borrow().clone();
    let kids = self.kids.borrow().clone();

    let cached_events = if !self.paint_dirty.get() {
      self.recorded.borrow().as_ref().and_then(|rec| {
        if rec.rect != rect || rec.interaction != interaction || rec.style != style {
          return None;
        }
        let mut pending = focus::is_pending();
        for event in &rec.events {
          if let WalkEvent::Focus { id, was_focused, was_hovered, was_pressed, .. } = event {
            let now_focused = pending || global_events.is_focused(id);
            pending = false;
            let now_hovered = pointer::is_focus_hovered(id);
            let now_pressed = pointer::is_focus_pressed(id);
            if now_focused != *was_focused
              || now_hovered != *was_hovered
              || now_pressed != *was_pressed
            {
              return None;
            }
          }
        }
        Some(rec.events.clone())
      })
    } else {
      None
    };

    if let Some(events) = cached_events {
      let mut cur = interaction;
      for event in events {
        match event {
          WalkEvent::Focus { id, activate_on_click, .. } => {
            cur.focused |= focus::register(&id, rect);
            pointer::register_focus_target(&id, rect, activate_on_click);
            cur.hovered |= pointer::is_focus_hovered(&id);
            cur.pressed |= pointer::is_focus_pressed(&id);
          }
          WalkEvent::Pointer(region) => {
            pointer::register_region(region);
          }
          WalkEvent::Child(i) => {
            kids[i].walk_paint(rect.origin(), cur, scene, backend, overlays);
          }
          WalkEvent::Overlay(i) => {
            overlays.push_back((kids[i].clone(), rect.origin(), cur));
          }
        }
      }
      return;
    }

    self.paint_dirty.set(false);
    let widget = self.widget();
    let theme = ThemeData::of(&self.context.borrow());
    let observer = self.observer.borrow().clone();
    let mut events = Vec::new();
    let list = track(&observer, || {
      let mut cx = PaintCx {
        renderer: Recorder::new(backend),
        rect,
        kids: &kids,
        theme: &theme,
        style: &style,
        interaction,
        paint_dirty: &self.paint_dirty,
        state: &self.state,
        backend,
        scene,
        overlays,
        events: &mut events,
      };
      widget.element.paint(&mut cx);
      cx.renderer.finish()
    });
    scene.set(self.layer_id, list);
    *self.recorded.borrow_mut() = Some(Recorded { rect, interaction, style, events });
  }

  /// Prints the widget tree (one line per node); never calls `build`.
  /// Composite widgets are shown as `Name()`.
  pub fn debug_tree(&self, depth: usize, out: &mut String) {
    let suffix = if self.composite.get() { "()" } else { "" };
    writeln!(out, "{}{}{}", "  ".repeat(depth), self.widget.borrow().name(), suffix).unwrap();
    for kid in self.kids.borrow().iter() {
      kid.debug_tree(depth + 1, out);
    }
  }
}

impl Drop for Node {
  fn drop(&mut self) {
    self.tree.borrow_mut().removed_layers.push(self.layer_id);
    let widget = self.widget.get_mut().clone();
    let context = self.context.get_mut().clone();
    widget.element.dispose(&NodeCx {
      context: &context,
      state: &self.state,
      repaint: &self.paint_dirty,
    });
  }
}
