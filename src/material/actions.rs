//! Actions: common buttons, icon buttons, floating action buttons, segmented buttons.

use std::rc::Rc;

use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::material::common::{Callback, ValueChanged, disabled_bg, disabled_fg, pressable};
use crate::render::{
  Border, BoxPaint, Color, IconData, Offset, Rect, Renderer, Size, draw_box, draw_focus,
};
use crate::theme::ThemeData;
use crate::{Element, Icon, Icons, MainAxisSize, Padding, Row, Text, Widget, ui};

// ---------------------------------------------------------------------------
// Shared button body
// ---------------------------------------------------------------------------

/// Colors of a button in one state.
#[derive(Clone, Copy)]
pub(crate) struct ButtonLook {
  pub bg: Color,
  pub fg: Color,
  pub border: Option<Border>,
  pub elevation: f32,
}

type LookFn = Rc<dyn Fn(&ThemeData, bool) -> ButtonLook>;

/// Internal: the focusable, pressable box every button is drawn with.
pub(crate) struct ButtonBox {
  pub child: Widget,
  pub on_pressed: Option<Callback>,
  /// Colors from (theme, enabled).
  pub look: LookFn,
  pub padding: Padding,
  pub min: Size,
  pub radius: f32,
}

impl ButtonBox {
  /// The box, focusable and pressable when it has `on_pressed`.
  pub(crate) fn into_widget(self) -> Widget {
    let on_pressed = self.on_pressed.clone();
    pressable(Widget::from(self), on_pressed)
  }
}

