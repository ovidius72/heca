# heca chrome, plugins and UI — the single design record

**Consolidated 2026-07-26.** This file replaces four: `pluggable-chrome-plugin-plan.md`,
`grid-ui-chrome-plan.md`, `docs/plugin-authoring.md` and `docs/declarative-ui.md`. Everything below
is the **verbatim** design text from those files, reordered, with each block labelled with the
planner ids that own the work. Nothing was summarised away.

> **The planner is the only record of status.** This file is design only. Where a heading says a
> phase is done or missing, that came from the planner on 2026-07-26 — re-check it, do not trust it.

## How to find things

| You want | Go to |
|---|---|
| how a description becomes widgets, and what a plugin may set | Part I |
| why the chrome is built this way | Part II §2–§5 |
| the formal contracts (ChromeHost, providers, overlays) | Part II §6–§11 |
| the widget library's rules and boundary | Part II §13–§18 |
| which planner phase owns what | Part III |

---

# Part I — The declarative UI model

*Owned by F003/P011 (plugin-ui), F003/P015, F003/P016 and F003/P017 (plugin-ui-gaps).*
*This part is current as of 2026-07-26 and supersedes anything in Part II that contradicts it.*


## 1. What the model is

A screen is described as a tree of nodes:

```
ViewNode {
  kind:     which widget           // Column, Row, Button, Input, Scroll, …
  props:    name → value           // padding, placeholder, variant, colour, …
  events:   name → Intent          // "press" → an action id + arguments
  children: [ ViewNode, … ]        // any depth
}
```

The same tree works for the app's own screens, for anything sent in over RPC, and for a plugin.
It is plain data, so it survives being turned into bytes.

`realize` (in `heca/src/chrome/realize.rs`) turns a tree into real widgets. It is the only path
from a description to a widget, and it lives in the app.

Two ways exist to build a screen and they produce the same widgets:

```rust
// The app's own code — it can hold functions and live values.
Button::destructive("Restart").on_click(move || restart(id))

// A description — plain data, so it can be sent anywhere.
ViewNode::new(WidgetKind::Button)
    .text("Restart")
    .prop("variant", PropValue::Variant(ViewVariant::Destructive))
    .on_press(Intent::new("docker.restart").arg("id", id))
```

---

## 2. Who owns what

```
heca (the app)  ──depends on──▶  heca-grid-ui (the widget library)
```

Never the other way round. The library does not know that plugins, descriptions or `ViewNode`
exist. It only knows widgets.

| Owned by the app | Owned by the library | Owned by the plugin |
|---|---|---|
| the description model, `realize`, the theme, focus, clipping, overlay stacking, action routing | widgets, layout, painting, the property surface each widget publishes | what its screen looks like, and which actions it asks for |

A plugin never draws pixels and never hands over a widget object.

---

## 3. The rules

### R1. A widget's own builders decide what a description can set

The list of settable properties is not written anywhere. Each widget publishes it, generated from
its own builder methods:

```rust
#[props]
impl Input {
    #[prop] pub fn placeholder(mut self, s: impl Into<String>) -> Self { .. }
    #[host_only("behaviour crosses as a reference, never a function")]
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self { .. }
}
```

**Every builder must say which it is.** A builder that is neither `#[prop]` nor `#[host_only]`
fails the build. Being left out quietly is exactly how `Input::placeholder` and `ScrollRegion`'s
second axis stayed unreachable for months while both worked perfectly in the library.

A second check fails the build when a whole widget has no published list at all.

### R2. Appearance comes from the theme, and code can override it — CHANGED 2026-07-26

**Old rule, now dead:** "styling is not a property; you pass semantic intent and the host resolves
the pixels." It appeared in `AGENTS.md` and in `pluggable-chrome-plugin-plan.md` §2.6.1 rule C.

**New rule:** the theme supplies the default. Code can override it, with a string, the same way it
sets any other property.

```rust
.prop("color", PropValue::Color("#ff8800".into()))   // a literal
.prop("color", PropValue::Color("muted".into()))     // a theme name
```

Set nothing and you get the theme, which is what almost everything should do — a widget that does
not override follows a theme reload. A literal colour does not follow a theme reload; that is the
trade for control, and it is the author's choice to make.

A property whose type has fields is written as a named group of values:

```rust
.prop("border", PropValue::Map(PropMap::from([
    ("color".into(), PropValue::Color("accent".into())),   // still a theme token, at any depth
    ("width".into(), PropValue::Float(2.0)),
])))
```

A property that accepts only a fixed set of words has a **type**, so a misspelling does not compile:

```rust
.prop("orientation", ViewOrientation::Vertical.into())   // not "vertcal"
.prop("severity", ViewSeverity::Danger.into())
```

`ViewOrientation`, `ViewScrollAxes`, `ViewSeverity` (which serves both toast severity and alert
variant), `ViewLabelSide`, `ViewMarker`, `ViewTextAlign`, and `ViewGlyph` for the 52 icon names.
They **do not** get a `PropValue` variant and neither should the next one — a fixed set travels as
its name, so `Text` already carries it and the type belongs in the authoring layer. `Size`,
`Variant` and `Align` predate that rule. `ViewGlyph` is generated from `heca_grid_ui::Glyph` and
guarded by a test that fails when an icon is added on one side only (F003/P011/T019).

**This did not work until 2026-07-27 (F003/P011/T018), and the doc said it did.** `border` and
`glow` are structs; a property value could only be a scalar, so neither could be written from a
description — whatever serde derives the types carried. Adding serde to a type is not the same as
having a value that can carry it, and because nothing tested the two, nothing failed. `Map` is the
extension point: a future struct-shaped property needs no new variant.

### R3. Order never matters

Properties are applied after children are attached. That removes the only real dependency there
was — `Select` and `Tabs` clamp `selected` against the number of options, so they need the options
to exist first.

Nobody sequences anything: not an author, not a caller, not an agent. `#[prop]` **rejects any
argument**, so a hint like `late` or `after = "x"` cannot be reintroduced one widget at a time. If
two properties on a widget ever do depend on each other, that widget's setters are the bug.

### R4. Behaviour crosses as a reference, not a function

A plugin is a separate program. You cannot send it a function — there is nothing to send.

So you send a **reference**: a name the other side can look up. That is what `Intent` is.

For a plugin written in TypeScript or another language, the plugin kit hides this. The author
writes a function; the kit stores it, invents a reference, and puts the reference on the wire:

```ts
Button({ text: "Restart", onClick: () => restart(id) })
// the kit stores the function and sends a reference; the author never sees it
```

One real limit either way: the function runs when the message arrives, not while drawing.

The app's own code has no boundary to cross, so it passes real functions directly through the
builder API. Same widgets, same result.

### R5. Plugins compose; the host extends

A plugin cannot invent a new widget. It composes the ones that exist, to any depth. Adding a widget
is the host's job.

This says nothing about **how** the host keeps its list. Today it is a fixed enum plus a matching
arm in `realize` plus an entry in an `ALL` list — three edits in two crates. A registry the app
fills at startup would satisfy the same rule in one step, and is the obvious improvement. React and
Dioxus work that way: you write the component and that is the end of it.

### R6. Overlays are requested, not mounted

A modal, dropdown or context menu is not something a plugin puts in its tree. It asks the host and
waits for a typed answer. The host owns stacking, focus trapping, Escape, click-outside and
placement.

Panels, toolbars and status segments go **into** a region. Modals and dropdowns are **requested**.

### R7. Layout and appearance are separate types

```rust
pub struct Style {
    pub layout: Layout,   // arrangement + the size variant
    pub visual: Visual,   // fill, border, glow, radius, font size and scale
}
```

Two peers. Layout can be read and written as data; appearance is heading the same way under R2.
Grouping them separately keeps "what the caller asked for" apart from "what the theme decided",
which stays useful even now that both are settable.

### R8. Sizing follows CSS flexbox — unset means stretch

Set no width and a node **fills its parent across the cross axis**, exactly as CSS
`align-items: stretch` does. A panel with no width in a 600px column is 600px wide, and the row
inside it fills the panel's content box in turn.

```rust
build::Panel::new()                    // fills the parent
build::Panel::new().width_pct(1.0)     // the same thing, said out loud
build::Panel::new().width_pct(0.5)     // half — what a percentage is actually for
build::Panel::new().width(240.0)       // fixed; this is what keeps two panels side by side
```

A percentage travels as `"100%"` / `"50%"`, which is one of the three spellings `Length` accepts
(a number is px, `"auto"` is content-sized).

Authors depend on the stretch without asking for it, so it is pinned by
`a_described_node_stretches_to_its_parent_like_css`. Changing a container's default `align` would
otherwise turn every full-width described panel into a content-width one with no test failing and
nothing to see in the diff.

### R9. A widget driven by live values is host-only

`ScrollBar` reads values the app rewrites every frame. A description is static data — it cannot
carry a live value, so a described scrollbar would be a thumb that never moves. The host refuses it
rather than handing back something that looks right and does nothing.

Use `Scroll` instead: it owns its own offset and needs no wiring.

The same applies to single builders, not just whole widgets. `DockFrame::rail(..)` binds a live
value, so it is host-only too.

---

## 4. What a description cannot set today, and why

Counted from the reasons written on each builder — 168 builders, 93 settable, 75 not:

| How many | Reason | Is it a gap? |
|---|---|---|
| 22 | a function; behaviour crosses as a reference (R4) | no — and the plugin kit will hide it |
| 19 | composed content; use `children` | no — that is what `children` is for |
| 12 | the builder carries no value at all | no |
| 8 | colour | **yes — R2 changed; being fixed** |
| 4 | bound to a live value (R9) | no |
| 2 | takes two values, and a property carries one | **yes — split the builder or extend the generator** |
| 1 | an argument type the generator does not map yet | **yes — small** |

That table is not written by hand. It is the reasons in the source, counted.

---

## 5. Writing a screen

### A panel

```rust
Panel::new()
    .title("Containers")                 // a real heading; omit it and no header row is drawn
    .child(VStack::new().gap(8)
        .child(Button::new("Refresh")
            .variant(Variant::Accent)
            .on_press(intent("docker.refresh", {}))))
```

`Panel` is a **titled section container** — a plugin's slice of a region. It is quiet by default
(no frame of its own, lighter padding than a `Card`); give it a `fill` or a `border` when it should
stand apart. The heading used to be fiction: `WidgetKind::Panel` realized to a plain `Surface`,
which has no title, so this example described something that could not be built. F003/P017/T008
made `Panel` a real widget.

### A table

There is no `Table` widget. A table is built from the pieces, and this is not a workaround —
composing is the model:

```rust
Scroll::new().axes(ScrollAxes::Both).child(
    VStack::new().gap(6)
        // A plain box for the header — it is not clickable.
        .child(HStack::new().gap(12)
            .child(Label::new("Name"))
            .child(Label::new("Status"))
            .child(Label::new("CPU")))
        .child(Separator::horizontal())
        // Each body row IS clickable and selectable, so it is a `Row`.
        .child(Row::new().gap(12)
            .active(selected == id)
            .on_press(intent("plugin.docker.select", { "id": id }))
            .child(Label::new(name))
            .child(Badge::new(status))
            .child(Label::new(cpu))))
```

Two things in this example only became writable as a description recently, and both were wrong here
for a while:

- **The rule** between the header and the body is a real widget, so it takes its colour and
  thickness from the theme. It reached the vocabulary in F003/P017/T5; before that a plugin had to
  fake the line with a thin sized `Surface` that hardcoded both.
- **The clickable row.** `Row` used to name the plain horizontal box, so this example described
  behaviour — press, selection, hover — against a kind that had none of it, and no reader could
  tell. F003/P017/T6 renamed the boxes to `VStack` / `HStack` and gave `Row` to the interactive
  widget it always meant in `heca-grid-ui`. A `Row` with a press intent is focusable, activates on
  click and on Enter/Space, and is reachable by `prefix+/` like any other actionable node.

