//! Async work started from event handlers. Failures are reported, not dropped.
//!
//! # Ownership
//!
//! Every task (`spawn`, `run_async`, `Slot::restart`, `run_latest`) belongs to the
//! nearest `Provider` above the place it was started from: the widget whose event
//! handler, `build` or `attach` started it, the `Provider`'s own `create`, or, for a
//! task started inside another task, the same owner as that task. When the owning
//! `Provider` is unmounted, its tasks are aborted at their next `.await`, so they
//! can't write into a store nobody shows any more. A task with no `Provider` above
//! it (or started inside `detached`) runs to the end, as before.

use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use tokio::task::{AbortHandle, JoinHandle};

use crate::Signal;

thread_local! {
  static ERRORS: Signal<Vec<String>> = Signal::new(Vec::new());
}

/// The tasks owned by one `Provider`; aborted together when it is unmounted.
#[derive(Clone, Default)]
pub(crate) struct TaskGroup(Rc<RefCell<GroupInner>>);

#[derive(Default)]
struct GroupInner {
  handles: Vec<AbortHandle>,
  /// Set once aborted: a task started afterwards (by code still running) is aborted at once.
  closed: bool,
}

impl TaskGroup {
  fn add(&self, handle: AbortHandle) {
    let mut inner = self.0.borrow_mut();
    if inner.closed {
      drop(inner);
      handle.abort();
      return;
    }
    inner.handles.retain(|h| !h.is_finished());
    inner.handles.push(handle);
  }

  /// Aborts every task of the group, and every task started in it from now on.
  pub(crate) fn abort_all(&self) {
    let handles = {
      let mut inner = self.0.borrow_mut();
      inner.closed = true;
      std::mem::take(&mut inner.handles)
    };
    for handle in handles {
      handle.abort();
    }
  }
}

thread_local! {
  /// Owner for tasks started now on the UI thread (set while an event handler,
  /// `build` or `attach` runs). `None`: not set, look at the current task instead.
  static OWNER: RefCell<Option<Option<TaskGroup>>> = const { RefCell::new(None) };
}

tokio::task_local! {
  /// Owner of the running task, inherited by the tasks it starts.
  static TASK_OWNER: Option<TaskGroup>;
}

/// Runs `f` with `owner` as the owner of the tasks it starts.
pub(crate) fn with_owner<R>(owner: Option<TaskGroup>, f: impl FnOnce() -> R) -> R {
  let previous = OWNER.with(|o| o.replace(Some(owner)));
  let out = f();
  OWNER.with(|o| *o.borrow_mut() = previous);
  out
}

/// Runs `f` so that the tasks it starts have no owner: they run to the end even if
/// the widget or `Provider` that started them goes away (e.g. saving on exit).
///
/// ```ignore
/// detached(|| store.clone().save().run_async());
/// ```
pub fn detached<R>(f: impl FnOnce() -> R) -> R {
  with_owner(None, f)
}

fn current_owner() -> Option<TaskGroup> {
  match OWNER.with(|o| o.borrow().clone()) {
    Some(owner) => owner,
    None => TASK_OWNER.try_with(Clone::clone).ok().flatten(),
  }
}

/// `spawn_local`, owned by the current owner (see the module docs).
pub(crate) fn spawn_owned<F>(task: F) -> JoinHandle<F::Output>
where
  F: Future + 'static,
  F::Output: 'static,
{
  let owner = current_owner();
  let handle = tokio::task::spawn_local(TASK_OWNER.scope(owner.clone(), task));
  if let Some(owner) = owner {
    owner.add(handle.abort_handle());
  }
  handle
}

/// Every error reported by `spawn` and `Slot::restart`, as a signal.
/// Read it in `build` like any other signal.
pub fn errors() -> Signal<Vec<String>> {
  ERRORS.with(|e| e.clone())
}

