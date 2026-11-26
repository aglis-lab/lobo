---
name: review
description: >-
  How to review a change in the lobo repository: code diffs, design docs in
  docs/, manifests and skills. Use when asked to review, check or audit a diff,
  branch, PR, doc or plan in this repo. Gives the order to read things in, the
  checks specific to lobo (crate boundaries, signals, tree, layout, focus,
  renderer proposal), how to verify claims against the code, and the report
  format.
metadata:
  author: project-maintainers
  category: Engineering
  version: "1.1.0"
---

# Reviewing lobo

Load `lobo-project` (map, status, commands) and `keep-it-simple` (the red-flag list) first.
This skill says how to apply them to a review. The reviewer is one person; a finding they
can't act on in a minute is noise.

## 1. Scope the change

```
git status --short
git diff HEAD --stat           # staged + unstaged
git diff HEAD -- <path>        # read every changed file, not just the stat
git log --oneline -5           # what came just before
```

For a branch or PR, diff against `main`. Read new files whole. For a changed file, also read
its neighbours (the module it lives in, the callers of what changed).

## 2. Verify every claim against the code

Docs and skills here describe code that moves fast. For each concrete name a doc or comment
mentions (a function, type, method, field, file), `grep -rn` for it in `src/`, `lobo-macros/`
and `renderer*/`:

- Says it **exists** today → it must be there, with that signature.
- Says it is **proposed / later** → it must not be described as built anywhere else
  (`README.md`, other `docs/*.md`, skills).
- A **file map** (in a skill or doc) → `ls src src/*/` and check nothing is missing or stale.

Wrong facts in docs and skills are high-severity: future AI sessions act on them.

## 3. Checks by area

**Crate boundaries** (most important)
- `lobo` names no backend type and does no damage, batching or pixel work.
- `renderer` does not depend on `lobo`; backends depend only on `renderer`.
- Anything moving between crates was asked for.

**Signals and the tree** (`src/signal.rs`, `src/tree.rs`)
- User state is a `Signal`; system state (hover, caret, focus ring) is not.
- Signals stay thread-local and single-threaded; background work uses the task API.
- Reconciliation: keys first, then position + type. A change here needs a test in
  `tests/reconcile.rs`.
- Unmount still drops the node and runs `dispose` immediately (no `Rc` cycles, no strong refs
  held by something long-lived).

**Layout, paint, focus** (`src/layout.rs`)
- Constraints down, size up, parent places children. `layout` does not paint.
- Focus ids are registered in `paint`, in paint order. Anything that skips `paint` for a node
  must keep Tab order intact.
- Overlays (`paint_child_on_top`) still draw after the tree.

**Widgets** (`src/basic.rs`, `src/widget.rs`, `src/material/`)
- Follows the neighbouring widget's pattern (`#[derive(Widget)]` + `impl Element`).
- Per-node mutable state goes through `cx.state::<T>()`, not statics.
- `ui!` lines stay under 100 columns (rustfmt skips them silently otherwise).

**Design docs** (`docs/`)
- Decision first, status stated (`proposal` / `done`), short.
- Every step is ownable by one crate; check who owns each piece of state the doc introduces
  and how it reaches the code that needs it (e.g. how does a `Drop` in `lobo` reach an object
  owned by the run loop?).
- Lifecycle edges: first frame, resize, unmount, hide-then-show, re-parent.
- Each listed test is runnable with what exists (spy `Renderer`, no window).
- Other docs that mention the same thing are updated in the same change.

**File size** (every changed `.rs` file; limits in `lobo-project`, Style and Testing)
- A source file at 1,000 lines or more → finding; name the seam to split along.
- An inline `#[cfg(test)] mod tests` at 500 lines or more → finding; move it to
  `<module>/tests.rs`.
- A test file at 1,000 lines or more → finding; split by behaviour.
- Count with `wc -l`. Report these as `simplify`.

**Manifests, licence, skills**
- Every crate in the workspace has the same `license`; it matches `LICENSE`.
- New dependencies are justified (keep-it-simple: std or an existing dep first).
- Skills: frontmatter `name` / `description` say when to use them; no personal data (emails,
  home-directory paths) in committed files; facts match the code (step 2).

## 4. Run the checks

```
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

Docs-only change: `cargo check --workspace` is enough if manifests changed, nothing if not.
Report the real result; never say "passes" without running it.

## 5. Report

Lead with a one-line verdict (ship / fix first / rethink). Then findings, most severe first:

```
[severity] path:line — what is wrong
  why it matters (one line) → the smaller or correct version (one line)
```

Severity: **bug** (wrong behaviour or a wrong fact a reader will act on), **gap** (a case the
design doesn't cover), **simplify** (a keep-it-simple red flag), **nit** (only if asked).
Skip praise and restating the diff. Say what you checked and what you did not.
