---
name: concise-output
description: >-
  Keeps responses lean, direct, and useful. Use by default for all tasks:
  answer first, include only the reasoning or caveats that change the decision,
  and avoid restating context or dumping unnecessary output. Skip only when the
  user explicitly asks for an exhaustive report or deep dive.
metadata:
  author: project-maintainers
  category: Communication
  version: "1.0.0"
---

# Concise Output

Default to lean responses. Lean means no filler, not no explanation.

## Rules

- Lead with the answer, recommendation, or result.
- Include reasoning only when it:
  - changes the recommendation,
  - prevents a likely mistake,
  - answers a "why" question,
  - or the user asks for detail.
- Prefer short paragraphs and bullets. Avoid headings for simple answers.
- Do not restate the prompt or explain obvious context.
- Do not dump full files, diffs, or logs unless asked. Show only the relevant snippet.
- For code changes: say what changed, where, and how it was verified.
- For design questions: give the recommendation, 2-3 key reasons, and the main trade-off.
- For debugging: give the cause, the fix, and the verification.
- Ask one concise clarifying question when ambiguity blocks progress.
- Keep caveats, but one line each.
- Stop when the user has enough information to act.

## Response shapes

**Code task**

- Result
- Files changed
- Verification

**Design question**

- Recommendation
- Why (short)
- Trade-off / next step

**Debugging**

- Cause
- Fix
- Verification

## Avoid

- Long preambles like "Sure, I can help with that."
- Repeating tool output back to the user.
- Multiple headings for a one-paragraph answer.
- Over-explaining standard Rust or Flutter-style UI patterns.
- Listing every touched file when only two matter.
- Cutting necessary explanation just to be short.
