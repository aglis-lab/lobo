//! General layout widgets (Flutter's `widgets` library): rows, stacks, alignment,
//! sizing, flex, icons, scrolling lists and wrapping.

use crate::layout::{Constraints, Flex, FlexFit, LayoutCx, PaintCx, focus};
use crate::render::{Color, IconData, Offset, Rect, Renderer, Size, draw_icon};
use crate::{Element, Padding, Widget};

// ---------------------------------------------------------------------------
// Alignment
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
  Horizontal,
  #[default]
  Vertical,
}

/// How children share the free space along a `Row` / `Column`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MainAxisAlignment {
  #[default]
  Start,
  Center,
  End,
  SpaceBetween,
  SpaceAround,
  SpaceEvenly,
}

/// Where children sit across a `Row` / `Column`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CrossAxisAlignment {
  #[default]
  Start,
  Center,
  End,
  /// Children are forced to the full cross size.
  Stretch,
}

/// Whether a `Row` / `Column` fills its main axis or shrinks to its children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MainAxisSize {
  Min,
  #[default]
  Max,
}

/// A point in a box: `x` and `y` run from -1 (left/top) to 1 (right/bottom).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alignment {
  pub x: f32,
  pub y: f32,
}

impl Default for Alignment {
  fn default() -> Self {
    Alignment::CENTER
  }
}

impl Alignment {
  pub const TOP_LEFT: Alignment = Alignment { x: -1.0, y: -1.0 };
  pub const TOP_CENTER: Alignment = Alignment { x: 0.0, y: -1.0 };
  pub const TOP_RIGHT: Alignment = Alignment { x: 1.0, y: -1.0 };
  pub const CENTER_LEFT: Alignment = Alignment { x: -1.0, y: 0.0 };
  pub const CENTER: Alignment = Alignment { x: 0.0, y: 0.0 };
  pub const CENTER_RIGHT: Alignment = Alignment { x: 1.0, y: 0.0 };
  pub const BOTTOM_LEFT: Alignment = Alignment { x: -1.0, y: 1.0 };
  pub const BOTTOM_CENTER: Alignment = Alignment { x: 0.0, y: 1.0 };
  pub const BOTTOM_RIGHT: Alignment = Alignment { x: 1.0, y: 1.0 };

  /// Horizontal offset of a child inside `free` px of spare width.
  pub fn offset(&self, free: f32) -> f32 {
    (self.x + 1.0) / 2.0 * free.max(0.0)
  }

  /// Vertical offset of a child inside `free` px of spare height.
  pub fn offset_y(&self, free: f32) -> f32 {
    (self.y + 1.0) / 2.0 * free.max(0.0)
  }

  /// Top-left of a `child` box aligned in `size`.
  pub fn place(&self, size: Size, child: Size) -> Offset {
    Offset::new(self.offset(size.width - child.width), self.offset_y(size.height - child.height))
  }
}

// ---------------------------------------------------------------------------
// Flex (Row / Column)
// ---------------------------------------------------------------------------

