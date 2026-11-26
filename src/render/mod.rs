//! Drawing backends.
//!
//! Every widget draws through [`Renderer`], in logical pixels (1 px = 1 Material dp).
//! A backend implements the drawing primitives (rects, borders, lines, circles, text,
//! clipping). The widget-level methods (`draw_checkbox`, `draw_switch`, ...) have
//! default implementations built from those primitives, so a pixel backend
//! (a texture, `wgpu`, Vello) only has to implement the primitives. A backend with a
//! coarse grid, like [`TerminalRenderer`], overrides them to stay readable.

pub mod terminal;

use std::ops::{Add, Sub};

pub use terminal::TerminalRenderer;

use crate::{Padding, Widget};

/// 720p: the default canvas size, in logical pixels.
pub const HD_720: Size = Size { width: 1280.0, height: 720.0 };

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offset {
  pub x: f32,
  pub y: f32,
}

impl Offset {
  pub const ZERO: Offset = Offset { x: 0.0, y: 0.0 };

  pub const fn new(x: f32, y: f32) -> Self {
    Offset { x, y }
  }
}

impl Add for Offset {
  type Output = Offset;
  fn add(self, o: Offset) -> Offset {
    Offset::new(self.x + o.x, self.y + o.y)
  }
}

impl Sub for Offset {
  type Output = Offset;
  fn sub(self, o: Offset) -> Offset {
    Offset::new(self.x - o.x, self.y - o.y)
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
  pub width: f32,
  pub height: f32,
}

impl Size {
  pub const ZERO: Size = Size { width: 0.0, height: 0.0 };

  pub const fn new(width: f32, height: f32) -> Self {
    Size { width, height }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
  pub x: f32,
  pub y: f32,
  pub width: f32,
  pub height: f32,
}

impl Rect {
  pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
    Rect { x, y, width, height }
  }

  pub fn from_origin(origin: Offset, size: Size) -> Self {
    Rect::new(origin.x, origin.y, size.width, size.height)
  }

  pub fn origin(&self) -> Offset {
    Offset::new(self.x, self.y)
  }

  pub fn size(&self) -> Size {
    Size::new(self.width, self.height)
  }

  pub fn right(&self) -> f32 {
    self.x + self.width
  }

  pub fn bottom(&self) -> f32 {
    self.y + self.height
  }

  pub fn center(&self) -> Offset {
    Offset::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
  }

  pub fn is_empty(&self) -> bool {
    self.width <= 0.0 || self.height <= 0.0
  }

  pub fn contains(&self, p: Offset) -> bool {
    p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
  }

  pub fn translate(&self, by: Offset) -> Rect {
    Rect::new(self.x + by.x, self.y + by.y, self.width, self.height)
  }

  /// Shrinks by `padding` on each side; never below zero size.
  pub fn deflate(&self, padding: Padding) -> Rect {
    Rect::new(
      self.x + padding.left,
      self.y + padding.top,
      (self.width - padding.left - padding.right).max(0.0),
      (self.height - padding.top - padding.bottom).max(0.0),
    )
  }

  /// Grows by `by` on every side (negative shrinks).
  pub fn inflate(&self, by: f32) -> Rect {
    Rect::new(
      self.x - by,
      self.y - by,
      (self.width + 2.0 * by).max(0.0),
      (self.height + 2.0 * by).max(0.0),
    )
  }

  /// Overlap of both rects; empty (zero size) when they don't overlap.
  pub fn intersect(&self, other: &Rect) -> Rect {
    let x = self.x.max(other.x);
    let y = self.y.max(other.y);
    let right = self.right().min(other.right());
    let bottom = self.bottom().min(other.bottom());
    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
  }

  /// A rect of `size` centered in this one.
  pub fn center_rect(&self, size: Size) -> Rect {
    Rect::new(
      self.x + (self.width - size.width) / 2.0,
      self.y + (self.height - size.height) / 2.0,
      size.width,
      size.height,
    )
  }
}

// ---------------------------------------------------------------------------
// Paint
// ---------------------------------------------------------------------------

/// sRGB color with straight (non-premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: u8,
}

impl Color {
  pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
  pub const BLACK: Color = Color::rgb(0, 0, 0);
  pub const WHITE: Color = Color::rgb(255, 255, 255);
  pub const RED: Color = Color::hex(0xF44336);
  pub const GREEN: Color = Color::hex(0x4CAF50);
  pub const BLUE: Color = Color::hex(0x2196F3);
  pub const YELLOW: Color = Color::hex(0xFFEB3B);
  pub const ORANGE: Color = Color::hex(0xFF9800);
  pub const PURPLE: Color = Color::hex(0x9C27B0);
  pub const GREY: Color = Color::hex(0x9E9E9E);

  pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
    Color { r, g, b, a: 255 }
  }

  pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
    Color { r, g, b, a }
  }

  /// Opaque color from `0xRRGGBB`.
  pub const fn hex(rgb: u32) -> Self {
    Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
  }

  /// Same color with alpha set to `opacity` (0.0..=1.0).
  pub fn with_opacity(self, opacity: f32) -> Self {
    Color { a: (opacity.clamp(0.0, 1.0) * 255.0).round() as u8, ..self }
  }

  /// This color composited over `below` (source-over).
  pub fn over(self, below: Color) -> Color {
    if self.a == 255 || below.a == 0 {
      return self;
    }
    if self.a == 0 {
      return below;
    }
    let sa = self.a as f32 / 255.0;
    let da = below.a as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    let mix = |s: u8, d: u8| ((s as f32 * sa + d as f32 * da * (1.0 - sa)) / out_a).round() as u8;
    Color::rgba(
      mix(self.r, below.r),
      mix(self.g, below.g),
      mix(self.b, below.b),
      (out_a * 255.0).round() as u8,
    )
  }
}

/// Fully resolved text style, as a backend sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct TextPaint {
  pub color: Color,
  /// Font size in logical pixels. A terminal ignores it (one cell per char).
  pub size: f32,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
}

impl Default for TextPaint {
  fn default() -> Self {
    TextPaint { color: Color::BLACK, size: 14.0, bold: false, italic: false, underline: false }
  }
}

/// A Material icon. `name` is the Material Symbols name (for a font-based backend);
/// `glyph` is a single-width Unicode stand-in (for the terminal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconData {
  pub name: &'static str,
  pub glyph: char,
}

/// An outline: `Border { color: Color::RED, width: 1.0 }`.
#[derive(Widget, Clone, Copy, Debug, PartialEq)]
pub struct Border {
  pub color: Color,
  pub width: f32,
}

/// Background, outline and shadow of a box (card, button, sheet, dialog...).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxPaint {
  pub color: Color,
  pub border: Option<Border>,
  /// Corner radius.
  pub radius: f32,
  /// Material elevation in dp; 0 draws no shadow.
  pub elevation: f32,
}

impl Default for BoxPaint {
  fn default() -> Self {
    BoxPaint { color: Color::TRANSPARENT, border: None, radius: 0.0, elevation: 0.0 }
  }
}

/// Colors of a two-state control (checkbox, radio, switch).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToggleColors {
  /// Fill when on / selected.
  pub active: Color,
  /// Mark drawn on top of `active` (check, switch thumb).
  pub on_active: Color,
  /// Outline / track when off.
  pub inactive: Color,
  /// Track fill when off (switch only).
  pub on_inactive: Color,
}

/// Colors of a track with progress (slider, progress indicators).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackColors {
  pub active: Color,
  pub inactive: Color,
  pub thumb: Color,
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// The one interface through which all text and widgets are drawn.
///
/// Coordinates are logical pixels from the top-left of the canvas.
pub trait Renderer {
  // ---- primitives: every backend implements these ----

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
  /// Pixel backends return it unchanged; widgets that place text on an outline use it
  /// so both land on the same cells.
  fn snap_rect(&self, rect: Rect) -> Rect {
    rect
  }

  // ---- widget-level: default Material drawing, backends may override ----

  /// Soft shadow under a box of the given elevation.
  fn draw_shadow(&mut self, rect: Rect, radius: f32, elevation: f32) {
    if elevation <= 0.0 {
      return;
    }
    let spread = elevation.min(8.0);
    let shadow =
      Rect::new(rect.x, rect.y + spread / 2.0, rect.width, rect.height).inflate(spread / 2.0);
    self.fill_rect(shadow, Color::BLACK.with_opacity(0.12), radius + spread / 2.0);
  }

  /// A Material surface: shadow, fill, then outline.
  fn draw_box(&mut self, rect: Rect, paint: &BoxPaint) {
    self.draw_shadow(rect, paint.radius, paint.elevation);
    if paint.color.a > 0 {
      self.fill_rect(rect, paint.color, paint.radius);
    }
    if let Some(border) = paint.border {
      self.stroke_rect(rect, border, paint.radius);
    }
  }

  /// Draws `icon` centered in `rect`. The default uses the icon's glyph as text;
  /// an icon-font backend would look it up by `icon.name`.
  fn draw_icon(&mut self, rect: Rect, icon: IconData, color: Color) {
    let paint = TextPaint { color, size: rect.height.min(rect.width), ..TextPaint::default() };
    let mut buf = [0u8; 4];
    let glyph = icon.glyph.encode_utf8(&mut buf);
    let size = self.measure_text(glyph, &paint);
    self.draw_text(rect.center_rect(size).origin(), glyph, &paint);
  }

