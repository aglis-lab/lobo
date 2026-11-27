//! CSS Flexbox and Grid layout powered by [`taffy`].
//!
//! Exposes [`TaffyBox`] (full `taffy::Style`), [`LayoutItem`] (per-child `taffy::Style`),
//! and [`Grid`] (ergonomic CSS Grid shorthand), while delegating leaf measurement to
//! Lobo's real [`LayoutCx`] so text wrapping and intrinsic widget sizing work seamlessly.

use taffy::LayoutOutput;
use taffy::prelude::*;

use crate::layout::{Constraints, LayoutCx};
use crate::render::{Offset, Size};
use crate::{Element, Padding, Widget};

/// Attaches a [`taffy::Style`] to a child inside [`TaffyBox`] or [`Grid`]
/// (e.g. `grid_column`, `grid_row`, `flex_grow`, `align_self`, `margin`, `min_size`).
#[derive(Widget)]
#[non_exhaustive]
pub struct LayoutItem {
  pub style: taffy::Style,
  pub child: Widget,
}

impl Element for LayoutItem {
  fn children(&self) -> Vec<Widget> {
    vec![self.child.clone()]
  }

  fn parent_data(&self) -> Option<Box<dyn std::any::Any>> {
    Some(Box::new(self.style.clone()))
  }
}

/// Full-featured CSS Flexbox / CSS Grid / Block layout container backed by [`taffy`].
#[derive(Widget)]
#[non_exhaustive]
pub struct TaffyBox {
  pub style: taffy::Style,
  pub children: Vec<Widget>,
}

impl Element for TaffyBox {
  fn children(&self) -> Vec<Widget> {
    self.children.clone()
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let n = cx.child_count();
    let mut tree: TaffyTree<usize> = TaffyTree::with_capacity(n + 1);
    let mut child_nodes = Vec::with_capacity(n);

    for i in 0..n {
      let child_style = cx.child_data::<taffy::Style>(i).unwrap_or_default();
      let node = tree.new_leaf_with_context(child_style, i).expect("taffy leaf");
      child_nodes.push(node);
    }

    let mut root_style = self.style.clone();
    if root_style.size.width.is_auto()
      && constraints.has_bounded_width()
      && (constraints.min_width - constraints.max_width).abs() < 0.01
    {
      root_style.size.width = length(constraints.max_width);
    }
    if root_style.size.height.is_auto()
      && constraints.has_bounded_height()
      && (constraints.min_height - constraints.max_height).abs() < 0.01
    {
      root_style.size.height = length(constraints.max_height);
    }
    let root = tree.new_with_children(root_style, &child_nodes).expect("taffy root");

    let available = taffy::Size {
      width: if constraints.has_bounded_width() {
        AvailableSpace::Definite(constraints.max_width)
      } else {
        AvailableSpace::MaxContent
      },
      height: if constraints.has_bounded_height() {
        AvailableSpace::Definite(constraints.max_height)
      } else {
        AvailableSpace::MaxContent
      },
    };

    tree
      .compute_layout_with_measure(
        root,
        available,
        |inputs, _node_id, node_context, _style| {
          let Some(&mut child_idx) = node_context else {
            return LayoutOutput::HIDDEN;
          };
          let (min_w, max_w) = match inputs.known_dimensions.width {
            Some(w) => (w, w),
            None => match inputs.available_space.width {
              AvailableSpace::Definite(w) => (0.0, w),
              AvailableSpace::MinContent => (0.0, 0.0),
              AvailableSpace::MaxContent => (0.0, f32::INFINITY),
            },
          };
          let (min_h, max_h) = match inputs.known_dimensions.height {
            Some(h) => (h, h),
            None => match inputs.available_space.height {
              AvailableSpace::Definite(h) => (0.0, h),
              AvailableSpace::MinContent => (0.0, 0.0),
              AvailableSpace::MaxContent => (0.0, f32::INFINITY),
            },
          };
          let measured = cx.layout_child(
            child_idx,
            Constraints { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h },
          );
          LayoutOutput::from_outer_size(taffy::Size { width: measured.width, height: measured.height })
        },
      )
      .expect("taffy compute_layout");

    for (i, &node) in child_nodes.iter().enumerate() {
      let layout = tree.layout(node).expect("taffy child layout");
      let child_size = Size::new(layout.size.width.max(0.0), layout.size.height.max(0.0));
      cx.layout_child(i, Constraints::tight(child_size));
      cx.place_child(i, Offset::new(layout.location.x, layout.location.y));
    }

    let root_layout = tree.layout(root).expect("taffy root layout");
    constraints.constrain(Size::new(root_layout.size.width, root_layout.size.height))
  }
}

/// Ergonomic CSS Grid container powered by [`taffy`].
#[derive(Widget)]
#[non_exhaustive]
pub struct Grid {
  /// Number of equal `1fr` columns (0 defaults to 2).
  #[widget(optional)]
  pub columns: usize,
  /// Uniform gap between rows and columns.
  #[widget(optional)]
  pub gap: f32,
  #[widget(optional)]
  pub column_gap: Option<f32>,
  #[widget(optional)]
  pub row_gap: Option<f32>,
  #[widget(optional)]
  pub padding: Padding,
  pub children: Vec<Widget>,
}

impl Element for Grid {
  fn build(&self, _context: &crate::Context) -> Option<Widget> {
    let cols = if self.columns == 0 { 2 } else { self.columns };
    let style = taffy::Style {
      display: Display::Grid,
      grid_template_columns: vec![fr(1.0); cols],
      gap: taffy::Size {
        width: length(self.column_gap.unwrap_or(self.gap)),
        height: length(self.row_gap.unwrap_or(self.gap)),
      },
      padding: taffy::Rect {
        left: length(self.padding.left),
        right: length(self.padding.right),
        top: length(self.padding.top),
        bottom: length(self.padding.bottom),
      },
      ..Default::default()
    };
    Some(Widget::from(TaffyBox { style, children: self.children.clone() }))
  }
}
