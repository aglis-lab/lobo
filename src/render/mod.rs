//! Drawing interface re-exported from [`renderer`], plus Material widget drawing helpers.
//!
//! Backends implement [`Renderer`] primitives; widgets paint through `Recorder` into a
//! retained [`Scene`] using these free functions for Material controls.

pub use renderer::*;

/// Soft shadow under a box of the given elevation.
pub fn draw_shadow(r: &mut dyn Renderer, rect: Rect, radius: f32, elevation: f32) {
  if elevation <= 0.0 {
    return;
  }
  let spread = elevation.min(8.0);
  let shadow =
    Rect::new(rect.x, rect.y + spread / 2.0, rect.width, rect.height).inflate(spread / 2.0);
  r.fill_rect(shadow, Color::BLACK.with_opacity(0.12), radius + spread / 2.0);
}

/// A Material surface: shadow, fill, then outline.
pub fn draw_box(r: &mut dyn Renderer, rect: Rect, paint: &BoxPaint) {
  draw_shadow(r, rect, paint.radius, paint.elevation);
  if paint.color.a > 0 {
    r.fill_rect(rect, paint.color, paint.radius);
  }
  if let Some(border) = paint.border {
    r.stroke_rect(rect, border, paint.radius);
  }
}

/// Draws `icon` centered in `rect`.
pub fn draw_icon(r: &mut dyn Renderer, rect: Rect, icon: IconData, color: Color) {
  let paint = TextPaint { color, size: rect.height.min(rect.width), ..TextPaint::default() };
  let mut buf = [0u8; 4];
  let glyph = icon.glyph.encode_utf8(&mut buf);
  let size = r.measure_text(glyph, &paint);
  r.draw_text(rect.center_rect(size).origin(), glyph, &paint);
}

/// Checkbox centered in `rect` (an 18 px box).
pub fn draw_checkbox(r: &mut dyn Renderer, rect: Rect, checked: bool, colors: &ToggleColors) {
  let b = rect.center_rect(Size::new(18.0, 18.0));
  if checked {
    r.fill_rect(b, colors.active, 2.0);
    let c = b.center();
    r.draw_line(
      Offset::new(b.x + 4.0, c.y),
      Offset::new(c.x - 1.0, b.bottom() - 5.0),
      colors.on_active,
      2.0,
    );
    r.draw_line(
      Offset::new(c.x - 1.0, b.bottom() - 5.0),
      Offset::new(b.right() - 4.0, b.y + 5.0),
      colors.on_active,
      2.0,
    );
  } else {
    r.stroke_rect(b, Border { color: colors.inactive, width: 2.0 }, 2.0);
  }
}

/// Radio button centered in `rect` (a 20 px ring).
pub fn draw_radio(r: &mut dyn Renderer, rect: Rect, selected: bool, colors: &ToggleColors) {
  let c = rect.center();
  let color = if selected { colors.active } else { colors.inactive };
  r.stroke_circle(c, 10.0, Border { color, width: 2.0 });
  if selected {
    r.fill_circle(c, 5.0, colors.active);
  }
}

/// Switch centered in `rect` (52x32 track).
pub fn draw_switch(r: &mut dyn Renderer, rect: Rect, on: bool, colors: &ToggleColors) {
  let track = rect.center_rect(Size::new(52.0, 32.0));
  if on {
    r.fill_rect(track, colors.active, 16.0);
    r.fill_circle(Offset::new(track.right() - 16.0, track.center().y), 12.0, colors.on_active);
  } else {
    r.fill_rect(track, colors.on_inactive, 16.0);
    r.stroke_rect(track, Border { color: colors.inactive, width: 2.0 }, 16.0);
    r.fill_circle(Offset::new(track.x + 16.0, track.center().y), 8.0, colors.inactive);
  }
}

/// Horizontal slider across `rect`; `fraction` is 0.0..=1.0.
pub fn draw_slider(r: &mut dyn Renderer, rect: Rect, fraction: f32, colors: &TrackColors) {
  let y = rect.center().y;
  let x = rect.x + rect.width * fraction.clamp(0.0, 1.0);
  r.fill_rect(Rect::new(rect.x, y - 2.0, x - rect.x, 4.0), colors.active, 2.0);
  r.fill_rect(Rect::new(x, y - 2.0, rect.right() - x, 4.0), colors.inactive, 2.0);
  r.fill_circle(Offset::new(x, y), 10.0, colors.thumb);
}

/// Linear progress bar across `rect`. `None` is indeterminate.
pub fn draw_linear_progress(
  r: &mut dyn Renderer,
  rect: Rect,
  value: Option<f32>,
  colors: &TrackColors,
) {
  r.fill_rect(rect, colors.inactive, rect.height / 2.0);
  let (start, end) = match value {
    Some(v) => (0.0, v.clamp(0.0, 1.0)),
    // A still frame of the moving segment.
    None => (0.2, 0.6),
  };
  let bar = Rect::new(rect.x + rect.width * start, rect.y, rect.width * (end - start), rect.height);
  r.fill_rect(bar, colors.active, rect.height / 2.0);
}

/// Circular progress ring in `rect`. `None` is indeterminate.
pub fn draw_circular_progress(
  r: &mut dyn Renderer,
  rect: Rect,
  value: Option<f32>,
  colors: &TrackColors,
) {
  let c = rect.center();
  let radius = rect.width.min(rect.height) / 2.0 - 2.0;
  r.stroke_circle(c, radius, Border { color: colors.inactive, width: 4.0 });
  let sweep = value.unwrap_or(0.25).clamp(0.0, 1.0);
  // Arc as short segments, clockwise from 12 o'clock.
  let steps = (sweep * 48.0).ceil() as usize;
  let point = |t: f32| {
    let a = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    Offset::new(c.x + radius * a.cos(), c.y + radius * a.sin())
  };
  for i in 0..steps {
    let t0 = sweep * i as f32 / steps as f32;
    let t1 = sweep * (i + 1) as f32 / steps as f32;
    r.draw_line(point(t0), point(t1), colors.active, 4.0);
  }
}

/// Keyboard focus indicator around `rect`.
pub fn draw_focus(r: &mut dyn Renderer, rect: Rect, color: Color, radius: f32) {
  r.stroke_rect(rect.inflate(2.0), Border { color, width: 2.0 }, radius + 2.0);
}

/// Text-input caret with its top at `at`.
pub fn draw_caret(r: &mut dyn Renderer, at: Offset, height: f32, color: Color) {
  r.draw_line(at, Offset::new(at.x, at.y + height), color, 2.0);
}
