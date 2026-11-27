//! Re-exports the standalone [`::anim`] crate plus [`lerp_color`] and declarative
//! animation widgets ([`AnimatedValue`], [`AnimatedSpring`]).

pub use ::anim::*;

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use anymap3::AnyMap;

use crate::render::Color;
use crate::{Context, Element, NodeCx, Signal, Widget, ui};

/// Linearly interpolates between two sRGB [`Color`]s by `t` (`0.0..=1.0`).
#[inline]
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
  let t = t.clamp(0.0, 1.0);
  let ch = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
  Color::rgba(ch(a.r, b.r), ch(a.g, b.g), ch(a.b, b.b), ch(a.a, b.a))
}

// ---------------------------------------------------------------------------
// Frame ticker registry for declarative animation widgets
// ---------------------------------------------------------------------------

type TickerFn = dyn Fn() -> bool;

thread_local! {
  static ACTIVE_TICKERS: RefCell<Vec<Weak<TickerFn>>> = const { RefCell::new(Vec::new()) };
}

fn register_ticker(ticker: &Rc<TickerFn>) {
  let weak = Rc::downgrade(ticker);
  ACTIVE_TICKERS.with(|list| {
    let mut list = list.borrow_mut();
    if !list.iter().any(|w| w.ptr_eq(&weak)) {
      list.push(weak);
    }
  });
}

/// Advances all active declarative animation widgets (`AnimatedValue`, `AnimatedSpring`).
/// Called automatically once per frame by `Node::update()`.
pub(crate) fn tick_animations() {
  let tickers: Vec<Rc<TickerFn>> = ACTIVE_TICKERS.with(|list| {
    let prev = std::mem::take(&mut *list.borrow_mut());
    prev.into_iter().filter_map(|w| w.upgrade()).collect()
  });
  let mut still_active = Vec::new();
  for ticker in tickers {
    if ticker() {
      still_active.push(Rc::downgrade(&ticker));
    }
  }
  if !still_active.is_empty() {
    ACTIVE_TICKERS.with(|list| {
      let mut list = list.borrow_mut();
      for w in still_active {
        if !list.iter().any(|e| e.ptr_eq(&w)) {
          list.push(w);
        }
      }
    });
  }
}

// ---------------------------------------------------------------------------
// ValueBuilder + shared animated slot
// ---------------------------------------------------------------------------

/// Builder closure `Fn(T) -> Widget` used by declarative animation widgets.
#[derive(Clone)]
pub struct ValueBuilder<T>(Rc<dyn Fn(T) -> Widget>);

impl<T> ValueBuilder<T> {
  pub fn call(&self, value: T) -> Widget {
    (self.0)(value)
  }
}

impl<T, F: Fn(T) -> Widget + 'static> From<F> for ValueBuilder<T> {
  fn from(f: F) -> Self {
    ValueBuilder(Rc::new(f))
  }
}

#[derive(Clone)]
struct AnimatedSlot(Signal<f32>);

#[derive(Widget)]
struct AnimatedSlotBody {
  builder: ValueBuilder<f32>,
}

impl Element for AnimatedSlotBody {
  fn build(&self, context: &Context) -> Option<Widget> {
    let val = context.try_read::<AnimatedSlot>().map(|s| s.0.get()).unwrap_or(0.0);
    Some(self.builder.call(val))
  }
}

// ---------------------------------------------------------------------------
// AnimatedValue (declarative easing curve animation)
// ---------------------------------------------------------------------------

struct AnimatedValueInner {
  anim: AnimatedF32,
  phase: LoopingPhase,
  target: f32,
  duration_ms: f32,
  curve: Curve,
  repeat: bool,
  initialized: bool,
}

impl Default for AnimatedValueInner {
  fn default() -> Self {
    AnimatedValueInner {
      anim: AnimatedF32::default(),
      phase: LoopingPhase::default(),
      target: 0.0,
      duration_ms: 300.0,
      curve: Curve::FastOutSlowIn,
      repeat: false,
      initialized: false,
    }
  }
}

