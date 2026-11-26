// ---------------------------------------------------------------------------
// Built-in widgets
// ---------------------------------------------------------------------------

// `Widget` is both the struct and the derive macro; this imports both.
use crate::basic::{CrossAxisAlignment, MainAxisAlignment, MainAxisSize, flex_layout};
use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::render::{Border, BoxPaint, Color, Offset, Size, TextPaint};
use crate::task::{TaskGroup, with_owner};
use crate::theme::TextStyle;
use crate::{Alignment, Axis, Context, Element, NodeCx, Widget};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// Built-in widgets are `#[non_exhaustive]`: outside this crate they can only
// be built with `ui!`, so there is one way to write a widget tree.

/// Text, wrapped at word boundaries to the available width.
#[derive(Widget)]
#[non_exhaustive]
pub struct Text {
  pub text: String,
  /// Merged over the inherited style (theme default, then enclosing widgets).
  #[widget(optional)]
  pub style: TextStyle,
  /// Maximum number of lines; 0 is unlimited. Cut text ends with `…`.
  #[widget(optional)]
  pub max_lines: usize,
}

impl Text {
  fn lines(
    &self,
    max_width: f32,
    paint: &TextPaint,
    measure: &dyn Fn(&str, &TextPaint) -> Size,
  ) -> Vec<String> {
    let mut lines = wrap_text(&self.text, max_width, &|s| measure(s, paint).width);
    if self.max_lines > 0 && lines.len() > self.max_lines {
      lines.truncate(self.max_lines);
      if let Some(last) = lines.last_mut() {
        last.push('…');
        while last.chars().count() > 1 && measure(last, paint).width > max_width {
          let cut = last.chars().count() - 2;
          *last = last.chars().take(cut).chain(['…']).collect();
        }
      }
    }
    lines
  }
}

impl Element for Text {
  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let paint = self.style.merge(cx.text_style()).resolve(cx.theme());
    let renderer = cx.renderer;
    let lines = self.lines(constraints.max_width, &paint, &|s, p| renderer.measure_text(s, p));
    let line_height = renderer.measure_text("", &paint).height;
    let width = lines.iter().map(|l| renderer.measure_text(l, &paint).width).fold(0.0, f32::max);
    Size::new(width, line_height * lines.len().max(1) as f32)
  }

  fn paint(&self, cx: &mut PaintCx) {
    let paint = self.style.merge(cx.text_style()).resolve(cx.theme());
    let rect = cx.rect();
    let line_height = cx.renderer.measure_text("", &paint).height;
    // Re-wrapping at the laid-out width gives the same lines as layout did.
    let lines = {
      let r = &*cx.renderer;
      self.lines(rect.width + 0.5, &paint, &|s, p| r.measure_text(s, p))
    };
    for (i, line) in lines.iter().enumerate() {
      cx.renderer.draw_text(Offset::new(rect.x, rect.y + i as f32 * line_height), line, &paint);
    }
  }
}

/// Greedy word wrap. Explicit `\n` always breaks; a word wider than the line is split.
pub(crate) fn wrap_text(text: &str, max_width: f32, width: &dyn Fn(&str) -> f32) -> Vec<String> {
  let mut out = Vec::new();
  for paragraph in text.split('\n') {
    let mut line = String::new();
    for word in paragraph.split(' ') {
      let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
      if width(&candidate) <= max_width || line.is_empty() && width(word) <= max_width {
        line = candidate;
        continue;
      }
      if !line.is_empty() {
        out.push(std::mem::take(&mut line));
      }
      // The word alone may still be too wide: split it by characters.
      for ch in word.chars() {
        let next = format!("{line}{ch}");
        if !line.is_empty() && width(&next) > max_width {
          out.push(std::mem::take(&mut line));
          line.push(ch);
        } else {
          line = next;
        }
      }
    }
    out.push(line);
  }
  out
}

// Implement Signal Builder which record make signal only rebuild on child widgets
#[derive(Widget)]
#[non_exhaustive]
pub struct SignalBuilder {
  pub builder: Rc<dyn Fn(&Context) -> Option<Widget>>,
}

impl Element for SignalBuilder {
  fn build(&self, context: &Context) -> Option<Widget> {
    log::trace!("SignalBuilder being called");

    self.builder.as_ref()(context)
  }
}

