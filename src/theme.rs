//! Material 3 theme: color scheme, type scale and inherited text style.
//!
//! The color values are the Material 3 baseline (purple) scheme. They were written
//! from memory of the M3 tokens and should be checked against m3.material.io.

use std::rc::Rc;

use crate::render::{Color, TextPaint};
use crate::{Context, Element, Widget};

// ---------------------------------------------------------------------------
// Text style
// ---------------------------------------------------------------------------

/// Partial text style: unset fields inherit from the enclosing style.
///
/// ```ignore
/// Text { text: "Title", style: TextStyle { bold: true, color: Color::RED } }
/// ```
#[derive(Widget, Clone, Debug, Default, PartialEq)]
#[widget(optional)]
pub struct TextStyle {
  pub color: Option<Color>,
  /// Font size in logical pixels; 0 inherits.
  pub size: f32,
  pub bold: Option<bool>,
  pub italic: Option<bool>,
  pub underline: Option<bool>,
}

impl TextStyle {
  /// This style on top of `parent`: fields set here win.
  pub fn merge(&self, parent: &TextStyle) -> TextStyle {
    TextStyle {
      color: self.color.or(parent.color),
      size: if self.size > 0.0 { self.size } else { parent.size },
      bold: self.bold.or(parent.bold),
      italic: self.italic.or(parent.italic),
      underline: self.underline.or(parent.underline),
    }
  }

  pub fn with_color(mut self, color: Color) -> TextStyle {
    self.color = Some(color);
    self
  }

  /// Fills unset fields from the theme defaults.
  pub fn resolve(&self, theme: &ThemeData) -> TextPaint {
    TextPaint {
      color: self.color.unwrap_or(theme.color_scheme.on_surface),
      size: if self.size > 0.0 { self.size } else { 14.0 },
      bold: self.bold.unwrap_or(false),
      italic: self.italic.unwrap_or(false),
      underline: self.underline.unwrap_or(false),
    }
  }
}

const fn style(size: f32, bold: bool) -> TextStyle {
  TextStyle { color: None, size, bold: Some(bold), italic: None, underline: None }
}

/// The Material 3 type scale. `bold` stands for weight 500.
#[derive(Clone, Debug, PartialEq)]
pub struct TextTheme {
  pub display_large: TextStyle,
  pub display_medium: TextStyle,
  pub display_small: TextStyle,
  pub headline_large: TextStyle,
  pub headline_medium: TextStyle,
  pub headline_small: TextStyle,
  pub title_large: TextStyle,
  pub title_medium: TextStyle,
  pub title_small: TextStyle,
  pub body_large: TextStyle,
  pub body_medium: TextStyle,
  pub body_small: TextStyle,
  pub label_large: TextStyle,
  pub label_medium: TextStyle,
  pub label_small: TextStyle,
}

impl Default for TextTheme {
  fn default() -> Self {
    TextTheme {
      display_large: style(57.0, false),
      display_medium: style(45.0, false),
      display_small: style(36.0, false),
      headline_large: style(32.0, false),
      headline_medium: style(28.0, false),
      headline_small: style(24.0, false),
      title_large: style(22.0, false),
      title_medium: style(16.0, true),
      title_small: style(14.0, true),
      body_large: style(16.0, false),
      body_medium: style(14.0, false),
      body_small: style(12.0, false),
      label_large: style(14.0, true),
      label_medium: style(12.0, true),
      label_small: style(11.0, true),
    }
  }
}

// ---------------------------------------------------------------------------
// Color scheme
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorScheme {
  pub primary: Color,
  pub on_primary: Color,
  pub primary_container: Color,
  pub on_primary_container: Color,
  pub secondary: Color,
  pub on_secondary: Color,
  pub secondary_container: Color,
  pub on_secondary_container: Color,
  pub tertiary: Color,
  pub on_tertiary: Color,
  pub tertiary_container: Color,
  pub on_tertiary_container: Color,
  pub error: Color,
  pub on_error: Color,
  pub error_container: Color,
  pub on_error_container: Color,
  pub surface: Color,
  pub on_surface: Color,
  pub on_surface_variant: Color,
  pub surface_container_lowest: Color,
  pub surface_container_low: Color,
  pub surface_container: Color,
  pub surface_container_high: Color,
  pub surface_container_highest: Color,
  pub outline: Color,
  pub outline_variant: Color,
  pub inverse_surface: Color,
  pub on_inverse_surface: Color,
  pub inverse_primary: Color,
  pub scrim: Color,
}

