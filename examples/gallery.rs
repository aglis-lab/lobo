//! Material 3 gallery: every implemented widget, rendered in the terminal at 720p
//! (160x45 cells, 24-bit color). Make the terminal at least 160x46.
//!
//! Keys act as soon as they are pressed (raw terminal mode):
//!   Tab / Shift-Tab   move the focus           Enter or space: press
//!   arrows, + / -     adjust a focused slider, date or time field
//!   x / Delete        delete a focused input chip, or a character in a text field
//!   Esc               clear the focus          q (nothing focused) or Ctrl+C: quit
//!
//! Run: `cargo run --example gallery`  (`LOBO_RENDER=plain` prints text only;
//! with piped input, or `LOBO_INPUT=line`, keys are read a line at a time)

use std::rc::Rc;

use lobo::*;

struct Gallery {
  page: Signal<usize>,
  dark: Signal<bool>,
  presses: Signal<u32>,
  menu_open: Signal<bool>,
  // Actions
  segments: Signal<Vec<usize>>,
  // Communication
  snack: Signal<bool>,
  progress: Signal<f32>,
  // Containment
  dialog: Signal<bool>,
  sheet: Signal<bool>,
  // Navigation
  tab: Signal<usize>,
  nav: Signal<usize>,
  drawer: Signal<bool>,
  // Selection
  checked: Signal<bool>,
  radio: Signal<usize>,
  switched: Signal<bool>,
  slider: Signal<f32>,
  filters: Signal<Vec<bool>>,
  choice: Signal<usize>,
  tags: Signal<Vec<String>>,
  date: Signal<Date>,
  time: Signal<TimeOfDay>,
  // Text inputs
  name: Signal<String>,
  email: Signal<String>,
  password: Signal<String>,
  submitted: Signal<String>,
}

impl Gallery {
  fn new() -> Self {
    Gallery {
      page: Signal::new(0),
      dark: Signal::new(false),
      presses: Signal::new(0),
      menu_open: Signal::new(false),
      segments: Signal::new(vec![1]),
      snack: Signal::new(false),
      progress: Signal::new(0.4),
      dialog: Signal::new(false),
      sheet: Signal::new(false),
      tab: Signal::new(0),
      nav: Signal::new(0),
      drawer: Signal::new(false),
      checked: Signal::new(true),
      radio: Signal::new(0),
      switched: Signal::new(true),
      slider: Signal::new(30.0),
      filters: Signal::new(vec![true, false, false]),
      choice: Signal::new(0),
      tags: Signal::new(vec!["rust".into(), "flutter".into(), "terminal".into()]),
      date: Signal::new(Date::new(2026, 11, 25)),
      time: Signal::new(TimeOfDay::new(10, 30)),
      name: Signal::new(String::new()),
      email: Signal::new("dev@example.com".into()),
      password: Signal::new(String::new()),
      submitted: Signal::new(String::new()),
    }
  }
}

const PAGES: [&str; 6] =
  ["Actions", "Communication", "Containment", "Navigation", "Selection", "Text inputs"];
/// Rail labels must fit in 80 px (10 terminal cells).
const SHORT: [&str; 6] = ["Actions", "Comms", "Contain", "Navigate", "Select", "Inputs"];

/// Rebuilds only this part when the signals it reads change, so the page's list
/// (and its scroll position) is kept.
fn watch(f: impl Fn(&Gallery) -> Widget + 'static) -> Widget {
  ui!(SignalBuilder { builder: move |context: &Context| Some(f(&context.read::<Gallery>())) })
}

fn text(s: impl Into<String>) -> Widget {
  ui!(Text { text: s.into() })
}

fn heading(s: &str) -> Widget {
  ui!(Text { text: s.to_string(), style: TextStyle { bold: true, size: 22.0 } })
}

/// `on_pressed` that runs `f` with the gallery state.
fn act(g: &Rc<Gallery>, f: impl Fn(&Gallery) + 'static) -> Callback {
  let g = g.clone();
  Callback::from(move || f(&g))
}

