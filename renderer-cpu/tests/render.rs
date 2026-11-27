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

#[test]
fn parley_text_metrics_are_proportional_and_include_trailing_whitespace() {
  let r = PixmapRenderer::new(320, 120);
  let base = TextPaint::default();
  let empty = r.measure_text("", &base);
  assert_eq!(empty.width, 0.0);
  assert!(empty.height > 0.0);

  let narrow = r.measure_text("iii", &base);
  let wide = r.measure_text("WWW", &base);
  assert!(narrow.width > 0.0);
  assert!(wide.width > narrow.width, "expected WWW ({}) > iii ({})", wide.width, narrow.width);

  let trimmed = r.measure_text("Hello", &base);
  let with_space = r.measure_text("Hello ", &base);
  assert!(
    with_space.width > trimmed.width,
    "expected trailing space to advance width: {} vs {}",
    with_space.width,
    trimmed.width
  );

  let large = r.measure_text("Hello", &TextPaint { size: 28.0, ..base });
  assert!(large.width > trimmed.width);
  assert!(large.height > trimmed.height);
}

#[test]
fn parley_text_styles_bold_italic_and_underline_render_distinctly() {
  let render_style = |style: TextStyle| {
    let (_, r) = render_widget(
      ui!(Align {
        alignment: Alignment::TOP_LEFT,
        child: Text { text: "Typography gjpqy", style },
      }),
      220,
      48,
    );
    r.pixmap().data().to_vec()
  };

  let regular = render_style(TextStyle::default());
  let bold = render_style(TextStyle { bold: Some(true), ..TextStyle::default() });
  let italic = render_style(TextStyle { italic: Some(true), ..TextStyle::default() });
  let underlined = render_style(TextStyle { underline: Some(true), ..TextStyle::default() });

  assert_ne!(regular, bold, "bold text should differ from regular");
  assert_ne!(regular, italic, "italic text should differ from regular");
  assert_ne!(regular, underlined, "underlined text should differ from regular");
}

#[cfg(debug_assertions)]
#[test]
fn perf_overlay_renders_stats_and_detects_bottleneck() {
  let timings = FrameTimings {
    update_ms: 0.4,
    layout_ms: 0.8,
    paint_ms: 1.2,
    scene_ms: 0.3,
    raster_ms: 6.5,
    present_ms: 1.1,
    cmd_count: 142,
  };
  assert_eq!(timings.bottleneck().0, "Raster");
  assert!((timings.total_ms() - 10.3).abs() < 0.01);

  let mut monitor = PerfMonitor::new();
  monitor.set_gpu_percent(Some(42.0));
  let stats = monitor.record_frame(timings);
  assert!(stats.fps > 0.0);
  assert!(!stats.fps_from_burst, "isolated first frame should be estimated on-demand FPS");
  assert_eq!(stats.gpu_percent, Some(42.0));

  let (_, r) = render_widget(
    ui!(Align {
      alignment: Alignment::TOP_LEFT,
      child: PerfOverlay { stats },
    }),
    320,
    240,
  );
  // Dark HUD background inside panel bounds, untouched outside.
  let bg = r.pixel(310, 230);
  assert_ne!(r.pixel(20, 20), bg);
  assert!(has_pixel_matching(&r, Rect::new(0.0, 0.0, 268.0, 204.0), |p| p != bg));
}

