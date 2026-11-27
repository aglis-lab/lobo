//! Theme: shadcn/ui (Neutral/Zinc) color scheme, type scale, and inherited text style.

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
  /// shadcn/ui default Neutral/Zinc light theme.
  pub const fn light() -> Self {
    ColorScheme {
      primary: Color::hex(0x18181B),
      on_primary: Color::hex(0xFAFAFA),
      primary_container: Color::hex(0xF4F4F5),
      on_primary_container: Color::hex(0x18181B),
      secondary: Color::hex(0x71717A),
      on_secondary: Color::hex(0xFAFAFA),
      secondary_container: Color::hex(0xF4F4F5),
      on_secondary_container: Color::hex(0x18181B),
      tertiary: Color::hex(0x52525B),
      on_tertiary: Color::hex(0xFAFAFA),
      tertiary_container: Color::hex(0xE4E4E7),
      on_tertiary_container: Color::hex(0x18181B),
      error: Color::hex(0xEF4444),
      on_error: Color::hex(0xFAFAFA),
      error_container: Color::hex(0xFEE2E2),
      on_error_container: Color::hex(0x991B1B),
      surface: Color::hex(0xFFFFFF),
      on_surface: Color::hex(0x09090B),
      on_surface_variant: Color::hex(0x71717A),
      surface_container_lowest: Color::hex(0xFFFFFF),
      surface_container_low: Color::hex(0xFAFAFA),
      surface_container: Color::hex(0xF4F4F5),
      surface_container_high: Color::hex(0xF4F4F5),
      surface_container_highest: Color::hex(0xE4E4E7),
      outline: Color::hex(0xD4D4D8),
      outline_variant: Color::hex(0xE4E4E7),
      inverse_surface: Color::hex(0x18181B),
      on_inverse_surface: Color::hex(0xFAFAFA),
      inverse_primary: Color::hex(0xFAFAFA),
      scrim: Color::hex(0x000000),
    }
  }

  /// shadcn/ui default Neutral/Zinc dark theme.
  pub const fn dark() -> Self {
    ColorScheme {
      primary: Color::hex(0xFAFAFA),
      on_primary: Color::hex(0x18181B),
      primary_container: Color::hex(0x27272A),
      on_primary_container: Color::hex(0xFAFAFA),
      secondary: Color::hex(0xA1A1AA),
      on_secondary: Color::hex(0x09090B),
      secondary_container: Color::hex(0x27272A),
      on_secondary_container: Color::hex(0xFAFAFA),
      tertiary: Color::hex(0xA1A1AA),
      on_tertiary: Color::hex(0x09090B),
      tertiary_container: Color::hex(0x3F3F46),
      on_tertiary_container: Color::hex(0xFAFAFA),
      error: Color::hex(0xDC2626),
      on_error: Color::hex(0xFAFAFA),
      error_container: Color::hex(0x450A0A),
      on_error_container: Color::hex(0xFCA5A5),
      surface: Color::hex(0x09090B),
      on_surface: Color::hex(0xFAFAFA),
      on_surface_variant: Color::hex(0xA1A1AA),
      surface_container_lowest: Color::hex(0x09090B),
      surface_container_low: Color::hex(0x111113),
      surface_container: Color::hex(0x18181B),
      surface_container_high: Color::hex(0x27272A),
      surface_container_highest: Color::hex(0x27272A),
      outline: Color::hex(0x3F3F46),
      outline_variant: Color::hex(0x27272A),
      inverse_surface: Color::hex(0xFAFAFA),
      on_inverse_surface: Color::hex(0x09090B),
      inverse_primary: Color::hex(0x18181B),
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
