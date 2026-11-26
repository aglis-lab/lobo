//! Communication: badges, progress indicators, snack bars.

use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::material::actions::{ButtonBox, ButtonLook, label};
use crate::material::common::Callback;
use crate::render::{BoxPaint, Color, Offset, Rect, Size, TextPaint, TrackColors};
use crate::theme::ThemeData;
use crate::{Element, MainAxisSize, Padding, Row, Spacer, Widget, ui};

/// A small status marker on the top-right corner of `child`: a dot, or a short label.
#[derive(Widget)]
#[non_exhaustive]
pub struct Badge {
  /// Text such as a count; `None` shows a small dot.
  #[widget(optional)]
  pub label: Option<String>,
  #[widget(optional)]
  pub hidden: bool,
  pub child: Widget,
}

impl Element for Badge {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn paint(&self, cx: &mut PaintCx) {
    cx.paint_children();
    if self.hidden {
      return;
    }
    let scheme = cx.theme().color_scheme;
    let child = cx.child_rect(0);
    match &self.label {
      None => {
        cx.renderer.fill_circle(Offset::new(child.right() - 3.0, child.y + 3.0), 3.0, scheme.error);
      }
      Some(text) => {
        let paint =
          TextPaint { color: scheme.on_error, size: 11.0, bold: true, ..TextPaint::default() };
        let s = cx.renderer.measure_text(text, &paint);
        let w = (s.width + 8.0).max(16.0);
        let rect = Rect::new(child.right() - w / 2.0, child.y - 4.0, w, 16.0);
        cx.renderer.fill_rect(rect, scheme.error, 8.0);
        let at = rect.center_rect(s).origin();
        cx.renderer.draw_text(at, text, &paint);
      }
    }
  }
}

fn track_colors(theme: &ThemeData) -> TrackColors {
  let s = &theme.color_scheme;
  TrackColors { active: s.primary, inactive: s.secondary_container, thumb: s.primary }
}

/// Horizontal progress bar. `value` is 0.0..=1.0; unset means indeterminate
/// (drawn as a still frame: there is no animation yet).
#[derive(Widget)]
#[non_exhaustive]
pub struct LinearProgressIndicator {
  #[widget(optional)]
  pub value: Option<f32>,
  /// Bar height; 0 is the default 4.
  #[widget(optional)]
  pub min_height: f32,
}

impl Element for LinearProgressIndicator {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let h = if self.min_height > 0.0 { self.min_height } else { 4.0 };
    let w = if constraints.has_bounded_width() { constraints.max_width } else { 200.0 };
    constraints.constrain(Size::new(w, h))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let colors = track_colors(cx.theme());
    let rect = cx.rect();
    cx.renderer.draw_linear_progress(rect, self.value, &colors);
  }
}

/// Circular progress ring (48x48). `value` unset means indeterminate.
#[derive(Widget)]
#[non_exhaustive]
pub struct CircularProgressIndicator {
  #[widget(optional)]
  pub value: Option<f32>,
}

impl Element for CircularProgressIndicator {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    constraints.constrain(Size::new(48.0, 48.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let colors = track_colors(cx.theme());
    let rect = cx.rect();
    cx.renderer.draw_circular_progress(rect, self.value, &colors);
  }
}

/// The button of a `SnackBar`.
#[derive(Widget, Clone)]
pub struct SnackBarAction {
  pub label: String,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
}

/// A short message at the bottom of the screen. Put it in `Scaffold::snack_bar` while
/// it should show; there is no timer or animation, so the app removes it.
#[derive(Widget)]
#[non_exhaustive]
pub struct SnackBar {
  pub content: Widget,
  #[widget(optional)]
  pub action: Option<SnackBarAction>,
}

impl Element for SnackBar {
  fn children(&self) -> Vec<Widget> {
    let mut row = vec![self.content.clone(), ui!(Spacer {})];
    if let Some(action) = &self.action {
      // Flutter's SnackBarAction: inverse-primary text.
      row.push(ButtonBox::into_widget(ButtonBox {
        child: label(&action.label),
        on_pressed: action.on_pressed.clone(),
        look: std::rc::Rc::new(|t: &ThemeData, _| ButtonLook {
          bg: Color::TRANSPARENT,
          fg: t.color_scheme.inverse_primary,
          border: None,
          elevation: 0.0,
        }),
        padding: Padding::symmetric(12.0, 0.0),
        min: Size::new(48.0, 40.0),
        radius: 20.0,
      }));
    }
    vec![ui!(Row {
      spacing: 8.0,
      main_axis_size: MainAxisSize::Max,
      cross_axis_alignment: crate::CrossAxisAlignment::Center,
      children: row
    })]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let t = cx.theme();
    cx.set_child_style(
      t.text_theme.body_medium.clone().with_color(t.color_scheme.on_inverse_surface),
    );
    let padding = Padding { left: 16.0, right: 8.0, top: 4.0, bottom: 4.0 };
    let child = cx.layout_child(
      0,
      constraints
        .deflate(padding)
        .with_max_height(f32::INFINITY)
        .loosen()
        .tighten(Some(constraints.max_width - padding.horizontal()), None),
    );
    let h = (child.height + padding.vertical()).max(48.0);
    cx.place_child(0, Offset::new(padding.left, (h - child.height) / 2.0));
    constraints.constrain(Size::new(constraints.max_width, h))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let scheme = cx.theme().color_scheme;
    let rect = cx.rect();
    cx.renderer.draw_box(
      rect,
      &BoxPaint { color: scheme.inverse_surface, border: None, radius: 4.0, elevation: 3.0 },
    );
    cx.paint_children();
  }
}
