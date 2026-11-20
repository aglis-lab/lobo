// ---------------------------------------------------------------------------
// Built-in widgets
// ---------------------------------------------------------------------------

// `Widget` is both the struct and the derive macro; this imports both.
use crate::{Context, Element, Widget};
use std::rc::Rc;

#[derive(Widget)]
pub struct Text {
    pub text: String,
}

impl Element for Text {
    fn describe(&self) -> String {
        format!("Text({:?})", self.text)
    }
}

// Implement Signal Builder which record make signal only rebuild on child widgets
#[derive(Widget)]
pub struct SignalBuilder {
    pub builder: Rc<dyn Fn(&Context) -> Option<Widget>>,
}

impl Element for SignalBuilder {
    fn build(&self, context: &Context) -> Option<Widget> {
        println!("SignalBuilder being called");

        self.builder.as_ref()(context)
    }
}

// `optional` on the struct: every field may be omitted (defaults to 0.0).
// Padding is plain data, not an Element.
#[derive(Widget, Default, Clone, Copy)]
#[widget(optional)]
pub struct Padding {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl std::fmt::Display for Padding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "l:{}, t:{}, r:{}, b:{}",
            self.left, self.top, self.right, self.bottom
        )
    }
}

// `padding` may be omitted, `child` is required.
#[derive(Widget)]
pub struct Container {
    #[widget(optional)]
    pub padding: Padding,
    pub child: Widget,
}

impl Element for Container {
    fn describe(&self) -> String {
        format!("Container(padding({}))", self.padding)
    }

    fn children(&self) -> Vec<Widget> {
        vec![self.child.clone()]
    }
}

/// Lays children out vertically.
#[derive(Widget)]
pub struct Column {
    /// Gap between children.
    #[widget(optional)]
    pub spacing: f32,
    pub children: Vec<Widget>,
}

impl Element for Column {
    fn describe(&self) -> String {
        format!("Column(spacing:{})", self.spacing)
    }

    fn children(&self) -> Vec<Widget> {
        self.children.clone()
    }
}