### A modal with a form

```rust
let result = ctx.overlay.open_modal(ModalSpec {
    title: "Restart a container".into(),
    body: VStack::new().gap(10)
        .child(Label::new("Pick a container:"))
        .child(containers_table(rows))
        .child(Input::new()
            .name("filter")                       // `name` submits it with the form
            .placeholder("filter…")),
    actions: vec![
        ModalAction::danger("restart", "Restart"),
        ModalAction::new("cancel", "Cancel"),
    ],
    dismissible: true,
}).await;
```

The result carries the chosen action **and** whatever the form collected. A widget with a `name`
joins that data.

### Menus and keyboard hints

A context menu is a host-owned dropdown you attach to a node; the host returns the chosen entry as
an intent.

You never build a keyboard hint. **Any node with a press intent is automatically reachable by the
leader key.** The host assigns the letters and routes the press. Opt out with `.hintable(false)`.

---

## 6. Values

- Numbers, text and true/false as themselves.
- Enums as their **name**, lower case with underscores: `"space_between"`, `"small"`, `"both"`.
  The enum's own variants are the accepted list, so adding a variant accepts it with no list to
  update anywhere.
- A size as a plain number (pixels), `"auto"`, or a percentage string like `"50%"`.
- A colour as `"#rrggbb"`, `"#rrggbbaa"`, or a theme name.

Anything the model does not understand is ignored and the widget keeps its own default. A single
bad value costs only itself — the good properties on the same node still apply. The model is
untrusted input and is treated that way.

---

## 7. Still open

- **Two-value builders** (`Overlay::panel_size(width, height)`). Either split them or let a
  property carry a pair.
- **A registry instead of the fixed widget list** (R5). Allowed by the rules; not built.
- **Where the plugin-facing types live.** A plugin written in Rust gets no editor help today,
  because the model is inside the app crate and a plugin cannot depend on the app. Tracked as
  `F003/P017/T9` and `F003/P001/T1`.
- **Generated type definitions** for plugins in other languages, from the same widget list.
  Tracked as `F003/P001/T6`.

---

## 8. Decisions, with dates

| Date | Decision |
|---|---|
| 2026-07-06 | The description model exists and covers the whole widget library, not just plugins. |
| 2026-07-06 | Behaviour crosses as an action id, never a function, so the tree stays sendable. |
| 2026-07-25 | Layout and appearance become separate types. |
| 2026-07-25 | Layout properties are read from the layout type itself — no list in the app. |
| 2026-07-26 | Widget properties are generated from each widget's builders; every builder must declare whether a description can set it. |
| 2026-07-26 | Order never matters, and the marker that would have reintroduced it is rejected at compile time. |
| 2026-07-26 | **Appearance is overridable.** The theme is the default, not a wall. Replaces the old "styling is not a property" rule in `AGENTS.md` and in the chrome plan §2.6.1 rule C. |
| 2026-07-26 | A plugin written in another language may use functions; the plugin kit turns them into references. No change to the boundary. |
| 2026-07-26 | The closed widget list is about plugins not inventing widgets. It does not require a fixed enum — a host-filled registry satisfies it. |
| 2026-07-30 | A context target **names** a row (container + `nav_key`); the component that wrote the key resolves it. The host enumerates no row kinds. (§2.11) |
| 2026-07-30 | A row's click, double-click and right-click are **named intents**, declared per item kind — never closures — so click, picker, menu, key and RPC are one path. (§2.11) |


---

# Part II — Architecture, transferred verbatim

Everything from here down is the original **prose** of the two plan files, unedited. Where it
contradicts Part I, **Part I wins** — the contradictions are listed in §21.

**One exception, 2026-07-27 (F003/P017/T008): the code examples were corrected in place.** The
verbatim rule exists so no design *reasoning* is summarised away; it was never meant to preserve
identifiers that no longer compile. A reader copies an example — leaving `Column::new()` in one
would teach a name the vocabulary does not have, and `Label::new(..).variant(Variant::Heading)`
names a builder `Label` has never had. The prose around them is untouched, and every claim that is
now wrong is still listed in §21 rather than quietly rewritten.


---

## 2. Core architectural decisions

> **Planner:** F003 — all phases

## 2. Core Architectural Decisions

## 2.1 Sidebar is only a shell, not the workspace tree

The left sidebar must no longer be treated as synonymous with the workspace tree.

Instead:

- the **Sidebar widget/shell** is only a visual/layout region shell
- it may be bordered, toggleable, and capable of informing children about its current display mode
  > **Display modes (decided 2026-07-11):** a region is **Expanded ⇄ Hidden**. The **collapsed icon
  > rail is dropped** for now; the shell still informs children of the mode, but only `Expanded` and
  > `Hidden` are used, and Expanded is resizable (width passed to the mounted Provider). The generic
  > "Provider renders an icon rail when collapsed" model — where a Provider describes its content once
  > and the host renders it per mode (write once) — is a **future** item. Full design + rationale:
  > **the planner (F003/P020) — see the planner (F003/P020)**.
- it hosts one or more mounted containers
- the workspace tree becomes a **WorkspacesContainer** mounted inside that shell
- other containers may also be present in the same sidebar, in a specific order, top-to-bottom

Examples of future containers:

- Workspaces
- AI Agents
- Docker Containers
- Git Status / Git Worktrees
- Tasks
- Project Notes
- Plugin-defined containers

This means the current `sidebar.rs` logic should ultimately be reinterpreted as:

- the first built-in **WorkspacesContainer** implementation
- not the definition of the entire sidebar system

It also means the following behaviors are **not Sidebar-shell concerns**:

- workspace/column/pane up/down navigation semantics
- workspace/column expand-collapse semantics
- pane drag/drop and swap semantics
- workspace-tree-specific highlight/visited logic

Those belong to the mounted `WorkspacesContainer`, not to the Sidebar shell.

---

## 2.2 Search belongs to containers, not to the sidebar shell

The Sidebar shell itself should not imply that a search input exists. A container may include one as part of its own content and behavior model.

The previous idea of a global sidebar search input was revised.

New rule:

- a **SidebarContainer** may expose its own search/filter input if it wants one
- the sidebar host itself should not own a mandatory search field

Why:

- not every container needs search
- different containers may need different filtering semantics
- search may be scoped differently depending on domain

Examples:

- WorkspacesContainer may filter workspaces / columns / panes
- AgentsContainer may filter by name, role, status
- DockerContainer may filter by container name, image, state
- a simple status container may not need search at all

---

## 2.3 Plugins must not mutate app state directly

A plugin must **not** receive raw mutable access to heca internals.

Instead, plugins should:

- observe events
- read state through a controlled host API / facade
- return UI contributions
- dispatch actions/intents that the app handles

This keeps state ownership explicit.

Correct model:

- **app owns canonical state**
- plugins own only **derived state** or local plugin state
- plugins ask the app to do things through **actions**

Examples:

- good: `app.actions.dispatch("pane.focus", { paneId: 42 })`
- bad: plugin directly mutates `session.workspaces[0]...`

---

## 2.4 ActionRegistry must become dynamically extensible

The current action system is not enough if plugins must add actions that can later be bound in config.

New rule:

- plugins must be able to **register actions dynamically**
- those actions must be visible to:
  - config keybindings
  - command palette
  - UI-triggered buttons/items
  - later RPC/automation if desired

This implies that the current static/closed action model will need to evolve.

The future action system must support:

- stable string action ids
- metadata for actions
- dynamic registration/unregistration
- dispatch with arguments
- separation between built-in actions and plugin-provided actions

Examples of future action ids:

- `workspace.focus_next`
- `pane.close`
- `plugin.docker.restart_selected`
- `plugin.agents.open_chat`

---

## 2.5 Plugins should be code-based, not just static JSON files

It was considered whether plugins could simply return static declarative content. That may help for trivial integrations, but it is too weak for the intended GUI interaction model.

The preferred direction is:

- **code plugins**, not raw text/template-only plugins
- specifically: **WASM plugins** as the long-term plugin format

Why WASM:

- safer than native dylib plugins
- avoids Rust ABI instability across dynamic library boundaries
- still allows code-based interaction and stateful behavior
- supports a host-controlled API boundary
- can be driven by events
- can perform async flows like modal interactions and action handling

WASM plugins should receive a host SDK/facade, not direct Rust object references.

---


---

## 3. How a plugin renders our widgets

> **Planner:** F003/P001 · F003/P011 · F003/P017

## 2.6 Plugins should contribute UI declaratively, but from code

Important distinction:

- plugins should not manually hand-write giant JSON blobs as their primary authoring experience
- plugins also should not directly instantiate internal Rust widget structs across the boundary

Preferred model:

- plugins are written in code
- they use a host SDK / builder API
- internally this produces a host-understood declarative model
- the host maps that model to `heca-grid-ui` widgets and manages rendering/event routing

So plugin authors get a code-first API, while the host still controls:

- rendering
- focus
- overlays
- clipping
- region constraints
- widget validation

### 2.6.1 How a plugin renders our widgets — host-adapter + declarative ViewModel (design; Phase 9)

This is the *design* for the plugin authoring path; it is **not built yet**. The
built-in Rust path (plugin-03) builds real widgets directly; the pieces below are
the WASM plugin path (Phase 9) and are recorded so the seam we ship now stays
compatible with them.

**A) Host-adapter pattern — how a plugin becomes a `Provider`.** A WASM plugin
does **not** cross the boundary as a Rust `Box<dyn Provider>` (unstable ABI,
safety). Instead the host wraps each plugin in a first-party **adapter** —
`WasmProviderAdapter: Provider` — that marshals `build_contribution`, events
(`app.on`), and action dispatch to/from the WASM module. So the `Provider` trait
is the **single seam**: built-ins implement it directly; plugins are reached
through a host-owned adapter that speaks the same trait. Everything downstream
(`ChromeHost`, regions, contributions) is identical for both.

**B) Authoring UI: code → builder SDK → declarative ViewModel → host maps to
`heca-grid-ui`.** The plugin never instantiates our widget structs across the
boundary and never hand-writes JSON as its primary experience. It calls a
**builder SDK** that produces a **serializable ViewModel** — a tree of *typed
nodes with props* drawn from the host's **closed widget vocabulary**. The host
receives that tree, maps each node to the real `heca-grid-ui` widget, applies
props, mounts the subtree into the region, and owns render/focus/clip. Sketch of
a plugin building a panel with a button:

```
// plugin code (compiles to WASM), using the host SDK builder:
Panel::new()
    .title("Containers")
    .child(HStack::new()
        .child(Label::new(state.name))
        .child(Button::new("Restart")
            .variant(Variant::Destructive)     // semantic variant; a raw colour is also allowed now
            .size(WidgetSize::Small)
            .on_press(intent("plugin.docker.restart", { "id": state.id }))))
// → serializes to: { kind:"panel", props:{text:"Containers"}, children:[ { kind:"h_stack", … } ] }
// → host maps each node to the grid-ui widget, themes it, mounts it.
```

**C) Props vs styling — CHANGED 2026-07-26.** Props are **serializable data** —
strings, numbers, bools, and *semantic enums* the host knows (`WidgetSize::Small`,
`Variant::Danger|Accent`, `TooltipSide`, …). **Appearance is a prop like any other:**
the `Theme` supplies the default and code may override it with a string (`"#ff8800"`
or a theme name like `"muted"`). Set nothing and you follow the theme, which is what
most widgets should do — a literal colour will not follow a runtime theme reload, and
that trade is the author's to make.

> The old rule read "styling is not a free prop: colors/fonts/alphas come from the
> `Theme`/config, never from raw values passed by the plugin." **It is dead — do not
> restore it.** Full model: [`docs/declarative-ui.md`](docs/declarative-ui.md) R2.

