//! In-place updates: when a parent rebuilds, its children's nodes are reused
//! (matched by key, else by position and type), so they keep state and focus.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lobo::*;

/// Draws nothing; `present` with it still lays out and registers focus.
struct Null;

impl Renderer for Null {
  fn size(&self) -> Size {
    HD_720
  }
  fn measure_text(&self, text: &str, paint: &TextPaint) -> Size {
    Size::new(text.chars().count() as f32 * paint.size * 0.5, paint.size)
  }
  fn clear(&mut self, _: Color) {}
  fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
  fn stroke_rect(&mut self, _: Rect, _: Border, _: f32) {}
  fn draw_line(&mut self, _: Offset, _: Offset, _: Color, _: f32) {}
  fn fill_circle(&mut self, _: Offset, _: f32, _: Color) {}
  fn stroke_circle(&mut self, _: Offset, _: f32, _: Border) {}
  fn draw_text(&mut self, _: Offset, _: &str, _: &TextPaint) {}
  fn push_clip(&mut self, _: Rect) {}
  fn pop_clip(&mut self) {}
}

/// Records its lifecycle, and how many times it was built, in shared logs.
#[derive(Widget)]
struct Probe {
  label: String,
  log: Rc<RefCell<Vec<String>>>,
}

/// Counts attaches on the node; `mounts` is how many distinct nodes were created.
#[derive(Default)]
struct ProbeState {
  attaches: u32,
}

impl Element for Probe {
  fn attach(&self, cx: &NodeCx) {
    let state = cx.state::<ProbeState>();
    let mut state = state.borrow_mut();
    state.attaches += 1;
    let what = if state.attaches == 1 { "mount" } else { "update" };
    self.log.borrow_mut().push(format!("{what} {}", self.label));
  }

  fn dispose(&self, _cx: &NodeCx) {
    self.log.borrow_mut().push(format!("dispose {}", self.label));
  }

  fn build(&self, _context: &Context) -> Option<Widget> {
    self.log.borrow_mut().push(format!("build {}", self.label));
    Some(ui!(Text { text: self.label.clone() }))
  }
}

/// Builds `items` as keyed or unkeyed `Probe`s from a signal.
#[derive(Widget)]
struct ProbeList {
  items: Signal<Vec<&'static str>>,
  keyed: bool,
  log: Rc<RefCell<Vec<String>>>,
}

impl Element for ProbeList {
  fn build(&self, _context: &Context) -> Option<Widget> {
    let children: Vec<Widget> = self
      .items
      .get()
      .into_iter()
      .map(|label| {
        let w = ui!(Probe { label, log: self.log.clone() });
        if self.keyed { w.with_key(label) } else { w }
      })
      .collect();
    Some(ui!(Column { children }))
  }
}

fn take(log: &Rc<RefCell<Vec<String>>>) -> Vec<String> {
  std::mem::take(&mut *log.borrow_mut())
}

#[test]
fn a_parent_rebuild_updates_children_in_place() {
  let log = Rc::new(RefCell::new(Vec::new()));
  let items = Signal::new(vec!["a", "b"]);
  let node = Node::mount(
    ui!(ProbeList { items: items.clone(), keyed: false, log: log.clone() }),
    Context::default(),
  );
  assert_eq!(take(&log), ["mount a", "build a", "mount b", "build b"]);

  items.set(vec!["a", "c"]);
  assert!(node.update());
  // Same type at the same position: updated (and rebuilt with the new props), never remounted.
  assert_eq!(take(&log), ["update a", "build a", "update c", "build c"]);

  items.set(vec!["a"]);
  assert!(node.update());
  assert_eq!(take(&log), ["dispose c", "update a", "build a"]);

  drop(node);
  assert_eq!(take(&log), ["dispose a"]);
}

#[test]
fn keyed_children_follow_their_key() {
  let log = Rc::new(RefCell::new(Vec::new()));
  let items = Signal::new(vec!["a", "b", "c"]);
  let node = Node::mount(
    ui!(ProbeList { items: items.clone(), keyed: true, log: log.clone() }),
    Context::default(),
  );
  take(&log);

  // Reordered and one removed: every surviving item keeps its own node.
  items.set(vec!["c", "a"]);
  assert!(node.update());
  assert_eq!(take(&log), ["dispose b", "update c", "build c", "update a", "build a"]);

  // A new key gets a new node even at a reused position.
  items.set(vec!["d", "a"]);
  assert!(node.update());
  assert_eq!(take(&log), ["dispose c", "mount d", "build d", "update a", "build a"]);
}