// `optional` on the struct: every field may be omitted (defaults to 0.0).
// Padding is plain data, not an Element.
#[derive(Widget, Default, Clone, Copy, Debug, PartialEq)]
#[widget(optional)]
pub struct Padding {
  pub left: f32,
  pub top: f32,
  pub right: f32,
  pub bottom: f32,
}

impl Padding {
  pub const ZERO: Padding = Padding { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };

  pub const fn all(v: f32) -> Self {
    Padding { left: v, top: v, right: v, bottom: v }
  }

  pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
    Padding { left: horizontal, top: vertical, right: horizontal, bottom: vertical }
  }

  pub fn horizontal(&self) -> f32 {
    self.left + self.right
  }

  pub fn vertical(&self) -> f32 {
    self.top + self.bottom
  }
}

impl std::fmt::Display for Padding {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "l:{}, t:{}, r:{}, b:{}", self.left, self.top, self.right, self.bottom)
  }
}

/// A box: optional size, padding, margin, background, border and alignment.
///
/// Sizes to its child plus padding; with no child it fills the available space
/// (or its `width` / `height`).
#[derive(Widget)]
#[non_exhaustive]
pub struct Container {
  #[widget(optional)]
  pub padding: Padding,
  /// Space outside the background.
  #[widget(optional)]
  pub margin: Padding,
  #[widget(optional)]
  pub color: Option<Color>,
  #[widget(optional)]
  pub border: Option<Border>,
  #[widget(optional)]
  pub radius: f32,
  #[widget(optional)]
  pub width: Option<f32>,
  #[widget(optional)]
  pub height: Option<f32>,
  /// Where the child sits when the box is larger than it; top-left if unset.
  #[widget(optional)]
  pub alignment: Option<Alignment>,
  #[widget(optional)]
  pub child: Option<Widget>,
}

impl Element for Container {
  fn children(&self) -> Vec<Widget> {
    self.child.iter().cloned().collect()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let outer = constraints.deflate(self.margin).tighten(self.width, self.height);
    let inner = outer.deflate(self.padding);
    let size = if cx.child_count() > 0 {
      let child = cx.layout_child(0, inner.loosen());
      let mut size =
        Size::new(child.width + self.padding.horizontal(), child.height + self.padding.vertical());
      if self.alignment.is_some() {
        // An aligned child gets the whole box when it's bounded.
        if outer.has_bounded_width() {
          size.width = outer.max_width;
        }
        if outer.has_bounded_height() {
          size.height = outer.max_height;
        }
      }
      let size = outer.constrain(size);
      let free = Size::new(
        size.width - self.padding.horizontal() - child.width,
        size.height - self.padding.vertical() - child.height,
      );
      let a = self.alignment.unwrap_or(Alignment::TOP_LEFT);
      cx.place_child(
        0,
        Offset::new(
          self.margin.left + self.padding.left + a.offset(free.width),
          self.margin.top + self.padding.top + a.offset_y(free.height),
        ),
      );
      size
    } else {
      let want = Size::new(
        if outer.has_bounded_width() { outer.max_width } else { self.padding.horizontal() },
        if outer.has_bounded_height() { outer.max_height } else { self.padding.vertical() },
      );
      outer.constrain(want)
    };
    Size::new(size.width + self.margin.horizontal(), size.height + self.margin.vertical())
  }

  fn paint(&self, cx: &mut PaintCx) {
    let rect = cx.rect().deflate(self.margin);
    let paint = BoxPaint {
      color: self.color.unwrap_or(Color::TRANSPARENT),
      border: self.border,
      radius: self.radius,
      elevation: 0.0,
    };
    cx.renderer.draw_box(rect, &paint);
    cx.paint_children();
  }
}

/// Lays children out vertically. Fills the available height by default
/// (`main_axis_size: MainAxisSize::Min` shrinks it to the children).
#[derive(Widget)]
#[non_exhaustive]
pub struct Column {
  /// Gap between children.
  #[widget(optional)]
  pub spacing: f32,
  #[widget(optional)]
  pub main_axis_alignment: MainAxisAlignment,
  #[widget(optional)]
  pub cross_axis_alignment: CrossAxisAlignment,
  #[widget(optional)]
  pub main_axis_size: MainAxisSize,
  pub children: Vec<Widget>,
}