**D) Interaction is intent-based, not callbacks across the boundary.** A
`Button::on_press` does not run plugin code during paint. It carries an **intent**
= a string action id + serializable args. The host routes it: either dispatch a
registered action, or deliver it to the plugin as an event (`app.on`). This keeps
input routing, ordering, and re-entrancy on the host side.

**E) Overlays are requested, not mounted.** A **modal/dropdown/popover is NOT a
region contribution** the plugin mounts — it is host-owned (§2.7.1). The plugin
**requests** it and awaits a typed result:

```
// Simple case via the message convenience; body could instead be a full tree (§2.6.2/§2.7.1).
let result = ctx.overlay.open_modal(
    ModalSpec::message("Restart nginx?", "The container will stop briefly.")
        .danger(true)
        .actions([ModalAction::danger("restart", "Restart"), ModalAction::new("cancel", "Cancel")]),
).await;                                // WASM: marshals as request-id + a resolve event
match result {
    ModalResult::Action { id, .. } if id == "restart" => ctx.actions.dispatch("plugin.docker.restart", args),
    _ => {}
}
```

The host owns z-order, focus trap, ESC, click-outside, positioning, and returns
the typed `ModalResult`/`DropdownResult`. So: **panels/toolbars/status segments →
mounted into a region as a `Contribution`; modals/dropdowns → requested from the
`OverlayHost`.**

**Why this shape:** no Rust object and no GPU/focus/overlay control crosses the
boundary — only serializable data + string ids; the host stays the single owner
of rendering, theming, input routing, and overlay z-order (§2.6/§2.7 guardrails).
Shipped today: the `heca-grid-ui` widgets themselves + the read/observe `App`
facade. Planned: the builder SDK + ViewModel + WASM bridge (Phase 9) and the
`OverlayHost` async API (Phase 8).

### 2.6.2 The declarative widget tree (`ViewNode`) — a SwiftUI/Flutter-style model

The "ViewModel" above is concretely a **recursive widget tree**: a *container*
node holds a **vector of child widgets**, each of which may itself be a container.
This is the same shape as Flutter's `Widget` tree or SwiftUI's `View` tree.

