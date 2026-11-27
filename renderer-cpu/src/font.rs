use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use parley::{
  FontContext, FontFamily, FontFamilyName, FontStyle, FontWeight, GenericFamily, Layout,
  LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty,
};
use renderer::{Color, Offset, Size, TextPaint};
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::Affine;
use vello_cpu::{Glyph, RenderContext, Resources};

const MAX_CACHE_ENTRIES: usize = 2048;

const DEFAULT_FONT_FAMILIES: &[FontFamilyName<'static>] = &[
  FontFamilyName::Generic(GenericFamily::SystemUi),
  FontFamilyName::Generic(GenericFamily::SansSerif),
  #[cfg(target_os = "macos")]
  FontFamilyName::Named(Cow::Borrowed("Apple Symbols")),
  #[cfg(target_os = "macos")]
  FontFamilyName::Named(Cow::Borrowed("Zapf Dingbats")),
  #[cfg(target_os = "windows")]
  FontFamilyName::Named(Cow::Borrowed("Segoe UI Symbol")),
  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  FontFamilyName::Named(Cow::Borrowed("DejaVu Sans")),
];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
  text: String,
  size_bits: u32,
  scale_bits: u32,
  bold: bool,
  italic: bool,
}

/// Parley-backed text shaping, measurement, and `vello_cpu` glyph rendering engine.
pub(crate) struct TextEngine {
  font_cx: FontContext,
  layout_cx: LayoutContext<()>,
  cache: HashMap<CacheKey, Arc<Layout<()>>>,
}

impl TextEngine {
  pub(crate) fn new() -> Self {
    TextEngine {
      font_cx: FontContext::new(),
      layout_cx: LayoutContext::new(),
      cache: HashMap::new(),
    }
  }

  /// Registers a custom font (TTF/OTF/TTC) with the engine and clears cached layouts.
  pub(crate) fn register_font(&mut self, data: Vec<u8>) {
    self.font_cx.collection.register_fonts(data.into(), None);
    self.cache.clear();
  }

  fn layout(&mut self, text: &str, paint: &TextPaint, scale: f32) -> Arc<Layout<()>> {
    let size = paint.size.max(1.0);
    let key = CacheKey {
      text: text.to_owned(),
      size_bits: size.to_bits(),
      scale_bits: scale.to_bits(),
      bold: paint.bold,
      italic: paint.italic,
    };
    if let Some(cached) = self.cache.get(&key) {
      return Arc::clone(cached);
    }
    if self.cache.len() >= MAX_CACHE_ENTRIES {
      self.cache.clear();
    }

    let mut builder = self.layout_cx.ranged_builder(&mut self.font_cx, text, scale, true);
    builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Borrowed(
      DEFAULT_FONT_FAMILIES,
    ))));
    builder.push_default(StyleProperty::FontSize(size));
    builder.push_default(StyleProperty::LineHeight(LineHeight::NORMAL));
    if paint.bold {
      builder.push_default(StyleProperty::FontWeight(FontWeight::SEMI_BOLD));
    }
    if paint.italic {
      builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
    }

    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let layout = Arc::new(layout);
    self.cache.insert(key, Arc::clone(&layout));
    layout
  }

  /// Measures single-line `text` in logical pixels using Parley.
  pub(crate) fn measure_text(&mut self, text: &str, paint: &TextPaint, scale: f32) -> Size {
    let inv_scale = 1.0 / scale;
    if text.is_empty() {
      let probe = self.layout(" ", paint, scale);
      return Size::new(0.0, probe.height() * inv_scale);
    }
    let layout = self.layout(text, paint, scale);
    let width = layout.full_width() * inv_scale;
    let height = if layout.height() > 0.0 {
      layout.height() * inv_scale
    } else {
      self.layout(" ", paint, scale).height() * inv_scale
    };
    Size::new(width, height)
  }

  /// Renders single-line `text` into `ctx` with its top-left at logical `origin`.
  ///
  /// Expects `ctx` to be in logical-pixel transform space (`Affine::scale(scale)`),
  /// switches temporarily to physical-pixel coordinates for hinted/quantized glyph rendering,
  /// and restores the logical transform before returning.
  pub(crate) fn draw_text(
    &mut self,
    ctx: &mut RenderContext,
    resources: &mut Resources,
    origin: Offset,
    text: &str,
    paint: &TextPaint,
    scale: f32,
  ) {
    if paint.color.a == 0 || text.is_empty() {
      return;
    }
    let layout = self.layout(text, paint, scale);
    let ox = f64::from((origin.x * scale).round());
    let oy = f64::from((origin.y * scale).round());

    ctx.set_transform(Affine::translate((ox, oy)));
    ctx.set_paint(to_paint(paint.color));

    for line in layout.lines() {
      for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
          continue;
        };
        let run = glyph_run.run();
        let font = run.font();
        let font_size = run.font_size();
        let normalized_coords: Vec<i16> =
          run.normalized_coords().iter().map(|c| c.to_bits()).collect();
        let glyph_xform = run
          .synthesis()
          .skew()
          .map(|angle| Affine::skew(-f64::from(angle).to_radians().tan(), 0.0));

        let glyphs: Vec<Glyph> = glyph_run
          .positioned_glyphs()
          .map(|g| Glyph { id: g.id, x: g.x, y: g.y })
          .collect();

        let mut builder = ctx
          .glyph_run(resources, font)
          .font_size(font_size)
          .hint(true)
          .normalized_coords(&normalized_coords);
        if let Some(xform) = glyph_xform {
          builder = builder.glyph_transform(xform);
        }
        let _ = builder.fill_glyphs(glyphs.iter().copied());

        if paint.underline {
          let metrics = run.font_metrics();
          let x0 = glyph_run.offset();
          let x1 = x0 + glyph_run.advance();
          let underline_size = metrics.underline_size.max(scale);
          let mut dec = ctx
            .glyph_run(resources, font)
            .font_size(font_size)
            .hint(true)
            .normalized_coords(&normalized_coords);
          if let Some(xform) = glyph_xform {
            dec = dec.glyph_transform(xform);
          }
          dec.render_decoration(
            glyphs.iter().copied(),
            x0..=x1,
            glyph_run.baseline(),
            metrics.underline_offset,
            underline_size,
            (1.5 * scale).max(1.0),
          );
        }
      }
    }

    if (scale - 1.0).abs() > f32::EPSILON {
      ctx.set_transform(Affine::scale(f64::from(scale)));
    } else {
      ctx.set_transform(Affine::IDENTITY);
    }
  }
}

fn to_paint(c: Color) -> AlphaColor<Srgb> {
  AlphaColor::<Srgb>::from_rgba8(c.r, c.g, c.b, c.a)
}
