use std::cell::Cell;

use renderer::*;

struct Spy {
  size: Cell<Size>,
  batches: Vec<(Rect, Vec<DrawCmd>)>,
}

impl Spy {
  fn new(width: f32, height: f32) -> Self {
    Spy { size: Cell::new(Size::new(width, height)), batches: Vec::new() }
  }

  fn take(&mut self) -> Vec<(Rect, Vec<DrawCmd>)> {
    std::mem::take(&mut self.batches)
  }
}

impl Renderer for Spy {
  fn size(&self) -> Size {
    self.size.get()
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

fn record(spy: &Spy, f: impl FnOnce(&mut Recorder<'_>)) -> DisplayList {
  let mut rec = Recorder::new(spy);
  f(&mut rec);
  rec.finish()
}

#[test]
fn first_frame_damages_whole_canvas_and_idle_frames_send_nothing() {
  let mut spy = Spy::new(200.0, 100.0);
  let mut scene = Scene::new();
  let root = LayerId(1);
  scene.set_root(root);
  scene.set(
    root,
    record(&spy, |r| {
      r.clear(Color::WHITE);
      r.fill_rect(Rect::new(10.0, 10.0, 20.0, 20.0), Color::RED, 0.0);
    }),
  );

  let dmg = scene.present(&mut spy);
  assert_eq!(dmg, Some(Rect::new(0.0, 0.0, 200.0, 100.0)));
  let batches = spy.take();
  assert_eq!(batches.len(), 1);
  // Clear is converted into FillRect(damage), never sent as DrawCmd::Clear.
  assert!(!batches[0].1.iter().any(|c| matches!(c, DrawCmd::Clear(_))));
  assert!(batches[0].1.iter().any(|c| matches!(
    c,
    DrawCmd::FillRect { rect, color, .. } if *rect == Rect::new(0.0, 0.0, 200.0, 100.0) && *color == Color::WHITE
  )));

  // No changes -> nothing sent.
  assert_eq!(scene.present(&mut spy), None);
  assert!(spy.take().is_empty());

  // Identical display list -> nothing sent.
  scene.set(
    root,
    record(&spy, |r| {
      r.clear(Color::WHITE);
      r.fill_rect(Rect::new(10.0, 10.0, 20.0, 20.0), Color::RED, 0.0);
    }),
  );
  assert_eq!(scene.present(&mut spy), None);
  assert!(spy.take().is_empty());
}

#[test]
fn resize_damages_whole_canvas() {
  let mut spy = Spy::new(100.0, 80.0);
  let mut scene = Scene::new();
  let root = LayerId(1);
  scene.set_root(root);
  scene.set(root, record(&spy, |r| r.clear(Color::WHITE)));
  assert_eq!(scene.present(&mut spy), Some(Rect::new(0.0, 0.0, 100.0, 80.0)));
  spy.take();

  spy.size.set(Size::new(150.0, 90.0));
  assert_eq!(scene.present(&mut spy), Some(Rect::new(0.0, 0.0, 150.0, 90.0)));
  assert_eq!(spy.take().len(), 1);
}

#[test]
fn changed_layer_damages_old_union_new_rounded_out_and_skips_untouched_siblings() {
  let mut spy = Spy::new(200.0, 200.0);
  let mut scene = Scene::new();
  let root = LayerId(1);
  let a = LayerId(2);
  let b = LayerId(3);
  scene.set_root(root);
  scene.set(
    root,
    record(&spy, |r| {
      r.clear(Color::WHITE);
      r.child(a);
      r.child(b);
    }),
  );
  scene.set(a, record(&spy, |r| r.fill_rect(Rect::new(10.2, 10.2, 10.0, 10.0), Color::RED, 0.0)));
  scene
    .set(b, record(&spy, |r| r.fill_rect(Rect::new(120.0, 120.0, 20.0, 20.0), Color::BLUE, 0.0)));
  scene.present(&mut spy);
  spy.take();

  // Move layer `a` slightly; `b` is completely outside the damaged region.
  scene.set(a, record(&spy, |r| r.fill_rect(Rect::new(15.3, 12.1, 10.0, 10.0), Color::RED, 0.0)));
  let dmg = scene.present(&mut spy).unwrap();
  // old bounds: [9.2, 9.2 .. 21.2, 21.2]; new bounds: [14.3, 11.1 .. 26.3, 23.1]
  // union rounded out: [9.0, 9.0, 18.0, 15.0] (right=27, bottom=24)
  assert_eq!(dmg, Rect::new(9.0, 9.0, 18.0, 15.0));

  let batches = spy.take();
  assert_eq!(batches.len(), 1);
  let cmds = &batches[0].1;
  // Background fill (damage) + `a`'s red rect; `b`'s blue rect is culled.
  assert!(
    cmds.iter().any(|c| matches!(c, DrawCmd::FillRect { color, .. } if *color == Color::RED))
  );
  assert!(
    !cmds.iter().any(|c| matches!(c, DrawCmd::FillRect { color, .. } if *color == Color::BLUE))
  );
}

#[test]
fn remove_unlink_and_relink_damage_and_reuse_kept_list() {
  let mut spy = Spy::new(200.0, 200.0);
  let mut scene = Scene::new();
  let root = LayerId(1);
  let child = LayerId(2);
  let temp = LayerId(3);
  scene.set_root(root);
  scene.set(
    root,
    record(&spy, |r| {
      r.clear(Color::WHITE);
      r.child(child);
      r.child(temp);
    }),
  );
  scene.set(
    child,
    record(&spy, |r| r.fill_rect(Rect::new(20.0, 20.0, 10.0, 10.0), Color::GREEN, 0.0)),
  );
  scene.set(
    temp,
    record(&spy, |r| r.fill_rect(Rect::new(80.0, 80.0, 10.0, 10.0), Color::ORANGE, 0.0)),
  );
  scene.present(&mut spy);
  spy.take();

  // Unlink `child` (without removing its layer).
  scene.set(
    root,
    record(&spy, |r| {
      r.clear(Color::WHITE);
      r.child(temp);
    }),
  );
  // Note: `root` only has `Clear` + child slots; `root`'s own `Clear` covers canvas, so let's
  // test unlink on a container layer that doesn't call `clear`.
  let container = LayerId(4);
  scene.set(
    root,
    record(&spy, |r| {
      r.clear(Color::WHITE);
      r.child(container);
    }),
  );
  scene.set(
    container,
    record(&spy, |r| {
      r.child(child);
      r.child(temp);
    }),
  );
  scene.present(&mut spy);
  spy.take();

  // Unlink `child` from `container`.
  scene.set(container, record(&spy, |r| r.child(temp)));
  let dmg = scene.present(&mut spy).unwrap();
  // `child` had [19, 19, 12, 12] (with 1 px AA pad).
  assert_eq!(dmg, Rect::new(19.0, 19.0, 12.0, 12.0));
  let batches = spy.take();
  assert!(
    !batches[0]
      .1
      .iter()
      .any(|c| matches!(c, DrawCmd::FillRect { color, .. } if *color == Color::GREEN))
  );

  // Re-link `child` without calling `scene.set(child, ...)`: kept list must redraw!
  scene.set(
    container,
    record(&spy, |r| {
      r.child(child);
      r.child(temp);
    }),
  );
  let dmg = scene.present(&mut spy).unwrap();
  assert_eq!(dmg, Rect::new(19.0, 19.0, 12.0, 12.0));
  let batches = spy.take();
  assert!(
    batches[0]
      .1
      .iter()
      .any(|c| matches!(c, DrawCmd::FillRect { color, .. } if *color == Color::GREEN))
  );

  // Remove `temp` while still unlinking it.
  scene.remove(temp);
  scene.set(container, record(&spy, |r| r.child(child)));
  let dmg = scene.present(&mut spy).unwrap();
  assert_eq!(dmg, Rect::new(79.0, 79.0, 12.0, 12.0));
}

#[test]
fn overlays_draw_after_the_whole_tree_and_clips_prune_empty_commands() {
  let mut spy = Spy::new(100.0, 100.0);
  let mut scene = Scene::new();
  let root = LayerId(1);
  let popup = LayerId(2);
  let sibling = LayerId(3);
  scene.set_root(root);
  scene.set(
    root,
    record(&spy, |r| {
      r.clear(Color::WHITE);
      r.push_clip(Rect::new(0.0, 0.0, 10.0, 10.0));
      // Completely outside clip -> dropped by Recorder, and clip pair becomes empty -> pruned!
      r.fill_rect(Rect::new(50.0, 50.0, 10.0, 10.0), Color::YELLOW, 0.0);
      r.pop_clip();
      r.overlay(popup);
      r.child(sibling);
    }),
  );
  scene.set(popup, record(&spy, |r| r.fill_rect(Rect::new(5.0, 5.0, 20.0, 20.0), Color::RED, 0.0)));
  scene
    .set(sibling, record(&spy, |r| r.fill_rect(Rect::new(5.0, 5.0, 20.0, 20.0), Color::BLUE, 0.0)));

  scene.present(&mut spy);
  let batches = spy.take();
  let cmds = &batches[0].1;

  // Empty clip pair around culled YELLOW rect must be pruned; only outer PushClip/PopClip remain.
  let push_count = cmds.iter().filter(|c| matches!(c, DrawCmd::PushClip(_))).count();
  assert_eq!(push_count, 1);
  assert!(
    !cmds.iter().any(|c| matches!(c, DrawCmd::FillRect { color, .. } if *color == Color::YELLOW))
  );

  // Sibling (BLUE) must appear before overlay (RED).
  let blue_pos = cmds
    .iter()
    .position(|c| matches!(c, DrawCmd::FillRect { color, .. } if *color == Color::BLUE))
    .unwrap();
  let red_pos = cmds
    .iter()
    .position(|c| matches!(c, DrawCmd::FillRect { color, .. } if *color == Color::RED))
    .unwrap();
  assert!(blue_pos < red_pos);
}
