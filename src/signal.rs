//! Thread-safe signals: reading a `Signal` while an `Observer` is being
//! tracked subscribes that observer; writing (from any thread) marks every
//! subscriber dirty and wakes the UI loop.
//!
//! Only the UI thread tracks: `CURRENT` is thread-local, so reads from other
//! threads or tasks never subscribe, they just read.

use std::cell::RefCell;
use std::rc::{self, Rc};
use std::sync::atomic::{AtomicBool, Ordering};

/// Something that re-runs when a signal it read changes (here: a tree node).
pub struct Observer {
  dirty: AtomicBool,
}

impl Observer {
  pub fn new() -> Rc<Self> {
    Rc::new(Observer { dirty: AtomicBool::new(false) })
  }

  pub fn is_dirty(&self) -> bool {
    self.dirty.load(Ordering::Acquire)
  }
}

thread_local! {
  static CURRENT: RefCell<Option<Rc<Observer>>> = const { RefCell::new(None) };
}

/// Runs `f`; every `Signal::get` inside (on this thread) subscribes `observer`.
pub fn track<R>(observer: &Rc<Observer>, f: impl FnOnce() -> R) -> R {
  let previous = CURRENT.with(|c| c.replace(Some(observer.clone())));
  let out = f();
  CURRENT.with(|c| *c.borrow_mut() = previous);
  out
}

/// Runs `f` without subscribing anything to the signals it reads.
pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
  let previous = CURRENT.with(|c| c.replace(None));
  let out = f();
  CURRENT.with(|c| *c.borrow_mut() = previous);
  out
}

type Waker = Rc<dyn Fn()>;

thread_local! {
  static WAKER: RefCell<Option<Waker>> = RefCell::new(None);
}

/// Installs the function called (possibly from any thread) whenever a signal
/// change or `Repaint::mark` dirties the UI; the run loop uses it to schedule a redraw.
pub fn set_redraw_waker(waker: Option<Waker>) {
  WAKER.set(waker)
}

pub(crate) fn set_waker(waker: Option<Waker>) {
  set_redraw_waker(waker);
}

pub(crate) fn wake() {
  // Cloned out first so the waker may itself touch signals. Cloning (not
  // taking) keeps it installed for the next change.
  if let Some(waker) = WAKER.with(|w| w.borrow().clone()) {
    waker();
  }
}

struct Inner<T> {
  value: T,
  subscribers: Vec<rc::Weak<Observer>>,
}

/// Shared, observable value; `Send + Sync` when `T: Send`. Cloning gives
/// another handle to the same value.
pub struct Signal<T>(Rc<RefCell<Inner<T>>>);

impl<T> Clone for Signal<T> {
  fn clone(&self) -> Self {
    Signal(self.0.clone())
  }
}

impl<T: Default> Default for Signal<T> {
  fn default() -> Self {
    Signal::new(T::default())
  }
}

impl<T> Signal<T> {
  pub fn new(value: T) -> Self {
    Signal(Rc::new(RefCell::new(Inner { value, subscribers: Vec::new() })))
  }

  /// Reads without cloning; subscribes the current observer, if any.
  ///
  /// The signal is locked while `f` runs: don't touch this same signal
  /// from inside `f` (that would deadlock).
  pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    CURRENT.with(|c| {
      if let Some(observer) = c.borrow().as_ref() {
        let mut inner = self.0.borrow_mut();
        // Short mutable borrow, released before `f` runs.
        let known = inner.subscribers.iter().any(|w| w.as_ptr() == Rc::as_ptr(observer));
        if !known {
          inner.subscribers.push(Rc::downgrade(observer));
        }
      }
    });

    f(&self.0.borrow().value)
  }

  /// Like `with`, but does not subscribe. Reading with `peek_with` never causes a rebuild.
  pub fn peek_with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    f(&self.0.borrow().value)
  }
}

impl<T: Clone + PartialEq> Signal<T> {
  /// Replaces the value. Subscribers are only notified if it actually changed.
  pub fn set(&self, value: T) {
    let subscribers = {
      let mut inner = self.0.borrow_mut();
      if inner.value == value {
        return;
      }
      inner.value = value;
      // Dirty observers rebuild with a fresh observer and re-subscribe.
      std::mem::take(&mut inner.subscribers)
    };
    // Lock released: marking and waking never run under it.
    let mut any = false;
    for observer in subscribers.iter().filter_map(rc::Weak::upgrade) {
      observer.dirty.store(true, Ordering::Release);
      any = true;
    }
    if any {
      wake();
    }
  }
}

impl<T: Clone> Signal<T> {
  /// Clones the value and subscribes the current observer, if any.
  pub fn get(&self) -> T {
    self.with(T::clone)
  }

  /// Clones the value without subscribing. Use it for reads that must not cause a rebuild,
  /// such as reading a value inside an event handler.
  pub fn peek(&self) -> T {
    self.peek_with(T::clone)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn same_value_does_not_notify_subscribers() {
    let signal = Signal::new(1);
    let observer = Observer::new();
    track(&observer, || signal.get());

    signal.set(1);
    assert!(!observer.is_dirty());

    signal.set(2);
    assert!(observer.is_dirty());
  }
}

#[cfg(test)]
mod peek_tests {
  use super::*;

  #[test]
  fn peek_does_not_subscribe() {
    let signal = Signal::new(1);
    let observer = Observer::new();

    track(&observer, || signal.peek());
    signal.set(2);
    assert!(!observer.is_dirty());

    track(&observer, || signal.get());
    signal.set(3);
    assert!(observer.is_dirty());
  }
}
