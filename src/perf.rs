//! Performance telemetry (`FrameTimings`, `PerfStats`, `PerfMonitor`), HUD widget
//! (`PerfOverlay`), and separate-renderer host (`PerfLayer`).
//!
//! In release builds (`not(debug_assertions)`), [`PerfLayer`] compiles down to a
//! zero-sized no-op and all monitoring/overlay code is stripped completely.

use crate::render::Size;
#[cfg(not(debug_assertions))]
use crate::render::{Rect, Renderer, Scene};
#[cfg(not(debug_assertions))]
use crate::tree::Node;

/// Logical size of the standalone performance overlay surface.
pub const PERF_OVERLAY_SIZE: Size = Size::new(268.0, 204.0);

#[cfg(debug_assertions)]
mod inner {
  use std::collections::VecDeque;
  use std::rc::Rc;
  use std::time::{Duration, Instant};

  use super::PERF_OVERLAY_SIZE;
  use crate::layout::{Constraints, LayoutCx, PaintCx};
  use crate::render::{Border, Color, Offset, Rect, Renderer, Scene, Size, TextPaint};
  use crate::signal::Signal;
  use crate::theme::{ColorScheme, ThemeData};
  use crate::tree::Node;
  use crate::{Context, Element, Widget, ui};

  // -------------------------------------------------------------------------
  // Timings & stats
  // -------------------------------------------------------------------------

  /// Breakdown of one rendered frame's phases, in milliseconds.
  #[derive(Clone, Copy, Debug, Default, PartialEq)]
  pub struct FrameTimings {
    /// Signal reconciliation and widget rebuild (`Node::update`).
    pub update_ms: f32,
    /// Tree layout pass (`Node::layout`).
    pub layout_ms: f32,
    /// Retained display-list recording (`Node::walk_paint`).
    pub paint_ms: f32,
    /// Scene diffing, damage computation, and flattening.
    pub scene_ms: f32,
    /// Backend rasterization (`backend.draw_batch`).
    pub raster_ms: f32,
    /// Surface presentation / pixel blit (e.g. `softbuffer`).
    pub present_ms: f32,
    /// Number of draw commands submitted to the backend.
    pub cmd_count: usize,
  }

  impl FrameTimings {
    /// Total measured frame time across all phases, in milliseconds.
    pub fn total_ms(&self) -> f32 {
      self.update_ms
        + self.layout_ms
        + self.paint_ms
        + self.scene_ms
        + self.raster_ms
        + self.present_ms
    }

    /// All phases in pipeline order: `(label, ms, color)`.
    pub fn phases(&self) -> [(&'static str, f32, Color); 6] {
      [
        ("Update", self.update_ms, Color::hex(0x26C6DA)),
        ("Layout", self.layout_ms, Color::hex(0x42A5F5)),
        ("Paint", self.paint_ms, Color::hex(0xAB47BC)),
        ("Scene", self.scene_ms, Color::hex(0xFFA726)),
        ("Raster", self.raster_ms, Color::hex(0xEF5350)),
        ("Present", self.present_ms, Color::hex(0x66BB6A)),
      ]
    }

    /// Name and duration (ms) of the slowest phase in this frame.
    pub fn bottleneck(&self) -> (&'static str, f32) {
      let mut best = ("Update", self.update_ms);
      for (name, ms, _) in self.phases() {
        if ms > best.1 {
          best = (name, ms);
        }
      }
      best
    }
  }

  /// Snapshot of runtime performance metrics displayed by [`PerfOverlay`].
  #[derive(Clone, Copy, Debug, Default, PartialEq)]
  pub struct PerfStats {
    /// Measured burst FPS during continuous rendering, or estimated capable FPS
    /// (`1000 / total_ms`) for isolated on-demand frames.
    pub fps: f32,
    /// `true` when `fps` is measured from a continuous frame burst; `false` when
    /// estimated from a single on-demand frame.
    pub fps_from_burst: bool,
    /// Process resident memory (RSS) in megabytes, if supported on this platform.
    pub memory_mb: Option<f32>,
    /// Process CPU usage percentage (can exceed 100% on multi-core workloads), if supported.
    pub cpu_percent: Option<f32>,
    /// GPU utilization percentage (0..=100), or `None` when using a CPU backend / unavailable.
    pub gpu_percent: Option<f32>,
    /// Per-phase timing breakdown of the most recent rendered frame.
    pub timings: FrameTimings,
  }

