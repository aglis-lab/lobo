//! Top-level [`App`] widget (Flutter's `MaterialApp` equivalent) and internal window runner.
//!
//! Configures the active [`ThemeData`] (`theme`, `dark_theme`, `theme_mode`),
//! application `title`, and debug `perf_monitor` toggle. The OS event loop is
//! completely encapsulated inside [`App::run_widget`] / [`run_app`] and never
//! exposed to user code.

use std::cell::{Cell, RefCell};

use crate::theme::{Theme, ThemeData};
use crate::{Context, Element, Widget, ui};

/// Selects which theme [`App`] applies to its subtree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
  #[default]
  Light,
  Dark,
}

thread_local! {
  static APP_TITLE: RefCell<String> = const { RefCell::new(String::new()) };
  static APP_MAX_FPS: Cell<Option<u32>> = const { Cell::new(None) };
}

fn set_app_max_fps(max_fps: u32) {
  let cap = if max_fps == 0 { None } else { Some(max_fps.clamp(1, 240)) };
  APP_MAX_FPS.with(|f| f.set(cap));
}

/// Returns the active [`App::max_fps`] cap, if configured.
#[inline]
pub fn current_max_fps() -> Option<u32> {
  APP_MAX_FPS.with(Cell::get)
}

#[cfg(debug_assertions)]
thread_local! {
  static PERF_MONITOR_ENABLED: Cell<bool> = const { Cell::new(false) };
  static PERF_MONITOR_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
  static SHOW_DAMAGE_ENABLED: Cell<bool> = const { Cell::new(false) };
  static SHOW_DAMAGE_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Returns whether the active [`App`] has `perf_monitor` enabled.
///
/// Always returns `false` in release builds (`not(debug_assertions)`).
#[inline]
pub fn is_perf_monitor_enabled() -> bool {
  #[cfg(debug_assertions)]
  {
    PERF_MONITOR_OVERRIDE
      .with(Cell::get)
      .unwrap_or_else(|| PERF_MONITOR_ENABLED.with(Cell::get))
  }
  #[cfg(not(debug_assertions))]
  {
    false
  }
}

/// Returns whether the debug damage/repaint visualizer is enabled (`App::show_damage` or `F4`).
///
/// Always returns `false` in release builds (`not(debug_assertions)`).
#[inline]
pub fn is_show_damage_enabled() -> bool {
  #[cfg(debug_assertions)]
  {
    SHOW_DAMAGE_OVERRIDE
      .with(Cell::get)
      .unwrap_or_else(|| SHOW_DAMAGE_ENABLED.with(Cell::get))
  }
  #[cfg(not(debug_assertions))]
  {
    false
  }
}

#[inline]
fn set_perf_monitor_enabled(enabled: bool) {
  #[cfg(debug_assertions)]
  {
    PERF_MONITOR_ENABLED.with(|f| f.set(enabled));
  }
  #[cfg(not(debug_assertions))]
  {
    let _ = enabled;
  }
}

#[inline]
fn set_show_damage_enabled(enabled: bool) {
  #[cfg(debug_assertions)]
  {
    SHOW_DAMAGE_ENABLED.with(|f| f.set(enabled));
  }
  #[cfg(not(debug_assertions))]
  {
    let _ = enabled;
  }
}

#[cfg(all(feature = "cpu", debug_assertions))]
fn toggle_perf_monitor_override() {
  let next = !is_perf_monitor_enabled();
  PERF_MONITOR_OVERRIDE.with(|o| o.set(Some(next)));
}

#[cfg(all(feature = "cpu", debug_assertions))]
fn toggle_show_damage_override() {
  let next = !is_show_damage_enabled();
  SHOW_DAMAGE_OVERRIDE.with(|o| o.set(Some(next)));
}

fn set_app_title(title: &str) {
  if !title.is_empty() {
    APP_TITLE.with(|t| {
      let mut cur = t.borrow_mut();
      if cur.as_str() != title {
        cur.clear();
        cur.push_str(title);
      }
    });
  }
}

#[cfg(feature = "cpu")]
fn current_app_title() -> String {
  APP_TITLE.with(|t| {
    let s = t.borrow();
    if s.is_empty() { String::from("Lobo App") } else { s.clone() }
  })
}

/// Top-level application widget providing [`ThemeData`] and debug performance monitoring.
///
/// ```ignore
/// run_app(ui!(App {
///   title: "My App",
///   theme: ThemeData::light(),
///   dark_theme: ThemeData::dark(),
///   theme_mode: ThemeMode::Light,
///   perf_monitor: true,
///   home: my_root_widget,
/// }))
/// ```
#[derive(Widget)]
#[non_exhaustive]
pub struct App {
  /// Application / window title.
  #[widget(optional)]
  pub title: String,
  /// Light theme (defaults to [`ThemeData::light`]).
  #[widget(optional)]
  pub theme: Option<ThemeData>,
  /// Dark theme (defaults to [`ThemeData::dark`] when `theme_mode` is [`ThemeMode::Dark`]).
  #[widget(optional)]
  pub dark_theme: Option<ThemeData>,
  /// Chooses between `theme` and `dark_theme`.
  #[widget(optional)]
  pub theme_mode: ThemeMode,
  /// Turns on the separate-renderer performance overlay in debug builds.
  /// Completely stripped out in release builds.
  #[widget(optional)]
  pub perf_monitor: bool,
  /// Highlights redrawn damage regions on screen in debug builds (also toggleable with `F4`).
  /// Completely stripped out in release builds.
  #[widget(optional)]
  pub show_damage: bool,
  /// Optional maximum frame rate cap in FPS (e.g. `max_fps: 60` or `max_fps: 30`).
  ///
  /// `0` (the default) automatically paces animations to the monitor's refresh rate.
  /// Any non-zero value caps all frame redraws to at most `max_fps` FPS (`1..=240`)
  /// to keep CPU usage bounded.
  #[widget(optional)]
  pub max_fps: u32,
  /// Root widget of the application.
  pub home: Widget,
}

impl App {
  /// Resolves the active [`ThemeData`] for the current `theme_mode`.
  pub fn resolved_theme(&self) -> ThemeData {
    match self.theme_mode {
      ThemeMode::Light => self.theme.clone().unwrap_or_else(ThemeData::light),
      ThemeMode::Dark => self.dark_theme.clone().unwrap_or_else(ThemeData::dark),
    }
  }

  /// Runs a root widget (such as `ui!(App { ... })` or a widget that builds `App`)
  /// in a native desktop window. The OS event loop is managed internally.
  #[cfg(feature = "cpu")]
  pub fn run_widget(widget: Widget) -> Result<(), Box<dyn std::error::Error>> {
    runner::run(widget)
  }

  /// Renders a root widget headlessly to PNG bytes at `width` x `height` (1x scale).
  #[cfg(feature = "cpu")]
  pub fn render_png(
    widget: Widget,
    width: u16,
    height: u16,
  ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    runner::render_png(widget, width, height)
  }
}

/// Convenience entry point: runs `widget` in a native desktop window.
#[cfg(feature = "cpu")]
pub fn run_app(widget: Widget) -> Result<(), Box<dyn std::error::Error>> {
  App::run_widget(widget)
}

impl Element for App {
  fn build(&self, _context: &Context) -> Option<Widget> {
    set_app_title(&self.title);
    set_perf_monitor_enabled(self.perf_monitor);
    set_show_damage_enabled(self.show_damage);
    set_app_max_fps(self.max_fps);
    let data = self.resolved_theme();
    Some(ui!(Theme { data, child: self.home.clone() }))
  }
}

// ---------------------------------------------------------------------------
// Internal desktop window runner (never exposed to user code)
// ---------------------------------------------------------------------------

#[cfg(feature = "cpu")]
mod runner {
  #[cfg(not(target_os = "macos"))]
  use std::num::NonZeroU32;
  use std::rc::Rc;
  use std::time::{Duration, Instant};

  use renderer_cpu::PixmapRenderer;
  #[cfg(not(target_os = "macos"))]
  use softbuffer::{Context as SbContext, Surface};
  use winit::application::ApplicationHandler;
  use winit::dpi::LogicalSize;
  use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
  use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
  use winit::keyboard::{Key, ModifiersState, NamedKey};
  use winit::window::{Window, WindowId};

  #[cfg(target_os = "macos")]
  use crate::present::MacPresenter;

  #[cfg(debug_assertions)]
  use super::{is_show_damage_enabled, toggle_perf_monitor_override, toggle_show_damage_override};
  use super::{current_app_title, current_max_fps};
  #[cfg(debug_assertions)]
  use crate::{PERF_OVERLAY_SIZE, Rect};
  use crate::{
    Context, EventKey, Modifiers, Node, Offset, PerfLayer, PointerButton, PointerEvent, Scene,
    Widget, dispatch_event, dispatch_pointer, keys, set_redraw_waker,
  };

  pub(super) fn run(widget: Widget) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<()>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    set_redraw_waker(Some(Rc::new(move || {
      let _ = proxy.send_event(());
    })));

    let root = Node::mount(widget, Context::default());
    let mut host = WindowHost::new(root);
    event_loop.run_app(&mut host)?;
    set_redraw_waker(None);
    Ok(())
  }

  pub(super) fn render_png(
    widget: Widget,
    width: u16,
    height: u16,
  ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut renderer = PixmapRenderer::new(width, height);
    let mut scene = Scene::new();
    let mut perf = PerfLayer::new();
    let root = Node::mount(widget, Context::default());
    perf.present_app(&root, &mut scene, &mut renderer, 0.0);
    #[cfg(debug_assertions)]
    {
      let mut perf_renderer =
        PixmapRenderer::with_scale(PERF_OVERLAY_SIZE.width, PERF_OVERLAY_SIZE.height, 1.0);
      if perf.render_overlay(0.0, &mut perf_renderer) {
        let ox =
          usize::from(renderer.width()).saturating_sub(usize::from(perf_renderer.width()) + 16);
        renderer.composite_over(&perf_renderer, ox, 72);
      }
    }
    Ok(renderer.to_png()?)
  }

  struct WindowHost {
    window: Option<Rc<Window>>,
    #[cfg(target_os = "macos")]
    mac_presenter: Option<MacPresenter>,
    #[cfg(not(target_os = "macos"))]
    _sb_context: Option<SbContext<winit::event_loop::OwnedDisplayHandle>>,
    #[cfg(not(target_os = "macos"))]
    surface: Option<Surface<winit::event_loop::OwnedDisplayHandle, Rc<Window>>>,
    #[cfg(not(target_os = "macos"))]
    surface_size: Option<(u32, u32)>,
    renderer: PixmapRenderer,
    scene: Scene,
    root: Rc<Node>,
    perf: PerfLayer,
    #[cfg(debug_assertions)]
    perf_renderer: PixmapRenderer,
    #[cfg(debug_assertions)]
    next_perf_tick: Instant,
    next_anim_tick: Option<Instant>,
    last_frame_at: Option<Instant>,
    pending_redraw: bool,
    #[cfg(debug_assertions)]
    damage_flashes: Vec<(Rect, Instant)>,
    last_present_ms: f32,
    cursor_pos: Offset,
    modifiers: ModifiersState,
    needs_present: bool,
  }

  impl WindowHost {
    #[cfg(debug_assertions)]
    const PERF_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
    #[cfg(debug_assertions)]
    const DAMAGE_FLASH_DURATION: Duration = Duration::from_millis(400);

    fn new(root: Rc<Node>) -> Self {
      WindowHost {
        window: None,
        #[cfg(target_os = "macos")]
        mac_presenter: None,
        #[cfg(not(target_os = "macos"))]
        _sb_context: None,
        #[cfg(not(target_os = "macos"))]
        surface: None,
        #[cfg(not(target_os = "macos"))]
        surface_size: None,
        renderer: PixmapRenderer::hd720(),
        scene: Scene::new(),
        root,
        perf: PerfLayer::new(),
        #[cfg(debug_assertions)]
        perf_renderer: PixmapRenderer::with_scale(
          PERF_OVERLAY_SIZE.width,
          PERF_OVERLAY_SIZE.height,
          1.0,
        ),
        #[cfg(debug_assertions)]
        next_perf_tick: Instant::now() + Self::PERF_REFRESH_INTERVAL,
        next_anim_tick: None,
        last_frame_at: None,
        pending_redraw: false,
        #[cfg(debug_assertions)]
        damage_flashes: Vec::new(),
        last_present_ms: 0.0,
        cursor_pos: Offset::ZERO,
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
      #[cfg(debug_assertions)]
      if self.perf_renderer.resize_with_scale(
        PERF_OVERLAY_SIZE.width,
        PERF_OVERLAY_SIZE.height,
        scale,
      ) {
        self.perf.invalidate();
      }
      #[cfg(not(target_os = "macos"))]
      {
        let target = (u32::from(self.renderer.width()), u32::from(self.renderer.height()));
        if self.surface_size != Some(target) {
          self.needs_present = true;
        }
      }
    }

    fn frame_interval(&self) -> Duration {
      if let Some(fps) = current_max_fps() {
        return Duration::from_secs_f64(1.0 / f64::from(fps.clamp(1, 240)));
      }
      let mhz = self
        .window
        .as_ref()
        .and_then(|w| w.current_monitor())
        .and_then(|m| m.refresh_rate_millihertz())
        .unwrap_or(60_000)
        .clamp(30_000, 240_000);
      Duration::from_secs_f64(1000.0 / f64::from(mhz))
    }

    #[cfg(target_os = "macos")]
    fn present_surface(&mut self, show_overlay: bool) {
      let (Some(presenter), Some(window)) = (&mut self.mac_presenter, &self.window) else { return };
      let dst_w = usize::from(self.renderer.width());
      let dst_h = usize::from(self.renderer.height());
      let scale = self.renderer.scale();
      window.pre_present_notify();
      presenter.present_bgra8(dst_w, dst_h, f64::from(scale), |buf, stride| {
        self.renderer.copy_bgra8_to(buf, stride);
        #[cfg(debug_assertions)]
        {
          if is_show_damage_enabled() && !self.damage_flashes.is_empty() {
            let now = Instant::now();
            self
              .damage_flashes
              .retain(|(_, t)| now.duration_since(*t) < Self::DAMAGE_FLASH_DURATION);
            for &(rect, started) in &self.damage_flashes {
              let frac = 1.0
                - (now.duration_since(started).as_secs_f32()
                  / Self::DAMAGE_FLASH_DURATION.as_secs_f32())
                  .clamp(0.0, 1.0);
              draw_damage_flash_bgra8(buf, dst_w, dst_h, stride, rect, scale, frac);
            }
            if !self.damage_flashes.is_empty() {
              crate::anim::request_frame();
            }
          }
          if show_overlay {
            let ox = dst_w.saturating_sub(
              usize::from(self.perf_renderer.width()) + (16.0 * scale).round() as usize,
            );
            let oy = (72.0 * scale).round() as usize;
            self.perf_renderer.blit_overlay_to_bgra8(buf, dst_w, dst_h, stride, ox, oy);
          }
        }
        #[cfg(not(debug_assertions))]
        let _ = show_overlay;
      });
    }

    #[cfg(not(target_os = "macos"))]
    fn present_surface(&mut self, show_overlay: bool) {
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
          self.renderer.copy_to_xrgb8(&mut buffer);
          #[cfg(debug_assertions)]
          {
            let dst_w = w.get() as usize;
            let dst_h = h.get() as usize;
            let scale = self.renderer.scale();
            if is_show_damage_enabled() && !self.damage_flashes.is_empty() {
              let now = Instant::now();
              self.damage_flashes.retain(|(_, t)| now.duration_since(*t) < Self::DAMAGE_FLASH_DURATION);
              for &(rect, started) in &self.damage_flashes {
                let frac = 1.0
                  - (now.duration_since(started).as_secs_f32()
                    / Self::DAMAGE_FLASH_DURATION.as_secs_f32())
                    .clamp(0.0, 1.0);
                draw_damage_flash_xrgb8(&mut buffer, dst_w, dst_h, rect, scale, frac);
              }
              if !self.damage_flashes.is_empty() {
                crate::anim::request_frame();
              }
            }
            if show_overlay {
              let ox = dst_w.saturating_sub(
                usize::from(self.perf_renderer.width()) + (16.0 * scale).round() as usize,
              );
              let oy = (72.0 * scale).round() as usize;
              self.perf_renderer.blit_overlay_to_xrgb8(&mut buffer, dst_w, dst_h, ox, oy);
            }
          }
          #[cfg(not(debug_assertions))]
          let _ = show_overlay;
          window.pre_present_notify();
          if let Err(err) = buffer.present() {
            eprintln!("softbuffer present failed: {err}");
          }
        }
        Err(err) => eprintln!("softbuffer buffer_mut failed: {err}"),
      }
    }
  }

  impl ApplicationHandler<()> for WindowHost {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
      if self.window.is_some() {
        return;
      }
      let attrs = Window::default_attributes()
        .with_title(current_app_title())
        .with_inner_size(LogicalSize::new(1280.0, 720.0));
      let window = Rc::new(event_loop.create_window(attrs).expect("create window"));
      #[cfg(target_os = "macos")]
      {
        self.mac_presenter = Some(MacPresenter::new(&window).expect("macOS CALayer presenter"));
      }
      #[cfg(not(target_os = "macos"))]
      {
        let sb_ctx = SbContext::new(event_loop.owned_display_handle()).expect("softbuffer context");
        let surface = Surface::new(&sb_ctx, window.clone()).expect("softbuffer surface");
        self._sb_context = Some(sb_ctx);
        self.surface = Some(surface);
      }
      self.window = Some(window.clone());
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
          if let Some(window) = &self.window {
            window.request_redraw();
          }
        }
        WindowEvent::ModifiersChanged(mods) => {
          self.modifiers = mods.state();
        }
        WindowEvent::CursorMoved { position, .. } => {
          let scale = self.renderer.scale();
          let pos = Offset::new(position.x as f32 / scale, position.y as f32 / scale);
          self.cursor_pos = pos;
          if dispatch_pointer(PointerEvent::Move(pos)) {
            self.root.update();
            if let Some(window) = &self.window {
              window.request_redraw();
            }
          }
        }
        WindowEvent::CursorLeft { .. } => {
          if dispatch_pointer(PointerEvent::Leave) {
            self.root.update();
            if let Some(window) = &self.window {
              window.request_redraw();
            }
          }
        }
        WindowEvent::MouseInput { state, button, .. } => {
          let btn = match button {
            MouseButton::Left => PointerButton::Primary,
            MouseButton::Right => PointerButton::Secondary,
            MouseButton::Middle => PointerButton::Middle,
            _ => return,
          };
          let ev = match state {
            ElementState::Pressed => PointerEvent::Down { pos: self.cursor_pos, button: btn },
            ElementState::Released => PointerEvent::Up { pos: self.cursor_pos, button: btn },
          };
          if dispatch_pointer(ev) {
            self.root.update();
            if let Some(window) = &self.window {
              window.request_redraw();
            }
          }
        }
        WindowEvent::MouseWheel { delta, .. } => {
          let scale = self.renderer.scale();
          let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => -y * 36.0,
            MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / scale,
          };
          if dispatch_pointer(PointerEvent::Scroll { pos: self.cursor_pos, delta_y }) {
            self.root.update();
            if let Some(window) = &self.window {
              window.request_redraw();
            }
          }
        }
        WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
          if is_quit(&event, self.modifiers) {
            event_loop.exit();
            return;
          }
          #[cfg(debug_assertions)]
          if matches!(event.logical_key, Key::Named(NamedKey::F3)) {
            toggle_perf_monitor_override();
            self.needs_present = true;
            if let Some(window) = &self.window {
              window.request_redraw();
            }
            return;
          }
          #[cfg(debug_assertions)]
          if matches!(event.logical_key, Key::Named(NamedKey::F4)) {
            toggle_show_damage_override();
            if !is_show_damage_enabled() {
              self.damage_flashes.clear();
            }
            self.needs_present = true;
            if let Some(window) = &self.window {
              window.request_redraw();
            }
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
          if let Some(fps) = current_max_fps()
            && let Some(last) = self.last_frame_at
          {
            let min_interval = Duration::from_secs_f64(1.0 / f64::from(fps.clamp(1, 240)));
            if last.elapsed() < min_interval {
              self.pending_redraw = true;
              return;
            }
          }
          let frame_start = Instant::now();
          self.last_frame_at = Some(frame_start);
          self.pending_redraw = false;

          self.sync_size();
          let t_update = Instant::now();
          self.root.update();
          let update_ms = t_update.elapsed().as_secs_f32() * 1000.0;

          let dmg =
            self.perf.present_app(&self.root, &mut self.scene, &mut self.renderer, update_ms);
          #[cfg(debug_assertions)]
          if let Some(rect) = dmg
            && is_show_damage_enabled()
          {
            if self.damage_flashes.len() >= 16 {
              self.damage_flashes.remove(0);
            }
            self.damage_flashes.push((rect, Instant::now()));
          }
          #[cfg(debug_assertions)]
          let has_flashes = is_show_damage_enabled() && !self.damage_flashes.is_empty();
          #[cfg(not(debug_assertions))]
          let has_flashes = false;
          if dmg.is_some() || self.needs_present || has_flashes {
            #[cfg(debug_assertions)]
            let show_overlay =
              self.perf.render_overlay(self.last_present_ms, &mut self.perf_renderer);
            #[cfg(not(debug_assertions))]
            let show_overlay = false;

            let t_present = Instant::now();
            self.present_surface(show_overlay);
            self.last_present_ms = t_present.elapsed().as_secs_f32() * 1000.0;
            self.needs_present = false;
          }
          if crate::anim::take_frame_request() {
            self.next_anim_tick = Some(frame_start + self.frame_interval());
          }
        }
        _ => {}
      }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
      let now = Instant::now();
      let mut earliest_wake: Option<Instant> = None;

      if self.pending_redraw
        && let Some(last) = self.last_frame_at
      {
        let wake = last + self.frame_interval();
        if now >= wake {
          self.pending_redraw = false;
          if let Some(window) = &self.window {
            window.request_redraw();
          }
        } else {
          earliest_wake = Some(wake);
        }
      }

      if let Some(anim_tick) = self.next_anim_tick {
        if now >= anim_tick {
          self.next_anim_tick = None;
          if let Some(window) = &self.window {
            window.request_redraw();
          }
        } else {
          earliest_wake = Some(match earliest_wake {
            Some(t) => t.min(anim_tick),
            None => anim_tick,
          });
        }
      }

      #[cfg(debug_assertions)]
      {
        if self.perf.is_enabled() {
          if now >= self.next_perf_tick {
            self.next_perf_tick = now + Self::PERF_REFRESH_INTERVAL;
            if self.perf.refresh_live(&mut self.perf_renderer) && self.next_anim_tick.is_none() {
              self.needs_present = true;
              if let Some(window) = &self.window {
                window.request_redraw();
              }
            }
          }
          earliest_wake = Some(match earliest_wake {
            Some(t) => t.min(self.next_perf_tick),
            None => self.next_perf_tick,
          });
        }
      }

      if let Some(wake) = earliest_wake {
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
      } else {
        event_loop.set_control_flow(ControlFlow::Wait);
      }
    }
  }

  #[cfg(all(debug_assertions, target_os = "macos"))]
  fn draw_damage_flash_bgra8(
    dst: &mut [u8],
    dst_w: usize,
    dst_h: usize,
    dst_stride: usize,
    rect: Rect,
    scale: f32,
    alpha_scale: f32,
  ) {
    let x0 = ((rect.x * scale).floor() as isize).clamp(0, dst_w as isize) as usize;
    let y0 = ((rect.y * scale).floor() as isize).clamp(0, dst_h as isize) as usize;
    let x1 = ((rect.right() * scale).ceil() as isize).clamp(0, dst_w as isize) as usize;
    let y1 = ((rect.bottom() * scale).ceil() as isize).clamp(0, dst_h as isize) as usize;
    if x1 <= x0 || y1 <= y0 {
      return;
    }
    let border = (2.0 * scale).round().max(1.0) as usize;
    let fill_a = (36.0 * alpha_scale).round() as u16;
    let border_a = (220.0 * alpha_scale).round() as u16;
    let (sr, sg, sb) = (244u16, 63u16, 94u16);
    for y in y0..y1 {
      let on_v_edge = y < y0 + border || y + border >= y1;
      let row_start = y * dst_stride + x0 * 4;
      let row_end = row_start + (x1 - x0) * 4;
      if row_end > dst.len() {
        break;
      }
      let row = &mut dst[row_start..row_end];
      for (x, px) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let abs_x = x0 + x;
        let on_edge = on_v_edge || abs_x < x0 + border || abs_x + border >= x1;
        let a = if on_edge { border_a } else { fill_a };
        if a == 0 {
          continue;
        }
        let inv = 255 - a;
        px[0] = ((sb * a + u16::from(px[0]) * inv) / 255) as u8;
        px[1] = ((sg * a + u16::from(px[1]) * inv) / 255) as u8;
        px[2] = ((sr * a + u16::from(px[2]) * inv) / 255) as u8;
        px[3] = 255;
      }
    }
  }

  #[cfg(all(debug_assertions, not(target_os = "macos")))]
  fn draw_damage_flash_xrgb8(
    dst: &mut [u32],
    dst_w: usize,
    dst_h: usize,
    rect: Rect,
    scale: f32,
    alpha_scale: f32,
  ) {
    let x0 = ((rect.x * scale).floor() as isize).clamp(0, dst_w as isize) as usize;
    let y0 = ((rect.y * scale).floor() as isize).clamp(0, dst_h as isize) as usize;
    let x1 = ((rect.right() * scale).ceil() as isize).clamp(0, dst_w as isize) as usize;
    let y1 = ((rect.bottom() * scale).ceil() as isize).clamp(0, dst_h as isize) as usize;
    if x1 <= x0 || y1 <= y0 {
      return;
    }
    let border = (2.0 * scale).round().max(1.0) as usize;
    let fill_a = (36.0 * alpha_scale).round() as u32;
    let border_a = (220.0 * alpha_scale).round() as u32;
    // Vibrant rose/magenta highlight (#F43F5E) so redrawn regions stand out on light & dark themes.
    let (sr, sg, sb) = (244u32, 63u32, 94u32);
    for y in y0..y1 {
      let on_v_edge = y < y0 + border || y + border >= y1;
      let row = &mut dst[y * dst_w..(y + 1) * dst_w];
      for (x, px) in row[x0..x1].iter_mut().enumerate() {
        let abs_x = x0 + x;
        let on_edge = on_v_edge || abs_x < x0 + border || abs_x + border >= x1;
        let a = if on_edge { border_a } else { fill_a };
        if a == 0 {
          continue;
        }
        let inv = 255 - a;
        let bg = *px;
        let dr = (bg >> 16) & 0xFF;
        let dg = (bg >> 8) & 0xFF;
        let db = bg & 0xFF;
        let r = (sr * a + dr * inv) / 255;
        let g = (sg * a + dg * inv) / 255;
        let b = (sb * a + db * inv) / 255;
        *px = (r << 16) | (g << 8) | b;
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
    let m = Modifiers {
      ctrl: mods.control_key(),
      alt: mods.alt_key(),
      shift: mods.shift_key(),
      meta: mods.super_key(),
    };
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
}
