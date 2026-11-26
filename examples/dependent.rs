//! Dependent-state example: derived values depend on several async sources
//! that answer in any order. The same app runs with one of four strategies for
//! handling an older response that arrives after a newer one:
//!
//! - `naive`  (default): nothing cancels or orders the responses. The last one
//!   to land wins, so the UI can settle on a user or currency that is no longer
//!   selected.
//! - `seq`:    each kind of request takes a `Latest` ticket when it starts. After
//!   its `.await` the task drops its result unless its ticket is still the newest.
//!   Nothing is cancelled: the old request still runs to the end.
//! - `slot`:   each kind of request has a `Slot`. Starting a new request aborts the
//!   previous one, so an old response never reaches its writes.
//! - `latest`: `run_latest` does the same as `slot` with no field to declare:
//!   starting a future aborts the previous future of the same type.
//!
//! Run: `cargo run --example dependent -- [naive|seq|slot|latest]`
//!
//! Keys: `1`/`2`/`3` select a user (loads its orders), `c` toggles the currency
//! (loads its rate), Ctrl+C quits. Keys act as soon as they are pressed; type quickly,
//! so `123c c` starts several overlapping requests.
//!
//! Only the inputs and the tagged results are state; the total is derived in `build`.

use std::rc::Rc;
use std::time::Duration;

use anyhow::{Error, Result};
use lobo::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Strategy {
  Naive,
  Seq,
  Slot,
  Latest,
}

