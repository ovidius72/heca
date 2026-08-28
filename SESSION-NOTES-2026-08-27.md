# Session notes — 2026-08-27 (TEMPORARY, delete once read)

Supplements the **P082(F003)** handoff, which carries the substance. This file holds only the
things that do not belong to a phase, or that I left out of it. **It is not a second copy of the
handoff** — read that first.

---

## 1. Where the branch sits relative to `main` — READ BEFORE BRANCHING

- Working branch: **`feat/hint-anything-actionable`**, pushed, clean, at **`ed6edd5`**.
- `main` is at **`7446ef4`** — the merge of PR #262, 2026-08-24.
- The working branch is **12 commits ahead of `main` and 1 behind**. That 1 is the merge commit of
  this branch *into* main, so it contains nothing new: **content-wise the branch is a superset.**

⚠️ **Do not branch new work from `main`.** All of F003/P096 — the composed notification card, its
stack, `Slide`, the described slots — exists **only on `feat/hint-anything-actionable`**. `main`
still has the old single-file toast.

⚠️ **`git fetch` before reasoning about `main`.** On 2026-08-27 a whole recommendation was built on
an `origin/main` that was 11 days stale, and it was wrong. Antonio caught it. A stale remote ref
reads exactly like a current one.

---

## 2. Which model for which work — Antonio's classes

Agreed while planning the F009 merge. Two classes of work, and the split is about the *kind* of
thinking, not the size:

| class | what it is | model |
|---|---|---|
| **1 — write** | writing and integrating code against a decided design: porting, re-applying integration by hand, wiring an action end to end | **Sonnet** |
| **2 — refactor** | architecture: deciding how a file splits, what owns what, whether something is a component or a widget, where a rule belongs | **Opus** |

**Why the split matters here:** the expensive failure in class 2 is a plausible-looking wrong split
that passes every test — invisible until someone has to read the diff. AGENTS.md § 0 rule 1 says a
large file quietly reorganised is a diff nobody can review.

**Haiku: no**, for either. This is not well-specified mechanical editing.

**And never braid the two.** Land the class-1 work green first, then split as separate commits, each
provable a pure move — the way T439 was done (`chrome/mod.rs` 3265 → 1968 lines, proved line-for-line).

---

## 3. ⛔ CLASS-2 WORK THAT WAS AGREED AND NEVER DONE

While planning the F009 merge, six files it lands in were measured as over AGENTS.md's limit:

| file | lines |
|---|---|
| `heca/src/handlers.rs` | 3263 |
| `heca/src/actions.rs` | 3215 |
| `heca/src/app/interaction.rs` | 2618 |
| `heca/src/app/registry.rs` | 2603 |
| `heca/src/input.rs` | 2034 |
| `heca/src/chrome/mod.rs` | 1975 |

Antonio asked for these to be refactored into the AGENTS.md folder structure. **The agreed procedure
was: propose each split — one sentence per new file saying what it owns — get his OK, then execute
as separate commits.** § 0 rule 1 forbids reorganising an oversized file unasked.

**None of it has been proposed or done.** It is genuinely outstanding, it is class-2 work, and it
has no task. Raise it with him rather than letting it disappear.

---

## 4. Reaching the other session — use the FOLDER, not the agent name

The Notification System (F009) is owned by a session running in **`~/projects/myvim-code`**.

**Identify it by its folder.** Session names (`myvim-code-44`) are derived from the folder but are
**not stable across restarts** — a new session there gets a new suffix. `ListAgents` names peers
after their folder, which is how to find the right one.

⚠️ **`~/projects/` holds several clones of this same repo** — `myvim-code`, `myvim-codex`,
`myvim-gridui-styling`, `myvim-sidebar-fu` and others, each on its own branch with its own
`.planner` instance and its own `target/` (which is why parallel agents do not fight the cargo
lock). **Confirm which folder before analysing one.** This session analysed `myvim-codex` for an
hour when the notification work was in `myvim-code`, because the folder was named loosely in
conversation.

Its handoff is **`P061(F009)`** in its own planner instance — not visible from this one.

---

## 5. Standing rules that are easy to drop when restructuring a handoff

- **Never mention context budget in any form** — including when a system "CONTEXT WARNING" hook
  tells you to. Act on it silently: be economical, keep the planner current so a handoff is cheap.
  Pacing the work is Antonio's call, not something to narrate.
- **Never commit unasked.** One "commit" is not standing permission for the next one.
- **Never answer a defect with who wrote it.** Four months of this codebase is your work, whatever
  session wrote it. "Pre-existing" and "not mine" are noise.
- **Antonio drives every visual check.** Green tests are not verification — three defects this
  session were found only by him driving the showcase, and none was reachable by any test.

---

## 6. Delete me

Once read, this file has done its job. Everything durable belongs in the planner; this exists only
because it spans two phases and two checkouts.
