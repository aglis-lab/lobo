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
  let spread = (elevation * 1.5).min(6.0);
  let shadow =
    Rect::new(rect.x, rect.y + spread * 0.5, rect.width, rect.height).inflate(spread * 0.25);
  r.fill_rect(shadow, Color::BLACK.with_opacity(0.06), radius + spread * 0.25);
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

/// Checkbox centered in `rect` (shadcn/ui 16 px rounded-sm box).
pub fn draw_checkbox(r: &mut dyn Renderer, rect: Rect, checked: bool, colors: &ToggleColors) {
  let b = rect.center_rect(Size::new(16.0, 16.0));
  if checked {
    r.fill_rect(b, colors.active, 4.0);
    let c = b.center();
    r.draw_line(
      Offset::new(b.x + 3.5, c.y + 0.5),
      Offset::new(c.x - 1.0, b.bottom() - 4.0),
      colors.on_active,
      1.75,
    );
    r.draw_line(
      Offset::new(c.x - 1.0, b.bottom() - 4.0),
      Offset::new(b.right() - 3.5, b.y + 4.5),
      colors.on_active,
      1.75,
    );
  } else {
    r.stroke_rect(b, Border { color: colors.active, width: 1.5 }, 4.0);
  }
}

/// Radio button centered in `rect` (shadcn/ui 16 px ring).
pub fn draw_radio(r: &mut dyn Renderer, rect: Rect, selected: bool, colors: &ToggleColors) {
  let c = rect.center();
  r.stroke_circle(c, 8.0, Border { color: colors.active, width: 1.5 });
  if selected {
    r.fill_circle(c, 4.0, colors.active);
  }
}

/// Switch centered in `rect` (44x24 shadcn/ui track).
pub fn draw_switch(r: &mut dyn Renderer, rect: Rect, on: bool, colors: &ToggleColors) {
  draw_switch_at(r, rect, if on { 1.0 } else { 0.0 }, colors);
}

/// Switch centered in `rect` at animated progress `t` (`0.0` = off, `1.0` = on).
pub fn draw_switch_at(r: &mut dyn Renderer, rect: Rect, t: f32, colors: &ToggleColors) {
  let t = t.clamp(0.0, 1.0);
  let track = rect.center_rect(Size::new(44.0, 24.0));
  let track_color = crate::anim::lerp_color(colors.on_inactive, colors.active, t);
  r.fill_rect(track, track_color, 12.0);
  let thumb_x = crate::anim::lerp(track.x + 12.0, track.right() - 12.0, t);
  r.fill_circle(Offset::new(thumb_x, track.center().y), 9.5, colors.on_active);
}

/// Horizontal slider across `rect`; `fraction` is 0.0..=1.0 (shadcn/ui style).
pub fn draw_slider(r: &mut dyn Renderer, rect: Rect, fraction: f32, colors: &TrackColors) {
  let y = rect.center().y;
  let x = rect.x + rect.width * fraction.clamp(0.0, 1.0);
  r.fill_rect(Rect::new(rect.x, y - 3.0, rect.width, 6.0), colors.inactive, 3.0);
  r.fill_rect(Rect::new(rect.x, y - 3.0, (x - rect.x).max(0.0), 6.0), colors.active, 3.0);
  r.fill_circle(Offset::new(x, y), 8.5, Color::WHITE);
  r.stroke_circle(Offset::new(x, y), 8.5, Border { color: colors.thumb, width: 2.0 });
}

/// Linear progress bar across `rect`. `None` is indeterminate.
pub fn draw_linear_progress(
  r: &mut dyn Renderer,
  rect: Rect,
  value: Option<f32>,
  colors: &TrackColors,
) {
  draw_linear_progress_at(r, rect, value, 0.2, colors);
}

/// Linear progress bar with explicit indeterminate `phase` (`0.0..1.0`).
pub fn draw_linear_progress_at(
  r: &mut dyn Renderer,
  rect: Rect,
  value: Option<f32>,
  phase: f32,
  colors: &TrackColors,
) {
  r.fill_rect(rect, colors.inactive, rect.height / 2.0);
  let (start, end) = match value {
    Some(v) => (0.0, v.clamp(0.0, 1.0)),
    None => {
      let head = (phase * 1.4).min(1.0);
      let tail = ((phase - 0.35) * 1.54).clamp(0.0, 1.0);
      (tail, head.max(tail))
    }
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
  draw_circular_progress_at(r, rect, value, 0.0, colors);
}

/// Circular progress ring with explicit indeterminate `phase` (`0.0..1.0`).
pub fn draw_circular_progress_at(
  r: &mut dyn Renderer,
  rect: Rect,
  value: Option<f32>,
  phase: f32,
  colors: &TrackColors,
) {
  let c = rect.center();
  let radius = rect.width.min(rect.height) / 2.0 - 2.0;
  r.stroke_circle(c, radius, Border { color: colors.inactive, width: 4.0 });
  let (start_turn, sweep) = match value {
    Some(v) => (0.0, v.clamp(0.0, 1.0)),
    None => (phase, 0.28 + 0.14 * (phase * std::f32::consts::TAU).sin()),
  };
  // Arc as short segments, clockwise from 12 o'clock.
  let steps = (sweep * 48.0).ceil().max(1.0) as usize;
  let point = |t: f32| {
    let a = (start_turn + t) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    Offset::new(c.x + radius * a.cos(), c.y + radius * a.sin())
  };
  for i in 0..steps {
    let t0 = sweep * i as f32 / steps as f32;
    let t1 = sweep * (i + 1) as f32 / steps as f32;
    r.draw_line(point(t0), point(t1), colors.active, 4.0);
  }
}

/// Keyboard focus indicator around `rect` (shadcn/ui soft ring).
pub fn draw_focus(r: &mut dyn Renderer, rect: Rect, color: Color, radius: f32) {
  r.stroke_rect(
    rect.inflate(2.0),
    Border { color: color.with_opacity(0.55), width: 1.5 },
    radius + 2.0,
  );
}

/// Text-input caret with its top at `at`.
pub fn draw_caret(r: &mut dyn Renderer, at: Offset, height: f32, color: Color) {
  r.draw_line(at, Offset::new(at.x, at.y + height), color, 2.0);
}
