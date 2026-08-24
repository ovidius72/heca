# Hint — one keystroke to any element

*Decided 2026-08-14, refined 2026-08-17. This file is the record — kept in the repo rather than only
in a phase handoff, so it survives independently of the planner's lifecycle.*

Hint is the keyboard-first primitive: **give an element a one-keystroke address so the keyboard can
reach it without arrow-walking.** Vimium's link hints, easymotion. It binds no app concept — a
`Button` declaring a hint says nothing about workspaces, panes or terminals — so by AGENTS.md § 0b it
lives in `heca-grid-ui`, not in `chrome/`.

---

## 1. The defect this fixes

A target's **identity** and its **pick** were two declarations landing on **different elements**, and
which one sat on top depended on how the tree happened to be built:

- a **row** carries `nav_key`, and the `KeyHint` wrapping it carries the hint → the pick is on the
  **parent**;
- a **mounted dock** names itself on the outer `FocusScope` and declares the pick within → the pick
  is on a **child**.

So `offer_hint_by_key` had to search **both directions**. Search one only and the target silently
never gets a letter — which is why sidebar lettering kept breaking in ways nothing caught.

Antonio, 2026-08-14: *"How would a plugin author or another developer know and fix this? That should
be transparent to clients… I always asked a DOM-like API."*

---

## 2. ⭐ Anything actionable is hintable. Opt out, never in.

**Decided by Antonio, 2026-08-17.** *"every actionable widget gets automatically peekable"*, with a
property to opt out.

If a widget can be acted on, it gets a letter — no builder, no wrapper, nothing to remember. Picking
it does what acting on it does. `on_hint` is the **override**, for the case where a pick should do
something *different* from a click, and `.hintable(false)` is the way out for a widget that should
never wear a letter.

