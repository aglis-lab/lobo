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
  /// Font size in logical pixels.
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
/// `glyph` is a single-width Unicode stand-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconData {
  pub name: &'static str,
  pub glyph: char,
}

/// An outline: `Border { color: Color::RED, width: 1.0 }`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
  pub color: Color,
  pub width: f32,
}

impl Border {
  pub const fn new(color: Color, width: f32) -> Self {
    Border { color, width }
  }
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
