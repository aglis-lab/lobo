//! A small Flutter-style widget framework.
//!
//! Define widgets in your own crate:
//!
//! ```ignore
//! use widgets::{Context, Element, Widget, run, ui};
//!
//! #[derive(Widget)]
//! struct Hello {}
//!
//! impl Element for Hello {
//!     fn build(&self, _context: &Context) -> Option<Widget> {
//!         Some(ui!(Container { child: Text { text: "hi" } }))
//!     }
//! }
//!
//! fn main() {
//!     // Pass any `Renderer` backend to `run`:
//!     // run(ui!(Hello {}), &mut backend);
//! }
//! ```

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use anymap3::AnyMap;

use crate::layout::{Constraints, LayoutCx, PaintCx, focus};
use crate::render::{Renderer, Scene, Size};
use crate::signal::set_waker;
use crate::task::TaskGroup;
use crate::term::{Input, InputMode, RawTerminal, spawn_line_reader};
use crate::tree::{Node, NodeCx};
use crate::{EventKey, EventResult, keys};

pub use lobo_macros::{Model, Widget, ui};

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// InnerContext is used to store values / act as a key-value storage.
pub trait InnerContext {
  fn get(&self, key: &str) -> Option<String>;
  /// Takes `&self` so widgets can update state while holding a `&Context`.
  fn set(&self, key: &str, value: String);
}

pub struct MapContext(pub RefCell<HashMap<String, String>>);

impl InnerContext for MapContext {
  fn get(&self, key: &str) -> Option<String> {
    self.0.borrow().get(key).cloned()
  }

  fn set(&self, key: &str, value: String) {
    self.0.borrow_mut().insert(key.to_string(), value);
  }
}

/// Values provided by one `Provider`, chained to the scope above it.
struct Scope {
  /// Shared, so a `Provider` can keep its values when it is re-parented.
  values: Rc<RefCell<AnyMap>>,
  /// The tasks owned by the `Provider` that made this scope (`None` for `Theme`).
  tasks: Option<TaskGroup>,
  parent: Option<Rc<Scope>>,
}

/// Cheap to clone. Every node holds the context its widget was built in;
/// a `Provider` gives its subtree a child context with one more scope.
#[derive(Clone)]
pub struct Context {
  pub inner: Rc<dyn InnerContext>,
  scope: Option<Rc<Scope>>,
}

impl Context {
  pub fn new(inner: impl InnerContext + 'static) -> Self {
    Context { inner: Rc::new(inner), scope: None }
  }

  /// Child context that also sees `values`; shadows the same types above.
  pub fn with_scope(&self, values: AnyMap) -> Context {
    self.with_shared_scope(Rc::new(RefCell::new(values)), None)
  }

  pub(crate) fn with_shared_scope(
    &self,
    values: Rc<RefCell<AnyMap>>,
    tasks: Option<TaskGroup>,
  ) -> Context {
    let parent = self.scope.clone();
    Context { inner: self.inner.clone(), scope: Some(Rc::new(Scope { values, tasks, parent })) }
  }

  /// Tasks of the nearest `Provider` above: the owner of tasks started here.
  pub(crate) fn task_owner(&self) -> Option<TaskGroup> {
    let mut scope = self.scope.as_deref();
    while let Some(s) = scope {
      if let Some(tasks) = &s.tasks {
        return Some(tasks.clone());
      }
      scope = s.parent.as_deref();
    }
    None
  }

  /// Whether both are the same context (same store and same scope chain), not just equal.
  pub(crate) fn same(&self, other: &Context) -> bool {
    Rc::ptr_eq(&self.inner, &other.inner)
      && match (&self.scope, &other.scope) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
      }
  }

  /// Nearest provided `T` above this widget, if any.
  pub fn try_read<T: 'static>(&self) -> Option<Rc<T>> {
    let mut scope = self.scope.as_deref();
    while let Some(s) = scope {
      if let Some(value) = s.values.borrow_mut().get::<Rc<T>>() {
        return Some(value.clone());
      }
      scope = s.parent.as_deref();
    }
    None
  }

  /// Like `try_read`, but panics if no `Provider` of `T` is above.
  pub fn read<T: 'static>(&self) -> Rc<T> {
    self.try_read().unwrap_or_else(|| {
      panic!("no Provider for `{}` above this widget", std::any::type_name::<T>())
    })
  }

  pub fn get(&self, key: &str) -> Option<String> {
    self.inner.get(key)
  }

  pub fn set(&self, key: &str, value: impl ToString) {
    self.inner.set(key, value.to_string());
  }
}

