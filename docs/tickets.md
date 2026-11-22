# Tickets: latest-wins for async results

A **ticket** lets a task check whether its result is still wanted when it finishes.
It's implemented as `Latest` and `Ticket` in `src/task.rs`.

## The problem

The user presses `1`, then `2` quickly.

```
t=0.0  press 1  -> request for user 1 starts (slow, 1.4s)
t=0.1  press 2  -> request for user 2 starts (fast, 0.3s)
t=0.4  user 2 response arrives  -> shows user 2     correct
t=1.4  user 1 response arrives  -> shows user 1     WRONG, user 2 is selected
```

Nothing is wrong with either request. The problem is that the older request finishes last
and overwrites the newer result. A ticket stops that.

## How it works

Each kind of request has a counter, the `Latest`. Starting a request takes a new number
from it, the `Ticket`. Only the most recent ticket is "current".

```rust
let ticket = self.user_latest.start();   // ticket 1, then ticket 2 for the next press

let orders = fetch_orders(user).await;   // wait for the network

if !ticket.is_current() {                // a newer request started since then?
    return Ok(());                       // yes: throw this result away
}
self.edit(|s| s.orders = orders);        // no: this is the newest, apply it
```

`is_current()` compares the ticket's number with the counter. If they differ, a newer
request was started and this result is stale.

Timeline with tickets:

```
t=0.0  press 1  -> ticket 1 (current = 1), request starts
t=0.1  press 2  -> ticket 2 (current = 2), request starts
t=0.4  user 2 response: ticket 2 is current -> applied
t=1.4  user 1 response: ticket 1 is not current (2 != 1) -> dropped
```

## Why not wait for the earlier request first?

Waiting is a different strategy. It runs requests one after another in a queue. It gives
the same final answer, but it has costs:

- **Slower.** The user waits for requests they no longer care about. In the example, user 2
  can't appear until user 1's slow request finishes, even though nobody wants user 1 any more.
- **The queue grows.** Each key press adds a request. Ten quick presses means ten requests
  in line, all but the last wasted.
- **Errors pile up.** A failing old request can block the queue, or show an error for a
  selection that's already gone.

Latest-wins is cheaper and more responsive because only the newest request matters.

## "But they depend on each other"

They depend on the *same state*, not on each other's *data*. Two different kinds of
dependency need two different strategies:

| Situation | Example | Strategy |
|---|---|---|
| B needs the **result** of A | fetch the user, then fetch that user's orders | **Wait**: `let user = fetch_user().await; let orders = fetch_orders(user.id).await;` |
| Requests of the same kind race, and only the newest intent matters | press `1`, then `2`: only user 2 should show | **Ticket**: drop the stale result |
| Every request must apply, in order | saving edits one after another | **Queue**: run them sequentially |

In `dependent.rs`, the orders for user 1 don't feed into the orders for user 2. Both are
independent fetches, and only the latest selection matters. So waiting is the wrong tool
and a ticket is the right one.

If B does need A's result, await A first inside the same function. A ticket can still guard
the final write:

```rust
async fn load_user_orders(self, user_id: u32) -> Result<(), AppError> {
    let ticket = self.user_latest.start();
    let user = fetch_user(user_id).await?;      // wait for A
    let orders = fetch_orders(user.id).await?;  // B depends on A's result
    if !ticket.is_current() { return Ok(()); }
    self.edit(|s| s.orders = orders);
    Ok(())
}
```

## Using tickets

1. Create one `Latest` per kind of request: `user_latest`, `rate_latest`, and so on.
   Each one should cover requests that replace each other.
2. Call `start()` at the beginning of the method, before the first `.await`.
3. After **every** `.await` that leads to a write, check `is_current()`.
4. If the result is stale, return without writing. Stale failures can usually be ignored too.

## Limits

- **Manual.** Each method needs the check. A missing check brings the bug back, silently.
- **No cancellation.** The old request still runs to the end, so the network work is wasted.
  Only its result is ignored.
- **One counter per kind.** Requests of different kinds don't affect each other, which is usually
  what you want.

## Flutter equivalents

The same idea exists in Flutter's state libraries, with less code per method:

- **Bloc** `restartable()`: a new event cancels the previous one. Same effect as latest-wins.
- **Bloc** `droppable()`: ignore new events while one is running.
- **Bloc** `sequential()`: queue events and run them in order. This is the "wait" strategy.
- **Riverpod** cancels the previous build's work when inputs change.

Lobo doesn't have these yet. Tickets are the manual version of `restartable()`.