/// Flutter's flex algorithm: fixed children first, flexible children share what's left.
pub(crate) fn flex_layout(
  cx: &mut LayoutCx,
  constraints: Constraints,
  axis: Axis,
  spacing: f32,
  main_align: MainAxisAlignment,
  cross_align: CrossAxisAlignment,
  main_size: MainAxisSize,
) -> Size {
  let n = cx.child_count();
  let horizontal = axis == Axis::Horizontal;
  let (main_max, cross_max, cross_min) = if horizontal {
    (constraints.max_width, constraints.max_height, constraints.min_height)
  } else {
    (constraints.max_height, constraints.max_width, constraints.min_width)
  };
  let child_constraints = |main_min: f32, main_max: f32| {
    let (cmin, cmax) = if cross_align == CrossAxisAlignment::Stretch && cross_max.is_finite() {
      (cross_max, cross_max)
    } else {
      (0.0, cross_max)
    };
    if horizontal {
      Constraints { min_width: main_min, max_width: main_max, min_height: cmin, max_height: cmax }
    } else {
      Constraints { min_width: cmin, max_width: cmax, min_height: main_min, max_height: main_max }
    }
  };
  let main_of = |s: Size| if horizontal { s.width } else { s.height };
  let cross_of = |s: Size| if horizontal { s.height } else { s.width };

  let flexes: Vec<Option<Flex>> = (0..n).map(|i| cx.child_flex(i)).collect();
  let total_flex: f32 = flexes.iter().flatten().map(|f| f.flex.max(0.0)).sum();
  let flexible = total_flex > 0.0 && main_max.is_finite();

  // Pass 1: fixed children.
  let mut sizes = vec![Size::ZERO; n];
  let mut used = spacing * n.saturating_sub(1) as f32;
  for i in 0..n {
    if flexible && flexes[i].is_some() {
      continue;
    }
    sizes[i] = cx.layout_child(i, child_constraints(0.0, f32::INFINITY));
    used += main_of(sizes[i]);
  }
  // Pass 2: flexible children share the rest.
  if flexible {
    let free = (main_max - used).max(0.0);
    for i in 0..n {
      if let Some(f) = flexes[i] {
        let share = free * f.flex.max(0.0) / total_flex;
        let min = if f.fit == FlexFit::Tight { share } else { 0.0 };
        sizes[i] = cx.layout_child(i, child_constraints(min, share));
        used += main_of(sizes[i]);
      }
    }
  }

  let main_total =
    if main_size == MainAxisSize::Max && main_max.is_finite() { main_max } else { used };
  let mut cross_total = sizes.iter().map(|s| cross_of(*s)).fold(cross_min, f32::max);
  if cross_align == CrossAxisAlignment::Stretch && cross_max.is_finite() {
    cross_total = cross_max;
  }
  let cross_total = cross_total.min(cross_max);

  let free = (main_total - used).max(0.0);
  let (mut pos, gap) = match main_align {
    MainAxisAlignment::Start => (0.0, 0.0),
    MainAxisAlignment::Center => (free / 2.0, 0.0),
    MainAxisAlignment::End => (free, 0.0),
    MainAxisAlignment::SpaceBetween if n > 1 => (0.0, free / (n - 1) as f32),
    MainAxisAlignment::SpaceBetween => (0.0, 0.0),
    MainAxisAlignment::SpaceAround => (free / n.max(1) as f32 / 2.0, free / n.max(1) as f32),
    MainAxisAlignment::SpaceEvenly => (free / (n + 1) as f32, free / (n + 1) as f32),
  };
  for (i, size) in sizes.iter().enumerate() {
    let cross = match cross_align {
      CrossAxisAlignment::Start | CrossAxisAlignment::Stretch => 0.0,
      CrossAxisAlignment::Center => (cross_total - cross_of(*size)) / 2.0,
      CrossAxisAlignment::End => cross_total - cross_of(*size),
    };
    cx.place_child(i, if horizontal { Offset::new(pos, cross) } else { Offset::new(cross, pos) });
    pos += main_of(*size) + spacing + gap;
  }
  if horizontal { Size::new(main_total, cross_total) } else { Size::new(cross_total, main_total) }
}

/// Lays children out horizontally. Fills the available width by default.
#[derive(Widget)]
#[non_exhaustive]
pub struct Row {
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

impl Element for Row {
  fn children(&self) -> Vec<Widget> {
    self.children.clone()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    flex_layout(
      cx,
      constraints,
      Axis::Horizontal,
      self.spacing,
      self.main_axis_alignment,
      self.cross_axis_alignment,
      self.main_axis_size,
    )
  }
}

/// In a `Row` / `Column`: takes an equal (or `flex`-weighted) share of the free space.
#[derive(Widget)]
#[non_exhaustive]
pub struct Expanded {
  /// Share weight; 0 counts as 1.
  #[widget(optional)]
  pub flex: f32,
  pub child: Widget,
}

impl Element for Expanded {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn parent_data(&self) -> Option<Box<dyn std::any::Any>> {
    Some(Box::new(Flex {
      flex: if self.flex > 0.0 { self.flex } else { 1.0 },
      fit: FlexFit::Tight,
    }))
  }
}

/// Like `Expanded`, but the child may be smaller than its share.
#[derive(Widget)]
#[non_exhaustive]
pub struct Flexible {
  #[widget(optional)]
  pub flex: f32,
  pub child: Widget,
}

impl Element for Flexible {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn parent_data(&self) -> Option<Box<dyn std::any::Any>> {
    Some(Box::new(Flex {
      flex: if self.flex > 0.0 { self.flex } else { 1.0 },
      fit: FlexFit::Loose,
    }))
  }
}

/// Empty flexible space in a `Row` / `Column`.
#[derive(Widget)]
#[non_exhaustive]
pub struct Spacer {
  #[widget(optional)]
  pub flex: f32,
}

impl Element for Spacer {
  fn parent_data(&self) -> Option<Box<dyn std::any::Any>> {
    Some(Box::new(Flex {
      flex: if self.flex > 0.0 { self.flex } else { 1.0 },
      fit: FlexFit::Tight,
    }))
  }

  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    constraints.smallest()
  }
}

