//! Flutter-style [`CustomPainter`] and [`CustomPaint`] widget.
//!
//! Prefer composing declarative widgets in `Element::build` for normal UI.
//! Use [`CustomPaint`] + [`CustomPainter`] only when you need custom 2D drawing
//! (charts, gauges, vector paths) that built-in widgets do not cover.

use std::any::Any;
use std::rc::Rc;

use crate::layout::{Constraints, LayoutCx, PaintCx};
use crate::render::{Recorder, Rect, Size};
use crate::theme::ThemeData;
use crate::{Element, Widget};

/// Custom 2D canvas painter, modeled after Flutter's `CustomPainter`.
///
/// Implement `paint` for your custom drawing and `should_repaint` to tell the
/// framework when a new painter instance actually needs to re-record its display
/// list (returning `false` keeps the cached display list at zero cost).
pub trait CustomPainter: 'static {
  /// Paints onto `canvas` inside `rect` using the active `theme`.
  fn paint(&self, canvas: &mut Recorder<'_>, rect: Rect, theme: &ThemeData);

  /// Returns `true` when this painter's output may differ from `old` and must
  /// be repainted. Defaults to `true`.
  fn should_repaint(&self, _old: &Self) -> bool {
    true
  }
}

trait ErasedPainter: 'static {
  fn paint(&self, canvas: &mut Recorder<'_>, rect: Rect, theme: &ThemeData);
  fn as_any(&self) -> &dyn Any;
  fn should_repaint_erased(&self, old: &dyn ErasedPainter) -> bool;
}

impl<P: CustomPainter> ErasedPainter for P {
  fn paint(&self, canvas: &mut Recorder<'_>, rect: Rect, theme: &ThemeData) {
    CustomPainter::paint(self, canvas, rect, theme);
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn should_repaint_erased(&self, old: &dyn ErasedPainter) -> bool {
    match old.as_any().downcast_ref::<P>() {
      Some(prev) => CustomPainter::should_repaint(self, prev),
      None => true,
    }
  }
}

/// Type-erased handle to a [`CustomPainter`]. Any `P: CustomPainter` converts into `Painter`.
#[derive(Clone)]
pub struct Painter(Rc<dyn ErasedPainter>);

impl Painter {
  pub fn new<P: CustomPainter>(painter: P) -> Self {
    Painter(Rc::new(painter))
  }

  fn should_repaint(&self, old: &Self) -> bool {
    if Rc::ptr_eq(&self.0, &old.0) {
      return false;
    }
    self.0.should_repaint_erased(&*old.0)
  }
}

impl<P: CustomPainter> From<P> for Painter {
  fn from(painter: P) -> Self {
    Painter::new(painter)
  }
}

/// A widget that delegates custom 2D drawing to a [`CustomPainter`].
///
/// When rebuilt, Lobo calls [`CustomPainter::should_repaint`] against the
/// previous painter and skips repainting completely when it returns `false`.
#[derive(Widget)]
#[non_exhaustive]
pub struct CustomPaint {
  pub painter: Painter,
  /// Preferred size when there is no `child` (defaults to `Size::ZERO`).
  #[widget(optional)]
  pub size: Size,
  /// Optional child widget painted on top of the custom canvas.
  #[widget(optional)]
  pub child: Option<Widget>,
}

impl Element for CustomPaint {
  fn children(&self) -> Vec<Widget> {
    self.child.iter().cloned().collect()
  }

  fn as_any(&self) -> Option<&dyn Any> {
    Some(self)
  }

  fn should_repaint(&self, old: &dyn Any) -> bool {
    let Some(prev) = old.downcast_ref::<CustomPaint>() else {
      return true;
    };
    if self.size != prev.size || self.child.is_some() || prev.child.is_some() {
      return true;
    }
    self.painter.should_repaint(&prev.painter)
  }

  fn layout(&self, cx: &mut LayoutCx, constraints: Constraints) -> Size {
    if cx.child_count() > 0 {
      cx.layout_stacked(constraints)
    } else {
      constraints.constrain(self.size)
    }
  }

  fn paint(&self, cx: &mut PaintCx) {
    let rect = cx.rect();
    let theme = cx.theme().clone();
    self.painter.0.paint(&mut cx.renderer, rect, &theme);
    cx.paint_children();
  }
}