fn toggle(s: &Signal<bool>) {
  s.set(!s.peek());
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

fn actions_page(g: &Rc<Gallery>) -> Vec<Widget> {
  let press = act(g, |g| g.presses.set(g.presses.peek() + 1));
  vec![
    heading("Common buttons"),
    ui!(Row {
      spacing: 16.0,
      main_axis_size: MainAxisSize::Min,
      children: [
        ElevatedButton { child: Text { text: "Elevated" }, on_pressed: press.clone() },
        FilledButton { child: Text { text: "Filled" }, on_pressed: press.clone() },
        FilledButton { child: Text { text: "Tonal" }, tonal: true, on_pressed: press.clone() },
        OutlinedButton { child: Text { text: "Outlined" }, on_pressed: press.clone() },
        TextButton { child: Text { text: "Text" }, on_pressed: press.clone() },
        FilledButton {
          child: Text { text: "With icon" },
          icon: Icons::SEND,
          on_pressed: press.clone()
        },
        FilledButton { child: Text { text: "Disabled" } },
      ],
    }),
    watch(|g| text(format!("Pressed {} times", g.presses.get()))),
    heading("Icon buttons"),
    ui!(Row {
      spacing: 16.0,
      main_axis_size: MainAxisSize::Min,
      children: [
        IconButton { icon: Icons::FAVORITE, on_pressed: press.clone() },
        IconButton {
          icon: Icons::ADD,
          variant: IconButtonVariant::Filled,
          on_pressed: press.clone()
        },
        IconButton {
          icon: Icons::EDIT,
          variant: IconButtonVariant::FilledTonal,
          on_pressed: press.clone()
        },
        IconButton {
          icon: Icons::SHARE,
          variant: IconButtonVariant::Outlined,
          on_pressed: press.clone()
        },
        IconButton { icon: Icons::DELETE },
      ],
    }),
    heading("Floating action buttons"),
    ui!(Row {
      spacing: 24.0,
      main_axis_size: MainAxisSize::Min,
      cross_axis_alignment: CrossAxisAlignment::Center,
      children: [
        FloatingActionButton {
          child: Icon { icon: Icons::ADD },
          size: FabSize::Small,
          on_pressed: press.clone()
        },
        FloatingActionButton { child: Icon { icon: Icons::ADD }, on_pressed: press.clone() },
        FloatingActionButton {
          child: Icon { icon: Icons::ADD },
          size: FabSize::Large,
          on_pressed: press.clone()
        },
        FloatingActionButton {
          child: Icon { icon: Icons::EDIT },
          label: Text { text: "Extended" },
          on_pressed: press
        },
      ],
    }),
    heading("Segmented button"),
    watch(|g| {
      let segments = g.segments.clone();
      ui!(SegmentedButton {
        segments: [
          ButtonSegment { label: "Day", icon: Icons::CALENDAR_TODAY },
          ButtonSegment { label: "Week" },
          ButtonSegment { label: "Month" },
          ButtonSegment { label: "Year" },
        ],
        selected: g.segments.get(),
        multi_selection_enabled: true,
        on_selection_changed: move |v: Vec<usize>| segments.set(v),
      })
    }),
  ]
}

fn communication_page(g: &Rc<Gallery>) -> Vec<Widget> {
  vec![
    heading("Badges"),
    ui!(Row {
      spacing: 32.0,
      main_axis_size: MainAxisSize::Min,
      children: [
        Badge { child: Icon { icon: Icons::MAIL } },
        Badge { label: "3", child: Icon { icon: Icons::NOTIFICATIONS } },
        Badge { label: "99+", child: Icon { icon: Icons::INBOX } },
      ],
    }),
    heading("Progress indicators"),
    watch(|g| {
      let p = g.progress.get();
      let progress = g.progress.clone();
      ui!(Column {
        spacing: 16.0,
        main_axis_size: MainAxisSize::Min,
        children: [
          Text {
            text: format!("Determinate: {:.0}%  (focus the slider and use arrows)", p * 100.0)
          },
          SizedBox { width: 600.0, child: LinearProgressIndicator { value: p } },
          SizedBox {
            width: 600.0,
            child: Slider { value: p, divisions: 20, on_changed: move |v: f32| progress.set(v) }
          },
          Text { text: "Indeterminate (still frame):" },
          SizedBox { width: 600.0, child: LinearProgressIndicator {} },
          Row {
            spacing: 24.0,
            main_axis_size: MainAxisSize::Min,
            children: [
              CircularProgressIndicator { value: 0.25 },
              CircularProgressIndicator { value: p },
              CircularProgressIndicator { value: 1.0 },
              CircularProgressIndicator {},
            ],
          },
        ],
      })
    }),
    heading("Snack bar"),
    ui!(FilledButton {
      child: Text { text: "Show snack bar" },
      on_pressed: act(g, |g| g.snack.set(true))
    }),
  ]
}

fn containment_page(g: &Rc<Gallery>) -> Vec<Widget> {
  let card = |variant: CardVariant, title: &str| {
    ui!(SizedBox {
      width: 280.0,
      child: Card {
        variant,
        child: Container {
          padding: Padding::all(16.0),
          child: Column {
            spacing: 8.0,
            main_axis_size: MainAxisSize::Min,
            children: [
              Text { text: title.to_string(), style: TextStyle { bold: true } },
              Text { text: "Related content, grouped in one surface." }
            ],
          },
        },
      },
    })
  };
  vec![
    heading("Cards"),
    ui!(Row {
      spacing: 16.0,
      main_axis_size: MainAxisSize::Min,
      children: [
        card(CardVariant::Elevated, "Elevated"),
        card(CardVariant::Filled, "Filled"),
        card(CardVariant::Outlined, "Outlined")
      ],
    }),
    heading("List tiles and dividers"),
    ui!(SizedBox {
      width: 640.0,
      child: Column {
        main_axis_size: MainAxisSize::Min,
        children: [
          ListTile {
            leading: Icon { icon: Icons::PERSON },
            title: Text { text: "One-line item" },
            trailing: Icon { icon: Icons::CHEVRON_RIGHT },
            on_tap: act(g, |_| {})
          },
          Divider {},
          ListTile {
            leading: Icon { icon: Icons::MAIL },
            title: Text { text: "Two-line item" },
            subtitle: Text { text: "Supporting text" },
            trailing: Text { text: "10 min" },
            on_tap: act(g, |_| {}),
          },
          Divider { indent: 16.0, end_indent: 16.0 },
          ListTile {
            leading: Icon { icon: Icons::STAR },
            title: Text { text: "Selected item" },
            selected: true,
            on_tap: act(g, |_| {})
          },
          ListTile {
            leading: Icon { icon: Icons::LOCK },
            title: Text { text: "Disabled item" },
            disabled: true
          },
        ],
      },
    }),
    heading("Dialog and bottom sheet"),
    ui!(Row {
      spacing: 16.0,
      main_axis_size: MainAxisSize::Min,
      children: [
        FilledButton {
          child: Text { text: "Open dialog" },
          on_pressed: act(g, |g| g.dialog.set(true))
        },
        OutlinedButton {
          child: Text { text: "Open bottom sheet" },
          on_pressed: act(g, |g| g.sheet.set(true))
        },
      ],
    }),
  ]
}

fn navigation_page(g: &Rc<Gallery>) -> Vec<Widget> {
  vec![
    heading("Tab bar"),
    watch(|g| {
      let tab = g.tab.clone();
      let index = g.tab.get();
      ui!(SizedBox {
        width: 720.0,
        child: Column {
          spacing: 8.0,
          main_axis_size: MainAxisSize::Min,
          children: [
            TabBar {
              tabs: [
                Tab { text: "Flights", icon: Icons::SEND },
                Tab { text: "Trips", icon: Icons::EXPLORE },
                Tab { text: "Explore", icon: Icons::SEARCH }
              ],
              selected_index: index,
              on_tap: move |i: usize| tab.set(i),
            },
            TabBarView {
              index,
              children: [
                Text { text: "Flights: no upcoming flights." },
                Text { text: "Trips: two trips planned." },
                Text { text: "Explore: popular destinations." }
              ],
            },
          ],
        },
      })
    }),
    heading("Navigation bar"),
    watch(|g| {
      let nav = g.nav.clone();
      ui!(SizedBox {
        width: 720.0,
        child: NavigationBar {
          destinations: [
            NavigationDestination { icon: Icons::HOME, label: "Home" },
            NavigationDestination { icon: Icons::EXPLORE, label: "Explore" },
            NavigationDestination { icon: Icons::BOOKMARK, label: "Saved" },
            NavigationDestination { icon: Icons::PERSON, label: "Profile" },
          ],
          selected_index: g.nav.get(),
          on_destination_selected: move |i: usize| nav.set(i),
        },
      })
    }),
    heading("Bottom app bar"),
    ui!(SizedBox {
      width: 720.0,
      child: BottomAppBar {
        child: Row {
          spacing: 8.0,
          cross_axis_alignment: CrossAxisAlignment::Center,
          children: [
            IconButton { icon: Icons::ARCHIVE, on_pressed: act(g, |_| {}) },
            IconButton { icon: Icons::DELETE, on_pressed: act(g, |_| {}) },
            IconButton { icon: Icons::LABEL, on_pressed: act(g, |_| {}) },
            Spacer {},
            FloatingActionButton { child: Icon { icon: Icons::ADD }, on_pressed: act(g, |_| {}) },
          ],
        },
      },
    }),
    heading("Navigation drawer and rail"),
    ui!(Row {
      spacing: 16.0,
      main_axis_size: MainAxisSize::Min,
      cross_axis_alignment: CrossAxisAlignment::Center,
      children: [
        FilledButton {
          child: Text { text: "Open drawer" },
          icon: Icons::MENU,
          on_pressed: act(g, |g| g.drawer.set(true))
        },
        Text { text: "The rail on the left switches these pages." },
      ],
    }),
  ]
}

fn selection_page(_g: &Rc<Gallery>) -> Vec<Widget> {
  let labeled = |control: Widget, label: &str| {
    ui!(Row {
      spacing: 8.0,
      main_axis_size: MainAxisSize::Min,
      cross_axis_alignment: CrossAxisAlignment::Center,
      children: [control, Text { text: label.to_string() }],
    })
  };
  vec![
    heading("Checkbox, radio, switch"),
    watch(move |g| {
      let (checked, radio, switched) = (g.checked.clone(), g.radio.clone(), g.switched.clone());
      let radio_value = g.radio.get();
      let radios: Vec<Widget> = ["Small", "Medium", "Large"]
        .iter()
        .enumerate()
        .map(|(i, l)| {
          let radio = radio.clone();
          labeled(
            ui!(Radio {
              value: i,
              group_value: radio_value,
              on_changed: move |v: usize| radio.set(v)
            }),
            l,
          )
        })
        .collect();
      let mut row = vec![
        labeled(
          ui!(Checkbox { value: g.checked.get(), on_changed: move |v: bool| checked.set(v) }),
          "Checkbox",
        ),
        labeled(ui!(Checkbox { value: false }), "Disabled"),
      ];
      row.extend(radios);
      row.push(labeled(
        ui!(Switch { value: g.switched.get(), on_changed: move |v: bool| switched.set(v) }),
        "Switch",
      ));
      ui!(Row { spacing: 24.0, main_axis_size: MainAxisSize::Min, children: row })
    }),
    heading("Slider"),
    watch(|g| {
      let slider = g.slider.clone();
      let v = g.slider.get();
      ui!(Row {
        spacing: 16.0,
        main_axis_size: MainAxisSize::Min,
        cross_axis_alignment: CrossAxisAlignment::Center,
        children: [
          SizedBox {
            width: 480.0,
            child: Slider { value: v, max: 100.0, on_changed: move |x: f32| slider.set(x) }
          },
          Text { text: format!("{v:.0}") },
        ],
      })
    }),
    heading("Chips"),
    watch(|g| {
      let mut chips: Vec<Widget> = Vec::new();
      for (i, name) in ["Hiking", "Running", "Cycling"].iter().enumerate() {
        let filters = g.filters.clone();
        chips.push(ui!(FilterChip {
          label: name.to_string(),
          selected: g.filters.get()[i],
          on_selected: move |on: bool| {
            let mut f = filters.peek();
            f[i] = on;
            filters.set(f);
          },
        }));
      }
      for (i, name) in ["XS", "M", "XL"].iter().enumerate() {
        let choice = g.choice.clone();
        chips.push(ui!(ChoiceChip {
          label: name.to_string(),
          selected: g.choice.get() == i,
          on_selected: move |_on: bool| choice.set(i)
        }));
      }
      let presses = g.presses.clone();
      chips.push(ui!(ActionChip {
        label: "Add to calendar",
        avatar: Icons::CALENDAR_TODAY,
        on_pressed: move || presses.set(presses.peek() + 1)
      }));
      for (i, tag) in g.tags.get().into_iter().enumerate() {
        let tags = g.tags.clone();
        chips.push(ui!(InputChip {
          label: tag,
          avatar: Icons::LABEL,
          on_deleted: move || {
            let mut t = tags.peek();
            t.remove(i);
            tags.set(t);
          },
        }));
      }
      chips.push(ui!(Chip { label: "Plain chip", avatar: Icons::INFO }));
      ui!(Wrap { spacing: 8.0, run_spacing: 8.0, children: chips })
    }),
    heading("Menu"),
    watch(|g| {
      let (open, close) = (g.menu_open.clone(), g.menu_open.clone());
      let item = move |label: &str, icon: IconData| {
        let close = close.clone();
        ui!(MenuItemButton {
          child: Text { text: label.to_string() },
          leading_icon: icon,
          on_pressed: move || close.set(false)
        })
      };
      ui!(MenuAnchor {
        open: g.menu_open.get(),
        child: OutlinedButton {
          child: Text { text: "Open menu" },
          icon: Icons::EXPAND_MORE,
          on_pressed: move || open.set(!open.peek())
        },
        menu_children: [
          item("Copy", Icons::SHARE),
          item("Edit", Icons::EDIT),
          item("Delete", Icons::DELETE)
        ],
      })
    }),
    heading("Date and time pickers"),
    watch(|g| {
      let (date, time) = (g.date.clone(), g.time.clone());
      let d = g.date.get();
      let t = g.time.get();
      ui!(Row {
        spacing: 48.0,
        main_axis_size: MainAxisSize::Min,
        children: [
          Column {
            spacing: 8.0,
            main_axis_size: MainAxisSize::Min,
            children: [
              CalendarDatePicker { selected: d, on_date_changed: move |v: Date| date.set(v) },
              Text { text: format!("Selected {}-{:02}-{:02}", d.year, d.month, d.day) },
            ],
          },
          Column {
            spacing: 8.0,
            main_axis_size: MainAxisSize::Min,
            children: [
              TimePicker { time: t, on_changed: move |v: TimeOfDay| time.set(v) },
              Text { text: format!("{:02}:{:02}", t.hour, t.minute) },
            ],
          },
        ],
      })
    }),
  ]
}

fn inputs_page(g: &Rc<Gallery>) -> Vec<Widget> {
  let submitted = g.submitted.clone();
  vec![
    heading("Text fields"),
    ui!(SizedBox {
      width: 480.0,
      child: Column {
        spacing: 24.0,
        main_axis_size: MainAxisSize::Min,
        children: [
          TextField {
            controller: g.name.clone(),
            label: "Name",
            hint: "Type a name, Enter submits",
            prefix_icon: Icons::PERSON,
            on_submitted: move |v: String| submitted.set(v),
          },
          TextField {
            controller: g.email.clone(),
            label: "Email",
            variant: TextFieldVariant::Filled,
            suffix_icon: Icons::MAIL
          },
          TextField {
            controller: g.password.clone(),
            label: "Password",
            obscure_text: true,
            suffix_icon: Icons::VISIBILITY_OFF
          },
          TextField {
            controller: Signal::new("Read only".to_string()),
            label: "Disabled",
            disabled: true
          },
        ],
      },
    }),
    watch(|g| text(format!("Submitted name: {:?}", g.submitted.get()))),
  ]
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Widget)]
struct App {}

