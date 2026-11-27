//! Drawing trait, geometry, paint types, and retained display-list scene.
//!
//! Backends implement [`Renderer`] primitives; [`Recorder`] builds per-layer
//! [`DisplayList`]s and [`Scene`] diffs them to submit one clipped batch per frame.

mod cmd;
mod geometry;
mod paint;
mod recorder;
mod scene;

pub use cmd::{DisplayList, DrawCmd, Item, LayerId};
pub use geometry::{HD_720, Offset, Rect, Size};
pub use paint::{Border, BoxPaint, Color, IconData, TextPaint, ToggleColors, TrackColors};
pub use recorder::Recorder;
pub use scene::{Scene, SceneTimings};

/// The one interface through which all text and widgets are drawn.
///
/// Coordinates are logical pixels from the top-left of the canvas.
pub trait Renderer {
  /// Canvas size in logical pixels.
  fn size(&self) -> Size;

  /// Size of `text` on one line (no wrapping). Layout uses this.
  fn measure_text(&self, text: &str, paint: &TextPaint) -> Size;

  /// Fills the whole canvas, ignoring the clip.
  fn clear(&mut self, color: Color);

  /// Fills `rect`. A translucent `color` is blended over what is below.
  fn fill_rect(&mut self, rect: Rect, color: Color, radius: f32);

  /// Outlines `rect`, inside its bounds.
  fn stroke_rect(&mut self, rect: Rect, border: Border, radius: f32);

  fn draw_line(&mut self, from: Offset, to: Offset, color: Color, width: f32);

  fn fill_circle(&mut self, center: Offset, radius: f32, color: Color);

  fn stroke_circle(&mut self, center: Offset, radius: f32, border: Border);

  /// Draws `text` on one line with its top-left at `origin`.
  fn draw_text(&mut self, origin: Offset, text: &str, paint: &TextPaint);

  /// Restricts drawing to `rect` (intersected with the current clip) until `pop_clip`.
  fn push_clip(&mut self, rect: Rect);

  fn pop_clip(&mut self);

  /// `rect` aligned to this backend's device grid: what the backend will actually cover.
  /// Pixel backends return it unchanged.
  fn snap_rect(&self, rect: Rect) -> Rect {
    rect
  }

  /// Draws one damaged frame batch. Surfaces keep their pixels between frames;
  /// only `damage` is redrawn.
  fn draw_batch(&mut self, _damage: Rect, cmds: &[DrawCmd]) {
    for cmd in cmds {
      cmd.apply(self);
    }
  }
}