impl Element for ButtonBox {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let look = (self.look)(cx.theme(), self.on_pressed.is_some());
    cx.set_child_style(cx.theme().text_theme.label_large.clone().with_color(look.fg));
    let inner = constraints.deflate(self.padding).loosen();
    let child = cx.layout_child(0, inner);
    let size = constraints.constrain(Size::new(
      (child.width + self.padding.horizontal()).max(self.min.width),
      (child.height + self.padding.vertical()).max(self.min.height),
    ));
    cx.place_child(
      0,
      Offset::new((size.width - child.width) / 2.0, (size.height - child.height) / 2.0),
    );
    size
  }

  fn paint(&self, cx: &mut PaintCx) {
    let look = (self.look)(cx.theme(), self.on_pressed.is_some());
    let rect = cx.rect();
    draw_box(
      &mut cx.renderer,
      rect,
      &BoxPaint {
        color: look.bg,
        border: look.border,
        radius: self.radius,
        elevation: look.elevation,
      },
    );
    cx.paint_children();
    if cx.is_focused() {
      let color = cx.theme().color_scheme.secondary;
      draw_focus(&mut cx.renderer, rect, color, self.radius);
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonKind {
  Elevated,
  Filled,
  Tonal,
  Outlined,
  Text,
}

fn button_look(kind: ButtonKind) -> LookFn {
  Rc::new(move |t: &ThemeData, enabled: bool| {
    let s = &t.color_scheme;
    if !enabled {
      let bg = match kind {
        ButtonKind::Outlined | ButtonKind::Text => Color::TRANSPARENT,
        _ => disabled_bg(t),
      };
      let border =
        (kind == ButtonKind::Outlined).then(|| Border { color: disabled_bg(t), width: 1.0 });
      return ButtonLook { bg, fg: disabled_fg(t), border, elevation: 0.0 };
    }
    match kind {
      ButtonKind::Elevated => {
        ButtonLook { bg: s.surface_container_low, fg: s.primary, border: None, elevation: 1.0 }
      }
      ButtonKind::Filled => {
        ButtonLook { bg: s.primary, fg: s.on_primary, border: None, elevation: 0.0 }
      }
      ButtonKind::Tonal => ButtonLook {
        bg: s.secondary_container,
        fg: s.on_secondary_container,
        border: None,
        elevation: 0.0,
      },
      ButtonKind::Outlined => ButtonLook {
        bg: Color::TRANSPARENT,
        fg: s.primary,
        border: Some(Border { color: s.outline, width: 1.0 }),
        elevation: 0.0,
      },
      ButtonKind::Text => {
        ButtonLook { bg: Color::TRANSPARENT, fg: s.primary, border: None, elevation: 0.0 }
      }
    }
  })
}

fn common_button(
  kind: ButtonKind,
  child: Widget,
  icon: Option<IconData>,
  on_pressed: Option<Callback>,
) -> Widget {
  let (padding, content) = match icon {
    Some(icon) => {
      let left = if kind == ButtonKind::Text { 12.0 } else { 16.0 };
      let right = if kind == ButtonKind::Text { 16.0 } else { 24.0 };
      let row = ui!(Row {
        spacing: 8.0,
        main_axis_size: MainAxisSize::Min,
        children: [Icon { icon, size: 18.0 }, child],
      });
      (Padding { left, right, top: 0.0, bottom: 0.0 }, row)
    }
    None => (Padding::symmetric(if kind == ButtonKind::Text { 12.0 } else { 24.0 }, 0.0), child),
  };
  ButtonBox::into_widget(ButtonBox {
    child: content,
    on_pressed,
    look: button_look(kind),
    padding,
    min: Size::new(64.0, 40.0),
    radius: 20.0,
  })
}

/// Turns a label string into a `Text`; used by widgets whose label is a `String`.
pub(crate) fn label(text: &str) -> Widget {
  ui!(Text { text: text.to_string(), max_lines: 1 })
}

// ---------------------------------------------------------------------------
// Common buttons
// ---------------------------------------------------------------------------

/// Tonal surface button with a shadow. `on_pressed: None` disables it.
#[derive(Widget)]
#[non_exhaustive]
pub struct ElevatedButton {
  pub child: Widget,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
  /// Leading icon (Flutter's `ElevatedButton.icon`).
  #[widget(optional)]
  pub icon: Option<IconData>,
}

impl Element for ElevatedButton {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    Some(common_button(
      ButtonKind::Elevated,
      self.child.clone(),
      self.icon,
      self.on_pressed.clone(),
    ))
  }
}

/// High-emphasis filled button; `tonal: true` is `FilledButton.tonal`.
#[derive(Widget)]
#[non_exhaustive]
pub struct FilledButton {
  pub child: Widget,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
  #[widget(optional)]
  pub icon: Option<IconData>,
  #[widget(optional)]
  pub tonal: bool,
}

impl Element for FilledButton {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let kind = if self.tonal { ButtonKind::Tonal } else { ButtonKind::Filled };
    Some(common_button(kind, self.child.clone(), self.icon, self.on_pressed.clone()))
  }
}

/// Medium-emphasis button with an outline.
#[derive(Widget)]
#[non_exhaustive]
pub struct OutlinedButton {
  pub child: Widget,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
  #[widget(optional)]
  pub icon: Option<IconData>,
}

impl Element for OutlinedButton {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    Some(common_button(
      ButtonKind::Outlined,
      self.child.clone(),
      self.icon,
      self.on_pressed.clone(),
    ))
  }
}

/// Low-emphasis button: label only.
#[derive(Widget)]
#[non_exhaustive]
pub struct TextButton {
  pub child: Widget,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
  #[widget(optional)]
  pub icon: Option<IconData>,
}

impl Element for TextButton {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    Some(common_button(ButtonKind::Text, self.child.clone(), self.icon, self.on_pressed.clone()))
  }
}

// ---------------------------------------------------------------------------
// Icon button
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconButtonVariant {
  #[default]
  Standard,
  Filled,
  FilledTonal,
  Outlined,
}

/// A 40x40 button showing one icon (Flutter's `IconButton`, `IconButton.filled`, ...).
#[derive(Widget)]
#[non_exhaustive]
pub struct IconButton {
  pub icon: IconData,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
  #[widget(optional)]
  pub variant: IconButtonVariant,
  /// Overrides the icon color (standard variant).
  #[widget(optional)]
  pub color: Option<Color>,
}

