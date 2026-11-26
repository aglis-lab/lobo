use std::collections::{HashMap, HashSet, VecDeque};

use crate::{DisplayList, DrawCmd, Item, LayerId, Offset, Rect, Renderer, Size};

struct Layer {
  list: DisplayList,
  bounds: Rect,
}

/// Retained scene of layers; diffs display lists, tracks damage, and submits one batch per frame.
pub struct Scene {
  root: Option<LayerId>,
  layers: HashMap<LayerId, Layer>,
  pending: HashMap<LayerId, DisplayList>,
  extra_damage: Rect,
  last_linked: HashSet<LayerId>,
  last_size: Option<Size>,
}

impl Default for Scene {
  fn default() -> Self {
    Self::new()
  }
}

impl Scene {
  pub fn new() -> Self {
    Scene {
      root: None,
      layers: HashMap::new(),
      pending: HashMap::new(),
      extra_damage: Rect::ZERO,
      last_linked: HashSet::new(),
      last_size: None,
    }
  }

  /// Forces a full redraw on the next [`Scene::present`] (e.g. if the backend recreated its surface).
  pub fn invalidate(&mut self) {
    self.last_size = None;
  }

  /// Sets which layer is the root of the scene.
  pub fn set_root(&mut self, id: LayerId) {
    if self.root != Some(id) {
      self.root = Some(id);
      if let Some(layer) = self.layers.get(&id) {
        self.extra_damage = self.extra_damage.union(&layer.bounds);
      }
    }
  }

  /// Stores `list` for `id` to be diffed on the next [`Scene::present`].
  pub fn set(&mut self, id: LayerId, list: DisplayList) {
    self.pending.insert(id, list);
  }

  /// Unmounts `id`, damaging its old bounds on the next frame.
  pub fn remove(&mut self, id: LayerId) {
    self.pending.remove(&id);
    if let Some(layer) = self.layers.remove(&id)
      && self.last_linked.remove(&id)
    {
      self.extra_damage = self.extra_damage.union(&layer.bounds);
    }
  }

  /// Diffs pending layers, computes damage, and sends one clipped batch to `backend` if needed.
  /// Returns the damaged canvas rect, or `None` when nothing changed.
  pub fn present(&mut self, backend: &mut dyn Renderer) -> Option<Rect> {
    let canvas_size = backend.size();
    let canvas_rect = Rect::from_origin(Offset::ZERO, canvas_size);
    let full_redraw = self.last_size != Some(canvas_size);
    self.last_size = Some(canvas_size);

    let mut damage = std::mem::take(&mut self.extra_damage);
    if full_redraw {
      damage = canvas_rect;
    }

    let mut reordered = Vec::new();
    for (id, list) in self.pending.drain() {
      let new_bounds = list_bounds(&list);
      if let Some(old) = self.layers.get(&id) {
        if old.list == list {
          continue;
        }
        if self.last_linked.contains(&id) {
          damage = damage.union(&old.bounds).union(&new_bounds);
          collect_reordered_slots(&old.list, &list, &mut reordered);
        }
      }
      self.layers.insert(id, Layer { list, bounds: new_bounds });
    }

    let mut current_linked = HashSet::new();
    if let Some(root) = self.root {
      self.collect_linked(root, &mut current_linked);
    }

    for &id in &self.last_linked {
      if !current_linked.contains(&id) {
        // Unlinked (not removed): damage its old bounds, but keep its list for re-linking.
        damage = damage.union(&self.subtree_bounds(id, &mut HashSet::new()));
      }
    }
    for &id in &current_linked {
      if !self.last_linked.contains(&id) {
        damage = damage.union(&self.subtree_bounds(id, &mut HashSet::new()));
      }
    }
    for id in reordered {
      if current_linked.contains(&id) {
        damage = damage.union(&self.subtree_bounds(id, &mut HashSet::new()));
      }
    }
    self.last_linked = current_linked;

    let damage = damage.intersect(&canvas_rect).round_out().intersect(&canvas_rect);
    if damage.is_empty() {
      return None;
    }

    let mut cmds = Vec::new();
    if let Some(root) = self.root {
      cmds.push(DrawCmd::PushClip(damage));
      let body_start = cmds.len();
      let mut overlays = VecDeque::new();
      let mut clips = vec![damage];
      self.flatten_layer(root, damage, body_start, &mut clips, &mut overlays, &mut cmds);
      while let Some(overlay) = overlays.pop_front() {
        let mut overlay_clips = vec![damage];
        self.flatten_layer(
          overlay,
          damage,
          body_start,
          &mut overlay_clips,
          &mut overlays,
          &mut cmds,
        );
      }
      cmds.push(DrawCmd::PopClip);
    }

    if cmds.len() <= 2 {
      // Only the outer PushClip/PopClip remained (e.g. root was empty).
      cmds.clear();
    }

    backend.draw_batch(damage, &cmds);
    Some(damage)
  }

