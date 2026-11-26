use std::rc::Rc;

use lobo::*;
use renderer_cpu::PixmapRenderer;

fn render_widget(w: Widget, width: u16, height: u16) -> (Rc<Node>, PixmapRenderer) {
  let node = Node::mount(w, Context::default());
  let mut renderer = PixmapRenderer::new(width, height);
  let mut scene = Scene::new();
  node.present(&mut scene, &mut renderer);
  (node, renderer)
}

fn has_pixel_matching(r: &PixmapRenderer, rect: Rect, pred: impl Fn([u8; 4]) -> bool) -> bool {
  let x0 = (rect.x.max(0.0) as u16).min(r.width());
  let y0 = (rect.y.max(0.0) as u16).min(r.height());
  let x1 = (rect.right().ceil().max(0.0) as u16).min(r.width());
  let y1 = (rect.bottom().ceil().max(0.0) as u16).min(r.height());
  for y in y0..y1 {
    for x in x0..x1 {
      if pred(r.pixel(x, y)) {
        return true;
      }
    }
  }
  false
}

#[test]
fn text_wrapping_and_max_lines_layout_and_render() {
  let (_, r) = render_widget(
    ui!(Align {
      alignment: Alignment::TOP_LEFT,
      child: SizedBox { width: 72.0, child: Text { text: "one two three four", max_lines: 2 } },
    }),
    200,
    100,
  );
  // 2 lines at default 14px line height => pixels drawn in y=0..28, none below y=30.
  let bg = r.pixel(190, 90);
  assert!(has_pixel_matching(&r, Rect::new(0.0, 0.0, 72.0, 28.0), |p| p != bg));
  assert!(!has_pixel_matching(&r, Rect::new(0.0, 32.0, 200.0, 68.0), |p| p != bg));
}

#[test]
fn stack_positioned_and_row_expanded_render_in_expected_regions() {
  let (_, r) = render_widget(
    ui!(Stack {
      children: [
        Positioned {
          right: 10.0,
          bottom: 10.0,
          child: Container { width: 30.0, height: 20.0, color: Color::RED },
        },
        Row {
          children: [
            Container { width: 20.0, height: 20.0, color: Color::GREEN },
            Expanded { child: Container { height: 20.0, color: Color::BLUE } },
          ],
        },
      ],
    }),
    200,
    100,
  );

  // Bottom-right positioned red box: [160..190, 70..90]
  assert_eq!(r.pixel(175, 80), [244, 67, 54, 255]);
  // Row green box at left: [0..20, 0..20]
  assert_eq!(r.pixel(10, 10), [76, 175, 80, 255]);
  // Row expanded blue box fills [20..200, 0..20]
  assert_eq!(r.pixel(150, 10), [33, 150, 243, 255]);
}

#[test]
fn listview_clips_overflowing_children() {
  let (_, r) = render_widget(
    ui!(Align {
      alignment: Alignment::TOP_LEFT,
      child: SizedBox {
        width: 100.0,
        height: 40.0,
        child: ListView {
          children: [
            Container { width: 80.0, height: 30.0, color: Color::GREEN },
            Container { width: 80.0, height: 30.0, color: Color::RED },
          ],
        },
      },
    }),
    200,
    100,
  );
  let bg = r.pixel(180, 80);
  // First 10 px of red item (y=30..40) visible; y >= 41 must be clipped to background.
  assert_eq!(r.pixel(10, 35), [244, 67, 54, 255]);
  assert_eq!(r.pixel(10, 45), bg);
}

#[test]
fn material_widgets_render_on_hd720() {
  global_events.clear();
  let checked = Signal::new(true);
  let text = Signal::new(String::from("Hello"));
  let app = ui!(Scaffold {
    app_bar: AppBar { title: Text { text: "Test" } },
    body: Container {
      padding: Padding::all(16.0),
      child: Column {
        spacing: 12.0,
        children: [
          Row {
            spacing: 8.0,
            children: [
              ElevatedButton { on_pressed: || {}, child: Text { text: "Elevated" } },
              FilledButton { on_pressed: || {}, child: Text { text: "Filled" } },
              OutlinedButton { on_pressed: || {}, child: Text { text: "Outlined" } },
              Checkbox { value: checked.get(), on_changed: |_: bool| {} },
              Switch { value: true, on_changed: |_: bool| {} },
            ],
          },
          Card {
            child: Container {
              padding: Padding::all(12.0),
              child: TextField { controller: text, label: "Name" },
            },
          },
          LinearProgressIndicator { value: 0.5 },
        ],
      },
    },
  });
  let (_, r) = render_widget(app, 1280, 720);
  let png = r.to_png().expect("png encoding");
  assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
  assert!(png.len() > 1000);
}