// ---------------------------------------------------------------------------
// Single-child layout
// ---------------------------------------------------------------------------

/// Fixed width and/or height. Without a child it is empty space.
#[derive(Widget)]
#[non_exhaustive]
pub struct SizedBox {
  #[widget(optional)]
  pub width: Option<f32>,
  #[widget(optional)]
  pub height: Option<f32>,
  #[widget(optional)]
  pub child: Option<Widget>,
}

impl Element for SizedBox {
  fn children(&self) -> Vec<Widget> {
    self.child.iter().cloned().collect()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let c = constraints.tighten(self.width, self.height);
    if cx.child_count() > 0 { cx.layout_child(0, c) } else { c.constrain(Size::ZERO) }
  }
}

/// Aligns its child inside the available space (and takes all of it).
#[derive(Widget)]
#[non_exhaustive]
pub struct Align {
  #[widget(optional)]
  pub alignment: Alignment,
  pub child: Widget,
}

impl Element for Align {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    align_child(cx, constraints, self.alignment)
  }
}

fn align_child(cx: &mut LayoutCx, constraints: Constraints, alignment: Alignment) -> Size {
  let child = cx.layout_child(0, constraints.loosen());
  let size = Size::new(
    if constraints.has_bounded_width() { constraints.max_width } else { child.width },
    if constraints.has_bounded_height() { constraints.max_height } else { child.height },
  );
  let size = constraints.constrain(size);
  cx.place_child(0, alignment.place(size, child));
  size
}

/// Centers its child.
#[derive(Widget)]
#[non_exhaustive]
pub struct Center {
  pub child: Widget,
}

impl Element for Center {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    align_child(cx, constraints, Alignment::CENTER)
  }
}

// ---------------------------------------------------------------------------
// Stack
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackFit {
  /// Children may be smaller than the stack.
  #[default]
  Loose,
  /// Children (and the stack) take all the available space.
  Expand,
}

/// Children on top of each other, later ones above. Non-positioned children are
/// placed with `alignment`; `Positioned` children by their edges.
#[derive(Widget)]
#[non_exhaustive]
pub struct Stack {
  #[widget(optional)]
  pub alignment: Option<Alignment>,
  #[widget(optional)]
  pub fit: StackFit,
  pub children: Vec<Widget>,
}

/// Edges of a `Positioned` child, read by its `Stack`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PositionedData {
  pub left: Option<f32>,
  pub top: Option<f32>,
  pub right: Option<f32>,
  pub bottom: Option<f32>,
  pub width: Option<f32>,
  pub height: Option<f32>,
}

