//! Selection: checkbox, radio, switch, slider, chips, menus, date and time pickers.
//!
//! All controls are controlled: they show `value` and report changes through
//! `on_changed`; the app stores the value (usually in a `Signal`) and rebuilds.
//! A control without a callback is disabled.

use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::material::common::{
  Callback, ValueChanged, control, control_no_activate, disabled_fg, focus_on_open, pressable,
};
use crate::render::{
  Border, BoxPaint, Color, IconData, Offset, Rect, Renderer, Size, TextPaint, ToggleColors,
  TrackColors, draw_box, draw_checkbox, draw_focus, draw_icon, draw_radio, draw_slider,
  draw_switch_at,
};
use crate::theme::ThemeData;
use crate::{Element, EventKey, Icons, KeyHandler, Widget, keys};

fn toggle_colors(t: &ThemeData, enabled: bool) -> ToggleColors {
  let s = &t.color_scheme;
  if enabled {
    ToggleColors {
      active: s.primary,
      on_active: s.on_primary,
      inactive: s.outline,
      on_inactive: s.surface_container_highest,
    }
  } else {
    let d = disabled_fg(t);
    ToggleColors {
      active: d,
      on_active: s.surface,
      inactive: d,
      on_inactive: s.surface_container_highest,
    }
  }
}

// ---------------------------------------------------------------------------
// Checkbox, radio, switch
// ---------------------------------------------------------------------------

/// A box that is checked or not.
#[derive(Widget, Clone)]
#[non_exhaustive]
pub struct Checkbox {
  pub value: bool,
  #[widget(optional)]
  pub on_changed: Option<ValueChanged<bool>>,
}

impl KeyHandler for Checkbox {
  fn on_key(&self, event: &EventKey) -> bool {
    match &self.on_changed {
      Some(cb) if keys::is_activate(event.key) => {
        cb.call(!self.value);
        true
      }
      _ => false,
    }
  }
}

/// The drawn `Checkbox`; `Checkbox` wraps it to make it focusable.
struct CheckboxBody(Checkbox);

impl std::ops::Deref for CheckboxBody {
  type Target = Checkbox;
  fn deref(&self) -> &Checkbox {
    &self.0
  }
}

impl Element for Checkbox {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let enabled = self.on_changed.is_some();
    Some(control(self, CheckboxBody(self.clone()), enabled))
  }
}

impl Element for CheckboxBody {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    constraints.constrain(Size::new(40.0, 40.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let colors = toggle_colors(cx.theme(), self.on_changed.is_some());
    let rect = cx.rect();
    draw_checkbox(&mut cx.renderer, rect, self.value, &colors);
    if cx.is_focused() {
      let color = cx.theme().color_scheme.secondary;
      let rect = cx.rect();
      draw_focus(&mut cx.renderer, rect, color, 6.0);
    }
  }
}

/// One option of a group: selected when `value == group_value`.
/// Flutter's `Radio<T>` is generic; here options are identified by index.
#[derive(Widget, Clone)]
#[non_exhaustive]
pub struct Radio {
  pub value: usize,
  #[widget(optional)]
  pub group_value: Option<usize>,
  #[widget(optional)]
  pub on_changed: Option<ValueChanged<usize>>,
}

impl KeyHandler for Radio {
  fn on_key(&self, event: &EventKey) -> bool {
    match &self.on_changed {
      Some(cb) if keys::is_activate(event.key) => {
        cb.call(self.value);
        true
      }
      _ => false,
    }
  }
}

/// The drawn `Radio`; `Radio` wraps it to make it focusable.
struct RadioBody(Radio);

impl std::ops::Deref for RadioBody {
  type Target = Radio;
  fn deref(&self) -> &Radio {
    &self.0
  }
}

impl Element for Radio {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let enabled = self.on_changed.is_some();
    Some(control(self, RadioBody(self.clone()), enabled))
  }
}