struct AnimatedValueState {
  value: Signal<f32>,
  inner: Rc<RefCell<AnimatedValueInner>>,
  ticker: Option<Rc<TickerFn>>,
}

impl Default for AnimatedValueState {
  fn default() -> Self {
    AnimatedValueState {
      value: Signal::new(0.0),
      inner: Rc::new(RefCell::new(AnimatedValueInner::default())),
      ticker: None,
    }
  }
}

/// Declaratively animates an `f32` toward `target` over `duration_ms` using `curve`,
/// rebuilding only its `builder` subtree as the value changes.
///
/// Set `repeat: true` to continuously ping-pong between `0.0` and `1.0`.
#[derive(Widget)]
#[non_exhaustive]
pub struct AnimatedValue {
  pub target: f32,
  /// Duration in milliseconds (defaults to `300.0`).
  #[widget(optional)]
  pub duration_ms: f32,
  /// Easing curve (defaults to [`Curve::FastOutSlowIn`]).
  #[widget(optional)]
  pub curve: Curve,
  /// Continuously ping-pong between `0.0` and `1.0` when `true`.
  #[widget(optional)]
  pub repeat: bool,
  pub builder: ValueBuilder<f32>,
}

impl Element for AnimatedValue {
  fn attach(&self, cx: &NodeCx) {
    let duration_ms = if self.duration_ms > 0.0 { self.duration_ms } else { 300.0 };
    let state = cx.state::<AnimatedValueState>();
    let mut st = state.borrow_mut();
    let should_run = {
      let mut inner = st.inner.borrow_mut();
      if !inner.initialized {
        inner.initialized = true;
        inner.target = self.target;
        inner.duration_ms = duration_ms;
        inner.curve = self.curve;
        inner.repeat = self.repeat;
        let (v, _) = inner.anim.advance(self.target, duration_ms, self.curve);
        st.value.set(v);
        self.repeat
      } else {
        if self.repeat && !inner.repeat {
          inner.phase = LoopingPhase::default();
        }
        inner.target = self.target;
        inner.duration_ms = duration_ms;
        inner.curve = self.curve;
        inner.repeat = self.repeat;
        self.repeat || (st.value.peek() - self.target).abs() > 1e-4
      }
    };

    let inner_ref = Rc::clone(&st.inner);
    let sig_ref = st.value.clone();
    let ticker = st.ticker.get_or_insert_with(|| {
      Rc::new(move || {
        let mut inner = inner_ref.borrow_mut();
        if inner.repeat {
          let p = inner.phase.sample(inner.duration_ms * 2.0);
          let raw = if p < 0.5 { p * 2.0 } else { (1.0 - p) * 2.0 };
          let v = inner.curve.apply(raw);
          sig_ref.set(v);
          is_enabled()
        } else {
          let target = inner.target;
          let duration_ms = inner.duration_ms;
          let curve = inner.curve;
          let (v, active) = inner.anim.advance(target, duration_ms, curve);
          sig_ref.set(v);
          active
        }
      })
    });

    if should_run {
      register_ticker(ticker);
      request_frame();
    }
  }

  fn scope(&self, cx: &NodeCx) -> Option<Context> {
    let state = cx.state::<AnimatedValueState>();
    let sig = state.borrow().value.clone();
    let mut map = AnyMap::new();
    map.insert(Rc::new(AnimatedSlot(sig)));
    Some(cx.context().with_scope(map))
  }

  fn build(&self, _context: &Context) -> Option<Widget> {
    Some(ui!(AnimatedSlotBody { builder: self.builder.clone() }))
  }
}

// ---------------------------------------------------------------------------
// AnimatedSpring (declarative physics spring animation)
// ---------------------------------------------------------------------------

struct AnimatedSpringInner {
  spring: Spring,
  phase: LoopingPhase,
  target: f32,
  repeat: bool,
  repeat_period_ms: f32,
  initialized: bool,
}

