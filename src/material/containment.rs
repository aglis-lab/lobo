//! Containment: dialogs, bottom sheets, cards, dividers, list tiles.

use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::material::common::{Callback, disabled_fg, focus_on_open, pressable, styled};
use crate::render::{Border, BoxPaint, Color, Offset, Rect, Renderer, Size, draw_box};
use crate::theme::ThemeData;
use crate::{
  Center, Column, CrossAxisAlignment, Element, Icon, MainAxisAlignment, MainAxisSize, Padding, Row,
  Widget, ui,
};

// ---------------------------------------------------------------------------
// Dialog
// ---------------------------------------------------------------------------

/// A modal dialog. It fills the space it is given with a scrim and centers itself,
/// so put it last in a `Stack` above the screen while it should show
/// (Flutter opens it with `showDialog`; there are no routes here yet).
#[derive(Widget)]
#[non_exhaustive]
pub struct AlertDialog {
  #[widget(optional)]
  pub icon: Option<crate::render::IconData>,
  #[widget(optional)]
  pub title: Option<Widget>,
  #[widget(optional)]
  pub content: Option<Widget>,
  #[widget(optional)]
  pub actions: Vec<Widget>,
}

impl Element for AlertDialog {
  fn children(&self) -> Vec<Widget> {
    let mut column = Vec::new();
    if let Some(icon) = self.icon {
      column.push(styled(ui!(Center { child: Icon { icon } }), |t| {
        crate::TextStyle::default().with_color(t.color_scheme.secondary)
      }));
    }
    if let Some(title) = &self.title {
      column.push(styled(title.clone(), |t| {
        t.text_theme.headline_small.clone().with_color(t.color_scheme.on_surface)
      }));
    }
    if let Some(content) = &self.content {
      column.push(styled(content.clone(), |t| {
        t.text_theme.body_medium.clone().with_color(t.color_scheme.on_surface_variant)
      }));
    }
    if !self.actions.is_empty() {
      column.push(ui!(Row {
        spacing: 8.0,
        main_axis_alignment: MainAxisAlignment::End,
        children: self.actions.clone()
      }));
    }
    vec![ui!(Column {
      spacing: 16.0,
      main_axis_size: MainAxisSize::Min,
      cross_axis_alignment: CrossAxisAlignment::Start,
      children: column,
    })]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let size = constraints.constrain(constraints.biggest());
    let width = (size.width - 96.0).clamp(280.0, 560.0);
    let padding = Padding::all(24.0);
    let child = cx.layout_child(
      0,
      Constraints {
        min_width: 0.0,
        max_width: width - padding.horizontal(),
        min_height: 0.0,
        max_height: (size.height - 96.0).max(0.0),
      },
    );
    let panel = Size::new(width, child.height + padding.vertical());
    let at = Offset::new((size.width - panel.width) / 2.0, (size.height - panel.height) / 2.0);
    cx.place_child(0, Offset::new(at.x + padding.left, at.y + padding.top));
    size
  }

  fn paint(&self, cx: &mut PaintCx) {
    let scheme = cx.theme().color_scheme;
    let rect = cx.rect();
    cx.renderer.fill_rect(rect, scheme.scrim.with_opacity(0.5), 0.0);
    // Same geometry as `layout`: the child sits 24 px inside a centered panel.
    let child = cx.child_rect(0);
    let width = (rect.width - 96.0).clamp(280.0, 560.0);
    let panel =
      Rect::new(rect.center().x - width / 2.0, child.y - 24.0, width, child.height + 48.0);
    draw_box(
      &mut cx.renderer,
      panel,
      &BoxPaint {
        color: scheme.surface,
        border: Some(Border { color: scheme.outline_variant, width: 1.0 }),
        radius: 8.0,
        elevation: 3.0,
      },
    );
    focus_on_open(cx);
    cx.paint_children();
  }
}

// ---------------------------------------------------------------------------
// Bottom sheet
// ---------------------------------------------------------------------------

/// Read by `Scaffold`: whether the sheet blocks the screen behind it.
#[derive(Clone, Copy)]
pub(crate) struct SheetData {
  pub modal: bool,
}

/// A surface anchored to the bottom of the screen. Put it in `Scaffold::bottom_sheet`;
/// `modal: true` dims the screen behind it (Flutter's `showModalBottomSheet`).
#[derive(Widget)]
#[non_exhaustive]
pub struct BottomSheet {
  pub child: Widget,
  #[widget(optional)]
  pub show_drag_handle: bool,
  #[widget(optional)]
  pub modal: bool,
}