impl Element for IconButton {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let variant = self.variant;
    let color = self.color;
    let look: LookFn = Rc::new(move |t: &ThemeData, enabled: bool| {
      let s = &t.color_scheme;
      if !enabled {
        let bg = match variant {
          IconButtonVariant::Filled | IconButtonVariant::FilledTonal => disabled_bg(t),
          _ => Color::TRANSPARENT,
        };
        return ButtonLook { bg, fg: disabled_fg(t), border: None, elevation: 0.0 };
      }
      match variant {
        IconButtonVariant::Standard => ButtonLook {
          bg: Color::TRANSPARENT,
          fg: color.unwrap_or(s.on_surface_variant),
          border: None,
          elevation: 0.0,
        },
        IconButtonVariant::Filled => {
          ButtonLook { bg: s.primary, fg: s.on_primary, border: None, elevation: 0.0 }
        }
        IconButtonVariant::FilledTonal => ButtonLook {
          bg: s.secondary_container,
          fg: s.on_secondary_container,
          border: None,
          elevation: 0.0,
        },
        IconButtonVariant::Outlined => ButtonLook {
          bg: Color::TRANSPARENT,
          fg: s.on_surface_variant,
          border: Some(Border { color: s.outline, width: 1.0 }),
          elevation: 0.0,
        },
      }
    });
    Some(ButtonBox::into_widget(ButtonBox {
      child: ui!(Icon { icon: self.icon }),
      on_pressed: self.on_pressed.clone(),
      look,
      padding: Padding::all(8.0),
      min: Size::new(40.0, 40.0),
      radius: 20.0,
    }))
  }
}

// ---------------------------------------------------------------------------
// Floating action button
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FabSize {
  /// 40x40
  Small,
  /// 56x56
  #[default]
  Regular,
  /// 96x96
  Large,
}

/// The primary action of a screen. Setting `label` makes it an extended FAB
/// (Flutter's `FloatingActionButton.extended`), with `child` as its icon.
#[derive(Widget)]
#[non_exhaustive]
pub struct FloatingActionButton {
  /// Usually an `Icon`.
  pub child: Widget,
  #[widget(optional)]
  pub label: Option<Widget>,
  #[widget(optional)]
  pub on_pressed: Option<Callback>,
  #[widget(optional)]
  pub size: FabSize,
}

impl Element for FloatingActionButton {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let look: LookFn = Rc::new(|t: &ThemeData, enabled: bool| {
      let s = &t.color_scheme;
      if enabled {
        ButtonLook {
          bg: s.primary_container,
          fg: s.on_primary_container,
          border: None,
          elevation: 3.0,
        }
      } else {
        ButtonLook { bg: disabled_bg(t), fg: disabled_fg(t), border: None, elevation: 0.0 }
      }
    });
    let (child, padding, min, radius) = match &self.label {
      Some(label) => (
        ui!(Row {
          spacing: 12.0,
          main_axis_size: MainAxisSize::Min,
          children: [self.child.clone(), label.clone()]
        }),
        Padding::symmetric(16.0, 0.0),
        Size::new(80.0, 56.0),
        16.0,
      ),
      None => {
        let (side, radius) = match self.size {
          FabSize::Small => (40.0, 12.0),
          FabSize::Regular => (56.0, 16.0),
          FabSize::Large => (96.0, 28.0),
        };
        (self.child.clone(), Padding::ZERO, Size::new(side, side), radius)
      }
    };
    Some(ButtonBox::into_widget(ButtonBox {
      child,
      on_pressed: self.on_pressed.clone(),
      look,
      padding,
      min,
      radius,
    }))
  }
}

// ---------------------------------------------------------------------------
// Segmented button
// ---------------------------------------------------------------------------

/// One option of a `SegmentedButton`.
#[derive(Widget, Clone)]
pub struct ButtonSegment {
  pub label: String,
  #[widget(optional)]
  pub icon: Option<IconData>,
  #[widget(optional)]
  pub disabled: bool,
}