impl Element for Stack {
  fn children(&self) -> Vec<Widget> {
    self.children.clone()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let alignment = self.alignment.unwrap_or(Alignment::TOP_LEFT);
    let inner = match self.fit {
      StackFit::Loose => constraints.loosen(),
      StackFit::Expand => Constraints::tight(constraints.constrain(constraints.biggest())),
    };
    // Non-positioned children decide the size.
    let mut size = constraints.smallest();
    let mut has_flow = false;
    for i in 0..cx.child_count() {
      if cx.child_data::<PositionedData>(i).is_none() {
        let s = cx.layout_child(i, inner);
        size.width = size.width.max(s.width);
        size.height = size.height.max(s.height);
        has_flow = true;
      }
    }
    if !has_flow || self.fit == StackFit::Expand {
      size = constraints.constrain(constraints.biggest());
    }
    let size = constraints.constrain(size);
    for i in 0..cx.child_count() {
      match cx.child_data::<PositionedData>(i) {
        None => {
          let s = cx.child_size(i);
          cx.place_child(i, alignment.place(size, s));
        }
        Some(p) => {
          let width = p.width.or(match (p.left, p.right) {
            (Some(l), Some(r)) => Some((size.width - l - r).max(0.0)),
            _ => None,
          });
          let height = p.height.or(match (p.top, p.bottom) {
            (Some(t), Some(b)) => Some((size.height - t - b).max(0.0)),
            _ => None,
          });
          let c = Constraints::loose(size).tighten(width, height);
          let s = cx.layout_child(i, c);
          let x = p.left.unwrap_or_else(|| {
            p.right.map_or(alignment.offset(size.width - s.width), |r| size.width - r - s.width)
          });
          let y = p.top.unwrap_or_else(|| {
            p.bottom
              .map_or(alignment.offset_y(size.height - s.height), |b| size.height - b - s.height)
          });
          cx.place_child(i, Offset::new(x, y));
        }
      }
    }
    size
  }
}

/// A `Stack` child placed by its distance from the stack's edges.
#[derive(Widget)]
#[non_exhaustive]
pub struct Positioned {
  #[widget(optional)]
  pub left: Option<f32>,
  #[widget(optional)]
  pub top: Option<f32>,
  #[widget(optional)]
  pub right: Option<f32>,
  #[widget(optional)]
  pub bottom: Option<f32>,
  #[widget(optional)]
  pub width: Option<f32>,
  #[widget(optional)]
  pub height: Option<f32>,
  pub child: Widget,
}

impl Element for Positioned {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn parent_data(&self) -> Option<Box<dyn std::any::Any>> {
    Some(Box::new(PositionedData {
      left: self.left,
      top: self.top,
      right: self.right,
      bottom: self.bottom,
      width: self.width,
      height: self.height,
    }))
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    cx.layout_child(0, constraints)
  }
}

// ---------------------------------------------------------------------------
// Icon
// ---------------------------------------------------------------------------

/// A Material icon (see `Icons`). Takes the inherited text color unless `color` is set.
#[derive(Widget)]
#[non_exhaustive]
pub struct Icon {
  pub icon: IconData,
  /// Size in px; 0 is the default 24.
  #[widget(optional)]
  pub size: f32,
  #[widget(optional)]
  pub color: Option<Color>,
}

impl Element for Icon {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let s = if self.size > 0.0 { self.size } else { 24.0 };
    constraints.constrain(Size::new(s, s))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let color = self.color.unwrap_or(cx.text_paint().color);
    let rect = cx.rect();
    draw_icon(&mut cx.renderer, rect, self.icon, color);
  }
}

// ---------------------------------------------------------------------------
// ListView
// ---------------------------------------------------------------------------

struct ScrollState {
  offset: crate::Signal<f32>,
  max_offset: f32,
  /// Canvas rect of the viewport in the last frame.
  viewport: Rect,
  region_id: Option<u64>,
}

impl Default for ScrollState {
  fn default() -> Self {
    ScrollState {
      offset: crate::Signal::new(0.0),
      max_offset: 0.0,
      viewport: Rect::ZERO,
      region_id: None,
    }
  }
}

/// A vertically scrolling list. It scrolls to keep the keyboard-focused child visible,
/// and `scroll_offset` (if set) scrolls it explicitly.
#[derive(Widget)]
#[non_exhaustive]
pub struct ListView {
  #[widget(optional)]
  pub padding: Padding,
  #[widget(optional)]
  pub spacing: f32,
  /// Pixels scrolled from the top. Unset: follows the keyboard focus.
  #[widget(optional)]
  pub scroll_offset: Option<f32>,
  pub children: Vec<Widget>,
}

