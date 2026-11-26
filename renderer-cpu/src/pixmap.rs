use renderer::{Border, Color, DrawCmd, Offset, Rect, Renderer, Size, TextPaint};
use vello_cpu::color::{AlphaColor, PremulRgba8, Srgb};
use vello_cpu::kurbo::{Affine, BezPath, Cap, Circle, Join, RoundedRect, Shape, Stroke};
use vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources};

use crate::font;

/// Software RGBA pixmap renderer backed by `vello_cpu`.
///
/// The canvas keeps its pixels between frames; [`Renderer::draw_batch`] rasterizes
/// only the damaged rect and blits it onto the persistent canvas.
pub struct PixmapRenderer {
  width: u16,
  height: u16,
  logical_width: f32,
  logical_height: f32,
  scale: f32,
  canvas: Pixmap,
  resources: Resources,
  clips: Vec<Rect>,
}

impl PixmapRenderer {
  /// Creates a blank 1x-scale pixmap renderer of `width` x `height` pixels.
  pub fn new(width: u16, height: u16) -> Self {
    Self::with_scale(f32::from(width), f32::from(height), 1.0)
  }

  /// Creates a blank pixmap renderer for logical dimensions at `scale` (e.g. 2.0 on Retina).
  pub fn with_scale(logical_width: f32, logical_height: f32, scale: f32) -> Self {
    let scale = scale.max(0.25);
    let (width, height) = physical_size(logical_width, logical_height, scale);
    PixmapRenderer {
      width,
      height,
      logical_width,
      logical_height,
      scale,
      canvas: Pixmap::new(width, height),
      resources: Resources::new(),
      clips: Vec::new(),
    }
  }

  /// Creates a 1280x720 (`HD_720`) pixmap renderer.
  pub fn hd720() -> Self {
    Self::new(1280, 720)
  }

  /// Resizes the backing pixmap at 1x scale (contents are cleared if size changed).
  pub fn resize(&mut self, width: u16, height: u16) {
    let _ = self.resize_with_scale(f32::from(width), f32::from(height), 1.0);
  }

  /// Resizes the backing pixmap for logical dimensions and HiDPI `scale`.
  /// Returns `true` if dimensions or scale changed (which clears the canvas).
  pub fn resize_with_scale(&mut self, logical_width: f32, logical_height: f32, scale: f32) -> bool {
    let scale = scale.max(0.25);
    let (width, height) = physical_size(logical_width, logical_height, scale);
    if width == self.width
      && height == self.height
      && (logical_width - self.logical_width).abs() < f32::EPSILON
      && (logical_height - self.logical_height).abs() < f32::EPSILON
      && (scale - self.scale).abs() < f32::EPSILON
    {
      return false;
    }
    self.width = width;
    self.height = height;
    self.logical_width = logical_width;
    self.logical_height = logical_height;
    self.scale = scale;
    self.canvas = Pixmap::new(width, height);
    self.clips.clear();
    true
  }

  pub fn width(&self) -> u16 {
    self.width
  }

  pub fn height(&self) -> u16 {
    self.height
  }

  pub fn scale(&self) -> f32 {
    self.scale
  }

  /// Underlying `vello_cpu::Pixmap` (premultiplied RGBA8).
  pub fn pixmap(&self) -> &Pixmap {
    &self.canvas
  }

  /// Premultiplied `[r, g, b, a]` bytes of the physical pixel at `(x, y)`.
  pub fn pixel(&self, x: u16, y: u16) -> [u8; 4] {
    let p = self.canvas.sample(x, y);
    [p.r, p.g, p.b, p.a]
  }

  /// Encodes the current canvas as PNG bytes.
  pub fn to_png(&self) -> Result<Vec<u8>, String> {
    self.canvas.clone().into_png().map_err(|e| e.to_string())
  }

  fn apply_scale(&self, ctx: &mut RenderContext) {
    if (self.scale - 1.0).abs() > f32::EPSILON {
      ctx.set_transform(Affine::scale(f64::from(self.scale)));
    }
  }

