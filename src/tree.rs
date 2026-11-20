//! Retained tree: one `Node` per mounted widget. A node caches what its
//! widget built, and rebuilds only when a signal it read has changed.

use std::cell::{Cell, RefCell};
use std::fmt::Write;
use std::rc::Rc;
use std::sync::Arc;

use crate::signal::{Observer, track};
use crate::{Context, Widget};

pub struct Node {
    widget: Widget,
    /// The context this node's widget is built in (set by its parent).
    context: Context,
    /// Replaced on every rebuild, which drops stale subscriptions.
    observer: RefCell<Arc<Observer>>,
    composite: Cell<bool>,
    kids: RefCell<Vec<Rc<Node>>>,
}

impl Node {
    pub fn mount(widget: Widget, context: Context) -> Rc<Node> {
        let node = Rc::new(Node {
            widget,
            context,
            observer: RefCell::new(Observer::new()),
            composite: Cell::new(false),
            kids: RefCell::new(Vec::new()),
        });
        node.rebuild();
        node
    }

    fn rebuild(&self) {
        let observer = Observer::new();
        *self.observer.borrow_mut() = observer.clone();

        let (built, scope) = track(&observer, || {
            (
                self.widget.0.build(&self.context),
                self.widget.0.scope(&self.context),
            )
        });
        self.composite.set(built.is_some());
        let widgets = match built {
            Some(w) => vec![w],
            None => self.widget.0.children(),
        };
        let child_context = scope.unwrap_or_else(|| self.context.clone());
        // Mounted outside `track`: children subscribe themselves, not us.
        let kids = widgets
            .into_iter()
            .map(|w| Node::mount(w, child_context.clone()))
            .collect();
        *self.kids.borrow_mut() = kids;
    }

    /// Rebuilds the topmost dirty nodes; clean nodes are only walked.
    /// Returns whether anything was rebuilt (i.e. whether to repaint).
    pub fn update(&self) -> bool {
        if self.observer.borrow().is_dirty() {
            self.rebuild();
            return true;
        }
        let kids = self.kids.borrow().clone();
        let mut rebuilt = false;
        for kid in kids {
            rebuilt |= kid.update();
        }
        rebuilt
    }

    pub fn dispatch_key(&self, key: char) {
        self.widget.0.on_key(&self.context, key);
        let kids = self.kids.borrow().clone();
        for kid in kids {
            kid.dispatch_key(key);
        }
    }

    /// Prints the cached tree; never calls `build`.
    pub fn paint(&self, depth: usize, out: &mut String) {
        let label = if self.composite.get() {
            format!("{}()", self.widget.0.name())
        } else {
            self.widget.0.describe()
        };
        writeln!(out, "{}{}", "  ".repeat(depth), label).unwrap();
        for kid in self.kids.borrow().iter() {
            kid.paint(depth + 1, out);
        }
    }
}
