//! A small Flutter-style widget framework.
//!
//! Define widgets in your own crate:
//!
//! ```ignore
//! use widgets::{Context, Element, Widget, run, ui};
//!
//! #[derive(Widget)]
//! struct Hello {}
//!
//! impl Element for Hello {
//!     fn build(&self, _context: &Context) -> Option<Widget> {
//!         Some(ui!(Container(child: Text(text: "hi"))).into())
//!     }
//! }
//!
//! fn main() {
//!     run(ui!(Hello()));
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::io::BufRead;
use std::rc::Rc;

use anymap3::AnyMap;

use crate::signal::set_waker;
use crate::tree::Node;

pub use lobo_macros::{Widget, ui};

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// InnerContext is used to store values / act as a key-value storage.
pub trait InnerContext {
    fn get(&self, key: &str) -> Option<String>;
    /// Takes `&self` so widgets can update state while holding a `&Context`.
    fn set(&self, key: &str, value: String);
}

pub struct MapContext(pub RefCell<HashMap<String, String>>);

impl InnerContext for MapContext {
    fn get(&self, key: &str) -> Option<String> {
        self.0.borrow().get(key).cloned()
    }

    fn set(&self, key: &str, value: String) {
        self.0.borrow_mut().insert(key.to_string(), value);
    }
}

/// Values provided by one `Provider`, chained to the scope above it.
struct Scope {
    values: RefCell<AnyMap>,
    parent: Option<Rc<Scope>>,
}

/// Cheap to clone. Every node holds the context its widget was built in;
/// a `Provider` gives its subtree a child context with one more scope.
#[derive(Clone)]
pub struct Context {
    pub inner: Rc<dyn InnerContext>,
    scope: Option<Rc<Scope>>,
    dispatcher: Option<Dispatcher>,
}

impl Context {
    pub fn new(inner: impl InnerContext + 'static) -> Self {
        Context {
            inner: Rc::new(inner),
            scope: None,
            dispatcher: None,
        }
    }

    /// Handle for sending work back to the UI thread. It is `Send + Clone`,
    /// so take it here (on the UI thread) and move it into a task.
    ///
    /// Panics if the context was not created by `run`.
    pub fn dispatcher(&self) -> Dispatcher {
        self.dispatcher
            .clone()
            .expect("this Context has no dispatcher (it is created by `run`)")
    }

    /// Child context that also sees `values`; shadows the same types above.
    pub fn with_scope(&self, values: AnyMap) -> Context {
        Context {
            inner: self.inner.clone(),
            dispatcher: self.dispatcher.clone(),
            scope: Some(Rc::new(Scope {
                values: RefCell::new(values),
                parent: self.scope.clone(),
            })),
        }
    }

    /// Nearest provided `T` above this widget, if any.
    pub fn try_read<T: 'static>(&self) -> Option<Rc<RefCell<T>>> {
        let mut scope = self.scope.as_deref();
        while let Some(s) = scope {
            if let Some(value) = s.values.borrow_mut().get::<Rc<RefCell<T>>>() {
                return Some(value.clone());
            }
            scope = s.parent.as_deref();
        }
        None
    }

    /// Like `try_read`, but panics if no `Provider` of `T` is above.
    pub fn read<T: 'static>(&self) -> Rc<RefCell<T>> {
        self.try_read().unwrap_or_else(|| {
            panic!(
                "no Provider for `{}` above this widget",
                std::any::type_name::<T>()
            )
        })
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.inner.get(key)
    }

    /// Runs an `!Send` future on the UI thread, so it can read and write
    /// `Signal`s directly — no handle, no channel, no `Send`.
    ///
    /// The future is polled by the run loop, so an `.await` inside it does not
    /// block the UI: whatever it awaits (IO, `spawn_blocking`, a channel fed by
    /// another thread) runs elsewhere, and only `Send` data crosses back.
    ///
    /// ```ignore
    /// let counter = context.read::<CounterState>();
    /// context.spawn_local(async move {
    ///     let data = fetch().await;              // on a worker thread
    ///     counter.count.set(data);               // back on the UI thread
    /// });
    /// ```
    ///
    /// Panics if called outside `run` (there is no UI executor then).
    pub fn run_async<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        tokio::task::spawn_local(future)
    }

    pub fn set(&self, key: &str, value: impl ToString) {
        self.inner.set(key, value.to_string());
    }
}

impl Default for Context {
    fn default() -> Self {
        Context::new(MapContext(RefCell::new(HashMap::new())))
    }
}

// ---------------------------------------------------------------------------
// Core traits
// ---------------------------------------------------------------------------

pub trait Element {
    /// Composite widgets expand into other widgets (like Flutter's `build`).
    /// Primitive widgets return `None` and draw themselves in `paint`.
    fn build(&self, _context: &Context) -> Option<Widget> {
        None
    }

    /// Child widgets of a primitive (composites use `build` instead).
    /// The framework mounts, rebuilds and paints them.
    fn children(&self) -> Vec<Widget> {
        Vec::new()
    }