**Half of it already exists.** `heca-grid-ui` is already a retained, recursive
tree: every widget has `Base.children: Vec<Box<dyn Component>>`, the `Parent`
trait exposes `.child(...)`, and `Flex`/`Row`/`Card`/… hold arbitrary nestable
children. That is our *RenderObject/Element* layer. What the plugin path adds is
the **declarative layer on top** — a serializable `ViewNode` (Flutter's `Widget` /
SwiftUI's `View`) that the host **realizes** into that existing retained tree.

**The declarative node** (design; Phase 9):

```
struct ViewNode {
    kind:     WidgetKind,               // closed enum of host-known widgets:
                                        //   containers: Column|Row|Grid|Card|Scroll|Panel
                                        //   leaves:     Label|Button|Badge|Icon|Input|Toggle|StatusDot|…
    props:    PropMap,                  // serializable scalars + semantic enums (variant, size, align…)
    events:   Map<EventName, Intent>,   // on_press / on_change → intent(action_id, args)
    children: Vec<ViewNode>,            // recursive; empty for leaves
}
```

Containers (`VStack`/`HStack`/`Row`/`Grid`/`Card`/`Scroll`/`Panel`, and the modal `body` in
§2.7.1) carry `children`; leaves don't. A typed, SwiftUI-like **builder SDK** sits
on top for ergonomics and emits this uniform node (just as Flutter's typed
`Widget` classes lower to `Element`/`RenderObject`):

```
VStack::new().gap(8).padding(12)
    .child(Label::new(title).size(WidgetSize::Header))
    .child(HStack::new()
        .child(Badge::new(status).variant(Variant::Accent))
        .child(Button::new("Restart").variant(Variant::Destructive)
            .on_press(intent("plugin.docker.restart", { "id": id }))))
```

**The host mapper** — `realize(&ViewNode) -> Box<dyn Component>` — is a recursive
walk: create the `heca-grid-ui` widget for `kind`, resolve `props` against the
`Theme`, wire `events` to intent routing, then recurse on `children` and attach
each via `.child(...)`. Because the retained tree already exists, the mapper only
**translates**; it never reimplements layout, paint, focus, or clipping.

**Vocabulary is closed to plugins, extensible by the host.** Adding a new
`WidgetKind` (e.g. `Table`) is host-side work — the widget in `heca-grid-ui`, its
showcase demo + `docs/widgets.md`, and a mapper arm — **never** something a plugin
invents. Until a first-class `Table` exists, a table is *composed* from the
existing building blocks (`Grid`/`Flex` + `Row` + `Label`/`Badge` + `Scroll`).

---


---

## 4. Overlays are host-owned

> **Planner:** F003/P008 (plugin-05) · F003/P012

## 2.7 Overlays must be host-owned

Because heca is a GUI app, modal/dialog/dropdown/popover behavior must be managed by the host.

Plugins may request overlays, but the host must own:

- z-order
- focus trap
- keyboard routing
- ESC behavior
- click-outside dismissal
- positioning / anchoring

This is critical.

So plugin model should support things like:

- `await app.overlay.openModal(...)`
- `await app.overlay.openDropdown(...)`

This solves the “how do I know which button was pressed?” problem much better than pure JSON triggers.

### 2.7.1 Formal contract (`plugin-task-03`, proposed 2026-07-02 — pending review)

**Grounding — what already exists.** The grid-ui overlay widgets (`Modal`,
`Select`, `CommandPalette`, `Tooltip`, `ToastStack`) each report
`overlay_active()` + `focusable()` while open, draw on the scene's overlay layer
via `cx.with_overlay(...)`, and the `FocusManager` routes pointer/key events to
the active overlay first. Open/close is a **host-owned `Signal<bool>`** per
widget (see `heca-grid-ui/src/widgets/modal.rs`). Dismissal paths are already
correct: buttons, `Esc` (= cancel), scrim click; `dismissible(false)` forces a
button decision. **Two gaps** this contract closes:

1. **No central stack.** Each widget owns its own bool signal; nothing arbitrates
   z-order between several open overlays or owns a single focus trap.
2. **No result value.** Interaction is callback-only (`Modal::confirm`/`cancel`
   closures) — a provider cannot `await` "which button was pressed".

**Decision — a host-owned `OverlayHost`.** A new app-side overlay stack
(`heca/src/chrome/overlay.rs`, built in `plugin-02`/Phase 8) owns an explicit
z-ordered `Vec` of active overlays. It renders each entry through the *existing*
grid-ui widget bound to host-owned signals — providers/plugins **never** build
overlay widgets across the boundary (§2.6); they submit a **spec** and receive a
**typed result**. Input routes to the top of the stack first (reusing the
`overlay_active()`/`FocusManager` path). A **modal** entry is focus-trapping +
scrim + blocks everything below; a **dropdown/popover** entry is light-dismiss
(click-outside or `Esc` pops it) with no scrim.

> **Built on the surface compositor (the planner (F003/P019)).** `OverlayHost` is **not**
> a separate stack: it is the overlay-level API on top of the app's `LayerStack`/
> `LayerRegistry` (the single layering mechanism that owns band z-order, occlusion, hint
> visibility, and later paint + input). `open_modal` `realize`s the `ViewNode` body + actions
> into a native tree, **pushes it as a `Modal`-band layer**, and resolves `ModalResult` when a
> button's intent fires. So the overlay z-order/focus-trap here and the compositor's layering
> are the same stack, described from the overlay API's angle.

**Result-returning API shape.**

```rust
/// Host-owned overlay stack. Providers/plugins submit a spec and await a typed
/// result; the host owns z-order, focus trap, Esc, click-outside, positioning.
pub trait OverlayHost {
    /// Push a modal; resolves when the user confirms, cancels, or dismisses.
    fn open_modal(&self, spec: ModalSpec) -> OverlayFuture<ModalResult>;
    /// Push a dropdown/popover anchored to a rect; resolves on pick or dismiss.
    fn open_dropdown(&self, spec: DropdownSpec) -> OverlayFuture<DropdownResult>;
}

pub struct ModalSpec {
    pub title: String,
    /// The dialog body — a full declarative widget tree (§2.6.2), so a modal can
    /// hold a table/form/list, not just text. `ModalSpec::message(&str)` is a
    /// convenience that wraps a single `Label` in a `body`.
    pub body: ViewNode,
    /// Bottom action buttons. Their id comes back in `ModalResult::Action`.
    pub actions: Vec<ModalAction>,
    pub danger: bool,
    /// `false` = forced decision (Esc/scrim swallowed) — mirrors `Modal::dismissible`.
    pub dismissible: bool,
}
pub struct ModalAction { pub id: String, pub label: String, pub danger: bool }
/// The chosen action id, plus any data the body collected (e.g. a selected row,
/// form field values) marshalled back from the realized widget tree.
pub enum ModalResult { Action { id: String, data: PropMap }, Dismissed }

pub struct DropdownSpec {
    pub anchor: heca_core::layout::Rectangle, // viewport-space anchor (§5.7 geometry)
    pub side: OverlaySide,                     // preferred side; host flips on overflow
    pub items: Vec<DropdownItem>,
}
pub struct DropdownItem { pub id: String, pub label: String, pub enabled: bool }
pub enum DropdownResult { Picked(String), Dismissed }

pub enum OverlaySide { Above, Below, Start, End }
```

**`OverlayFuture<T>` — single-threaded reality.** heca's UI is single-threaded
(`floem_reactive`, `Rc`), so this is **not** a `Send`/`Sync` executor future. It
is a host one-shot handle whose result is delivered on the UI thread. First-party
providers may equivalently pass an `FnOnce(T)` completion; both map to the same
host stack entry. For the WASM bridge (Phase 9) the call marshals as a
`request-id` + a later `resolve` event carrying the result variant — the same
event→read boundary as `App::on` / `App::state`.

**Positioning.** Anchors are in viewport space using `heca-core::layout`
`Rectangle`/`Point`/`Size` (§5.7). The host clamps to the viewport and flips
`side` on overflow — the same behavior `Modal` (centering) and `Select`
(anchoring) already implement, now owned once by the host.

### 2.7.2 Intent / dispatch / overlay-control — decided 2026-07-06

Ratified while building the surface compositor + `ViewNode`; drives `realize`
(plugin-task-ui-3) and `OverlayHost`. **Everything is an action; there is one dispatch.**

- **`view::Intent { action, args }` is the universal invocation currency** — used identically
  by click, the KeyHint picker (`prefix+/`), **RPC**, and plugins. A `ViewNode` node carries
  it via `events` (`on_press`/`on_change`); it holds no closures, so it stays serializable.
- **Convergence carrier:** `InteractionIntent::View(view::Intent)`. `realize` wires each
  actionable node to BOTH `on_click → emit(View(vi))` and a KeyHint target
  `hints.register(View(vi))`, so click + picker fire the same thing; RPC feeds the same
  `dispatch_view_intent` directly. `realize` stays context-agnostic.
- **One dispatch point** `dispatch_view_intent(state, registry, vi)`: resolve `vi.action` to a
  built-in `WmAction` (`action_from_name` + args) or a plugin action, execute via
  `ActionRegistry`. No parallel dispatch path.
- **Overlay control is actions too**, carrying the overlay **id** so any surface can target a
  specific overlay: `WmAction::SubmitOverlay { overlay: OverlayId, action: String }`,
  `WmAction::CloseOverlay { overlay: OverlayId }` (`OverlayId` = the layer's `LayerId`). The
  **`OverlayHost` owns the id** and injects it into each action button when it builds them
  from `ModalSpec.actions` (the author only supplies `ModalAction{id,label,danger}`). The
  `SubmitOverlay` handler resolves that overlay's `OverlayFuture<ModalResult>` (collecting the
  realized body's data into `ModalResult::Action{id,data}`) and pops it.
- **How RPC closes/confirms a modal:** it received the `OverlayId` from `open_modal`, so it
  dispatches `submit_overlay{ overlay: <id>, action }` / `close_overlay{ overlay }` — the same
  action a button press or KeyHint pick fires. Modals are RPC-driven identically to the UI
  (action reachability), targeting a specific overlay by id (not "the top").

So `OverlayHost` is the overlay-level API built **on** the `LayerRegistry` (see §2.7.1 note +
the planner (F003/P019) §9): `open_modal` realizes the `ViewNode` body + injected action
buttons and pushes a `Modal`-band layer; its buttons dispatch overlay-control actions.

---


---

## 5. Chrome-wide, and moving containers

> **Planner:** F003/P006 · F003/P007

## 2.8 The design is not sidebar-only; it is chrome-wide

The same pluggable system should power:

- left sidebar
- right sidebar
- top bar
- bottom bar

This means the real target is not a “sidebar plugin API”.

The real target is a:

- **pluggable chrome host system**

with region-specific contribution APIs.

**Every region is a container over an ordered `Vec` of items, not a fixed hand-built
layout.** The left sidebar, right sidebar, top bar, and bottom bar each hold an
ordered list of contributions (buttons / segments / containers) that first-party code
*and plugins* append to and reorder (via `app.regions.<region>.add_container(...)` —
`plugin-task-16`). So any region restyle (`sidebar-fu-9` bottom bar, `sidebar-fu-15`
top bar, the sidebars) must be **built as a vector-of-items container from the start**,
so a plugin adding a button is just a push into that region's list — never a rewrite.

Examples:

- left sidebar may host WorkspacesContainer
- right sidebar may host AgentsContainer
- top bar may host mode/status/tool segments
- bottom bar may host diagnostics, notifications, git info, plugin status

---

## 2.9 Container movement across compatible regions is a host concern

The architecture should support mounted containers being:

- reordered within a region
- moved between compatible regions
- persisted in their chosen placement

Important distinction:

- **host-level container drag/drop** is a ChromeHost concern
- **container-internal drag/drop** remains the mounted container’s concern

Examples:

- moving `WorkspacesContainer` from left sidebar to right sidebar is **host-level placement behavior**
- dragging panes inside `WorkspacesContainer` is **container-internal behavior**

To support this cleanly, mounted containers should expose metadata such as:

- `id`
- `title`
- `supported_regions`
- `default_region`
- `default_order`
- `movable`
- `collapsible`

This movement must not be mouse-only.

Important app-wide action rule:

- when container movement is supported, it must also be representable as an **action**
- for example, moving a container from left sidebar to right sidebar should be doable by:
  - mouse drag/drop
  - keybinding via an action
  - RPC via the same action model

This rule aligns with the broader heca principle that app capabilities should not be trapped behind only one input surface.

## 2.10 Chrome keyboard focus is a container id — decided 2026-07-28 (F003/P011/T020)

**Which dock the keyboard is aimed at is held as a container id.** Not a side, not a region: a
sidebar is a shell and left/right is only a position, so a dock is focused wherever it happens to be
seated and stays focused when it is moved between regions. Sticky, as focus is.

The bug this replaced read `left_visible()` and expanded the *left* container. The workspaces dock
declares `RegionSet::sidebars()`, so it may sit on the right — and then the focus key expanded an
empty left sidebar and navigated a tree drawn on the right.

**One action, two doors.** `focus_dock` is a single `WmAction` with an **optional** `dock` argument:

- **bare** (a keybinding, `prefix+Shift+e` by default) → the **pick**: every dock on screen lights a
  letter (a `KeyHint` keycap, tinted `warning` so it reads distinctly from a pane or column pick) and
  the next keypress focuses that one. `InputMode::DockPick` holds the candidates; the letter resolves
  back through the same action carrying the id.
- **with an id** (`focus_dock(dock="workspaces")`, `focus-dock workspaces` over RPC, a menu entry's
  `Intent`) → focuses it directly, no pick.

Both doors are the same action, so the keyboard, RPC and a plugin reach one code path. The optional
argument is what makes the bare binding legal — an action with a *required* argument cannot be bound
to a key at all.

**Focus is visible, and the ring is the same one every control draws.** The host wraps each mounted
container in a `FocusScope` (the generic grid-ui wrapper) bound to that placement's
`keyboard_target` signal — which is also the **gate**: keys and widget intents enter a container's
subtree only while it holds focus, and an unfocused one declines rather than consuming, so the host
broadcasts one intent and the focused container answers. It is the **same signal** the container's own
`ScrollRegion` binds as its keyboard target, so the ring and the keys cannot disagree about which dock
has focus, and neither has to be told where the container sits in the tree.

**What a provider says about it.** `Provider::keyboard_navigable()` (default `false`) declares whether
a dock does anything with focus *beyond scrolling* — its own cursor, its own selection.
`WorkspacesContainerProvider` returns `true`. `sidebar_focus` looks for that: it focuses the navigable
dock, reveals whichever region that dock is seated in, and enters nav mode; with no navigable dock
mounted it does **nothing** rather than expanding a region to show an empty frame.

**Per placement, not per kind.** Both the focus target and the keyboard-target signals are keyed by
**mount id**, like the scroll offsets: the same container can be seated twice and only one of the two
can hold focus.

## 2.11 A row is named, not described — the mouse and the menu — decided 2026-07-30 (F003/P086/T365)

**The host resolves *which* row; the component says *what* it is.** A right-click, like a left-click,
resolves to `(container under the point, the row's `nav_key`)` off the retained tree's real bounds.
That pair — `ContextTarget::Row { container, key }` — is all the host carries, and it never parses a
key. Two component-side calls complete it:

- **`Provider::context_path(key, ctx) -> Option<String>`** — which of my menus describes this row.
  Answered by *matching the key against my own rows*, never by parsing it: a tiled pane and a
  floating one are both `pane:<id>`, and only the row knows which it is. `None` (an unknown key, a
  component with no row menus) opens nothing rather than something wrong.
- **the menu builder** — resolves the same key against the component's own model for the facts the
  entries need. A pane's column comes from the component's tree; a workspace's custom name from its
  own projection.

This replaces three workspace-shaped `ContextTarget` variants that the **host** filled in (`ws_idx`,
`col_idx`, `custom_name`) — the last place the host enumerated another component's row kinds, and the
reason a plugin's row could not be right-clicked at all. A component seated twice is asked for every
menu twice, so a builder answers for its own **mount id** and returns nothing for another's.

**A click is a name, not a closure.** Each item kind declares its gestures as an `Intent` (an action
id plus arguments), so the click, the `prefix+/` pick, a menu entry, a keybinding and RPC all reach
the same thing, routed by that action's own policy and passing the destructive-confirm gate. Native
code wires it with `named_press`, the mirror of `realize`'s `press_intent` — one declaration, both
ends. Nothing is inherited between kinds, and a kind with no gesture declares none.

**Nothing is restored when an overlay closes.** A menu opened from a focused container used to leave
and re-enter `SidebarNav`; a container's keyboard focus is not a mode and an overlay never takes it
away, so `overlay_origin_mode`, `restorable_mode` and the `PendingContext` that carried a target
across the `Prefix` transition are all gone.

**An app action is not bent at a container's cursor.** `prefix+$` / `prefix+Shift+w` mean the focused
pane and the active workspace wherever the keyboard is; renaming *the row under the cursor* is the
component's own verb (`workspaces.rename_selected`, `r`), beside `delete_selected` (`x`) — the same
ownership test that keeps `create_workspace` and `zoom_column` out of a component's declarations
(user decision, 2026-07-30).

---


---

## 6. Runtime architecture map

> **Planner:** keep current — F003 overall

## 3. Target Architecture

## 3.0 Runtime architecture map (⚠️ KEEP THIS CURRENT)

> **This map is a living document.** Every phase that adds, moves, or renames a
> runtime subsystem **must** update this tree in the same change, and flip its
> `SHIPPED` / `PLANNED(phase)` marker. A stale map is worse than no map — if you
> touch the ownership graph and don't update this, the change is incomplete.
> Verified against code 2026-07-02 (post plugin-02).

Legend: `[✓]` shipped · `[~]` partially shipped · `[ ]` planned (owning phase noted).

```
HecaApp                                   # winit runtime — the outer shell
├── registry: ActionRegistry          [✓] # NOT inside AppState: stateless rules,
├── keymap: KeymapRegistry             [✓] #   handlers run it on `state`
│   └── mode_keymaps / mode_triggers   [✓]
├── app_config, event_proxy            [✓]
└── state: Box<AppState>               [✓] # THE canonical app state
     │
     ├── ── Axis 1: canonical layout/session (the source of truth) ──
     ├── session: Session              [✓] # workspaces → columns → panes
     │    ├── workspaces: Vec<Workspace>    #   position = index (ordinal, not px)
     │    ├── active_workspace_idx          #   which workspace is visible/focused
     │    └── Workspace{ scrolling(columns), floating_panes, focus_domain }
     │         └── Column{ panes, active_pane_idx, width, … }
     │              └── Pane{ id, title, custom_name, runtime, … }
     ├── sidebar_tree: SidebarTree     [✓] # nav model (a projection of session)
     ├── focused_pane: Option<PaneId>  [✓] # cache of the resolved focused pane
     ├── input_mode: InputMode         [✓]
     │
     ├── ── Axis 2: chrome (the pluggable surround that views/drives Axis 1) ──
     ├── chrome_state: SharedChromeState[✓] # signal-backed DERIVED mirror:
     │    │                                 #   active_pane, per-ws collapse, pick,
     │    │                                 #   scroll, PaneRuntime (proc/status/cwd/git)
     │    ├── focused_container           [✓] # chrome keyboard focus — a CONTAINER ID (§2.10)
     │    ├── keyboard_target[mount]      [✓] #   derived: exactly one placement is true
     │    ├── container_scroll[mount]     [✓] # per-placement scroll offsets (T021)
     │    ├── dock_pick_candidates        [✓] # letter → container id while a dock pick is open
     │    └── events: ChromeEventBus    [✓] # string-named events + "*" catch-all
     ├── chrome_host: ChromeHost        [~] # SHIPPED runtime (plugin-02), still EMPTY
     │    └── regions: [RegionHost; 4]  [~] #   one generic RegionHost per RegionId
     │         │                            #   (LeftSidebar/RightSidebar/TopBar/BottomBar)
     │         └── MountedContribution  [ ] #   mounted providers — plugin-03
     │              └── (built via Provider::build_contribution → grid-ui subtree)
     └── renderers / backends / theme   [✓]

App facade (app.on / app.state)         [✓] # built FROM chrome_state (AppState::host());
                                            #   the read/observe half of the plugin API

Providers (mounted into chrome_host.regions):
  WorkspacesContainerProvider           [ ] # plugin-03 — first built-in; projects `session`
  Agents / Docker / Git / Notes …       [ ] # later built-ins, then WASM plugins

Planned subsystems (not yet in the tree):
  ActionRegistry (dynamic, string-id)   [ ] # plugin-04 (Phase 6) — beside the enum registry
  OverlayHost (modal/dropdown + async)  [ ] # Phase 8 (§2.7.1)
  WASM plugin runtime + host SDK        [ ] # Phase 9
  AgentDriverRegistry (per-pane agents) [ ] # agent-integration plan (parked)
  Container placement persistence        [ ] # plugin-07 (in-memory today)
```

**Two orthogonal axes — the load-bearing idea.** *Axis 1* (`session`) owns the
canonical layout: which workspaces/columns/panes exist, their ordinal position,
what's visible (`active_workspace_idx`), and focus (hierarchical:
`active_workspace_idx` → `workspace.focus_domain` → `column.active_pane_idx`,
cached in `focused_pane`). It is mutated **only** through actions
(`WmAction → ActionRegistry.execute → handler`). *Axis 2* (chrome:
`chrome_state` + `chrome_host` + event bus) is a **derived mirror + pluggable
surround** that *reads/observes* Axis 1 and *dispatches actions* to change it —
it never owns layout truth. A pane therefore has two faces: its layout position
lives in `session`; its runtime (process/status/cwd/git) is mirrored into
`chrome_state` (`PaneRuntime`) and emitted on the bus so chrome/plugins react
without touching the session. The `WorkspacesContainer` is one Axis-2 *projection*
of Axis 1 — **the sidebar is a shell that hosts it, not the workspace tree itself.**


---

## 7. ChromeHost

> **Planner:** F003/P006 (plugin-02) — done

## 3.1 ChromeHost

A new host/controller layer should own all pluggable chrome regions.

Responsibilities:

- maintain registries for all regions
- track region ordering and visibility
- track container placement within and across regions
- collect container contributions
- mount/unmount built-in or plugin-provided containers
- own host-level container move/reorder behavior
- own valid drop-target logic for container placement
- handle invalidation / refresh scheduling
- bridge plugins with app state and action system

Subregions conceptually:

- `LeftSidebarHost`
- `RightSidebarHost`
- `TopBarHost`
- `BottomBarHost`

These may be implementations of one generic region host abstraction.

### 3.1.1 Formal contract (`plugin-task-01`, proposed 2026-07-02 — pending review)

**Decision — canonical region identity: `RegionId`.** Today the only region
identifier in the app is the **event-payload** enum
`chrome::events::ChromeRegion { Left, Right }` (used by `RegionModeChanged` /
`RegionSizeChanged`), and `SharedChromeState` exposes only `left_*` / `right_*`
region reads/writes. The contract widens this to the four canonical regions:

```rust
pub enum RegionId { LeftSidebar, RightSidebar, TopBar, BottomBar }
```

- **In `plugin-02`**, rename the event enum `ChromeRegion` → `RegionId`, add
  `TopBar`/`BottomBar`, and generalize the `SharedChromeState` region API from
  `left_*`/`right_*` pairs to a per-`RegionId` map. The two event variants carry
  `RegionId` unchanged in shape.
- The **grid-ui `ChromeRegion` widget keeps its name** — it is the *oriented
  shell*, not a region identity. One widget instance is mounted per `RegionId`:
  `ChromeRegion::vertical()` for the two sidebars, `ChromeRegion::horizontal()`
  for the two bars. `RegionId` says *which* region; the widget says *how it
  renders*.

**Decision — contribution taxonomy + per-region allow-list.** A contribution is
one of five semantic units (never raw pixels):

```rust
pub enum Contribution {
    Container(ContainerContribution),   // mounted, movable domain container
    ToolbarGroup(ToolbarGroup),         // clustered action buttons
    StatusSegment(StatusSegment),       // text/badge segment
    Panel(PanelContribution),           // fixed, non-movable panel
    OverlayRequest(OverlaySpec),        // ModalSpec | DropdownSpec → OverlayHost (§2.7.1)
}

/// The two overlay specs from §2.7.1, unified for the `OverlayRequest` variant.
pub enum OverlaySpec { Modal(ModalSpec), Dropdown(DropdownSpec) }
```

Allowed per region:

| RegionId                      | Allowed contributions                 |
| ----------------------------- | ------------------------------------- |
| `LeftSidebar` / `RightSidebar`| `Container` (primary), `Panel`        |
| `TopBar` / `BottomBar`        | `StatusSegment`, `ToolbarGroup`       |
| any                           | `OverlayRequest` (region-agnostic → OverlayHost) |

The **`Container`** carries all host-level placement metadata plus a build hook:

```rust
pub struct ContainerContribution {
    pub id: ContainerId,              // stable string id (== provider id)
    pub title: String,
    pub supported_regions: RegionSet, // which RegionIds it may live in
    pub default_region: RegionId,
    pub default_order: i32,           // stacking order within a region (lower = earlier)
    pub movable: bool,
    pub collapsible: bool,
    /// Builds the container body as a host-understood grid-ui subtree. Called by
    /// the region host on (re)mount / invalidation. Returns a *model*; the host
    /// owns render/focus/clip/overlays (§2.6).
    pub build: Box<dyn Fn(&ChromeCtx<'_>, &mut BuildCx<'_>) -> Box<dyn heca_grid_ui::Component>>,
}
```

**The two halves of the seam (settled, plugin-03 `t005`).** Building a body is not a
pure read: it *allocates host ids* — a drag id per draggable/droppable row, a hint
target id per pickable row, a signal per value that updates without a structural
rebuild. So the seam takes two contexts, and the split is the point:

- **`ChromeCtx<'a>` — read-only.** The plugin-facing facade: state selectors
  (`app.state.*`), event subscriptions, plus the frame's render inputs — `tree()`,
  `programs()`, `theme()`, `emit_intent()`. These are `Option`: a context built to
  *observe* (`ChromeCtx::new`, e.g. at `on_activate`) has no frame in flight, so there
  is no theme or tree to read; a context built for a render pass
  (`ChromeCtx::for_build`) has them all.
- **`BuildCx<'a>` — the mutable half.** The host's per-build registries (`signals`,
  `drag`, `hints`), borrowed `&mut` for the duration of one build.

