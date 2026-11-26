use std::sync::Arc;

use crate::{
  Border, Color, DisplayList, DrawCmd, Item, LayerId, Offset, Rect, Renderer, Size, TextPaint,
};

/// A [`Renderer`] that records primitives into a [`DisplayList`].
///
/// Size queries, `measure_text`, and `snap_rect` are forwarded to the real backend
/// so layout and painting behave identically to immediate rendering.
pub struct Recorder<'a> {
  backend: &'a dyn Renderer,
  items: DisplayList,
  clips: Vec<Rect>,
}

impl<'a> Recorder<'a> {
  pub fn new(backend: &'a dyn Renderer) -> Self {
    Recorder { backend, items: Vec::new(), clips: Vec::new() }
  }

  /// Inserts a child layer slot at the current position.
  pub fn child(&mut self, id: LayerId) {
    self.items.push(Item::Child(id));
  }

  /// Inserts an overlay layer slot drawn after the whole tree.
  pub fn overlay(&mut self, id: LayerId) {
    self.items.push(Item::Overlay(id));
  }

  /// Finishes recording and returns the built [`DisplayList`].
  pub fn finish(self) -> DisplayList {
    self.items
  }

  fn push_cmd(&mut self, cmd: DrawCmd, raw: Rect) {
    let clipped = self.clips.last().map_or(raw, |c| raw.intersect(c));
    if clipped.is_empty() {
      return;
    }
    // Pad 1 px for anti-aliased edge pixels.
    self.items.push(Item::Cmd(cmd, clipped.inflate(1.0)));
  }
}

impl Renderer for Recorder<'_> {
  fn size(&self) -> Size {
    self.backend.size()
  }

  fn measure_text(&self, text: &str, paint: &TextPaint) -> Size {
    self.backend.measure_text(text, paint)
  }

  fn clear(&mut self, color: Color) {
    let bounds = Rect::from_origin(Offset::ZERO, self.backend.size());
    self.items.push(Item::Cmd(DrawCmd::Clear(color), bounds));
  }

  fn fill_rect(&mut self, rect: Rect, color: Color, radius: f32) {
    if color.a == 0 || rect.is_empty() {
      return;
    }
    self.push_cmd(DrawCmd::FillRect { rect, color, radius }, rect);
  }

  fn stroke_rect(&mut self, rect: Rect, border: Border, radius: f32) {
    if border.color.a == 0 || border.width <= 0.0 || rect.is_empty() {
      return;
    }
    self.push_cmd(DrawCmd::StrokeRect { rect, border, radius }, rect);
  }

  fn draw_line(&mut self, from: Offset, to: Offset, color: Color, width: f32) {
    if color.a == 0 || width <= 0.0 {
      return;
    }
    let x0 = from.x.min(to.x);
    let y0 = from.y.min(to.y);
    let x1 = from.x.max(to.x);
    let y1 = from.y.max(to.y);
    let raw = Rect::new(x0, y0, x1 - x0, y1 - y0).inflate(width * 0.5);
    self.push_cmd(DrawCmd::Line { from, to, color, width }, raw);
  }

  fn fill_circle(&mut self, center: Offset, radius: f32, color: Color) {
    if color.a == 0 || radius <= 0.0 {
      return;
    }
    let raw = Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0);
    self.push_cmd(DrawCmd::FillCircle { center, radius, color }, raw);
  }

  fn stroke_circle(&mut self, center: Offset, radius: f32, border: Border) {
    if border.color.a == 0 || border.width <= 0.0 || radius <= 0.0 {
      return;
    }
    let r = radius + border.width * 0.5;
    let raw = Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0);
    self.push_cmd(DrawCmd::StrokeCircle { center, radius, border }, raw);
  }

  fn draw_text(&mut self, origin: Offset, text: &str, paint: &TextPaint) {
    if paint.color.a == 0 || text.is_empty() {
      return;
    }
    let size = self.backend.measure_text(text, paint);
    let raw = Rect::from_origin(origin, size);
    self.push_cmd(DrawCmd::Text { origin, text: Arc::from(text), paint: paint.clone() }, raw);
  }

  fn push_clip(&mut self, rect: Rect) {
    let effective = self.clips.last().map_or(rect, |c| c.intersect(&rect));
    self.clips.push(effective);
    self.items.push(Item::Cmd(DrawCmd::PushClip(rect), rect));
  }

  fn pop_clip(&mut self) {
    self.clips.pop();
    self.items.push(Item::Cmd(DrawCmd::PopClip, Rect::ZERO));
  }

  fn snap_rect(&self, rect: Rect) -> Rect {
    self.backend.snap_rect(rect)
  }
}