/// A row of connected options; `selected` holds the indices of the chosen segments.
/// Flutter's `SegmentedButton<T>` is generic; here segments are identified by index.
#[derive(Widget)]
#[non_exhaustive]
pub struct SegmentedButton {
  pub segments: Vec<ButtonSegment>,
  pub selected: Vec<usize>,
  #[widget(optional)]
  pub on_selection_changed: Option<ValueChanged<Vec<usize>>>,
  #[widget(optional)]
  pub multi_selection_enabled: bool,
  #[widget(optional)]
  pub empty_selection_allowed: bool,
}

impl Element for SegmentedButton {
  fn children(&self) -> Vec<Widget> {
    self
      .segments
      .iter()
      .enumerate()
      .map(|(i, segment)| {
        let selected = self.selected.contains(&i);
        // Selection after pressing this segment.
        let mut next = self.selected.clone();
        if self.multi_selection_enabled {
          if selected {
            next.retain(|&s| s != i);
          } else {
            next.push(i);
            next.sort_unstable();
          }
        } else {
          next = if selected { Vec::new() } else { vec![i] };
        }
        let allowed = (!next.is_empty() || self.empty_selection_allowed) && next != self.selected;
        let on_pressed = match (&self.on_selection_changed, segment.disabled) {
          (Some(cb), false) => {
            let cb = cb.clone();
            Some(Callback::from(move || {
              if allowed {
                cb.call(next.clone())
              }
            }))
          }
          _ => None,
        };
        // A selected segment shows a check instead of its icon.
        let icon = if selected { Some(Icons::CHECK) } else { segment.icon };
        let content = match icon {
          Some(icon) => ui!(Row {
            spacing: 8.0,
            main_axis_size: MainAxisSize::Min,
            children: [Icon { icon, size: 18.0 }, label(&segment.label)],
          }),
          None => label(&segment.label),
        };
        let look: LookFn = Rc::new(move |t: &ThemeData, enabled: bool| {
          let s = &t.color_scheme;
          match (enabled, selected) {
            (true, true) => ButtonLook {
              bg: s.secondary_container,
              fg: s.on_secondary_container,
              border: None,
              elevation: 0.0,
            },
            (true, false) => {
              ButtonLook { bg: Color::TRANSPARENT, fg: s.on_surface, border: None, elevation: 0.0 }
            }
            (false, _) => ButtonLook {
              bg: Color::TRANSPARENT,
              fg: disabled_fg(t),
              border: None,
              elevation: 0.0,
            },
          }
        });
        ButtonBox::into_widget(ButtonBox {
          child: content,
          on_pressed,
          look,
          padding: Padding::symmetric(12.0, 0.0),
          min: Size::new(48.0, 40.0),
          radius: 0.0,
        })
      })
      .collect()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    // Equal-width segments, as wide as the widest one.
    let n = cx.child_count();
    let mut widest: f32 = 0.0;
    for i in 0..n {
      widest =
        widest.max(cx.layout_child(i, Constraints::loose(Size::new(f32::INFINITY, 40.0))).width);
    }
    if constraints.has_bounded_width() && n > 0 {
      widest = widest.min(constraints.max_width / n as f32);
    }
    for i in 0..n {
      cx.layout_child(i, Constraints::tight(Size::new(widest, 40.0)));
      cx.place_child(i, Offset::new(i as f32 * widest, 0.0));
    }
    constraints.constrain(Size::new(widest * n as f32, 40.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    cx.paint_children();
    let color = cx.theme().color_scheme.outline;
    let n = cx.child_count();
    if n == 0 {
      return;
    }
    // Around the segments, even if the widget was forced larger.
    let (first, last) = (cx.child_rect(0), cx.child_rect(n - 1));
    let rect = Rect::new(first.x, first.y, last.right() - first.x, first.height);
    for i in 1..n {
      let x = cx.child_rect(i).x;
      cx.renderer.draw_line(Offset::new(x, rect.y), Offset::new(x, rect.bottom()), color, 1.0);
    }
    cx.renderer.stroke_rect(rect, Border { color, width: 1.0 }, 20.0);
  }
}
