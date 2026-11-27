use lobo::*;
use renderer_cpu::PixmapRenderer;

fn assert_pixels_equal(actual: &PixmapRenderer, expected: &PixmapRenderer) {
  assert_eq!(actual.width(), expected.width());
  assert_eq!(actual.height(), expected.height());
  let a = actual.pixmap().data();
  let b = expected.pixmap().data();
  for (i, (pa, pb)) in a.iter().zip(b).enumerate() {
    if pa != pb {
      let x = i % usize::from(actual.width());
      let y = i / usize::from(actual.width());
      panic!("pixel mismatch at ({x}, {y}): actual={pa:?}, expected={pb:?}");
    }
  }
}

#[test]
fn partial_redraw_matches_full_redraw_across_shapes_text_clips_and_overlays() {
  global_events.clear();
  anim::set_enabled(false);
  let step = Signal::new(0u32);
  let menu_open = Signal::new(false);

  #[derive(Widget)]
  struct Demo {
    step: Signal<u32>,
    menu_open: Signal<bool>,
  }

  impl Element for Demo {
    fn build(&self, _: &Context) -> Option<Widget> {
      let s = self.step.get();
      let open = self.menu_open.get();
      let box_top = 20.0 + (s as f32) * 35.7;
      let label = format!("Step {s}: {}", if s.is_multiple_of(2) { "Alpha" } else { "Beta-Gamma" });

      Some(ui!(Stack {
        children: [
          Positioned {
            left: 16.5,
            top: box_top,
            child: Container {
              width: 140.0,
              height: 44.0,
              color: Color::hex(0x6750A4),
              border: Border::new(Color::WHITE, 2.0),
              radius: 10.0,
              padding: Padding::all(8.0),
              child: Text { text: label, style: TextStyle { color: Color::WHITE } },
            },
          },
          Positioned {
            left: 200.0,
            top: 24.0,
            child: SizedBox {
              width: 150.0,
              height: 60.0,
              child: ListView {
                spacing: 4.0,
                children: [
                  Text { text: "Row 1" },
                  Text { text: "Row 2" },
                  Text { text: "Row 3" },
                  Text { text: "Row 4 (clipped)" },
                ],
              },
            },
          },
          Positioned {
            left: 200.0,
            top: 110.0,
            child: MenuAnchor {
              open,
              menu_children: [
                MenuItemButton { child: Text { text: "Cut" }, on_pressed: || {} },
                MenuItemButton { child: Text { text: "Copy" }, on_pressed: || {} },
              ],
              child: FilledButton { on_pressed: || {}, child: Text { text: "Menu" } },
            },
          },
          Positioned {
            left: 20.0,
            top: 210.0,
            child: Row {
              spacing: 12.0,
              children: [
                Checkbox { value: s % 2 == 1, on_changed: |_: bool| {} },
                Switch { value: s >= 2, on_changed: |_: bool| {} },
                CircularProgressIndicator { value: 0.65 },
              ],
            },
          },
        ],
      }))
    }
  }

  let app = ui!(Demo { step: step.clone(), menu_open: menu_open.clone() });
  let node = Node::mount(app.clone(), Context::default());
  let mut inc = PixmapRenderer::new(420, 300);
  let mut scene = Scene::new();

  let check_full = |inc: &PixmapRenderer, s: u32, open: bool| {
    let full_step = Signal::new(s);
    let full_open = Signal::new(open);
    let full_node =
      Node::mount(ui!(Demo { step: full_step, menu_open: full_open }), Context::default());
    let mut full = PixmapRenderer::new(420, 300);
    let mut full_scene = Scene::new();
    full_node.present(&mut full_scene, &mut full);
    assert_pixels_equal(inc, &full);
  };

  node.present(&mut scene, &mut inc);
  check_full(&inc, 0, false);

  // Step 1: move box + change text + toggle checkbox.
  step.set(1);
  assert!(node.update());
  let dmg = node.present(&mut scene, &mut inc).unwrap();
  assert!(dmg.width < 420.0);
  check_full(&inc, 1, false);

  // Step 2: open menu overlay.
  menu_open.set(true);
  assert!(node.update());
  node.present(&mut scene, &mut inc).unwrap();
  check_full(&inc, 1, true);

  // Step 3: close menu overlay & flip switch.
  menu_open.set(false);
  step.set(2);
  assert!(node.update());
  node.present(&mut scene, &mut inc).unwrap();
  check_full(&inc, 2, false);
}

