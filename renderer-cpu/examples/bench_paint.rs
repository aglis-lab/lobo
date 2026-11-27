//! Animation benchmark using custom `Element::layout` + `Element::paint`
//! (zero per-frame widget tree reconciliation or relayout).
//!
//! Compare directly against `bench_build` (`cargo run -p renderer-cpu --example bench_build`):
//!   cargo run -p renderer-cpu --example bench_paint

use lobo::*;

#[derive(Clone)]
struct BenchState {
  anim_toggled: Signal<bool>,
  anim_loop: Signal<bool>,
  spring_toggled: Signal<bool>,
  spring_loop: Signal<bool>,
  max_fps: Signal<u32>,
  dark: Signal<bool>,
}

impl Default for BenchState {
  fn default() -> Self {
    BenchState {
      anim_toggled: Signal::new(false),
      anim_loop: Signal::new(true),
      spring_toggled: Signal::new(false),
      spring_loop: Signal::new(true),
      max_fps: Signal::new(0), // 0 = Auto (monitor refresh rate)
      dark: Signal::new(false),
    }
  }
}

/// Custom `layout` + `paint` easing bar: repaints only its own display list with zero relayout.
#[derive(Widget)]
struct PaintCurveBar {
  label: &'static str,
  target: f32,
  curve: Curve,
  color: Color,
  #[widget(optional)]
  loop_forever: bool,
}

impl Element for PaintCurveBar {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let w = if constraints.has_bounded_width() { constraints.max_width } else { 372.0 };
    constraints.constrain(Size::new(w, 28.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let s = cx.theme().color_scheme;
    let r = cx.rect();
    let t = if self.loop_forever {
      let p = cx.animate_loop(1400.0);
      let raw = if p < 0.5 { p * 2.0 } else { (1.0 - p) * 2.0 };
      self.curve.apply(raw)
    } else {
      cx.animate_f32(self.target, 700.0, self.curve)
    };
    let label_w = 112.0;
    let paint = TextPaint { color: s.on_surface, size: 12.5, ..Default::default() };
    let lsz = cx.renderer.measure_text(self.label, &paint);
    cx.renderer.draw_text(Offset::new(r.x, r.center().y - lsz.height / 2.0), self.label, &paint);
    let track = Rect::new(r.x + label_w, r.center().y - 6.0, (r.width - label_w).max(20.0), 12.0);
    cx.renderer.fill_rect(track, s.secondary_container, 6.0);
    let fill_w = (track.width * (0.08 + 0.92 * t.clamp(0.0, 1.15))).min(track.width).max(12.0);
    cx.renderer.fill_rect(Rect::new(track.x, track.y, fill_w, track.height), self.color, 6.0);
  }
}

/// Custom `layout` + `paint` physics spring track (zero per-frame relayout or `build()`).
#[derive(Widget)]
struct PaintSpringTrack {
  target: f32,
  #[widget(optional)]
  loop_forever: bool,
}

impl Element for PaintSpringTrack {
  fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
    let w = if constraints.has_bounded_width() { constraints.max_width } else { 340.0 };
    constraints.constrain(Size::new(w, 44.0))
  }

  fn paint(&self, cx: &mut PaintCx) {
    let s = cx.theme().color_scheme;
    let r = cx.rect();
    let pos = cx.animate_spring(self.target, self.loop_forever.then_some(1800.0));
    let track = Rect::new(r.x, r.center().y - 5.0, r.width, 10.0);
    cx.renderer.fill_rect(track, s.secondary_container, 5.0);
    let puck_w = 48.0;
    let x = r.x + ((r.width - puck_w).max(0.0) * pos).clamp(0.0, (r.width - puck_w).max(0.0));
    cx.renderer.fill_rect(Rect::new(x, r.y + 6.0, puck_w, 32.0), s.primary, 8.0);
  }
}

/// Example `CustomPainter` with `should_repaint` showing zero-repaint caching when unchanged.
struct AccentStripePainter {
  accent: Color,
}

impl CustomPainter for AccentStripePainter {
  fn paint(&self, canvas: &mut Recorder<'_>, rect: Rect, _theme: &ThemeData) {
    canvas.fill_rect(rect, self.accent, 2.0);
  }

  fn should_repaint(&self, old: &Self) -> bool {
    self.accent != old.accent
  }
}

#[derive(Widget)]
struct PaintBenchApp {
  state: BenchState,
}