/// Argument of a setter for an optional (`Option<T>`) widget field: either something
/// convertible to `T` (set) or an `Option<T>` (set or unset). `M` only selects the impl.
pub trait IntoOptionArg<T, M> {
  fn into_option_arg(self) -> Option<T>;
}

#[doc(hidden)]
pub struct OptionValue;
#[doc(hidden)]
pub struct OptionAsIs;

impl<T, U: Into<T>> IntoOptionArg<T, OptionValue> for U {
  fn into_option_arg(self) -> Option<T> {
    Some(self.into())
  }
}

impl<T> IntoOptionArg<T, OptionAsIs> for Option<T> {
  fn into_option_arg(self) -> Option<T> {
    self
  }
}

/// Runs an `!Send` future on the UI thread, so it can read and write
/// `Signal`s directly — no handle, no channel, no `Send`.
///
/// The future must own what it uses (no borrows of temporaries), so take the
/// `Rc` from `context.read` and call a method that takes `self: Rc<Self>`:
///
/// ```ignore
/// context.read::<CounterController>().decrease().run_async();
/// ```
///
/// The future is polled by the run loop, so an `.await` inside it does not
/// block the UI. It is aborted if the `Provider` that owns it is unmounted
/// (see `task` ownership, and `detached`). Panics if called outside `run`
/// (there is no UI executor).
pub trait RunAsync: Future + Sized + 'static {
  fn run_async(self) -> tokio::task::JoinHandle<Self::Output>
  where
    Self::Output: 'static,
  {
    crate::task::spawn_owned(self)
  }
}

impl<F: Future + 'static> RunAsync for F {}

impl Default for Context {
  fn default() -> Self {
    Context::new(MapContext(RefCell::new(HashMap::new())))
  }
}

// ---------------------------------------------------------------------------
// Core traits
// ---------------------------------------------------------------------------

/// A widget. Every method has a default, so a composite widget implements only `build`,
/// and a primitive implements `children`, `layout` and `paint`.
///
/// Keyboard input is not part of this trait: wrap content in a focusable `InputEvent`,
/// usually with `focusable(handler, child)` and a `KeyHandler`.
pub trait Element {
  /// Composite widgets expand into other widgets (like Flutter's `build`).
  /// Primitive widgets return `None` and lay out / paint themselves.
  fn build(&self, _context: &Context) -> Option<Widget> {
    None
  }

  /// Child widgets of a primitive (composites use `build` instead).
  /// The framework mounts and rebuilds them; `layout` and `paint` place and draw them.
  fn children(&self) -> Vec<Widget> {
    Vec::new()
  }

  /// Context for this widget's descendants; `None` passes the parent's (`cx.context()`)
  /// through. Used by `Provider` and `Theme`. Runs under the same tracking as `build`.
  fn scope(&self, _cx: &NodeCx) -> Option<Context> {
    None
  }

  /// Called each time this widget value is put on a node: when the node is mounted,
  /// and when a rebuilt parent updates the node in place with this widget (same type,
  /// same position or key). Keep what must outlive one widget value in `cx.state()`:
  /// the node keeps it until it is unmounted, then drops it (so `Drop` on the state
  /// is the place to release resources).
  fn attach(&self, _cx: &NodeCx) {}

  /// Called once when the node is unmounted, with its current widget.
  fn dispose(&self, _cx: &NodeCx) {}