impl Element for Column {
  fn children(&self) -> Vec<Widget> {
    self.children.clone()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    flex_layout(
      cx,
      constraints,
      Axis::Vertical,
      self.spacing,
      self.main_axis_alignment,
      self.cross_axis_alignment,
      self.main_axis_size,
    )
  }
}

/// Special keys, as delivered in `EventKey::key`. Arrows, Shift-Tab and Delete
/// arrive as escape sequences and are mapped to Unicode private-use characters.
pub mod keys {
  /// An empty input line (just Enter). Activates the focused widget.
  pub const ENTER: char = '\n';
  pub const TAB: char = '\t';
  pub const ESCAPE: char = '\u{1b}';
  pub const BACKSPACE: char = '\u{7f}';
  pub const UP: char = '\u{F700}';
  pub const DOWN: char = '\u{F701}';
  pub const LEFT: char = '\u{F702}';
  pub const RIGHT: char = '\u{F703}';
  pub const DELETE: char = '\u{F728}';
  pub const BACK_TAB: char = '\u{F72F}';

  /// Whether `key` activates a button: Enter or space.
  pub fn is_activate(key: char) -> bool {
    key == ENTER || key == ' '
  }
}

/// Modifier keys held during a key event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
  pub ctrl: bool,
  pub alt: bool,
  pub shift: bool,
}

impl Modifiers {
  pub const NONE: Modifiers = Modifiers { ctrl: false, alt: false, shift: false };
  pub const CTRL: Modifiers = Modifiers { ctrl: true, alt: false, shift: false };
  pub const ALT: Modifiers = Modifiers { ctrl: false, alt: true, shift: false };
  pub const SHIFT: Modifiers = Modifiers { ctrl: false, alt: false, shift: true };
}

/// The result of an event dispatch, indicating whether propagation should stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult {
  /// The event was handled; stop propagating to remaining listeners.
  Handled,
  /// The event was ignored; continue propagating to remaining listeners.
  Ignored,
}

impl From<()> for EventResult {
  fn from(_: ()) -> Self {
    EventResult::Ignored
  }
}

impl From<bool> for EventResult {
  fn from(handled: bool) -> Self {
    if handled { EventResult::Handled } else { EventResult::Ignored }
  }
}

#[derive(Clone, Debug)]
pub struct EventKey {
  pub key: char,
  pub modifiers: Modifiers,
  handled: Cell<bool>,
}

impl EventKey {
  pub fn new(key: char) -> Self {
    Self { key, modifiers: Modifiers::NONE, handled: Cell::new(false) }
  }

  pub fn with_modifiers(key: char, modifiers: Modifiers) -> Self {
    Self { key, modifiers, handled: Cell::new(false) }
  }

  /// Marks the event as handled, stopping further bubbling/propagation.
  pub fn handle(&self) {
    self.handled.set(true);
  }

  /// Whether this event has been handled.
  pub fn is_handled(&self) -> bool {
    self.handled.get()
  }

  /// Returns true if this event matches the given character with no modifiers.
  pub fn is_char(&self, c: char) -> bool {
    self.key == c && self.modifiers == Modifiers::NONE
  }

  /// Returns true if Ctrl is pressed.
  pub fn is_ctrl(&self) -> bool {
    self.modifiers.ctrl
  }

  /// Returns true if Alt is pressed.
  pub fn is_alt(&self) -> bool {
    self.modifiers.alt
  }

  /// Returns true if Shift is pressed.
  pub fn is_shift(&self) -> bool {
    self.modifiers.shift
  }
}

impl From<char> for EventKey {
  fn from(key: char) -> Self {
    Self::new(key)
  }
}

impl PartialEq for EventKey {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key && self.modifiers == other.modifiers
  }
}

impl Eq for EventKey {}

pub type EventCallback = Rc<dyn Fn(&EventKey)>;

#[derive(Clone)]
struct EventRegistration {
  callback: EventCallback,
  focus_id: Option<String>,
}

