//! Terminal backend: draws into a grid of character cells, printed with 24-bit ANSI colors.
//!
//! One cell is 8x16 logical pixels, so the 1280x720 (720p) canvas is 160x45 cells.
//! Mapping rules, chosen so text, fills and outlines line up:
//! - A rect covers the cells between its rounded edges.
//! - A rect shorter than [`SINGLE_ROW_BELOW`] collapses to the one row holding its center,
//!   so a 40 px button is one row with its label, not three.
//! - A point (text line, line, glyph) lands in the cell that contains it.

use super::{
  Border, Color, IconData, Offset, Rect, Renderer, Size, TextPaint, ToggleColors, TrackColors,
};

pub const CELL_WIDTH: f32 = 8.0;
pub const CELL_HEIGHT: f32 = 16.0;

/// Rects shorter than this (in px) are drawn on a single row.
pub const SINGLE_ROW_BELOW: f32 = 44.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
  pub ch: char,
  pub fg: Color,
  pub bg: Color,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
}

impl Cell {
  fn blank(bg: Color) -> Self {
    Cell { ch: ' ', fg: Color::BLACK, bg, bold: false, italic: false, underline: false }
  }
}

/// Cell range; `c1`/`r1` are exclusive.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cells {
  c0: i32,
  r0: i32,
  c1: i32,
  r1: i32,
}

impl Cells {
  fn intersect(self, o: Cells) -> Cells {
    Cells {
      c0: self.c0.max(o.c0),
      r0: self.r0.max(o.r0),
      c1: self.c1.min(o.c1),
      r1: self.r1.min(o.r1),
    }
  }

  fn rows(&self) -> i32 {
    self.r1 - self.r0
  }

  fn cols(&self) -> i32 {
    self.c1 - self.c0
  }
}

pub struct TerminalRenderer {
  size: Size,
  cols: i32,
  rows: i32,
  cells: Vec<Cell>,
  clips: Vec<Cells>,
}

impl TerminalRenderer {
  /// A grid for a canvas of `size` logical pixels.
  pub fn new(size: Size) -> Self {
    let cols = (size.width / CELL_WIDTH).floor().max(1.0) as i32;
    let rows = (size.height / CELL_HEIGHT).floor().max(1.0) as i32;
    TerminalRenderer {
      size,
      cols,
      rows,
      cells: vec![Cell::blank(Color::WHITE); (cols * rows) as usize],
      clips: Vec::new(),
    }
  }

  /// 1280x720 logical pixels: 160x45 cells.
  pub fn hd720() -> Self {
    Self::new(super::HD_720)
  }

  pub fn cols(&self) -> i32 {
    self.cols
  }

  pub fn rows(&self) -> i32 {
    self.rows
  }

  pub fn cell(&self, col: i32, row: i32) -> Option<&Cell> {
    if col < 0 || row < 0 || col >= self.cols || row >= self.rows {
      return None;
    }
    self.cells.get((row * self.cols + col) as usize)
  }

  /// The characters only, one line per row, trailing spaces trimmed. Used by tests.
  pub fn to_plain_text(&self) -> String {
    let mut out = String::new();
    for row in 0..self.rows {
      let line: String =
        (0..self.cols).map(|c| self.cells[(row * self.cols + c) as usize].ch).collect();
      out.push_str(line.trim_end());
      out.push('\n');
    }
    out
  }

  /// The frame as ANSI escapes (24-bit color), one line per row, starting at the cursor.
  pub fn to_ansi(&self) -> String {
    let mut out = String::new();
    for row in self.ansi_rows() {
      out.push_str(&row);
      out.push('\n');
    }
    out
  }

