// Lets `ui!` output (`::lobo::Widget`) resolve inside this crate too.
extern crate self as lobo;

mod basic;
mod context;
pub mod layout;
pub mod material;
mod provider;
pub mod render;
mod signal;
mod task;
mod term;
mod theme;
mod tree;
mod widget;

// Re-exported so `#[derive(Model)]` output can name them without an extra dependency.
pub use serde;
pub use serde_json;

pub use basic::*;
pub use context::*;
pub use layout::{Constraints, Flex, FlexFit, LayoutCx, PaintCx, focus, render_frame};
pub use material::*;
pub use provider::*;
pub use render::{
  Border, BoxPaint, Color, HD_720, IconData, Offset, Rect, Renderer, Size, TerminalRenderer,
  TextPaint, ToggleColors, TrackColors,
};
pub use signal::*;
pub use task::*;
pub use term::{InputMode, map_key};
pub use theme::*;
pub use tree::*;
pub use widget::*;
