//! `ui!` syntax coverage. These mostly check that each form compiles to a
//! `Widget`; the rendering itself is covered by the examples.

use lobo::*;

#[derive(Widget)]
#[allow(dead_code)] // only constructed, never rendered
struct Probe {
  #[widget(optional)]
  count: i32,
  #[widget(optional)]
  r#type: String,
  child: Widget,
}

impl Element for Probe {}

fn mode() -> u8 {
  std::hint::black_box(1)
}

#[test]
fn struct_syntax_with_defaults() {
  let _: Widget = ui!(Container {
    padding: Padding { left: 8.0 },
    child: Column { children: [Text { text: "a" }, Container { child: Text { text: "b" } }] },
  });
  // Only the required field.
  let _: Widget = ui!(Column { children: [] });
}

#[test]
fn field_shorthand_and_paths() {
  let text = String::from("hi");
  let _: Widget = ui!(lobo::Text { text });
  let _: Widget = ui!(Probe { r#type: "raw", child: Text { text: "x" } });
}

#[test]
fn if_in_field_may_mix_widget_types() {
  let wide = mode() == 1;
  let _: Widget = ui!(Container {
    child: if wide { Column { children: [Text { text: "a" }] } } else { Text { text: "b" } },
  });
  // Non-widget field values work too.
  let _: Widget = ui!(Column { spacing: if wide { 4.0 } else { 0.0 }, children: [] });
}

#[test]
fn if_and_match_in_list_may_mix_widget_types() {
  let _: Widget = ui!(Column {
    children: [
      if mode() == 0 { Text { text: "zero" } } else { Container { child: Text { text: "n" } } },
      match mode() {
        0 => Text { text: "a" },
        1 => {
          let label = format!("b{}", 1);
          Column { children: [Text { text: label }] }
        }
        _ => unreachable!(),
      },
    ],
  });
}

#[test]
fn nested_ui_and_else_if_in_branches() {
  let _: Widget = ui!(Container {
    child: if mode() == 0 {
      ui!(Text { text: "nested" })
    } else if mode() == 1 {
      Text { text: "else if" }
    } else {
      Column { children: [] }
    },
  });
}

#[test]
fn root_may_be_an_if() {
  let _: Widget = ui!(if mode() == 0 { Text { text: "a" } } else { Column { children: [] } });
}

#[test]
fn plain_struct_literal_in_parentheses_or_closure() {
  struct Point {
    x: i32,
  }
  let _: Widget = ui!(Probe { count: (Point { x: 3 }).x, child: Text { text: "p" } });
  let _: Widget = ui!(Provider { create: |_| Point { x: 1 }, child: Column { children: [] } });
}

#[test]
fn iterator_children() {
  let rows = ["a", "b"].map(|t| ui!(Text { text: t }));
  let _: Widget = ui!(Column { children: rows });
}