impl Element for RadioBody {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    constraints.constrain(Size::new(40.0, 40.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let colors = toggle_colors(cx.theme(), self.on_changed.is_some());
    let rect = cx.rect();
    draw_radio(&mut cx.renderer, rect, self.group_value == Some(self.value), &colors);
    if cx.is_focused() {
      let color = cx.theme().color_scheme.secondary;
      let rect = cx.rect();
      draw_focus(&mut cx.renderer, rect, color, 6.0);
    }
  }
}

/// An on/off toggle.
#[derive(Widget, Clone)]
#[non_exhaustive]
pub struct Switch {
  pub value: bool,
  #[widget(optional)]
  pub on_changed: Option<ValueChanged<bool>>,
}

impl KeyHandler for Switch {
  fn on_key(&self, event: &EventKey) -> bool {
    match &self.on_changed {
      Some(cb) if keys::is_activate(event.key) => {
        cb.call(!self.value);
        true
      }
      _ => false,
    }
  }
}

/// The drawn `Switch`; `Switch` wraps it to make it focusable.
struct SwitchBody(Switch);

impl std::ops::Deref for SwitchBody {
  type Target = Switch;
  fn deref(&self) -> &Switch {
    &self.0
  }
}

impl Element for Switch {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let enabled = self.on_changed.is_some();
    Some(control(self, SwitchBody(self.clone()), enabled))
  }
}

impl Element for SwitchBody {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    constraints.constrain(Size::new(60.0, 40.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let colors = toggle_colors(cx.theme(), self.on_changed.is_some());
    let rect = cx.rect();
    let t = cx.animate_f32(if self.value { 1.0 } else { 0.0 }, 160.0, crate::Curve::FastOutSlowIn);
    draw_switch_at(&mut cx.renderer, rect, t, &colors);
    if cx.is_focused() {
      let color = cx.theme().color_scheme.secondary;
      let rect = cx.rect();
      draw_focus(&mut cx.renderer, rect, color, 6.0);
    }
  }
}

// ---------------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------------

/// Picks a value in `min..=max` (default 0..=1). When focused, Left/Right or
/// `-`/`+` move it by one division (or 5% without divisions).
#[derive(Widget, Clone)]
#[non_exhaustive]
pub struct Slider {
  pub value: f32,
  #[widget(optional)]
  pub min: f32,
  #[widget(optional)]
  pub max: Option<f32>,
  #[widget(optional)]
  pub divisions: Option<u32>,
  #[widget(optional)]
  pub on_changed: Option<ValueChanged<f32>>,
}

impl Slider {
  fn max(&self) -> f32 {
    self.max.unwrap_or(1.0)
  }

  fn fraction(&self) -> f32 {
    let span = self.max() - self.min;
    if span <= 0.0 { 0.0 } else { (self.value - self.min) / span }
  }
}

impl KeyHandler for Slider {
  fn on_key(&self, event: &EventKey) -> bool {
    let Some(cb) = &self.on_changed else { return false };
    let span = self.max() - self.min;
    let step = match self.divisions {
      Some(d) if d > 0 => span / d as f32,
      _ => span / 20.0,
    };
    let delta = match event.key {
      keys::LEFT | keys::DOWN | '-' => -step,
      keys::RIGHT | keys::UP | '+' | '=' => step,
      _ => return false,
    };
    let mut next = (self.value + delta).clamp(self.min, self.max());
    if let Some(d) = self.divisions.filter(|d| *d > 0) {
      // Snap to a division.
      let i = ((next - self.min) / span * d as f32).round();
      next = self.min + i * span / d as f32;
    }
    if next != self.value {
      cb.call(next);
    }
    true
  }
}

/// The drawn `Slider`; `Slider` wraps it to make it focusable.
struct SliderBody(Slider);

impl std::ops::Deref for SliderBody {
  type Target = Slider;
  fn deref(&self) -> &Slider {
    &self.0
  }
}

#[derive(Default)]
struct SliderPointerState {
  region_id: Option<u64>,
  dragging: bool,
}

impl Element for Slider {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let enabled = self.on_changed.is_some();
    Some(control_no_activate(self, SliderBody(self.clone()), enabled))
  }
}

