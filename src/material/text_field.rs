//! Text input.

use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::material::common::{ValueChanged, control_no_activate, disabled_fg};
use crate::render::{
  Border, BoxPaint, IconData, Offset, Rect, Renderer, Size, TextPaint, draw_box, draw_caret,
  draw_icon,
};
use crate::{Element, EventKey, KeyHandler, Signal, Widget, keys};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextFieldVariant {
  #[default]
  Outlined,
  Filled,
}

/// A single-line text input. The text lives in `controller` (Flutter's
/// `TextEditingController`), so the app reads and writes it like any signal.
///
/// When focused, typed characters are appended, Delete / Backspace removes the last
/// one, and Enter calls `on_submitted`. The cursor stays at the end:
/// there is no cursor movement or selection yet.
#[derive(Widget, Clone)]
#[non_exhaustive]
pub struct TextField {
  pub controller: Signal<String>,
  /// Label rendered above the input box (shadcn/ui style).
  #[widget(optional)]
  pub label: Option<String>,
  /// Placeholder shown inside the input box while empty.
  #[widget(optional)]
  pub hint: Option<String>,
  /// Helper / description text rendered below the input box.
  #[widget(optional)]
  pub helper_text: Option<String>,
  #[widget(optional)]
  pub prefix_icon: Option<IconData>,
  #[widget(optional)]
  pub suffix_icon: Option<IconData>,
  #[widget(optional)]
  pub obscure_text: bool,
  #[widget(optional)]
  pub disabled: bool,
  #[widget(optional)]
  pub max_length: Option<usize>,
  #[widget(optional)]
  pub variant: TextFieldVariant,
  #[widget(optional)]
  pub on_changed: Option<ValueChanged<String>>,
  #[widget(optional)]
  pub on_submitted: Option<ValueChanged<String>>,
}

impl TextField {
  fn set(&self, text: String) {
    self.controller.set(text.clone());
    if let Some(cb) = &self.on_changed {
      cb.call(text);
    }
  }
}

#[derive(Clone)]
struct TextFieldHandler {
  field: TextField,
  selected_all: Signal<bool>,
}

impl KeyHandler for TextFieldHandler {
  fn on_key(&self, event: &EventKey) -> bool {
    if event.modifiers.is_primary() && matches!(event.key, 'a' | 'A') {
      if !self.field.controller.peek().is_empty() {
        self.selected_all.set(true);
      }
      return true;
    }
    match event.key {
      keys::ESCAPE | keys::LEFT | keys::RIGHT => {
        if self.selected_all.peek() {
          self.selected_all.set(false);
          return true;
        }
        false
      }
      keys::ENTER => {
        self.selected_all.set(false);
        if let Some(cb) = &self.field.on_submitted {
          cb.call(self.field.controller.peek());
        }
        true
      }
      keys::BACKSPACE | keys::DELETE => {
        if self.selected_all.peek() {
          self.selected_all.set(false);
          self.field.set(String::new());
        } else {
          let mut text = self.field.controller.peek();
          text.pop();
          self.field.set(text);
        }
        true
      }
      // Ignore other shortcut combos and non-printable/special keys.
      _ if event.modifiers.is_primary() || event.modifiers.alt => false,
      k if k.is_control() || ('\u{F700}'..='\u{F8FF}').contains(&k) => false,
      k => {
        let mut text = if self.selected_all.peek() {
          self.selected_all.set(false);
          String::new()
        } else {
          self.field.controller.peek()
        };
        if self.field.max_length.is_none_or(|max| text.chars().count() < max) {
          text.push(k);
          self.field.set(text);
        }
        true
      }
    }
  }
}

/// The drawn `TextField`; `TextField` wraps it to make it focusable.
struct TextFieldBody {
  field: TextField,
  selected_all: Signal<bool>,
}

impl std::ops::Deref for TextFieldBody {
  type Target = TextField;
  fn deref(&self) -> &TextField {
    &self.field
  }
}

impl Element for TextField {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let enabled = !self.disabled;
    let selected_all = Signal::new(false);
    let handler = TextFieldHandler { field: self.clone(), selected_all: selected_all.clone() };
    Some(control_no_activate(
      &handler,
      TextFieldBody { field: self.clone(), selected_all },
      enabled,
    ))
  }
}

impl TextFieldBody {
  const INPUT_HEIGHT: f32 = 40.0;
  const LABEL_GAP: f32 = 22.0;
  const HELPER_GAP: f32 = 22.0;

  fn total_height(&self) -> f32 {
    let mut h = Self::INPUT_HEIGHT;
    if self.label.is_some() {
      h += Self::LABEL_GAP;
    }
    if self.helper_text.is_some() {
      h += Self::HELPER_GAP;
    }
    h
  }
}

