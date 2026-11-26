//! Material icon set (a subset). Each icon carries its Material Symbols name, for a
//! font-based backend, and a single-width Unicode stand-in for the terminal.
//!
//! The glyphs are approximations: some terminals or fonts may render a few of them
//! (e.g. `⚙`, `⚠`) two cells wide or as emoji.

use crate::render::IconData;

/// `Icons::ADD`, `Icons::MENU`, ... (Flutter's `Icons.add`).
pub struct Icons;

macro_rules! icons {
  ($($id:ident = $name:literal, $glyph:literal;)*) => {
    impl Icons {
      $(pub const $id: IconData = IconData { name: $name, glyph: $glyph };)*

      /// Every icon in the set.
      pub const ALL: &'static [IconData] = &[$(Icons::$id),*];
    }
  };
}

icons! {
  ADD = "add", '+';
  REMOVE = "remove", '−';
  MENU = "menu", '☰';
  SEARCH = "search", '⌕';
  CLOSE = "close", '✕';
  CHECK = "check", '✓';
  ARROW_BACK = "arrow_back", '←';
  ARROW_FORWARD = "arrow_forward", '→';
  ARROW_UPWARD = "arrow_upward", '↑';
  ARROW_DOWNWARD = "arrow_downward", '↓';
  CHEVRON_LEFT = "chevron_left", '‹';
  CHEVRON_RIGHT = "chevron_right", '›';
  EXPAND_MORE = "expand_more", '⌄';
  EXPAND_LESS = "expand_less", '⌃';
  MORE_VERT = "more_vert", '⋮';
  MORE_HORIZ = "more_horiz", '⋯';
  HOME = "home", '⌂';
  SETTINGS = "settings", '⚙';
  FAVORITE = "favorite", '♥';
  FAVORITE_BORDER = "favorite_border", '♡';
  STAR = "star", '★';
  STAR_BORDER = "star_border", '☆';
  PERSON = "person", '☺';
  MAIL = "mail", '✉';
  INBOX = "inbox", '⊡';
  SEND = "send", '➤';
  EDIT = "edit", '✎';
  DELETE = "delete", '✗';
  SHARE = "share", '⤴';
  INFO = "info", 'ℹ';
  WARNING = "warning", '⚠';
  ERROR = "error", '⊗';
  HELP = "help", '?';
  NOTIFICATIONS = "notifications", '♪';
  CALENDAR_TODAY = "calendar_today", '▤';
  SCHEDULE = "schedule", '◷';
  PHONE = "phone", '☎';
  IMAGE = "image", '▣';
  FOLDER = "folder", '⌸';
  ATTACH_FILE = "attach_file", '⌇';
  DOWNLOAD = "download", '⤓';
  UPLOAD = "upload", '⤒';
  REFRESH = "refresh", '↻';
  FILTER_LIST = "filter_list", '⋿';
  SORT = "sort", '⇅';
  LIST = "list", '≣';
  GRID_VIEW = "grid_view", '▦';
  DASHBOARD = "dashboard", '▥';
  PLAY_ARROW = "play_arrow", '▶';
  PAUSE = "pause", '‖';
  BOOKMARK = "bookmark", '⚑';
  EXPLORE = "explore", '⊕';
  LABEL = "label", '⊳';
  ARCHIVE = "archive", '⊟';
  LOCK = "lock", '⊠';
  VISIBILITY = "visibility", '◎';
  VISIBILITY_OFF = "visibility_off", '⊘';
  LOCATION_ON = "location_on", '⌖';
  SHOPPING_CART = "shopping_cart", '⊻';
  LOGOUT = "logout", '⇥';
  PALETTE = "palette", '◐';
  WIDGETS = "widgets", '◇';
}