impl Default for AnimatedSpringInner {
  fn default() -> Self {
    AnimatedSpringInner {
      spring: Spring::default(),
      phase: LoopingPhase::default(),
      target: 0.0,
      repeat: false,
      repeat_period_ms: 1800.0,
      initialized: false,
    }
  }
}

struct AnimatedSpringState {
  value: Signal<f32>,
  inner: Rc<RefCell<AnimatedSpringInner>>,
  ticker: Option<Rc<TickerFn>>,
}

impl Default for AnimatedSpringState {
  fn default() -> Self {
    AnimatedSpringState {
      value: Signal::new(0.0),
      inner: Rc::new(RefCell::new(AnimatedSpringInner::default())),
      ticker: None,
    }
  }
}

/// Declaratively animates an `f32` toward `target` with damped spring physics,
/// rebuilding only its `builder` subtree while in motion.
///
/// Set `repeat: true` to continuously bounce between `0.0` and `1.0`.
#[derive(Widget)]
#[non_exhaustive]
pub struct AnimatedSpring {
  pub target: f32,
  /// Spring stiffness (defaults to `320.0`).
  #[widget(optional)]
  pub stiffness: f32,
  /// Spring damping (defaults to `28.0`).
  #[widget(optional)]
  pub damping: f32,
  /// Continuously alternate target between `1.0` and `0.0` when `true`.
  #[widget(optional)]
  pub repeat: bool,
  /// Full cycle period in milliseconds when `repeat` is `true` (defaults to `1800.0`).
  #[widget(optional)]
  pub repeat_period_ms: f32,
  pub builder: ValueBuilder<f32>,
}

impl Element for AnimatedSpring {
  fn attach(&self, cx: &NodeCx) {
    let stiffness = if self.stiffness > 0.0 { self.stiffness } else { 320.0 };
    let damping = if self.damping > 0.0 { self.damping } else { 28.0 };
    let period_ms = if self.repeat_period_ms > 0.0 { self.repeat_period_ms } else { 1800.0 };
    let state = cx.state::<AnimatedSpringState>();
    let mut st = state.borrow_mut();
    let should_run = {
      let mut inner = st.inner.borrow_mut();
      inner.spring.stiffness = stiffness;
      inner.spring.damping = damping;
      inner.repeat_period_ms = period_ms;
      if !inner.initialized {
        inner.initialized = true;
        inner.target = self.target;
        inner.repeat = self.repeat;
        let (v, _) = inner.spring.advance(self.target);
        st.value.set(v);
        self.repeat
      } else {
        if self.repeat && !inner.repeat {
          inner.phase = LoopingPhase::default();
        }
        inner.target = self.target;
        inner.repeat = self.repeat;
        self.repeat || (st.value.peek() - self.target).abs() > 1e-4
      }
    };

    let inner_ref = Rc::clone(&st.inner);
    let sig_ref = st.value.clone();
    let ticker = st.ticker.get_or_insert_with(|| {
      Rc::new(move || {
        let mut inner = inner_ref.borrow_mut();
        let eff_target = if inner.repeat {
          let p = inner.phase.sample(inner.repeat_period_ms);
          if p < 0.5 { 1.0 } else { 0.0 }
        } else {
          inner.target
        };
        let (v, active) = inner.spring.advance(eff_target);
        sig_ref.set(v);
        (inner.repeat && is_enabled()) || active
      })
    });

    if should_run {
      register_ticker(ticker);
      request_frame();
    }
  }

  fn scope(&self, cx: &NodeCx) -> Option<Context> {
    let state = cx.state::<AnimatedSpringState>();
    let sig = state.borrow().value.clone();
    let mut map = AnyMap::new();
    map.insert(Rc::new(AnimatedSlot(sig)));
    Some(cx.context().with_scope(map))
  }

  fn build(&self, _context: &Context) -> Option<Widget> {
    Some(ui!(AnimatedSlotBody { builder: self.builder.clone() }))
  }
}