#[test]
fn app_widget_controls_theme_and_separate_perf_layer() {
  global_events.clear();
  let pressed = Signal::new(0u32);
  let p = pressed.clone();
  let app = ui!(App {
    title: "Test App",
    theme: ThemeData::light(),
    dark_theme: ThemeData::dark(),
    theme_mode: ThemeMode::Dark,
    perf_monitor: true,
    home: Center {
      child: FilledButton {
        on_pressed: move || p.set(p.peek() + 1),
        child: Text { text: "Hello App" },
      },
    },
  });
  let root = Node::mount(app, Context::default());
  let mut app_renderer = PixmapRenderer::new(400, 300);
  let mut app_scene = Scene::new();
  let mut perf = PerfLayer::new();

  let dmg = perf.present_app(&root, &mut app_scene, &mut app_renderer, 0.2);
  assert!(dmg.is_some());

  let mut overlay_renderer =
    PixmapRenderer::with_scale(PERF_OVERLAY_SIZE.width, PERF_OVERLAY_SIZE.height, 1.0);
  let rendered = perf.render_overlay(0.5, &mut overlay_renderer);
  assert_eq!(rendered, cfg!(debug_assertions));

  // Rendering the separate perf overlay must NOT clear the app's keyboard focus ring.
  dispatch_event(&EventKey::new(keys::TAB));
  dispatch_event(&EventKey::new(keys::ENTER));
  assert_eq!(pressed.peek(), 1, "Tab + Enter should activate the button even with perf overlay on");

  #[cfg(debug_assertions)]
  {
    // Transparent corner outside rounded HUD card, opaque/translucent inside.
    assert_eq!(overlay_renderer.pixel(0, 0)[3], 0);
    assert!(overlay_renderer.pixel(20, 20)[3] > 0);
  }
}

#[test]
fn pointer_clicks_buttons_textfields_sliders_gestures_and_scrolls_listview() {
  global_events.clear();
  clear_pointer_state();

  let clicks = Signal::new(0u32);
  let submitted = Signal::new(0u32);
  let tapped = Signal::new(0u32);
  let text = Signal::new(String::from("Hi"));
  let slider_val = Signal::new(0.2f32);

  let c = clicks.clone();
  let sub = submitted.clone();
  let tap = tapped.clone();
  let sl = slider_val.clone();

  let app = ui!(Align {
    alignment: Alignment::TOP_LEFT,
    child: Column {
      spacing: 8.0,
      main_axis_size: MainAxisSize::Min,
      cross_axis_alignment: CrossAxisAlignment::Start,
      children: [
        FilledButton {
          on_pressed: move || c.set(c.peek() + 1),
          child: Text { text: "Click" },
        },
        SizedBox {
          width: 200.0,
          child: TextField {
            controller: text.clone(),
            on_submitted: move |_: String| sub.set(sub.peek() + 1),
          },
        },
        SizedBox {
          width: 200.0,
          child: Slider {
            value: slider_val.get(),
            on_changed: move |v: f32| sl.set(v),
          },
        },
        GestureDetector {
          on_tap: move || tap.set(tap.peek() + 1),
          child: Container { width: 80.0, height: 30.0, color: Color::GREEN },
        },
      ],
    },
  });

  let (node, mut r) = render_widget(app, 320, 260);
  let mut scene = Scene::new();
  node.present(&mut scene, &mut r);

  // 1. Click FilledButton at (30, 20)
  let btn_pos = Offset::new(30.0, 20.0);
  dispatch_pointer(PointerEvent::Down { pos: btn_pos, button: PointerButton::Primary });
  dispatch_pointer(PointerEvent::Up { pos: btn_pos, button: PointerButton::Primary });
  assert_eq!(clicks.peek(), 1);

  // 2. Click TextField at (40, 62): focuses without firing on_submitted, then typing appends.
  let tf_pos = Offset::new(40.0, 62.0);
  dispatch_pointer(PointerEvent::Down { pos: tf_pos, button: PointerButton::Primary });
  dispatch_pointer(PointerEvent::Up { pos: tf_pos, button: PointerButton::Primary });
  assert_eq!(submitted.peek(), 0);
  dispatch_event(&EventKey::new('!'));
  assert_eq!(text.peek(), "Hi!");

  // 3. Hover over Slider at (160, 114) must NOT change value; Down+Move+Up drags it.
  let sl_pos = Offset::new(160.0, 114.0);
  dispatch_pointer(PointerEvent::Move(sl_pos));
  assert!((slider_val.peek() - 0.2).abs() < 1e-4);
  dispatch_pointer(PointerEvent::Down { pos: sl_pos, button: PointerButton::Primary });
  assert!(slider_val.peek() > 0.7);
  dispatch_pointer(PointerEvent::Up { pos: sl_pos, button: PointerButton::Primary });

  // 4. Tap GestureDetector at (40, 155)
  let gd_pos = Offset::new(40.0, 155.0);
  dispatch_pointer(PointerEvent::Down { pos: gd_pos, button: PointerButton::Primary });
  dispatch_pointer(PointerEvent::Up { pos: gd_pos, button: PointerButton::Primary });
  assert_eq!(tapped.peek(), 1);
}

