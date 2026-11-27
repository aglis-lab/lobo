//! Standalone, zero-dependency animation library for Lobo.
//!
//! Provides easing [`Curve`]s, generic [`Lerp`] + [`Tween`], physics-based [`Spring`],
//! time-based [`AnimatedF32`], [`LoopingPhase`], and a lightweight frame ticker that
//! idles at 0% CPU once all animations settle.

use std::cell::Cell;
use std::time::Instant;

thread_local! {
  static FRAME_REQUESTED: Cell<bool> = const { Cell::new(false) };
  static ANIMATIONS_ENABLED: Cell<bool> = const { Cell::new(true) };
}

/// Signals that at least one animation is in-flight and needs another frame.
#[inline]
pub fn request_frame() {
  FRAME_REQUESTED.with(|f| f.set(true));
}

/// Returns whether any animation requested a follow-up frame, and resets the flag.
#[inline]
pub fn take_frame_request() -> bool {
  FRAME_REQUESTED.with(|f| f.replace(false))
}

/// Enables or disables animations on the current thread (useful for reduce-motion or snapshot tests).
#[inline]
pub fn set_enabled(enabled: bool) {
  ANIMATIONS_ENABLED.with(|e| e.set(enabled));
}

/// Returns whether animations are currently enabled.
#[inline]
pub fn is_enabled() -> bool {
  ANIMATIONS_ENABLED.with(Cell::get)
}

// ---------------------------------------------------------------------------
// Curves
// ---------------------------------------------------------------------------

/// Easing curves for time-based animations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Curve {
  Linear,
  EaseIn,
  EaseOut,
  EaseInOut,
  /// Standard emphasized-decelerate curve.
  #[default]
  FastOutSlowIn,
  /// Custom cubic-bezier curve `(x1, y1, x2, y2)` with `x1, x2` in `0.0..=1.0`.
  CubicBezier(f32, f32, f32, f32),
}

impl Curve {
  /// Evaluates the curve for normalized progress `t` in `0.0..=1.0`.
  pub fn apply(self, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match self {
      Curve::Linear => t,
      Curve::EaseIn => t * t * t,
      Curve::EaseOut => {
        let u = 1.0 - t;
        1.0 - u * u * u
      }
      Curve::EaseInOut => {
        if t < 0.5 {
          4.0 * t * t * t
        } else {
          let u = -2.0 * t + 2.0;
          1.0 - (u * u * u) / 2.0
        }
      }
      Curve::FastOutSlowIn => {
        let u = 1.0 - t;
        1.0 - u * u * u * (1.0 + 1.5 * t)
      }
      Curve::CubicBezier(x1, y1, x2, y2) => eval_cubic_bezier(t, x1, y1, x2, y2),
    }
  }
}

fn eval_cubic_bezier(x: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
  let sample = |u: f32, p1: f32, p2: f32| -> f32 {
    let inv = 1.0 - u;
    3.0 * inv * inv * u * p1 + 3.0 * inv * u * u * p2 + u * u * u
  };
  let mut lo = 0.0f32;
  let mut hi = 1.0f32;
  let mut u = x;
  for _ in 0..8 {
    let est = sample(u, x1, x2);
    if (est - x).abs() < 1e-4 {
      break;
    }
    if est < x {
      lo = u;
    } else {
      hi = u;
    }
    u = 0.5 * (lo + hi);
  }
  sample(u, y1, y2)
}

// ---------------------------------------------------------------------------
// Lerp & Tween
// ---------------------------------------------------------------------------

/// Linearly interpolates between `a` and `b` by `t` (`0.0..=1.0`).
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
  a + (b - a) * t.clamp(0.0, 1.0)
}

/// Trait for types that can be linearly interpolated.
pub trait Lerp: Copy {
  fn lerp(self, other: Self, t: f32) -> Self;
}

impl Lerp for f32 {
  #[inline]
  fn lerp(self, other: Self, t: f32) -> Self {
    lerp(self, other, t)
  }
}

impl Lerp for f64 {
  #[inline]
  fn lerp(self, other: Self, t: f32) -> Self {
    self + (other - self) * f64::from(t.clamp(0.0, 1.0))
  }
}

impl Lerp for (f32, f32) {
  #[inline]
  fn lerp(self, other: Self, t: f32) -> Self {
    (lerp(self.0, other.0, t), lerp(self.1, other.1, t))
  }
}

/// A value range `[begin, end]` evaluated with a [`Curve`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween<T: Lerp> {
  pub begin: T,
  pub end: T,
  pub curve: Curve,
}

impl<T: Lerp> Tween<T> {
  pub const fn new(begin: T, end: T) -> Self {
    Tween { begin, end, curve: Curve::FastOutSlowIn }
  }

  pub const fn with_curve(mut self, curve: Curve) -> Self {
    self.curve = curve;
    self
  }

  pub fn eval(&self, t: f32) -> T {
    self.begin.lerp(self.end, self.curve.apply(t))
  }
}

// ---------------------------------------------------------------------------
// AnimatedF32 (time-based)
// ---------------------------------------------------------------------------

/// Persistent animated `f32` value that smoothly interpolates whenever `target` changes.
#[derive(Clone, Debug, Default)]
pub struct AnimatedF32 {
  initialized: bool,
  from: f32,
  to: f32,
  current: f32,
  started: Option<Instant>,
  duration_ms: f32,
}

