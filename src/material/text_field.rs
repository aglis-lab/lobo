//! Text input.

use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::material::common::{ValueChanged, control, disabled_fg};
use crate::render::{Border, BoxPaint, Color, IconData, Offset, Rect, Size, TextPaint};
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
  /// Floating label.
  #[widget(optional)]
  pub label: Option<String>,
  /// Placeholder while empty.
  #[widget(optional)]
  pub hint: Option<String>,
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

impl KeyHandler for TextField {
  fn on_key(&self, event: &EventKey) -> bool {
    match event.key {
      keys::ENTER => {
        if let Some(cb) = &self.on_submitted {
          cb.call(self.controller.peek());
        }
        true
      }
      keys::BACKSPACE | keys::DELETE => {
        let mut text = self.controller.peek();
        text.pop();
        self.set(text);
        true
      }
      // Arrows and other special keys are left to other listeners.
      k if k.is_control() || ('\u{F700}'..='\u{F8FF}').contains(&k) => false,
      k => {
        let mut text = self.controller.peek();
        if self.max_length.is_none_or(|max| text.chars().count() < max) {
          text.push(k);
          self.set(text);
        }
        true
      }
    }
  }
}

/// The drawn `TextField`; `TextField` wraps it to make it focusable.
struct TextFieldBody(TextField);

impl std::ops::Deref for TextFieldBody {
  type Target = TextField;
  fn deref(&self) -> &TextField {
    &self.0
  }
}

impl Element for TextField {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let enabled = !self.disabled;
    Some(control(self, TextFieldBody(self.clone()), enabled))
  }
}

impl Element for TextFieldBody {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let w = if constraints.has_bounded_width() { constraints.max_width } else { 280.0 };
    constraints.constrain(Size::new(w, 56.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let t = cx.theme().clone();
    let s = t.color_scheme;
    // On a coarse grid, the label and text must land on the outline's cells.
    let r = cx.renderer.snap_rect(cx.rect());
    let focused = cx.is_focused();
    // Read with `get` so a change from anywhere repaints the field.
    let text = self.controller.get();

    let accent = if self.disabled {
      disabled_fg(&t)
    } else if focused {
      s.primary
    } else {
      s.on_surface_variant
    };
    match self.variant {
      TextFieldVariant::Outlined => {
        let border = if self.disabled {
          s.on_surface.with_opacity(0.12)
        } else if focused {
          s.primary
        } else {
          s.outline
        };
        let width = if focused { 2.0 } else { 1.0 };
        cx.renderer.draw_box(
          r,
          &BoxPaint {
            color: Color::TRANSPARENT,
            border: Some(Border { color: border, width }),
            radius: 4.0,
            elevation: 0.0,
          },
        );
      }
      TextFieldVariant::Filled => {
        cx.renderer.fill_rect(r, s.surface_container_highest, 4.0);
        let y = r.bottom() - if focused { 1.0 } else { 0.5 };
        cx.renderer.draw_line(
          Offset::new(r.x, y),
          Offset::new(r.right(), y),
          accent,
          if focused { 2.0 } else { 1.0 },
        );
      }
    }

    let mut x = r.x + 16.0;
    if let Some(icon) = self.prefix_icon {
      cx.renderer.draw_icon(
        Rect::new(r.x + 12.0, r.center().y - 12.0, 24.0, 24.0),
        icon,
        s.on_surface_variant,
      );
      x = r.x + 52.0;
    }
    if let Some(icon) = self.suffix_icon {
      cx.renderer.draw_icon(
        Rect::new(r.right() - 36.0, r.center().y - 12.0, 24.0, 24.0),
        icon,
        s.on_surface_variant,
      );
    }

    let body = t.text_theme.body_large.clone().resolve(&t);
    let line_h = cx.renderer.measure_text("", &body).height;
    let text_y = r.center().y - line_h / 2.0;
    let floating = focused || !text.is_empty();
    if let Some(label) = &self.label {
      if floating {
        // Small label on the top border (outlined) or inside the top (filled).
        let paint = TextPaint { color: accent, ..t.text_theme.body_small.clone().resolve(&t) };
        let ls = cx.renderer.measure_text(label, &paint);
        let at = match self.variant {
          TextFieldVariant::Outlined => {
            let at = Offset::new(r.x + 12.0, r.y - ls.height / 2.0);
            // Gap in the outline behind the label.
            cx.renderer.fill_rect(
              Rect::new(at.x - 4.0, at.y, ls.width + 8.0, ls.height),
              s.surface,
              0.0,
            );
            Offset::new(at.x, at.y)
          }
          TextFieldVariant::Filled => Offset::new(x, r.y + 4.0),
        };
        cx.renderer.draw_text(at, label, &paint);
      } else {
        let paint = TextPaint {
          color: if self.disabled { disabled_fg(&t) } else { s.on_surface_variant },
          ..body.clone()
        };
        cx.renderer.draw_text(Offset::new(x, text_y), label, &paint);
      }
    }

    let shown = if self.obscure_text { "•".repeat(text.chars().count()) } else { text.clone() };
    let available = (r.right() - x - if self.suffix_icon.is_some() { 44.0 } else { 16.0 }).max(0.0);
    // Keep the end of long text visible.
    let mut visible = shown.clone();
    while !visible.is_empty() && cx.renderer.measure_text(&visible, &body).width > available {
      visible.remove(0);
    }
    if text.is_empty() {
      if let (Some(hint), true) = (&self.hint, focused || self.label.is_none()) {
        let paint = TextPaint { color: s.on_surface_variant, ..body.clone() };
        cx.renderer.draw_text(Offset::new(x, text_y), hint, &paint);
      }
    } else {
      let color = if self.disabled { disabled_fg(&t) } else { s.on_surface };
      cx.renderer.draw_text(Offset::new(x, text_y), &visible, &TextPaint { color, ..body.clone() });
    }
    if focused {
      let w = cx.renderer.measure_text(&visible, &body).width;
      cx.renderer.draw_caret(Offset::new(x + w, text_y), line_h, s.primary);
    }
  }
}