impl Element for SliderBody {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let w = if constraints.has_bounded_width() { constraints.max_width } else { 200.0 };
    constraints.constrain(Size::new(w, 40.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let t = cx.theme();
    let enabled = self.on_changed.is_some();
    let s = &t.color_scheme;
    let colors = if enabled {
      TrackColors { active: s.primary, inactive: s.secondary_container, thumb: s.primary }
    } else {
      let d = disabled_fg(t);
      TrackColors { active: d, inactive: s.on_surface.with_opacity(0.12), thumb: d }
    };
    let rect = cx.rect();
    let track = Rect::new(rect.x + 12.0, rect.y, (rect.width - 24.0).max(0.0), rect.height);

    if let Some(cb) = self.on_changed.clone() {
      let state = cx.state::<SliderPointerState>();
      let id = *state.borrow_mut().region_id.get_or_insert_with(crate::pointer::alloc_region_id);
      let min = self.min;
      let max = self.max();
      let divisions = self.divisions;
      let state_cb = state.clone();
      cx.register_pointer(
        id,
        std::rc::Rc::new(move |ev: &crate::PointerEvent, r: Rect| {
          let x = match ev {
            crate::PointerEvent::Down { pos, button: crate::PointerButton::Primary } => {
              state_cb.borrow_mut().dragging = true;
              pos.x
            }
            crate::PointerEvent::Move(pos) => {
              if !state_cb.borrow().dragging {
                return false;
              }
              pos.x
            }
            crate::PointerEvent::Up { button: crate::PointerButton::Primary, .. } => {
              state_cb.borrow_mut().dragging = false;
              return true;
            }
            _ => return false,
          };
          let track_w = (r.width - 24.0).max(1.0);
          let frac = ((x - (r.x + 12.0)) / track_w).clamp(0.0, 1.0);
          let span = max - min;
          let mut next = min + frac * span;
          if let Some(d) = divisions.filter(|d| *d > 0) {
            let i = ((next - min) / span * d as f32).round();
            next = (min + i * span / d as f32).clamp(min, max);
          }
          cb.call(next);
          true
        }),
      );
    }

    draw_slider(&mut cx.renderer, track, self.fraction(), &colors);
    if cx.is_focused() {
      let color = cx.theme().color_scheme.secondary;
      let rect = cx.rect();
      draw_focus(&mut cx.renderer, rect, color, 6.0);
    }
  }
}

// ---------------------------------------------------------------------------
// Chips
// ---------------------------------------------------------------------------

/// Internal: the box every chip variant is drawn with.
#[derive(Clone)]
struct ChipBox {
  label: String,
  leading: Option<IconData>,
  selected: bool,
  on_pressed: Option<Callback>,
  on_deleted: Option<Callback>,
  /// Drawn without an outline (assist chips with elevation are out of scope).
  enabled: bool,
}

impl KeyHandler for ChipBox {
  fn on_key(&self, event: &EventKey) -> bool {
    if keys::is_activate(event.key)
      && let Some(cb) = &self.on_pressed
    {
      cb.call();
      return true;
    }
    if matches!(event.key, keys::DELETE | keys::BACKSPACE | 'x')
      && let Some(cb) = &self.on_deleted
    {
      cb.call();
      return true;
    }
    false
  }
}

impl ChipBox {
  /// The chip, focusable when it can be pressed or deleted.
  fn into_widget(self) -> Widget {
    let enabled = self.enabled && (self.on_pressed.is_some() || self.on_deleted.is_some());
    control(&self.clone(), self, enabled)
  }

  fn paint_text(&self, t: &ThemeData) -> TextPaint {
    let s = &t.color_scheme;
    let color = if !self.enabled {
      disabled_fg(t)
    } else if self.selected {
      s.on_secondary_container
    } else {
      s.on_surface_variant
    };
    t.text_theme.label_large.clone().with_color(color).resolve(t)
  }