#[test]
fn animated_f32_and_switch_request_frames_until_settled() {
  let _ = anim::take_frame_request();
  let mut a = AnimatedF32::default();
  // First call snaps without animating.
  let (v0, active0) = a.advance(0.0, 100.0, Curve::FastOutSlowIn);
  assert_eq!(v0, 0.0);
  assert!(!active0);

  // Changing target starts animation and requests a frame.
  let (_, active1) = a.advance(1.0, 100.0, Curve::FastOutSlowIn);
  assert!(active1);
  assert!(anim::take_frame_request());

  // After duration elapses, animation settles at target and stops requesting frames.
  std::thread::sleep(std::time::Duration::from_millis(110));
  let (v2, active2) = a.advance(1.0, 100.0, Curve::FastOutSlowIn);
  assert_eq!(v2, 1.0);
  assert!(!active2);
  assert!(!anim::take_frame_request());
}

#[test]
fn textfield_select_all_with_cmd_a_and_ctrl_a() {
  global_events.clear();
  clear_pointer_state();

  let text = Signal::new(String::from("Hello World"));
  let app = ui!(Align {
    alignment: Alignment::TOP_LEFT,
    child: SizedBox { width: 220.0, child: TextField { controller: text.clone() } },
  });
  let (node, mut r) = render_widget(app, 300, 80);
  let mut scene = Scene::new();
  node.present(&mut scene, &mut r);

  // Click to focus TextField.
  let pos = Offset::new(40.0, 20.0);
  dispatch_pointer(PointerEvent::Down { pos, button: PointerButton::Primary });
  dispatch_pointer(PointerEvent::Up { pos, button: PointerButton::Primary });

  // Cmd+A selects all; typing replaces entire text.
  dispatch_event(&EventKey::with_modifiers('a', Modifiers::META));
  dispatch_event(&EventKey::new('X'));
  assert_eq!(text.peek(), "X");

  // Ctrl+A selects all; Backspace clears entire text.
  dispatch_event(&EventKey::with_modifiers('a', Modifiers::CTRL));
  dispatch_event(&EventKey::new(keys::BACKSPACE));
  assert_eq!(text.peek(), "");
}

#[test]
fn taffy_grid_lays_out_columns_and_gaps_accurately() {
  let app = ui!(Align {
    alignment: Alignment::TOP_LEFT,
    child: SizedBox {
      width: 210.0,
      height: 110.0,
      child: Grid {
        columns: 2,
        gap: 10.0,
        children: [
          Container { height: 50.0, color: Color::RED },
          Container { height: 50.0, color: Color::GREEN },
          Container { height: 50.0, color: Color::BLUE },
          Container { height: 50.0, color: Color::YELLOW },
        ],
      },
    },
  });
  let (_, r) = render_widget(app, 240, 140);
  // 2 columns in 210px with 10px gap => each column is 100px: [0..100], [110..210].
  // Row 0: [0..50], Row 1: [60..110].
  assert_eq!(r.pixel(50, 25), [244, 67, 54, 255]); // RED
  assert_eq!(r.pixel(160, 25), [76, 175, 80, 255]); // GREEN
  assert_eq!(r.pixel(50, 85), [33, 150, 243, 255]); // BLUE
  assert_eq!(r.pixel(160, 85), [255, 235, 59, 255]); // YELLOW
}

