//! Layout, terminal rendering, focus and the Material widgets, checked on a
//! 720p `TerminalRenderer` (160x45 cells).

use std::cell::Cell;
use std::rc::Rc;

use lobo::*;

fn frame(w: Widget) -> (Rc<Node>, TerminalRenderer) {
  let node = Node::mount(w, Context::default());
  let mut r = TerminalRenderer::hd720();
  render_frame(&node, &mut r);
  (node, r)
}

fn text_of(w: Widget) -> String {
  frame(w).1.to_plain_text()
}

/// (col, row) of the first cell where `needle` starts.
fn find(r: &TerminalRenderer, needle: &str) -> Option<(i32, i32)> {
  let text = r.to_plain_text();
  text.lines().enumerate().find_map(|(row, line)| {
    line.find(needle).map(|byte| (line[..byte].chars().count() as i32, row as i32))
  })
}

#[test]
fn canvas_is_720p_in_8x16_cells() {
  let r = TerminalRenderer::hd720();
  assert_eq!((r.cols(), r.rows()), (160, 45));
  assert_eq!(r.size(), Size::new(1280.0, 720.0));
}

#[test]
fn text_wraps_to_the_available_width() {
  // Tight constraints override a `SizedBox` (as in Flutter), so align it first.
  let r = text_of(ui!(Align {
    alignment: Alignment::TOP_LEFT,
    child: SizedBox { width: 72.0, child: Text { text: "one two three four" } }
  }));
  let lines: Vec<&str> = r.lines().take(3).collect();
  assert_eq!(lines, ["one two", "three", "four"]);
}

#[test]
fn max_lines_cuts_with_an_ellipsis() {
  let r = text_of(ui!(Align {
    alignment: Alignment::TOP_LEFT,
    child: SizedBox { width: 72.0, child: Text { text: "one two three four", max_lines: 2 } }
  }));
  let lines: Vec<&str> = r.lines().take(3).collect();
  assert_eq!(lines, ["one two", "three…", ""]);
}

#[test]
fn row_gives_expanded_children_the_free_space() {
  let (_, r) = frame(ui!(Row {
    children: [Text { text: "L" }, Expanded { child: Text { text: "mid" } }, Text { text: "R" }],
  }));
  assert_eq!(find(&r, "L"), Some((0, 0)));
  assert_eq!(find(&r, "mid"), Some((1, 0)));
  assert_eq!(find(&r, "R"), Some((159, 0)));
}

#[test]
fn column_alignment_and_center() {
  let (_, r) = frame(ui!(Column {
    main_axis_alignment: MainAxisAlignment::End,
    cross_axis_alignment: CrossAxisAlignment::Center,
    children: [Text { text: "bottom" }],
  }));
  assert_eq!(find(&r, "bottom"), Some((77, 44)));
}

#[test]
fn stack_places_positioned_children_by_their_edges() {
  let (_, r) = frame(ui!(Stack {
    children: [
      Positioned { right: 0.0, bottom: 0.0, child: Text { text: "corner" } },
      Positioned { left: 16.0, top: 32.0, child: Text { text: "near" } },
    ],
  }));
  assert_eq!(find(&r, "corner"), Some((154, 44)));
  assert_eq!(find(&r, "near"), Some((2, 2)));
}

#[test]
fn filled_button_uses_the_primary_color() {
  let (_, r) =
    frame(ui!(Center { child: FilledButton { child: Text { text: "Save" }, on_pressed: || {} } }));
  let (col, row) = find(&r, "Save").unwrap();
  let cell = r.cell(col, row).unwrap();
  let scheme = ColorScheme::light();
  assert_eq!(cell.bg, scheme.primary);
  assert_eq!(cell.fg, scheme.on_primary);
  assert!(cell.bold, "label large is weight 500");
}

#[test]
fn disabled_button_is_not_focusable() {
  let (_node, _r) = frame(ui!(Column { children: [FilledButton { child: Text { text: "Off" } }] }));
  assert_eq!(focus::count(), 0);
}

#[test]
fn theme_widget_switches_to_dark() {
  let (_, r) =
    frame(ui!(Theme { data: ThemeData::dark(), child: Scaffold { body: Text { text: "hi" } } }));
  let (col, row) = find(&r, "hi").unwrap();
  let cell = r.cell(col, row).unwrap();
  assert_eq!(cell.bg, ColorScheme::dark().surface);
  assert_eq!(cell.fg, ColorScheme::dark().on_surface);
}