  /// Each row as ANSI escapes (24-bit color), ending with a style reset; no newlines.
  pub fn ansi_rows(&self) -> Vec<String> {
    (0..self.rows)
      .map(|row| {
        let mut out = String::with_capacity(self.cols as usize * 4);
        let mut last: Option<(Color, Color, bool, bool, bool)> = None;
        for col in 0..self.cols {
          let cell = &self.cells[(row * self.cols + col) as usize];
          // A blank cell's foreground doesn't show; don't switch style for it.
          let fg = if cell.ch == ' ' && !cell.underline {
            last.map_or(cell.fg, |l| l.0)
          } else {
            cell.fg.over(cell.bg)
          };
          let style = (fg, cell.bg, cell.bold, cell.italic, cell.underline);
          if last != Some(style) {
            out.push_str("\x1b[0");
            if cell.bold {
              out.push_str(";1");
            }
            if cell.italic {
              out.push_str(";3");
            }
            if cell.underline {
              out.push_str(";4");
            }
            out.push_str(&format!(
              ";38;2;{};{};{};48;2;{};{};{}m",
              fg.r, fg.g, fg.b, cell.bg.r, cell.bg.g, cell.bg.b
            ));
            last = Some(style);
          }
          out.push(cell.ch);
        }
        out.push_str("\x1b[0m");
        out
      })
      .collect()
  }

  // ---- mapping ----

  fn col(x: f32) -> i32 {
    (x / CELL_WIDTH).floor() as i32
  }

  fn row(y: f32) -> i32 {
    (y / CELL_HEIGHT).floor() as i32
  }

  fn cells_of(rect: Rect) -> Cells {
    let c0 = (rect.x / CELL_WIDTH).round() as i32;
    let mut c1 = (rect.right() / CELL_WIDTH).round() as i32;
    if c1 == c0 && rect.width > 0.0 {
      c1 = c0 + 1;
    }
    let (r0, r1) = if rect.height < SINGLE_ROW_BELOW {
      let r = Self::row(rect.center().y);
      (r, if rect.height > 0.0 { r + 1 } else { r })
    } else {
      ((rect.y / CELL_HEIGHT).round() as i32, (rect.bottom() / CELL_HEIGHT).round() as i32)
    };
    Cells { c0, r0, c1, r1 }
  }

  fn clip(&self) -> Cells {
    self.clips.last().copied().unwrap_or(Cells { c0: 0, r0: 0, c1: self.cols, r1: self.rows })
  }

  fn visible(&self, col: i32, row: i32) -> bool {
    let c = self.clip();
    col >= c.c0 && col < c.c1 && row >= c.r0 && row < c.r1
  }

  fn with_cell(&mut self, col: i32, row: i32, f: impl FnOnce(&mut Cell)) {
    if self.visible(col, row) {
      let i = (row * self.cols + col) as usize;
      f(&mut self.cells[i]);
    }
  }

  /// Puts a colored glyph in a cell, keeping its background.
  fn glyph(&mut self, col: i32, row: i32, ch: char, color: Color, bold: bool) {
    self.with_cell(col, row, |cell| {
      cell.ch = ch;
      cell.fg = color.over(cell.bg);
      cell.bold = bold;
      cell.italic = false;
      cell.underline = false;
    });
  }

  fn fill_cells(&mut self, cells: Cells, color: Color) {
    for row in cells.r0..cells.r1 {
      for col in cells.c0..cells.c1 {
        self.with_cell(col, row, |cell| {
          if color.a == 255 {
            *cell = Cell::blank(color);
          } else {
            cell.bg = color.over(cell.bg);
            cell.fg = color.over(cell.fg);
          }
        });
      }
    }
  }

  fn text_at(&mut self, col: i32, row: i32, text: &str, color: Color, bold: bool) {
    for (i, ch) in text.chars().enumerate() {
      self.glyph(col + i as i32, row, ch, color, bold);
    }
  }
}

impl Renderer for TerminalRenderer {
  fn size(&self) -> Size {
    self.size
  }

  fn measure_text(&self, text: &str, _paint: &TextPaint) -> Size {
    let chars = text.chars().filter(|c| !c.is_control()).count();
    Size::new(chars as f32 * CELL_WIDTH, CELL_HEIGHT)
  }

