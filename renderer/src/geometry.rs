use std::ops::{Add, Sub};

/// 720p: the default canvas size, in logical pixels.
pub const HD_720: Size = Size { width: 1280.0, height: 720.0 };

/// A 2D offset in logical pixels.
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

/// A 2D size in logical pixels.
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

/// An axis-aligned rectangle in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
  pub x: f32,
  pub y: f32,
  pub width: f32,
  pub height: f32,
}

impl Rect {
  pub const ZERO: Rect = Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };

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

  /// Shrinks by the given edge insets on each side; never below zero size.
  pub fn deflate(&self, left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::new(
      self.x + left,
      self.y + top,
      (self.width - left - right).max(0.0),
      (self.height - top - bottom).max(0.0),
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

  /// Whether both rects have a non-empty overlap.
  pub fn intersects(&self, other: &Rect) -> bool {
    !self.intersect(other).is_empty()
  }

  /// Smallest rect covering both; an empty rect is an identity element.
  pub fn union(&self, other: &Rect) -> Rect {
    if self.is_empty() {
      return *other;
    }
    if other.is_empty() {
      return *self;
    }
    let x = self.x.min(other.x);
    let y = self.y.min(other.y);
    let right = self.right().max(other.right());
    let bottom = self.bottom().max(other.bottom());
    Rect::new(x, y, right - x, bottom - y)
  }

  /// Grows outward to whole integer pixel edges.
  pub fn round_out(&self) -> Rect {
    if self.is_empty() {
      return Rect::ZERO;
    }
    let x = self.x.floor();
    let y = self.y.floor();
    let right = self.right().ceil();
    let bottom = self.bottom().ceil();
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