/// Runs a fallible task on the UI thread. An `Err` or a panic is logged and
/// added to `errors()`, instead of being silently dropped.
pub fn spawn(task: impl Future<Output = anyhow::Result<()>> + 'static) {
  watch(spawn_owned(task));
}

/// Holds the task currently running for one kind of request.
/// `restart` aborts that task and starts the new one. An aborted task stops at
/// its next `.await`, so it never writes its stale result.
#[derive(Clone, Default)]
pub struct Slot(Rc<RefCell<Option<AbortHandle>>>);

impl Slot {
  pub fn restart(&self, task: impl Future<Output = anyhow::Result<()>> + 'static) {
    // Take the old handle out first so the borrow ends before `abort`.
    let previous = self.0.borrow_mut().take();
    if let Some(previous) = previous {
      previous.abort();
    }
    let handle = spawn_owned(task);
    *self.0.borrow_mut() = Some(handle.abort_handle());
    watch(handle);
  }
}

thread_local! {
  // One running task per (future type, optional key).
  static LATEST: RefCell<HashMap<(TypeId, Option<u64>), AbortHandle>> =
    RefCell::new(HashMap::new());
}

/// Latest-wins for async work, with no `Slot` to declare.
///
/// Starting a future aborts the previous one of the same type, so an old response
/// never reaches its writes. Every `async fn` has its own future type, so
/// `select_user(..)` and `toggle_currency(..)` already queue separately.
/// Use `run_latest_by` to split one function into several queues (one per key).
///
/// ```ignore
/// store.clone().select_user(user).run_latest();
/// store.clone().refresh_row(id).run_latest_by(id); // one queue per row
/// ```
pub trait RunLatest: Future<Output = anyhow::Result<()>> + Sized + 'static {
  fn run_latest(self) {
    start_latest::<Self>(self, None);
  }

  fn run_latest_by(self, key: impl Hash) {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    start_latest::<Self>(self, Some(hasher.finish()));
  }
}

impl<F: Future<Output = anyhow::Result<()>> + 'static> RunLatest for F {}

fn start_latest<F: Future<Output = anyhow::Result<()>> + 'static>(task: F, key: Option<u64>) {
  let handle = spawn_owned(task);
  let abort = handle.abort_handle();
  // The borrow ends with this statement, before `abort` runs.
  let previous = LATEST.with(|m| m.borrow_mut().insert((TypeId::of::<F>(), key), abort));
  if let Some(previous) = previous {
    previous.abort();
  }
  watch(handle);
}

/// Reports the outcome of a spawned task. A task cancelled by `Slot::restart`
/// or by its owner's unmount is expected, so it is not reported.
fn watch(handle: JoinHandle<anyhow::Result<()>>) {
  tokio::task::spawn_local(async move {
    let errors = errors();
    match handle.await {
      Ok(Ok(())) => {}
      Ok(Err(e)) => {
        let msg = format!("{e:#}");
        log::error!("{msg}");
        push_error(&errors, msg);
      }
      Err(join) if join.is_cancelled() => {}
      Err(join) => {
        let msg = format!("task panicked: {join}");
        log::error!("{msg}");
        push_error(&errors, msg);
      }
    }
  });
}

/// Latest-wins guard. `start` hands out a ticket; only the newest one is current,
/// so a slow older request can't overwrite a newer result.
#[derive(Clone, Default)]
pub struct Latest(Rc<Cell<u64>>);

#[derive(Clone)]
pub struct Ticket {
  latest: Rc<Cell<u64>>,
  id: u64,
}

impl Latest {
  pub fn start(&self) -> Ticket {
    let id = self.0.get() + 1;
    self.0.set(id);
    Ticket { latest: self.0.clone(), id }
  }
}

impl Ticket {
  pub fn is_current(&self) -> bool {
    self.latest.get() == self.id
  }
}

fn push_error(errors: &Signal<Vec<String>>, msg: String) {
  let mut list = errors.get();
  list.push(msg);
  errors.set(list);
}