  fn clear(&mut self, color: Color) {
    self.cells.fill(Cell::blank(color));
  }

  fn fill_rect(&mut self, rect: Rect, color: Color, _radius: f32) {
    if rect.is_empty() || color.a == 0 {
      return;
    }
    // Thin bars become half-block glyphs instead of a whole row or column of color.
    if rect.height < 10.0 {
      let row = Self::row(rect.center().y);
      let upper = rect.center().y - row as f32 * CELL_HEIGHT < CELL_HEIGHT / 2.0;
      let ch = if upper { '▀' } else { '▄' };
      let cells = Self::cells_of(rect);
      for col in cells.c0..cells.c1 {
        self.glyph(col, row, ch, color, false);
      }
      return;
    }
    if rect.width < 6.0 {
      let col = Self::col(rect.center().x);
      let cells = Self::cells_of(rect);
      for row in cells.r0..cells.r1 {
        self.glyph(col, row, '▌', color, false);
      }
      return;
    }
    self.fill_cells(Self::cells_of(rect), color);
  }

  fn stroke_rect(&mut self, rect: Rect, border: Border, radius: f32) {
    if rect.is_empty() || border.color.a == 0 {
      return;
    }
    let c = Self::cells_of(rect);
    if c.cols() < 2 || c.rows() < 1 {
      return;
    }
    if c.rows() == 1 {
      let (l, r) = if radius >= 8.0 { ('(', ')') } else { ('[', ']') };
      self.glyph(c.c0, c.r0, l, border.color, false);
      self.glyph(c.c1 - 1, c.r0, r, border.color, false);
      return;
    }
    let heavy = border.width >= 2.0;
    let (h, v) = if heavy { ('━', '┃') } else { ('─', '│') };
    let (tl, tr, bl, br) = if heavy {
      ('┏', '┓', '┗', '┛')
    } else if radius >= 4.0 {
      ('╭', '╮', '╰', '╯')
    } else {
      ('┌', '┐', '└', '┘')
    };
    let (top, bottom, left, right) = (c.r0, c.r1 - 1, c.c0, c.c1 - 1);
    for col in left + 1..right {
      self.glyph(col, top, h, border.color, false);
      self.glyph(col, bottom, h, border.color, false);
    }
    for row in top + 1..bottom {
      self.glyph(left, row, v, border.color, false);
      self.glyph(right, row, v, border.color, false);
    }
    self.glyph(left, top, tl, border.color, false);
    self.glyph(right, top, tr, border.color, false);
    self.glyph(left, bottom, bl, border.color, false);
    self.glyph(right, bottom, br, border.color, false);
  }

  fn draw_line(&mut self, from: Offset, to: Offset, color: Color, width: f32) {
    let heavy = width >= 2.0;
    if (from.y - to.y).abs() < CELL_HEIGHT / 4.0 {
      let row = Self::row((from.y + to.y) / 2.0);
      let (x0, x1) = (from.x.min(to.x), from.x.max(to.x));
      let c0 = (x0 / CELL_WIDTH).round() as i32;
      let c1 = ((x1 / CELL_WIDTH).round() as i32).max(c0 + 1);
      for col in c0..c1 {
        self.glyph(col, row, if heavy { '━' } else { '─' }, color, false);
      }
      return;
    }
    if (from.x - to.x).abs() < CELL_WIDTH / 2.0 {
      let col = Self::col((from.x + to.x) / 2.0);
      let (y0, y1) = (from.y.min(to.y), from.y.max(to.y));
      // Same rows as a rect of this height, so a divider inside a one-row control stays in it.
      let cells = Self::cells_of(Rect::new(from.x, y0, 1.0, y1 - y0));
      let (r0, r1) = (cells.r0, cells.r1.max(cells.r0 + 1));
      for row in r0..r1 {
        self.glyph(col, row, if heavy { '┃' } else { '│' }, color, false);
      }
      return;
    }
    // Diagonal: step through the cells along the line.
    let ch = if (to.x - from.x) * (to.y - from.y) > 0.0 { '╲' } else { '╱' };
    let steps =
      ((to.x - from.x).abs() / CELL_WIDTH).max((to.y - from.y).abs() / CELL_HEIGHT).ceil().max(1.0)
        as i32;
    for i in 0..=steps {
      let t = i as f32 / steps as f32;
      let p = Offset::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
      self.glyph(Self::col(p.x), Self::row(p.y), ch, color, false);
    }
  }

