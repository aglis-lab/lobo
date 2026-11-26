use std::num::NonZeroU32;
use std::rc::Rc;

use lobo::*;
use renderer_cpu::PixmapRenderer;
use softbuffer::{Context as SbContext, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

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
      status: Signal::new(String::from("Tab/Shift-Tab: focus | Enter/Space: activate")),
    }
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
                Text { text: "Text Inputs" },
                TextField { controller: s.name.clone(), label: "Project name" },
                TextField {
                  controller: s.note.clone(),
                  variant: TextFieldVariant::Filled,
                  label: "Notes",
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
      _ => self.overlays_page(),
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

    let theme = if dark { ThemeData::dark() } else { ThemeData::light() };
    Some(ui!(Theme { data: theme, child: root }))
  }
}

struct GuiApp {
  window: Option<Rc<Window>>,
  _sb_context: Option<SbContext<winit::event_loop::OwnedDisplayHandle>>,
  surface: Option<Surface<winit::event_loop::OwnedDisplayHandle, Rc<Window>>>,
  surface_size: Option<(u32, u32)>,
  renderer: PixmapRenderer,
  scene: Scene,
  root: Rc<Node>,
  modifiers: ModifiersState,
  needs_present: bool,
}

impl GuiApp {
  fn new(root: Rc<Node>) -> Self {
    GuiApp {
      window: None,
      _sb_context: None,
      surface: None,
      surface_size: None,
      renderer: PixmapRenderer::hd720(),
      scene: Scene::new(),
      root,
      modifiers: ModifiersState::empty(),
      needs_present: true,
    }
  }

  fn sync_size(&mut self) {
    let Some(window) = &self.window else { return };
    let phys = window.inner_size();
    if phys.width == 0 || phys.height == 0 {
      return;
    }
    let scale = window.scale_factor() as f32;
    let logical_w = (phys.width as f32 / scale).max(1.0);
    let logical_h = (phys.height as f32 / scale).max(1.0);
    if self.renderer.resize_with_scale(logical_w, logical_h, scale) {
      self.scene.invalidate();
      self.needs_present = true;
    }
    let target = (u32::from(self.renderer.width()), u32::from(self.renderer.height()));
    if self.surface_size != Some(target) {
      self.needs_present = true;
    }
  }

  fn present_surface(&mut self) {
    let (Some(surface), Some(window)) = (&mut self.surface, &self.window) else { return };
    let w = NonZeroU32::new(u32::from(self.renderer.width()));
    let h = NonZeroU32::new(u32::from(self.renderer.height()));
    let (Some(w), Some(h)) = (w, h) else { return };
    let target = (w.get(), h.get());
    if self.surface_size != Some(target) {
      if let Err(err) = surface.resize(w, h) {
        eprintln!("softbuffer resize failed: {err}");
        return;
      }
      self.surface_size = Some(target);
    }
    match surface.buffer_mut() {
      Ok(mut buffer) => {
        for (dst, px) in buffer.iter_mut().zip(self.renderer.pixmap().data()) {
          *dst = (u32::from(px.r) << 16) | (u32::from(px.g) << 8) | u32::from(px.b);
        }
        window.pre_present_notify();
        if let Err(err) = buffer.present() {
          eprintln!("softbuffer present failed: {err}");
        }
      }
      Err(err) => eprintln!("softbuffer buffer_mut failed: {err}"),
    }
  }
}