| what you write | what happens |
|---|---|
| nothing | actionable → gets a letter; picking it does what clicking it does |
| `.on_hint(…)` | gets a letter; picking it does **this** instead (heca's sidebar row: a click leaves the sidebar, a pick stays) |
| `.hintable(false)` | never gets a letter, however actionable it is |

`.hintable(true)` is the default and does nothing on a widget that cannot be acted on — there would
be nothing for the letter to run.

### This was already the design, and it was deleted

`BACKLOG.md:1490` quotes what `docs/chrome-and-ui.md` used to say:

> *"KeyHint is the host's universal leader/vimium overlay… any widget that exposes an `on_press`
> intent is automatically hintable; the leader assigns letters to every clickable target (app +
> plugin) and emits the intent on keypress; **opt out with `.hintable(false)`**. One system covers
> app and plugin alike."*

The native side only ever implemented explicit opt-in. A later session noticed the mismatch and
**rewrote the documentation to match the code**, so `docs/chrome-and-ui.md` came to say:

> *"`.hintable(false)` was written here as the opt-out and never existed. There is nothing to opt out
> of: a node with neither `press` nor `hint` is not a pick target."*

**Corrected 2026-08-17** — that file now documents the opt-out properly, with a note recording what
it used to claim. Keep the episode in mind rather than the sentence: it is a decision overwritten by
an implementation gap, and it is how this was lost the first time. **When the code and a decision
disagree, the code is what changes.**

### The declarative half already does it

`heca-view-realize/src/lib.rs:638`:

```rust
node.intent("hint").or_else(|| node.intent("press")).cloned()
```

So a plugin's two buttons already get letters from their `press` alone. Only native Rust widgets
still require an explicit declaration — that is the drift this closes.

### Delegation is still not how you declare

Rejected separately and still rejected: a container handler reading `ev.target` is how you *listen*
for picks in a region, never how a widget becomes pickable. Being actionable is what makes a widget
pickable; nothing a parent says changes that for its children.

---

## 2a. ⭐ One letter. Always. 52 targets, and that is the limit.

**Decided by Antonio, 2026-08-17:** *"typing 2 letters is not an option. always 1. stay with 52."*

A pick is **one keystroke**. Not a sequence you type and narrow, not a first letter followed by a
second. `a`–`z` then `A`–`Z` is 52, and past 52 a target simply gets no letter.

Do not propose two-key sequences again. It has been raised and refused; vimium doing it is not an
argument. Anything that needs more than 52 at once is a **scope** problem — the picker is over too
much — and the answer is a smaller picker (§2c), never a longer keystroke.

The 52 cap is already how the link picker behaves (`heca/src/app/terminal_host.rs:783`,
`return hints; // 52-label cap reached`). Keep it, and keep it silent-free: a target with no letter
is not an error, but it must not look like a broken picker either.

---

## 2b. ⭐ What "actionable" means

**Decided by Antonio, 2026-08-17:** *"if we have on_click, on_key_up, on_double_click also maybe it
needs to be hintable."*

So: a widget is actionable when it listens for a deliberate act on itself — a click, a double click,
a key. If it does, it gets a letter.

### How it is asked: read the handler list at pick time

**Antonio, 2026-08-17:** *"we can see when prefix+/ is pressed if there is an actionable event like
on_click, on_key_up etc… and if it has hintable(false). Can we?"* Yes.

The plan was to read `Handlers::has(kind)` at pick time and add no field. **It was tried and it does
not work** — two red tests said so before anything was built on it:

- **Eight widgets keep their action in a private field of their own** — `Button`, `IconButton`,
  `BadgeButton`, `Toast`, `Choice`, `RailCell`, `Item`, `Row`. `Handlers::has(Click)` is `false` for a
  `Button`, the case that matters most.
- **`on_click` and `on_activate` are two spellings of one thing**, so no set of `EventKind`s names it
  either.

So it is a field after all: **`Base::activatable`**, set wherever the action is wired — once in
`ComponentExt::on` for the generic listeners (`Click`, `DoubleClick`, `Key`, which covers `on_click`,
`on_double_click`, `on_key_down` and `on_key_up`), and one line in each of the eight. That line sits
beside their existing `base.focusable` and `base.one_click_target`, which are the same pattern.

```
gets a letter = activatable && !hintable(false)
```

**Right-click and middle-click are deliberately out** — a right-click opens a context menu rather
than doing the thing, so it should not spend one of the 52.

**Known consequence, accepted:** `on_key_down` and `on_key_up` both register as `EventKind::Key` and
sort themselves out *inside* the closure (`builders.rs:601`), so a widget listening only for Escape or
arrows is still marked activatable. Those get a letter too. Watch it on screen; `.hintable(false)` is
the fix where it is wrong, not a narrower rule.

**And the cost is nothing.** The walk runs **once, when the picker opens** — not per frame. Afterwards
each widget just reads its own signal to draw its letter. Do **not** cache the result: trees are
rebuilt between frames, and a cached path pointing at a widget that has moved is precisely the bug
this design avoids (`fire_hint`: a rebuilt tree "simply offers a fresh set").

---

## 2c. ⭐ Which targets a picker covers

**Decided by Antonio, 2026-08-17:** *"prefix+/ should work on all surfaces but show hint only on the
active one"*, and — on why the sidebars do not disappear from it — *"top/bottom/left-right sidebar
all live in the main surface. the exposé is another overlay."*

So there are two levels, and they already exist in the code:

**The global picker, `prefix+/`.** Letters the surface in front of you:

- the **top-most overlay** when one is up — the exposé, a modal, a plugin's panel;
- otherwise **the main surface**, which is the sidebars *and* the top/bottom bars *and* the panes,
  together, as one picture. Focusing a sidebar does not make it a separate surface — that changes
  who gets keys, not what you are looking at.

This is what `active_hint_targets` (`heca/src/chrome/hint.rs:490`) already does: it walks layers
front-to-back, stops at the first modal one, and otherwise falls through to chrome + panes together.
Its occluder rectangles stay too — inside the main surface, a pane behind the left sidebar should not
wear a letter that draws underneath it.

⚠️ **Note the distinction, it has been got wrong once.** `FocusedSurface`
(`heca/src/app/input.rs:60`) is `Layer | Dock | Panes` and answers *who holds the keyboard*. That is
**not** this question. A focused sidebar is its own surface for keys and part of the main surface for
letters.

**A surface's own picker.** A component declares an action that opens a `KeyHintGroup` over its own
subtree; config binds the key per surface. The exposé already runs this end to end: it declares
`heca.expose.pick`, `[[keys.surface]]` binds `pick = "s"`, and `s` letters its cards only. That is
the answer whenever a picker would otherwise cover too much.

### And a letter goes only where it can be SEEN

There are two halves to that, and they live on opposite sides of the library/app line:

| what hides a target | who asks | where |
|---|---|---|
| another surface in front, or the window's edge | the **app** | `resolve_hint_layers` — the coarse/fine rule above |
| a **clipping ancestor** — a row scrolled past a sidebar's fold | the **library** | `hint::collect`, through `Component::clips_children` |

The app cannot answer the second one: a row hidden by the sidebar's own fold is still inside the
window, and its bounds are perfectly real — it simply is not drawn. The tree is where that answer
is, and the framework already had a name for it. **Paint honours `clips_children`. Input honours it
(`pointer::hit_test`). The picker was the one walk that never asked** — so `prefix+q` lettered rows
past the fold, and their keycaps painted over the top bar and the status bar, because a cap goes
into the overlay band and an overlay segment starts unclipped on purpose so a dropdown can escape a
scroll region (Antonio, driving, 2026-08-23; F003/P082/T438).

**Any overlap at all counts as visible**, so a row you can half see keeps its letter — *"a half
visible pane row should have the letter to peek"* — and its keycap is deliberately **not** clipped
to match: it is drawn whole so it stays readable. What stops a cap for a row nobody can see is
candidacy, not clipping.

**It filters the view, not the candidate.** A pane is shown in several places at once — the tiled
area, the left sidebar, the right one — and the letter belongs to the *pane*. A row past the fold is
simply not one of the places that can show it; the pane keeps its letter and its other views still
wear it. That is the same rule the app already follows for a pane covered by a sidebar
(`chrome/hint/letters.rs`), which is why `collect_hints` drops the candidate (it must not spend one
of the 52) while `offer_hint_by_key` only declines to *write* (a withdrawal is never refused, or the
keycap outlives the picker).

---

## 2d. ⭐ Identity — see `docs/widgets.md` § Identity

A letter cannot stay with its target between openings unless the target can be recognised next time,
so the picker needs an identity for every widget. That grew past hints — the keyboard cursor, the
right-click and drag all read the same thing — and lives in its own record:
**[`docs/widgets.md`](widgets.md) § Identity** — the widget catalog, where the reference for it
belongs. (It was briefly its own file; folded back in, because separate files fragment the docs.)

The short of it, decided 2026-08-17:

- **You write nothing** on an ordinary widget. A button, an icon, a card, a label: nothing.
- **`.key(…)`** on items in a collection you are iterating — the item's own id from your data, never
  a counter. It is React's `key`, and means the same thing.
- **Regions are derived**, as the nearest keyed ancestor. `scope_key` disappears; nesting is
  structure, and the sidebar's tree carries a key at every level because each level is both a row and
  a container.
- Anything unkeyed still gets a **derived** identity, from its accessible name within the nearest
  keyed ancestor — from content, never from position, because position is exactly what drifts.

`nav_key` and `scope_key` are gone. They named *how the framework used the string* rather than what
it was, which is why nobody remembered they existed.

---

## 2e. ⭐ A letter stays with its target

**Antonio, driving, 2026-08-17:** *"i want to expand a pane, prefix+/ and `k` appears on that icon. I
press k and it expands correctly. Then I want to collapse. prefix+/ and `j` appears on that button,
while I was expecting `k`."*

**The letter is currently the index.** `handle_hint_pick` enumerates targets in document order and
calls `candidate_letter(i)`, over a flat `a…z` then `A…Z`. Add or remove any target earlier in the
tree and everything after it shifts.

**The fix:** remember the assignment across openings, keyed by the target's identity (§2d). Two
passes — targets that had a letter and are still there keep it, then the rest fill the gaps.

**Home row first.** The order is `asdfghjkl`, then the rest of `a–z`, then the capitals. Two places
hold it today: `CANDIDATE_ALPHABET` (`heca/src/app/selection.rs`) and `letters()`
(`key_hint_group.rs:51`) — which is one of the two reasons the labeller becomes caller-supplied.

**Do home row first, stability second**, or the letters get relearned twice: changing the alphabet
reshuffles everything once.

---

## 3. Candidacy is not the pick

The single most important distinction, and the one that dissolves "does it bubble?":

| | what it is | traversal | when |
|---|---|---|---|
| **candidacy** — "I am hintable" | a *property* | collection walk, **top-down** over the picker's visible subtree | when the picker opens |
| **the pick** — "letter `a` resolved to me" | an *event* | dispatch walk, **target → up** along one path | when the key is pressed |

**Candidacy never propagates**, in either direction. A container declaring a hint does not make its
children hintable, and does not make itself hintable on their behalf. There is no event yet when the
letters are handed out.

**The pick propagates** like any other event, through the existing `deliver_to_path`
(`heca-grid-ui/src/component.rs:830`): capture down → target → bubble up, with `stop_propagation`
ending the walk. No second dispatch path.

The closest DOM analogy is **not** `onclick`. It is `tabindex`: scarce, explicit, per-element, and a
`tabindex` on a parent tells you nothing about its children — while the *focus event* bubbles
normally. Hint is that pair.

---

## 4. `on_hint` fires in the target phase only

**Decided 2026-08-17.** `on_hint` runs only when *this widget is the target* — not when a pick
merely passes through it on the bubble.

The case it exists for: a Pane that is itself hintable **and** contains hintable rows. Pick a row and
without target-only the pane's handler fires too, so you land on the pane instead of the row.
Otherwise every such container writes the DOM's `e.target !== e.currentTarget` guard by hand — N
copies of a framework rule, which by our own rule means the API is missing.

This diverges from `on_click`, and the divergence is correct: clicking a child of a clickable div
**is** clicking the div, because the pointer is over both. Picking a row is **not** picking the pane
— a pick is nominal, not spatial.

**Delegation still exists**, as a separate spelling: the ordinary bubbled `hint` event, seeing
`ev.target`, able to `stop_propagation`. It declares nothing and gets no letter. Both forms ship
together — "when a plugin asks" is how work gets forgotten.

---

## 5. The three roles, so they stop blurring

| | what it is | analogy |
|---|---|---|
| `on_hint` on a widget | *this element is pickable, and here is what picking it does* | `onclick` |
| `KeyHint` | draws a cap over a **region** that is not a widget you can put a builder on | a decorator |
| `KeyHintGroup` | **the picker**: opens, letters the declarations beneath it, reads the keystroke | a dialog / a mode |

They are not alternatives. `KeyHint` and `KeyHintGroup` read alike because both take a subtree — that
is the whole source of the confusion — but one is **a target** and the other is **a picker**.

---

## 6. Library vs app — where the line falls

Hint must survive `heca-grid-ui` becoming a standalone reusable UI library.

| layer | belongs to |
|---|---|
| `Base::hint`, `Base::hint_label`, `on_hint`, cap painting | **library** — mechanism, no policy |
| `KeyHintGroup` — the picker widget | **library** — same category as Menu or Dialog |
| which letters, in what order, who is excluded | **`chrome/hint.rs`** — this app's judgement |

`chrome/hint.rs` already holds the policy correctly: `wanted`,
`the_pane_you_are_on_gets_no_letter`, and the withdrawal bookkeeping (whoever offers a letter owns it
until they withdraw it).

**One leak, found 2026-08-17:** `heca-grid-ui/src/widgets/key_hint_group.rs:51` hardcodes
`('a'..='z').chain('A'..='Z')`. That is policy inside a reusable widget — an outside user cannot
choose homerow-first ordering, a different layout, or reserve a letter for dismissal. The labeller
must be caller-supplied, with the current alphabet as the default.

The refactor **loosens** coupling rather than tightening it: what existed before was a *host paint
pass* in `chrome/` that encoded how heca builds its trees. Mechanism down, policy up.

**The name, decided 2026-08-17.** This was called `peek` until Antonio renamed it to `hint`: an
outsider reaching for a keyboard-navigation library searches for *hints*, and the library module was
already called `hint` — only the functions inside said "peek", so the rename made it agree with
itself.

**One thing deliberately NOT renamed:** `workspaces.peek_selected` (`Space` — focus the row but keep
the keyboard on the dock). That is the *other* meaning of peek — look at it without committing — and
has nothing to do with letters. A hint pick happens to fire it, but its own name is about previewing,
so it stays `peek_selected`.

---

## 7. The steps, in order

Each deletes something. The order is load-bearing: nothing can carry a hint until the framework can
draw its cap without a wrapper.

**1. The framework paints the cap.** Drawing moves out of `KeyHint::paint` into
`component::paint_child`; `Base::hint_label` already holds the letter, so the drawing is generic.
⚠️ Check `Pane` and `DockFrame` — they have bespoke paint loops — still route through it.
*Deletes:* `KeyHint` as a **required** type (it stays as an optional decorator) and most of
`HintPlacement`, since a control places its own cap. **Antonio drives this one**: it is rendering,
and green tests do not verify it.




**2. Anything actionable is hintable, and `.hintable(false)` opts out** (§2). `collect_hints`
(`heca-grid-ui/src/hint.rs:39`) stops requiring an explicit `Base::hint` and takes anything
actionable; `fire_hint` runs the explicit hint when there is one and the widget's own action
otherwise — the same `hint.or_else(press)` rule the declarative side has had all along. **Correct
`docs/chrome-and-ui.md:394` in this step**, or the contradiction outlives the fix.

**3. `on_hint` onto `ComponentExt`, and `hint` becomes a real event.** ✅ **DONE — F003/P082/T432.** Now the *override*, not the
switch: it says a pick does something other than a click. Target-phase-only (§4), plus the bubbled
delegation seam, on the existing `deliver_to_path` walk. *Deletes:* the wrapper requirement; the
two-direction search in `offer_hint_by_key`; the "is the pick on the parent or the child" question.
`Base::hint` is **already** on `Base` (`component.rs:254`) — only the builder was in the wrong place,
and the doc at `component.rs:245` claiming it belongs on the wrapper is what this falsifies.

**What it actually landed as.** `Base::hint` became a `Hint { intent: Option<Intent>, run }` rather
than a bare closure, and `heca-grid-ui` took a dependency on `heca-view` to name that `Intent` —
serde-only with no dependencies of its own, so it is not a cycle and nothing moved. The reason is
the refused-pick rule below: **a host cannot ask its policy about an opaque closure.** The library
stores the value and never reads it; which candidates are excluded stays the app's judgement, per
§ 6. Both authoring paths write the same slot — `chrome::picks(mount, intent, emit)` natively,
`realize`'s `hint` event declaratively — so there is still one door.

The pick is delivered as `Event::Hint(HintEvent { key, bounds })` through `deliver_to_path`. The
widget's own declaration runs in the **target phase** (`component::run_pick`); what bubbles is the
event, which is the delegation seam — `.on(EventKind::Hint, …)`, which declares nothing and gets no
letter. `stop_propagation` there stops **outer** listeners hearing it; it does not un-run the widget
that was picked, because the target phase already happened. That is the phases working, not a gap in
them.

**A refused pick gets no letter, and the rule knows no kinds.** `chrome::active_hint_targets` asks
one question of every candidate on every surface — *what do you do?* — takes the `Intent` off the
declaration and asks `route_interaction` about it. There is no `state.panes` in it, no key
resolution and no list of kinds, so a widget nobody has written yet and a plugin's row are judged
exactly as heca's own rows are. Two supporting pieces make it honest:

- **The source comes from the emitter, never from the filter.** `ChromeIntentEmitter` carries the
  `InteractionSource` it stamps, `RetainedChrome` stores it, and `pick_source` reads it off the
  surface. Candidacy asking as `Keyboard` while the chrome tree dispatches as `MouseLeftSidebar`
  resolved to two different domains, so the picker offered letters in `Container` that execution
  refused in `Floating`.
- **A chrome button's pick is named where its tooltip is** (`action_tooltip`). A button hands over
  one closure for its click and its pick; a closure is opaque, so every chrome button escaped the
  filter — the sidebar toggles wore letters while `sidebar_left` was already being refused.

The other half of the contract is the **declaration's**: `ActionMeta.policy` is required and has no
permissive default, and an action that misdescribes itself is the one thing no mechanism can
correct. `workspaces.peek_selected` and `workspaces.activate_selected` both declared `Global` — the
one policy no domain refuses — while both end by focusing a pane; they share one named constant
(`reaches_a_pane = SourceDependent`) so the rule has one name and no copy is left to be found by
hand later.

**Still standing, deliberately:** the two-direction search in `offer_hint_by_key`. It is only
deletable once the app's declarations move off their `KeyHint` wrappers onto the widgets themselves,
and today the wrapper is also what carries the **placement** (`Base::hint_style`, whose builders are
`KeyHint`'s). Moving them without a placement builder on `ComponentExt` would silently relocate every
keycap — a visual change nobody can verify from a test. It goes with T438.

**4. The labeller becomes caller-supplied** (§6), current alphabet as default. **One letter, never
two** (§2a).

**5. The guard test, written failing, before the ViewNode work.** *Every widget that can declare a
hint natively must be declarable with a `hint` event through `ViewNode`* — same shape as
`every_widget_property_is_reachable_from_the_sdk`, which caught two mistakes in one day without
anyone remembering it existed. Written first and left red; steps 6 and 7 turn it green. Written
afterwards it would only record what was built.

**6. `hint` as a `ViewNode` event carrying an `Intent`.** The actual plugin half — a closure cannot
cross the boundary, so the declarative spelling is not a nicety, it is the feature:

```json
{ "kind": "Row",
  "props":  { "nav_key": "docker:abc" },
  "events": { "hint": { "action": "docker.restart", "args": { "id": "abc" } } } }
```

`realize` wires it to `on_hint` firing the intent — the same two-spellings-one-slot pattern `press`
and `change` already use. Both converge on `Base::hint`: one door, no second path to drift.

**7. `KeyHintGroup` gets a `WidgetKind`, and `on_action` its declarative form.** ✅ **DONE —
F003/P082/T436.** Otherwise a plugin can contribute *targets* but still cannot own a *picker* — the
gap just closed for native code and left open for everyone else.

**What it landed as.** The picker's three native parts — a signal, `open_when` binding it, and an
`on_action` closure that flips it — are one builder, `KeyHintGroup::opens_on("mypanel.pick")`. That
is what made it declarable at all: static data can carry a *name*, and could never have carried the
other two. The exposé says the same line now (`.opens_on(PICK_ACTION)`), so the described picker is
the native one rather than a copy of it, and the rule that an open picker holds the keyboard lives
inside the widget instead of at each call site.

`on_action`'s declarative form is a node's **`actions`** map (`name → Intent`), read once in
`realize` for every kind beside `hint`, `key` and `hintable` — universal, because
`ComponentExt::on_action` is universal, and a per-kind arm would be the same framework rule written
thirty-three times. An **event** is fired *at* a node by what the user did to it; an **action** is a
name said out loud by a binding, the palette or a script, and answered by whoever on screen declares
it. One refusal, at the only place that knows both names: a verb whose intent names *itself* is not
declared, because it would resolve back to the same widget and re-post itself forever.

---

## 8. What a plugin author writes, after it

Everything is either a builder on their own widget or a string in their own `ViewNode`. No registry,
no host-private type in any signature, no id to pre-register and release; the identity is the
widget's own `nav_key`, which already serializes; and they react to the picker through
`hint.changed` (`ChromeEvent::HintLettersChanged`).

---

## 9. Why a test, not a paragraph

RULE ZERO is written in three places, was read three times in one day, and a host-side paint fix was
still written. **Encode a rule as a failing test, not a prose section.** §4's guard test is that
rule applied to this feature.
