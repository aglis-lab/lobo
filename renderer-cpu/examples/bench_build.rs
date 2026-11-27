//! Animation benchmark using 100% declarative `Element::build` widgets
//! (`AnimatedValue`, `AnimatedSpring`, `Row`, `Stack`, `Positioned`, `Container`).
//!
//! Compare directly against `bench_paint` (`cargo run -p renderer-cpu --example bench_paint`):
//!   cargo run -p renderer-cpu --example bench_build

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

/// 100% declarative easing bar: uses only `Element::build` (no `layout` or `paint`).
#[derive(Widget)]
struct DeclarativeCurveBar {
  label: &'static str,
  target: f32,
  curve: Curve,
  color: Color,
  #[widget(optional)]
  loop_forever: bool,
}

impl Element for DeclarativeCurveBar {
  fn build(&self, context: &Context) -> Option<Widget> {
    let track_bg = ThemeData::of(context).color_scheme.secondary_container;
    let label = self.label;
    let color = self.color;
    Some(ui!(AnimatedValue {
      target: self.target,
      duration_ms: 700.0,
      curve: self.curve,
      repeat: self.loop_forever,
      builder: move |t: f32| {
        let track_w = 260.0;
        let fill_w = (track_w * (0.08 + 0.92 * t.clamp(0.0, 1.15))).clamp(12.0, track_w);
        ui!(Row {
          spacing: 12.0,
          cross_axis_alignment: CrossAxisAlignment::Center,
          children: [
            SizedBox {
              width: 100.0,
              child: Text { text: label, style: TextStyle { size: 12.5 } },
            },
            Stack {
              children: [
                Container { width: track_w, height: 12.0, color: track_bg, radius: 6.0 },
                Container { width: fill_w, height: 12.0, color, radius: 6.0 },
              ],
            },
          ],
        })
      },
    }))
  }
}

/// 100% declarative spring puck: uses only `Element::build` (no `layout` or `paint`).
#[derive(Widget)]
struct DeclarativeSpringTrack {
  target: f32,
  #[widget(optional)]
  loop_forever: bool,
}

impl Element for DeclarativeSpringTrack {
  fn build(&self, context: &Context) -> Option<Widget> {
    let scheme = ThemeData::of(context).color_scheme;
    let track_bg = scheme.secondary_container;
    let puck_bg = scheme.primary;
    Some(ui!(AnimatedSpring {
      target: self.target,
      repeat: self.loop_forever,
      builder: move |pos: f32| {
        let track_w = 340.0;
        let puck_w = 48.0;
        let x = ((track_w - puck_w) * pos).clamp(0.0, track_w - puck_w);
        ui!(SizedBox {
          width: track_w,
          height: 44.0,
          child: Stack {
            children: [
              Positioned {
                left: 0.0,
                top: 17.0,
                child: Container { width: track_w, height: 10.0, color: track_bg, radius: 5.0 },
              },
              Positioned {
                left: x,
                top: 6.0,
                child: Container { width: puck_w, height: 32.0, color: puck_bg, radius: 8.0 },
              },
            ],
          },
        })
      },
    }))
  }
}

#[derive(Widget)]
struct BuildBenchApp {
  state: BenchState,
}

impl Element for BuildBenchApp {
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
                    Text { text: "Easing Curves (build)" },
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
                DeclarativeCurveBar { label: "Linear", target: t_curves, curve: Curve::Linear, color: Color::hex(0x3B82F6), loop_forever: anim_loop },
                DeclarativeCurveBar { label: "EaseIn", target: t_curves, curve: Curve::EaseIn, color: Color::hex(0x8B5CF6), loop_forever: anim_loop },
                DeclarativeCurveBar { label: "EaseOut", target: t_curves, curve: Curve::EaseOut, color: Color::hex(0x10B981), loop_forever: anim_loop },
                DeclarativeCurveBar { label: "EaseInOut", target: t_curves, curve: Curve::EaseInOut, color: Color::hex(0xF59E0B), loop_forever: anim_loop },
                DeclarativeCurveBar { label: "FastOutSlowIn", target: t_curves, curve: Curve::FastOutSlowIn, color: Color::hex(0xEF4444), loop_forever: anim_loop },
                DeclarativeCurveBar {
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
                    Text { text: "Physics Spring (build)" },
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
                DeclarativeSpringTrack { target: t_spring, loop_forever: spring_loop },
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
      title: "Lobo Bench — Declarative build()",
      theme: ThemeData::light(),
      dark_theme: ThemeData::dark(),
      theme_mode: if dark { ThemeMode::Dark } else { ThemeMode::Light },
      perf_monitor: true,
      max_fps: max_fps,
      home: Scaffold {
        app_bar: AppBar {
          title: Text { text: "Bench: 100% Declarative build() — F3: perf HUD | F4: damage" },
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
  run_app(ui!(BuildBenchApp { state: BenchState::default() }))
}
