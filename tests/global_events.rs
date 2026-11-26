use lobo::*;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn global_events_direct_push_and_dispatch() {
  global_events.clear();
  assert_eq!(global_events.count(), 0);

  let received = Rc::new(RefCell::new(Vec::new()));
  let rx = received.clone();

  let callback: EventCallback = Rc::new(move |event: &EventKey| {
    rx.borrow_mut().push(event.key);
  });

  global_events.push(&callback);
  assert_eq!(global_events.count(), 1);

  global_events.dispatch_key('a');
  global_events.dispatch_key('b');
  assert_eq!(*received.borrow(), vec!['a', 'b']);

  global_events.remove(&callback);
  assert_eq!(global_events.count(), 0);

  global_events.dispatch_key('c');
  assert_eq!(*received.borrow(), vec!['a', 'b']);
}

#[test]
fn input_event_widget_lifecycle_mount_and_dispose() {
  global_events.clear();
  assert_eq!(global_events.count(), 0);

  let received = Rc::new(RefCell::new(Vec::new()));
  let rx = received.clone();

  let widget: Widget = ui!(InputEvent {
    event: move |event: &EventKey| {
      rx.borrow_mut().push(event.key);
    },
    child: Text { text: "content" },
  });

  // Before mounting, no listeners registered
  assert_eq!(global_events.count(), 0);

  // Mount into tree node
  let node = Node::mount(widget, Context::default());
  assert_eq!(global_events.count(), 1);

  global_events.dispatch_key('+');
  global_events.dispatch_key('-');
  assert_eq!(*received.borrow(), vec!['+', '-']);

  // Dropping the root node triggers dispose() on InputEvent
  drop(node);
  assert_eq!(global_events.count(), 0);

  // After dropping, no events received
  global_events.dispatch_key('x');
  assert_eq!(*received.borrow(), vec!['+', '-']);
}

#[test]
fn input_event_without_child() {
  global_events.clear();

  let received = Rc::new(RefCell::new(Vec::new()));
  let rx = received.clone();

  let widget: Widget = ui!(InputEvent {
    event: move |event: &EventKey| {
      rx.borrow_mut().push(event.key);
    },
  });

  let node = Node::mount(widget, Context::default());
  assert_eq!(global_events.count(), 1);

  global_events.dispatch_key('z');
  assert_eq!(*received.borrow(), vec!['z']);

  drop(node);
  assert_eq!(global_events.count(), 0);
}

#[test]
fn input_event_updates_in_place_on_parent_rebuild() {
  global_events.clear();

  let flag = Signal::new(0);
  let flag_clone = flag.clone();

  let received = Rc::new(RefCell::new(Vec::new()));
  let rx = received.clone();

  #[derive(Widget)]
  struct DynamicTree {
    flag: Signal<i32>,
    rx: Rc<RefCell<Vec<String>>>,
  }

  impl Element for DynamicTree {
    fn build(&self, _context: &Context) -> Option<Widget> {
      let current = self.flag.get();
      let rx = self.rx.clone();
      Some(ui!(InputEvent {
        event: move |event: &EventKey| {
          rx.borrow_mut().push(format!("{}:{}", current, event.key));
        },
        child: Text { text: format!("v{current}") },
      }))
    }
  }

  let app: Widget = ui!(DynamicTree { flag: flag_clone, rx: rx.clone() });

  let node = Node::mount(app, Context::default());
  // Initial mount: 1 listener
  assert_eq!(global_events.count(), 1);

  global_events.dispatch_key('1');
  assert_eq!(*received.borrow(), vec!["0:1"]);

  // Update the signal, triggering rebuild of DynamicTree
  flag.set(1);
  assert!(node.update());

  // Updated in place: the same listener now calls the new callback -> still exactly 1 listener
  assert_eq!(global_events.count(), 1);

  global_events.dispatch_key('2');
  assert_eq!(*received.borrow(), vec!["0:1", "1:2"]);

  drop(node);
  assert_eq!(global_events.count(), 0);
}

#[test]
fn unmounted_input_event_widget_drop_is_safe_and_side_effect_free() {
  global_events.clear();
  assert_eq!(global_events.count(), 0);

  // Creating and dropping a widget blueprint without mounting has no side effects.
  {
    let _widget = ui!(InputEvent { event: |_: &EventKey| {}, child: Text { text: "blueprint" } });
  }
  assert_eq!(global_events.count(), 0);
}

