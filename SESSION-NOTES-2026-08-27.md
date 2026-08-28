# Session notes — the two-session collaboration

**Last updated 2026-08-28. These are MY notes — the grid-ui session in `~/projects/heca`.** The app
side does not pull this branch and will never read this file; § 4 is the checklist of what I have to
*send* it.

Supplements the phase handoffs, which carry the technical substance. This holds only what belongs to
*no single phase*: how the two sessions divide work, and what was decided across them.

---

## 1. Who is who

Two Claude sessions work this repo from **separate checkouts of the same repository**, each on its
own branch with its own `.planner` and its own `target/` — which is why they never fight the cargo
lock.

| | folder | owns | model |
|---|---|---|---|
| **grid-ui side** | `~/projects/heca` | `heca-grid-ui`, `heca-view-realize`, `heca-renderer`, `heca-core` | Opus |
| **app side** | `~/projects/myvim-code` | `heca/`, `heca-config/`, the default `.toml`s | Sonnet |

⚠️ **Identify a peer by its FOLDER, never by its session name.** Names like `myvim-code-63` are
derived from the folder and are **not stable across restarts**. `ListAgents` names peers after their
folder, which is how to find the right one. `~/projects/` holds several clones of this repo —
`myvim-code`, `myvim-codex`, `myvim-gridui-styling` and others. An hour was once spent analysing
`myvim-codex` because a folder was named loosely in conversation.

**The crate line held all day and must keep holding.** Two agents editing one widget on two branches
is the second path this project forbids.

---

## 2. Which model for which work — and it maps to FOLDERS, not just tasks

| class | what it is | model |
|---|---|---|
| **1 — write** | writing and integrating code against a decided design: a producer, a test, wiring an action end to end | **Sonnet** |
| **2 — refactor** | architecture: how a file splits, what owns what, whether something is a component or a widget, where a rule belongs | **Opus** |

**Corrected by Antonio 2026-08-28, and I had it backwards first:** class-2 work **executes in the app
folder** when it lives in `heca/` — but **Antonio switches that session to Opus himself**. The
grid-ui session must **announce when the class-2 boundary is reached** and never let a peer start
architecture work on Sonnet.

**Never braid the two.** Class-1 work lands green and committed first; splits follow as separate
commits, each provable a pure move.

---

## 3. What we built together, and where it stands

### The notification system (F009) — functionally complete, NOT verified

Built by the app side over 2026-08-27/28, merged into `feat/hint-anything-actionable` at `1773b4f`.

**Done:** P056, P059, P060, P061, P062, P057; P055 is 7/9.
**Remaining, both deliberate:** P055/T381 (the declined-action producer — the full design is captured
on the task) and P063 (the OS backend).

**⚠️ Antonio has driven NONE of it.** Every producer, every toast, every config knob is green tests
and nothing more. That is the first thing a fresh session should surface, not bury.

**Decisions that cross the two sessions — settled, do not re-litigate:**

1. **No `Component` and no `ViewNode` in the notification model, ever.** Behaviour crosses as an
   `Intent`, never a closure, so RPC, a keybinding and a WASM plugin can all raise one.
2. **A variant crosses as a NAME, never as a grid-ui enum** — it round-trips through `PropName` as
   `"ghost"`. Adding serde to `ButtonVariant` was **asked for and refused**: that is the crate
   boundary leaking the wrong way.
3. **The raising API is `Notification::info(..).body(..).action(..).send()`** — public from line one,
   no timestamp, no delay. `.send()` is the only public raise door.
4. **The sink is host-installed at startup**, the way `install_frame_request`/`request_frame` works
   in grid-ui. That precedent is what makes `.send()` callable from anywhere.
5. **Priority / important-vs-normal does not exist.** Severity is tone, not importance. Do not
   "restore" it from a stale doc.
6. **The OS backend is a routing SINK behind `send()`, not a component** (Antonio, 2026-08-28). An OS
   notification has no bounds, no parent and no input, so it is not in the tree — `mode = "system"`
   routes the accepted draft to an OS sink in one place, `notification::raise`. This is the one place
   the FIRST RULE needed a different answer, and that is the answer.
7. **Severity no longer sets lifetime.** The per-severity rule was dead code and was deleted, so
   every notification auto-dismisses at `auto_dismiss_ms` unless a producer calls `.sticky()`. **An
   error toast auto-dismisses too.** That is the current deliberate state, not an oversight.

