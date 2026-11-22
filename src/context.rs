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

pub use lobo_macros::{Model, Widget, ui};

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
}

impl Context {
    pub fn new(inner: impl InnerContext + 'static) -> Self {
        Context {
            inner: Rc::new(inner),
            scope: None,
        }
    }

    /// Child context that also sees `values`; shadows the same types above.
    pub fn with_scope(&self, values: AnyMap) -> Context {
        Context {
            inner: self.inner.clone(),
            scope: Some(Rc::new(Scope {
                values: RefCell::new(values),
                parent: self.scope.clone(),
            })),
        }
    }

    /// Nearest provided `T` above this widget, if any.
    pub fn try_read<T: 'static>(&self) -> Option<Rc<T>> {
        let mut scope = self.scope.as_deref();
        while let Some(s) = scope {
            if let Some(value) = s.values.borrow_mut().get::<Rc<T>>() {
                return Some(value.clone());
            }
            scope = s.parent.as_deref();
        }
        None
    }

    /// Like `try_read`, but panics if no `Provider` of `T` is above.
    pub fn read<T: 'static>(&self) -> Rc<T> {
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

    pub fn set(&self, key: &str, value: impl ToString) {
        self.inner.set(key, value.to_string());
    }
}

/// Runs an `!Send` future on the UI thread, so it can read and write
/// `Signal`s directly — no handle, no channel, no `Send`.
///
/// The future must own what it uses (no borrows of temporaries), so take the
/// `Rc` from `context.read` and call a method that takes `self: Rc<Self>`:
///
/// ```ignore
/// context.read::<CounterController>().decrease().run_async();
/// ```
///
/// The future is polled by the run loop, so an `.await` inside it does not
/// block the UI. Panics if called outside `run` (there is no UI executor).
pub trait RunAsync: Future + Sized + 'static {
    fn run_async(self) -> tokio::task::JoinHandle<Self::Output>
    where
        Self::Output: 'static,
    {
        tokio::task::spawn_local(self)
    }
}

impl<F: Future + 'static> RunAsync for F {}

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
    let root = Node::mount(app, Context::default());

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
            // A signal changed outside of key handling (e.g. an async task).
            _ = redraw.notified() => {}
        }
        if root.update() {
            draw(&root);
        }
    }
}