impl ApplicationHandler<()> for GuiApp {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    if self.window.is_some() {
      return;
    }
    let attrs = Window::default_attributes()
      .with_title("Lobo Material Gallery (CPU)")
      .with_inner_size(LogicalSize::new(1280.0, 720.0));
    let window = Rc::new(event_loop.create_window(attrs).expect("create window"));
    let sb_ctx = SbContext::new(event_loop.owned_display_handle()).expect("softbuffer context");
    let surface = Surface::new(&sb_ctx, window.clone()).expect("softbuffer surface");
    self.window = Some(window.clone());
    self._sb_context = Some(sb_ctx);
    self.surface = Some(surface);
    self.sync_size();
    window.request_redraw();
  }

  fn user_event(&mut self, _event_loop: &ActiveEventLoop, _: ()) {
    if let Some(window) = &self.window {
      window.request_redraw();
    }
  }

  fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
    match event {
      WindowEvent::CloseRequested => event_loop.exit(),
      WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
        self.sync_size();
        if let Some(window) = &self.window {
          window.request_redraw();
        }
      }
      WindowEvent::ModifiersChanged(mods) => {
        self.modifiers = mods.state();
      }
      WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
        if is_quit(&event, self.modifiers) {
          event_loop.exit();
          return;
        }
        if let Some(key) = map_winit_key(&event, self.modifiers) {
          dispatch_event(&key);
          self.root.update();
          if let Some(window) = &self.window {
            window.request_redraw();
          }
        }
      }
      WindowEvent::RedrawRequested => {
        self.root.update();
        let dmg = self.root.present(&mut self.scene, &mut self.renderer);
        if dmg.is_some() || self.needs_present {
          self.present_surface();
          self.needs_present = false;
        }
      }
      _ => {}
    }
  }
}

fn is_quit(event: &KeyEvent, mods: ModifiersState) -> bool {
  let Key::Character(ref s) = event.logical_key else { return false };
  let ch = s.chars().next();
  (mods.super_key() && matches!(ch, Some('q' | 'Q')))
    || (mods.control_key() && matches!(ch, Some('c' | 'C' | 'q' | 'Q')))
}

fn map_winit_key(event: &KeyEvent, mods: ModifiersState) -> Option<EventKey> {
  let m = Modifiers { ctrl: mods.control_key(), alt: mods.alt_key(), shift: mods.shift_key() };
  let ch = match &event.logical_key {
    Key::Named(NamedKey::Tab) => {
      return Some(EventKey::with_modifiers(
        if mods.shift_key() { keys::BACK_TAB } else { keys::TAB },
        Modifiers { shift: false, ..m },
      ));
    }
    Key::Named(NamedKey::Enter) => keys::ENTER,
    Key::Named(NamedKey::Escape) => keys::ESCAPE,
    Key::Named(NamedKey::Backspace) => keys::BACKSPACE,
    Key::Named(NamedKey::Delete) => keys::DELETE,
    Key::Named(NamedKey::ArrowUp) => keys::UP,
    Key::Named(NamedKey::ArrowDown) => keys::DOWN,
    Key::Named(NamedKey::ArrowLeft) => keys::LEFT,
    Key::Named(NamedKey::ArrowRight) => keys::RIGHT,
    Key::Named(NamedKey::Space) => ' ',
    Key::Character(s) => {
      let c = s.chars().next()?;
      if c.is_control() {
        return None;
      }
      c
    }
    _ => return None,
  };
  Some(EventKey::with_modifiers(ch, m))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut args = std::env::args().skip(1);
  if args.next().as_deref() == Some("--png") {
    let out_path = args.next().unwrap_or_else(|| "/tmp/lobo-gallery.png".to_string());
    let mut renderer = PixmapRenderer::hd720();
    let mut scene = Scene::new();
    let root = Node::mount(ui!(Gallery { state: GalleryState::default() }), Context::default());
    root.present(&mut scene, &mut renderer);
    std::fs::write(&out_path, renderer.to_png()?)?;
    println!("rendered {}x{} gallery to {out_path}", renderer.width(), renderer.height());
    return Ok(());
  }

  let event_loop = EventLoop::<()>::with_user_event().build()?;
  let proxy = event_loop.create_proxy();
  set_redraw_waker(Some(Rc::new(move || {
    let _ = proxy.send_event(());
  })));

  let root = Node::mount(ui!(Gallery { state: GalleryState::default() }), Context::default());
  let mut app = GuiApp::new(root);
  event_loop.run_app(&mut app)?;
  set_redraw_waker(None);
  Ok(())
}
