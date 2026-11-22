//! Same dependent-state app as `dependent.rs`, with `Slot` instead of tickets.
//!
//! Each kind of request has a slot. Starting a new request aborts the previous one,
//! so an old response never reaches its writes. No counters, no checks.
//!
//! Keys: `1`/`2`/`3` select a user, `c` toggles the currency, `q` quits.
//

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
    selected_user: Signal<Option<u32>>,
    orders: Signal<Vec<Order>>,
    orders_for: Signal<Option<u32>>,
    currency: Signal<&'static str>,
    rate: Signal<f64>,
    rate_for: Signal<&'static str>,
    total: Signal<f64>,
    status: Signal<String>,

    // One slot per kind of request.
    user_slot: Slot,
    rate_slot: Slot,
}

impl Store {
    fn new() -> Self {
        Store {
            selected_user: Signal::new(None),
            orders: Signal::new(Vec::new()),
            orders_for: Signal::new(None),
            currency: Signal::new("USD"),
            rate: Signal::new(1.0),
            rate_for: Signal::new("USD"),
            total: Signal::new(0.0),
            status: Signal::new("idle".to_string()),
            user_slot: Slot::default(),
            rate_slot: Slot::default(),
        }
    }

    fn recompute(&self) {
        let sum: i64 = self.orders.with(|o| o.iter().map(|o| o.amount).sum());
        self.total.set(sum as f64 * self.rate.get());
    }

    // What the user asked for is set right away. Only the slow fetch is cancellable,
    // so an aborted request can't lose a key press.
    fn select_user(self: Rc<Self>, user: u32) {
        self.selected_user.set(Some(user));
        self.status.set(format!("loading user {user}"));
        let store = self.clone();
        self.user_slot
            .restart(async move { store.load_orders(user).await });
    }

    fn toggle_currency(self: Rc<Self>) {
        let next = if self.currency.get() == "USD" {
            "EUR"
        } else {
            "USD"
        };
        self.currency.set(next);
        let store = self.clone();
        self.rate_slot
            .restart(async move { store.load_rate(next).await });
    }

    // Only the newest task reaches these writes: an older one is aborted at its `.await`.
    async fn load_orders(self: Rc<Self>, user: u32) -> Result<()> {
        let orders = fetch_orders(user)
            .await
            .inspect_err(|_| self.status.set("failed".to_string()))?;

        self.orders.set(orders);
        self.orders_for.set(Some(user));
        self.status.set("ready".to_string());
        self.recompute();
        Ok(())
    }

    async fn load_rate(self: Rc<Self>, next: &'static str) -> Result<()> {
        let rate = fetch_rate(next).await?;

        self.rate.set(rate);
        self.rate_for.set(next);
        self.recompute();
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
            '1' => store.clone().select_user(1),
            '2' => store.clone().select_user(2),
            '3' => store.clone().select_user(3),
            'c' => store.clone().toggle_currency(),
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
                                let orders_for = store.orders_for.get();
                                let flag = if selected != orders_for { "  (loading)" } else { "" };
                                Some(ui!(Text(text: format!("User: {selected:?} | orders for: {orders_for:?}{flag}"))).into())
                            }
                        ),
                        SignalBuilder(
                            builder: |context: &Context| {
                                let store = context.read::<Store>();
                                let currency = store.currency.get();
                                let rate = store.rate.get();
                                let rate_for = store.rate_for.get();
                                let flag = if currency != rate_for { "  (loading)" } else { "" };
                                Some(ui!(Text(text: format!("Currency: {currency} | rate {rate} for {rate_for}{flag}"))).into())
                            }
                        ),
                        SignalBuilder(
                            builder: |context: &Context| {
                                let total = context.read::<Store>().total.get();
                                Some(ui!(Text(text: format!("Total: {total:.2}"))).into())
                            }
                        ),
                        SignalBuilder(
                            builder: |context: &Context| {
                                let status = context.read::<Store>().status.get();
                                Some(ui!(Text(text: format!("Status: {status}"))).into())
                            }
                        ),
                        SignalBuilder(
                            builder: |context: &Context| {
                                let orders = context.read::<Store>().orders.get();
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