    /// One-line description of a primitive in the printed tree.
    fn describe(&self) -> String {
        self.name().to_string()
    }

    /// Context for this widget's descendants; `None` passes the parent's
    /// through. Used by `Provider`. Runs under the same tracking as `build`.
    fn scope(&self, _context: &Context) -> Option<Context> {
        None
    }

    /// Called for every key press, on every mounted widget.
    fn on_key(&self, _context: &Context, _key: char) {}

    /// Name shown in the printed tree for composite widgets. Defaults to the
    /// struct name (`widgets::Hello` -> `Hello`); override to customise.
    fn name(&self) -> &'static str {
        let full = std::any::type_name::<Self>();
        let base = full.split('<').next().unwrap_or(full);
        base.rsplit("::").next().unwrap_or(base)
    }
}

/// Cheaply clonable handle to any element. A newtype (instead of a bare
/// `Rc<dyn Element>` alias) so `From<E: Element>` is allowed by orphan rules.
#[derive(Clone)]
pub struct Widget(pub(crate) Rc<dyn Element>);

impl<E: Element + 'static> From<E> for Widget {
    fn from(element: E) -> Self {
        Widget(Rc::new(element))
    }
}

// ---------------------------------------------------------------------------
// UI-thread dispatch
// ---------------------------------------------------------------------------

type UiTask = Box<dyn FnOnce(&Ui) + Send>;

/// Sends closures to the UI thread, from any thread or task.
#[derive(Clone)]
pub struct Dispatcher {
    tx: tokio::sync::mpsc::UnboundedSender<UiTask>,
    /// The thread that created this dispatcher, i.e. the UI thread.
    ui_thread: std::thread::ThreadId,
}

impl Dispatcher {
    fn new(tx: tokio::sync::mpsc::UnboundedSender<UiTask>) -> Self {
        Dispatcher {
            tx,
            ui_thread: std::thread::current().id(),
        }
    }

    /// Queues `f` to run on the UI thread. Returns `false` if the app has
    /// already quit. After it runs, changed signals trigger a rebuild.
    pub fn run(&self, f: impl FnOnce(&Ui) + Send + 'static) -> bool {
        self.tx.send(Box::new(f)).is_ok()
    }

    /// Whether the caller is already on the UI thread (where `run` would
    /// queue work behind the code that is running right now).
    pub fn is_ui_thread(&self) -> bool {
        std::thread::current().id() == self.ui_thread
    }
}

/// What a dispatched closure can reach on the UI thread.
pub struct Ui<'a> {
    root: &'a Node,
}

impl Ui<'_> {
    /// Delivers a key press to every widget, as if typed.
    pub fn key(&self, key: char) {
        self.root.dispatch_key(key);
    }
}

fn draw(root: &Node) {
    let mut out = String::new();
    root.paint(0, &mut out);
    // No screen clear, so trace logs from `build` stay visible between frames.
    print!("{out}[+] increment  [-] decrement  [q] quit (press Enter after each)\n---\n");
}

/// Mounts the tree once, then loops on this thread, with a multi-threaded tokio runtime:
/// a stdin line is delivered as key presses, and any signal change (from a
/// key press or from an async task) wakes the loop to rebuild only the
/// nodes that read it, then repaint. `q` or EOF quits.
///
/// The widget tree (`build`, `on_key`, painting) stays on the calling thread.
/// Async work goes to the runtime's worker threads with `tokio::spawn`; tasks
/// and plain threads may set `Signal`s, which wakes this loop.
///
/// Input is line-buffered (no raw terminal mode), so keys take effect on Enter.
pub fn run(app: impl Into<Widget>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let local = tokio::task::LocalSet::new();
    runtime.block_on(local.run_until(run_loop(app.into())));
    set_waker(None);
}

async fn run_loop(app: Widget) {
    let (ui_tx, mut ui_tasks) = tokio::sync::mpsc::unbounded_channel::<UiTask>();
    let mut context = Context::default();
    context.dispatcher = Some(Dispatcher::new(ui_tx));
    let root = Node::mount(app, context);

    let redraw = Rc::new(tokio::sync::Notify::new());
    {
        let redraw = redraw.clone();
        set_waker(Some(Rc::new(move || redraw.notify_one())));
    }

    // Blocking stdin lives on its own thread; lines arrive over a channel.
    let (tx, mut lines) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    draw(&root);
    loop {
        tokio::select! {
            line = lines.recv() => {
                let Some(line) = line else { return };
                for key in line.chars() {
                    if key == 'q' {
                        return;
                    }
                    root.dispatch_key(key);
                }
            }
            // Work sent from another thread via `Dispatcher::run`.
            task = ui_tasks.recv() => {
                if let Some(task) = task {
                    task(&Ui { root: &root });
                }
            }
            // A signal changed outside of key handling (e.g. an async task).
            _ = redraw.notified() => {}
        }
        if root.update() {
            draw(&root);
        }
    }
}
