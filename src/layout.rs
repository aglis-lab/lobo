//! Layout and paint protocol, in the style of Flutter's render boxes.
//!
//! Each frame, the framework lays the tree out top-down: a parent passes
//! [`Constraints`] to each child, the child returns its size, and the parent
//! places it. Then it paints top-down: a widget paints itself and calls
//! `paint_children` where its children go. Children painted "on top" are painted
//! after the whole tree.
//!
//! Keyboard focus: focusable `InputEvent`s register in paint order; Tab / Shift-Tab
//! move the focus, and the focused listener gets keys before the global ones.

use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::Padding;
use crate::render::{Offset, Recorder, Rect, Renderer, Scene, Size, TextPaint};
use crate::theme::{TextStyle, ThemeData};
use crate::tree::{Node, WalkEvent};

// ---------------------------------------------------------------------------
// Constraints
// ---------------------------------------------------------------------------

/// Allowed sizes for a child. `max_*` may be `f32::INFINITY` (unbounded).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraints {
  pub min_width: f32,
  pub max_width: f32,
  pub min_height: f32,
  pub max_height: f32,
}

impl Constraints {
  /// Exactly `size`.
  pub fn tight(size: Size) -> Self {
    Constraints {
      min_width: size.width,
      max_width: size.width,
      min_height: size.height,
      max_height: size.height,
    }
  }

  /// Anything up to `size`.
  pub fn loose(size: Size) -> Self {
    Constraints { min_width: 0.0, max_width: size.width, min_height: 0.0, max_height: size.height }
  }

  /// Same maximums, no minimums.
  pub fn loosen(&self) -> Self {
    Constraints { min_width: 0.0, min_height: 0.0, ..*self }
  }

  /// Clamps `size` into these constraints. An infinite size becomes the minimum.
  pub fn constrain(&self, size: Size) -> Size {
    let w = if size.width.is_finite() { size.width } else { self.min_width };
    let h = if size.height.is_finite() { size.height } else { self.min_height };
    Size::new(
      w.clamp(self.min_width, self.max_width.max(self.min_width)),
      h.clamp(self.min_height, self.max_height.max(self.min_height)),
    )
  }

  /// Room left inside `padding`.
  pub fn deflate(&self, padding: Padding) -> Self {
    let h = padding.left + padding.right;
    let v = padding.top + padding.bottom;
    Constraints {
      min_width: (self.min_width - h).max(0.0),
      max_width: (self.max_width - h).max(0.0),
      min_height: (self.min_height - v).max(0.0),
      max_height: (self.max_height - v).max(0.0),
    }
  }

  /// Forces the given dimensions (clamped to what is allowed).
  pub fn tighten(&self, width: Option<f32>, height: Option<f32>) -> Self {
    let mut c = *self;
    if let Some(w) = width {
      let w = w.clamp(self.min_width, self.max_width);
      c.min_width = w;
      c.max_width = w;
    }
    if let Some(h) = height {
      let h = h.clamp(self.min_height, self.max_height);
      c.min_height = h;
      c.max_height = h;
    }
    c
  }

  pub fn with_max_width(&self, w: f32) -> Self {
    Constraints { max_width: w, min_width: self.min_width.min(w), ..*self }
  }

  pub fn with_max_height(&self, h: f32) -> Self {
    Constraints { max_height: h, min_height: self.min_height.min(h), ..*self }
  }

  pub fn biggest(&self) -> Size {
    Size::new(self.max_width, self.max_height)
  }

  pub fn smallest(&self) -> Size {
    Size::new(self.min_width, self.min_height)
  }

  pub fn has_bounded_width(&self) -> bool {
    self.max_width.is_finite()
  }

  pub fn has_bounded_height(&self) -> bool {
    self.max_height.is_finite()
  }
}

