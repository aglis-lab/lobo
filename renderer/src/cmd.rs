use std::sync::Arc;

use crate::{Border, Color, Offset, Rect, Renderer, TextPaint};

/// Stable identifier of one retained layer in a [`Scene`](crate::Scene).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(pub u64);

/// Recorded primitive drawing command.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCmd {
  Clear(Color),
  FillRect { rect: Rect, color: Color, radius: f32 },
  StrokeRect { rect: Rect, border: Border, radius: f32 },
  Line { from: Offset, to: Offset, color: Color, width: f32 },
  FillCircle { center: Offset, radius: f32, color: Color },
  StrokeCircle { center: Offset, radius: f32, border: Border },
  Text { origin: Offset, text: Arc<str>, paint: TextPaint },
  PushClip(Rect),
  PopClip,
}

impl DrawCmd {
  /// Replays this command onto `r`.
  pub fn apply<R: Renderer + ?Sized>(&self, r: &mut R) {
    match self {
      DrawCmd::Clear(color) => r.clear(*color),
      DrawCmd::FillRect { rect, color, radius } => r.fill_rect(*rect, *color, *radius),
      DrawCmd::StrokeRect { rect, border, radius } => r.stroke_rect(*rect, *border, *radius),
      DrawCmd::Line { from, to, color, width } => r.draw_line(*from, *to, *color, *width),
      DrawCmd::FillCircle { center, radius, color } => r.fill_circle(*center, *radius, *color),
      DrawCmd::StrokeCircle { center, radius, border } => {
        r.stroke_circle(*center, *radius, *border)
      }
      DrawCmd::Text { origin, text, paint } => r.draw_text(*origin, text, paint),
      DrawCmd::PushClip(rect) => r.push_clip(*rect),
      DrawCmd::PopClip => r.pop_clip(),
    }
  }
}

/// One entry in a layer's [`DisplayList`].
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
  /// A primitive command and its clipped, anti-alias-padded canvas bounds.
  Cmd(DrawCmd, Rect),
  /// Slot where a child layer's items are expanded inline.
  Child(LayerId),
  /// Slot where an overlay layer's items are expanded after the whole tree.
  Overlay(LayerId),
}

/// Retained drawing output of a single layer.
pub type DisplayList = Vec<Item>;
