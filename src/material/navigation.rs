//! Navigation: app bars, navigation bar / rail / drawer, tabs, and the `Scaffold`
//! that places them on the screen.

use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::material::common::{Callback, ValueChanged, focus_on_open, pressable, styled};
use crate::material::containment::SheetData;
use crate::render::{
  BoxPaint, Color, IconData, Offset, Rect, Renderer, Size, TextPaint, draw_box, draw_focus,
  draw_icon,
};
use crate::theme::TextStyle;
use crate::{Element, Widget};

// ---------------------------------------------------------------------------
// Destinations
// ---------------------------------------------------------------------------

/// One destination of a `NavigationBar` or `NavigationRail`.
#[derive(Widget, Clone)]
pub struct NavigationDestination {
  pub icon: IconData,
  /// Icon when selected; defaults to `icon`.
  #[widget(optional)]
  pub selected_icon: Option<IconData>,
  pub label: String,
}

/// Same fields as `NavigationDestination`.
pub type NavigationRailDestination = NavigationDestination;

#[derive(Clone, Copy, PartialEq, Eq)]
enum NavStyle {
  Bar,
  Rail,
  Drawer,
}

/// Internal: one focusable destination, drawn with its indicator, icon and label.
struct NavItem {
  icon: IconData,
  label: String,
  selected: bool,
  style: NavStyle,
}

impl NavItem {
  fn widget(
    d: &NavigationDestination,
    index: usize,
    selected: usize,
    on: &Option<ValueChanged<usize>>,
    style: NavStyle,
  ) -> Widget {
    let is_selected = index == selected;
    let on_select = on.clone().map(|cb| Callback::from(move || cb.call(index)));
    pressable(
      Widget::from(NavItem {
        icon: if is_selected { d.selected_icon.unwrap_or(d.icon) } else { d.icon },
        label: d.label.clone(),
        selected: is_selected,
        style,
      }),
      on_select,
    )
  }

  /// Indicator pill relative to the item's rect.
  fn indicator(&self, r: Rect) -> Rect {
    match self.style {
      NavStyle::Bar => Rect::new(r.center().x - 32.0, r.y + 12.0, 64.0, 32.0),
      NavStyle::Rail => Rect::new(r.center().x - 28.0, r.y + 4.0, 56.0, 32.0),
      NavStyle::Drawer => r,
    }
  }
}

impl Element for NavItem {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let size = match self.style {
      NavStyle::Bar => Size::new(constraints.max_width, 80.0),
      NavStyle::Rail => Size::new(80.0, 64.0),
      NavStyle::Drawer => Size::new(constraints.max_width, 56.0),
    };
    constraints.constrain(size)
  }

  fn paint(&self, cx: &mut PaintCx) {
    let t = cx.theme().clone();
    let s = t.color_scheme;
    let r = cx.rect();
    let pill = self.indicator(r);
    if self.selected {
      cx.renderer.fill_rect(pill, s.secondary_container, 16.0);
    }
    let icon_color = if self.selected { s.on_secondary_container } else { s.on_surface_variant };
    let label_style = if self.style == NavStyle::Drawer {
      &t.text_theme.label_large
    } else {
      &t.text_theme.label_medium
    };
    let label_color = match (self.selected, self.style) {
      (true, NavStyle::Drawer) => s.on_secondary_container,
      (true, _) => s.on_surface,
      (false, _) => s.on_surface_variant,
    };
    let mut paint = label_style.clone().with_color(label_color).resolve(&t);
    paint.bold |= self.selected;
    // Labels longer than the item are cut with an ellipsis.
    let room = if self.style == NavStyle::Drawer { r.width - 68.0 } else { r.width - 4.0 };
    let mut label = self.label.clone();
    while label.chars().count() > 1 && cx.renderer.measure_text(&label, &paint).width > room {
      let keep = label.chars().count() - 2;
      label = label.chars().take(keep).chain(['…']).collect();
    }
    let text = cx.renderer.measure_text(&label, &paint);
    match self.style {
      NavStyle::Drawer => {
        let icon = Rect::new(r.x + 16.0, r.center().y - 12.0, 24.0, 24.0);
        draw_icon(&mut cx.renderer, icon, self.icon, icon_color);
        cx.renderer.draw_text(
          Offset::new(r.x + 52.0, r.center().y - text.height / 2.0),
          &label,
          &paint,
        );
      }
      _ => {
        let icon = pill.center_rect(Size::new(24.0, 24.0));
        draw_icon(&mut cx.renderer, icon, self.icon, icon_color);
        cx.renderer.draw_text(
          Offset::new(r.center().x - text.width / 2.0, pill.bottom() + 4.0),
          &label,
          &paint,
        );
      }
    }
    if cx.is_focused() {
      let color = s.secondary;
      draw_focus(&mut cx.renderer, pill, color, 16.0);
    }
  }
}

