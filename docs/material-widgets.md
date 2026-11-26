# Material widgets: implemented and missing

Coverage of the Flutter Material component catalog
(<https://docs.flutter.dev/ui/widgets/material>, read on 2026-11-25: 28 entries in 6 groups)
by Lobo's widgets, rendered through the `Renderer` trait in the terminal at 720p.

Run `cargo run --example gallery` to see all of them (make the terminal at least 160x46).

**Status**
- **Done**: the component and its main variants work, with keyboard input where it applies.
- **Partial**: usable, but a notable part of the Flutter component is missing (listed in Notes).
- **Missing**: not implemented.

Every interactive widget works with the keyboard only (there is no pointer input yet), and
nothing animates. See [Cross-cutting gaps](#cross-cutting-gaps).

## Catalog coverage

### Actions

| Catalog entry | Lobo widget | Status | Notes |
|---|---|---|---|
| Common buttons | `ElevatedButton`, `FilledButton` (`tonal: true` = `FilledButton.tonal`), `OutlinedButton`, `TextButton` | Done | `icon:` adds a leading icon (Flutter's `.icon` constructors). `on_pressed: None` disables. No `style:` override yet. |
| FloatingActionButton | `FloatingActionButton` | Done | `size: FabSize::Small / Regular / Large`. |
| Extended FloatingActionButton | `FloatingActionButton { label: ... }` | Done | Setting `label` makes it extended; `child` is the icon. |
| IconButton | `IconButton` | Done | `variant: Standard / Filled / FilledTonal / Outlined`. No toggle (`isSelected`) state. |
| SegmentedButton | `SegmentedButton`, `ButtonSegment` | Done | Segments are identified by index (no generic `T`). Single or multi selection, `empty_selection_allowed`. |

### Communication

| Catalog entry | Lobo widget | Status | Notes |
|---|---|---|---|
| Badge | `Badge` | Done | Dot (no label) or text label; `hidden`. |
| LinearProgressIndicator | `LinearProgressIndicator` (+ `CircularProgressIndicator`) | Partial | Determinate works. Indeterminate is drawn as a still frame (no animation). |
| SnackBar | `SnackBar`, `SnackBarAction` | Partial | Shown while set in `Scaffold::snack_bar`. No `ScaffoldMessenger`, auto-dismiss timer or slide animation. |

### Containment

| Catalog entry | Lobo widget | Status | Notes |
|---|---|---|---|
| AlertDialog | `AlertDialog` | Partial | Icon, title, content, actions, scrim; takes the focus when it opens. Shown declaratively (last child of a `Stack`): there is no `showDialog` / route, and Tab is not trapped inside the dialog. |
| Bottom sheet | `BottomSheet` | Partial | Standard and `modal: true` (scrim, takes focus), drag handle. Shown via `Scaffold::bottom_sheet`. No dragging, no `showModalBottomSheet`. |
| Card | `Card` | Done | `variant: Elevated / Filled / Outlined`, `color`, `margin`. |
| Divider | `Divider`, `VerticalDivider` | Done | `height`/`width`, `thickness`, `indent`, `end_indent`, `color`. |
| ListTile | `ListTile` | Done | Leading, title, subtitle, trailing, `on_tap`, `selected`, `disabled`. No `dense` / `isThreeLine` / `CheckboxListTile` family (compose with `Checkbox` instead). |

### Navigation

| Catalog entry | Lobo widget | Status | Notes |
|---|---|---|---|
| AppBar | `AppBar` | Partial | Small top app bar: leading, title, actions, `center_title`. No medium / large / `SliverAppBar`, no scroll-under elevation. |
| Bottom app bar | `BottomAppBar` | Done | |
| NavigationBar | `NavigationBar`, `NavigationDestination` | Done | Indicator, selected icon. No label behavior option (always shown). |
| NavigationDrawer | `NavigationDrawer`, `NavigationDrawerDestination` | Partial | Headers and dividers between destinations. Modal via `Scaffold::drawer` (scrim, takes focus) or permanent in a `Row`. No slide animation. |
| Navigation rail | `NavigationRail`, `NavigationRailDestination` | Partial | Leading widget, labels always shown. No `extended` mode or `trailing`. |
| TabBar | `TabBar`, `Tab`, `TabBarView` | Partial | Primary tabs with text and optional icon. No secondary tabs, no scrollable tabs, no `TabController` / swipe. |

### Selection

| Catalog entry | Lobo widget | Status | Notes |
|---|---|---|---|
| Checkbox | `Checkbox` | Partial | No tristate (`value: null`). |
| Chip | `Chip`, `ActionChip`, `FilterChip`, `ChoiceChip`, `InputChip` | Done | Delete (`x` / Delete key) on `Chip` and `InputChip`. No elevated variants. Lay out with `Wrap`. |
| DatePicker | `CalendarDatePicker`, `Date` | Partial | Month calendar with `first_date` / `last_date`; arrows move a day / week, `<` `>` a month. Missing: `showDatePicker` dialog, text input mode, year picker, date *range* picker. |
| Menu | `MenuAnchor`, `MenuItemButton` | Partial | Popup above everything, first item takes the focus. Missing: `DropdownMenu`, `PopupMenuButton`, `MenuBar`, `SubmenuButton`, closing on outside tap. |
| Radio | `Radio` | Done | Options identified by index (no generic `T`). No `RadioGroup`. |
| Slider | `Slider` | Partial | `min`, `max`, `divisions`; arrows / `+` `-` adjust. No value label, no `RangeSlider`. |
| Switch | `Switch` | Done | No thumb icon. |
| TimePicker | `TimePicker`, `TimeOfDay` | Partial | Input mode only (hour, minute, AM/PM fields; Up/Down adjust). Missing: the clock dial and `showTimePicker`. |

### Text inputs

| Catalog entry | Lobo widget | Status | Notes |
|---|---|---|---|
| TextField | `TextField`, `TextFieldVariant` | Partial | Outlined / filled, floating label, hint, prefix / suffix icon, `obscure_text`, `max_length`, `on_changed`, `on_submitted`; text in a `Signal<String>` (the controller). Missing: cursor movement, selection, multi-line, error / helper text, input formatters. |

**Totals (28 catalog entries):** 14 done, 14 partial, 0 missing entirely. The missing parts are listed per row above.

## Supporting widgets added

These aren't catalog entries, but the components need them:

- Layout: `Row`, `Column` (alignment and size options), `Stack` + `Positioned`, `Align`, `Center`,
  `SizedBox`, `Expanded`, `Flexible`, `Spacer`, `Wrap`, `ListView` (scrolls to keep the focused
  widget visible), `Container` (now with color, border, radius, size, margin, alignment).
- Material: `Scaffold` (app bar, body, FAB, bottom navigation, bottom sheet, snack bar, drawer),
  `Theme` / `ThemeData` / `ColorScheme` / `TextTheme` (Material 3 baseline, light and dark),
  `Icon` + `Icons` (62 icons), `DefaultTextStyle`, `Text` (wrapping, `max_lines`, `style`).

## Rendering

- `Renderer` (`src/render/mod.rs`) is the one interface all text and widgets draw through.
  A backend implements the primitives (rects, outlines, lines, circles, text, clipping,
  text measuring). The widget-level methods (`draw_box`, `draw_icon`, `draw_checkbox`,
  `draw_radio`, `draw_switch`, `draw_slider`, `draw_linear_progress`,
  `draw_circular_progress`, `draw_focus`, `draw_caret`) have default implementations
  built from the primitives. A future texture backend only needs the primitives, and
  can override the rest for better quality.
- `TerminalRenderer` (`src/render/terminal.rs`) draws a 1280x720 logical-pixel canvas into 160x45
  cells (8x16 px each) with 24-bit ANSI color. Rects shorter than 44 px collapse to a single row,
  so a 40 px button is one row. Small controls are drawn as glyphs (`[✓]`, `◉`, `━━●──`).
- Layout is in logical pixels using real Material sizes, so the same tree should lay out
  identically on a pixel backend.
- `LOBO_RENDER=plain` prints frames as plain text (used for tests and piping), and
  `LOBO_RENDER=tree` prints the old widget-tree dump.

## Cross-cutting gaps

These affect every component:

1. **No pointer input.** The keyboard is read in raw mode (each key acts at once, via
   `crossterm`) when stdin and stdout are terminals, and a line at a time otherwise
   (`LOBO_INPUT=line|raw` overrides). Tab / Shift-Tab move the focus; Enter or space presses
   the focused widget; Ctrl+C or Ctrl+D quits. Mouse events are not handled yet.
2. **No animation**: no ripples, state-layer transitions, indeterminate progress or sheet/drawer motion.
3. **No hover / pressed states.** Only the focus indicator is shown.
4. **No routes or overlay API** (`Navigator`, `showDialog`, `showModalBottomSheet`). Dialogs,
   sheets, drawers and snack bars appear while a field is set. `MenuAnchor` is the only real
   overlay (it is painted after the whole tree).
5. **Focus order is paint order**, and modal surfaces don't trap the focus.
6. **Children are matched by position and type unless keyed.** A rebuild updates existing child
   nodes in place (state, scroll offsets and focus are kept). If items of a list are inserted,
   removed or reordered, give each one `Widget::with_key(id)`, or state may stay at the old
   position. A rebuilt widget rebuilds its whole subtree (only an identical child widget value is
   skipped), so `SignalBuilder` is still the way to keep rebuilds narrow (the gallery does).
7. **Generic widgets use indices.** `#[derive(Widget)]` doesn't support generic structs, so
   `Radio`, `SegmentedButton` and the navigation widgets identify options by `usize`.
8. **Terminal glyphs are approximations.** A few may be drawn two cells wide or as emoji by some
   fonts / terminals (`⚙`, `⚠`, `➤`). Output uses 24-bit color; a terminal without truecolor support (possibly older macOS Terminal.app, not verified) will show wrong colors.
9. **The color tokens were written from memory** of the Material 3 baseline scheme and need
   checking against m3.material.io.
10. **`ui!` list literals infer integers as `i32`.** Inside `[...]` each item goes through
    `Into::into`, so write `selected: vec![1]` (or `1usize`) for a `Vec<usize>` field.