  /// Sizes this widget within `constraints` and places its children (see `LayoutCx`).
  /// The default stacks every child at the top-left with the same constraints,
  /// which suits composites (one child) and simple wrappers.
  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    cx.layout_stacked(constraints)
  }

  /// Draws this widget. Children are drawn where the widget calls
  /// `cx.paint_children()` (or `paint_child` / `paint_child_on_top`), so it can
  /// draw under them, over them, or clip them. The default only paints the children.
  fn paint(&self, cx: &mut PaintCx) {
    cx.paint_children();
  }

  /// Data for the parent's layout, read with `LayoutCx::child_data`
  /// (e.g. `Positioned` edges, the `Flex` of `Expanded`).
  fn parent_data(&self) -> Option<Box<dyn std::any::Any>> {
    None
  }
}

/// Cheaply clonable handle to any element, with the element's type and an optional key.
/// A struct (instead of a bare `Rc<dyn Element>` alias) so `From<E: Element>` is
/// allowed by orphan rules.
///
/// When a parent rebuilds, each new child widget updates the existing node of the same
/// type at the same position (or with the same key), so the node keeps its state,
/// focus and subtree; see `with_key`.
#[derive(Clone)]
pub struct Widget {
  pub(crate) element: Rc<dyn Element>,
  name: &'static str,
  pub(crate) type_id: TypeId,
  pub(crate) key: Option<u64>,
}

impl Widget {
  /// The element's struct name, without module path or generics (`lobo::Text` -> `Text`).
  pub fn name(&self) -> &'static str {
    self.name
  }

  /// The element itself (e.g. to read its `parent_data`).
  pub fn element(&self) -> &dyn Element {
    &*self.element
  }

  /// Identifies this widget among its siblings, like Flutter's `Key`. On a rebuild a keyed
  /// widget updates the old sibling node with the same key and type wherever it was, and
  /// never an unkeyed one. Use it for items that can be inserted, removed or reordered,
  /// so each item keeps its own state and focus; use a different key to force a fresh node.
  pub fn with_key<K: Hash + 'static>(mut self, key: K) -> Self {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    TypeId::of::<K>().hash(&mut hasher);
    key.hash(&mut hasher);
    self.key = Some(hasher.finish());
    self
  }
}

impl<E: Element + 'static> From<E> for Widget {
  fn from(element: E) -> Self {
    let full = std::any::type_name::<E>();
    let base = full.split('<').next().unwrap_or(full);
    Widget {
      element: Rc::new(element),
      name: base.rsplit("::").next().unwrap_or(base),
      type_id: TypeId::of::<E>(),
      key: None,
    }
  }
}

thread_local! {
  static EXIT_NOTIFIER: RefCell<Option<Rc<tokio::sync::Notify>>> = const { RefCell::new(None) };
}

/// Requests the running UI loop to terminate.
pub fn exit() {
  EXIT_NOTIFIER.with(|n| {
    if let Some(notifier) = n.borrow().as_ref() {
      notifier.notify_one();
    }
  });
}

// ---------------------------------------------------------------------------
// Run loop
// ---------------------------------------------------------------------------

/// Turns one input line into key presses. Escape sequences for arrows, Shift-Tab and
/// Delete become the `keys` constants; an empty line (just Enter) is `keys::ENTER`.
pub fn parse_keys(line: &str) -> Vec<char> {
  if line.is_empty() {
    return vec![keys::ENTER];
  }
  let chars: Vec<char> = line.chars().collect();
  let mut out = Vec::new();
  let mut i = 0;
  while i < chars.len() {
    if chars[i] == '\x1b' && i + 2 < chars.len() && (chars[i + 1] == '[' || chars[i + 1] == 'O') {
      let key = match chars[i + 2] {
        'A' => Some((keys::UP, 3)),
        'B' => Some((keys::DOWN, 3)),
        'C' => Some((keys::RIGHT, 3)),
        'D' => Some((keys::LEFT, 3)),
        'Z' => Some((keys::BACK_TAB, 3)),
        '3' if chars.get(i + 3) == Some(&'~') => Some((keys::DELETE, 4)),
        _ => None,
      };
      if let Some((key, len)) = key {
        out.push(key);
        i += len;
        continue;
      }
    }
    out.push(chars[i]);
    i += 1;
  }
  out
}