#[test]
fn custom_paint_should_repaint_skips_unchanged_and_repaints_on_change() {
  use std::cell::Cell;
  use std::rc::Rc;

  struct BoxPainter {
    color: Color,
    paints: Rc<Cell<u32>>,
  }

  impl CustomPainter for BoxPainter {
    fn paint(&self, canvas: &mut Recorder<'_>, rect: Rect, _theme: &ThemeData) {
      self.paints.set(self.paints.get() + 1);
      canvas.fill_rect(rect, self.color, 0.0);
    }

    fn should_repaint(&self, old: &Self) -> bool {
      self.color != old.color
    }
  }

  #[derive(Widget)]
  struct Host {
    color: Signal<Color>,
    tick: Signal<u32>,
    paints: Rc<Cell<u32>>,
  }

  impl Element for Host {
    fn build(&self, _: &Context) -> Option<Widget> {
      let _ = self.tick.get();
      let color = self.color.get();
      Some(ui!(CustomPaint {
        painter: Painter::new(BoxPainter { color, paints: Rc::clone(&self.paints) }),
        size: Size::new(80.0, 40.0),
      }))
    }
  }

  let color = Signal::new(Color::RED);
  let tick = Signal::new(0u32);
  let paints = Rc::new(Cell::new(0u32));
  let node = Node::mount(
    ui!(Host { color: color.clone(), tick: tick.clone(), paints: Rc::clone(&paints) }),
    Context::default(),
  );
  let mut r = PixmapRenderer::new(120, 80);
  let mut scene = Scene::new();

  node.present(&mut scene, &mut r);
  assert_eq!(paints.get(), 1);
  assert_eq!(r.pixel(20, 20), [244, 67, 54, 255]);

  // Parent rebuilds with the same color -> should_repaint returns false -> no repaint!
  tick.set(1);
  node.update();
  let dmg = node.present(&mut scene, &mut r);
  assert_eq!(dmg, None);
  assert_eq!(paints.get(), 1);

  // Color changes -> should_repaint returns true -> repaints!
  color.set(Color::BLUE);
  node.update();
  let dmg = node.present(&mut scene, &mut r);
  assert!(dmg.is_some());
  assert_eq!(paints.get(), 2);
  assert_eq!(r.pixel(20, 20), [33, 150, 243, 255]);
}

#[test]
fn declarative_animated_value_spring_and_max_fps() {
  let target = Signal::new(0.0f32);
  let t_sig = target.clone();

  #[derive(Widget)]
  struct AnimDemo {
    target: Signal<f32>,
  }

  impl Element for AnimDemo {
    fn build(&self, _: &Context) -> Option<Widget> {
      let t = self.target.get();
      Some(ui!(App {
        title: "Anim Demo",
        max_fps: 30,
        home: Column {
          children: [
            AnimatedValue {
              target: t,
              duration_ms: 200.0,
              curve: Curve::Linear,
              builder: move |v: f32| ui!(Container { width: 20.0 + 80.0 * v, height: 16.0, color: Color::RED }),
            },
            AnimatedSpring {
              target: t,
              builder: move |v: f32| ui!(Container { width: 20.0 + 80.0 * v.clamp(0.0, 1.0), height: 16.0, color: Color::BLUE }),
            },
          ],
        },
      }))
    }
  }

  let node = Node::mount(ui!(AnimDemo { target: t_sig }), Context::default());
  assert_eq!(current_max_fps(), Some(30));

  let mut r = PixmapRenderer::new(160, 80);
  let mut scene = Scene::new();
  node.present(&mut scene, &mut r);

  // Trigger animation and verify declarative tickers request follow-up frames.
  let _ = lobo::anim::take_frame_request();
  target.set(1.0);
  node.update();
  assert!(lobo::anim::take_frame_request());
  std::thread::sleep(std::time::Duration::from_millis(20));
  assert!(node.update());
  let dmg = node.present(&mut scene, &mut r);
  assert!(dmg.is_some());
}
