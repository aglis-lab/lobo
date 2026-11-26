use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lobo::*;

struct Spy {
  size: Size,
  batches: Vec<(Rect, Vec<DrawCmd>)>,
}

impl Spy {
  fn new(width: f32, height: f32) -> Self {
    Spy { size: Size::new(width, height), batches: Vec::new() }
  }

  fn take(&mut self) -> Vec<(Rect, Vec<DrawCmd>)> {
    std::mem::take(&mut self.batches)
  }
}

impl Renderer for Spy {
  fn size(&self) -> Size {
    self.size
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

  fn draw_batch(&mut self, damage: Rect, cmds: &[DrawCmd]) {
    self.batches.push((damage, cmds.to_vec()));
  }
}

#[derive(Widget)]
struct CounterProbe {
  builds: Rc<Cell<u32>>,
  layouts: Rc<Cell<u32>>,
  paints: Rc<Cell<u32>>,
}

impl Element for CounterProbe {
  fn build(&self, _: &Context) -> Option<Widget> {
    self.builds.set(self.builds.get() + 1);
    None
  }

  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    self.layouts.set(self.layouts.get() + 1);
    constraints.constrain(Size::new(40.0, 20.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    self.paints.set(self.paints.get() + 1);
    cx.renderer.fill_rect(cx.rect(), Color::BLUE, 0.0);
  }
}

#[test]
fn repaint_mark_rerecords_only_marked_node_without_build_or_layout() {
  #[derive(Widget)]
  struct Blinker {
    on: Rc<Cell<bool>>,
    handle: Rc<RefCell<Option<Repaint>>>,
    paints: Rc<Cell<u32>>,
  }

  impl Element for Blinker {
    fn attach(&self, cx: &NodeCx) {
      *self.handle.borrow_mut() = Some(cx.repaint());
    }

    fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
      constraints.constrain(Size::new(10.0, 20.0))
    }

    fn paint(&self, cx: &mut PaintCx) {
      self.paints.set(self.paints.get() + 1);
      if self.on.get() {
        cx.renderer.fill_rect(cx.rect(), Color::RED, 0.0);
      }
    }
  }

  let on = Rc::new(Cell::new(false));
  let handle = Rc::new(RefCell::new(None));
  let blink_paints = Rc::new(Cell::new(0));
  let sibling_builds = Rc::new(Cell::new(0));
  let sibling_layouts = Rc::new(Cell::new(0));
  let sibling_paints = Rc::new(Cell::new(0));

  let app = ui!(Align {
    alignment: Alignment::TOP_LEFT,
    child: Column {
      main_axis_size: MainAxisSize::Min,
      children: [
        Blinker { on: on.clone(), handle: handle.clone(), paints: blink_paints.clone() },
        CounterProbe {
          builds: sibling_builds.clone(),
          layouts: sibling_layouts.clone(),
          paints: sibling_paints.clone(),
        },
      ],
    },
  });

  let node = Node::mount(app, Context::default());
  let mut spy = Spy::new(400.0, 300.0);
  let mut scene = Scene::new();

  assert_eq!(node.present(&mut scene, &mut spy), Some(Rect::new(0.0, 0.0, 400.0, 300.0)));
  assert_eq!(
    (blink_paints.get(), sibling_builds.get(), sibling_layouts.get(), sibling_paints.get()),
    (1, 1, 1, 1)
  );
  spy.take();