/// How a flexible child of a `Row` / `Column` fills its share.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexFit {
  /// Exactly its share (`Expanded`).
  #[default]
  Tight,
  /// At most its share (`Flexible`).
  Loose,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flex {
  pub flex: f32,
  pub fit: FlexFit,
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// What a widget's `layout` can do with its children.
pub struct LayoutCx<'a> {
  pub(crate) kids: &'a [Rc<Node>],
  pub(crate) renderer: &'a dyn Renderer,
  pub(crate) theme: &'a ThemeData,
  /// Style in effect at this widget.
  pub(crate) style: &'a TextStyle,
  /// Style passed to the children (`set_child_style`).
  pub(crate) child_style: TextStyle,
  pub(crate) state: &'a RefCell<Option<Rc<dyn Any>>>,
}

/// State kept on the node between frames; created with `Default` on first use.
pub(crate) fn node_state<T: Default + 'static>(
  slot: &RefCell<Option<Rc<dyn Any>>>,
) -> Rc<RefCell<T>> {
  let mut slot = slot.borrow_mut();
  if let Some(state) = slot.as_ref().and_then(|s| s.clone().downcast::<RefCell<T>>().ok()) {
    return state;
  }
  let state = Rc::new(RefCell::new(T::default()));
  *slot = Some(state.clone());
  state
}

impl LayoutCx<'_> {
  /// This widget's state, kept for the node's whole life: across frames, rebuilds and
  /// in-place updates (it is lost only when the node is unmounted). Same value as
  /// `PaintCx::state` and `NodeCx::state`.
  pub fn state<T: Default + 'static>(&self) -> Rc<RefCell<T>> {
    node_state(self.state)
  }

  /// Data a child exposes to its parent (`Element::parent_data`), e.g. `Positioned`.
  pub fn child_data<T: 'static>(&self, i: usize) -> Option<T> {
    self.kids[i].widget().element.parent_data().and_then(|d| d.downcast::<T>().ok()).map(|b| *b)
  }

  pub fn child_count(&self) -> usize {
    self.kids.len()
  }

  /// Text style for the children, merged over the inherited one. Call it before
  /// `layout_child`; it also applies when they paint.
  pub fn set_child_style(&mut self, style: TextStyle) {
    self.child_style = style.merge(self.style);
  }

  /// Lays out child `i` and returns its size. Call once per child per frame.
  pub fn layout_child(&mut self, i: usize, constraints: Constraints) -> Size {
    self.kids[i].layout(constraints, self.renderer, &self.child_style)
  }

  /// Positions child `i`, relative to this widget's top-left.
  pub fn place_child(&mut self, i: usize, at: Offset) {
    self.kids[i].offset.set(at);
  }

  pub fn child_size(&self, i: usize) -> Size {
    self.kids[i].size.get()
  }

  /// Flex factor of child `i`, if it is `Expanded` / `Flexible` / `Spacer`.
  pub fn child_flex(&self, i: usize) -> Option<Flex> {
    self.child_data::<Flex>(i)
  }

  pub fn measure_text(&self, text: &str, paint: &TextPaint) -> Size {
    self.renderer.measure_text(text, paint)
  }

  pub fn theme(&self) -> &ThemeData {
    self.theme
  }

  /// The text style in effect at this widget.
  pub fn text_style(&self) -> &TextStyle {
    self.style
  }

  pub fn text_paint(&self) -> TextPaint {
    self.style.resolve(self.theme)
  }

  /// Default layout: every child gets the same constraints and sits at the top-left;
  /// the size is the largest child (or the smallest allowed size with no children).
  pub fn layout_stacked(&mut self, constraints: Constraints) -> Size {
    let mut size = constraints.smallest();
    for i in 0..self.child_count() {
      let s = self.layout_child(i, constraints);
      size.width = size.width.max(s.width);
      size.height = size.height.max(s.height);
    }
    size
  }

  /// Lays out the only child inside `padding` and returns the padded size.
  pub fn layout_padded(&mut self, constraints: Constraints, padding: Padding) -> Size {
    if self.child_count() == 0 {
      return constraints
        .constrain(Size::new(padding.left + padding.right, padding.top + padding.bottom));
    }
    let s = self.layout_child(0, constraints.deflate(padding).loosen());
    self.place_child(0, Offset::new(padding.left, padding.top));
    Size::new(s.width + padding.left + padding.right, s.height + padding.top + padding.bottom)
  }
}

