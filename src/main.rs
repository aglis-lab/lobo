use std::time::Duration;

use widgets::*;

#[derive(Widget)]
struct Root {
    child: Widget,
}

impl Element for Root {
    fn build(&self, _context: &Context) -> Option<Widget> {
        println!("Root Being called");

        return Some(self.child.to_owned());
    }
}

struct CounterState {
    count: Signal<i32>,
}

#[derive(Widget)]
struct Hello {}

impl Hello {
    async fn test_api() -> i32 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        rand::random_range(1..3)
    }
}

impl Element for Hello {
    fn on_key(&self, context: &Context, key: char) {
        let sign = match key {
            '+' => 1,
            '-' => -1,
            '!' => {
                // Arrived via `Dispatcher::run` from a worker thread.
                println!("callback handled on {:?}", std::thread::current().id());
                return;
            }
            _ => return,
        };
        let counter = context.read::<CounterState>();
        let ui = context.dispatcher();

        // Returns immediately; runs on a worker thread. The signal update from
        // there wakes the UI thread, which rebuilds `Hello`.
        tokio::task::spawn(async move {
            let amount = Hello::test_api().await;
            println!("task finished on {:?}", std::thread::current().id());
            counter.count.update(|c| *c += sign * amount);
            // Back into the widget tree, on the UI thread.
            ui.run(|ui| ui.key('!'));
        });
    }

    fn build(&self, _: &Context) -> Option<Widget> {
        println!("Hello Being called on {:?}", std::thread::current().id());

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
                                let count = context.read::<CounterState>().count.get();

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
    let app = ui!(
        Root(
            child: Provider(
                create: |_| CounterState { count: Signal::new(0) },
                child: Hello()
            )
        )
    );

    run(app);
}
