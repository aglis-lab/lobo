//! Material 3 components, following the Flutter catalog
//! (https://docs.flutter.dev/ui/widgets/material). See `docs/material-widgets.md`
//! for what is implemented, what is partial and what is missing.

mod actions;
mod common;
mod communication;
mod containment;
mod icons;
mod navigation;
mod selection;
mod text_field;

pub use actions::*;
pub use common::{Callback, DefaultTextStyle, ValueChanged};
pub use communication::*;
pub use containment::*;
pub use icons::Icons;
pub use navigation::*;
pub use selection::*;
pub use text_field::*;
