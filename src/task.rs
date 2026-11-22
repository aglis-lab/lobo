//! Async work started from event handlers. Failures are reported, not dropped.

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

/// Every error reported by `spawn` and `Slot::restart`, as a signal.
/// Read it in `build` like any other signal.
pub fn errors() -> Signal<Vec<String>> {
    ERRORS.with(|e| e.clone())
}

/// Runs a fallible task on the UI thread. An `Err` or a panic is logged and
/// added to `errors()`, instead of being silently dropped.
pub fn spawn(task: impl Future<Output = anyhow::Result<()>> + 'static) {
    watch(tokio::task::spawn_local(task));
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
        let handle = tokio::task::spawn_local(task);
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
    let handle = tokio::task::spawn_local(task);
    let abort = handle.abort_handle();
    // The borrow ends with this statement, before `abort` runs.
    let previous = LATEST.with(|m| m.borrow_mut().insert((TypeId::of::<F>(), key), abort));
    if let Some(previous) = previous {
        previous.abort();
    }
    watch(handle);
}

/// Reports the outcome of a spawned task. A task cancelled by `Slot::restart`
/// is expected, so it is not reported.
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
        Ticket {
            latest: self.0.clone(),
            id,
        }
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
