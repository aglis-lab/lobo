//! `Provider` makes a value available to its whole subtree, looked up by type
//! with `context.read::<T>()`.
//!
//! ```ignore
//! ui!(Provider {
//!     create: |_| CounterState { count: Signal::new(0) },
//!     child: Hello {},
//! })
//! // anywhere below:
//! let state = context.read::<CounterState>();
//! ```

use std::any::TypeId;
use std::cell::RefCell;
use std::rc::Rc;

use anymap3::AnyMap;

use crate::signal::untracked;
use crate::task::{TaskGroup, with_owner};
use crate::{Context, Element, NodeCx, Widget};

/// Type-erased `create` function: builds a value and stores it in a scope.
/// Made from any `Fn(Context) -> T`, so `T` is picked by the closure.
///
/// Takes `Context` by value (a cheap clone) rather than `&Context`: a closure
/// like `|_| ...` can't be inferred to accept a reference for any lifetime.
#[derive(Clone)]
pub struct ProviderCreate {
  make: Rc<dyn Fn(Context, &mut AnyMap)>,
  /// The provided type, so a `Provider` of another type at the same place
  /// creates its own value instead of reusing the old one.
  provides: TypeId,
}

impl<T, F> From<F> for ProviderCreate
where
  T: 'static,
  F: Fn(Context) -> T + 'static,
{
  fn from(create: F) -> Self {
    ProviderCreate {
      make: Rc::new(move |context, values| {
        // Stored as `Rc<T>`, the type `Context::read` looks up.
        values.insert(Rc::new(create(context)));
      }),
      provides: TypeId::of::<T>(),
    }
  }
}

#[derive(Widget)]
#[non_exhaustive]
pub struct Provider {
  /// Called once, when the provider is mounted, with the parent's context.
  /// The value lives as long as the provider's node: rebuilds and in-place updates
  /// keep it (like Flutter's `Provider(create:)`). Signals read here don't make the
  /// provider rebuild. To get a fresh value, give the provider a new key
  /// (`Widget::with_key`).
  ///
  /// Tasks started below the provider (and in `create`) belong to it: they are
  /// aborted when it is unmounted (see `detached` for work that must finish).
  pub create: ProviderCreate,
  pub child: Widget,
}

/// What a mounted `Provider` keeps on its node.
#[derive(Default)]
struct ProviderState {
  provides: Option<TypeId>,
  values: Option<Rc<RefCell<AnyMap>>>,
  tasks: TaskGroup,
  /// The last parent context and the child context made from it.
  scope: Option<(Context, Context)>,
}

impl Element for Provider {
  fn build(&self, _context: &Context) -> Option<Widget> {
    Some(self.child.clone())
  }

  fn scope(&self, cx: &NodeCx) -> Option<Context> {
    let state = cx.state::<ProviderState>();
    let mut state = state.borrow_mut();
    let parent = cx.context();
    if state.provides != Some(self.create.provides) {
      // A provider of another type took this node's place: the old value's tasks end with it.
      state.tasks.abort_all();
      *state = ProviderState { provides: Some(self.create.provides), ..ProviderState::default() };
    }
    if let Some((last_parent, child)) = &state.scope
      && last_parent.same(parent)
    {
      // Same context as before: the subtree keeps its context (and isn't rebuilt for it).
      return Some(child.clone());
    }
    let tasks = state.tasks.clone();
    let values = state
      .values
      .get_or_insert_with(|| {
        let mut values = AnyMap::new();
        // Tasks `create` starts (e.g. a first load) belong to this provider.
        with_owner(Some(tasks.clone()), || {
          untracked(|| (self.create.make)(parent.clone(), &mut values))
        });
        Rc::new(RefCell::new(values))
      })
      .clone();
    let child = parent.with_shared_scope(values, Some(tasks));
    state.scope = Some((parent.clone(), child.clone()));
    Some(child)
  }

  fn dispose(&self, cx: &NodeCx) {
    cx.state::<ProviderState>().borrow().tasks.abort_all();
  }
}