impl Element for TextFieldBody {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let w = if constraints.has_bounded_width() { constraints.max_width } else { 280.0 };
    constraints.constrain(Size::new(w, self.total_height()))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let t = cx.theme().clone();
    let s = t.color_scheme;
    let outer = cx.renderer.snap_rect(cx.rect());
    let focused = cx.is_focused();
    // Read with `get` so a change from anywhere repaints the field.
    let text = self.controller.get();

    // 1. Optional label above the input box.
    let mut input_y = outer.y;
    if let Some(label) = &self.label {
      let label_color = if self.disabled { s.on_surface_variant.with_opacity(0.6) } else { s.on_surface };
      let label_paint = TextPaint {
        color: label_color,
        size: 13.5,
        bold: true,
        ..Default::default()
      };
      cx.renderer.draw_text(Offset::new(outer.x, outer.y), label, &label_paint);
      input_y += Self::LABEL_GAP;
    }

    let r = Rect::new(outer.x, input_y, outer.width, Self::INPUT_HEIGHT.min(outer.bottom() - input_y).max(0.0));

    // 2. Input box (shadcn/ui rounded-md with crisp 1px border + outer focus ring).
    let bg = match self.variant {
      TextFieldVariant::Outlined => {
        if self.disabled { s.surface_container_low } else { s.surface }
      }
      TextFieldVariant::Filled => s.surface_container,
    };
    let border_color = if self.disabled { s.outline_variant } else { s.outline };
    if focused && !self.disabled {
      cx.renderer.stroke_rect(
        r.inflate(2.0),
        Border { color: s.secondary.with_opacity(0.65), width: 2.0 },
        8.0,
      );
    }
    draw_box(
      &mut cx.renderer,
      r,
      &BoxPaint {
        color: bg,
        border: Some(Border { color: border_color, width: 1.0 }),
        radius: 6.0,
        elevation: 0.0,
      },
    );

    // 3. Optional prefix / suffix icons.
    let mut x = r.x + 12.0;
    if let Some(icon) = self.prefix_icon {
      draw_icon(
        &mut cx.renderer,
        Rect::new(r.x + 10.0, r.center().y - 10.0, 20.0, 20.0),
        icon,
        s.on_surface_variant,
      );
      x = r.x + 38.0;
    }
    if let Some(icon) = self.suffix_icon {
      draw_icon(
        &mut cx.renderer,
        Rect::new(r.right() - 30.0, r.center().y - 10.0, 20.0, 20.0),
        icon,
        s.on_surface_variant,
      );
    }

    // 4. Value text or placeholder + caret.
    let body = t.text_theme.body_medium.clone().resolve(&t);
    let line_h = cx.renderer.measure_text("", &body).height;
    let text_y = r.center().y - line_h / 2.0;
    let shown = if self.obscure_text { "•".repeat(text.chars().count()) } else { text.clone() };
    let available = (r.right() - x - if self.suffix_icon.is_some() { 38.0 } else { 12.0 }).max(0.0);
    let mut visible = shown;
    while !visible.is_empty() && cx.renderer.measure_text(&visible, &body).width > available {
      visible.remove(0);
    }
    if !focused {
      self.selected_all.set(false);
    }
    let is_selected_all = focused && !self.disabled && self.selected_all.get() && !visible.is_empty();
    if text.is_empty() {
      if let Some(hint) = &self.hint {
        let hint_color =
          if self.disabled { s.on_surface_variant.with_opacity(0.5) } else { s.on_surface_variant };
        cx.renderer.draw_text(
          Offset::new(x, text_y),
          hint,
          &TextPaint { color: hint_color, ..body.clone() },
        );
      }
    } else {
      if is_selected_all {
        let sel_w = cx.renderer.measure_text(&visible, &body).width;
        cx.renderer.fill_rect(
          Rect::new(x - 1.0, text_y - 1.0, sel_w + 2.0, line_h + 2.0),
          s.primary.with_opacity(0.18),
          3.0,
        );
      }
      let color = if self.disabled { disabled_fg(&t) } else { s.on_surface };
      cx.renderer.draw_text(Offset::new(x, text_y), &visible, &TextPaint { color, ..body.clone() });
    }
    if focused && !self.disabled && !is_selected_all {
      let w = cx.renderer.measure_text(&visible, &body).width;
      draw_caret(&mut cx.renderer, Offset::new(x + w, text_y), line_h, s.on_surface);
    }

    // 5. Optional helper text below the input box.
    if let Some(helper) = &self.helper_text {
      let helper_color =
        if self.disabled { s.on_surface_variant.with_opacity(0.55) } else { s.on_surface_variant };
      let helper_paint = TextPaint { color: helper_color, size: 12.5, ..Default::default() };
      cx.renderer.draw_text(Offset::new(outer.x, r.bottom() + 6.0), helper, &helper_paint);
    }
  }
}