  // -------------------------------------------------------------------------
  // PerfMonitor
  // -------------------------------------------------------------------------

  /// Stateful sampler that tracks FPS, process CPU/memory usage, and frame timings.
  pub struct PerfMonitor {
    burst_times: VecDeque<Instant>,
    last_sample: Instant,
    last_cpu_secs: Option<f64>,
    memory_mb: Option<f32>,
    cpu_percent: Option<f32>,
    gpu_percent: Option<f32>,
    sample_interval: Duration,
  }

  impl Default for PerfMonitor {
    fn default() -> Self {
      Self::new()
    }
  }

  impl PerfMonitor {
    /// Max gap between consecutive frames to treat them as part of a continuous burst.
    const BURST_GAP: Duration = Duration::from_millis(250);

    pub fn new() -> Self {
      let now = Instant::now();
      PerfMonitor {
        burst_times: VecDeque::with_capacity(128),
        last_sample: now,
        last_cpu_secs: process_cpu_seconds(),
        memory_mb: process_memory_mb(),
        cpu_percent: Some(0.0),
        gpu_percent: None,
        sample_interval: Duration::from_millis(100),
      }
    }

    /// Samples fresh process CPU & memory stats (called every 100 ms while idle or active).
    pub fn sample_system(&mut self) {
      let now = Instant::now();
      let elapsed = now.duration_since(self.last_sample);
      if elapsed < self.sample_interval {
        return;
      }
      self.memory_mb = process_memory_mb();
      if let (Some(prev), Some(curr)) = (self.last_cpu_secs, process_cpu_seconds()) {
        let dt = elapsed.as_secs_f64();
        if dt > 0.0 {
          let pct = ((curr - prev).max(0.0) / dt * 100.0) as f32;
          self.cpu_percent = Some(pct);
        }
        self.last_cpu_secs = Some(curr);
      }
      self.last_sample = now;
    }

    /// Returns a [`PerfStats`] snapshot with updated system metrics, preserving `prev_timings`.
    pub fn sample_live(&mut self, prev: PerfStats) -> PerfStats {
      self.sample_system();
      PerfStats {
        memory_mb: self.memory_mb,
        cpu_percent: self.cpu_percent,
        gpu_percent: self.gpu_percent,
        ..prev
      }
    }

    /// Sets the reported GPU utilization percentage (`None` displays as `N/A (CPU)`).
    pub fn set_gpu_percent(&mut self, gpu_percent: Option<f32>) {
      self.gpu_percent = gpu_percent;
    }

    /// Records one completed frame and returns an updated [`PerfStats`] snapshot.
    pub fn record_frame(&mut self, timings: FrameTimings) -> PerfStats {
      let now = Instant::now();
      // Reset the burst window after an idle gap so on-demand frames don't report "1 FPS".
      if let Some(&prev) = self.burst_times.back()
        && now.duration_since(prev) > Self::BURST_GAP
      {
        self.burst_times.clear();
      }
      self.burst_times.push_back(now);
      let window = Duration::from_secs(1);
      while let Some(&front) = self.burst_times.front() {
        if now.duration_since(front) > window {
          self.burst_times.pop_front();
        } else {
          break;
        }
      }

      let (fps, fps_from_burst) = if self.burst_times.len() >= 2 {
        let span = now.duration_since(*self.burst_times.front().unwrap()).as_secs_f32();
        if span > 0.001 {
          ((self.burst_times.len() - 1) as f32 / span, true)
        } else {
          let total = timings.total_ms();
          (if total > 0.001 { (1000.0 / total).min(999.0) } else { 0.0 }, false)
        }
      } else {
        let total = timings.total_ms();
        (if total > 0.001 { (1000.0 / total).min(999.0) } else { 0.0 }, false)
      };

      self.sample_system();

      PerfStats {
        fps,
        fps_from_burst,
        memory_mb: self.memory_mb,
        cpu_percent: self.cpu_percent,
        gpu_percent: self.gpu_percent,
        timings,
      }
    }
  }

  // -------------------------------------------------------------------------
  // PerfOverlay widget
  // -------------------------------------------------------------------------