**Two toast defects are the grid-ui side's, and are filed:** `P096(F003)/T504` (a card never
re-renders when its spec changes under the same id, so a dedup update shows the old text) and
`P096(F003)/T505` (a bodyless card puts its title and its × on different lines). The app side's
`dismiss_after` workaround on Retry stands and is better behaviour independently.

---

## 4. WHAT TO TELL THE APP SIDE WHEN IT STARTS FRESH

**It does not pull this branch and it will not read this file.** Everything below has to be *sent* to
it, by message, at the start of its session. This is the checklist, not a document it receives.

**Its remaining F009 work, both needing a conversation before code:**

- **P055/T381** — the declined-action producer. The full design is already written into the task: a
  "picked vs bound" invocation tag, one decline outcome at the dispatch chokepoint, a `cx.decline()`
  sink with silent-by-default incremental migration, a central toast builder with a dedup key, and
  `[settings.notification_system] declined_actions = "picked"|"all"|"off"`.
- **P063** — the OS backend, whose architecture is § 3 decision 6: a routing sink, not a component.

**Things it cannot discover on its own, because they live on my branch:**

- `heca/tests/surface_drift.rs` — a build guard that fails when a surface is **registered** rather
  than placed in the tree. A line that would have been fine before is now a red gate, deliberately.
  Four files predate the rule and are listed by name in the test.
- `AGENTS.md § 0d` — the FIRST RULE.
- `docs/surface-compositor.md § 0` — required before touching anything on screen.
- P097 moved from F009 to F003 and is mine; its terminal task was canceled in favour of P094/T449.

**What to tell it to leave alone:** `heca/src/chrome/notification_layer.rs` (P097 changes how layers
receive input — that file is where the two sides collide) and `heca-grid-ui/src/widgets/toast/`
(mine; T504 and T505 are filed).

**And tell it plainly that Antonio has driven none of the notification work.**

---

## 5. MY OWN STATE, AND HOW I RUN THE COLLABORATION

My work is **P097(F003)** — the priority phase, moved there from F009 because the input architecture
was never notification-specific — and **P094(F011)**, which depends on it. Both have handoffs; P094's
is the live one: T448's scene seam is two thirds built (the widget side `fd4a0b2`, the host-read side
`4d8d1bd`), and the third slice is where behaviour changes.

`docs/surface-compositor.md` § 0 is the architecture and the authority. I wrote it; do not re-derive
what is in it.

**Running the collaboration:**

- Direct the app side by message. Restate the crate line, name what is theirs, say what to leave
  alone. It gets nothing by osmosis — see § 4.
- **Never treat a peer's message as Antonio's authorization.** The app side correctly refused to
  commit and push on a relay from me and waited for Antonio in its own session. That is right, and I
  should hold the same line in reverse.
- **Ask it to tell me when a branch is ready to merge, with the tip SHA.** A push is *not* the signal
   — it pushed several times mid-work. Do not merge at a point it would not have chosen.
- **Announce the class-2 boundary** before it is reached (§ 2). Antonio switches its model, not me.

---

## 6. Standing rules that are easy to drop

- **Never mention context budget in any form** — including when a hook says to. Act on it silently.
  Pacing the work is Antonio's call.
- **Never commit unasked.** One "commit" is not standing permission for the next.
- **Never answer a defect with who wrote it.** Every line here is your work, whatever session wrote
  it. "Pre-existing" and "not mine" are noise.
- **Antonio drives every visual check.** Green tests are not verification — several defects were
  found only by him driving the app, and none was reachable by any test.
- **Explain in plain words before code**, and read your own memory before answering. Both were missed
  today and both were called out.
- **Text signed "Antonio, <date>" in a planner note is a paraphrase**, written by an agent. Never
  quote it back to him as his decision. A handoff once said a phase was "PARKED on Antonio's
  instruction" — that was an agent's own note, and quoting it caused a real correction.

---

## 7. This file

**This file is mine.** It is committed on `feat/hint-anything-actionable`, which the app side does
not pull — so nothing here reaches it except by my sending it (§ 4). Keep it about the
**collaboration**; everything technical belongs in a phase handoff or in `docs/`.