// ---------------------------------------------------------------------------
// Paint
// ---------------------------------------------------------------------------

/// What a widget's `paint` gets: the recorder, where it was laid out, and its children.
pub struct PaintCx<'a> {
  pub renderer: Recorder<'a>,
  pub(crate) rect: Rect,
  pub(crate) kids: &'a [Rc<Node>],
  pub(crate) theme: &'a ThemeData,
  pub(crate) style: &'a TextStyle,
  /// Inside the focused `InputEvent` (or it is this widget).
  pub(crate) focused: bool,
  pub(crate) state: &'a RefCell<Option<Rc<dyn Any>>>,
  pub(crate) backend: &'a dyn Renderer,
  pub(crate) scene: &'a mut Scene,
  pub(crate) overlays: &'a mut VecDeque<(Rc<Node>, Offset, bool)>,
  pub(crate) events: &'a mut Vec<WalkEvent>,
}

impl PaintCx<'_> {
  /// This widget's state (see `LayoutCx::state`).
  pub fn state<T: Default + 'static>(&self) -> Rc<RefCell<T>> {
    node_state(self.state)
  }

  pub fn child_count(&self) -> usize {
    self.kids.len()
  }

  /// Paints every child, in order (later ones on top).
  pub fn paint_children(&mut self) {
    for i in 0..self.kids.len() {
      self.paint_child(i);
    }
  }

  /// Paints child `i` now.
  pub fn paint_child(&mut self, i: usize) {
    self.renderer.child(self.kids[i].layer_id);
    self.events.push(WalkEvent::Child(i));
    self.kids[i].walk_paint(
      self.rect.origin(),
      self.focused,
      self.scene,
      self.backend,
      self.overlays,
    );
  }

  /// Paints child `i` after the whole tree, above everything (menus, popups).
  pub fn paint_child_on_top(&mut self, i: usize) {
    self.renderer.overlay(self.kids[i].layer_id);
    self.events.push(WalkEvent::Overlay(i));
    self.overlays.push_back((self.kids[i].clone(), self.rect.origin(), self.focused));
  }

  /// This widget's bounds on the canvas.
  pub fn rect(&self) -> Rect {
    self.rect
  }

  /// Bounds of child `i` on the canvas.
  pub fn child_rect(&self, i: usize) -> Rect {
    let kid = &self.kids[i];
    Rect::from_origin(self.rect.origin() + kid.offset.get(), kid.size.get())
  }

  pub fn theme(&self) -> &ThemeData {
    self.theme
  }

  pub fn text_style(&self) -> &TextStyle {
    self.style
  }

  pub fn text_paint(&self) -> TextPaint {
    self.style.resolve(self.theme)
  }

  /// Whether this widget is inside the keyboard-focused `InputEvent`.
  pub fn is_focused(&self) -> bool {
    self.focused
  }

  /// Adds this widget to the Tab order under `id` (a `global_events` focus id);
  /// returns whether it has the focus. Its subtree then sees `is_focused`.
  pub(crate) fn register_focus(&mut self, id: &str) -> bool {
    let focused = focus::register(id, self.rect);
    self.events.push(WalkEvent::Focus { id: id.to_string(), was_focused: focused });
    self.focused |= focused;
    focused
  }
}

// ---------------------------------------------------------------------------
// Focus
// ---------------------------------------------------------------------------

/// Tab order over the focusable `InputEvent`s of the last painted frame.
///
/// Which listener has the focus is `global_events`' focus id; this module keeps the
/// order and moves that id. A focusable widget rebuilt with a new id keeps the focus
/// by its position.
pub mod focus {
  use super::*;
  use crate::global_events;

