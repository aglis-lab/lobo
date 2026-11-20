//! `Provider` makes a value available to its whole subtree, looked up by type
//! with `context.read::<T>()`.
//!
//! ```ignore
//! ui!(Provider(
//!     create: |_| CounterState { count: Signal::new(0) },
//!     child: Hello(),
//! ))
//! // anywhere below:
//! let state = context.read::<CounterState>();
//! ```

use std::rc::Rc;
use std::sync::Arc;

use anymap3::AnyMap;

use crate::{Context, Element, Widget};

/// Type-erased `create` function: builds a value and stores it in a scope.
/// Made from any `Fn(&Context) -> T`, so `T` is picked by the closure.
#[derive(Clone)]
///
/// Takes `Context` by value (a cheap clone) rather than `&Context`: a closure
/// like `|_| ...` can't be inferred to accept a reference for any lifetime.
pub struct ProviderCreate(Rc<dyn Fn(Context, &mut AnyMap)>);

impl<T, F> From<F> for ProviderCreate
where
    T: 'static,
    F: Fn(Context) -> T + 'static,
{
    fn from(create: F) -> Self {
        ProviderCreate(Rc::new(move |context, values| {
            // Stored as `Arc<T>` so `read` hands out handles that tasks on other
            // threads can hold (when `T: Send + Sync`).
            values.insert(Arc::new(create(context)));
        }))
    }
}

#[derive(Widget)]
pub struct Provider {
    /// Called with the parent's context each time the provider is (re)built,
    /// so the value is recreated if the provider itself rebuilds.
    pub create: ProviderCreate,
    pub child: Widget,
}

impl Element for Provider {
    fn build(&self, _context: &Context) -> Option<Widget> {
        Some(self.child.clone())
    }

    fn scope(&self, context: &Context) -> Option<Context> {
        let mut values = AnyMap::new();
        (self.create.0)(context.clone(), &mut values);
        Some(context.with_scope(values))
    }
}
