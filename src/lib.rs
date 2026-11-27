// Lets `ui!` output (`::lobo::Widget`) resolve inside this crate too.
extern crate self as lobo;

pub mod anim;
mod app;
mod basic;
mod context;
mod custom_paint;
pub mod layout;
pub mod material;
mod perf;
pub mod pointer;
#[cfg(all(feature = "cpu", target_os = "macos"))]
mod present;
mod provider;
mod render;
mod signal;
mod taffy_layout;
mod task;
mod theme;
mod tree;
mod widget;

// Re-exported so `#[derive(Model)]` output can name them without an extra dependency.
pub use serde;
pub use serde_json;
pub use taffy;

pub use anim::{
  AnimatedF32, AnimatedSpring, AnimatedValue, Curve, Lerp, LoopingPhase, Spring, Tween,
  ValueBuilder, lerp, lerp_color,
};
pub use app::*;
pub use basic::*;
pub use context::*;
pub use custom_paint::{CustomPaint, CustomPainter, Painter};
pub use layout::{Constraints, Flex, FlexFit, LayoutCx, PaintCx, focus};
pub use material::*;
pub use perf::*;
pub use pointer::{
  GestureDetector, PointerButton, PointerEvent, clear_pointer_state, dispatch_pointer,
};
pub use provider::*;
pub use render::*;
pub use signal::*;
pub use taffy_layout::{Grid, LayoutItem, TaffyBox};
pub use task::*;
pub use theme::*;
pub use tree::*;
pub use widget::*;