thread_local! {
  static LISTENERS: RefCell<Vec<EventRegistration>> = const { RefCell::new(Vec::new()) };
  static CURRENT_FOCUS: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub trait IntoEventListener {
  fn into_event_listener(self) -> EventCallback;
}

impl IntoEventListener for EventCallback {
  fn into_event_listener(self) -> EventCallback {
    self
  }
}

impl IntoEventListener for &EventCallback {
  fn into_event_listener(self) -> EventCallback {
    self.clone()
  }
}

impl IntoEventListener for Box<dyn Fn(&EventKey)> {
  fn into_event_listener(self) -> EventCallback {
    Rc::from(self)
  }
}

pub trait AsEventListener {
  fn as_listener_ptr(&self) -> *const ();
}

impl AsEventListener for EventCallback {
  fn as_listener_ptr(&self) -> *const () {
    Rc::as_ptr(self) as *const ()
  }
}

impl AsEventListener for &EventCallback {
  fn as_listener_ptr(&self) -> *const () {
    Rc::as_ptr(self) as *const ()
  }
}

/// Global dispatcher for input events, supporting Flutter-like event bubbling and focus.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlobalEvents;

#[allow(non_upper_case_globals)]
pub const global_events: GlobalEvents = GlobalEvents;

impl GlobalEvents {
  /// Registers a global event listener callback.
  pub fn push(&self, event: impl IntoEventListener) {
    self.register(event, None);
  }

  /// Registers an event listener callback with an optional focus identifier.
  pub fn register(&self, event: impl IntoEventListener, focus_id: Option<String>) {
    LISTENERS.with(|l| {
      l.borrow_mut().push(EventRegistration { callback: event.into_event_listener(), focus_id });
    });
  }

  /// Removes an event listener callback.
  pub fn remove(&self, target: impl AsEventListener) {
    let target_ptr = target.as_listener_ptr();
    LISTENERS.with(|l| {
      l.borrow_mut().retain(|item| Rc::as_ptr(&item.callback) as *const () != target_ptr);
    });
  }

  /// Sets the currently active focus. Pass `None` to clear focus.
  pub fn set_focus(&self, focus: Option<impl Into<String>>) {
    CURRENT_FOCUS.with(|f| {
      *f.borrow_mut() = focus.map(|s| s.into());
    });
  }

  /// Returns the currently active focus identifier, if any.
  pub fn current_focus(&self) -> Option<String> {
    CURRENT_FOCUS.with(|f| f.borrow().clone())
  }

  /// Checks if a specific identifier has active focus.
  pub fn is_focused(&self, focus_id: &str) -> bool {
    CURRENT_FOCUS.with(|f| f.borrow().as_deref() == Some(focus_id))
  }

  /// Dispatches an event: first to the listeners registered for the focused id, then
  /// to the listeners without a focus id, each in reverse-mount order (innermost first).
  /// Listeners for other focus ids are skipped. If a listener marks the event as
  /// handled (`event.handle()`), propagation stops.
  pub fn dispatch(&self, event: &EventKey) -> EventResult {
    let focus = CURRENT_FOCUS.with(|f| f.borrow().clone());
    let listeners: Vec<EventRegistration> = LISTENERS.with(|l| l.borrow().clone());
    let focused = listeners.iter().rev().filter(|r| r.focus_id.is_some() && r.focus_id == focus);
    let unfocused = listeners.iter().rev().filter(|r| r.focus_id.is_none());
    for reg in focused.chain(unfocused) {
      (reg.callback)(event);
      if event.is_handled() {
        return EventResult::Handled;
      }
    }
    EventResult::Ignored
  }

  /// Convenience helper to dispatch a key directly.
  pub fn dispatch_key(&self, key: char) -> EventResult {
    self.dispatch(&EventKey::new(key))
  }

  /// Returns the number of currently registered listeners.
  pub fn count(&self) -> usize {
    LISTENERS.with(|l| l.borrow().len())
  }

  /// Clears all registered listeners and resets active focus.
  pub fn clear(&self) {
    LISTENERS.with(|l| l.borrow_mut().clear());
    CURRENT_FOCUS.with(|f| *f.borrow_mut() = None);
  }
}

/// Listens to key presses for its subtree.
///
/// - Without `focus_id` / `focusable`, it receives every key nobody focused handled.
/// - With `focus_id`, it receives keys only while `global_events` has that focus id.
/// - With `focusable: true`, it also joins the Tab order (using `focus_id`, or an id of
///   its own), and its subtree paints as focused (`PaintCx::is_focused`) while it has
///   the focus. `focusable(handler, child)` builds one from a `KeyHandler`.
///
/// Call `event.handle()` to stop the key from reaching the next listener.
#[derive(Widget)]
#[non_exhaustive]
pub struct InputEvent {
  pub event: Rc<dyn Fn(&EventKey)>,
  #[widget(optional)]
  pub focus_id: Option<String>,
  #[widget(optional)]
  pub focusable: bool,
  #[widget(optional)]
  pub child: Option<Widget>,
}

/// What a mounted `InputEvent` keeps on its node.
#[derive(Default)]
struct InputState {
  listener: Option<Listener>,
  /// Allocated on first use; never reused, so a later widget can't inherit the focus.
  auto_id: Option<u64>,
}

/// A registration in `global_events`. It calls the current widget's `event`, so an
/// in-place update swaps the callback without moving the listener or changing its id.
struct Listener {
  id: Option<String>,
  /// The callback, and the owner of the tasks it starts (the nearest `Provider`).
  current: Rc<RefCell<(EventCallback, Option<TaskGroup>)>>,
  registered: EventCallback,
}

impl Drop for Listener {
  fn drop(&mut self) {
    global_events.remove(&self.registered);
  }
}

thread_local! {
  static NEXT_ID: Cell<u64> = const { Cell::new(0) };
}

impl InputEvent {
  /// The focus id it listens under, if any.
  fn id(&self, state: &mut InputState) -> Option<String> {
    match (&self.focus_id, self.focusable) {
      (Some(id), _) => Some(id.clone()),
      (None, true) => {
        let n = *state.auto_id.get_or_insert_with(|| {
          NEXT_ID.with(|c| {
            c.set(c.get() + 1);
            c.get()
          })
        });
        Some(format!("lobo:{n}"))
      }
      (None, false) => None,
    }
  }
}

impl Element for InputEvent {
  fn attach(&self, cx: &NodeCx) {
    let state = cx.state::<InputState>();
    let mut state = state.borrow_mut();
    let id = self.id(&mut state);
    let current = (self.event.clone(), cx.context().task_owner());
    match &state.listener {
      Some(listener) if listener.id == id => *listener.current.borrow_mut() = current,
      _ => {
        // First attach, or the focus id changed: (re-)register.
        state.listener = None;
        let current = Rc::new(RefCell::new(current));
        let target = current.clone();
        let registered: EventCallback = Rc::new(move |event: &EventKey| {
          // Cloned out so the callback may update this node (and swap `current`).
          let (callback, owner) = target.borrow().clone();
          with_owner(owner, || callback(event));
        });
        global_events.register(&registered, id.clone());
        state.listener = Some(Listener { id, current, registered });
      }
    }
  }

  fn build(&self, _context: &Context) -> Option<Widget> {
    self.child.clone()
  }

  fn paint(&self, cx: &mut PaintCx) {
    if self.focusable {
      let id = cx.state::<InputState>().borrow().listener.as_ref().and_then(|l| l.id.clone());
      if let Some(id) = id {
        cx.register_focus(&id);
      }
    }
    cx.paint_children();
  }
}

/// Keyboard behavior of a focusable widget, separate from `Element`.
///
/// Most widgets only implement `on_activate` (what Enter or space does); widgets that
/// take other keys (sliders, pickers, text fields) override `on_key`.
/// Use it with `focusable(handler, child)`.
pub trait KeyHandler {
  /// Enter or space while focused. Return `true` if handled.
  fn on_activate(&self) -> bool {
    false
  }

  /// Any key while focused. The default sends Enter and space to `on_activate`.
  fn on_key(&self, event: &EventKey) -> bool {
    keys::is_activate(event.key) && self.on_activate()
  }
}

/// A closure is a key handler: `focusable(|e: &EventKey| e.key == 'x', child)`.
impl<F: Fn(&EventKey) -> bool> KeyHandler for F {
  fn on_key(&self, event: &EventKey) -> bool {
    self(event)
  }
}

/// `child` in the Tab order, with `handler` receiving its keys while focused.
pub fn focusable(handler: impl KeyHandler + 'static, child: impl Into<Widget>) -> Widget {
  let handler = Rc::new(handler);
  Widget::from(InputEvent {
    event: Rc::new(move |e: &EventKey| {
      if handler.on_key(e) {
        e.handle();
      }
    }),
    focus_id: None,
    focusable: true,
    child: Some(child.into()),
  })
}