  fn pads(&self) -> (f32, f32) {
    (
      if self.leading.is_some() { 8.0 } else { 16.0 },
      if self.on_deleted.is_some() { 8.0 } else { 16.0 },
    )
  }
}

impl Element for ChipBox {
  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let text = cx.measure_text(&self.label, &self.paint_text(cx.theme()));
    let (l, r) = self.pads();
    let icon = if self.leading.is_some() { 18.0 + 8.0 } else { 0.0 };
    let delete = if self.on_deleted.is_some() { 8.0 + 18.0 } else { 0.0 };
    constraints.constrain(Size::new(l + icon + text.width + delete + r, 32.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let t = cx.theme().clone();
    let s = t.color_scheme;
    let r = cx.rect();
    let paint = if self.selected && self.enabled {
      BoxPaint {
        color: s.secondary_container,
        border: Some(Border { color: s.outline, width: 1.0 }),
        radius: 6.0,
        elevation: 0.0,
      }
    } else {
      let border = if self.enabled { s.outline_variant } else { s.on_surface.with_opacity(0.12) };
      BoxPaint {
        color: s.surface,
        border: Some(Border { color: border, width: 1.0 }),
        radius: 6.0,
        elevation: 0.0,
      }
    };
    draw_box(&mut cx.renderer, r, &paint);
    let text = self.paint_text(&t);
    let (l, _) = self.pads();
    let mut x = r.x + l;
    if let Some(icon) = self.leading {
      let color = if self.enabled {
        if self.selected { s.on_secondary_container } else { s.primary }
      } else {
        text.color
      };
      draw_icon(&mut cx.renderer, Rect::new(x, r.center().y - 9.0, 18.0, 18.0), icon, color);
      x += 18.0 + 8.0;
    }
    let size = cx.renderer.measure_text(&self.label, &text);
    cx.renderer.draw_text(Offset::new(x, r.center().y - size.height / 2.0), &self.label, &text);
    if self.on_deleted.is_some() {
      let at = Rect::new(x + size.width + 8.0, r.center().y - 9.0, 18.0, 18.0);
      draw_icon(&mut cx.renderer, at, Icons::CLOSE, text.color);
    }
    if cx.is_focused() {
      let color = s.secondary;
      draw_focus(&mut cx.renderer, r, color, 6.0);
    }
  }
}

/// A compact element showing a label, with an optional avatar icon and delete button.
#[derive(Widget)]
#[non_exhaustive]
pub struct Chip {
  pub label: String,
  #[widget(optional)]
  pub avatar: Option<IconData>,
  /// Shows a delete icon; Delete / `x` calls it when focused.
  #[widget(optional)]
  pub on_deleted: Option<Callback>,
}

impl Element for Chip {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    Some(ChipBox::into_widget(ChipBox {
      label: self.label.clone(),
      leading: self.avatar,
      selected: false,
      on_pressed: None,
      on_deleted: self.on_deleted.clone(),
      enabled: true,
    }))
  }
}

/// A chip that triggers an action (Material 3 assist chip).
#[derive(Widget)]
#[non_exhaustive]
pub struct ActionChip {
  pub label: String,
  #[widget(optional)]
  pub avatar: Option<IconData>,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
}

impl Element for ActionChip {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    Some(ChipBox::into_widget(ChipBox {
      label: self.label.clone(),
      leading: self.avatar,
      selected: false,
      on_pressed: self.on_pressed.clone(),
      on_deleted: None,
      enabled: self.on_pressed.is_some(),
    }))
  }
}

/// A chip that toggles a filter; shows a check when selected.
#[derive(Widget)]
#[non_exhaustive]
pub struct FilterChip {
  pub label: String,
  pub selected: bool,
  #[widget(optional)]
  pub on_selected: Option<ValueChanged<bool>>,
}

impl Element for FilterChip {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let selected = self.selected;
    Some(ChipBox::into_widget(ChipBox {
      label: self.label.clone(),
      leading: selected.then_some(Icons::CHECK),
      selected,
      on_pressed: self.on_selected.clone().map(|cb| Callback::from(move || cb.call(!selected))),
      on_deleted: None,
      enabled: self.on_selected.is_some(),
    }))
  }
}

/// One choice among a set of chips (single selection).
#[derive(Widget)]
#[non_exhaustive]
pub struct ChoiceChip {
  pub label: String,
  pub selected: bool,
  #[widget(optional)]
  pub on_selected: Option<ValueChanged<bool>>,
}

impl Element for ChoiceChip {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let selected = self.selected;
    Some(ChipBox::into_widget(ChipBox {
      label: self.label.clone(),
      leading: selected.then_some(Icons::CHECK),
      selected,
      on_pressed: self.on_selected.clone().map(|cb| Callback::from(move || cb.call(!selected))),
      on_deleted: None,
      enabled: self.on_selected.is_some(),
    }))
  }
}

