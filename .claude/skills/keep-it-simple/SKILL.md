---
name: keep-it-simple
description: >-
  Makes every change in the lobo repository the simplest one that works and can
  still be tested. Use before designing or writing any feature, fix or refactor
  in this repo (lobo, lobo-macros, renderer, renderer-cpu, renderer-gpu), and
  when reviewing a plan, doc or diff for over-engineering: extra layers, traits,
  generics, copied types, options nobody asked for.
metadata:
  author: project-maintainers
  category: Engineering
  version: "2.1.0"
---

# Keep it simple

Lobo is a UI library. It has to be lean enough to read in an afternoon and powerful enough
to build real apps. One person reviews all of this code: every line you add is a line they
read. Write the smallest change that does what was asked, with a test that proves it.

## Crates and what each may know

| Crate | Is | Knows |
| --- | --- | --- |
| `lobo` | declarative UI: widgets, tree, signals, layout, focus | `renderer` only |
| `lobo-macros` | `ui!`, `Widget`, `Model` derives | nothing else |
| `renderer` | the `Renderer` trait, geometry, paint types, display lists, damage, scene | no `lobo`, no backend |
| `renderer-cpu`, `renderer-gpu` | one backend each (`vello_cpu`, `wgpu`) | `renderer` only |

`lobo` must stay declarative: no damage tracking, batching, pixels or backend names in it.
If a change needs `lobo` to know a backend, the change is in the wrong crate.

## Before you write code

Answer these in your head (or in one short line to the user when the task is big):

1. **What is the one thing asked?** Build that. Not the "while I'm here" extras, not the
   version that also handles cases nobody named.
2. **Where does it already almost exist?** Extend the nearest function, file or type. A new
   file, crate or module needs a reason you can say in one sentence. A file at its size limit
   (`lobo-project`, Style and Testing) is one such reason.
3. **How will it be tested?** Pick the cheapest real test: a pure function test, then a test
   through a small spy `Renderer`, and only then a new seam.
4. **What is the smallest diff?** If the plan touches more than ~5 files or adds a new
   layer, say so and propose the smaller version first.

## Challenge the user

The goal is the developer who builds an app with lobo: a small API that reads well and does
the obvious thing. When you see a better way to reach that, say so **before** building, even
if the user asked for something specific:

- A simpler design that gives the same result.
- An API that makes the app author do work the library could do (extra calls, flags,
  lifecycle to remember).
- A request that serves the library's internals more than its users.
- A plan that conflicts with these rules or an earlier decision.

Keep it short: the concern, the better option, the trade-off, your recommendation. One
challenge per real issue, not a list of nitpicks. If the user still wants their version after
hearing it, build their version.

## How to write it

- **Plain functions inside a module.** A function with inputs and an output is the easiest
  thing to test. Reach for a struct when there is state.
- **Crates talk through traits.** `lobo` depends on the `Renderer` trait, never on a backend
  type. The trait lives with the code that **uses** it (or in `renderer`, where both sides
  already depend), names only what the caller needs, and has one real implementation per
  backend plus a spy in tests. Plain data (`Rect`, `Color`, `DrawCmd`) crosses the boundary;
  no callbacks into the caller's internals.
- **No traits inside a crate.** Code in the same crate calls each other directly. A trait with
  one implementation and no fake, or one that is not at a crate boundary, is a layer to
  delete. (`Element`, the widget trait, is the exception: many implementations.)
- **Pass the dependency in, once.** Take `&mut dyn Renderer` as an argument. Prefer `dyn` over
  threading `<R: Renderer>` through many types. No global registry or backend singleton.
- **No layer that only forwards.** Call the inner function directly.
- **One type per piece of data**, declared where it is made. No `*Dto` copy plus a converter;
  widget structs are their own props.
- **No generics, builders, registries, plugin systems or config flags** until there are two
  real callers that need them. Duplicating 5 lines twice is fine; the third copy is the time
  to extract.
- **No "for later" code.** No unused parameters, variants, options or hooks. A design doc may
  name a later step; the code does not carry it. Git keeps history.
- **State lives in one struct per area**, not in scattered statics. Thread-locals only where
  the UI thread is the contract already (signals, focus), not for new state.
- **Fewer public items is better.** `pub(crate)` by default; make a thing `pub` when an app
  or another crate needs it. A small public API is what keeps the library readable.
- **Errors:** return them (`anyhow::Result` in apps, a small enum only when a caller matches
  on it). Don't build an error hierarchy.
- **Follow the file you are in.** Match its names and patterns; read the surrounding widget or
  module before adding a new pattern.
- **Split at a seam, when a limit forces it.** A file or test module at its line limit
  (`lobo-project`, Style and Testing) moves one group of related items out. Split by job, not
  by line count, and don't add a layer just to make a file shorter.

## Testable without complexity

| Need | Do | Don't |
| --- | --- | --- |
| Test logic | Pull it into a pure fn, test inputs → output | Mock everything around it |
| Fake a backend | A small spy `Renderer` that records calls | A mock crate or a real window |
| Check layout / focus | Mount a tree, run layout with a null `Renderer`, assert rects | Test through pixels |
| Check redraw | Spy `Renderer`; assert the damage rect and the commands sent | Assert internal cache fields |
| Fake time / ids | Pass them as arguments | A global clock service |

A test checks behaviour a user of the library cares about (what is drawn, where, how often),
not that a function called another function.

## Red flags in a plan, doc or diff

Stop and simplify when you see:

- `lobo` naming a backend, or doing damage / batching work that belongs in `renderer`.
- A trait inside a crate, or one with one implementation and no fake.
- A boundary trait with many methods when the caller uses a few.
- A function whose body is one call with the same arguments.
- The same data as two structs plus a `From`/mapper.
- `<T: Something>` threaded through many types for a single concrete `T`.
- A `Manager`, `Registry`, `Factory`, `Handler` that holds one thing.
- Several hooks, phases or observers where one would do.
- Options, flags or settings nobody asked for.
- A new dependency for something the standard library or an existing dep does.
- Tests longer than the code, mostly setting up mocks.

When reviewing, name the flag, the file and line, and the smaller version.

## Reference code

Local checkouts of `wgpu` and `vello` (siblings of this repo, e.g. `../wgpu`, `../vello`), if
present, are read-only references.
Read them for **behaviour** (what a clip, a glyph run or a render pass does, and in what
order), not for structure. Backends depend on the released crates; don't copy their code in.

## Finish

- Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`.
- Check the line count of each file you touched against the limits in `lobo-project` (Style,
  Testing) before reporting.
- Report: what changed, where, how it was tested, and anything you chose **not** to build
  (one line each) so the user can ask for it.