/// Delivers one key press: Tab / Shift-Tab move the focus; any other key goes to
/// `global_events` (the focused listener first, then the unfocused ones).
/// An unhandled Escape clears the focus.
pub fn dispatch_key_event(key: char) -> EventResult {
  dispatch_event(&EventKey::new(key))
}

/// Like `dispatch_key_event`, with modifiers (raw mode reports Ctrl / Alt / Shift).
pub fn dispatch_event(event: &EventKey) -> EventResult {
  match event.key {
    keys::TAB => {
      focus::next();
      return EventResult::Handled;
    }
    keys::BACK_TAB => {
      focus::previous();
      return EventResult::Handled;
    }
    _ => {}
  }
  let result = crate::global_events.dispatch(event);
  if result == EventResult::Ignored && event.key == keys::ESCAPE && focus::current().is_some() {
    focus::set(None);
    return EventResult::Handled;
  }
  result
}

/// Mounts the tree once, then loops on this thread: key presses are dispatched,
/// and any signal change or `Repaint::mark()` wakes the loop to rebuild, lay out,
/// re-record changed layers, and present the damaged region to `backend`. `exit()`,
/// Ctrl+C, Ctrl+D or the end of input quits.
///
/// When stdin and stdout are terminals, input is raw: every key acts at once, on the
/// alternate screen, and the terminal is restored on exit. Otherwise (piped input, or
/// `LOBO_INPUT=line`) it is read a line at a time; see `InputMode`.
/// Tab moves the keyboard focus; Enter or space activates the focused widget.
pub fn run(app: impl Into<Widget>, backend: &mut dyn Renderer) {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .expect("failed to build tokio runtime");

  let local = tokio::task::LocalSet::new();
  runtime.block_on(local.run_until(run_loop(app.into(), backend)));
  set_waker(None);
  EXIT_NOTIFIER.with(|n| *n.borrow_mut() = None);
}

async fn run_loop(app: Widget, backend: &mut dyn Renderer) {
  let root = Node::mount(app, Context::default());
  let mut scene = Scene::new();

  let redraw = Rc::new(tokio::sync::Notify::new());
  {
    let redraw = redraw.clone();
    set_waker(Some(Rc::new(move || redraw.notify_one())));
  }

  let exit_notify = Rc::new(tokio::sync::Notify::new());
  EXIT_NOTIFIER.with(|n| *n.borrow_mut() = Some(exit_notify.clone()));

  // Input is read on its own thread and arrives over a channel.
  let (tx, mut inputs) = tokio::sync::mpsc::unbounded_channel::<Input>();
  let mut input_mode = InputMode::from_env();
  // Dropped when this function returns, which restores the terminal.
  let mut _raw = None;
  if input_mode == InputMode::Raw {
    match RawTerminal::enter() {
      Ok(raw) => {
        raw.spawn_reader(tx.clone());
        _raw = Some(raw);
      }
      Err(e) => {
        log::warn!("raw terminal mode unavailable ({e}); reading lines instead");
        input_mode = InputMode::Line;
      }
    }
  }
  if input_mode == InputMode::Line {
    spawn_line_reader(tx);
  }

  root.present(&mut scene, backend);
  loop {
    tokio::select! {
      _ = exit_notify.notified() => {
        return;
      }
      input = inputs.recv() => {
        let keys = match input {
          None | Some(Input::Quit) => return,
          Some(Input::Key(key)) => vec![key],
          Some(Input::Line(line)) => parse_keys(&line).into_iter().map(EventKey::new).collect(),
          Some(Input::Resize) => Vec::new(),
        };
        for key in keys {
          dispatch_event(&key);
          // Rebuild so the next key of the same line reaches the updated widgets.
          root.update();
        }
      }
      // A signal changed or `Repaint::mark` fired outside key handling.
      _ = redraw.notified() => {}
    }
    root.update();
    root.present(&mut scene, backend);
  }
}