impl Strategy {
  fn from_args() -> Self {
    match std::env::args().nth(1).as_deref() {
      None | Some("naive") => Strategy::Naive,
      Some("seq") => Strategy::Seq,
      Some("slot") => Strategy::Slot,
      Some("latest") => Strategy::Latest,
      Some(other) => {
        eprintln!("unknown strategy `{other}`, expected naive|seq|slot|latest");
        std::process::exit(2);
      }
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
struct Order {
  amount: i64,
}

async fn fetch_orders(user: u32) -> Result<Vec<Order>> {
  tokio::time::sleep(Duration::from_millis(rand::random_range(300..1500))).await;
  if rand::random_range(0..4) == 0 {
    return Err(Error::msg("network error while loading orders"));
  }
  Ok((0..3).map(|_| Order { amount: rand::random_range(10..100) + user as i64 }).collect())
}

async fn fetch_rate(currency: &'static str) -> Result<f64> {
  tokio::time::sleep(Duration::from_millis(rand::random_range(200..1200))).await;
  if rand::random_range(0..4) == 0 {
    return Err(Error::msg("network error while loading rate"));
  }
  Ok(if currency == "EUR" { 0.9 } else { 1.0 })
}

struct Store {
  strategy: Strategy,

  // Inputs: what the user asked for.
  selected_user: Signal<Option<u32>>,
  currency: Signal<&'static str>,
  // Results, each stored with the input it was fetched for.
  orders: Signal<(Option<u32>, Vec<Order>)>,
  rate: Signal<(&'static str, f64)>,

  // Used by `seq`: one ticket counter per kind of request.
  user_latest: Latest,
  rate_latest: Latest,
  // Used by `slot`: one slot per kind of request.
  user_slot: Slot,
  rate_slot: Slot,
}

impl Store {
  fn new(strategy: Strategy) -> Self {
    Store {
      strategy,
      selected_user: Signal::new(None),
      currency: Signal::new("USD"),
      orders: Signal::new((None, Vec::new())),
      rate: Signal::new(("USD", 1.0)),
      user_latest: Latest::default(),
      rate_latest: Latest::default(),
      user_slot: Slot::default(),
      rate_slot: Slot::default(),
    }
  }

  // The input is set right away, so a cancelled request can't lose a key press.
  // Only the slow fetch is subject to the strategy.
  fn select_user(self: Rc<Self>, user: u32) {
    self.selected_user.set(Some(user));
    let task = |ticket| self.clone().load_orders(user, ticket);
    match self.strategy {
      Strategy::Naive => spawn(task(None)),
      Strategy::Seq => spawn(task(Some(self.user_latest.start()))),
      Strategy::Slot => self.user_slot.restart(task(None)),
      Strategy::Latest => task(None).run_latest(),
    }
  }

  fn toggle_currency(self: Rc<Self>) {
    let next = if self.currency.get() == "USD" { "EUR" } else { "USD" };
    self.currency.set(next);
    let task = |ticket| self.clone().load_rate(next, ticket);
    match self.strategy {
      Strategy::Naive => spawn(task(None)),
      Strategy::Seq => spawn(task(Some(self.rate_latest.start()))),
      Strategy::Slot => self.rate_slot.restart(task(None)),
      Strategy::Latest => task(None).run_latest(),
    }
  }

  // `ticket` is only set by `seq`. `slot` and `latest` abort an older task at
  // its `.await`, so it never gets here; `naive` lets every task through.
  async fn load_orders(self: Rc<Self>, user: u32, ticket: Option<Ticket>) -> Result<()> {
    let result = fetch_orders(user).await;
    if ticket.is_some_and(|t| !t.is_current()) {
      return Ok(()); // Superseded: drop the result, and its error too.
    }
    self.orders.set((Some(user), result?));
    Ok(())
  }

  async fn load_rate(self: Rc<Self>, next: &'static str, ticket: Option<Ticket>) -> Result<()> {
    let result = fetch_rate(next).await;
    if ticket.is_some_and(|t| !t.is_current()) {
      return Ok(());
    }
    self.rate.set((next, result?));
    Ok(())
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
    let store = context.read::<Store>();
    Some(ui!(InputEvent {
      event: move |event: &EventKey| {
        match event.key {
          '1' | '2' | '3' => {
            let user = event.key.to_digit(10).unwrap();
            store.clone().select_user(user);
          }
          'c' => store.clone().toggle_currency(),
          _ => {}
        }
      },
      child: Container {
        padding: Padding { left: 8.0, top: 4.0 },
        child: Column {
          spacing: 1.0,
          children: [
            SignalBuilder {
              builder: |context: &Context| {
                let strategy = context.read::<Store>().strategy;
                Some(ui!(Text { text: format!("Strategy: {strategy:?}") }))
              }
            },
            SignalBuilder {
              builder: |context: &Context| {
                let store = context.read::<Store>();
                let selected = store.selected_user.get();
                let orders_for = store.orders.with(|(for_user, _)| *for_user);
                let flag = mismatch_flag(store.strategy, selected != orders_for);
                Some(ui!(Text {
                  text: format!("User: {selected:?} | orders for: {orders_for:?}{flag}")
                }))
              }
            },
            SignalBuilder {
              builder: |context: &Context| {
                let store = context.read::<Store>();
                let currency = store.currency.get();
                let (rate_for, rate) = store.rate.get();
                let flag = mismatch_flag(store.strategy, currency != rate_for);
                Some(ui!(Text {
                  text: format!("Currency: {currency} | rate {rate} for {rate_for}{flag}")
                }))
              }
            },
            SignalBuilder {
              builder: |context: &Context| {
                // Derived on read: it always matches the orders and rate shown.
                let store = context.read::<Store>();
                let sum: i64 = store.orders.with(|(_, o)| o.iter().map(|o| o.amount).sum());
                let (_, rate) = store.rate.get();
                Some(ui!(Text { text: format!("Total: {:.2}", sum as f64 * rate) }))
              }
            },
            SignalBuilder {
              builder: |context: &Context| {
                let (_, orders) = context.read::<Store>().orders.get();
                let rows: Vec<Widget> = orders
                  .iter()
                  .map(|o| ui!(Text { text: format!("order: {}", o.amount) }))
                  .collect();
                Some(ui!(Column { children: rows }))
              }
            },
            SignalBuilder {
              builder: |_context: &Context| {
                let errors = errors().get();
                let rows: Vec<Widget> = errors
                  .iter()
                  .rev()
                  .take(3)
                  .map(|e| ui!(Text { text: format!("error: {e}") }))
                  .collect();
                Some(ui!(Column { children: rows }))
              }
            },
          ],
        },
      },
    }))
  }
}

// A result that doesn't match its input is either still loading, or (with
// `naive` only) a stale response that landed last and will never be corrected.
// A failed fetch also leaves a mismatch until the next request.
fn mismatch_flag(strategy: Strategy, mismatch: bool) -> &'static str {
  match (mismatch, strategy) {
    (false, _) => "",
    (true, Strategy::Naive) => "  (loading, or STALE)",
    (true, _) => "  (loading)",
  }
}

fn main() {
  dotenvy::dotenv().ok();
  env_logger::init();

  let strategy = Strategy::from_args();
  let app =
    ui!(Root { child: Provider { create: move |_| Store::new(strategy), child: Hello {} } });

  run(app);
}
