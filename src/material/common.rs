//! Pieces shared by the Material components: callbacks, inherited text style, colors.

use std::rc::Rc;

use crate::layout::{Constraints, LayoutCx, PaintCx, focus};
use crate::render::{Color, Size};
use crate::theme::{TextStyle, ThemeData};
use crate::{Element, KeyHandler, Widget, focusable};

/// A no-argument callback (`on_pressed`, `on_tap`). Any `Fn()` closure converts into it.
/// A widget with no callback is disabled, as in Flutter.
#[derive(Clone)]
pub struct Callback(Rc<dyn Fn()>);

impl Callback {
  pub fn call(&self) {
    (self.0)()
  }
}

/// A button's callback is its key handler: Enter or space calls it.
impl KeyHandler for Callback {
  fn on_activate(&self) -> bool {
    self.call();
    true
  }
}

/// `child` focusable and pressed with Enter / space when `on_pressed` is set;
/// unchanged (not focusable) when it is `None`, i.e. disabled.
pub(crate) fn pressable(child: Widget, on_pressed: Option<Callback>) -> Widget {
  match on_pressed {
    Some(cb) => focusable(cb, child),
    None => child,
  }
}

/// A control: `body` in the Tab order with `handler` taking its keys while `enabled`,
/// or just `body` when disabled.
pub(crate) fn control<H: KeyHandler + Clone + 'static>(
  handler: &H,
  body: impl Element + 'static,
  enabled: bool,
) -> Widget {
  if enabled { focusable(handler.clone(), Widget::from(body)) } else { Widget::from(body) }
}

/// Like [`control`], but clicking focuses the widget without firing Enter/activate
/// (used by `TextField` and `Slider`).
pub(crate) fn control_no_activate<H: KeyHandler + Clone + 'static>(
  handler: &H,
  body: impl Element + 'static,
  enabled: bool,
) -> Widget {
  if enabled {
    crate::widget::focusable_no_activate(handler.clone(), Widget::from(body))
  } else {
    Widget::from(body)
  }
}

impl<F: Fn() + 'static> From<F> for Callback {
  fn from(f: F) -> Self {
    Callback(Rc::new(f))
  }
}

/// A callback with the new value (`on_changed`). Any `Fn(T)` closure converts into it;
/// annotate the parameter type: `on_changed: |v: bool| ...`.
pub struct ValueChanged<T>(Rc<dyn Fn(T)>);

impl<T> Clone for ValueChanged<T> {
  fn clone(&self) -> Self {
    ValueChanged(self.0.clone())
  }
}

impl<T> ValueChanged<T> {
  pub fn call(&self, value: T) {
    (self.0)(value)
  }
}

impl<T, F: Fn(T) + 'static> From<F> for ValueChanged<T> {
  fn from(f: F) -> Self {
    ValueChanged(Rc::new(f))
  }
}

/// Sets the text style of its subtree (merged over the inherited one).
#[derive(Widget)]
#[non_exhaustive]
pub struct DefaultTextStyle {
  pub style: TextStyle,
  pub child: Widget,
}

impl Element for DefaultTextStyle {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    cx.set_child_style(self.style.clone());
    cx.layout_stacked(constraints)
  }
}

type StyleFn = Rc<dyn Fn(&ThemeData) -> TextStyle>;

/// Internal: a text style computed from the theme, wrapped around a child.
pub(crate) struct Styled {
  style: StyleFn,
  child: Widget,
}

impl Element for Styled {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let style = (self.style)(cx.theme());
    cx.set_child_style(style);
    cx.layout_stacked(constraints)
  }
}

/// Wraps `child` so its text uses `style(theme)`.
pub(crate) fn styled(child: Widget, style: impl Fn(&ThemeData) -> TextStyle + 'static) -> Widget {
  Widget::from(Styled { style: Rc::new(style), child })
}

/// Foreground for disabled controls (on-surface at 38%).
pub(crate) fn disabled_fg(theme: &ThemeData) -> Color {
  theme.color_scheme.on_surface.with_opacity(0.38)
}

/// Container color for disabled controls (on-surface at 12%).
pub(crate) fn disabled_bg(theme: &ThemeData) -> Color {
  theme.color_scheme.on_surface.with_opacity(0.12)
}

#[derive(Default)]
struct Opened {
  done: bool,
}

/// Call from `paint` of a modal surface: the first time it is painted, the first
/// focusable widget painted after it takes the keyboard focus.
pub(crate) fn focus_on_open(cx: &PaintCx) {
  let state = cx.state::<Opened>();
  if !state.borrow().done {
    state.borrow_mut().done = true;
    focus::request_next();
  }
}