impl ColorScheme {
  pub const fn light() -> Self {
    ColorScheme {
      primary: Color::hex(0x6750A4),
      on_primary: Color::hex(0xFFFFFF),
      primary_container: Color::hex(0xEADDFF),
      on_primary_container: Color::hex(0x21005D),
      secondary: Color::hex(0x625B71),
      on_secondary: Color::hex(0xFFFFFF),
      secondary_container: Color::hex(0xE8DEF8),
      on_secondary_container: Color::hex(0x1D192B),
      tertiary: Color::hex(0x7D5260),
      on_tertiary: Color::hex(0xFFFFFF),
      tertiary_container: Color::hex(0xFFD8E4),
      on_tertiary_container: Color::hex(0x31111D),
      error: Color::hex(0xB3261E),
      on_error: Color::hex(0xFFFFFF),
      error_container: Color::hex(0xF9DEDC),
      on_error_container: Color::hex(0x410E0B),
      surface: Color::hex(0xFEF7FF),
      on_surface: Color::hex(0x1D1B20),
      on_surface_variant: Color::hex(0x49454F),
      surface_container_lowest: Color::hex(0xFFFFFF),
      surface_container_low: Color::hex(0xF7F2FA),
      surface_container: Color::hex(0xF3EDF7),
      surface_container_high: Color::hex(0xECE6F0),
      surface_container_highest: Color::hex(0xE6E0E9),
      outline: Color::hex(0x79747E),
      outline_variant: Color::hex(0xCAC4D0),
      inverse_surface: Color::hex(0x322F35),
      on_inverse_surface: Color::hex(0xF5EFF7),
      inverse_primary: Color::hex(0xD0BCFF),
      scrim: Color::hex(0x000000),
    }
  }

  pub const fn dark() -> Self {
    ColorScheme {
      primary: Color::hex(0xD0BCFF),
      on_primary: Color::hex(0x381E72),
      primary_container: Color::hex(0x4F378B),
      on_primary_container: Color::hex(0xEADDFF),
      secondary: Color::hex(0xCCC2DC),
      on_secondary: Color::hex(0x332D41),
      secondary_container: Color::hex(0x4A4458),
      on_secondary_container: Color::hex(0xE8DEF8),
      tertiary: Color::hex(0xEFB8C8),
      on_tertiary: Color::hex(0x492532),
      tertiary_container: Color::hex(0x633B48),
      on_tertiary_container: Color::hex(0xFFD8E4),
      error: Color::hex(0xF2B8B5),
      on_error: Color::hex(0x601410),
      error_container: Color::hex(0x8C1D18),
      on_error_container: Color::hex(0xF9DEDC),
      surface: Color::hex(0x141218),
      on_surface: Color::hex(0xE6E0E9),
      on_surface_variant: Color::hex(0xCAC4D0),
      surface_container_lowest: Color::hex(0x0F0D13),
      surface_container_low: Color::hex(0x1D1B20),
      surface_container: Color::hex(0x211F26),
      surface_container_high: Color::hex(0x2B2930),
      surface_container_highest: Color::hex(0x36343B),
      outline: Color::hex(0x938F99),
      outline_variant: Color::hex(0x49454F),
      inverse_surface: Color::hex(0xE6E0E9),
      on_inverse_surface: Color::hex(0x322F35),
      inverse_primary: Color::hex(0x6750A4),
      scrim: Color::hex(0x000000),
    }
  }
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeData {
  pub color_scheme: ColorScheme,
  pub text_theme: TextTheme,
}

impl Default for ThemeData {
  fn default() -> Self {
    ThemeData::light()
  }
}

impl ThemeData {
  pub fn light() -> Self {
    ThemeData { color_scheme: ColorScheme::light(), text_theme: TextTheme::default() }
  }

  pub fn dark() -> Self {
    ThemeData { color_scheme: ColorScheme::dark(), text_theme: TextTheme::default() }
  }

  /// The theme provided by the nearest `Theme` above, or the light theme.
  pub fn of(context: &Context) -> Rc<ThemeData> {
    thread_local! {
      static DEFAULT: Rc<ThemeData> = Rc::new(ThemeData::light());
    }
    context.try_read::<ThemeData>().unwrap_or_else(|| DEFAULT.with(Rc::clone))
  }
}

/// Provides `data` to its subtree (read with `ThemeData::of(context)`).
#[derive(Widget)]
#[non_exhaustive]
pub struct Theme {
  pub data: ThemeData,
  pub child: Widget,
}

impl Element for Theme {
  fn build(&self, _context: &Context) -> Option<Widget> {
    Some(self.child.clone())
  }

  fn scope(&self, cx: &crate::NodeCx) -> Option<Context> {
    let mut values = anymap3::AnyMap::new();
    values.insert(Rc::new(self.data.clone()));
    Some(cx.context().with_scope(values))
  }
}