  fn collect_linked(&self, id: LayerId, out: &mut HashSet<LayerId>) {
    if !out.insert(id) {
      return;
    }
    let Some(layer) = self.layers.get(&id) else {
      return;
    };
    for item in &layer.list {
      if let Item::Child(kid) | Item::Overlay(kid) = item {
        self.collect_linked(*kid, out);
      }
    }
  }

  fn subtree_bounds(&self, id: LayerId, visited: &mut HashSet<LayerId>) -> Rect {
    if !visited.insert(id) {
      return Rect::ZERO;
    }
    let Some(layer) = self.layers.get(&id) else {
      return Rect::ZERO;
    };
    let mut bounds = layer.bounds;
    for item in &layer.list {
      if let Item::Child(kid) | Item::Overlay(kid) = item {
        bounds = bounds.union(&self.subtree_bounds(*kid, visited));
      }
    }
    bounds
  }

  fn flatten_layer(
    &self,
    id: LayerId,
    damage: Rect,
    body_start: usize,
    clips: &mut Vec<Rect>,
    overlays: &mut VecDeque<LayerId>,
    cmds: &mut Vec<DrawCmd>,
  ) {
    let Some(layer) = self.layers.get(&id) else {
      return;
    };
    for item in &layer.list {
      match item {
        Item::Child(kid) => {
          self.flatten_layer(*kid, damage, body_start, clips, overlays, cmds);
        }
        Item::Overlay(kid) => overlays.push_back(*kid),
        Item::Cmd(DrawCmd::Clear(color), bounds) => {
          let active = clips.last().copied().unwrap_or(damage);
          if bounds.intersects(&active) {
            cmds.push(DrawCmd::FillRect { rect: damage, color: *color, radius: 0.0 });
          }
        }
        Item::Cmd(DrawCmd::PushClip(rect), _) => {
          let active = clips.last().copied().unwrap_or(damage);
          clips.push(active.intersect(rect));
          cmds.push(DrawCmd::PushClip(*rect));
        }
        Item::Cmd(DrawCmd::PopClip, _) => {
          if clips.len() > 1 {
            clips.pop();
          }
          if cmds.len() > body_start && matches!(cmds.last(), Some(DrawCmd::PushClip(_))) {
            cmds.pop();
          } else {
            cmds.push(DrawCmd::PopClip);
          }
        }
        Item::Cmd(cmd, bounds) => {
          let active = clips.last().copied().unwrap_or(damage);
          if bounds.intersects(&active) {
            cmds.push(cmd.clone());
          }
        }
      }
    }
  }
}

fn list_bounds(list: &[Item]) -> Rect {
  let mut bounds = Rect::ZERO;
  for item in list {
    if let Item::Cmd(cmd, rect) = item
      && !matches!(cmd, DrawCmd::PushClip(_) | DrawCmd::PopClip)
    {
      bounds = bounds.union(rect);
    }
  }
  bounds
}

fn collect_reordered_slots(old: &[Item], new: &[Item], out: &mut Vec<LayerId>) {
  let slots = |items: &[Item]| -> Vec<(bool, LayerId)> {
    items
      .iter()
      .filter_map(|item| match item {
        Item::Child(id) => Some((false, *id)),
        Item::Overlay(id) => Some((true, *id)),
        Item::Cmd(..) => None,
      })
      .collect()
  };
  let old_slots = slots(old);
  let new_slots = slots(new);
  let old_common: Vec<_> = old_slots
    .iter()
    .copied()
    .filter(|(_, id)| new_slots.iter().any(|(_, nid)| nid == id))
    .collect();
  let new_common: Vec<_> = new_slots
    .iter()
    .copied()
    .filter(|(_, id)| old_slots.iter().any(|(_, oid)| oid == id))
    .collect();
  if old_common == new_common {
    return;
  }
  for (a, b) in old_common.iter().zip(&new_common) {
    if a != b {
      out.push(a.1);
      out.push(b.1);
    }
  }
}