  /// Compact performance HUD displaying FPS, CPU, memory, GPU, and per-phase render times.
  #[derive(Widget)]
  #[non_exhaustive]
  pub struct PerfOverlay {
    pub stats: PerfStats,
    /// Optional width override in logical pixels (defaults to `268.0`).
    #[widget(optional)]
    pub width: f32,
  }

  impl Element for PerfOverlay {
    fn layout(&self, _cx: &mut LayoutCx, constraints: Constraints) -> Size {
      let w = if self.width > 0.0 { self.width } else { PERF_OVERLAY_SIZE.width };
      constraints.constrain(Size::new(w, PERF_OVERLAY_SIZE.height))
    }

    fn paint(&self, cx: &mut PaintCx) {
      let r = cx.rect();
      let pad = 10.0;
      let inner_x = r.x + pad;
      let inner_w = (r.width - pad * 2.0).max(40.0);

      // Translucent dark HUD background.
      let bg = Color::rgba(18, 18, 24, 224);
      let border = Border::new(Color::rgba(255, 255, 255, 38), 1.0);
      cx.renderer.fill_rect(r, bg, 10.0);
      cx.renderer.stroke_rect(r, border, 10.0);

      let title_paint =
        TextPaint { color: Color::WHITE, size: 12.0, bold: true, ..Default::default() };
      let label_paint =
        TextPaint { color: Color::rgba(210, 214, 224, 255), size: 11.0, ..Default::default() };
      let dim_paint =
        TextPaint { color: Color::rgba(155, 160, 175, 255), size: 10.5, ..Default::default() };

      let s = &self.stats;
      let total_ms = s.timings.total_ms();
      let fps_color = if s.fps >= 55.0 {
        Color::hex(0x66BB6A)
      } else if s.fps >= 30.0 {
        Color::hex(0xFFA726)
      } else {
        Color::hex(0xEF5350)
      };

      // Row 1: FPS + total frame time
      let mut y = r.y + pad;
      let fps_text = if s.fps_from_burst {
        format!("FPS {:.1}", s.fps)
      } else {
        format!("FPS ~{:.0} (on-demand)", s.fps)
      };
      cx.renderer.draw_text(
        Offset::new(inner_x, y),
        &fps_text,
        &TextPaint { color: fps_color, ..title_paint.clone() },
      );
      let total_text = format!("{total_ms:5.2} ms");
      let total_sz = cx.renderer.measure_text(&total_text, &title_paint);
      cx.renderer.draw_text(
        Offset::new(inner_x + inner_w - total_sz.width, y),
        &total_text,
        &title_paint,
      );
      y += 19.0;

      // Row 2: CPU / MEM / GPU
      let cpu_str = s.cpu_percent.map_or_else(|| "N/A".to_string(), |v| format!("{v:.1}%"));
      let mem_str = s.memory_mb.map_or_else(|| "N/A".to_string(), |v| format!("{v:.1} MB"));
      let gpu_str = s.gpu_percent.map_or_else(|| "N/A (CPU)".to_string(), |v| format!("{v:.0}%"));

      cx.renderer.draw_text(Offset::new(inner_x, y), &format!("CPU {cpu_str}"), &label_paint);
      cx.renderer.draw_text(
        Offset::new(inner_x + inner_w * 0.45, y),
        &format!("MEM {mem_str}"),
        &label_paint,
      );
      y += 16.0;

      cx.renderer.draw_text(Offset::new(inner_x, y), &format!("GPU {gpu_str}"), &label_paint);
      let cmds_str = format!("Cmds {}", s.timings.cmd_count);
      let cmds_sz = cx.renderer.measure_text(&cmds_str, &dim_paint);
      cx.renderer.draw_text(
        Offset::new(inner_x + inner_w - cmds_sz.width, y),
        &cmds_str,
        &dim_paint,
      );
      y += 19.0;

      // Divider
      cx.renderer.draw_line(
        Offset::new(inner_x, y),
        Offset::new(inner_x + inner_w, y),
        Color::rgba(255, 255, 255, 28),
        1.0,
      );
      y += 6.0;

      // Stacked timeline bar across all render phases
      let bar_h = 7.0;
      let bar_bg = Color::rgba(255, 255, 255, 24);
      cx.renderer.fill_rect(Rect::new(inner_x, y, inner_w, bar_h), bar_bg, 3.0);
      let phases = s.timings.phases();
      if total_ms > 0.0001 {
        let mut bx = inner_x;
        for (_, ms, color) in phases {
          if ms <= 0.0 {
            continue;
          }
          let seg_w = (ms / total_ms) * inner_w;
          if seg_w >= 0.5 {
            cx.renderer.fill_rect(Rect::new(bx, y, seg_w, bar_h), color, 2.0);
            bx += seg_w;
          }
        }
      }
      y += bar_h + 7.0;

      // Per-phase rows with mini proportional bar + ms readout (slowest phase highlighted)
      let (bottleneck_name, bottleneck_ms) = s.timings.bottleneck();
      let label_col_w = 54.0;
      let ms_col_w = 52.0;
      let mini_bar_x = inner_x + label_col_w;
      let mini_bar_w = (inner_w - label_col_w - ms_col_w - 6.0).max(20.0);

      for (name, ms, color) in phases {
        let is_bottleneck = bottleneck_ms > 0.01 && name == bottleneck_name;
        cx.renderer.fill_circle(Offset::new(inner_x + 3.5, y + 6.5), 3.0, color);

        let row_paint = if is_bottleneck {
          TextPaint { color: Color::WHITE, size: 10.5, bold: true, ..Default::default() }
        } else {
          label_paint.clone()
        };
        cx.renderer.draw_text(Offset::new(inner_x + 10.0, y), name, &row_paint);

        let track = Rect::new(mini_bar_x, y + 3.5, mini_bar_w, 5.0);
        cx.renderer.fill_rect(track, bar_bg, 2.5);
        if total_ms > 0.0001 && ms > 0.0 {
          let fill_w = ((ms / total_ms).clamp(0.0, 1.0) * mini_bar_w).max(2.0);
          cx.renderer.fill_rect(Rect::new(track.x, track.y, fill_w, track.height), color, 2.5);
        }

        let ms_str = format!("{ms:5.2} ms");
        let ms_sz = cx.renderer.measure_text(&ms_str, &row_paint);
        cx.renderer.draw_text(
          Offset::new(inner_x + inner_w - ms_sz.width, y),
          &ms_str,
          &row_paint,
        );
        y += 16.5;
      }

      // Bottleneck summary footer
      y += 3.0;
      let footer = if bottleneck_ms > 0.01 {
        format!("Slowest: {bottleneck_name} ({bottleneck_ms:.2} ms)")
      } else {
        "Slowest: —".to_string()
      };
      cx.renderer.draw_text(Offset::new(inner_x, y), &footer, &dim_paint);
    }
  }

