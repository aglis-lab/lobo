//! Dependent-state example: derived values depend on several async sources
//! that answer in any order, and nothing cancels or orders the responses.
//!
//! Keys: `1`/`2`/`3` select a user (loads its orders), `c` toggles the currency
//! (loads its rate), `q` quits. Keys typed on one line are all applied at once.
//!
//! Expect stale and mismatched UI: the framework has no cancellation, no
//! ordering and no derived-state primitive, so each task must reason about
//! every other task by hand.

use std::rc::Rc;
use std::time::Duration;

use lobo::*;

#[derive(Clone, Debug, PartialEq)]
struct Order {
    amount: i64,
}

async fn fetch_orders(user: u32) -> Vec<Order> {
    tokio::time::sleep(Duration::from_millis(rand::random_range(300..1500))).await;
    (0..3)
        .map(|_| Order {
            amount: rand::random_range(10..100) + user as i64,
        })
        .collect()
}

async fn fetch_rate(currency: &'static str) -> f64 {
    tokio::time::sleep(Duration::from_millis(rand::random_range(200..1200))).await;
    if currency == "EUR" { 0.9 } else { 1.0 }
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
        }
    }

    // Derived value: must be recomputed whenever `orders` OR `rate` changes.
    // It reads whatever is current at call time, which may belong to a
    // different selection than the one that started the task.
    fn recompute(&self) {
        let sum: i64 = self.orders.with(|o| o.iter().map(|o| o.amount).sum());
        self.total.set(sum as f64 * self.rate.get());
    }

    async fn select_user(self: Rc<Self>, user: u32) {
        self.selected_user.set(Some(user));
        self.status.set(format!("loading user {user}"));

        let orders = fetch_orders(user).await;

        // Nothing checks that `user` is still the selection. An earlier, slower
        // request can land after a newer one and overwrite it.
        self.orders.set(orders);
        self.orders_for.set(Some(user));
        self.status.set("ready".to_string());
        self.recompute();
    }

    async fn toggle_currency(self: Rc<Self>) {
        let next = if self.currency.get() == "USD" { "EUR" } else { "USD" };
        self.currency.set(next);

        let rate = fetch_rate(next).await;

        // Same race for rates: the response for an older toggle can arrive last,
        // leaving `rate` and `rate_for` pointing at a currency that is no longer selected.
        self.rate.set(rate);
        self.rate_for.set(next);
        self.recompute();
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
                store.clone().select_user(user).run_async();
            }
            'c' => {
                store.clone().toggle_currency().run_async();
            }
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
                                let flag = if selected != orders_for { "  <- STALE" } else { "" };
                                Some(ui!(Text(text: format!("User: {selected:?} | orders for: {orders_for:?}{flag}"))).into())
                            }
                        ),
                        SignalBuilder(
                            builder: |context: &Context| {
                                let store = context.read::<Store>();
                                let currency = store.currency.get();
                                let rate = store.rate.get();
                                let rate_for = store.rate_for.get();
                                let flag = if currency != rate_for { "  <- STALE" } else { "" };
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
