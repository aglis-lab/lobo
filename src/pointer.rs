//! Pointer / mouse input: hit-testing, click-to-focus, hover/press states, scroll, and [`GestureDetector`].

use std::cell::RefCell;
use std::rc::Rc;

use crate::layout::{Constraints, LayoutCx, PaintCx, focus};
use crate::render::{Offset, Rect, Size};
use crate::signal::wake;
use crate::{
  Callback, Element, EventKey, ValueChanged, Widget, dispatch_event, global_events, keys,
};

/// Mouse / pointer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
  Primary,
  Secondary,
  Middle,
}

/// Logical-pixel pointer event delivered to [`dispatch_pointer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEvent {
  Move(Offset),
  Down {
    pos: Offset,
    button: PointerButton,
  },
  Up {
    pos: Offset,
    button: PointerButton,
  },
  /// Vertical scroll delta in logical pixels (positive = scroll down).
  Scroll {
    pos: Offset,
    delta_y: f32,
  },
  Leave,
}

/// Callback invoked when a pointer event hits a registered region.
/// Returns `true` if handled (stops propagation to widgets behind it).
pub(crate) type PointerCallback = Rc<dyn Fn(&PointerEvent, Rect) -> bool>;

#[derive(Clone)]
pub(crate) struct PointerRegion {
  pub(crate) id: u64,
  pub(crate) rect: Rect,
  pub(crate) handler: PointerCallback,
}

#[derive(Clone)]
struct FocusTarget {
  id: String,
  rect: Rect,
  /// When `false` (e.g. `TextField`), clicking focuses the widget without firing Enter/activate.
  activate_on_click: bool,
}

#[derive(Default)]
struct PointerState {
  /// Focusable widgets in paint order (last = topmost).
  focus_targets: Vec<FocusTarget>,
  /// Custom pointer regions (sliders, scroll views, `GestureDetector`) in paint order.
  regions: Vec<PointerRegion>,
  /// Currently hovered focus id.
  hovered_focus: Option<String>,
  /// Currently pressed focus id (primary button held down).
  pressed_focus: Option<String>,
  /// Currently captured custom region id (while primary button is held down, e.g. dragging a slider).
  captured_region: Option<u64>,
  /// Last known pointer position in logical pixels.
  last_pos: Option<Offset>,
  next_region_id: u64,
}

thread_local! {
  static STATE: RefCell<PointerState> = RefCell::new(PointerState::default());
}

pub(crate) fn alloc_region_id() -> u64 {
  STATE.with(|s| {
    let mut s = s.borrow_mut();
    s.next_region_id += 1;
    s.next_region_id
  })
}

pub(crate) fn begin_frame() {
  STATE.with(|s| {
    let mut s = s.borrow_mut();
    s.focus_targets.clear();
    s.regions.clear();
  });
}

pub(crate) fn register_focus_target(id: &str, rect: Rect, activate_on_click: bool) {
  STATE.with(|s| {
    s.borrow_mut().focus_targets.push(FocusTarget { id: id.to_string(), rect, activate_on_click });
  });
}

pub(crate) fn register_region(region: PointerRegion) {
  STATE.with(|s| {
    s.borrow_mut().regions.push(region);
  });
}

/// Whether the focusable widget with `id` is currently hovered by the pointer.
pub(crate) fn is_focus_hovered(id: &str) -> bool {
  STATE.with(|s| s.borrow().hovered_focus.as_deref() == Some(id))
}

/// Whether the focusable widget with `id` is currently pressed by the primary pointer button.
pub(crate) fn is_focus_pressed(id: &str) -> bool {
  STATE.with(|s| s.borrow().pressed_focus.as_deref() == Some(id))
}

/// Clears all pointer state (useful in tests).
pub fn clear_pointer_state() {
  STATE.with(|s| *s.borrow_mut() = PointerState::default());
}

/// Dispatches a pointer event into the last painted frame's hit-test regions.
/// Returns `true` if any widget or hover/press visual state changed.
pub fn dispatch_pointer(event: PointerEvent) -> bool {
  match event {
    PointerEvent::Move(pos) => handle_move(pos),
    PointerEvent::Down { pos, button } => handle_down(pos, button),
    PointerEvent::Up { pos, button } => handle_up(pos, button),
    PointerEvent::Scroll { pos, delta_y } => handle_scroll(pos, delta_y),
    PointerEvent::Leave => handle_leave(),
  }
}

fn hit_focus_target(pos: Offset) -> Option<FocusTarget> {
  STATE.with(|s| s.borrow().focus_targets.iter().rev().find(|t| t.rect.contains(pos)).cloned())
}

fn handle_move(pos: Offset) -> bool {
  let captured = STATE.with(|s| {
    let mut st = s.borrow_mut();
    st.last_pos = Some(pos);
    let cap_id = st.captured_region?;
    st.regions.iter().rev().find(|r| r.id == cap_id).cloned()
  });

  let mut changed = false;
  if let Some(region) = captured {
    changed |= (region.handler)(&PointerEvent::Move(pos), region.rect);
  } else {
    // Deliver move to custom regions (for hover callbacks).
    let regions: Vec<PointerRegion> = STATE.with(|s| s.borrow().regions.clone());
    for region in regions.iter().rev() {
      if region.rect.contains(pos) && (region.handler)(&PointerEvent::Move(pos), region.rect) {
        changed = true;
        break;
      }
    }
  }

  let next_hover = hit_focus_target(pos).map(|t| t.id);
  let hover_changed = STATE.with(|s| {
    let mut st = s.borrow_mut();
    if st.hovered_focus != next_hover {
      st.hovered_focus = next_hover;
      true
    } else {
      false
    }
  });
  if hover_changed {
    wake();
    changed = true;
  }
  changed
}