#[test]
fn a_different_type_at_the_same_position_remounts() {
  let log = Rc::new(RefCell::new(Vec::new()));
  let probe = Signal::new(true);

  #[derive(Widget)]
  struct Switcher {
    probe: Signal<bool>,
    log: Rc<RefCell<Vec<String>>>,
  }
  impl Element for Switcher {
    fn build(&self, _context: &Context) -> Option<Widget> {
      Some(if self.probe.get() {
        ui!(Probe { label: "p", log: self.log.clone() })
      } else {
        ui!(Text { text: "t" })
      })
    }
  }

  let node =
    Node::mount(ui!(Switcher { probe: probe.clone(), log: log.clone() }), Context::default());
  take(&log);
  probe.set(false);
  assert!(node.update());
  assert_eq!(take(&log), ["dispose p"]);
}

#[test]
fn an_unchanged_child_widget_is_not_rebuilt() {
  // `Provider` builds `self.child.clone()`: the very same widget value, so a rebuild of
  // the provider (here: an outer rebuild that keeps the same child) leaves it alone.
  let log = Rc::new(RefCell::new(Vec::new()));
  let tick = Signal::new(0);
  let child = ui!(Probe { label: "same", log: log.clone() });

  #[derive(Widget)]
  struct Outer {
    tick: Signal<i32>,
    child: Widget,
  }
  impl Element for Outer {
    fn build(&self, _context: &Context) -> Option<Widget> {
      self.tick.get();
      Some(self.child.clone())
    }
  }

  let node = Node::mount(ui!(Outer { tick: tick.clone(), child }), Context::default());
  take(&log);
  tick.set(1);
  assert!(node.update());
  assert!(take(&log).is_empty(), "the same child widget in the same context is kept as is");
}

#[test]
fn focus_and_listener_order_survive_a_parent_rebuild() {
  global_events.clear();
  let pressed = Rc::new(RefCell::new(Vec::new()));
  let round = Signal::new(0);

  #[derive(Widget)]
  struct Buttons {
    round: Signal<i32>,
    pressed: Rc<RefCell<Vec<String>>>,
  }
  impl Element for Buttons {
    fn build(&self, _context: &Context) -> Option<Widget> {
      let round = self.round.get();
      let button = |name: &'static str| {
        let pressed = self.pressed.clone();
        ui!(TextButton {
          on_pressed: move || pressed.borrow_mut().push(format!("{name}{round}")),
          child: Text { text: name },
        })
      };
      Some(ui!(Column { children: [button("x"), button("y")] }))
    }
  }

  let node = Node::mount(
    ui!(Buttons { round: round.clone(), pressed: pressed.clone() }),
    Context::default(),
  );
  let mut r = Null;
  let mut scene = Scene::new();
  node.present(&mut scene, &mut r);
  let listeners = global_events.count();
  focus::set(Some(1));
  let focused = global_events.current_focus();

  round.set(1);
  assert!(node.update());
  // Same listeners, same focus id: nothing was re-registered.
  assert_eq!(global_events.count(), listeners);
  assert_eq!(global_events.current_focus(), focused);
  node.present(&mut scene, &mut r);
  assert_eq!(focus::current(), Some(1));

  // The kept listener calls the new widget's callback (it sees `round` = 1).
  dispatch_key_event(keys::ENTER);
  assert_eq!(*pressed.borrow(), ["y1"]);
  focus::set(None);
}

#[test]
fn a_provider_keeps_its_value_when_its_parent_rebuilds() {
  let created = Rc::new(Cell::new(0));
  let tick = Signal::new(0);
  let seen = Rc::new(RefCell::new(Vec::new()));

  struct Store {
    id: u32,
  }

  #[derive(Widget)]
  struct Reader {
    seen: Rc<RefCell<Vec<u32>>>,
  }
  impl Element for Reader {
    fn build(&self, context: &Context) -> Option<Widget> {
      self.seen.borrow_mut().push(context.read::<Store>().id);
      None
    }
  }

  #[derive(Widget)]
  struct Host {
    tick: Signal<i32>,
    created: Rc<Cell<u32>>,
    seen: Rc<RefCell<Vec<u32>>>,
  }
  impl Element for Host {
    fn build(&self, _context: &Context) -> Option<Widget> {
      let n = self.tick.get();
      let created = self.created.clone();
      Some(ui!(Provider {
        create: move |_| {
          created.set(created.get() + 1);
          Store { id: created.get() }
        },
        // A new `Reader` value each time, so it is rebuilt and reads the store again.
        child: Column {
          children: [Reader { seen: self.seen.clone() }, Text { text: n.to_string() }]
        },
      }))
    }
  }

  let node = Node::mount(
    ui!(Host { tick: tick.clone(), created: created.clone(), seen: seen.clone() }),
    Context::default(),
  );
  tick.set(1);
  assert!(node.update());
  tick.set(2);
  assert!(node.update());
  assert_eq!(created.get(), 1, "`create` runs once per provider node");
  assert_eq!(*seen.borrow(), [1, 1, 1]);
}