They are two explicit parameters rather than one context with interior mutability:
that keeps `ChromeCtx` a pure read/observe facade (what the WASM bridge will marshal),
turns a double borrow into a **compile** error instead of a runtime panic, and matches
how `realize()` already threads the same registries. A WASM provider never sees
`BuildCx` — it returns a `ViewNode`, and the host's adapter realizes it, registering the
ids on its behalf.

**ChromeHost responsibilities** (app-side, `heca/src/chrome/host.rs`, `plugin-02`;
bridges to the shipped `App` facade in `heca/src/host.rs` for state + events):

- own a registry of mounted contributions **per `RegionId`**, with order + visibility;
- track container placement (which `RegionId`, which order) and **persist** it;
- own **host-level** container move/reorder between compatible regions, validated
  against `supported_regions` — distinct from **container-internal** DnD, which
  stays inside the mounted container (§2.9);
- compute host-level drop targets for container DnD;
- schedule **invalidation** — re-call a provider's `build_contribution` when the
  events it subscribed to fire (bridged from the `ChromeEvent` bus via `App::on`);
- bridge event dispatch to providers.

```rust
pub struct ChromeHost { /* per-RegionId registries, placement map, App bridge */ }
impl ChromeHost {
    pub fn register(&mut self, provider: Box<dyn Provider>);
    pub fn contributions(&self, region: RegionId) -> &[MountedContribution];
    pub fn move_container(&mut self, id: ContainerId, to: RegionId) -> Result<(), MoveError>;
    pub fn reorder(&mut self, id: ContainerId, before: Option<ContainerId>);
    pub fn set_region_visible(&mut self, region: RegionId, visible: bool);
}
```

**Geometry rule (§5.7).** Every rect/point/size in ChromeHost / provider /
overlay APIs uses `heca-core/src/layout/types.rs` `Rectangle` / `Point` / `Size`.
The legacy `heca_core::types::Rect` must not appear in any chrome-facing API
(cleanup is `plugin-task-04`).

**Movement-as-action rule (§2.9).** Every host-level placement mutation
(`move_container`, `reorder`, `set_region_visible`) must also be reachable as a
named `WmAction` (`plugin-task-08`) so mouse, keyboard, and RPC hit the same
path. The methods above are the internal API; the actions are the public surface.

---


---

## 8. Region contributions

> **Planner:** F003/P006 — done

## 3.2 Region contributions

A contribution should not mean “raw pixels”.

A contribution should be one of a small set of semantic units, such as:

- container
- toolbar group
- status segment
- panel
- overlay request

For sidebar regions, the most important contribution type is:

- a **mounted container contribution** hosted inside the Sidebar shell

Important separation:

- the Sidebar shell provides visual/layout hosting behavior
- the mounted container provides domain-specific interaction behavior

Examples:

- `WorkspacesContainer` owns workspace-tree semantics
- `AgentsContainer` owns agent-list semantics
- `DockerContainer` owns docker-list semantics

A future `SidebarContainerFrame` widget may be useful as a visual wrapper around mounted containers, but it should not be confused with the provider/container logic itself.

Important convergence note with terminal work:

- the real terminal implementation should eventually mount here as a hosted
  content provider inside a pane shell / ChromeHost-managed container boundary
- the terminal backend/renderer must not become a separate competing pane
  architecture
- the pane shell / ChromeHost layer should own:
  - outer chrome
  - region placement
  - content rect and clipping
  - process/global metadata presentation
- the terminal host should own:
  - PTY/backend/runtime state
  - terminal snapshots
  - terminal content rendering
  - terminal input routing
- this convergence is tracked in `terminal-implementation.md` Phase 8

---


---

## 9. Shared UI / chrome state

> **Planner:** BUILT — heca/src/chrome/state.rs

## 3.3 Shared UI / chrome state

> **Foundation landed** (pane-runtime initiative Phases 0–1): `SharedChromeState`
> (`heca/src/chrome/state.rs`) is the shared, signal-backed store — region
> visibility/size, active/hovered pane, per-workspace collapse, pick candidates,
> scroll, and the per-pane runtime mirror. Reads via selectors; writes via the
> store's `set_*` chokepoint (which emits events). Providers read it through the
> host API's `app.state.*` (§3.5), not directly.

A new shared state layer will be required to coordinate:

- scrolling area
- workspace tree / WorkspacesContainer
- future additional containers
- overlay state
- focus/selection
- per-container UI state such as search/filter query or collapse state

This state must live outside the widgets.

Examples of likely shared UI state:

- expanded/collapsed container ids
- selected row ids
- hovered row ids
- drag state
- per-container search queries
- container order
- container placement by region
- region visibility
- scroll offsets
- active overlay stack

> **Concrete bug this state must solve (2026-06-22):** today the expanded
> sidebar / WorkspacesContainer highlights only `active_pane`, while sidebar
> navigation mutates `AppState.sidebar_tree.cursor/current_item()`. Result:
> `prefix+e` → `j/k` moves the nav model internally, but **nothing visibly
> changes** in the expanded sidebar because the selected row/item is not
> projected into shared chrome state. This is the canonical example of why
> `selected row ids` / `focus-selection` must live in shared UI state rather
> than inside ad hoc widget-local or module-local structures.

---


---

## 10. Provider model

> **Planner:** F003/P004 (plugin-03) — done

## 3.4 Provider model

Built-in first, plugin-driven later.

Conceptually each provider should:

- identify itself
- declare which region(s) it supports
- provide container placement metadata
- subscribe to host/app events
- read state through the host API
- build **container contribution models** / UI models
- register actions
- respond to action invocations

A provider should not be thought of as “providing sidebar rows”. It provides a mounted container contribution.

The first provider should be:

- `WorkspacesContainerProvider`

The current sidebar code should be gradually migrated into that shape.

### 3.4.1 Formal contract (`plugin-task-02`, proposed 2026-07-02 — pending review)

**The `Provider` trait.** A provider never mutates app state directly (§2.3): it
reads through `ChromeCtx` selectors, reacts to events, and dispatches actions.

