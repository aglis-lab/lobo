use lobo::*;

#[derive(Clone)]
struct GalleryState {
  tab: Signal<usize>,
  dark: Signal<bool>,
  clicks: Signal<u32>,
  checked_a: Signal<bool>,
  checked_b: Signal<bool>,
  radio: Signal<usize>,
  slider: Signal<f32>,
  filter_on: Signal<bool>,
  choice: Signal<usize>,
  name: Signal<String>,
  note: Signal<String>,
  list_idx: Signal<usize>,
  menu_open: Signal<bool>,
  dialog_open: Signal<bool>,
  anim_toggled: Signal<bool>,
  anim_loop: Signal<bool>,
  spring_toggled: Signal<bool>,
  spring_loop: Signal<bool>,
  max_fps: Signal<u32>,
  status: Signal<String>,
}

impl Default for GalleryState {
  fn default() -> Self {
    GalleryState {
      tab: Signal::new(0),
      dark: Signal::new(false),
      clicks: Signal::new(0),
      checked_a: Signal::new(true),
      checked_b: Signal::new(false),
      radio: Signal::new(0),
      slider: Signal::new(0.6),
      filter_on: Signal::new(true),
      choice: Signal::new(0),
      name: Signal::new(String::from("Lobo UI")),
      note: Signal::new(String::from("Type while focused")),
      list_idx: Signal::new(0),
      menu_open: Signal::new(false),
      dialog_open: Signal::new(false),
      anim_toggled: Signal::new(false),
      anim_loop: Signal::new(false),
      spring_toggled: Signal::new(false),
      spring_loop: Signal::new(false),
      max_fps: Signal::new(60),
      status: Signal::new(String::from("Tab: focus | F3: perf HUD | F4: damage flash")),
    }
  }
}

/// Demo widget: declaratively animates a bar with a chosen easing `Curve` over 700 ms.
#[derive(Widget)]
struct CurveBar {
  label: &'static str,
  target: f32,
  curve: Curve,
  color: Color,
  #[widget(optional)]
  loop_forever: bool,
}