#[test]
fn event_bubbling_and_handling_stops_propagation() {
  global_events.clear();

  let outer_seen = Rc::new(RefCell::new(false));
  let inner_seen = Rc::new(RefCell::new(false));

  let outer_flag = outer_seen.clone();
  let inner_flag = inner_seen.clone();

  // Outer widget wraps Inner widget
  let tree: Widget = ui!(InputEvent {
    event: move |event: &EventKey| {
      if event.key == 'q' {
        *outer_flag.borrow_mut() = true;
      }
    },
    child: InputEvent {
      event: move |event: &EventKey| {
        if event.key == 'q' {
          *inner_flag.borrow_mut() = true;
          // Inner handles the event, consuming it like in Flutter
          event.handle();
        }
      },
      child: Text { text: "content" },
    },
  });

  let node = Node::mount(tree, Context::default());
  assert_eq!(global_events.count(), 2);

  let result = global_events.dispatch_key('q');
  assert_eq!(result, EventResult::Handled);

  // Inner saw and handled it
  assert!(*inner_seen.borrow());
  // Outer was shielded because inner consumed the event
  assert!(!*outer_seen.borrow());

  drop(node);
  assert_eq!(global_events.count(), 0);
}

#[test]
fn focus_management_routes_events_to_focused_widget() {
  global_events.clear();

  let input1_log = Rc::new(RefCell::new(Vec::new()));
  let input2_log = Rc::new(RefCell::new(Vec::new()));

  let l1 = input1_log.clone();
  let l2 = input2_log.clone();

  let tree: Widget = ui!(Column {
    children: [
      InputEvent {
        focus_id: "input_1".to_string(),
        event: move |e: &EventKey| {
          l1.borrow_mut().push(e.key);
        },
        child: Text { text: "Field 1" },
      },
      InputEvent {
        focus_id: "input_2".to_string(),
        event: move |e: &EventKey| {
          l2.borrow_mut().push(e.key);
        },
        child: Text { text: "Field 2" },
      },
    ],
  });

  let node = Node::mount(tree, Context::default());
  assert_eq!(global_events.count(), 2);

  // With no active focus, neither focused input receives the event
  global_events.dispatch_key('a');
  assert!(input1_log.borrow().is_empty());
  assert!(input2_log.borrow().is_empty());

  // Focus input_1
  global_events.set_focus(Some("input_1"));
  assert!(global_events.is_focused("input_1"));
  global_events.dispatch_key('b');
  assert_eq!(*input1_log.borrow(), vec!['b']);
  assert!(input2_log.borrow().is_empty());

  // Switch focus to input_2
  global_events.set_focus(Some("input_2"));
  assert!(global_events.is_focused("input_2"));
  global_events.dispatch_key('c');
  assert_eq!(*input1_log.borrow(), vec!['b']);
  assert_eq!(*input2_log.borrow(), vec!['c']);

  drop(node);
  assert_eq!(global_events.count(), 0);
  global_events.set_focus(None::<String>);
  assert_eq!(global_events.current_focus(), None);
}

#[test]
fn modifiers_support() {
  let key = EventKey::with_modifiers('c', Modifiers::CTRL);
  assert!(key.is_ctrl());
  assert!(!key.is_alt());
  assert!(!key.is_shift());
  assert_eq!(key.key, 'c');
  assert!(!key.is_char('c')); // false because modifiers are present
}

#[test]
fn q_key_is_dispatched_as_normal_event() {
  global_events.clear();

  let seen = Rc::new(RefCell::new(Vec::new()));
  let seen_flag = seen.clone();

  let tree: Widget = ui!(InputEvent {
    event: move |e: &EventKey| {
      seen_flag.borrow_mut().push(e.key);
    },
    child: Text { text: "text" },
  });

  let node = Node::mount(tree, Context::default());
  global_events.dispatch_key('q');
  global_events.dispatch_key('u');
  global_events.dispatch_key('i');
  global_events.dispatch_key('t');

  assert_eq!(*seen.borrow(), vec!['q', 'u', 'i', 't']);
  drop(node);
}