```rust
/// A built-in (later WASM-backed) contributor of chrome content.
pub trait Provider {
    /// Stable identity — also the `ContainerId` when it contributes a container.
    fn id(&self) -> &str;
    /// Regions this provider's contribution may be placed in.
    fn supported_regions(&self) -> RegionSet;
    /// Where it mounts by default on first run.
    fn default_region(&self) -> RegionId;
    /// Default stacking order within a region (lower = earlier).
    fn default_order(&self) -> i32 { 0 }
    /// Human title (rail/tab label, move menu).
    fn title(&self) -> &str;
    /// Host-level move/reorder allowed?
    fn movable(&self) -> bool { true }
    /// Collapsible within its region shell?
    fn collapsible(&self) -> bool { true }
    /// Build the contribution model. Called on mount and on each invalidation.
    fn build_contribution(&self, ctx: &ChromeCtx<'_>) -> Contribution;
    /// Subscribe to events / register actions on activation. The returned RAII
    /// handles are held by the host while the provider is mounted, and dropped
    /// (unsubscribing) on unmount.
    fn on_activate(&mut self, _ctx: &ChromeCtx<'_>) -> ProviderHandles {
        ProviderHandles::default()
    }
}
```

**`ChromeCtx` — the provider/plugin-facing facade.** It *extends* the shipped
read/observe `App` (`heca/src/host.rs`, which already gives `on(event)` +
`state()` selectors) with the write/contribute halves that §3.5 rows 3–10 defer.
`plugin-01` only names them; they are implemented in later phases.

```rust
pub struct ChromeCtx {
    app: App,                 // SHIPPED: on(event) + state() read selectors (§3.5 rows 1–2)
    // actions: ActionDispatch,   // dispatch/register string actions   (plugin-04/05)
    // overlay: OverlayHandle,    // open_modal/open_dropdown (§2.7.1)   (plugin-05/Phase 8)
    // regions: RegionHandle,     // add/move containers                 (plugin-05)
}
```

**Lifecycle (state machine).**

1. **register** — `ChromeHost::register(Box<dyn Provider>)` records it and reads
   its placement metadata (`supported_regions` / `default_region` / `default_order`).
2. **on_activate** — provider subscribes to events (`ctx.on(...)`) and registers
   actions; returns `ProviderHandles` the host keeps alive.
3. **build_contribution** — host calls it, receives a `Contribution` *model*, and
   mounts the mapped grid-ui subtree into the region shell.
4. **react** — on a subscribed `ChromeEvent`, the provider marks itself dirty; the
   host **invalidates** and re-calls `build_contribution`.
5. **move / reorder** — host updates placement (validated vs `supported_regions`);
   the contribution is remounted in its new slot.
6. **unmount** — host drops the provider's `ProviderHandles`, unsubscribing events
   and unregistering actions.

**Rules.** `build_contribution` returns a *model*, never widget references held
across rebuilds — the host owns render/focus/clip/overlays (§2.6). The first
concrete provider is **`WorkspacesContainerProvider`** (`plugin-task-10`),
migrating `heca/src/sidebar/` into this shape; its container-internal DnD stays
inside the container (§2.9), and its sidebar-nav selection projects into shared
chrome state (`plugin-task-10a`).

---


---

## 11. Future WASM plugin host API

> **Planner:** F003/P001 (plugin-08)

## 3.5 Future WASM plugin host API

The host should expose a controlled plugin API that supports:

- `app.on(event, handler)`
- `app.state.*` read accessors/selectors
- `app.actions.register(...)`
- `app.actions.dispatch(...)`
- `app.overlay.openModal(...)`
- `app.overlay.openDropdown(...)`
- `app.regions.leftSidebar.addContainer(...)`
- `app.regions.rightSidebar.addContainer(...)`
- `app.regions.topBar.addContainer(...)`
- `app.regions.bottomBar.addContainer(...)`
- `app.regions.moveContainer(containerId, targetRegion, options?)`

This is the eventual developer-facing contract.

> **Foundation landed (pane-runtime initiative Phase 8, 2026-06-21).** The first two
> rows — `app.on(event, handler)` and `app.state.*` read selectors — are implemented
> first-party in `heca/src/host.rs` (`App::on` / `App::state()`), over the Phase 0
> event bus + reactive store. `App` is a cheap clone of `SharedChromeState`;
> `state.host()` hands one out. Plugins/providers **react via events and read via
> selectors — never the internal `floem_reactive` signals** — which is exactly the
> boundary the WASM bridge (Phase 9) will marshal. `app.actions.*`, `app.overlay.*`,
> and `app.regions.*` remain future phases.

---


---

## 12. What must change in the codebase

> **Planner:** F003 — see each subsection

## 5. Things That Must Explicitly Change in the Existing Codebase

This architecture implies future changes to at least these areas:

### 5.1 Sidebar assumptions

- current sidebar code must stop being treated as “the sidebar”
- it becomes `WorkspacesContainer` logic

### 5.2 Action system

- current static action routing must evolve toward dynamic registration

### 5.3 Shared UI state

- more state must move out of ad hoc widget/module-local assumptions and into a shared UI/chrome state layer

### 5.4 App/plugin event system

- the app needs a formal event publication/subscription model
- **DONE** (pane-runtime Phase 0 + 8): a typed `ChromeEvent` bus (`heca/src/chrome/events.rs`)
  with string-named events + `"*"` catch-all and RAII subscriptions, emitted from the
  store's mutation chokepoint; exposed first-party as `app.on(event, handler)` in
  `heca/src/host.rs`. WASM bridging is Phase 9.

### 5.5 Overlay ownership

- modals/dropdowns/popovers must be managed by the host, not ad hoc per feature

### 5.6 `heca-grid-ui`

- must expand with richer chrome/container/item primitives
- but should still remain presentation-focused

### 5.7 Geometry unification — **DONE (verified 2026-07-02, `plugin-task-04`)**

- chrome-facing geometry is unified on `heca-core/src/layout/types.rs`
- ~~migrate remaining legacy `heca_core::types::Rect` usage out of chrome-facing code~~ — **already gone**: `heca-core` has no `types` module and no `Rect` geometry type (only `Rectangle` in `layout::types`); nothing in `heca/src` imports a bare `Rect`. `heca/src/chrome.rs` is now the split `heca/src/chrome/`, which uses `Rectangle`/`Point`/`Size`.
- new ChromeHost / provider / overlay APIs use `Rectangle` / `Point` / `Size` as the canonical geometry contract (§3.1.1)

---


---

## 13. Library principles and ownership

> **Planner:** F004 — all phases

## 0. Principles that drive everything

**(P1) `heca-grid-ui` is presentation vocabulary only.** It provides shells, frames, rows, layout, icons, and the *hooks* for drag-and-drop. It owns **no** canonical state, **no** domain logic, and never depends on `heca`/WM state. Layering stays `heca → heca-renderer → heca-grid-ui → heca-core`.

**(P2) Input parity — mouse = keyboard = RPC (hard, app-wide rule).** Every chrome interaction — collapse/expand a region, focus or peek a collapsed dock, move/reorder a dock, select/activate a row — must be a **named action** reachable equally by **mouse, keyboard, and RPC**. **No mouse-only behavior, ever.** A collapsed icon-rail must be keyboard-expandable via an action, exactly like today's keyboard-driven sidebar. (Mirrors chrome plan §2.9 / §11.) For grid-ui this means: widgets expose *intents* (callbacks / opaque action ids), never bury behavior in pointer handlers that the keyboard can't reach.

**(P3) State is read via signals, written via actions.** Widgets and Docks **read** fine-grained `Signal`s and **never mutate canonical state directly** — they dispatch actions (chrome plan §2.3). This keeps writes uniform and automatically reachable from keyboard/RPC (P2).

```
heca (app)                         heca-grid-ui (library)            heca-renderer
──────────                         ──────────────────────            ─────────────
ChromeHost (regions, placement)    ChromeRegion / Sidebar shell      Scene → GPU
AppState (namespaced signal store) DockFrame (title/collapse/handle) PushClip/PopClip
Docks: WorkspacesDock, GitDock…    Grid (tracks + named areas)       (needed for scroll)
  └ own logic + internal items     Item / ItemGroup (rows)
Actions (the only writes)          Icon, StatusDot, Badge, Tag       (shipped: src/drag/
DnD meaning = dispatch(action)     DnD hooks → shipped src/drag/      DragContext etc.)
```

---

## 1. Terminology & ownership

| Term | Lives in | What it is |
|------|----------|-----------|
| **Region** | app (ChromeHost) + grid-ui shell | A chrome area: left/right **sidebar** (vertical), **top/bottom bar** (horizontal). App owns which regions exist + visibility + placement; grid-ui renders the **region shell**. |
| **Dock** | **app** | The hosted, **movable** unit a region mounts (`WorkspacesDock`, `GitDock`, the docker one, …). Owns its domain logic + internal items. Declares metadata (`id`, `title`, `supported_regions`, `default_region`, `movable`, `collapsible`). The region can host/move/drag a Dock but knows **nothing of its contents**. |
| **`DockFrame`** | grid-ui (**new**) | Visual wrapper around a mounted Dock: title bar, collapse toggle, drag handle, a header-controls slot (a Dock may put its own search/filter there), body. Reuses `Pane`'s rounded-bracket painting. The chrome plan's `SidebarContainerFrame`. |
| **`ChromeRegion` / `Sidebar`** | grid-ui (**new**) | The **shell**: oriented (vertical/horizontal), toggle/collapsible, mode-aware, stacks `DockFrame`s, scrolls, and is a **drop target** for Dock-level DnD. No tree/workspace/expand/drag *semantics* of its own. |
| **AppState** | **app** | A **namespaced signal store** (see §4): canonical state + per-namespace derived/UI state. grid-ui widgets **read** signals from it; they never own it. |
| **Item / ItemGroup** | grid-ui | Reusable row + collapsible group a Dock *may* use internally. Domain-neutral (menus, dropdowns, Docks all reuse them). `Item` already exists = the chrome plan's "SidebarItem". |

> App-side dock names are the app's call (e.g. the docker one as `DockerDock` reads awkwardly — name it by function/title). grid-ui is indifferent.

---


---

## 14. Terminology and ownership

> **Planner:** F003/P006 · F004

## 2. What grid-ui must provide

Presentation-only. "Status" is relative to PR #30/#32.

### 2.1 `Grid` layout widget — the flexible item content
The enabler for rich items ("a CSS grid where we can put whatever we want"). taffy (already a dep) supports CSS Grid, so:
- New `Grid` widget over taffy `display: grid`, exposing **both** explicit tracks and named areas:
  - column/row **tracks** (`px` / `fr` / `auto` / `min-content`), `gap`;
  - place a child by **named area** *or* by explicit **cell + span**.
- A rich pane row becomes a `Grid` of arbitrary cells:
  ```
  areas:  "icon  title    status"
          "icon  subtext  exit"
  ```
  each cell holds any `Component` (Label, Badge, StatusDot, Icon, …).
- Extends `Style`/`to_taffy()` with grid fields (or a parallel `GridStyle`).
- **Status:** new. Independent — first to build.

### 2.2 Icon support — embedded, host-registered icon font
- Vendor a permissively-licensed glyph icon font; the **host registers** it as a second family (same mechanism as Geist Mono; family stays theme-configurable). Needs a ~1-line OK from the renderer dev to register a 2nd font.
- `Icon` widget renders a single codepoint via the existing text path — no renderer texture work.
- **Status:** new (known gap). Confirm 2nd-font registration with renderer.

### 2.3 `ItemGroup` (collapsible group)
- Header row (label + chevron + optional count/controls) over a collapsible child set. Collapse state is a `Signal` (owned by AppState in real use). Expand/collapse is an **action** (P2).
- **Status:** new. Builds on `Item`.

### 2.4 `DockFrame` (new; reuses Pane brackets)
- Titled, **collapsible** frame: title bar (title + collapse toggle + **drag handle** + header-controls slot) over a body that hosts the Dock's content; keeps the rounded corner brackets.
- Keep `Pane` as the plain framed container; `DockFrame` is the chrome wrapper.
- **Status:** new (Pane exists, has no header/collapse/handle).