#[test]
fn tab_focuses_and_enter_activates() {
  let pressed = Rc::new(Cell::new(0));
  let (p1, p2) = (pressed.clone(), pressed.clone());
  let (node, mut r) = frame(ui!(Row {
    main_axis_alignment: MainAxisAlignment::Center,
    children: [
      TextButton { child: Text { text: "one" }, on_pressed: move || p1.set(p1.get() + 1) },
      TextButton { child: Text { text: "two" }, on_pressed: move || p2.set(p2.get() + 10) },
    ],
  }));
  focus::set(None);
  assert_eq!(focus::count(), 2);
  dispatch_key_event(keys::TAB);
  dispatch_key_event(keys::TAB);
  assert_eq!(focus::current(), Some(1));
  dispatch_key_event(keys::ENTER);
  assert_eq!(pressed.get(), 10);
  dispatch_key_event(keys::BACK_TAB);
  dispatch_key_event(' ');
  assert_eq!(pressed.get(), 11);
  // The focused button shows markers in the next frame.
  render_frame(&node, &mut r);
  let (col, row) = find(&r, "one").unwrap();
  assert!(r.to_plain_text().lines().nth(row as usize).unwrap().contains('▸'));
  assert!(col > 1);
  focus::set(None);
}

#[test]
fn keys_parse_escape_sequences() {
  assert_eq!(parse_keys(""), vec![keys::ENTER]);
  assert_eq!(
    parse_keys("a\x1b[Cb\x1b[Z\x1b[3~"),
    vec!['a', keys::RIGHT, 'b', keys::BACK_TAB, keys::DELETE]
  );
  assert_eq!(parse_keys("\x1bOA"), vec![keys::UP]);
}

#[test]
fn raw_keys_map_to_lobo_keys() {
  use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
  let key = |code, mods| map_key(KeyEvent::new(code, mods)).map(|k| (k.key, k.modifiers));
  assert_eq!(key(KeyCode::Char('a'), KeyModifiers::NONE), Some(('a', Modifiers::NONE)));
  // Shift is already in the character.
  assert_eq!(key(KeyCode::Char('A'), KeyModifiers::SHIFT), Some(('A', Modifiers::NONE)));
  assert_eq!(key(KeyCode::Char('s'), KeyModifiers::CONTROL), Some(('s', Modifiers::CTRL)));
  assert_eq!(key(KeyCode::Enter, KeyModifiers::NONE), Some((keys::ENTER, Modifiers::NONE)));
  assert_eq!(key(KeyCode::BackTab, KeyModifiers::SHIFT), Some((keys::BACK_TAB, Modifiers::SHIFT)));
  assert_eq!(key(KeyCode::Backspace, KeyModifiers::NONE), Some((keys::BACKSPACE, Modifiers::NONE)));
  assert_eq!(key(KeyCode::Left, KeyModifiers::NONE), Some((keys::LEFT, Modifiers::NONE)));
  assert_eq!(key(KeyCode::F(1), KeyModifiers::NONE), None);
  let release =
    KeyEvent::new_with_kind(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Release);
  assert!(map_key(release).is_none(), "only presses and repeats count");
}

#[test]
fn checkbox_switch_and_slider_report_changes() {
  let checked = Signal::new(false);
  let value = Signal::new(0.5f32);
  let (c, v) = (checked.clone(), value.clone());
  let (_node, _r) = frame(ui!(Column {
    children: [
      Checkbox { value: false, on_changed: move |x: bool| c.set(x) },
      Slider { value: 0.5, divisions: 10, on_changed: move |x: f32| v.set(x) },
    ],
  }));
  focus::set(Some(0));
  dispatch_key_event(' ');
  assert!(checked.peek());
  focus::set(Some(1));
  dispatch_key_event(keys::RIGHT);
  assert!((value.peek() - 0.6).abs() < 1e-5);
  focus::set(None);
}

#[test]
fn text_field_edits_its_controller() {
  let name = Signal::new(String::new());
  let submitted = Signal::new(String::new());
  let s = submitted.clone();
  let (_node, _r) = frame(ui!(TextField {
    controller: name.clone(),
    label: "Name",
    on_submitted: move |v: String| s.set(v)
  }));
  focus::set(Some(0));
  for k in parse_keys("Adx") {
    dispatch_key_event(k);
  }
  dispatch_key_event(keys::DELETE);
  dispatch_key_event('a');
  assert_eq!(name.peek(), "Ada");
  dispatch_key_event(keys::ENTER);
  assert_eq!(submitted.peek(), "Ada");
  // Tab is never typed into the field: it moves the focus.
  dispatch_key_event(keys::TAB);
  assert_eq!(name.peek(), "Ada");
  focus::set(None);
}