impl Element for BottomSheet {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn parent_data(&self) -> Option<Box<dyn std::any::Any>> {
    Some(Box::new(SheetData { modal: self.modal }))
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let top = if self.show_drag_handle { 48.0 } else { 16.0 };
    let padding = Padding { left: 0.0, right: 0.0, top, bottom: 16.0 };
    let child = cx.layout_child(
      0,
      constraints.deflate(padding).loosen().tighten(Some(constraints.max_width), None),
    );
    cx.place_child(0, Offset::new(0.0, top));
    constraints.constrain(Size::new(constraints.max_width, child.height + padding.vertical()))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let scheme = cx.theme().color_scheme;
    let rect = cx.rect();
    draw_box(
      &mut cx.renderer,
      rect,
      &BoxPaint { color: scheme.surface_container_low, border: None, radius: 28.0, elevation: 1.0 },
    );
    if self.show_drag_handle {
      let handle = Rect::new(rect.center().x - 16.0, rect.y + 22.0, 32.0, 4.0);
      cx.renderer.fill_rect(handle, scheme.on_surface_variant.with_opacity(0.4), 2.0);
    }
    cx.paint_children();
  }
}

// ---------------------------------------------------------------------------
// Card
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardVariant {
  #[default]
  Elevated,
  Filled,
  Outlined,
}

/// A rounded surface grouping related content (Flutter's `Card`, `Card.filled`, `Card.outlined`).
#[derive(Widget)]
#[non_exhaustive]
pub struct Card {
  pub child: Widget,
  #[widget(optional)]
  pub variant: CardVariant,
  #[widget(optional)]
  pub color: Option<Color>,
  /// Space around the card; unset is 4 px.
  #[widget(optional)]
  pub margin: Option<Padding>,
}

impl Card {
  fn margin(&self) -> Padding {
    self.margin.unwrap_or(Padding::all(4.0))
  }
}

impl Element for Card {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    cx.layout_padded(constraints, self.margin())
  }

  fn paint(&self, cx: &mut PaintCx) {
    let s = cx.theme().color_scheme;
    let m = self.margin();
    let rect = cx.rect().deflate(m.left, m.top, m.right, m.bottom);
    let paint = match self.variant {
      CardVariant::Elevated => BoxPaint {
        color: s.surface,
        border: Some(Border { color: s.outline_variant, width: 1.0 }),
        radius: 8.0,
        elevation: 1.0,
      },
      CardVariant::Filled => BoxPaint {
        color: s.surface_container,
        border: Some(Border { color: s.outline_variant, width: 1.0 }),
        radius: 8.0,
        elevation: 0.0,
      },
      CardVariant::Outlined => BoxPaint {
        color: s.surface,
        border: Some(Border { color: s.outline_variant, width: 1.0 }),
        radius: 8.0,
        elevation: 0.0,
      },
    };
    let paint = BoxPaint { color: self.color.unwrap_or(paint.color), ..paint };
    draw_box(&mut cx.renderer, rect, &paint);
    cx.paint_children();
  }
}

// ---------------------------------------------------------------------------
// Dividers
// ---------------------------------------------------------------------------

/// A thin horizontal line, in a box `height` tall (default 16).
#[derive(Widget)]
#[non_exhaustive]
pub struct Divider {
  #[widget(optional)]
  pub height: f32,
  #[widget(optional)]
  pub thickness: f32,
  #[widget(optional)]
  pub indent: f32,
  #[widget(optional)]
  pub end_indent: f32,
  #[widget(optional)]
  pub color: Option<Color>,
}

impl Element for Divider {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let h = if self.height > 0.0 { self.height } else { 16.0 };
    let w = if constraints.has_bounded_width() { constraints.max_width } else { 0.0 };
    constraints.constrain(Size::new(w, h))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = self.color.unwrap_or(cx.theme().color_scheme.outline_variant);
    let r = cx.rect();
    let y = r.center().y;
    let thickness = if self.thickness > 0.0 { self.thickness } else { 1.0 };
    cx.renderer.draw_line(
      Offset::new(r.x + self.indent, y),
      Offset::new(r.right() - self.end_indent, y),
      color,
      thickness,
    );
  }
}

/// A thin vertical line, in a box `width` wide (default 16).
#[derive(Widget)]
#[non_exhaustive]
pub struct VerticalDivider {
  #[widget(optional)]
  pub width: f32,
  #[widget(optional)]
  pub thickness: f32,
  #[widget(optional)]
  pub color: Option<Color>,
}

impl Element for VerticalDivider {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let w = if self.width > 0.0 { self.width } else { 16.0 };
    let h = if constraints.has_bounded_height() { constraints.max_height } else { 0.0 };
    constraints.constrain(Size::new(w, h))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = self.color.unwrap_or(cx.theme().color_scheme.outline_variant);
    let r = cx.rect();
    let x = r.center().x;
    let thickness = if self.thickness > 0.0 { self.thickness } else { 1.0 };
    cx.renderer.draw_line(Offset::new(x, r.y), Offset::new(x, r.bottom()), color, thickness);
  }
}

// ---------------------------------------------------------------------------
// List tile
// ---------------------------------------------------------------------------