  // -------------------------------------------------------------------------
  // PerfLayer: independent Scene + Node rendered to a separate Renderer
  // -------------------------------------------------------------------------

  #[derive(Widget)]
  struct PerfHudRoot {
    stats: Signal<PerfStats>,
  }

  impl Element for PerfHudRoot {
    fn build(&self, _: &Context) -> Option<Widget> {
      Some(ui!(PerfOverlay { stats: self.stats.get() }))
    }
  }

  /// Hosts the performance overlay on its own independent [`Scene`] and [`Node`] tree
  /// so it renders to a separate [`Renderer`] without touching or dirtying the app's scene.
  ///
  /// Controlled automatically by [`App::perf_monitor`](crate::App::perf_monitor).
  pub struct PerfLayer {
    monitor: PerfMonitor,
    scene: Scene,
    node: Option<Rc<Node>>,
    stats: Signal<PerfStats>,
    pending_timings: Option<FrameTimings>,
  }

  impl Default for PerfLayer {
    fn default() -> Self {
      Self::new()
    }
  }

  impl PerfLayer {
    pub fn new() -> Self {
      PerfLayer {
        monitor: PerfMonitor::new(),
        scene: Scene::new(),
        node: None,
        stats: Signal::new(PerfStats::default()),
        pending_timings: None,
      }
    }

    /// Returns whether [`App::perf_monitor`](crate::App::perf_monitor) is currently enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
      crate::is_perf_monitor_enabled()
    }