impl AnimatedF32 {
  /// Advances the animation toward `target` over `duration_ms` using `curve`.
  ///
  /// Returns `(current_value, is_animating)`. Snaps on the first call or when
  /// [`is_enabled`] is `false`.
  pub fn advance(&mut self, target: f32, duration_ms: f32, curve: Curve) -> (f32, bool) {
    let now = Instant::now();
    if !self.initialized || !is_enabled() {
      self.initialized = true;
      self.from = target;
      self.to = target;
      self.current = target;
      self.started = None;
      self.duration_ms = duration_ms;
      return (target, false);
    }

    if (target - self.to).abs() > 1e-4 {
      self.from = self.current;
      self.to = target;
      self.started = Some(now);
      self.duration_ms = duration_ms.max(1.0);
    }

    let Some(start) = self.started else {
      return (self.current, false);
    };

    let elapsed_ms = now.duration_since(start).as_secs_f32() * 1000.0;
    let raw_t = (elapsed_ms / self.duration_ms).clamp(0.0, 1.0);
    if raw_t >= 1.0 {
      self.current = self.to;
      self.from = self.to;
      self.started = None;
      return (self.current, false);
    }

    self.current = lerp(self.from, self.to, curve.apply(raw_t));
    request_frame();
    (self.current, true)
  }

  pub fn value(&self) -> f32 {
    self.current
  }

  pub fn is_animating(&self) -> bool {
    self.started.is_some()
  }
}

// ---------------------------------------------------------------------------
// Spring (physics-based)
// ---------------------------------------------------------------------------

/// Damped harmonic spring animator (`stiffness`, `damping`) for natural UI motion.
#[derive(Clone, Debug)]
pub struct Spring {
  initialized: bool,
  position: f32,
  velocity: f32,
  target: f32,
  pub stiffness: f32,
  pub damping: f32,
  last_tick: Option<Instant>,
}

impl Default for Spring {
  fn default() -> Self {
    Self::new(320.0, 28.0)
  }
}

impl Spring {
  pub fn new(stiffness: f32, damping: f32) -> Self {
    Spring {
      initialized: false,
      position: 0.0,
      velocity: 0.0,
      target: 0.0,
      stiffness,
      damping,
      last_tick: None,
    }
  }

  /// Advances the spring toward `target` using wall-clock delta time.
  pub fn advance(&mut self, target: f32) -> (f32, bool) {
    let now = Instant::now();
    if !self.initialized || !is_enabled() {
      self.initialized = true;
      self.position = target;
      self.target = target;
      self.velocity = 0.0;
      self.last_tick = None;
      return (target, false);
    }
    let dt = self
      .last_tick
      .map(|t| now.duration_since(t).as_secs_f32().clamp(0.001, 0.05))
      .unwrap_or(1.0 / 60.0);
    self.last_tick = Some(now);
    let (pos, active) = self.step(target, dt);
    if !active {
      self.last_tick = None;
    }
    (pos, active)
  }

  /// Advances the spring toward `target` by an explicit `dt` seconds.
  pub fn step(&mut self, target: f32, dt: f32) -> (f32, bool) {
    if !self.initialized || !is_enabled() {
      self.initialized = true;
      self.position = target;
      self.target = target;
      self.velocity = 0.0;
      return (target, false);
    }
    self.target = target;
    let dt = dt.clamp(0.0001, 0.05);
    let force = -self.stiffness * (self.position - self.target) - self.damping * self.velocity;
    self.velocity += force * dt;
    self.position += self.velocity * dt;

    if (self.position - self.target).abs() < 0.001 && self.velocity.abs() < 0.01 {
      self.position = self.target;
      self.velocity = 0.0;
      (self.position, false)
    } else {
      request_frame();
      (self.position, true)
    }
  }

  pub fn value(&self) -> f32 {
    self.position
  }
}

// ---------------------------------------------------------------------------
// LoopingPhase (for indeterminate progress / spinners)
// ---------------------------------------------------------------------------

/// Looping normalized phase (`0.0..1.0`) for indeterminate progress indicators.
#[derive(Clone, Debug)]
pub struct LoopingPhase {
  origin: Instant,
}

impl Default for LoopingPhase {
  fn default() -> Self {
    LoopingPhase { origin: Instant::now() }
  }
}

impl LoopingPhase {
  /// Returns current `0.0..1.0` phase for a cycle of `period_ms` and requests the next frame.
  pub fn sample(&self, period_ms: f32) -> f32 {
    if !is_enabled() {
      return 0.25;
    }
    request_frame();
    let elapsed_ms = self.origin.elapsed().as_secs_f32() * 1000.0;
    (elapsed_ms / period_ms.max(1.0)).fract()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn curves_and_tweens_preserve_endpoints() {
    let curves = [
      Curve::Linear,
      Curve::EaseIn,
      Curve::EaseOut,
      Curve::EaseInOut,
      Curve::FastOutSlowIn,
      Curve::CubicBezier(0.25, 0.1, 0.25, 1.0),
    ];
    for c in curves {
      assert!((c.apply(0.0) - 0.0).abs() < 1e-4);
      assert!((c.apply(1.0) - 1.0).abs() < 1e-4);
    }
    let tw = Tween::new(10.0f32, 30.0).with_curve(Curve::Linear);
    assert!((tw.eval(0.5) - 20.0).abs() < 1e-4);
  }

  #[test]
  fn spring_converges_to_target() {
    set_enabled(true);
    let mut s = Spring::default();
    assert_eq!(s.step(0.0, 1.0 / 60.0), (0.0, false));
    let mut active = true;
    for _ in 0..120 {
      let (_, a) = s.step(1.0, 1.0 / 60.0);
      active = a;
      if !active {
        break;
      }
    }
    assert!(!active);
    assert!((s.value() - 1.0).abs() < 0.01);
  }
}