impl Element for ListView {
  fn children(&self) -> Vec<Widget> {
    self.children.clone()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let width = constraints.max_width;
    // Children may be narrower than the list (rows, buttons); full-width ones use `max_width`.
    let child_c = Constraints {
      min_width: 0.0,
      max_width: (width - self.padding.horizontal()).max(0.0),
      min_height: 0.0,
      max_height: f32::INFINITY,
    };
    let mut y = self.padding.top;
    let mut tops = Vec::new();
    for i in 0..cx.child_count() {
      let s = cx.layout_child(i, child_c);
      tops.push((y, s.height));
      y += s.height + self.spacing;
    }
    let content = y - if cx.child_count() > 0 { self.spacing } else { 0.0 } + self.padding.bottom;
    let size = constraints.constrain(Size::new(
      width,
      if constraints.has_bounded_height() { constraints.max_height } else { content },
    ));

    let state = cx.state::<ScrollState>();
    let mut st = state.borrow_mut();
    let max_offset = (content - size.height).max(0.0);
    st.max_offset = max_offset;
    let cur_offset = st.offset.get();
    let mut offset = self.scroll_offset.unwrap_or(cur_offset);
    if self.scroll_offset.is_none() {
      // Keep the focused widget (as of the last frame) in view, if it's inside this list.
      if let Some(f) = focus::current_rect() {
        let vp = st.viewport;
        let in_list = f.x >= vp.x && f.right() <= vp.right() + 0.5 && !vp.is_empty();
        let content_top = vp.y - cur_offset;
        if in_list && f.y >= content_top && f.bottom() <= content_top + content + 0.5 {
          if f.y < vp.y {
            offset -= vp.y - f.y;
          } else if f.bottom() > vp.bottom() {
            offset += f.bottom() - vp.bottom();
          }
        }
      }
    }
    let offset = offset.clamp(0.0, max_offset);
    if (offset - cur_offset).abs() > 0.01 {
      st.offset.set(offset);
    }
    for (i, (top, _)) in tops.iter().enumerate() {
      cx.place_child(i, Offset::new(self.padding.left, top - offset));
    }
    size
  }

  fn paint(&self, cx: &mut PaintCx) {
    let rect = cx.rect();
    let state = cx.state::<ScrollState>();
    let id = {
      let mut st = state.borrow_mut();
      st.viewport = rect;
      *st.region_id.get_or_insert_with(crate::pointer::alloc_region_id)
    };
    if self.scroll_offset.is_none() {
      let st_cb = state.clone();
      cx.register_pointer(
        id,
        std::rc::Rc::new(move |ev: &crate::PointerEvent, _| {
          if let crate::PointerEvent::Scroll { delta_y, .. } = ev {
            let st = st_cb.borrow();
            let next = (st.offset.peek() + *delta_y).clamp(0.0, st.max_offset);
            if (next - st.offset.peek()).abs() > 0.01 {
              st.offset.set(next);
              return true;
            }
          }
          false
        }),
      );
    }
    cx.renderer.push_clip(rect);
    cx.paint_children();
    cx.renderer.pop_clip();
  }
}

// ---------------------------------------------------------------------------
// Wrap
// ---------------------------------------------------------------------------

/// Lays children out in rows, starting a new row when one is full (like chips).
#[derive(Widget)]
#[non_exhaustive]
pub struct Wrap {
  /// Gap between children in a row.
  #[widget(optional)]
  pub spacing: f32,
  /// Gap between rows.
  #[widget(optional)]
  pub run_spacing: f32,
  pub children: Vec<Widget>,
}

impl Element for Wrap {
  fn children(&self) -> Vec<Widget> {
    self.children.clone()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let max_w = constraints.max_width;
    let (mut x, mut y, mut row_h, mut width) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for i in 0..cx.child_count() {
      let s = cx.layout_child(i, Constraints::loose(Size::new(max_w, f32::INFINITY)));
      if x > 0.0 && x + s.width > max_w {
        x = 0.0;
        y += row_h + self.run_spacing;
        row_h = 0.0;
      }
      cx.place_child(i, Offset::new(x, y));
      x += s.width + self.spacing;
      row_h = row_h.max(s.height);
      width = width.max(x - self.spacing);
    }
    constraints.constrain(Size::new(width, y + row_h))
  }
}