    /// Sets the reported GPU utilization percentage (`None` displays as `N/A (CPU)`).
    pub fn set_gpu_percent(&mut self, gpu_percent: Option<f32>) {
      self.monitor.set_gpu_percent(gpu_percent);
    }

    /// Presents the app root, collecting per-phase timings only when `perf_monitor` is on.
    pub fn present_app(
      &mut self,
      app_root: &Node,
      app_scene: &mut Scene,
      app_backend: &mut dyn Renderer,
      update_ms: f32,
    ) -> Option<Rect> {
      if !self.is_enabled() {
        self.pending_timings = None;
        return app_root.present(app_scene, app_backend);
      }
      let (dmg, mut timings) = app_root.present_timed(app_scene, app_backend);
      timings.update_ms = update_ms;
      if dmg.is_some() {
        self.pending_timings = Some(timings);
      }
      dmg
    }

    /// Invalidates the overlay scene (e.g. when the overlay renderer's scale changed).
    pub fn invalidate(&mut self) {
      self.scene.invalidate();
    }

    /// Records `present_ms` (from the previous frame's blit) and renders the HUD into
    /// `overlay_backend` if `perf_monitor` is enabled. Returns `true` when the overlay
    /// should be composited onto the output surface.
    pub fn render_overlay(
      &mut self,
      present_ms: f32,
      overlay_backend: &mut dyn Renderer,
    ) -> bool {
      if !self.is_enabled() {
        return false;
      }
      if let Some(mut timings) = self.pending_timings.take() {
        timings.present_ms = present_ms;
        let stats = self.monitor.record_frame(timings);
        self.stats.set(stats);
      }
      let node = self.node.get_or_insert_with(|| {
        let mut theme = ThemeData::dark();
        theme.color_scheme = ColorScheme { surface: Color::TRANSPARENT, ..theme.color_scheme };
        let mut values = anymap3::AnyMap::new();
        values.insert(Rc::new(theme));
        let ctx = Context::default().with_scope(values);
        Node::mount(ui!(PerfHudRoot { stats: self.stats.clone() }), ctx)
      });
      node.update();
      node.present_isolated(&mut self.scene, overlay_backend);
      true
    }

    /// Refreshes live CPU/memory/GPU stats and redraws only `overlay_backend` (every 100 ms),
    /// without touching the app's scene or renderer.
    pub fn refresh_live(&mut self, overlay_backend: &mut dyn Renderer) -> bool {
      if !self.is_enabled() {
        return false;
      }
      let next = self.monitor.sample_live(self.stats.peek());
      self.stats.set(next);
      let node = self.node.get_or_insert_with(|| {
        let mut theme = ThemeData::dark();
        theme.color_scheme = ColorScheme { surface: Color::TRANSPARENT, ..theme.color_scheme };
        let mut values = anymap3::AnyMap::new();
        values.insert(Rc::new(theme));
        let ctx = Context::default().with_scope(values);
        Node::mount(ui!(PerfHudRoot { stats: self.stats.clone() }), ctx)
      });
      node.update();
      node.present_isolated(&mut self.scene, overlay_backend);
      true
    }
  }

  // -------------------------------------------------------------------------
  // Platform CPU & Memory sampling (zero extra dependencies)
  // -------------------------------------------------------------------------

  #[cfg(target_os = "macos")]
  fn process_memory_mb() -> Option<f32> {
    #[repr(C)]
    struct TimeValue {
      seconds: i32,
      microseconds: i32,
    }
    #[repr(C)]
    struct MachTaskBasicInfo {
      virtual_size: u64,
      resident_size: u64,
      resident_size_max: u64,
      user_time: TimeValue,
      system_time: TimeValue,
      policy: i32,
      suspend_count: i32,
    }
    unsafe extern "C" {
      static mach_task_self_: u32;
      fn task_info(
        target_task: u32,
        flavor: i32,
        task_info_out: *mut MachTaskBasicInfo,
        task_info_out_cnt: *mut u32,
      ) -> i32;
    }
    const MACH_TASK_BASIC_INFO: i32 = 20;
    let mut info = MachTaskBasicInfo {
      virtual_size: 0,
      resident_size: 0,
      resident_size_max: 0,
      user_time: TimeValue { seconds: 0, microseconds: 0 },
      system_time: TimeValue { seconds: 0, microseconds: 0 },
      policy: 0,
      suspend_count: 0,
    };
    let mut count =
      (std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<u32>()) as u32;
    // SAFETY: `info` and `count` match `MACH_TASK_BASIC_INFO` layout for macOS.
    let kr = unsafe { task_info(mach_task_self_, MACH_TASK_BASIC_INFO, &mut info, &mut count) };
    if kr == 0 { Some(info.resident_size as f32 / (1024.0 * 1024.0)) } else { None }
  }