/// A chip for a piece of user input (a contact, a tag), usually deletable.
#[derive(Widget)]
#[non_exhaustive]
pub struct InputChip {
  pub label: String,
  #[widget(optional)]
  pub avatar: Option<IconData>,
  #[widget(optional)]
  pub selected: bool,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
  #[widget(optional)]
  pub on_deleted: Option<Callback>,
}

impl Element for InputChip {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    Some(ChipBox::into_widget(ChipBox {
      label: self.label.clone(),
      leading: self.avatar,
      selected: self.selected,
      on_pressed: self.on_pressed.clone(),
      on_deleted: self.on_deleted.clone(),
      enabled: true,
    }))
  }
}

// ---------------------------------------------------------------------------
// Menu
// ---------------------------------------------------------------------------

/// Shows `menu_children` in a popup below `child` while `open` is true. The popup is
/// painted above everything else and its first item takes the keyboard focus.
/// The app closes it (sets `open` to false), e.g. from an item's `on_pressed`.
#[derive(Widget)]
#[non_exhaustive]
pub struct MenuAnchor {
  pub child: Widget,
  pub menu_children: Vec<Widget>,
  #[widget(optional)]
  pub open: bool,
}

impl Element for MenuAnchor {
  fn children(&self) -> Vec<Widget> {
    let mut out = vec![self.child.clone()];
    if self.open {
      out.push(Widget::from(MenuPanel { items: self.menu_children.clone() }));
    }
    out
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let anchor = cx.layout_child(0, constraints);
    if cx.child_count() > 1 {
      cx.layout_child(1, Constraints::loose(Size::new(280.0, f32::INFINITY)));
      cx.place_child(1, Offset::new(0.0, anchor.height + 4.0));
    }
    anchor
  }

  fn paint(&self, cx: &mut PaintCx) {
    cx.paint_child(0);
    if cx.child_count() > 1 {
      // Above everything else, painted after the whole tree.
      cx.paint_child_on_top(1);
    }
  }
}

struct MenuPanel {
  items: Vec<Widget>,
}

impl Element for MenuPanel {
  fn children(&self) -> Vec<Widget> {
    self.items.clone()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let n = cx.child_count();
    let mut width: f32 = 148.0;
    for i in 0..n {
      width = width.max(
        cx.layout_child(i, Constraints::loose(Size::new(constraints.max_width, f32::INFINITY)))
          .width,
      );
    }
    let width = width.min(constraints.max_width.max(148.0));
    let mut y = 4.0;
    for i in 0..n {
      let s = cx.layout_child(
        i,
        Constraints {
          min_width: width,
          max_width: width,
          min_height: 0.0,
          max_height: f32::INFINITY,
        },
      );
      cx.place_child(i, Offset::new(0.0, y));
      y += s.height;
    }
    Size::new(width, y + 4.0)
  }

  fn paint(&self, cx: &mut PaintCx) {
    let s = cx.theme().color_scheme;
    let rect = cx.rect();
    draw_box(
      &mut cx.renderer,
      rect,
      &BoxPaint {
        color: s.surface,
        border: Some(Border { color: s.outline_variant, width: 1.0 }),
        radius: 6.0,
        elevation: 2.0,
      },
    );
    focus_on_open(cx);
    cx.paint_children();
  }
}

/// An item of a `MenuAnchor` (48 px tall).
#[derive(Widget, Clone)]
#[non_exhaustive]
pub struct MenuItemButton {
  pub child: Widget,
  #[widget(optional)]
  pub leading_icon: Option<IconData>,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
}

impl Element for MenuItemButton {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    Some(pressable(Widget::from(MenuItemBody(self.clone())), self.on_pressed.clone()))
  }
}

/// The drawn `MenuItemButton`.
struct MenuItemBody(MenuItemButton);

impl std::ops::Deref for MenuItemBody {
  type Target = MenuItemButton;
  fn deref(&self) -> &MenuItemButton {
    &self.0
  }
}