fn handle_down(pos: Offset, button: PointerButton) -> bool {
  STATE.with(|s| s.borrow_mut().last_pos = Some(pos));
  if button != PointerButton::Primary {
    return false;
  }

  // 1. Custom pointer regions first (reverse paint order = topmost first).
  let regions: Vec<PointerRegion> = STATE.with(|s| s.borrow().regions.clone());
  for region in regions.iter().rev() {
    if region.rect.contains(pos)
      && (region.handler)(&PointerEvent::Down { pos, button }, region.rect)
    {
      STATE.with(|s| s.borrow_mut().captured_region = Some(region.id));
      // Also focus the underlying focusable target if one is at `pos`.
      if let Some(target) = hit_focus_target(pos) {
        focus::focus_by_id(&target.id);
      }
      wake();
      return true;
    }
  }

  // 2. Focusable widgets (reverse paint order = topmost first).
  if let Some(target) = hit_focus_target(pos) {
    focus::focus_by_id(&target.id);
    STATE.with(|s| {
      let mut st = s.borrow_mut();
      st.pressed_focus = Some(target.id.clone());
      st.hovered_focus = Some(target.id);
    });
    wake();
    true
  } else {
    // Clicking empty background clears focus & pressed state.
    let had = global_events.current_focus().is_some();
    focus::set(None);
    STATE.with(|s| s.borrow_mut().pressed_focus = None);
    if had {
      wake();
    }
    had
  }
}

fn handle_up(pos: Offset, button: PointerButton) -> bool {
  if button != PointerButton::Primary {
    return false;
  }

  // Release any captured custom region (e.g. Slider drag).
  let captured = STATE.with(|s| {
    let mut st = s.borrow_mut();
    let cap_id = st.captured_region.take()?;
    st.regions.iter().rev().find(|r| r.id == cap_id).cloned()
  });
  if let Some(region) = captured {
    let handled = (region.handler)(&PointerEvent::Up { pos, button }, region.rect);
    wake();
    return handled;
  }

  // Activate the pressed focusable widget if released inside its bounds.
  let pressed = STATE.with(|s| s.borrow_mut().pressed_focus.take());
  if let Some(pressed_id) = pressed {
    if let Some(target) = hit_focus_target(pos)
      && target.id == pressed_id
      && target.activate_on_click
    {
      dispatch_event(&EventKey::new(keys::ENTER));
    }
    wake();
    return true;
  }
  false
}

fn handle_scroll(pos: Offset, delta_y: f32) -> bool {
  let regions: Vec<PointerRegion> = STATE.with(|s| s.borrow().regions.clone());
  for region in regions.iter().rev() {
    if region.rect.contains(pos)
      && (region.handler)(&PointerEvent::Scroll { pos, delta_y }, region.rect)
    {
      wake();
      return true;
    }
  }
  false
}

fn handle_leave() -> bool {
  let changed = STATE.with(|s| {
    let mut st = s.borrow_mut();
    let had = st.hovered_focus.take().is_some() || st.pressed_focus.take().is_some();
    st.captured_region = None;
    st.last_pos = None;
    had
  });
  if changed {
    wake();
  }
  changed
}

// ---------------------------------------------------------------------------
// GestureDetector widget
// ---------------------------------------------------------------------------

#[derive(Default)]
struct GestureState {
  region_id: Option<u64>,
  pressed: bool,
  hovered: bool,
}

/// Detects pointer taps and hover on its `child` (Flutter's `GestureDetector`).
#[derive(Widget)]
#[non_exhaustive]
pub struct GestureDetector {
  #[widget(optional)]
  pub on_tap: Option<Callback>,
  #[widget(optional)]
  pub on_hover: Option<ValueChanged<bool>>,
  pub child: Widget,
}

impl Element for GestureDetector {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    cx.layout_stacked(constraints)
  }

  fn paint(&self, cx: &mut PaintCx) {
    let state = cx.state::<GestureState>();
    let id = {
      let mut st = state.borrow_mut();
      *st.region_id.get_or_insert_with(alloc_region_id)
    };
    let on_tap = self.on_tap.clone();
    let on_hover = self.on_hover.clone();
    let state_cb = state.clone();
    cx.register_pointer(
      id,
      Rc::new(move |ev: &PointerEvent, rect: Rect| match ev {
        PointerEvent::Down { button: PointerButton::Primary, .. } => {
          if on_tap.is_some() {
            state_cb.borrow_mut().pressed = true;
            true
          } else {
            false
          }
        }
        PointerEvent::Up { pos, button: PointerButton::Primary } => {
          let was_pressed = std::mem::replace(&mut state_cb.borrow_mut().pressed, false);
          if was_pressed && rect.contains(*pos) {
            if let Some(cb) = &on_tap {
              cb.call();
            }
            true
          } else {
            false
          }
        }
        PointerEvent::Move(_) => {
          if let Some(cb) = &on_hover {
            let mut st = state_cb.borrow_mut();
            if !st.hovered {
              st.hovered = true;
              cb.call(true);
            }
          }
          false
        }
        _ => false,
      }),
    );
    cx.paint_children();
  }
}
