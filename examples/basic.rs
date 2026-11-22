use std::rc::Rc;
use std::time::Duration;

use lobo::*;

#[derive(Default)]
struct CounterState {
    count: Signal<i32>,
}

#[derive(Default)]
struct CounterService {}

#[derive(Default)]
struct CounterController {
    counter_state: CounterState,
    counter_service: CounterService,
}

impl CounterService {
    async fn get_multiply(&self) -> i32 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        rand::random_range(1..3)
    }
}

impl CounterController {
    // `self: Rc<Self>` lets the returned future own the controller, so it can
    // be spawned with `.run_async()` from a temporary handle.
    async fn increment(self: Rc<Self>) {
        let multiply = self.counter_service.get_multiply().await;

        let count = self.counter_state.count.peek();
        self.counter_state.count.set(count + multiply);
    }

    async fn decrease(self: Rc<Self>) {
        let multiply = self.counter_service.get_multiply().await;

        let count = self.counter_state.count.peek();
        self.counter_state.count.set(count - multiply);
    }
}

#[derive(Widget)]
struct Root {
    child: Widget,
}

impl Element for Root {
    fn build(&self, _context: &Context) -> Option<Widget> {
        log::trace!("Root Being called");

        return Some(self.child.to_owned());
    }
}

#[derive(Widget)]
struct Hello {}

impl Element for Hello {
    fn on_key(&self, context: &Context, key: char) {
        let sign = match key {
            '+' => 1,
            '-' => -1,
            _ => return,
        };

        if sign > 0 {
            context.read::<CounterController>().increment().run_async();
        } else {
            context.read::<CounterController>().decrease().run_async();
        }
    }

    fn build(&self, _: &Context) -> Option<Widget> {
        log::trace!("Hello Being called on {:?}", std::thread::current().id());

        Some(
            ui!(Container(
                padding: Padding(left: 8.0, top: 4.0),
                child: Column(
                    spacing: 2.0,
                    children: [
                        Text(text: "Title"),
                        Container(child: Text(text: "Body")),
                        SignalBuilder(
                            builder: |context: &Context| {
                                // Reading the signal here subscribes this node to it.
                                let count = context.read::<CounterController>().counter_state.count.get();

                                Some(ui!(Text(text: format!("Count: {count}"))).into())
                            }
                        ),
                        Text(text: "Footer"),
                    ],
                )
            ))
            .into(),
        )
    }
}

fn main() {
    // Loads .env if present; variables already set in the shell take precedence.
    dotenvy::dotenv().ok();

    // Set RUST_LOG=trace (in .env or the shell) to see the widget trace output.
    env_logger::init();

    let app = ui!(
        Root(
            child: Provider(
                create: |_| CounterController::default(),
                child: Hello()
            )
        )
    );

    run(app);
}
