//! Same dependent-state app as `dependent.rs`, using `run_latest`.
//!
//! Starting `select_user` again aborts the previous `select_user`, so an old
//! response never reaches its writes. No Slot, no counters, no checks.
//! Only the inputs and the tagged results are state; the rest is derived in `build`.
//!
//! Keys: `1`/`2`/`3` select a user, `c` toggles the currency, `q` quits.

use std::rc::Rc;
use std::time::Duration;

use anyhow::{Error, Result};
use lobo::*;

#[derive(Clone, Debug, PartialEq)]
struct Order {
    amount: i64,
}

async fn fetch_orders(user: u32) -> Result<Vec<Order>> {
    tokio::time::sleep(Duration::from_millis(rand::random_range(300..1500))).await;
    if rand::random_range(0..4) == 0 {
        return Err(Error::msg("network error while loading orders"));
    }
    Ok((0..3)
        .map(|_| Order {
            amount: rand::random_range(10..100) + user as i64,
        })
        .collect())
}

async fn fetch_rate(currency: &'static str) -> Result<f64> {
    tokio::time::sleep(Duration::from_millis(rand::random_range(200..1200))).await;
    if rand::random_range(0..4) == 0 {
        return Err(Error::msg("network error while loading rate"));
    }
    Ok(if currency == "EUR" { 0.9 } else { 1.0 })
}

struct Store {
    // Inputs: what the user asked for.
    selected_user: Signal<Option<u32>>,
    currency: Signal<&'static str>,
    // Results, each stored with the input it was fetched for.
    orders: Signal<(Option<u32>, Vec<Order>)>,
    rate: Signal<(&'static str, f64)>,
}

impl Store {
    fn new() -> Self {
        Store {
            selected_user: Signal::new(None),
            currency: Signal::new("USD"),
            orders: Signal::new((None, Vec::new())),
            rate: Signal::new(("USD", 1.0)),
        }
    }

    // The input is set right away; only the fetch can be aborted at its `.await`.
    async fn select_user(self: Rc<Self>, user: u32) -> Result<()> {
        self.selected_user.set(Some(user));
        let orders = fetch_orders(user).await?;
        self.orders.set((Some(user), orders));
        Ok(())
    }

    async fn toggle_currency(self: Rc<Self>) -> Result<()> {
        let next = if self.currency.get() == "USD" { "EUR" } else { "USD" };
        self.currency.set(next);
        let rate = fetch_rate(next).await?;
        self.rate.set((next, rate));
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
    fn on_key(&self, context: &Context, key: char) {
        let store = context.read::<Store>();
        match key {
            '1' | '2' | '3' => {
                let user = key.to_digit(10).unwrap();
                store.clone().select_user(user).run_latest();
            }
            'c' => store.clone().toggle_currency().run_latest(),
            _ => {}
        }
    }

    fn build(&self, _: &Context) -> Option<Widget> {
        Some(
            ui!(Container(
                padding: Padding(left: 8.0, top: 4.0),
                child: Column(
                    spacing: 1.0,
                    children: [
                        SignalBuilder(
                            builder: |context: &Context| {
                                let store = context.read::<Store>();
                                let selected = store.selected_user.get();
                                let orders_for = store.orders.with(|(for_user, _)| *for_user);
                                let flag = if selected != orders_for { "  (loading)" } else { "" };
                                Some(ui!(Text(text: format!("User: {selected:?} | orders for: {orders_for:?}{flag}"))).into())
                            }
                        ),
                        SignalBuilder(
                            builder: |context: &Context| {
                                let store = context.read::<Store>();
                                let currency = store.currency.get();
                                let (rate_for, rate) = store.rate.get();
                                let flag = if currency != rate_for { "  (loading)" } else { "" };
                                Some(ui!(Text(text: format!("Currency: {currency} | rate {rate} for {rate_for}{flag}"))).into())
                            }
                        ),
                        SignalBuilder(
                            builder: |context: &Context| {
                                // Derived on read: it can't go stale.
                                let store = context.read::<Store>();
                                let sum: i64 = store.orders.with(|(_, o)| o.iter().map(|o| o.amount).sum());
                                let (_, rate) = store.rate.get();
                                Some(ui!(Text(text: format!("Total: {:.2}", sum as f64 * rate))).into())
                            }
                        ),
                        SignalBuilder(
                            builder: |context: &Context| {
                                let (_, orders) = context.read::<Store>().orders.get();
                                let rows: Vec<Widget> = orders
                                    .iter()
                                    .map(|o| Text { text: format!("order: {}", o.amount) }.into())
                                    .collect();
                                Some(Column { spacing: 0.0, children: rows }.into())
                            }
                        ),
                        SignalBuilder(
                            builder: |_context: &Context| {
                                let errors = errors().get();
                                let rows: Vec<Widget> = errors
                                    .iter()
                                    .rev()
                                    .take(3)
                                    .map(|e| Text { text: format!("error: {e}") }.into())
                                    .collect();
                                Some(Column { spacing: 0.0, children: rows }.into())
                            }
                        ),
                    ],
                )
            ))
            .into(),
        )
    }
}

fn main() {
    dotenvy::dotenv().ok();
    env_logger::init();

    let app = ui!(
        Root(
            child: Provider(
                create: |_| Store::new(),
                child: Hello()
            )
        )
    );

    run(app);
}