impl Element for MenuItemBody {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let t = cx.theme();
    let color = if self.on_pressed.is_some() { t.color_scheme.on_surface } else { disabled_fg(t) };
    let style = t.text_theme.body_medium.clone().with_color(color);
    cx.set_child_style(style);
    let left = 12.0 + if self.leading_icon.is_some() { 18.0 + 8.0 } else { 0.0 };
    let item_h = 34.0;
    let child = cx.layout_child(
      0,
      Constraints::loose(Size::new((constraints.max_width - left - 12.0).max(0.0), item_h)),
    );
    cx.place_child(0, Offset::new(left, (item_h - child.height) / 2.0));
    constraints.constrain(Size::new(left + child.width + 12.0, item_h))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let s = cx.theme().color_scheme;
    let r = cx.rect();
    if (cx.is_focused() || cx.is_hovered()) && self.on_pressed.is_some() {
      cx.renderer.fill_rect(r.deflate(4.0, 1.0, 4.0, 1.0), s.secondary_container, 4.0);
    }
    if let Some(icon) = self.leading_icon {
      let color = if self.on_pressed.is_some() { s.on_surface_variant } else { disabled_fg(cx.theme()) };
      draw_icon(
        &mut cx.renderer,
        Rect::new(r.x + 12.0, r.center().y - 9.0, 18.0, 18.0),
        icon,
        color,
      );
    }
    cx.paint_children();
  }
}

// ---------------------------------------------------------------------------
// Date picker
// ---------------------------------------------------------------------------

/// A calendar date (proleptic Gregorian).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
  pub year: i32,
  /// 1..=12
  pub month: u32,
  /// 1..=31
  pub day: u32,
}

impl Date {
  pub const fn new(year: i32, month: u32, day: u32) -> Self {
    Date { year, month, day }
  }

  pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
      1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
      4 | 6 | 9 | 11 => 30,
      _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
      _ => 28,
    }
  }

  /// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
  pub fn to_days(&self) -> i64 {
    let y = self.year as i64 - i64::from(self.month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = self.month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + self.day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
  }

  pub fn from_days(days: i64) -> Self {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    Date { year, month, day }
  }

  /// 0 = Sunday .. 6 = Saturday.
  pub fn weekday(&self) -> u32 {
    (self.to_days() + 4).rem_euclid(7) as u32
  }

  pub fn add_days(&self, days: i64) -> Self {
    Date::from_days(self.to_days() + days)
  }

  /// Same day in another month, clamped to that month's length.
  pub fn add_months(&self, months: i32) -> Self {
    let index = self.year * 12 + self.month as i32 - 1 + months;
    let (year, month) = (index.div_euclid(12), index.rem_euclid(12) as u32 + 1);
    Date { year, month, day: self.day.min(Date::days_in_month(year, month)) }
  }
}

const MONTHS: [&str; 12] = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/// A month calendar for picking a date (Flutter's `CalendarDatePicker`). When focused:
/// arrows move by a day / week, `<` and `>` by a month.
#[derive(Widget, Clone)]
#[non_exhaustive]
pub struct CalendarDatePicker {
  pub selected: Date,
  #[widget(optional)]
  pub on_date_changed: Option<ValueChanged<Date>>,
  #[widget(optional)]
  pub first_date: Option<Date>,
  #[widget(optional)]
  pub last_date: Option<Date>,
}

const DAY: f32 = 40.0;
const HEADER: f32 = 48.0;
const WEEKDAYS: f32 = 32.0;

impl CalendarDatePicker {
  fn day_rect(&self, r: Rect, day: u32) -> Rect {
    let first = Date::new(self.selected.year, self.selected.month, 1).weekday();
    let cell = first + day - 1;
    Rect::new(
      r.x + 12.0 + (cell % 7) as f32 * DAY,
      r.y + HEADER + WEEKDAYS + (cell / 7) as f32 * DAY,
      DAY,
      DAY,
    )
  }

  fn clamp(&self, d: Date) -> Date {
    let d = self.first_date.map_or(d, |f| d.max(f));
    self.last_date.map_or(d, |l| d.min(l))
  }
}

impl KeyHandler for CalendarDatePicker {
  fn on_key(&self, event: &EventKey) -> bool {
    let Some(cb) = &self.on_date_changed else { return false };
    let next = match event.key {
      keys::LEFT => self.selected.add_days(-1),
      keys::RIGHT => self.selected.add_days(1),
      keys::UP => self.selected.add_days(-7),
      keys::DOWN => self.selected.add_days(7),
      '<' | ',' => self.selected.add_months(-1),
      '>' | '.' => self.selected.add_months(1),
      _ => return false,
    };
    let next = self.clamp(next);
    if next != self.selected {
      cb.call(next);
    }
    true
  }
}

