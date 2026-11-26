//! Stress example: many concurrent async jobs, a background ticker, and widgets
//! whose shape depends on the shared state. Used to find where the current
//! API breaks down. Keys: `+` starts 3 jobs, `t` starts a ticker, `q` quits.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use lobo::*;

#[derive(Default)]
struct Dashboard {
  count: Signal<i32>,
  pending: Signal<usize>,
  ticks: Signal<u32>,
  log: Signal<Vec<String>>,
  next_id: Cell<usize>,
}

impl Dashboard {
  async fn fetch_delta(&self) -> i32 {
    let ms = rand::random_range(200..1500);
    tokio::time::sleep(Duration::from_millis(ms)).await;
    rand::random_range(-3..4)
  }

  // Takes `Rc<Self>` so the future owns the handle and can be spawned.
  async fn job(self: Rc<Self>, id: usize) {
    self.pending.set(self.pending.get() + 1);

    let delta = self.fetch_delta().await;
    self.count.set(self.count.get() + delta);
    let mut log = self.log.get();
    log.push(format!("job {id}: {delta:+}"));
    self.log.set(log);

    self.pending.set(self.pending.get() - 1);
  }

  async fn ticker(self: Rc<Self>) {
    loop {
      tokio::time::sleep(Duration::from_millis(500)).await;
      self.ticks.set(self.ticks.get() + 1);
    }
  }
}

#[derive(Widget)]
struct Root {
  child: Widget,
}

impl Element for Root {
  fn build(&self, _context: &Context) -> Option<Widget> {
    Some(self.child.clone())
  }
}

#[derive(Widget)]
struct Hello {}

impl Element for Hello {
  fn build(&self, context: &Context) -> Option<Widget> {
    let dashboard = context.read::<Dashboard>();
    Some(ui!(InputEvent {
      event: move |event: &EventKey| {
        match event.key {
          '+' => {
            for _ in 0..3 {
              let id = dashboard.next_id.get();
              dashboard.next_id.set(id + 1);
              dashboard.clone().job(id).run_async();
            }
          }
          // Unguarded on purpose: each `t` adds another ticker.
          't' => {
            dashboard.clone().ticker().run_async();
          }
          _ => {}
        }
      },
      child: Container {
        padding: Padding { left: 8.0, top: 4.0 },
        child: Column {
          spacing: 1.0,
          children: [
            // Shape depends on the count: winning / losing / even.
            SignalBuilder {
              builder: |context: &Context| {
                let count = context.read::<Dashboard>().count.get();
                let label = match count {
                  c if c > 0 => "Winning",
                  c if c < 0 => "Losing",
                  _ => "Even",
                };
                Some(ui!(Text { text: format!("Count: {count} ({label})") }))
              }
            },
            SignalBuilder {
              builder: |context: &Context| {
                let pending = context.read::<Dashboard>().pending.get();
                Some(ui!(Text { text: format!("In flight: {pending}") }))
              }
            },
            SignalBuilder {
              builder: |context: &Context| {
                let ticks = context.read::<Dashboard>().ticks.get();
                Some(ui!(Text { text: format!("Ticks: {ticks}") }))
              }
            },
            // Variable number of rows: one Text per log entry.
            SignalBuilder {
              builder: |context: &Context| {
                let log = context.read::<Dashboard>().log.get();
                let rows: Vec<Widget> =
                  log.iter().rev().take(10).map(|line| ui!(Text { text: line.clone() })).collect();
                Some(ui!(Column { children: rows }))
              }
            },
          ],
        },
      },
    }))
  }
}

fn main() {
  dotenvy::dotenv().ok();
  env_logger::init();

  let app = ui!(Root { child: Provider { create: |_| Dashboard::default(), child: Hello {} } });

  run(app);
}