impl Element for CurveBar {
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
        let track_w = 240.0;
        let fill_w = (track_w * (0.08 + 0.92 * t.clamp(0.0, 1.15))).clamp(12.0, track_w);
        ui!(Row {
          spacing: 12.0,
          cross_axis_alignment: CrossAxisAlignment::Center,
          children: [
            SizedBox { width: 100.0, child: Text { text: label, style: TextStyle { size: 12.5 } } },
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

/// Demo widget: declaratively animates a physics-based `Spring` puck across a track.
#[derive(Widget)]
struct SpringTrack {
  target: f32,
  #[widget(optional)]
  loop_forever: bool,
}

impl Element for SpringTrack {
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
struct Gallery {
  state: GalleryState,
}

impl Gallery {
  fn controls_page(&self) -> Widget {
    let s = &self.state;
    let clicks = s.clicks.clone();
    let st_btn = s.status.clone();
    let bump = move |label: &'static str| {
      let clicks = clicks.clone();
      let st = st_btn.clone();
      move || {
        let next = clicks.peek() + 1;
        clicks.set(next);
        st.set(format!("Pressed {label} (total clicks: {next})"));
      }
    };
    let dark = s.dark.clone();
    let ca = s.checked_a.clone();
    let cb = s.checked_b.clone();
    let radio = s.radio.clone();
    let slider = s.slider.clone();
    let filter_on = s.filter_on.clone();
    let choice = s.choice.clone();
    let val = s.slider.get();
    let r_sel = s.radio.get();
    let f_sel = s.filter_on.get();
    let c_sel = s.choice.get();
    let st_chip = s.status.clone();

    ui!(Column {
      spacing: 18.0,
      main_axis_size: MainAxisSize::Min,
      cross_axis_alignment: CrossAxisAlignment::Start,
      children: [
        Text { text: format!("Buttons (clicks: {})", s.clicks.get()) },
        Row {
          spacing: 12.0,
          children: [
            ElevatedButton { on_pressed: bump("Elevated"), child: Text { text: "Elevated" } },
            FilledButton { on_pressed: bump("Filled"), child: Text { text: "Filled" } },
            FilledButton { tonal: true, on_pressed: bump("Tonal"), child: Text { text: "Tonal" } },
            OutlinedButton { on_pressed: bump("Outlined"), child: Text { text: "Outlined" } },
            TextButton { on_pressed: bump("Text"), child: Text { text: "Text" } },
          ],
        },
        Text { text: "Selection & Theme Switch" },
        Row {
          spacing: 20.0,
          cross_axis_alignment: CrossAxisAlignment::Center,
          children: [
            Checkbox { value: s.checked_a.get(), on_changed: move |v: bool| ca.set(v) },
            Checkbox { value: s.checked_b.get(), on_changed: move |v: bool| cb.set(v) },
            Radio {
              value: 0,
              group_value: r_sel,
              on_changed: {
                let r = radio.clone();
                move |v: usize| r.set(v)
              },
            },
            Radio { value: 1, group_value: r_sel, on_changed: move |v: usize| radio.set(v) },
            Switch { value: s.dark.get(), on_changed: move |v: bool| dark.set(v) },
            Text { text: if s.dark.get() { "Dark mode" } else { "Light mode" } },
          ],
        },
        Text { text: format!("Slider & Progress ({:.0}%)", val * 100.0) },
        Row {
          spacing: 24.0,
          cross_axis_alignment: CrossAxisAlignment::Center,
          children: [
            SizedBox {
              width: 240.0,
              child: Slider { value: val, on_changed: move |v: f32| slider.set(v) },
            },
            SizedBox { width: 220.0, child: LinearProgressIndicator { value: val } },
            CircularProgressIndicator { value: val },
            Badge {
              label: format!("{}", s.clicks.get()),
              child: Icon { icon: Icons::NOTIFICATIONS }
            },
          ],
        },
        Text { text: "Chips" },
        Wrap {
          spacing: 10.0,
          run_spacing: 8.0,
          children: [
            Chip { label: "Assist", avatar: Icons::STAR },
            FilterChip {
              label: "Filter",
              selected: f_sel,
              on_selected: move |v: bool| filter_on.set(v),
            },
            ChoiceChip {
              label: "Choice A",
              selected: c_sel == 0,
              on_selected: {
                let c = choice.clone();
                let st = st_chip.clone();
                move |_: bool| {
                  c.set(0);
                  st.set("Selected Choice A".to_string());
                }
              },
            },
            ChoiceChip {
              label: "Choice B",
              selected: c_sel == 1,
              on_selected: move |_: bool| {
                choice.set(1);
                st_chip.set("Selected Choice B".to_string());
              },
            },
          ],
        },
      ],
    })
  }

  fn inputs_page(&self) -> Widget {
    let s = &self.state;
    let list_idx = s.list_idx.get();
    let list_sig = s.list_idx.clone();
    let st_list = s.status.clone();
    let tiles: Vec<Widget> = (0..8)
      .map(|i| {
        let sig = list_sig.clone();
        let st = st_list.clone();
        ui!(ListTile {
          leading: Icon { icon: Icons::FOLDER },
          title: Text { text: format!("List item {}", i + 1) },
          subtitle: Text { text: "Press Enter/Space to select" },
          selected: list_idx == i,
          on_tap: move || {
            sig.set(i);
            st.set(format!("Selected item {}", i + 1));
          },
        })
      })
      .collect();

    ui!(Row {
      spacing: 24.0,
      cross_axis_alignment: CrossAxisAlignment::Start,
      children: [
        Card {
          variant: CardVariant::Elevated,
          child: Container {
            width: 340.0,
            padding: Padding::all(16.0),
            child: Column {
              spacing: 16.0,
              main_axis_size: MainAxisSize::Min,
              children: [
                Text { text: "Inputs (shadcn/ui)" },
                TextField {
                  controller: s.name.clone(),
                  label: "Email",
                  hint: "Email",
                  helper_text: "Enter your email address",
                },
                TextField { controller: s.note.clone(), label: "Notes", hint: "Add value" },
                TextField {
                  controller: Signal::new(String::new()),
                  label: "Disabled",
                  hint: "Email",
                  helper_text: "Enter your email address",
                  disabled: true,
                },
              ],
            },
          },
        },
        Card {
          variant: CardVariant::Outlined,
          child: SizedBox { width: 380.0, height: 360.0, child: ListView { children: tiles } },
        },
      ],
    })
  }

  fn overlays_page(&self) -> Widget {
    let s = &self.state;
    let open = s.menu_open.get();
    let menu_sig = s.menu_open.clone();
    let st = s.status.clone();
    let menu_item = |label: &'static str| {
      let m = menu_sig.clone();
      let st = st.clone();
      ui!(MenuItemButton {
        child: Text { text: label },
        on_pressed: move || {
          m.set(false);
          st.set(format!("Picked menu item: {label}"));
        },
      })
    };
    let toggle_menu = s.menu_open.clone();
    let open_dialog = s.dialog_open.clone();

    ui!(Column {
      spacing: 20.0,
      main_axis_size: MainAxisSize::Min,
      cross_axis_alignment: CrossAxisAlignment::Start,
      children: [
        Text { text: "Overlays (MenuAnchor & AlertDialog)" },
        Row {
          spacing: 16.0,
          children: [
            MenuAnchor {
              open,
              menu_children: [menu_item("New file"), menu_item("Duplicate"), menu_item("Close")],
              child: FilledButton {
                on_pressed: move || toggle_menu.set(!toggle_menu.peek()),
                child: Text { text: if open { "Close Menu" } else { "Open Menu" } },
              },
            },
            OutlinedButton {
              on_pressed: move || open_dialog.set(true),
              child: Text { text: "Show AlertDialog" },
            },
          ],
        },
      ],
    })
  }
  fn animations_page(&self) -> Widget {
    let s = &self.state;
    let t_curves = if s.anim_toggled.get() { 1.0 } else { 0.0 };
    let anim_loop = s.anim_loop.get();
    let t_spring = if s.spring_toggled.get() { 1.0 } else { 0.0 };
    let spring_loop = s.spring_loop.get();
    let anim_sig = s.anim_toggled.clone();
    let anim_loop_sig = s.anim_loop.clone();
    let spring_sig = s.spring_toggled.clone();
    let spring_loop_sig = s.spring_loop.clone();
    let cap_fps = s.max_fps.get();
    let fps_chip = |label: &'static str, target: u32| {
      let sig = s.max_fps.clone();
      ui!(ChoiceChip { label, selected: cap_fps == target, on_selected: move |_| sig.set(target) })
    };

    ui!(Grid {
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
                    Text { text: "Easing Curves (700 ms)" },
                    FilledButton {
                      on_pressed: move || anim_sig.set(!anim_sig.peek()),
                      child: Text { text: "Animate" },
                    },
                    Checkbox { value: anim_loop, on_changed: move |v: bool| anim_loop_sig.set(v) },
                    Text { text: "Infinite" },
                  ],
                },
                CurveBar {
                  label: "Linear",
                  target: t_curves,
                  curve: Curve::Linear,
                  color: Color::hex(0x3B82F6),
                  loop_forever: anim_loop
                },
                CurveBar {
                  label: "EaseIn",
                  target: t_curves,
                  curve: Curve::EaseIn,
                  color: Color::hex(0x8B5CF6),
                  loop_forever: anim_loop
                },
                CurveBar {
                  label: "EaseOut",
                  target: t_curves,
                  curve: Curve::EaseOut,
                  color: Color::hex(0x10B981),
                  loop_forever: anim_loop
                },
                CurveBar {
                  label: "EaseInOut",
                  target: t_curves,
                  curve: Curve::EaseInOut,
                  color: Color::hex(0xF59E0B),
                  loop_forever: anim_loop
                },
                CurveBar {
                  label: "FastOutSlowIn",
                  target: t_curves,
                  curve: Curve::FastOutSlowIn,
                  color: Color::hex(0xEF4444),
                  loop_forever: anim_loop
                },
                CurveBar {
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
                    Text { text: "Physics Spring" },
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
                SpringTrack { target: t_spring, loop_forever: spring_loop },
                Text { text: "Infinite Spinners & Indeterminate Progress" },
                Row {
                  spacing: 16.0,
                  cross_axis_alignment: CrossAxisAlignment::Center,
                  children: [
                    Spinner { size: 18.0, stroke_width: 2.0 },
                    Spinner { size: 28.0 },
                    Spinner { size: 38.0, stroke_width: 3.5, period_ms: 1000.0 },
                    CircularProgressIndicator {},
                  ],
                },
                LinearProgressIndicator {},
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
    })
  }
}

impl Element for Gallery {
  fn build(&self, _: &Context) -> Option<Widget> {
    let s = &self.state;
    let tab = s.tab.get();
    let dark = s.dark.get();
    let dialog_open = s.dialog_open.get();
    let status = s.status.get();
    let tab_sig = s.tab.clone();
    let dark_sig = s.dark.clone();
    let clicks_sig = s.clicks.clone();
    let st_fab = s.status.clone();
    let dialog_sig = s.dialog_open.clone();

    let body = match tab {
      0 => self.controls_page(),
      1 => self.inputs_page(),
      2 => self.overlays_page(),
      _ => self.animations_page(),
    };

    let scaffold = ui!(Scaffold {
      app_bar: AppBar {
        title: Text { text: format!("Lobo Gallery — {status}") },
        actions: [IconButton {
          icon: if dark { Icons::STAR } else { Icons::STAR_BORDER },
          on_pressed: move || dark_sig.set(!dark_sig.peek()),
        }],
      },
      floating_action_button: FloatingActionButton {
        on_pressed: move || {
          let next = clicks_sig.peek() + 1;
          clicks_sig.set(next);
          st_fab.set(format!("FAB pressed (clicks: {next})"));
        },
        child: Icon { icon: Icons::ADD },
      },
      bottom_navigation_bar: NavigationBar {
        selected_index: tab,
        on_destination_selected: move |idx: usize| tab_sig.set(idx),
        destinations: [
          NavigationDestination { icon: Icons::WIDGETS, label: "Controls" },
          NavigationDestination { icon: Icons::EDIT, label: "Inputs" },
          NavigationDestination { icon: Icons::MENU, label: "Overlays" },
          NavigationDestination { icon: Icons::PLAY_ARROW, label: "Animations" },
        ],
      },
      body: Container { padding: Padding::all(24.0), child: body },
    });

    let root = if dialog_open {
      ui!(Stack {
        children: [
          scaffold,
          AlertDialog {
            title: Text { text: "Lobo CPU Window" },
            content: Text { text: "Rendered via vello_cpu + softbuffer with partial damage." },
            actions: [TextButton {
              on_pressed: move || dialog_sig.set(false),
              child: Text { text: "OK" },
            }],
          },
        ],
      })
    } else {
      scaffold
    };

    let max_fps = s.max_fps.get();
    Some(ui!(App {
      title: "Lobo Material Gallery",
      theme: ThemeData::light(),
      dark_theme: ThemeData::dark(),
      theme_mode: if dark { ThemeMode::Dark } else { ThemeMode::Light },
      perf_monitor: true,
      max_fps,
      home: root,
    }))
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let state = GalleryState::default();
  let mut args = std::env::args().skip(1);
  if args.next().as_deref() == Some("--png") {
    let out_path = args.next().unwrap_or_else(|| "/tmp/lobo-gallery.png".to_string());
    if let Some(tab_arg) = args.next()
      && let Ok(tab) = tab_arg.parse::<usize>()
    {
      state.tab.set(tab);
    }
    let png = App::render_png(ui!(Gallery { state }), 1280, 720)?;
    std::fs::write(&out_path, png)?;
    println!("rendered 1280x720 gallery to {out_path}");
    return Ok(());
  }
  run_app(ui!(Gallery { state }))
}