/// The drawn `CalendarDatePicker`; `CalendarDatePicker` wraps it to make it focusable.
struct CalendarDatePickerBody(CalendarDatePicker);

impl std::ops::Deref for CalendarDatePickerBody {
  type Target = CalendarDatePicker;
  fn deref(&self) -> &CalendarDatePicker {
    &self.0
  }
}

impl Element for CalendarDatePicker {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let enabled = self.on_date_changed.is_some();
    Some(control(self, CalendarDatePickerBody(self.clone()), enabled))
  }
}

impl Element for CalendarDatePickerBody {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    constraints.constrain(Size::new(7.0 * DAY + 24.0, HEADER + WEEKDAYS + 6.0 * DAY))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let t = cx.theme().clone();
    let s = t.color_scheme;
    let r = cx.rect();
    let enabled = self.on_date_changed.is_some();

    let title = format!("{} {}", MONTHS[(self.selected.month - 1) as usize], self.selected.year);
    let header = t.text_theme.title_small.clone().with_color(s.on_surface_variant).resolve(&t);
    let hs = cx.renderer.measure_text(&title, &header);
    cx.renderer.draw_text(
      Offset::new(r.x + 24.0, r.y + (HEADER - hs.height) / 2.0),
      &title,
      &header,
    );
    let arrows = Rect::new(r.right() - 88.0, r.y + 4.0, 40.0, 40.0);
    draw_icon(&mut cx.renderer, arrows, Icons::CHEVRON_LEFT, s.on_surface_variant);
    draw_icon(
      &mut cx.renderer,
      arrows.translate(Offset::new(40.0, 0.0)),
      Icons::CHEVRON_RIGHT,
      s.on_surface_variant,
    );

    let small = t.text_theme.body_small.clone().with_color(s.on_surface).resolve(&t);
    for (i, d) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
      let cell = Rect::new(r.x + 12.0 + i as f32 * DAY, r.y + HEADER, DAY, WEEKDAYS);
      let ds = cx.renderer.measure_text(d, &small);
      cx.renderer.draw_text(cell.center_rect(ds).origin(), d, &small);
    }

    let body = t.text_theme.body_large.clone().resolve(&t);
    for day in 1..=Date::days_in_month(self.selected.year, self.selected.month) {
      let cell = self.day_rect(r, day);
      let date = Date::new(self.selected.year, self.selected.month, day);
      let in_range = self.clamp(date) == date;
      let selected = day == self.selected.day;
      let color = if selected {
        if enabled { s.on_primary } else { s.surface }
      } else if in_range && enabled {
        s.on_surface
      } else {
        disabled_fg(&t)
      };
      if selected {
        cx.renderer.fill_rect(cell, if enabled { s.primary } else { disabled_fg(&t) }, DAY / 2.0);
      }
      let text = day.to_string();
      let paint = TextPaint { color, ..body.clone() };
      let ts = cx.renderer.measure_text(&text, &paint);
      cx.renderer.draw_text(cell.center_rect(ts).origin(), &text, &paint);
    }
    if cx.is_focused() {
      // Ring around the selected day rather than the whole calendar.
      let color = cx.theme().color_scheme.secondary;
      let cell = self.day_rect(cx.rect(), self.selected.day);
      draw_focus(&mut cx.renderer, cell, color, DAY / 2.0);
    }
  }
}

// ---------------------------------------------------------------------------
// Time picker
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeOfDay {
  /// 0..=23
  pub hour: u32,
  /// 0..=59
  pub minute: u32,
}