  #[derive(Default)]
  struct State {
    entries: Vec<(String, Rect)>,
    current: Option<usize>,
    /// The next widget to register takes the focus (`request_next`).
    pending: bool,
    /// The focus id this module last set (to tell it from one the app set).
    synced: Option<String>,
  }

  thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
  }

  pub(crate) fn begin_frame() {
    STATE.with(|s| {
      let mut s = s.borrow_mut();
      s.entries.clear();
      s.pending = false;
    });
  }

  pub(crate) fn is_pending() -> bool {
    STATE.with(|s| s.borrow().pending)
  }

  pub(crate) fn end_frame() {
    let focused = global_events.current_focus();
    let resync = STATE.with(|s| {
      let mut s = s.borrow_mut();
      let present = focused.as_ref().is_some_and(|f| s.entries.iter().any(|e| &e.0 == f));
      if present || focused.is_none() {
        if focused.is_none() {
          s.current = None;
        }
        return None;
      }
      if focused != s.synced {
        // The app focused something outside the Tab order.
        s.current = None;
        return None;
      }
      // Our widget was rebuilt with a new id: keep the focus at its position.
      let len = s.entries.len();
      s.current = s.current.map(|i| i.min(len.saturating_sub(1))).filter(|_| len > 0);
      let id = s.current.map(|i| s.entries[i].0.clone());
      s.synced = id.clone();
      Some(id)
    });
    if let Some(id) = resync {
      global_events.set_focus(id);
    }
  }

  /// Adds a focusable widget; returns whether it has the focus.
  pub(crate) fn register(id: &str, rect: Rect) -> bool {
    let take = STATE.with(|s| {
      let mut s = s.borrow_mut();
      s.entries.push((id.to_string(), rect));
      std::mem::take(&mut s.pending)
    });
    if take {
      focus_id(id);
    }
    let focused = global_events.is_focused(id);
    if focused {
      STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.current = Some(s.entries.len() - 1);
      });
    }
    focused
  }

  fn focus_id(id: &str) {
    STATE.with(|s| s.borrow_mut().synced = Some(id.to_string()));
    global_events.set_focus(Some(id));
  }

  /// Gives focus to the next focusable widget painted after this call. A menu calls it
  /// from `paint` when it opens, so its first item takes the focus.
  pub fn request_next() {
    STATE.with(|s| s.borrow_mut().pending = true);
  }

  /// Focuses the `index`-th focusable widget in paint order (`None` clears focus).
  pub fn set(index: Option<usize>) {
    let id = STATE.with(|s| {
      let mut s = s.borrow_mut();
      match index {
        Some(i) if i < s.entries.len() => {
          s.current = Some(i);
          Some(s.entries[i].0.clone())
        }
        Some(_) => s.current.and_then(|i| s.entries.get(i)).map(|e| e.0.clone()),
        None => {
          s.current = None;
          None
        }
      }
    });
    match id {
      Some(id) => focus_id(&id),
      None if index.is_none() => {
        STATE.with(|s| s.borrow_mut().synced = None);
        global_events.set_focus(None::<String>);
      }
      None => {}
    }
  }

  /// Moves focus to the next focusable widget (wraps around).
  pub fn next() {
    let len = count();
    if len > 0 {
      set(Some(current().map_or(0, |i| (i + 1) % len)));
    }
  }

  /// Moves focus to the previous focusable widget (wraps around).
  pub fn previous() {
    let len = count();
    if len > 0 {
      set(Some(current().map_or(len - 1, |i| (i + len - 1) % len)));
    }
  }

  pub fn current() -> Option<usize> {
    STATE.with(|s| s.borrow().current)
  }

  /// Number of focusable widgets in the last frame.
  pub fn count() -> usize {
    STATE.with(|s| s.borrow().entries.len())
  }

  /// Bounds of the focused widget in the last frame.
  pub fn current_rect() -> Option<Rect> {
    STATE.with(|s| {
      let s = s.borrow();
      s.current.and_then(|i| s.entries.get(i)).map(|e| e.1)
    })
  }
}