  fn render_region(&mut self, damage: Rect, cmds: &[DrawCmd]) {
    let x0 = ((damage.x * self.scale).floor() as i32).clamp(0, i32::from(self.width)) as u16;
    let y0 = ((damage.y * self.scale).floor() as i32).clamp(0, i32::from(self.height)) as u16;
    let x1 = ((damage.right() * self.scale).ceil() as i32).clamp(0, i32::from(self.width)) as u16;
    let y1 = ((damage.bottom() * self.scale).ceil() as i32).clamp(0, i32::from(self.height)) as u16;
    if x1 <= x0 || y1 <= y0 {
      return;
    }

    let dw = x1 - x0;
    let dh = y1 - y0;
    let mut ctx = RenderContext::new(self.width, self.height);
    self.apply_scale(&mut ctx);

    let mut clip_depth = 0usize;
    for cmd in cmds {
      apply_cmd(&mut ctx, cmd, self.scale, &mut clip_depth);
    }
    while clip_depth > 0 {
      ctx.pop_clip();
      clip_depth -= 1;
    }

    ctx.flush();
    let mut scratch = Pixmap::new(dw, dh);
    let settings = RasterizerSettings {
      offset: (-i32::from(x0), -i32::from(y0)),
      ..RasterizerSettings::default()
    };
    ctx.render_with(&mut scratch, &mut self.resources, settings);

    let dst_w = usize::from(self.width);
    let src_w = usize::from(dw);
    let src = scratch.data();
    let dst = self.canvas.data_mut();
    for row in 0..usize::from(dh) {
      let src_start = row * src_w;
      let dst_start = (usize::from(y0) + row) * dst_w + usize::from(x0);
      dst[dst_start..dst_start + src_w].copy_from_slice(&src[src_start..src_start + src_w]);
    }
  }

  fn run_immediate(&mut self, cmd: DrawCmd) {
    let clips = self.clips.clone();
    let mut cmds = Vec::with_capacity(clips.len() * 2 + 1);
    for c in &clips {
      cmds.push(DrawCmd::PushClip(*c));
    }
    cmds.push(cmd);
    for _ in &clips {
      cmds.push(DrawCmd::PopClip);
    }
    let mut ctx = RenderContext::new(self.width, self.height);
    self.apply_scale(&mut ctx);
    let mut depth = 0usize;
    for c in &cmds {
      apply_cmd(&mut ctx, c, self.scale, &mut depth);
    }
    ctx.flush();
    let mut layer = Pixmap::new(self.width, self.height);
    ctx.render(&mut layer, &mut self.resources);
    for (dst, src) in self.canvas.data_mut().iter_mut().zip(layer.data()) {
      *dst = premul_over(*src, *dst);
    }
  }
}

fn physical_size(logical_width: f32, logical_height: f32, scale: f32) -> (u16, u16) {
  let w = (logical_width * scale).round().clamp(1.0, f32::from(u16::MAX)) as u16;
  let h = (logical_height * scale).round().clamp(1.0, f32::from(u16::MAX)) as u16;
  (w, h)
}

impl Renderer for PixmapRenderer {
  fn size(&self) -> Size {
    Size::new(self.logical_width, self.logical_height)
  }

  fn measure_text(&self, text: &str, paint: &TextPaint) -> Size {
    font::measure_text(text, paint)
  }

  fn clear(&mut self, color: Color) {
    let mut ctx = RenderContext::new(self.width, self.height);
    ctx.set_paint(to_paint(color));
    ctx.fill_rect(&vello_cpu::kurbo::Rect::new(
      0.0,
      0.0,
      f64::from(self.width),
      f64::from(self.height),
    ));
    ctx.flush();
    ctx.render(&mut self.canvas, &mut self.resources);
  }

  fn fill_rect(&mut self, rect: Rect, color: Color, radius: f32) {
    self.run_immediate(DrawCmd::FillRect { rect, color, radius });
  }

  fn stroke_rect(&mut self, rect: Rect, border: Border, radius: f32) {
    self.run_immediate(DrawCmd::StrokeRect { rect, border, radius });
  }

  fn draw_line(&mut self, from: Offset, to: Offset, color: Color, width: f32) {
    self.run_immediate(DrawCmd::Line { from, to, color, width });
  }

  fn fill_circle(&mut self, center: Offset, radius: f32, color: Color) {
    self.run_immediate(DrawCmd::FillCircle { center, radius, color });
  }

  fn stroke_circle(&mut self, center: Offset, radius: f32, border: Border) {
    self.run_immediate(DrawCmd::StrokeCircle { center, radius, border });
  }

  fn draw_text(&mut self, origin: Offset, text: &str, paint: &TextPaint) {
    self.run_immediate(DrawCmd::Text { origin, text: text.into(), paint: paint.clone() });
  }

  fn push_clip(&mut self, rect: Rect) {
    self.clips.push(rect);
  }

  fn pop_clip(&mut self) {
    self.clips.pop();
  }

  fn draw_batch(&mut self, damage: Rect, cmds: &[DrawCmd]) {
    self.render_region(damage, cmds);
  }
}

fn to_paint(c: Color) -> AlphaColor<Srgb> {
  AlphaColor::<Srgb>::from_rgba8(c.r, c.g, c.b, c.a)
}