impl TimeOfDay {
  pub const fn new(hour: u32, minute: u32) -> Self {
    TimeOfDay { hour, minute }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TimePart {
  Hour,
  Minute,
  Period,
}

#[derive(Clone)]
struct TimeField {
  part: TimePart,
  time: TimeOfDay,
  use_24_hour: bool,
  on_changed: Option<ValueChanged<TimeOfDay>>,
}

impl TimeField {
  fn text(&self) -> String {
    match self.part {
      TimePart::Hour if self.use_24_hour => format!("{:02}", self.time.hour),
      TimePart::Hour => format!("{:02}", (self.time.hour + 11) % 12 + 1),
      TimePart::Minute => format!("{:02}", self.time.minute),
      TimePart::Period => if self.time.hour < 12 { "AM" } else { "PM" }.to_string(),
    }
  }
}

impl KeyHandler for TimeField {
  fn on_key(&self, event: &EventKey) -> bool {
    let Some(cb) = &self.on_changed else { return false };
    let up = match event.key {
      keys::UP | '+' | '=' => true,
      keys::DOWN | '-' => false,
      k if keys::is_activate(k) && self.part == TimePart::Period => true,
      _ => return false,
    };
    let TimeOfDay { hour, minute } = self.time;
    let next = match self.part {
      TimePart::Hour => TimeOfDay::new((hour + if up { 1 } else { 23 }) % 24, minute),
      TimePart::Minute => TimeOfDay::new(hour, (minute + if up { 1 } else { 59 }) % 60),
      TimePart::Period => TimeOfDay::new((hour + 12) % 24, minute),
    };
    cb.call(next);
    true
  }
}

impl Element for TimeField {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let w = if self.part == TimePart::Period { 52.0 } else { 96.0 };
    constraints.constrain(Size::new(w, 72.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let t = cx.theme().clone();
    let s = t.color_scheme;
    let r = cx.rect();
    let focused = cx.is_focused();
    let (bg, fg) = if focused {
      (s.primary_container, s.on_primary_container)
    } else {
      (s.surface_container_highest, s.on_surface)
    };
    let border = (self.part == TimePart::Period).then_some(Border { color: s.outline, width: 1.0 });
    draw_box(
      &mut cx.renderer,
      r,
      &BoxPaint {
        color: if self.part == TimePart::Period && !focused { Color::TRANSPARENT } else { bg },
        border,
        radius: 8.0,
        elevation: 0.0,
      },
    );
    let style = if self.part == TimePart::Period {
      &t.text_theme.title_medium
    } else {
      &t.text_theme.display_medium
    };
    let paint = style.clone().with_color(fg).resolve(&t);
    let text = self.text();
    let ts = cx.renderer.measure_text(&text, &paint);
    cx.renderer.draw_text(r.center_rect(ts).origin(), &text, &paint);
  }
}

/// Time entry (Material 3 time input: hour and minute fields plus AM/PM).
/// When a field is focused, Up/Down or `+`/`-` change it; Enter toggles AM/PM.
/// The clock dial is not implemented.
#[derive(Widget)]
#[non_exhaustive]
pub struct TimePicker {
  pub time: TimeOfDay,
  #[widget(optional)]
  pub on_changed: Option<ValueChanged<TimeOfDay>>,
  #[widget(optional)]
  pub use_24_hour: bool,
}

impl Element for TimePicker {
  fn children(&self) -> Vec<Widget> {
    let field = |part| {
      let field = TimeField {
        part,
        time: self.time,
        use_24_hour: self.use_24_hour,
        on_changed: self.on_changed.clone(),
      };
      let enabled = field.on_changed.is_some();
      control(&field.clone(), field, enabled)
    };
    let mut out = vec![field(TimePart::Hour), field(TimePart::Minute)];
    if !self.use_24_hour {
      out.push(field(TimePart::Period));
    }
    out
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let mut x = 0.0;
    for i in 0..cx.child_count() {
      let s = cx.layout_child(i, Constraints::loose(Size::new(96.0, 72.0)));
      cx.place_child(i, Offset::new(x, 0.0));
      // 24 px between hour and minute holds the colon.
      x += s.width + if i == 0 { 24.0 } else { 12.0 };
    }
    constraints.constrain(Size::new(x - 12.0, 72.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    cx.paint_children();
    let t = cx.theme().clone();
    let paint =
      t.text_theme.display_medium.clone().with_color(t.color_scheme.on_surface).resolve(&t);
    let hour = cx.child_rect(0);
    let colon = Rect::new(hour.right(), hour.y, 24.0, hour.height);
    let s = cx.renderer.measure_text(":", &paint);
    cx.renderer.draw_text(colon.center_rect(s).origin(), ":", &paint);
  }
}
