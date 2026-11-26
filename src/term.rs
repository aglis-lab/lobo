//! Terminal input. In raw mode every key press arrives at once (no Enter needed);
//! in line mode (stdin is not a terminal, or `LOBO_INPUT=line`) input is read a line at a time.

use std::io::{BufRead, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, terminal};
use tokio::sync::mpsc::UnboundedSender;

use crate::{EventKey, Modifiers, keys};

/// How `run` reads the keyboard. Set with `LOBO_INPUT=raw|line`; by default raw when
/// both stdin and stdout are terminals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
  /// Each key press is delivered immediately; Enter is a key like any other.
  Raw,
  /// A line at a time (`parse_keys`); an empty line is Enter.
  Line,
}

impl InputMode {
  pub fn from_env() -> Self {
    match std::env::var("LOBO_INPUT").as_deref() {
      Ok("raw") => InputMode::Raw,
      Ok("line") => InputMode::Line,
      _ if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() => InputMode::Raw,
      _ => InputMode::Line,
    }
  }
}

/// What the input thread sends to the run loop.
pub(crate) enum Input {
  Key(EventKey),
  Line(String),
  Resize,
  /// End of input, or Ctrl+C / Ctrl+D in raw mode.
  Quit,
}

/// Translates a crossterm key press into Lobo's key (`keys::*` for special keys).
/// Returns `None` for releases and keys Lobo has no code for (F-keys, Home, ...).
pub fn map_key(event: KeyEvent) -> Option<EventKey> {
  if event.kind == KeyEventKind::Release {
    return None;
  }
  let m = event.modifiers;
  let modifiers = Modifiers {
    ctrl: m.contains(KeyModifiers::CONTROL),
    alt: m.contains(KeyModifiers::ALT),
    shift: m.contains(KeyModifiers::SHIFT),
  };
  let key = match event.code {
    KeyCode::Char(c) => c,
    KeyCode::Enter => keys::ENTER,
    KeyCode::Tab => keys::TAB,
    KeyCode::BackTab => keys::BACK_TAB,
    KeyCode::Esc => keys::ESCAPE,
    KeyCode::Backspace => keys::BACKSPACE,
    KeyCode::Delete => keys::DELETE,
    KeyCode::Up => keys::UP,
    KeyCode::Down => keys::DOWN,
    KeyCode::Left => keys::LEFT,
    KeyCode::Right => keys::RIGHT,
    _ => return None,
  };
  // Shift is already in the character ('A'); keep it only for special keys.
  let modifiers = if matches!(event.code, KeyCode::Char(_)) {
    Modifiers { shift: false, ..modifiers }
  } else {
    modifiers
  };
  Some(EventKey::with_modifiers(key, modifiers))
}

/// Puts the terminal in raw mode on the alternate screen; `Drop` (and a panic) restores it.
pub(crate) struct RawTerminal {
  stop: Arc<AtomicBool>,
}

impl RawTerminal {
  pub(crate) fn enter() -> std::io::Result<Self> {
    terminal::enable_raw_mode()?;
    // Alternate screen, hidden cursor, no line wrap (a frame wider than the terminal is cut).
    execute!(
      std::io::stdout(),
      terminal::EnterAlternateScreen,
      cursor::Hide,
      terminal::DisableLineWrap
    )?;
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
      restore();
      previous(info);
    }));
    Ok(RawTerminal { stop: Arc::new(AtomicBool::new(false)) })
  }

  /// Reads key presses and resizes on a thread until this terminal is dropped.
  pub(crate) fn spawn_reader(&self, tx: UnboundedSender<Input>) {
    let stop = self.stop.clone();
    std::thread::spawn(move || {
      while !stop.load(Ordering::Relaxed) {
        // Poll with a timeout so the thread notices `stop` and releases stdin.
        match event::poll(Duration::from_millis(100)) {
          Ok(true) => {}
          Ok(false) => continue,
          Err(_) => break,
        }
        let input = match event::read() {
          Ok(Event::Key(k)) => {
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            // Raw mode turns off the terminal's Ctrl+C; keep it (and Ctrl+D) as quit.
            if ctrl && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('d')) {
              Input::Quit
            } else {
              match map_key(k) {
                Some(key) => Input::Key(key),
                None => continue,
              }
            }
          }
          Ok(Event::Resize(..)) => Input::Resize,
          Ok(_) => continue,
          Err(_) => Input::Quit,
        };
        let quit = matches!(input, Input::Quit);
        if tx.send(input).is_err() || quit {
          break;
        }
      }
    });
  }
}

impl Drop for RawTerminal {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Relaxed);
    restore();
  }
}

fn restore() {
  let mut out = std::io::stdout();
  let _ = execute!(out, terminal::EnableLineWrap, cursor::Show, terminal::LeaveAlternateScreen);
  let _ = terminal::disable_raw_mode();
  let _ = out.flush();
}

/// Line mode: blocking stdin lines on a thread.
pub(crate) fn spawn_line_reader(tx: UnboundedSender<Input>) {
  std::thread::spawn(move || {
    for line in std::io::stdin().lock().lines() {
      let Ok(line) = line else { break };
      if tx.send(Input::Line(line)).is_err() {
        return;
      }
    }
    let _ = tx.send(Input::Quit);
  });
}