  // Repaint::mark() re-records Blinker only; no build, no layout, no sibling paint.
  on.set(true);
  handle.borrow().as_ref().unwrap().mark();
  assert!(!node.update());
  let dmg = node.present(&mut scene, &mut spy).unwrap();
  assert_eq!(dmg, Rect::new(0.0, 0.0, 11.0, 21.0));
  assert_eq!(
    (blink_paints.get(), sibling_builds.get(), sibling_layouts.get(), sibling_paints.get()),
    (2, 1, 1, 1)
  );
  let batches = spy.take();
  assert_eq!(batches.len(), 1);
  assert!(
    batches[0]
      .1
      .iter()
      .any(|c| matches!(c, DrawCmd::FillRect { color, .. } if *color == Color::RED))
  );
}

#[test]
fn rebuild_that_paints_same_pixels_sends_nothing() {
  #[derive(Widget)]
  struct SamePaint {
    tick: Signal<i32>,
  }

  impl Element for SamePaint {
    fn build(&self, _: &Context) -> Option<Widget> {
      let _ = self.tick.get();
      Some(ui!(Text { text: "unchanged" }))
    }
  }

  let tick = Signal::new(0);
  let node = Node::mount(ui!(SamePaint { tick: tick.clone() }), Context::default());
  let mut spy = Spy::new(300.0, 200.0);
  let mut scene = Scene::new();

  assert_eq!(node.present(&mut scene, &mut spy), Some(Rect::new(0.0, 0.0, 300.0, 200.0)));
  spy.take();

  tick.set(1);
  assert!(node.update());
  assert_eq!(node.present(&mut scene, &mut spy), None);
  assert!(spy.take().is_empty());
}

#[test]
fn moving_one_widget_in_column_damages_old_and_new_rects_not_canvas() {
  #[derive(Widget)]
  struct Movable {
    gap: Signal<f32>,
  }

  impl Element for Movable {
    fn build(&self, _: &Context) -> Option<Widget> {
      let gap = self.gap.get();
      Some(ui!(Align {
        alignment: Alignment::TOP_LEFT,
        child: Column {
          main_axis_size: MainAxisSize::Min,
          children: [
            SizedBox { height: gap },
            Container { width: 40.0, height: 20.0, color: Color::RED },
          ],
        },
      }))
    }
  }

  let gap = Signal::new(10.0);
  let node = Node::mount(ui!(Movable { gap: gap.clone() }), Context::default());
  let mut spy = Spy::new(800.0, 600.0);
  let mut scene = Scene::new();

  node.present(&mut scene, &mut spy);
  spy.take();

  gap.set(50.0);
  assert!(node.update());
  let dmg = node.present(&mut scene, &mut spy).unwrap();
  // Old container: [0, 10, 40, 20] -> padded [0..41, 9..31]
  // New container: [0, 50, 40, 20] -> padded [0..41, 49..71]
  assert_eq!(dmg, Rect::new(0.0, 9.0, 41.0, 62.0));
  assert!(dmg.width < 800.0 && dmg.height < 600.0);
}

#[test]
fn closing_menu_clears_overlay_area() {
  global_events.clear();
  let open = Signal::new(true);

  #[derive(Widget)]
  struct Host {
    open: Signal<bool>,
  }

  impl Element for Host {
    fn build(&self, _: &Context) -> Option<Widget> {
      Some(ui!(Align {
        alignment: Alignment::TOP_LEFT,
        child: MenuAnchor {
          open: self.open.get(),
          menu_children: [MenuItemButton { child: Text { text: "Item 1" }, on_pressed: || {} }],
          child: Text { text: "Anchor" },
        },
      }))
    }
  }

  let node = Node::mount(ui!(Host { open: open.clone() }), Context::default());
  let mut spy = Spy::new(800.0, 600.0);
  let mut scene = Scene::new();

  node.present(&mut scene, &mut spy);
  let first_batches = spy.take();
  // Anchor is 14 px tall; menu sits below y=14 with a panel and text.
  assert!(
    first_batches[0]
      .1
      .iter()
      .any(|c| matches!(c, DrawCmd::Text { text, .. } if &**text == "Item 1"))
  );

  open.set(false);
  assert!(node.update());
  let dmg = node.present(&mut scene, &mut spy).unwrap();
  assert!(dmg.y >= 10.0 && dmg.height < 200.0 && dmg.width < 300.0);
  let batches = spy.take();
  assert_eq!(batches.len(), 1);
  // Batch repaints the damaged overlay region with surface background and no menu item text.
  assert!(
    !batches[0].1.iter().any(|c| matches!(c, DrawCmd::Text { text, .. } if &**text == "Item 1"))
  );
  assert!(batches[0].1.iter().any(|c| matches!(c, DrawCmd::FillRect { rect, .. } if *rect == dmg)));
}

#[test]
fn tab_order_and_focus_rings_survive_caching() {
  global_events.clear();
  let pressed = Rc::new(RefCell::new(Vec::new()));
  let p0 = pressed.clone();
  let p1 = pressed.clone();
  let static_paints = Rc::new(Cell::new(0));
  let static_builds = Rc::new(Cell::new(0));
  let static_layouts = Rc::new(Cell::new(0));

  let app = ui!(Align {
    alignment: Alignment::TOP_LEFT,
    child: Column {
      main_axis_size: MainAxisSize::Min,
      children: [
        TextButton {
          on_pressed: move || p0.borrow_mut().push("first"),
          child: Text { text: "One" }
        },
        TextButton {
          on_pressed: move || p1.borrow_mut().push("second"),
          child: Text { text: "Two" }
        },
        CounterProbe {
          builds: static_builds.clone(),
          layouts: static_layouts.clone(),
          paints: static_paints.clone(),
        },
      ],
    },
  });

  let node = Node::mount(app, Context::default());
  let mut spy = Spy::new(600.0, 400.0);
  let mut scene = Scene::new();

  node.present(&mut scene, &mut spy);
  assert_eq!(focus::count(), 2);
  assert_eq!(focus::current(), None);
  assert_eq!(static_paints.get(), 1);
  spy.take();

  // Tab to first button: no rebuild or relayout, static sibling stays cached.
  dispatch_key_event(keys::TAB);
  let dmg1 = node.present(&mut scene, &mut spy).unwrap();
  assert_eq!(focus::count(), 2);
  assert_eq!(focus::current(), Some(0));
  assert!(dmg1.height < 60.0);
  assert_eq!((static_builds.get(), static_layouts.get(), static_paints.get()), (1, 1, 1));
  spy.take();

  // Idle frame with focus cached: still keeps focus count & order and sends nothing.
  assert_eq!(node.present(&mut scene, &mut spy), None);
  assert_eq!(focus::count(), 2);
  assert_eq!(focus::current(), Some(0));

  // Tab to second button: only old and new buttons re-record; Enter activates second.
  dispatch_key_event(keys::TAB);
  let dmg2 = node.present(&mut scene, &mut spy).unwrap();
  assert_eq!(focus::current(), Some(1));
  assert!(dmg2.height < 100.0);
  assert_eq!(static_paints.get(), 1);

  dispatch_key_event(keys::ENTER);
  assert_eq!(*pressed.borrow(), ["second"]);
  focus::set(None);
}