impl Element for PaintBenchApp {
  fn build(&self, _: &Context) -> Option<Widget> {
    let s = &self.state;
    let dark = s.dark.get();
    let t_curves = if s.anim_toggled.get() { 1.0 } else { 0.0 };
    let anim_loop = s.anim_loop.get();
    let t_spring = if s.spring_toggled.get() { 1.0 } else { 0.0 };
    let spring_loop = s.spring_loop.get();
    let max_fps = s.max_fps.get();

    let anim_sig = s.anim_toggled.clone();
    let anim_loop_sig = s.anim_loop.clone();
    let spring_sig = s.spring_toggled.clone();
    let spring_loop_sig = s.spring_loop.clone();
    let dark_sig = s.dark.clone();

    let fps_chip = |label: &'static str, target: u32| {
      let sig = s.max_fps.clone();
      ui!(ChoiceChip {
        label: label,
        selected: max_fps == target,
        on_selected: move |_| sig.set(target),
      })
    };

    let content = ui!(Grid {
      columns: 2,
      gap: 20.0,
      children: [
        Card {
          variant: CardVariant::Outlined,
          child: Container {
            padding: Padding::all(16.0),
            child: Column {
              spacing: 10.0,
              main_axis_size: MainAxisSize::Min,
              cross_axis_alignment: CrossAxisAlignment::Start,
              children: [
                Row {
                  spacing: 12.0,
                  cross_axis_alignment: CrossAxisAlignment::Center,
                  children: [
                    Text { text: "Easing Curves (layout + paint)" },
                    FilledButton {
                      on_pressed: move || anim_sig.set(!anim_sig.peek()),
                      child: Text { text: "Animate" },
                    },
                    Checkbox {
                      value: anim_loop,
                      on_changed: move |v: bool| anim_loop_sig.set(v),
                    },
                    Text { text: "Infinite" },
                  ],
                },
                CustomPaint {
                  painter: Painter::new(AccentStripePainter { accent: Color::hex(0x3B82F6) }),
                  size: Size::new(360.0, 3.0),
                },
                PaintCurveBar { label: "Linear", target: t_curves, curve: Curve::Linear, color: Color::hex(0x3B82F6), loop_forever: anim_loop },
                PaintCurveBar { label: "EaseIn", target: t_curves, curve: Curve::EaseIn, color: Color::hex(0x8B5CF6), loop_forever: anim_loop },
                PaintCurveBar { label: "EaseOut", target: t_curves, curve: Curve::EaseOut, color: Color::hex(0x10B981), loop_forever: anim_loop },
                PaintCurveBar { label: "EaseInOut", target: t_curves, curve: Curve::EaseInOut, color: Color::hex(0xF59E0B), loop_forever: anim_loop },
                PaintCurveBar { label: "FastOutSlowIn", target: t_curves, curve: Curve::FastOutSlowIn, color: Color::hex(0xEF4444), loop_forever: anim_loop },
                PaintCurveBar {
                  label: "CubicBezier",
                  target: t_curves,
                  curve: Curve::CubicBezier(0.34, 1.4, 0.64, 1.0),
                  color: Color::hex(0xEC4899),
                  loop_forever: anim_loop,
                },
              ],
            },
          },
        },
        Card {
          variant: CardVariant::Outlined,
          child: Container {
            padding: Padding::all(16.0),
            child: Column {
              spacing: 16.0,
              main_axis_size: MainAxisSize::Min,
              cross_axis_alignment: CrossAxisAlignment::Start,
              children: [
                Row {
                  spacing: 12.0,
                  cross_axis_alignment: CrossAxisAlignment::Center,
                  children: [
                    Text { text: "Physics Spring (layout + paint)" },
                    OutlinedButton {
                      on_pressed: move || spring_sig.set(!spring_sig.peek()),
                      child: Text { text: "Bounce" },
                    },
                    Checkbox {
                      value: spring_loop,
                      on_changed: move |v: bool| spring_loop_sig.set(v),
                    },
                    Text { text: "Infinite" },
                  ],
                },
                PaintSpringTrack { target: t_spring, loop_forever: spring_loop },
                Row {
                  spacing: 8.0,
                  cross_axis_alignment: CrossAxisAlignment::Center,
                  children: [
                    Text { text: "FPS Cap:" },
                    fps_chip("Auto", 0),
                    fps_chip("60", 60),
                    fps_chip("30", 30),
                  ],
                },
              ],
            },
          },
        },
      ],
    });

    Some(ui!(App {
      title: "Lobo Bench — Custom layout() + paint()",
      theme: ThemeData::light(),
      dark_theme: ThemeData::dark(),
      theme_mode: if dark { ThemeMode::Dark } else { ThemeMode::Light },
      perf_monitor: true,
      max_fps: max_fps,
      home: Scaffold {
        app_bar: AppBar {
          title: Text { text: "Bench: Custom layout() + paint() — F3: perf HUD | F4: damage" },
          actions: [IconButton {
            icon: if dark { Icons::STAR } else { Icons::STAR_BORDER },
            on_pressed: move || dark_sig.set(!dark_sig.peek()),
          }],
        },
        body: Container { padding: Padding::all(24.0), child: content },
      },
    }))
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  run_app(ui!(PaintBenchApp { state: BenchState::default() }))
}