#[test]
fn resize_redraws_full_canvas() {
  let app = ui!(Center {
    child: Container {
      width: 80.0,
      height: 40.0,
      color: Color::BLUE,
      radius: 8.0,
      child: Text { text: "Hi", style: TextStyle { color: Color::WHITE } },
    },
  });
  let node = Node::mount(app.clone(), Context::default());
  let mut inc = PixmapRenderer::new(200, 120);
  let mut scene = Scene::new();
  node.present(&mut scene, &mut inc);

  inc.resize(320, 180);
  let dmg = node.present(&mut scene, &mut inc);
  assert_eq!(dmg, Some(Rect::new(0.0, 0.0, 320.0, 180.0)));

  let fresh_node = Node::mount(app, Context::default());
  let mut fresh = PixmapRenderer::new(320, 180);
  let mut fresh_scene = Scene::new();
  fresh_node.present(&mut fresh_scene, &mut fresh);
  assert_pixels_equal(&inc, &fresh);
}

#[test]
fn hidpi_partial_redraw_matches_full_redraw() {
  let offset_y = Signal::new(10.0f32);

  #[derive(Widget)]
  struct HiDpiBox {
    y: Signal<f32>,
  }

  impl Element for HiDpiBox {
    fn build(&self, _: &Context) -> Option<Widget> {
      let y = self.y.get();
      Some(ui!(Stack {
        children: [Positioned {
          left: 24.5,
          top: y,
          child: Container {
            width: 110.0,
            height: 38.0,
            color: Color::hex(0x2196F3),
            radius: 6.0,
            padding: Padding::all(6.0),
            child: Text { text: "Retina", style: TextStyle { color: Color::WHITE } },
          },
        }],
      }))
    }
  }

  let node = Node::mount(ui!(HiDpiBox { y: offset_y.clone() }), Context::default());
  let mut inc = PixmapRenderer::with_scale(300.0, 200.0, 2.0);
  assert_eq!((inc.width(), inc.height()), (600, 400));
  let mut scene = Scene::new();
  node.present(&mut scene, &mut inc);

  offset_y.set(52.25);
  assert!(node.update());
  let dmg = node.present(&mut scene, &mut inc).unwrap();
  assert!(dmg.width < 300.0 && dmg.height < 200.0);

  let full_node = Node::mount(ui!(HiDpiBox { y: Signal::new(52.25) }), Context::default());
  let mut full = PixmapRenderer::with_scale(300.0, 200.0, 2.0);
  let mut full_scene = Scene::new();
  full_node.present(&mut full_scene, &mut full);
  assert_pixels_equal(&inc, &full);
}

#[test]
fn same_size_resize_keeps_pixels_and_scale_change_redraws() {
  let app = ui!(Center { child: Container { width: 80.0, height: 40.0, color: Color::RED } });
  let node = Node::mount(app.clone(), Context::default());
  let mut r = PixmapRenderer::with_scale(300.0, 200.0, 2.0);
  let mut scene = Scene::new();
  node.present(&mut scene, &mut r);
  let before = r.pixel(300, 200);
  assert_ne!(before, [0, 0, 0, 0]);

  // Same logical & physical size must not wipe the backing pixmap.
  assert!(!r.resize_with_scale(300.0, 200.0, 2.0));
  assert_eq!(node.present(&mut scene, &mut r), None);
  assert_eq!(r.pixel(300, 200), before);

  // Scale change at the same logical size wipes the pixmap; invalidate triggers a full redraw.
  assert!(r.resize_with_scale(300.0, 200.0, 1.0));
  scene.invalidate();
  assert_eq!(node.present(&mut scene, &mut r), Some(Rect::new(0.0, 0.0, 300.0, 200.0)));

  let full_node = Node::mount(app, Context::default());
  let mut full = PixmapRenderer::with_scale(300.0, 200.0, 1.0);
  let mut full_scene = Scene::new();
  full_node.present(&mut full_scene, &mut full);
  assert_pixels_equal(&r, &full);
}