### 2.5 `ChromeRegion` / `Sidebar` shell (generic, all 4 regions)

> **UPDATE 2026-07-11 — the collapsed icon rail is DROPPED for the app.** A region is now
> **Expanded ⇄ Hidden**; there is no icon rail in heca. The collapsed/rail material in this section
> (the "two rail flavors", `RailCell`-per-item, `KeyHint`-over-cells) is **deferred**: it is the
> *future generic* design to build only when a Provider needs an always-visible status rail. `RailCell`
> and `KeyHint` remain shipped grid-ui widgets, but nothing in the app mounts a rail. Authoritative
> decision + rationale + the full future spec: **the planner (F003/P020) — see the planner (F003/P020)**.

- **Oriented** shell: vertical (sidebars) or horizontal (top/bottom bars). One widget covers all four regions.
- Toggle/collapse, **mode-aware**: informs children of the display mode via a signal. **In the app today
  the modes used are `Expanded` and `Hidden`** (`RegionMode::CollapsedRail` stays in the enum, unused).
- **[DEFERRED] Collapsed = icon rail** (thin rail of dock icons; click *or keyboard action* to expand/peek — P2). **Two rail flavors (locked 2026-06-10):** a *tool* dock **folds** to a single icon (`DockFrame::rail(mode_signal, Glyph)`); a *list* dock (workspaces/columns/panes) **enumerates** — one `RailCell` (square icon cell) **per item**. **Icons by default**, not letters. The move/swap/focus-select **pick letters** appear over the cells via the generic **`KeyHint`** overlay, driven by a host-owned `Signal<Option<String>>`. **Shipped (grid-ui side):** `RailCell` + `KeyHint` + showcase `p`-pick demo. **App-side mapping: dropped (see the update note above); revive as the generic render-per-mode path if a Provider needs a rail.**
- Stacks `DockFrame`s, scrolls (§2.8), exposes **Dock-level drop targets**.
- **No** workspace/tree/expand/drag *semantics* — those belong to the mounted Dock. Replaces the old "Sidebar = tree-nav".
- **Status:** new.

### 2.6 Drag-and-Drop — extend the shipped framework (see §3)
- **The DnD system has landed** in `heca-grid-ui/src/drag/` (`DragSurfaceId`/`DragItem`/`SurfaceDragState`/`DragContext`). **Do not build a parallel one.**
- grid-ui's job: drive `SurfaceDragState`/`DragContext` from `DockFrame`'s drag handle + `ChromeRegion`/`Item` drop targets; ensure every drop is an **action** (P2).
- Extend **additively** (new `DragSurfaceId`/`DragItemKind` variants); never modify/retype existing drag types or remove variants.
- **Status:** framework present; DockFrame/region **hooks** are new work — tracked in the planner under F004, not here.

### 2.7 Status-driven item composition — neutral primitives + a recipe
- The rich pane row (program name · git status+icon · exit code · running/idle/stopped style) is **composed**, not a monolith: `Grid` + `Label` + `StatusDot` + `Badge`/`Tag` + `Icon`.
- grid-ui **stays domain-neutral**: ships the primitives + style/color tokens; the **Dock maps** program/git/activity state onto them (running→accent, idle→muted, stopped/exit≠0→danger). No `ActivityStatus` enum in the library.
- Add a thin `Tag`/`Chip` (e.g. git branch) and optionally a `MetricRow` convenience.
- **Status:** primitives mostly exist; needs `Grid` (2.1) + `Icon` (2.2) + `Tag`.

### 2.8 Scroll / list primitive — **DONE** (`gridui-01`)
- Region shells and dock lists overflow → need embeddable scrolling.
- **Resolved:** `PushClip`/`PopClip` is implemented in `heca-renderer/src/scene.rs` (nesting + intersection). `gridui-01` ships the `ScrollRegion` widget (`heca-grid-ui/src/widgets/scroll_region.rs`) reusing the **whole-page scroll pattern** (shift subtree bounds + clip) inside a widget — bakes `-scroll_offset` into the children's bounds so paint/hit-testing/DnD all see the visual position, and clips via `PushClip`. A new post-order `Component::on_layout` hook (called by the layout engine after `assign`) resets the baked offset on a fresh layout so the shift never compounds — the enabler for an embeddable scroll viewport that doesn't own the layout/scroll cycle. v1: vertical-only, multi-child column; wheel (~10% viewport/notch, viewport-proportional) hover-gated via `PointerMoved`; draggable theme-accent thumb (wider 16px grab lane, `Theme::control_radius()` radius); offset as `Signal<f32>` + `scroll_to` (clamps + bakes shift). **Focus-gated keyboard scroll** (focusable; `Event::Key` goes to the focused component only, so the gate is just `focused`): arrows + `j`/`k` (with or without `Ctrl`) step, `Home`/`End` jump, focus ring; a focused child keeps its keys. **Scroll-into-view API** for keyboard cursor following: `ensure_visible(visual_rect)` (minimal scroll, recovers natural position internally via the baked shift) + `scroll_to_child(index)` — widget-side prep for the sidebar (mount tree in a `ScrollRegion`, `SidebarNav` cursor handler calls `ensure_visible`). Same radius-token fix applied to `MarkerGroup`'s bar. Grid-ui 47 tests, clippy 0. **Future:** horizontal scroll, a dedicated scrollbar color token, PageUp/PageDown keys (`GridKey` lacks page keys), host-side hit-testing for nested scroll regions, and the sidebar wiring itself (separate phase).

---


---

## 15. What the widget library provides

> **Planner:** feature **F004** · all widgets built

## 3. Drag-and-Drop — build on the **shipped** framework (`heca-grid-ui/src/drag/`)

The DnD system **landed** (merged from `feature/gpt-refactoring`, 2026-06-09). grid-ui now contains `src/drag/` — a **surface-agnostic, GPU-free state framework**. We **extend** it; we do not invent a parallel one.

### What shipped (the real API)
- **`DragSurfaceId`** — a **closed enum** of drag surfaces (`LeftSidebar` today; `RightSidebar`/`Inspector` are TODO variants). Enum (not trait) on purpose: compiler-checked exhaustiveness, zero-cost dispatch, and it avoids the `Box<dyn> + &mut AppState` self-borrow.
- **`DragItem { surface, id: DragItemId, kind: DragItemKind, pane_id: Option<u64> }`** — `DragItemId(usize)` is an opaque flat index each surface interprets; `DragItemKind` = `{Pane, Workspace, Column, FloatingPane}`.
- **`SurfaceDragState { phase, hover_item, source_item, ghost_label }`** — `phase: SurfaceDragPhase = Idle → Starting{threshold,…} → Dragging`. Helpers: `is_dragging`, `dragged_pane_id`, `original_ws`, `is_swap`, `reset`.
- **`DragContext { active_surface, surfaces: HashMap<DragSurfaceId, SurfaceDragState> }`** — one mouse ⇒ one active surface, but **all** surfaces update `hover_item`, so multiple drop targets highlight at once. This already *is* the cross-region "drag a Dock from one sidebar to another" model.
- **`DragLabel`** (ghost geometry the renderer follows); **`math::rubberband()` + `DEFAULT_DRAG_THRESHOLD_SQ`**.
- **Dispatch + meaning are app-side** (`heca/src/mouse/{target.rs (enum dispatch), surface_left.rs, interactive.rs}`); every drop routes through the WM **action registry**. `InteractiveMove` (content-area pane drag) stays a separate app concept, *not* a `SurfaceDragState`.

### Two altitudes — both map onto the shipped model (the hooks are an extension, not a new system)
- **Dock-level** (move/reorder a `DockFrame` within a region or **between regions**) → a **region is a `DragSurfaceId`**; a Dock is a `DragItem` on it — `DragContext`'s multi-surface hover already supports this.
- **Item-level** (reorder rows inside a Dock, e.g. panes in `WorkspacesDock`) → the Dock's own surface + its `DragItem`s.

### grid-ui's responsibilities (presentation + state)
- `DockFrame` (via its drag handle) and `Item` drive `SurfaceDragState.phase` (`Starting`/`Dragging`) carrying a `DragItem`.
- `ChromeRegion` / inter-dock gaps / `Item` rows set `hover_item` when they can receive.
- Render ghost/insertion visuals in the **overlay layer** (exists). Clipping the ghost / dropping inside a scroll region needs `PushClip`/`PopClip` (blocked, §2.8).

### App's responsibilities (P2 + P3) — unchanged from what shipped
- Decide what's draggable; on drop, **dispatch an action** (`chrome.dock.move_to_region`, `chrome.dock.reorder_before`, Dock-internal `workspace.pane.move`, …) through the registry — never mutate directly. Persist placement in AppState.
- The **same move must be reachable by keyboard + RPC** via that action. DnD is only the mouse surface.

### Extensibility gap & its cheap fix (closed enum ↔ plugins)
The closed `DragSurfaceId` / `DragItemKind` enums are **correct for built-in surfaces** but a plugin can't extend them at runtime. This **does not bite yet** (built-in-first; plugins = chrome-plan Phase 9). Resolve later, **additively**, with *closed-core + one open variant*:
- `DragItemKind::Custom(u32)` — so non-WM Docks (Git/Docker) don't inherit WM nouns.
- `DragSurfaceId::Plugin(PluginSurfaceId)` — one variant funnels dynamic surfaces through runtime dispatch; built-ins keep zero-cost enum dispatch.

> **Additive-only rule (no app impact).** *Extend* the shipped framework — add surfaces, item kinds, and new widget files. **Never** modify/retype an existing drag type (keep `(f32,f32)` as-is; don't swap to `Rectangle`) and **never** remove/rename a variant — the app constructs and exhaustively matches these. Adding a variant changes no behavior; the compiler simply requires new `match` arms at dispatch sites (the point of the closed enum).

---


---

## 16. Drag and drop — the shipped framework

> **Planner:** **P079(F004)** (gridui-08) — NOT BUILT: hooks

## 4. Shared state strategy (the flexible, plugin-ready pattern)

> Answers: "keep the scrollable area, the WorkspacesDock (panes), and future plugins in sync — most flexible solution" and "state must not be hardwired to panes/ws/columns; define a reusable pattern for future plugins/core features."

**Model: one app-owned, namespaced, signal-backed store. Read via signals; write via actions. grid-ui widgets are pure consumers.**

### Shape
- **AppState** (app-side) = a registry of **namespaces**, each owned by a feature/dock/plugin:
  - `chrome` — region visibility, dock placement/order per region, region collapse mode, scroll offsets, overlay stack, active drag session.
  - `workspaces` — canonical ws / columns / panes (from the WM).
  - `<dock-id>` / `<plugin-id>` — that unit's own UI + derived state.
- **Two tiers** (chrome plan §2.3 / §3.3):
  - **canonical** — owned by the app/WM; never mutated directly by docks/plugins.
  - **derived / UI** — per-namespace; a dock owns its slice (selection, hovered, expanded ids, search query, scroll offset).
- **Everything is addressable as `(namespace, key) → Signal<T>`.** UI concerns like selection / hover / collapse / search are **not special-cased** — they're ordinary namespaced keys, so a plugin gets the same capabilities as a core dock with no core change.

### Access contract
- **Read** = subscribe to fine-grained `Signal`s / selectors. floem reactivity gives "attach and react" for free — only dependents repaint; the scroll area, WorkspacesDock, and plugin docks all observe one coherent store and update reactively (no manual refresh wiring).
- **Write** = **dispatch an action** (`namespace.verb`, args) — even for a unit's own slice. Uniform, and keyboard/RPC-reachable (P2/P3).

### How grid-ui consumes it (two options; start simple, scale later)
- **(a) Explicit signal injection** *(start here)* — the app passes the specific `Signal`s a Dock/widget needs at construction. Simplest, explicit, zero new library machinery.
- **(b) Scoped context handle** *(scale path)* — the app hands a Dock a `ChromeCtx` carrying read-selectors + `dispatch` for its namespace. More ergonomic with many docks/plugins. The handle is **app-side**; grid-ui *may* later add a tiny "pass a context down the subtree" helper, but it is **not** required and must not become a global store inside the library.

### Extensibility
- A new dock/plugin **registers a namespace** + declares its state keys + its actions — no core change. This is what lets future plugins (WASM, later) read coherent state through a controlled surface and write only via dispatched actions (chrome plan §3.5).

### Selection model
- Lives as a namespaced key (`<dock>.selection`): a single id today, `Vec<id>` when a dock needs multi-select. `Item`/`ItemGroup` stay pure — they read an `is_active` signal and emit activate **intents**; they never own selection.

---

## 5. What stays OUT of grid-ui (app-side)


---

## 17. Shared state strategy

> **Planner:** BUILT — heca/src/chrome/state.rs

- `ChromeHost`, region registry, dock placement/order, persistence.
- `AppState` (the namespaced store), canonical state, the **action registry**, keybindings, RPC.
- The **Docks** themselves — `WorkspacesDock` (the current `sidebar.rs` logic, migrated per chrome plan Phase 5), `GitDock`, the docker dock, …
- The **DnD dispatch + meaning** (`heca/src/mouse/{target,surface_left,interactive}.rs`) — the drop's action. (The drag *state framework* itself lives in grid-ui's `src/drag/`.)
- Overlay *ownership* (host-owned per chrome plan §2.7) — grid-ui provides the overlay *layer*; the host owns z-order/focus-trap/ESC/anchoring.