  #[cfg(target_os = "linux")]
  fn process_memory_mb() -> Option<f32> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let rss_pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some((rss_pages as f64 * 4096.0 / (1024.0 * 1024.0)) as f32)
  }

  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  fn process_memory_mb() -> Option<f32> {
    None
  }

  #[cfg(target_os = "macos")]
  fn process_cpu_seconds() -> Option<f64> {
    #[repr(C)]
    struct Timeval {
      tv_sec: i64,
      tv_usec: i32,
      _pad: i32,
    }
    #[repr(C)]
    struct Rusage {
      ru_utime: Timeval,
      ru_stime: Timeval,
      _rest: [i64; 14],
    }
    unsafe extern "C" {
      fn getrusage(who: i32, usage: *mut Rusage) -> i32;
    }
    const RUSAGE_SELF: i32 = 0;
    let mut usage: Rusage = unsafe { std::mem::zeroed() };
    // SAFETY: `usage` matches macOS `struct rusage` layout.
    if unsafe { getrusage(RUSAGE_SELF, &mut usage) } == 0 {
      let user = usage.ru_utime.tv_sec as f64 + f64::from(usage.ru_utime.tv_usec) * 1e-6;
      let sys = usage.ru_stime.tv_sec as f64 + f64::from(usage.ru_stime.tv_usec) * 1e-6;
      Some(user + sys)
    } else {
      None
    }
  }

  #[cfg(target_os = "linux")]
  fn process_cpu_seconds() -> Option<f64> {
    #[repr(C)]
    struct Timeval {
      tv_sec: i64,
      tv_usec: i64,
    }
    #[repr(C)]
    struct Rusage {
      ru_utime: Timeval,
      ru_stime: Timeval,
      _rest: [i64; 14],
    }
    unsafe extern "C" {
      fn getrusage(who: i32, usage: *mut Rusage) -> i32;
    }
    const RUSAGE_SELF: i32 = 0;
    let mut usage: Rusage = unsafe { std::mem::zeroed() };
    // SAFETY: `usage` matches Linux 64-bit `struct rusage` layout.
    if unsafe { getrusage(RUSAGE_SELF, &mut usage) } == 0 {
      let user = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 * 1e-6;
      let sys = usage.ru_stime.tv_sec as f64 + usage.ru_stime.tv_usec as f64 * 1e-6;
      Some(user + sys)
    } else {
      None
    }
  }

  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  fn process_cpu_seconds() -> Option<f64> {
    None
  }
}

#[cfg(debug_assertions)]
pub use inner::{FrameTimings, PerfLayer, PerfMonitor, PerfOverlay, PerfStats};

/// Release-build stub: zero-sized no-op that strips all performance monitoring completely.
#[cfg(not(debug_assertions))]
#[derive(Default)]
pub struct PerfLayer;

#[cfg(not(debug_assertions))]
impl PerfLayer {
  #[inline]
  pub fn new() -> Self {
    PerfLayer
  }

  #[inline]
  pub fn is_enabled(&self) -> bool {
    false
  }

  #[inline]
  pub fn set_gpu_percent(&mut self, _gpu_percent: Option<f32>) {}

  #[inline]
  pub fn present_app(
    &mut self,
    app_root: &Node,
    app_scene: &mut Scene,
    app_backend: &mut dyn Renderer,
    _update_ms: f32,
  ) -> Option<Rect> {
    app_root.present(app_scene, app_backend)
  }

  #[inline]
  pub fn invalidate(&mut self) {}

  #[inline]
  pub fn render_overlay(&mut self, _present_ms: f32, _overlay_backend: &mut dyn Renderer) -> bool {
    false
  }

  #[inline]
  pub fn refresh_live(&mut self, _overlay_backend: &mut dyn Renderer) -> bool {
    false
  }
}