fn rect_path(r: Rect, radius: f32) -> BezPath {
  let max_r = (r.width.min(r.height) * 0.5).max(0.0);
  let rad = radius.clamp(0.0, max_r);
  let krect = vello_cpu::kurbo::Rect::new(
    f64::from(r.x),
    f64::from(r.y),
    f64::from(r.right()),
    f64::from(r.bottom()),
  );
  if rad > 0.0 {
    RoundedRect::from_rect(krect, f64::from(rad)).to_path(0.1)
  } else {
    krect.to_path(0.1)
  }
}

fn apply_cmd(ctx: &mut RenderContext, cmd: &DrawCmd, scale: f32, clip_depth: &mut usize) {
  match cmd {
    DrawCmd::Clear(color) => {
      ctx.set_paint(to_paint(*color));
      ctx.fill_rect(&vello_cpu::kurbo::Rect::new(
        0.0,
        0.0,
        f64::from(ctx.width()) / f64::from(scale),
        f64::from(ctx.height()) / f64::from(scale),
      ));
    }
    DrawCmd::FillRect { rect, color, radius } => {
      if color.a == 0 || rect.is_empty() {
        return;
      }
      ctx.set_paint(to_paint(*color));
      if *radius <= 0.0 {
        ctx.fill_rect(&vello_cpu::kurbo::Rect::new(
          f64::from(rect.x),
          f64::from(rect.y),
          f64::from(rect.right()),
          f64::from(rect.bottom()),
        ));
      } else {
        ctx.fill_path(&rect_path(*rect, *radius));
      }
    }
    DrawCmd::StrokeRect { rect, border, radius } => {
      if border.color.a == 0 || border.width <= 0.0 || rect.is_empty() {
        return;
      }
      let stroke_w = border.width.min(rect.width).min(rect.height);
      let half = stroke_w * 0.5;
      let inset = rect.deflate(half, half, half, half);
      let path = rect_path(inset, (*radius - half).max(0.0));
      ctx.set_paint(to_paint(border.color));
      ctx.set_stroke(Stroke::new(f64::from(stroke_w)).with_join(Join::Round));
      ctx.stroke_path(&path);
    }
    DrawCmd::Line { from, to, color, width } => {
      if color.a == 0 || *width <= 0.0 {
        return;
      }
      let mut path = BezPath::new();
      path.move_to((f64::from(from.x), f64::from(from.y)));
      path.line_to((f64::from(to.x), f64::from(to.y)));
      ctx.set_paint(to_paint(*color));
      ctx.set_stroke(Stroke::new(f64::from(*width)).with_caps(Cap::Round).with_join(Join::Round));
      ctx.stroke_path(&path);
    }
    DrawCmd::FillCircle { center, radius, color } => {
      if color.a == 0 || *radius <= 0.0 {
        return;
      }
      let path =
        Circle::new((f64::from(center.x), f64::from(center.y)), f64::from(*radius)).to_path(0.1);
      ctx.set_paint(to_paint(*color));
      ctx.fill_path(&path);
    }
    DrawCmd::StrokeCircle { center, radius, border } => {
      if border.color.a == 0 || border.width <= 0.0 || *radius <= 0.0 {
        return;
      }
      let path =
        Circle::new((f64::from(center.x), f64::from(center.y)), f64::from(*radius)).to_path(0.1);
      ctx.set_paint(to_paint(border.color));
      ctx.set_stroke(Stroke::new(f64::from(border.width)));
      ctx.stroke_path(&path);
    }
    DrawCmd::Text { origin, text, paint } => {
      if paint.color.a == 0 || text.is_empty() {
        return;
      }
      let path = font::glyph_path(*origin, text, paint);
      ctx.set_paint(to_paint(paint.color));
      ctx.fill_path(&path);
    }
    DrawCmd::PushClip(rect) => {
      let krect = vello_cpu::kurbo::Rect::new(
        f64::from(rect.x),
        f64::from(rect.y),
        f64::from(rect.right()),
        f64::from(rect.bottom()),
      );
      ctx.push_clip_rect(&krect);
      *clip_depth += 1;
    }
    DrawCmd::PopClip => {
      if *clip_depth > 0 {
        ctx.pop_clip();
        *clip_depth -= 1;
      }
    }
  }
}

fn premul_over(src: PremulRgba8, dst: PremulRgba8) -> PremulRgba8 {
  if src.a == 255 {
    return src;
  }
  if src.a == 0 {
    return dst;
  }
  let inv = 255 - u16::from(src.a);
  let blend = |s: u8, d: u8| -> u8 {
    let v = u16::from(s) + (u16::from(d) * inv + 127) / 255;
    v.min(255) as u8
  };
  PremulRgba8 {
    r: blend(src.r, dst.r),
    g: blend(src.g, dst.g),
    b: blend(src.b, dst.b),
    a: blend(src.a, dst.a),
  }
}