#[test]
fn segmented_button_single_selection() {
  let selected = Signal::new(vec![0usize]);
  let s = selected.clone();
  let (_node, r) = frame(ui!(SegmentedButton {
    segments: [ButtonSegment { label: "Day" }, ButtonSegment { label: "Week" }],
    selected: vec![0],
    on_selection_changed: move |v: Vec<usize>| s.set(v),
  }));
  let row = r.to_plain_text().lines().find(|l| l.contains("Day")).unwrap().to_string();
  assert!(row.contains('✓') && row.trim_end().ends_with(')'), "{row}");
  focus::set(Some(1));
  dispatch_key_event(keys::ENTER);
  assert_eq!(selected.peek(), vec![1]);
  // Pressing the only selected segment can't empty the selection.
  focus::set(Some(0));
  dispatch_key_event(keys::ENTER);
  assert_eq!(selected.peek(), vec![1]);
  focus::set(None);
}

#[test]
fn optional_widget_fields_take_an_option() {
  let none: Option<Widget> = None;
  let (_, r) = frame(ui!(Scaffold {
    body: Text { text: "body" },
    snack_bar: none,
    app_bar: Some(ui!(AppBar { title: Text { text: "Title" } }))
  }));
  let text = r.to_plain_text();
  assert!(text.contains("Title") && text.contains("body"));
}

#[test]
fn dialog_takes_focus_when_it_opens() {
  let (_node, r) = frame(ui!(Stack {
    fit: StackFit::Expand,
    children: [
      Center { child: TextButton { child: Text { text: "behind" }, on_pressed: || {} } },
      AlertDialog {
        title: Text { text: "Title" },
        actions: [TextButton { child: Text { text: "OK" }, on_pressed: || {} }]
      },
    ],
  }));
  assert_eq!(focus::current(), Some(1), "the dialog's first button");
  let ok = find(&r, "OK").unwrap();
  let line = r.to_plain_text().lines().nth(ok.1 as usize).unwrap().to_string();
  assert!(line.contains('▸'));
  focus::set(None);
}

#[test]
fn date_math() {
  let d = Date::new(2026, 11, 25);
  assert_eq!(d.weekday(), 3, "a Wednesday");
  assert_eq!(d.add_days(7), Date::new(2026, 12, 2));
  assert_eq!(Date::new(2024, 1, 31).add_months(1), Date::new(2024, 2, 29));
  assert_eq!(Date::new(2026, 12, 15).add_months(1), Date::new(2027, 1, 15));
  assert_eq!(Date::from_days(Date::new(2000, 2, 29).to_days()), Date::new(2000, 2, 29));
}