  fn fill_circle(&mut self, center: Offset, radius: f32, color: Color) {
    if radius <= 12.0 {
      let (col, row) = (Self::col(center.x), Self::row(center.y));
      let ring = self.cell(col, row).is_some_and(|c| c.ch == '○');
      self.glyph(col, row, if ring { '◉' } else { '●' }, color, false);
      return;
    }
    let c =
      Self::cells_of(Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0));
    for row in c.r0..c.r1 {
      for col in c.c0..c.c1 {
        let dx = (col as f32 + 0.5) * CELL_WIDTH - center.x;
        let dy = (row as f32 + 0.5) * CELL_HEIGHT - center.y;
        if dx * dx + dy * dy <= radius * radius {
          self.fill_cells(Cells { c0: col, r0: row, c1: col + 1, r1: row + 1 }, color);
        }
      }
    }
  }

  fn stroke_circle(&mut self, center: Offset, radius: f32, border: Border) {
    if radius <= 12.0 {
      let (col, row) = (Self::col(center.x), Self::row(center.y));
      let filled = self.cell(col, row).is_some_and(|c| c.ch == '●');
      self.glyph(col, row, if filled { '◉' } else { '○' }, border.color, false);
      return;
    }
    let c =
      Self::cells_of(Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0));
    for row in c.r0..c.r1 {
      for col in c.c0..c.c1 {
        let dx = (col as f32 + 0.5) * CELL_WIDTH - center.x;
        let dy = (row as f32 + 0.5) * CELL_HEIGHT - center.y;
        if ((dx * dx + dy * dy).sqrt() - radius).abs() <= CELL_WIDTH / 2.0 + border.width {
          self.glyph(col, row, '•', border.color, false);
        }
      }
    }
  }

  fn draw_text(&mut self, origin: Offset, text: &str, paint: &TextPaint) {
    let row = Self::row(origin.y + CELL_HEIGHT / 2.0);
    let col0 = (origin.x / CELL_WIDTH).round() as i32;
    for (i, ch) in text.chars().filter(|c| !c.is_control()).enumerate() {
      self.with_cell(col0 + i as i32, row, |cell| {
        cell.ch = ch;
        cell.fg = paint.color.over(cell.bg);
        cell.bold = paint.bold;
        cell.italic = paint.italic;
        cell.underline = paint.underline;
      });
    }
  }

  fn push_clip(&mut self, rect: Rect) {
    let cells = Self::cells_of(rect).intersect(self.clip());
    self.clips.push(cells);
  }

  fn pop_clip(&mut self) {
    self.clips.pop();
  }

  fn snap_rect(&self, rect: Rect) -> Rect {
    let c = Self::cells_of(rect);
    Rect::new(
      c.c0 as f32 * CELL_WIDTH,
      c.r0 as f32 * CELL_HEIGHT,
      c.cols() as f32 * CELL_WIDTH,
      c.rows() as f32 * CELL_HEIGHT,
    )
  }

  // ---- widget-level overrides: glyphs read better than 8x16 px blocks ----

  fn draw_shadow(&mut self, _rect: Rect, _radius: f32, _elevation: f32) {
    // No room for a shadow; Material 3 surfaces also differ by tone, which still shows.
  }

  fn draw_icon(&mut self, rect: Rect, icon: IconData, color: Color) {
    let c = rect.center();
    self.glyph(Self::col(c.x), Self::row(c.y), icon.glyph, color, false);
  }

  fn draw_checkbox(&mut self, rect: Rect, checked: bool, colors: &ToggleColors) {
    let c = rect.center();
    let (col, row) = (Self::col(c.x) - 1, Self::row(c.y));
    if checked {
      self.text_at(col, row, "[✓]", colors.active, true);
    } else {
      self.text_at(col, row, "[ ]", colors.inactive, false);
    }
  }

  fn draw_radio(&mut self, rect: Rect, selected: bool, colors: &ToggleColors) {
    let c = rect.center();
    let (ch, color) = if selected { ('◉', colors.active) } else { ('○', colors.inactive) };
    self.glyph(Self::col(c.x), Self::row(c.y), ch, color, selected);
  }

  fn draw_switch(&mut self, rect: Rect, on: bool, colors: &ToggleColors) {
    let c = Self::cells_of(rect.center_rect(Size::new(52.0, 32.0)));
    let track = if on { colors.active } else { colors.on_inactive };
    self.fill_cells(c, track);
    if on {
      self.glyph(c.c1 - 1, c.r0, '●', colors.on_active, true);
    } else {
      self.glyph(c.c0, c.r0, '●', colors.inactive, false);
    }
  }

  fn draw_slider(&mut self, rect: Rect, fraction: f32, colors: &TrackColors) {
    let c = Self::cells_of(rect);
    let row = Self::row(rect.center().y);
    let n = c.cols().max(1);
    let thumb = c.c0 + ((n - 1) as f32 * fraction.clamp(0.0, 1.0)).round() as i32;
    for col in c.c0..c.c1 {
      if col < thumb {
        self.glyph(col, row, '━', colors.active, false);
      } else if col > thumb {
        self.glyph(col, row, '─', colors.inactive, false);
      }
    }
    self.glyph(thumb, row, '●', colors.thumb, true);
  }

  fn draw_linear_progress(&mut self, rect: Rect, value: Option<f32>, colors: &TrackColors) {
    let c = Self::cells_of(rect);
    let row = Self::row(rect.center().y);
    let n = c.cols() as f32;
    let (start, end) = match value {
      Some(v) => (0, (n * v.clamp(0.0, 1.0)).round() as i32),
      None => ((n * 0.2) as i32, (n * 0.6) as i32),
    };
    for (i, col) in (c.c0..c.c1).enumerate() {
      let i = i as i32;
      if i >= start && i < end {
        self.glyph(col, row, '━', colors.active, false);
      } else {
        self.glyph(col, row, '─', colors.inactive, false);
      }
    }
  }

  fn draw_circular_progress(&mut self, rect: Rect, value: Option<f32>, colors: &TrackColors) {
    let ch = match value {
      None => '◌',
      Some(v) if v <= 0.0 => '○',
      Some(v) if v < 0.375 => '◔',
      Some(v) if v < 0.625 => '◑',
      Some(v) if v < 1.0 => '◕',
      Some(_) => '●',
    };
    let c = rect.center();
    self.glyph(Self::col(c.x), Self::row(c.y), ch, colors.active, true);
  }

  fn draw_focus(&mut self, rect: Rect, color: Color, radius: f32) {
    let c = Self::cells_of(rect);
    if c.rows() <= 1 {
      // Markers just outside a one-row control.
      self.glyph(c.c0 - 1, c.r0, '▸', color, true);
      self.glyph(c.c1, c.r0, '◂', color, true);
    } else {
      self.stroke_rect(rect, Border { color, width: 2.0 }, radius);
    }
  }

  fn draw_caret(&mut self, at: Offset, height: f32, color: Color) {
    let (col, row) = (Self::col(at.x), Self::row(at.y + height / 2.0));
    self.with_cell(col, row, |cell| cell.bg = color);
  }
}