// ---------------------------------------------------------------------------
// Navigation bar
// ---------------------------------------------------------------------------

/// Bottom navigation between 3-5 primary destinations.
#[derive(Widget)]
#[non_exhaustive]
pub struct NavigationBar {
  pub destinations: Vec<NavigationDestination>,
  #[widget(optional)]
  pub selected_index: usize,
  #[widget(optional)]
  pub on_destination_selected: Option<ValueChanged<usize>>,
}

impl Element for NavigationBar {
  fn children(&self) -> Vec<Widget> {
    let on = &self.on_destination_selected;
    self
      .destinations
      .iter()
      .enumerate()
      .map(|(i, d)| NavItem::widget(d, i, self.selected_index, on, NavStyle::Bar))
      .collect()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let width = if constraints.has_bounded_width() { constraints.max_width } else { 400.0 };
    let n = cx.child_count().max(1);
    let item = width / n as f32;
    for i in 0..cx.child_count() {
      cx.layout_child(i, Constraints::tight(Size::new(item, 80.0)));
      cx.place_child(i, Offset::new(i as f32 * item, 0.0));
    }
    constraints.constrain(Size::new(width, 80.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = cx.theme().color_scheme.surface_container;
    let rect = cx.rect();
    cx.renderer.fill_rect(rect, color, 0.0);
    cx.paint_children();
  }
}

// ---------------------------------------------------------------------------
// Navigation rail
// ---------------------------------------------------------------------------

/// Vertical navigation along the leading edge, for wide screens. Put it in a `Row`
/// next to the content, as in Flutter.
#[derive(Widget)]
#[non_exhaustive]
pub struct NavigationRail {
  pub destinations: Vec<NavigationRailDestination>,
  #[widget(optional)]
  pub selected_index: usize,
  #[widget(optional)]
  pub on_destination_selected: Option<ValueChanged<usize>>,
  /// Above the destinations, e.g. a `FloatingActionButton`.
  #[widget(optional)]
  pub leading: Option<Widget>,
}

impl Element for NavigationRail {
  fn children(&self) -> Vec<Widget> {
    let on = &self.on_destination_selected;
    let mut out: Vec<Widget> = self.leading.iter().cloned().collect();
    out.extend(
      self
        .destinations
        .iter()
        .enumerate()
        .map(|(i, d)| NavItem::widget(d, i, self.selected_index, on, NavStyle::Rail)),
    );
    out
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let mut y = 8.0;
    let first = if self.leading.is_some() {
      let s = cx.layout_child(0, Constraints::loose(Size::new(80.0, f32::INFINITY)));
      cx.place_child(0, Offset::new((80.0 - s.width) / 2.0, y));
      y += s.height + 24.0;
      1
    } else {
      0
    };
    for i in first..cx.child_count() {
      let s = cx.layout_child(i, Constraints::loose(Size::new(80.0, f32::INFINITY)));
      cx.place_child(i, Offset::new(0.0, y));
      y += s.height + 12.0;
    }
    let h = if constraints.has_bounded_height() { constraints.max_height } else { y };
    constraints.constrain(Size::new(80.0, h))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = cx.theme().color_scheme.surface;
    let rect = cx.rect();
    cx.renderer.fill_rect(rect, color, 0.0);
    cx.paint_children();
  }
}

// ---------------------------------------------------------------------------
// Navigation drawer
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct DrawerDestinationData {
  icon: IconData,
  selected_icon: Option<IconData>,
  label: String,
}

/// A destination inside a `NavigationDrawer`; other children of the drawer
/// (headers, dividers) are shown as they are.
#[derive(Widget)]
#[non_exhaustive]
pub struct NavigationDrawerDestination {
  pub icon: IconData,
  #[widget(optional)]
  pub selected_icon: Option<IconData>,
  pub label: String,
}

impl Element for NavigationDrawerDestination {
  fn parent_data(&self) -> Option<Box<dyn std::any::Any>> {
    Some(Box::new(DrawerDestinationData {
      icon: self.icon,
      selected_icon: self.selected_icon,
      label: self.label.clone(),
    }))
  }
}

/// A side panel of destinations. Show it with `Scaffold::drawer` (modal), or put it
/// in a `Row` to keep it open. `selected_index` counts destinations only.
#[derive(Widget)]
#[non_exhaustive]
pub struct NavigationDrawer {
  pub children: Vec<Widget>,
  #[widget(optional)]
  pub selected_index: usize,
  #[widget(optional)]
  pub on_destination_selected: Option<ValueChanged<usize>>,
}

impl Element for NavigationDrawer {
  fn children(&self) -> Vec<Widget> {
    let mut index = 0;
    self
      .children
      .iter()
      .map(|child| {
        let data =
          child.element().parent_data().and_then(|d| d.downcast::<DrawerDestinationData>().ok());
        match data {
          Some(d) => {
            let dest = NavigationDestination {
              icon: d.icon,
              selected_icon: d.selected_icon,
              label: d.label.clone(),
            };
            let item = NavItem::widget(
              &dest,
              index,
              self.selected_index,
              &self.on_destination_selected,
              NavStyle::Drawer,
            );
            index += 1;
            item
          }
          None => styled(child.clone(), |t| {
            t.text_theme.title_small.clone().with_color(t.color_scheme.on_surface_variant)
          }),
        }
      })
      .collect()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let width = constraints.max_width.min(360.0);
    let inner = Constraints {
      min_width: width - 24.0,
      max_width: width - 24.0,
      min_height: 0.0,
      max_height: f32::INFINITY,
    };
    let mut y = 12.0;
    for i in 0..cx.child_count() {
      let s = cx.layout_child(i, inner);
      cx.place_child(i, Offset::new(12.0, y));
      y += s.height;
    }
    let h = if constraints.has_bounded_height() { constraints.max_height } else { y + 12.0 };
    constraints.constrain(Size::new(width, h))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = cx.theme().color_scheme.surface_container_low;
    let rect = cx.rect();
    draw_box(
      &mut cx.renderer,
      rect,
      &BoxPaint { color, border: None, radius: 16.0, elevation: 1.0 },
    );
    cx.renderer.push_clip(rect);
    cx.paint_children();
    cx.renderer.pop_clip();
  }
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

/// One tab of a `TabBar`.
#[derive(Widget, Clone)]
pub struct Tab {
  pub text: String,
  #[widget(optional)]
  pub icon: Option<IconData>,
}

struct TabItem {
  tab: Tab,
  selected: bool,
}

impl Element for TabItem {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    constraints.constrain(Size::new(
      constraints.max_width,
      if self.tab.icon.is_some() { 64.0 } else { 48.0 },
    ))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let t = cx.theme().clone();
    let s = t.color_scheme;
    let r = cx.rect();
    let color = if self.selected { s.primary } else { s.on_surface_variant };
    let paint =
      TextPaint { bold: true, ..t.text_theme.title_small.clone().with_color(color).resolve(&t) };
    let text = cx.renderer.measure_text(&self.tab.text, &paint);
    let label_y = if let Some(icon) = self.tab.icon {
      draw_icon(
        &mut cx.renderer,
        Rect::new(r.center().x - 12.0, r.y + 10.0, 24.0, 24.0),
        icon,
        color,
      );
      r.y + 36.0
    } else {
      r.center().y - text.height / 2.0
    };
    cx.renderer.draw_text(
      Offset::new(r.center().x - text.width / 2.0, label_y),
      &self.tab.text,
      &paint,
    );
    if self.selected {
      let w = text.width.max(24.0);
      cx.renderer.fill_rect(
        Rect::new(r.center().x - w / 2.0, r.bottom() - 3.0, w, 3.0),
        s.primary,
        1.5,
      );
    }
    if cx.is_focused() {
      let color = s.secondary;
      draw_focus(&mut cx.renderer, r.inflate(-4.0), color, 4.0);
    }
  }
}

/// A row of tabs with an indicator under the selected one (Material 3 primary tabs).
/// Pair with `TabBarView` for the content.
#[derive(Widget)]
#[non_exhaustive]
pub struct TabBar {
  pub tabs: Vec<Tab>,
  #[widget(optional)]
  pub selected_index: usize,
  #[widget(optional)]
  pub on_tap: Option<ValueChanged<usize>>,
}

impl Element for TabBar {
  fn children(&self) -> Vec<Widget> {
    self
      .tabs
      .iter()
      .enumerate()
      .map(|(i, tab)| {
        let on_select = self.on_tap.clone().map(|cb| Callback::from(move || cb.call(i)));
        pressable(
          Widget::from(TabItem { tab: tab.clone(), selected: i == self.selected_index }),
          on_select,
        )
      })
      .collect()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let width = if constraints.has_bounded_width() { constraints.max_width } else { 360.0 };
    let n = cx.child_count().max(1);
    let item = width / n as f32;
    let mut h: f32 = 48.0;
    for i in 0..cx.child_count() {
      h = h.max(
        cx.layout_child(i, Constraints::loose(Size::new(item, 64.0)).tighten(Some(item), None))
          .height,
      );
      cx.place_child(i, Offset::new(i as f32 * item, 0.0));
    }
    constraints.constrain(Size::new(width, h))
  }

  fn paint(&self, cx: &mut PaintCx) {
    // Under the tabs, so the selected tab's indicator covers it.
    let color = cx.theme().color_scheme.surface_container_highest;
    let r = cx.rect();
    cx.renderer.draw_line(
      Offset::new(r.x, r.bottom() - 0.5),
      Offset::new(r.right(), r.bottom() - 0.5),
      color,
      1.0,
    );
    cx.paint_children();
  }
}

/// Shows the child at `index` (the content of the selected tab).
#[derive(Widget)]
#[non_exhaustive]
pub struct TabBarView {
  #[widget(optional)]
  pub index: usize,
  pub children: Vec<Widget>,
}

impl Element for TabBarView {
  fn children(&self) -> Vec<Widget> {
    self.children.get(self.index).cloned().into_iter().collect()
  }
}

// ---------------------------------------------------------------------------
// App bars
// ---------------------------------------------------------------------------

/// The bar at the top of a screen: leading widget, title and actions (64 px).
#[derive(Widget)]
#[non_exhaustive]
pub struct AppBar {
  #[widget(optional)]
  pub leading: Option<Widget>,
  #[widget(optional)]
  pub title: Option<Widget>,
  #[widget(optional)]
  pub actions: Vec<Widget>,
  #[widget(optional)]
  pub center_title: bool,
  #[widget(optional)]
  pub background_color: Option<Color>,
}

impl Element for AppBar {
  fn children(&self) -> Vec<Widget> {
    let mut out: Vec<Widget> = self.leading.iter().cloned().collect();
    if let Some(title) = &self.title {
      out.push(styled(title.clone(), |t| {
        t.text_theme.title_large.clone().with_color(t.color_scheme.on_surface)
      }));
    }
    out.extend(self.actions.iter().cloned());
    out
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    cx.set_child_style(TextStyle::default().with_color(cx.theme().color_scheme.on_surface_variant));
    let width = if constraints.has_bounded_width() { constraints.max_width } else { 360.0 };
    let height = 64.0;
    let loose = Constraints::loose(Size::new(width, height));
    let mut i = 0;
    let mut left = 16.0;
    if self.leading.is_some() {
      let s = cx.layout_child(0, loose);
      cx.place_child(0, Offset::new(4.0, (height - s.height) / 2.0));
      left = 4.0 + s.width.max(48.0) + 8.0;
      i = 1;
    }
    let title = self.title.is_some().then_some(i);
    let first_action = i + usize::from(self.title.is_some());
    // Actions from the right edge.
    let mut right = width - 4.0;
    for a in (first_action..cx.child_count()).rev() {
      let s = cx.layout_child(a, loose);
      right -= s.width;
      cx.place_child(a, Offset::new(right, (height - s.height) / 2.0));
    }
    if let Some(t) = title {
      let s =
        cx.layout_child(t, Constraints::loose(Size::new((right - left - 8.0).max(0.0), height)));
      let x = if self.center_title { (width - s.width) / 2.0 } else { left };
      cx.place_child(t, Offset::new(x, (height - s.height) / 2.0));
    }
    constraints.constrain(Size::new(width, height))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = self.background_color.unwrap_or(cx.theme().color_scheme.surface);
    let rect = cx.rect();
    cx.renderer.fill_rect(rect, color, 0.0);
    cx.paint_children();
  }
}

/// A bar at the bottom of the screen for actions (80 px).
#[derive(Widget)]
#[non_exhaustive]
pub struct BottomAppBar {
  pub child: Widget,
  #[widget(optional)]
  pub color: Option<Color>,
}

impl Element for BottomAppBar {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let width = if constraints.has_bounded_width() { constraints.max_width } else { 360.0 };
    let s = cx.layout_child(
      0,
      Constraints::loose(Size::new(width - 32.0, 56.0)).tighten(Some(width - 32.0), None),
    );
    cx.place_child(0, Offset::new(16.0, (80.0 - s.height) / 2.0));
    constraints.constrain(Size::new(width, 80.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = self.color.unwrap_or(cx.theme().color_scheme.surface_container);
    let rect = cx.rect();
    cx.renderer.fill_rect(rect, color, 0.0);
    cx.paint_children();
  }
}

// ---------------------------------------------------------------------------
// Scaffold
// ---------------------------------------------------------------------------

/// Dims everything painted before it.
struct Scrim;

impl Element for Scrim {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    constraints.constrain(constraints.biggest())
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = cx.theme().color_scheme.scrim.with_opacity(0.32);
    let rect = cx.rect();
    cx.renderer.fill_rect(rect, color, 0.0);
    // Whatever opens above the scrim (drawer, modal sheet) takes the focus.
    focus_on_open(cx);
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
  Body,
  AppBar,
  BottomNav,
  Sheet,
  Fab,
  SnackBar,
  Scrim,
  Drawer,
}

/// The basic Material screen layout: app bar, body, bottom navigation, floating
/// action button, bottom sheet, snack bar and drawer. Each is shown while it is set.
#[derive(Widget)]
#[non_exhaustive]
pub struct Scaffold {
  #[widget(optional)]
  pub app_bar: Option<Widget>,
  #[widget(optional)]
  pub body: Option<Widget>,
  #[widget(optional)]
  pub floating_action_button: Option<Widget>,
  #[widget(optional)]
  pub bottom_navigation_bar: Option<Widget>,
  #[widget(optional)]
  pub bottom_sheet: Option<Widget>,
  #[widget(optional)]
  pub snack_bar: Option<Widget>,
  /// Shown (open, over a scrim) while set.
  #[widget(optional)]
  pub drawer: Option<Widget>,
  #[widget(optional)]
  pub background_color: Option<Color>,
}

impl Scaffold {
  fn sheet_is_modal(&self) -> bool {
    self
      .bottom_sheet
      .as_ref()
      .and_then(|s| s.element().parent_data())
      .and_then(|d| d.downcast::<SheetData>().ok())
      .is_some_and(|d| d.modal)
  }

  /// Children in paint order (later is on top).
  fn slots(&self) -> Vec<(Slot, Widget)> {
    let mut out = Vec::new();
    let mut push = |slot, w: &Option<Widget>| {
      if let Some(w) = w {
        out.push((slot, w.clone()));
      }
    };
    // The app bar first: paint order is also Tab order, so it reads top to bottom.
    push(Slot::AppBar, &self.app_bar);
    push(Slot::Body, &self.body);
    push(Slot::BottomNav, &self.bottom_navigation_bar);
    let modal = self.sheet_is_modal();
    if !modal {
      push(Slot::Sheet, &self.bottom_sheet);
    }
    push(Slot::Fab, &self.floating_action_button);
    push(Slot::SnackBar, &self.snack_bar);
    if modal {
      out.push((Slot::Scrim, Widget::from(Scrim)));
      if let Some(w) = &self.bottom_sheet {
        out.push((Slot::Sheet, w.clone()));
      }
    }
    if let Some(drawer) = &self.drawer {
      out.push((Slot::Scrim, Widget::from(Scrim)));
      out.push((Slot::Drawer, drawer.clone()));
    }
    out
  }
}

impl Element for Scaffold {
  fn children(&self) -> Vec<Widget> {
    self.slots().into_iter().map(|(_, w)| w).collect()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let size = constraints.constrain(constraints.biggest());
    let (w, h) = (size.width, size.height);
    let slots: Vec<Slot> = self.slots().into_iter().map(|(s, _)| s).collect();
    let find = |slot: Slot| slots.iter().position(|s| *s == slot);
    let loose = Constraints::loose(size);

    let top =
      find(Slot::AppBar).map_or(0.0, |i| cx.layout_child(i, loose.tighten(Some(w), None)).height);
    let bottom = find(Slot::BottomNav).map_or(0.0, |i| {
      let s = cx.layout_child(i, loose.tighten(Some(w), None));
      cx.place_child(i, Offset::new(0.0, h - s.height));
      s.height
    });
    if let Some(i) = find(Slot::Body) {
      cx.layout_child(i, Constraints::tight(Size::new(w, (h - top - bottom).max(0.0))));
      cx.place_child(i, Offset::new(0.0, top));
    }
    let mut above = h - bottom;
    if let Some(i) = find(Slot::Sheet) {
      let sw = w.min(640.0);
      let s =
        cx.layout_child(i, Constraints::loose(Size::new(sw, h - top)).tighten(Some(sw), None));
      cx.place_child(i, Offset::new((w - sw) / 2.0, h - bottom - s.height));
      above -= s.height;
    }
    if let Some(i) = find(Slot::SnackBar) {
      let sw = (w - 32.0).min(600.0);
      let s = cx.layout_child(i, Constraints::loose(Size::new(sw, h)).tighten(Some(sw), None));
      above -= 16.0 + s.height;
      cx.place_child(i, Offset::new((w - sw) / 2.0, above));
    }
    if let Some(i) = find(Slot::Fab) {
      let s = cx.layout_child(i, loose);
      cx.place_child(i, Offset::new(w - 16.0 - s.width, above - 16.0 - s.height));
    }
    for (i, slot) in slots.iter().enumerate() {
      match slot {
        Slot::Scrim => {
          cx.layout_child(i, Constraints::tight(size));
        }
        Slot::Drawer => {
          let dw = (w - 56.0).min(360.0);
          cx.layout_child(i, Constraints::tight(Size::new(dw, h)));
        }
        _ => {}
      }
    }
    size
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = self.background_color.unwrap_or(cx.theme().color_scheme.surface);
    let rect = cx.rect();
    cx.renderer.fill_rect(rect, color, 0.0);
    cx.paint_children();
  }
}