  /// Checkbox centered in `rect` (an 18 px box).
  fn draw_checkbox(&mut self, rect: Rect, checked: bool, colors: &ToggleColors) {
    let b = rect.center_rect(Size::new(18.0, 18.0));
    if checked {
      self.fill_rect(b, colors.active, 2.0);
      let c = b.center();
      self.draw_line(
        Offset::new(b.x + 4.0, c.y),
        Offset::new(c.x - 1.0, b.bottom() - 5.0),
        colors.on_active,
        2.0,
      );
      self.draw_line(
        Offset::new(c.x - 1.0, b.bottom() - 5.0),
        Offset::new(b.right() - 4.0, b.y + 5.0),
        colors.on_active,
        2.0,
      );
    } else {
      self.stroke_rect(b, Border { color: colors.inactive, width: 2.0 }, 2.0);
    }
  }

  /// Radio button centered in `rect` (a 20 px ring).
  fn draw_radio(&mut self, rect: Rect, selected: bool, colors: &ToggleColors) {
    let c = rect.center();
    let color = if selected { colors.active } else { colors.inactive };
    self.stroke_circle(c, 10.0, Border { color, width: 2.0 });
    if selected {
      self.fill_circle(c, 5.0, colors.active);
    }
  }

  /// Switch centered in `rect` (52x32 track).
  fn draw_switch(&mut self, rect: Rect, on: bool, colors: &ToggleColors) {
    let track = rect.center_rect(Size::new(52.0, 32.0));
    if on {
      self.fill_rect(track, colors.active, 16.0);
      self.fill_circle(Offset::new(track.right() - 16.0, track.center().y), 12.0, colors.on_active);
    } else {
      self.fill_rect(track, colors.on_inactive, 16.0);
      self.stroke_rect(track, Border { color: colors.inactive, width: 2.0 }, 16.0);
      self.fill_circle(Offset::new(track.x + 16.0, track.center().y), 8.0, colors.inactive);
    }
  }

  /// Horizontal slider across `rect`; `fraction` is 0.0..=1.0.
  fn draw_slider(&mut self, rect: Rect, fraction: f32, colors: &TrackColors) {
    let y = rect.center().y;
    let x = rect.x + rect.width * fraction.clamp(0.0, 1.0);
    self.fill_rect(Rect::new(rect.x, y - 2.0, x - rect.x, 4.0), colors.active, 2.0);
    self.fill_rect(Rect::new(x, y - 2.0, rect.right() - x, 4.0), colors.inactive, 2.0);
    self.fill_circle(Offset::new(x, y), 10.0, colors.thumb);
  }

  /// Linear progress bar across `rect`. `None` is indeterminate.
  fn draw_linear_progress(&mut self, rect: Rect, value: Option<f32>, colors: &TrackColors) {
    self.fill_rect(rect, colors.inactive, rect.height / 2.0);
    let (start, end) = match value {
      Some(v) => (0.0, v.clamp(0.0, 1.0)),
      // A still frame of the moving segment.
      None => (0.2, 0.6),
    };
    let bar =
      Rect::new(rect.x + rect.width * start, rect.y, rect.width * (end - start), rect.height);
    self.fill_rect(bar, colors.active, rect.height / 2.0);
  }

  /// Circular progress ring in `rect`. `None` is indeterminate.
  fn draw_circular_progress(&mut self, rect: Rect, value: Option<f32>, colors: &TrackColors) {
    let c = rect.center();
    let r = rect.width.min(rect.height) / 2.0 - 2.0;
    self.stroke_circle(c, r, Border { color: colors.inactive, width: 4.0 });
    let sweep = value.unwrap_or(0.25).clamp(0.0, 1.0);
    // Arc as short segments, clockwise from 12 o'clock.
    let steps = (sweep * 48.0).ceil() as usize;
    let point = |t: f32| {
      let a = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
      Offset::new(c.x + r * a.cos(), c.y + r * a.sin())
    };
    for i in 0..steps {
      let t0 = sweep * i as f32 / steps as f32;
      let t1 = sweep * (i + 1) as f32 / steps as f32;
      self.draw_line(point(t0), point(t1), colors.active, 4.0);
    }
  }

  /// Keyboard focus indicator around `rect`.
  fn draw_focus(&mut self, rect: Rect, color: Color, radius: f32) {
    self.stroke_rect(rect.inflate(2.0), Border { color, width: 2.0 }, radius + 2.0);
  }

  /// Text-input caret with its top at `at`.
  fn draw_caret(&mut self, at: Offset, height: f32, color: Color) {
    self.draw_line(at, Offset::new(at.x, at.y + height), color, 2.0);
  }
}