impl Element for App {
  fn build(&self, context: &Context) -> Option<Widget> {
    let g = context.read::<Gallery>();
    let page = g.page.get();
    let dark = g.dark.get();

    let content = match page {
      0 => actions_page(&g),
      1 => communication_page(&g),
      2 => containment_page(&g),
      3 => navigation_page(&g),
      4 => selection_page(&g),
      _ => inputs_page(&g),
    };

    let destinations: Vec<NavigationRailDestination> =
      [Icons::WIDGETS, Icons::INFO, Icons::DASHBOARD, Icons::EXPLORE, Icons::CHECK, Icons::EDIT]
        .iter()
        .zip(SHORT)
        .map(|(icon, label)| NavigationRailDestination {
          icon: *icon,
          selected_icon: None,
          label: label.to_string(),
        })
        .collect();

    let set_page = g.page.clone();
    let drawer_page = g.page.clone();
    let drawer_close = g.drawer.clone();

    let snack_bar = g.snack.get().then(|| {
      ui!(SnackBar {
        content: Text { text: "Message archived" },
        action: SnackBarAction { label: "Undo", on_pressed: act(&g, |g| g.snack.set(false)) },
      })
    });
    let bottom_sheet = g.sheet.get().then(|| {
      ui!(BottomSheet {
        modal: true,
        show_drag_handle: true,
        child: Column {
          main_axis_size: MainAxisSize::Min,
          children: [
            ListTile {
              leading: Icon { icon: Icons::SHARE },
              title: Text { text: "Share" },
              on_tap: act(&g, |g| g.sheet.set(false))
            },
            ListTile {
              leading: Icon { icon: Icons::DOWNLOAD },
              title: Text { text: "Download" },
              on_tap: act(&g, |g| g.sheet.set(false))
            },
            ListTile {
              leading: Icon { icon: Icons::CLOSE },
              title: Text { text: "Close" },
              on_tap: act(&g, |g| g.sheet.set(false))
            },
          ],
        },
      })
    });
    let drawer = g.drawer.get().then(|| {
      let mut children = vec![ui!(Container {
        padding: Padding { left: 16.0, top: 16.0, bottom: 16.0 },
        child: Text { text: "Pages" }
      })];
      for (icon, label) in
        [Icons::WIDGETS, Icons::INFO, Icons::DASHBOARD, Icons::EXPLORE, Icons::CHECK, Icons::EDIT]
          .iter()
          .zip(PAGES)
      {
        children.push(ui!(NavigationDrawerDestination { icon: *icon, label }));
      }
      children.push(ui!(Divider {}));
      children.push(ui!(NavigationDrawerDestination { icon: Icons::CLOSE, label: "Close drawer" }));
      ui!(NavigationDrawer {
        selected_index: page,
        on_destination_selected: move |i: usize| {
          if i < PAGES.len() {
            drawer_page.set(i);
          }
          drawer_close.set(false);
        },
        children,
      })
    });
    let dialog = if g.dialog.get() {
      ui!(AlertDialog {
        icon: Icons::DELETE,
        title: Text { text: "Discard draft?" },
        content: Text { text: "This removes the draft from this device. You can't undo it." },
        actions: [
          TextButton {
            child: Text { text: "Cancel" },
            on_pressed: act(&g, |g| g.dialog.set(false))
          },
          TextButton {
            child: Text { text: "Discard" },
            on_pressed: act(&g, |g| g.dialog.set(false))
          },
        ],
      })
    } else {
      ui!(SizedBox {})
    };

    let menu_close = g.menu_open.clone();
    Some(ui!(Theme {
      data: if dark { ThemeData::dark() } else { ThemeData::light() },
      child: InputEvent {
        event: move |e: &EventKey| {
          if e.key == 'q' {
            exit();
          }
          if e.key == keys::ESCAPE {
            menu_close.set(false);
          }
        },
        child: Stack {
          fit: StackFit::Expand,
          children: [
            Scaffold {
              app_bar: AppBar {
                leading: IconButton {
                  icon: Icons::MENU,
                  on_pressed: act(&g, |g| g.drawer.set(true))
                },
                title: Text { text: format!("Lobo Material 3 · {}", PAGES[page]) },
                actions: [
                  IconButton { icon: Icons::PALETTE, on_pressed: act(&g, |g| toggle(&g.dark)) },
                  IconButton { icon: Icons::HELP, on_pressed: act(&g, |g| g.dialog.set(true)) },
                ],
              },
              body: Row {
                children: [
                  NavigationRail {
                    destinations,
                    selected_index: page,
                    on_destination_selected: move |i: usize| set_page.set(i),
                    leading: FloatingActionButton {
                      child: Icon { icon: Icons::EDIT },
                      size: FabSize::Small,
                      on_pressed: act(&g, |g| g.snack.set(true))
                    },
                  },
                  VerticalDivider { width: 1.0 },
                  Expanded {
                    child: ListView {
                      padding: Padding { left: 24.0, top: 8.0, right: 24.0, bottom: 24.0 },
                      spacing: 16.0,
                      children: content
                    },
                  },
                ],
              },
              snack_bar,
              bottom_sheet,
              drawer,
            },
            dialog,
          ],
        },
      },
    }))
  }
}

fn main() {
  dotenvy::dotenv().ok();
  env_logger::init();

  run(ui!(Provider { create: |_| Gallery::new(), child: App {} }));
}