---

## 6. Phases / tasks — REMOVED 2026-07-26

The G1–G8 list that stood here is gone. Every item in it was **built**: `Grid`, `Icon`,
`ItemGroup`, `DockFrame`, `ChromeRegion`, the drag-and-drop framework (`heca-grid-ui/src/drag/`),
`ScrollRegion` and `Tag` all ship today. The list was written 2026-06-09, was never updated, and
used ids (G1…G8) that nothing else in the project refers to — only G7 ever had a counterpart in the
planner (**P019(F004)**, gridui-01).

**The planner is the only record of what is planned or done.** Anything still open on the grid-ui
side lives under feature **F004**. Do not track work here. **Phase numbers are global, not
per-feature**: F004's phases are `P019`, `P052`, `P079`… so there is no such thing as `F004/P001`, and
a hand-written `F00x/P00y` in prose is almost certainly invented. Ask the planner
(`planner-phase-show <shortId or title>`) and paste the ref it gives back.

## 7. Open questions (resolved + remaining)

**Resolved 2026-06-09:** sequencing (vocabulary now) · region scope (generic all-4) · DockFrame (new, reuse Pane) · status styling (neutral) · DnD (build on shipped `src/drag/`, extend additively) · Grid API (tracks + areas) · icons (embedded, host-registered) · scroll (request renderer clip) · collapsed mode (icon rail, keyboard-expandable) · state (namespaced signal store; read=signals, write=actions; inject explicitly first, context handle later) · selection (namespaced key, app-owned).

**Remaining:**
- **DnD ↔ action bridge signature** — define the `DockFrame`/region hook → `dispatch(action)` shape on top of `DragContext`; align with the `ActionSink` opaque-id design (grid-ui-plan §12) so DnD + shortcuts share one path.
- **Closed-enum ↔ plugins** — when non-WM Docks / plugins arrive, add `DragItemKind::Custom(..)` / `DragSurfaceId::Plugin(..)` (additive; chrome-plan Phase 9). Don't genericize now.
- **Icon font choice** — which family + licensing; confirm renderer 2nd-font registration.
- **`Grid` surface detail** — how much of taffy grid to expose (min-viable: tracks + areas + span).
- **`ChromeCtx` shape** — defer until enough docks exist to justify (b) over (a).
- App-side (owned by the chrome plan, not here): dock metadata, the `AppState` namespace registry, the action ids.

---


---

## 18. What stays out of the widget library

> **Planner:** F004 — boundary rule




---

## 19. Open questions from the library plan

> **Planner:** mostly stale — see §21

## 7. Open questions (resolved + remaining)

**Resolved 2026-06-09:** sequencing (vocabulary now) · region scope (generic all-4) · DockFrame (new, reuse Pane) · status styling (neutral) · DnD (build on shipped `src/drag/`, extend additively) · Grid API (tracks + areas) · icons (embedded, host-registered) · scroll (request renderer clip) · collapsed mode (icon rail, keyboard-expandable) · state (namespaced signal store; read=signals, write=actions; inject explicitly first, context handle later) · selection (namespaced key, app-owned).

**Remaining:**
- **DnD ↔ action bridge signature** — define the `DockFrame`/region hook → `dispatch(action)` shape on top of `DragContext`; align with the `ActionSink` opaque-id design (grid-ui-plan §12) so DnD + shortcuts share one path.
- **Closed-enum ↔ plugins** — when non-WM Docks / plugins arrive, add `DragItemKind::Custom(..)` / `DragSurfaceId::Plugin(..)` (additive; chrome-plan Phase 9). Don't genericize now.
- **Icon font choice** — which family + licensing; confirm renderer 2nd-font registration.
- **`Grid` surface detail** — how much of taffy grid to expose (min-viable: tracks + areas + span).
- **`ChromeCtx` shape** — defer until enough docks exist to justify (b) over (a).
- App-side (owned by the chrome plan, not here): dock metadata, the `AppState` namespace registry, the action ids.

---



---

# Part III — Planner mapping, gaps, and known contradictions

## 20. Every phase in the old plan, against the planner

Status is the planner's, read 2026-07-26.

| Old plan phase | Planner | Status |
|---|---|---|
| Phase 0 — finish the current refactor | none needed | **done** — everything built on it is complete |
| Phase 1 — architecture contracts | **F003/P005** (plugin-01) | done |
| Phase 2 — shared UI / chrome state layer | none needed | **built** — `heca/src/chrome/state.rs`, 1158 lines |
| Phase 3 — ChromeHost + region hosts | **F003/P006** (plugin-02) | done |
| Phase 4 — built-in provider system | **F003/P004** (plugin-03) | done |
| Phase 5 — workspaces container migration | **F003/P004** | done |
| Phase 6 — dynamic action registry | **F003/P003** (plugin-04) | done |
| Phase 7 — grid-ui widget expansion | **F004** | widgets all built; the open F004 phases hold what is left |
| Phase 7.5 — transparency and blur | **F005** (Compositor Frost) | base merged, tuning deferred |
| Phase 8 — overlay / modal / dropdown host APIs | **F003/P008** (plugin-05) | planned |
| Phase 8.1 — placeholder variables | **F003/P002** (plugin-06) | planned |
| Phase 8.2 — plugins from config.toml | **F003/P009** (plugin-07) | planned |
| Phase 9 — WASM plugin runtime | **F003/P001** (plugin-08) | planned |
| Phase 10 — multi-region proof providers | **F003/P007** (plugin-09) | planned |
| Phase 11 — config half | **F003/P007** | planned |
| Phase 11 — keybinding + palette half | **F003/P018** (plugin-10) | added 2026-07-26 |

Planner phases the old plan never knew about, because it stopped being updated:
**F003/P010** declarative action interaction · **F003/P012** context menu (done) ·
**F003/P013** shared list and menu navigation · **F003/P014** top-bar menu (stub) ·
**F003/P015** and **F003/P016** ViewNode across all widgets (both done) ·
**F003/P017** the UI-model gap work.

## 21. Where Part II is out of date

Part II's prose is transferred verbatim, so it still contains statements that are no longer true.
Each is listed here rather than edited, so the original wording stays readable. (Its *code examples*
are the one exception — see the note at the top of Part II.)

| In Part II | Why it is wrong now |
|---|---|
| "Styling is not a free prop… colors come from the Theme, never from raw values passed by the plugin" (§3, rule C) | **Reversed 2026-07-26.** The theme is the default; code may override it with a string. Part I R2. |
| "Behaviour is an Intent — never a closure" | Still true on the wire. A plugin in TypeScript may *write* a function; its kit stores it and sends a reference. Part I R4. |
| "Adding a `WidgetKind` is host-side work — the widget, a mapper arm, the docs" | Still true that plugins cannot invent widgets. It does **not** require a fixed enum — a host-filled registry satisfies the same rule in one step. Part I R5. |
| "Until a first-class `Table` exists, a table is composed" | Still true. **F003/P011/T007** deferred 2026-07-26: no consumer. |
| Collapsed icon rail, `RailCell` per item, rail flavours (§15) | **Dropped 2026-07-11.** A region is Expanded or Hidden. `RailCell` and `KeyHint` still ship; nothing mounts a rail. See the planner (F003/P020). |
| "G1–G8" task ids (§19 references them) | **Removed 2026-07-26.** All eight were built. Only G7 ever had a planner id (**P019(F004)**, gridui-01). |
| Open questions: icon font, `Grid` surface detail (§19) | Both shipped. `Icon` and `Grid` are live widgets. |
| `Column::new()` / `Row::new()` as the layout boxes | **Renamed 2026-07-27 (F003/P017/T006).** The vocabulary's boxes are `VStack` / `HStack`; `Row` is now the **clickable, selectable** widget it always was in `heca-grid-ui`. Examples in both parts use the new names. |
| `Label::new(..).variant(Variant::Heading)` (§3 SDK sketch) | **Never existed.** `Label` has no `variant` builder. A heading is `size: header` (`ViewSize::Header`). Corrected in place 2026-07-27. |
| `Variant::Danger` on a `Button` (§3 SDK sketches) | **Never existed.** The button vocabulary is `Primary`/`Secondary`/`Destructive`/`Outline`/`Ghost`/`Link`; the destructive one is `Destructive`. Corrected in place 2026-07-27. |
| "No dedicated `Panel` widget — a bare panel is a plain `Surface`" | **Built 2026-07-27 (F003/P017/T008).** `Panel` is its own widget with a real title. It was the reason the published panel example described something unbuildable. |
| A context menu's target carries host-resolved facts about a row (§4, `ContextTarget`) | **Replaced 2026-07-30 (F003/P086/T365).** A target names a row — `Row { container, key }` — and its component reads its own model for the facts. See §2.11. |
| Mode-restore: a menu opened from a non-Normal mode returns to it (§4 intro, `overlay_origin_mode`) | **Gone 2026-07-30.** Chrome focus is not a mode and an overlay does not take it away, so there is nothing to restore. See §2.11. |
| `ChromeCtx` carries the frame's render inputs `tree()` / `programs()` (§7, "The two halves of the seam") | **Removed 2026-07-30 (F003/P086/T367).** Both were one component's — a workspaces model and a pane-program catalog handed to every component through the contract they share. The context carries `theme()` and `emit_intent()`; a component's model lives on its own state. |
| Any "Status:" line anywhere in Part II | The planner is the record. Ignore them. |

## 22. Work that is designed here but was tracked nowhere

Found by auditing this file against the planner on 2026-07-26. Both now exist:

- **Dragging a container between regions** → **P079(F004)** (gridui-08). The framework ships, but
  `DragSurfaceId` has one variant (`LeftSidebar`), `DockFrame` has no drag handle, and
  `ChromeRegion` receives nothing. The plugin escape hatch (`DragItemKind::Custom`,
  `DragSurfaceId::Plugin`) is folded into the same phase.
- **Binding and listing contributed actions** → **F003/P018** (plugin-10). `ActionMeta` already
  carries `default_binding` and `describe_all()` already returns everything a palette needs.
  Nothing reads either, so a registered action can never be triggered by a key.