/// Every catalog widget mounts, lays out and paints at 720p without panicking.
#[test]
fn every_material_widget_renders() {
  let widgets: Vec<Widget> = vec![
    ui!(ElevatedButton { child: Text { text: "e" }, on_pressed: || {} }),
    ui!(FilledButton { child: Text { text: "f" }, tonal: true }),
    ui!(OutlinedButton { child: Text { text: "o" }, icon: Icons::ADD }),
    ui!(TextButton { child: Text { text: "t" } }),
    ui!(IconButton { icon: Icons::ADD, variant: IconButtonVariant::Outlined }),
    ui!(FloatingActionButton { child: Icon { icon: Icons::ADD }, size: FabSize::Large }),
    ui!(FloatingActionButton { child: Icon { icon: Icons::ADD }, label: Text { text: "x" } }),
    ui!(SegmentedButton { segments: [ButtonSegment { label: "a" }], selected: Vec::new() }),
    ui!(Badge { label: "1", child: Icon { icon: Icons::MAIL } }),
    ui!(LinearProgressIndicator {}),
    ui!(CircularProgressIndicator { value: 0.5 }),
    ui!(SnackBar { content: Text { text: "s" }, action: SnackBarAction { label: "Undo" } }),
    ui!(AlertDialog { title: Text { text: "d" } }),
    ui!(BottomSheet { child: Text { text: "b" }, show_drag_handle: true }),
    ui!(Card { child: Text { text: "c" }, variant: CardVariant::Outlined }),
    ui!(Divider {}),
    ui!(VerticalDivider {}),
    ui!(ListTile { title: Text { text: "l" }, subtitle: Text { text: "s" } }),
    ui!(AppBar { title: Text { text: "a" }, actions: [IconButton { icon: Icons::SEARCH }] }),
    ui!(BottomAppBar { child: Text { text: "b" } }),
    ui!(NavigationBar { destinations: [NavigationDestination { icon: Icons::HOME, label: "h" }] }),
    ui!(NavigationDrawer {
      children: [NavigationDrawerDestination { icon: Icons::HOME, label: "h" }]
    }),
    ui!(NavigationRail {
      destinations: [NavigationRailDestination { icon: Icons::HOME, label: "h" }]
    }),
    ui!(TabBar { tabs: [Tab { text: "t" }] }),
    ui!(TabBarView { children: [Text { text: "v" }] }),
    ui!(Checkbox { value: true }),
    ui!(Radio { value: 1, group_value: 1 }),
    ui!(Switch { value: true }),
    ui!(Slider { value: 0.5 }),
    ui!(Chip { label: "c" }),
    ui!(ActionChip { label: "c" }),
    ui!(FilterChip { label: "c", selected: true }),
    ui!(ChoiceChip { label: "c", selected: false }),
    ui!(InputChip { label: "c", on_deleted: || {} }),
    ui!(MenuAnchor {
      open: true,
      child: Text { text: "m" },
      menu_children: [MenuItemButton { child: Text { text: "i" } }]
    }),
    ui!(CalendarDatePicker { selected: Date::new(2026, 2, 28) }),
    ui!(TimePicker { time: TimeOfDay::new(23, 59) }),
    ui!(TextField { controller: Signal::new("x".to_string()), variant: TextFieldVariant::Filled }),
    ui!(Scaffold {
      body: Text { text: "b" },
      floating_action_button: FloatingActionButton { child: Icon { icon: Icons::ADD } }
    }),
  ];
  for w in widgets {
    let (_node, r) = frame(ui!(Center { child: w }));
    assert_eq!(r.to_plain_text().lines().count(), 45);
  }
  focus::set(None);
}

#[test]
fn focused_listener_gets_keys_before_global_ones() {
  global_events.clear();
  let quit = Rc::new(Cell::new(false));
  let q = quit.clone();
  let name = Signal::new(String::new());
  let (_node, _r) = frame(ui!(InputEvent {
    event: move |e: &EventKey| {
      if e.key == 'q' {
        q.set(true);
      }
    },
    child: TextField { controller: name.clone() },
  }));
  focus::set(Some(0));
  dispatch_key_event('q');
  assert_eq!(name.peek(), "q", "typed into the focused field");
  assert!(!quit.get(), "the global listener never saw it");
  focus::set(None);
  dispatch_key_event('q');
  assert!(quit.get());
}

#[derive(Widget)]
struct Toggle {}

impl Element for Toggle {
  fn build(&self, context: &Context) -> Option<Widget> {
    let on = context.read::<Signal<bool>>();
    let set = on.as_ref().clone();
    Some(ui!(Row {
      children: [
        Text { text: "before" },
        Checkbox { value: on.get(), on_changed: move |v: bool| set.set(v) }
      ]
    }))
  }
}

#[test]
fn focus_survives_a_rebuild_of_the_focused_widget() {
  global_events.clear();
  let on = Signal::new(false);
  let s = on.clone();
  let (node, mut r) = frame(ui!(Provider { create: move |_| s.clone(), child: Toggle {} }));
  focus::set(Some(0));
  dispatch_key_event(' ');
  assert!(on.peek());
  // The checkbox is rebuilt with a new listener; the next frame keeps it focused.
  assert!(node.update());
  render_frame(&node, &mut r);
  assert_eq!(focus::current(), Some(0));
  dispatch_key_event(' ');
  assert!(!on.peek(), "the rebuilt checkbox got the key");
  focus::set(None);
}

#[test]
fn widget_name_is_the_struct_name() {
  assert_eq!(ui!(Text { text: "x" }).name(), "Text");
  assert_eq!(ui!(Toggle {}).name(), "Toggle");
}

#[test]
fn custom_widgets_become_focusable_with_a_key_handler() {
  global_events.clear();
  let seen = Rc::new(Cell::new('-'));
  let s = seen.clone();
  let w = focusable(
    move |e: &EventKey| {
      s.set(e.key);
      true
    },
    ui!(Text { text: "custom" }),
  );
  let (_node, _r) = frame(w);
  assert_eq!(focus::count(), 1);
  focus::set(Some(0));
  assert_eq!(dispatch_key_event('k'), EventResult::Handled);
  assert_eq!(seen.get(), 'k');
  focus::set(None);
}