/// One row of a list: optional leading widget, title, subtitle and trailing widget.
/// Focusable when `on_tap` is set.
#[derive(Widget, Clone)]
#[non_exhaustive]
pub struct ListTile {
  #[widget(optional)]
  pub leading: Option<Widget>,
  #[widget(optional)]
  pub title: Option<Widget>,
  #[widget(optional)]
  pub subtitle: Option<Widget>,
  #[widget(optional)]
  pub trailing: Option<Widget>,
  #[widget(optional)]
  pub on_tap: Option<Callback>,
  #[widget(optional)]
  pub selected: bool,
  #[widget(optional)]
  pub disabled: bool,
}

/// The laid-out tile; `ListTile` wraps it to make it focusable.
struct ListTileBody(ListTile);

impl std::ops::Deref for ListTileBody {
  type Target = ListTile;
  fn deref(&self) -> &ListTile {
    &self.0
  }
}

impl Element for ListTile {
  fn build(&self, _: &crate::Context) -> Option<Widget> {
    let on_tap = self.on_tap.clone().filter(|_| !self.disabled);
    Some(pressable(Widget::from(ListTileBody(self.clone())), on_tap))
  }
}

impl Element for ListTileBody {
  fn children(&self) -> Vec<Widget> {
    let (selected, disabled) = (self.selected, self.disabled);
    let color = move |t: &ThemeData, normal: Color| {
      if disabled {
        disabled_fg(t)
      } else if selected {
        t.color_scheme.primary
      } else {
        normal
      }
    };
    let mut out = Vec::new();
    if let Some(w) = &self.leading {
      out.push(styled(w.clone(), move |t| {
        crate::TextStyle::default().with_color(color(t, t.color_scheme.on_surface_variant))
      }));
    }
    if let Some(w) = &self.title {
      out.push(styled(w.clone(), move |t| {
        t.text_theme.body_large.clone().with_color(color(t, t.color_scheme.on_surface))
      }));
    }
    if let Some(w) = &self.subtitle {
      out.push(styled(w.clone(), move |t| {
        t.text_theme.body_medium.clone().with_color(if disabled {
          disabled_fg(t)
        } else {
          t.color_scheme.on_surface_variant
        })
      }));
    }
    if let Some(w) = &self.trailing {
      out.push(styled(w.clone(), move |t| {
        t.text_theme.label_small.clone().with_color(color(t, t.color_scheme.on_surface_variant))
      }));
    }
    out
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let (pad_l, pad_r, gap) = (16.0, 24.0, 16.0);
    let width = if constraints.has_bounded_width() { constraints.max_width } else { 360.0 };
    let mut i = 0;
    let mut next = |present: bool| {
      present.then(|| {
        i += 1;
        i - 1
      })
    };
    let (lead, title, sub, trail) = (
      next(self.leading.is_some()),
      next(self.title.is_some()),
      next(self.subtitle.is_some()),
      next(self.trailing.is_some()),
    );

    let free = Constraints::loose(Size::new(width, f32::INFINITY));
    let lead_s = lead.map(|i| cx.layout_child(i, free)).unwrap_or(Size::ZERO);
    let trail_s = trail.map(|i| cx.layout_child(i, free)).unwrap_or(Size::ZERO);
    let text_x = pad_l + if lead.is_some() { lead_s.width + gap } else { 0.0 };
    let text_w =
      (width - text_x - pad_r - if trail.is_some() { trail_s.width + gap } else { 0.0 }).max(0.0);
    let text_c = Constraints::loose(Size::new(text_w, f32::INFINITY));
    let title_s = title.map(|i| cx.layout_child(i, text_c)).unwrap_or(Size::ZERO);
    let sub_s = sub.map(|i| cx.layout_child(i, text_c)).unwrap_or(Size::ZERO);

    let text_h = title_s.height + sub_s.height;
    let min_h: f32 = if sub.is_some() { 72.0 } else { 56.0 };
    let height = min_h.max(text_h + 16.0).max(lead_s.height + 16.0).max(trail_s.height + 16.0);

    if let Some(i) = lead {
      cx.place_child(i, Offset::new(pad_l, (height - lead_s.height) / 2.0));
    }
    let top = (height - text_h) / 2.0;
    if let Some(i) = title {
      cx.place_child(i, Offset::new(text_x, top));
    }
    if let Some(i) = sub {
      cx.place_child(i, Offset::new(text_x, top + title_s.height));
    }
    if let Some(i) = trail {
      cx.place_child(
        i,
        Offset::new(width - pad_r - trail_s.width, (height - trail_s.height) / 2.0),
      );
    }
    constraints.constrain(Size::new(width, height))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let s = cx.theme().color_scheme;
    let rect = cx.rect();
    let inset = rect.deflate(6.0, 2.0, 6.0, 2.0);
    if (self.selected || cx.is_focused()) && !self.disabled {
      cx.renderer.fill_rect(inset, s.secondary_container, 6.0);
    } else if cx.is_hovered() && !self.disabled {
      cx.renderer.fill_rect(inset, s.secondary_container.with_opacity(0.55), 6.0);
    }
    cx.paint_children();
  }
}
