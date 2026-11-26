//! Retained tree: one `Node` per mounted widget. A node caches what its
//! widget built, and rebuilds only when a signal it read has changed.
//! Each frame it is laid out and painted (see `layout.rs`).
//!
//! When a node rebuilds, its new child widgets are matched against its current
//! child nodes (by key, else by position and type). A matched node is updated in
//! place: it keeps its state, its focus and its own children, which are matched
//! the same way. Only unmatched widgets are mounted, and only unmatched nodes are
//! unmounted.

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Write;
use std::rc::Rc;

use crate::layout::{Constraints, LayoutCx, PaintCx, node_state};
use crate::render::{Offset, Rect, Renderer, Size};
use crate::signal::{Observer, track};
use crate::task::with_owner;
use crate::theme::{TextStyle, ThemeData};
use crate::{Context, Widget};

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
}

/// What `Element::attach`, `dispose` and `scope` get: the node's context and state.
pub struct NodeCx<'a> {
  context: &'a Context,
  state: &'a RefCell<Option<Rc<dyn Any>>>,
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
}

impl Node {
  pub fn mount(widget: Widget, context: Context) -> Rc<Node> {
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
      widget.element.attach(&NodeCx { context: &context, state: &self.state })
    });
  }

  /// Puts `widget` (same type as the current one) on this node, built in `context`,
  /// and rebuilds the node, unless both are the very same as before.
  fn update_with(&self, widget: Widget, context: Context) {
    let unchanged = Rc::ptr_eq(&self.widget.borrow().element, &widget.element)
      && self.context.borrow().same(&context);
    if unchanged {
      // Same widget value in the same context builds the same thing: keep the subtree.
      // If a signal it read changed, the `update` walk rebuilds it.
      return;
    }
    *self.widget.borrow_mut() = widget;
    *self.context.borrow_mut() = context;
    self.attach();
    self.rebuild();
  }

  fn rebuild(&self) {
    let observer = Observer::new();
    *self.observer.borrow_mut() = observer.clone();

    let (widget, context) = (self.widget(), self.context());
    // Tasks started while building belong to the nearest `Provider` above.
    let (built, scope, children) = with_owner(context.task_owner(), || {
      let (built, scope) = track(&observer, || {
        let built = widget.element.build(&context);
        let scope = widget.element.scope(&NodeCx { context: &context, state: &self.state });
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
        None => Node::mount(w, context.clone()),
      })
      .collect();
    *self.kids.borrow_mut() = kids;
  }

  /// Rebuilds the nodes whose signals changed; clean nodes are only walked.
  /// Returns whether anything was rebuilt (i.e. whether to repaint).
  pub fn update(&self) -> bool {
    let mut rebuilt = false;
    if self.observer.borrow().is_dirty() {
      self.rebuild();
      rebuilt = true;
    }
    // Children kept by the rebuild (or untouched by it) may be dirty themselves.
    let kids = self.kids.borrow().clone();
    for kid in kids {
      rebuilt |= kid.update();
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

  /// Paints this node; the widget paints its children through `PaintCx`.
  /// `parent_origin` is the parent's top-left on the canvas; `focused` is whether an
  /// ancestor `InputEvent` has the keyboard focus.
  pub(crate) fn paint(&self, parent_origin: Offset, renderer: &mut dyn Renderer, focused: bool) {
    let rect = self.rect_in(parent_origin);
    let widget = self.widget();
    let theme = ThemeData::of(&self.context.borrow());
    let style = self.style.borrow().clone();
    let kids = self.kids.borrow().clone();
    let observer = self.observer.borrow().clone();
    track(&observer, || {
      let mut cx = PaintCx {
        renderer,
        rect,
        kids: &kids,
        theme: &theme,
        style: &style,
        focused,
        state: &self.state,
      };
      widget.element.paint(&mut cx);
    });
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
    let widget = self.widget.get_mut().clone();
    let context = self.context.get_mut().clone();
    widget.element.dispose(&NodeCx { context: &context, state: &self.state });
  }
}
