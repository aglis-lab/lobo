//! Abort-on-dispose: a task belongs to the nearest `Provider` above where it was
//! started, and is aborted when that `Provider` is unmounted.

use std::rc::Rc;
use std::time::Duration;

use lobo::*;

/// Runs `f` like `run` does: a current-thread runtime with a `LocalSet`.
fn on_ui_thread<F: std::future::Future>(f: F) -> F::Output {
  let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
  tokio::task::LocalSet::new().block_on(&rt, f)
}

/// Sets `done` after a short delay.
async fn finish_later(done: Signal<bool>) -> anyhow::Result<()> {
  tokio::time::sleep(Duration::from_millis(20)).await;
  done.set(true);
  Ok(())
}

/// Calls `start` when it builds (like a screen that loads on open), and also on any key.
#[derive(Widget)]
struct Starter {
  start: Rc<dyn Fn()>,
  on_build: bool,
}

impl Element for Starter {
  fn build(&self, _context: &Context) -> Option<Widget> {
    if self.on_build {
      (self.start)();
    }
    let start = self.start.clone();
    Some(ui!(InputEvent { event: move |_: &EventKey| start() }))
  }
}

/// `Starter` under a `Provider` while `shown` is true.
#[derive(Widget)]
struct Screen {
  shown: Signal<bool>,
  start: Rc<dyn Fn()>,
  on_build: bool,
  provider: bool,
}

impl Element for Screen {
  fn build(&self, _context: &Context) -> Option<Widget> {
    if !self.shown.get() {
      return Some(ui!(Text { text: "closed" }));
    }
    let start = self.start.clone();
    let starter = ui!(Starter { start: move || start(), on_build: self.on_build });
    Some(if self.provider { ui!(Provider { create: |_| (), child: starter }) } else { starter })
  }
}

struct Case {
  on_build: bool,
  provider: bool,
  close: bool,
}

/// Mounts a screen, starts the task (in `build` or from a key), optionally closes the
/// screen, waits, and returns whether the task finished.
fn finished(case: Case, start: fn(Signal<bool>)) -> bool {
  on_ui_thread(async move {
    global_events.clear();
    let done = Signal::new(false);
    let shown = Signal::new(true);
    let d = done.clone();
    let node = Node::mount(
      ui!(Screen {
        shown: shown.clone(),
        start: move || start(d.clone()),
        on_build: case.on_build,
        provider: case.provider,
      }),
      Context::default(),
    );
    if !case.on_build {
      dispatch_key_event('x');
    }
    tokio::task::yield_now().await;
    if case.close {
      shown.set(false);
      assert!(node.update());
    }
    tokio::time::sleep(Duration::from_millis(80)).await;
    done.peek()
  })
}

#[test]
fn a_task_started_in_build_is_aborted_when_its_provider_unmounts() {
  let start = |done| spawn(finish_later(done));
  assert!(finished(Case { on_build: true, provider: true, close: false }, start));
  assert!(!finished(Case { on_build: true, provider: true, close: true }, start));
}

#[test]
fn a_task_started_by_a_key_is_aborted_when_its_provider_unmounts() {
  let start = |done| {
    finish_later(done).run_async();
  };
  assert!(finished(Case { on_build: false, provider: true, close: false }, start));
  assert!(!finished(Case { on_build: false, provider: true, close: true }, start));
}

#[test]
fn slot_and_run_latest_tasks_are_owned_too() {
  let slot = |done| Slot::default().restart(finish_later(done));
  assert!(!finished(Case { on_build: false, provider: true, close: true }, slot));
  let latest = |done| finish_later(done).run_latest();
  assert!(!finished(Case { on_build: false, provider: true, close: true }, latest));
}

#[test]
fn a_task_started_by_a_task_has_the_same_owner() {
  // The outer task ends at once; the inner one is still owned by the provider.
  let start = |done: Signal<bool>| {
    spawn(async move {
      spawn(finish_later(done));
      Ok(())
    })
  };
  assert!(!finished(Case { on_build: false, provider: true, close: true }, start));
}

#[test]
fn detached_and_unowned_tasks_run_to_the_end() {
  let start = |done| detached(|| spawn(finish_later(done)));
  assert!(finished(Case { on_build: false, provider: true, close: true }, start));
  // No `Provider` above: nothing owns the task.
  let start = |done| spawn(finish_later(done));
  assert!(finished(Case { on_build: false, provider: false, close: true }, start));
}

#[test]
fn a_provider_rebuild_does_not_abort_its_tasks() {
  // Re-parenting or rebuilding keeps the node, so its tasks keep running.
  assert!(on_ui_thread(async {
    let done = Signal::new(false);
    let tick = Signal::new(0);

    #[derive(Widget)]
    struct Host {
      tick: Signal<i32>,
      done: Signal<bool>,
    }
    impl Element for Host {
      fn build(&self, _context: &Context) -> Option<Widget> {
        let n = self.tick.get();
        let done = self.done.clone();
        Some(ui!(Provider {
          create: move |_| spawn(finish_later(done.clone())),
          child: Text { text: n.to_string() },
        }))
      }
    }

    let node =
      Node::mount(ui!(Host { tick: tick.clone(), done: done.clone() }), Context::default());
    tokio::task::yield_now().await;
    tick.set(1);
    assert!(node.update());
    tokio::time::sleep(Duration::from_millis(80)).await;
    done.peek()
  }));
}
