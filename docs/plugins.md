# Writing a plugin for heca

> **Laying things out — [`layout.md`](layout.md).** Every size, space, track and alignment
> spelling below is the one the app's own builders take; that guide is the full reference.


**The plugin author's manual.** Everything here is about the interface *you* write and what the host
does with it. The chrome's own architecture — how the sidebar, the regions, the overlays and the
containers are built — is not your business and lives in
[`chrome-and-ui.md`](chrome-and-ui.md).

> **This file says how it works, not what is built.** Where something is described as missing or
> planned, the planner is the only record of whether it is still true.

| You want | Go to |
|---|--- |
| what a plugin can put on screen, and who owns each shape | [the map](#the-plugin-story-on-one-page) |
| why any of this is shaped the way it is — read before the rules | [§0](#0-what-all-of-this-is-for--read-this-before-the-rules) |
| the model: nodes, props, events, children | [§1](#1-what-the-model-is) |
| the rules an author actually trips over | [§3](#3-the-rules) |
| what a description cannot set today, and why | [§4](#4-what-a-description-cannot-set-today-and-why) |
| worked screens — a panel, a table, a modal with a form, menus and hints | [§5](#5-writing-a-screen) |
| values: sizes, spaces, colours, enums | [§6](#6-values) — and the full references are [`widgets.md` → Writing a size](widgets.md#writing-a-size--one-vocabulary-native-and-described-alike) and [→ Writing a space](widgets.md#writing-a-space) |
| every widget's own properties and events | [`widgets.md`](widgets.md) |
| a plugin that is only a config file | [the section at the end of the map](#the-plugin-that-needs-no-code-at-all) |
---

## The plugin story on one page

*A **map of the pieces and what each one is for**, not a status board.*

**Where this is going:** a plugin author writes their interface the way
they would in Flutter or SwiftUI — a tree of typed widgets, composed to any depth, behaviour
attached to the widget — and gets **exactly what the app's own chrome gets**: the same widgets, the
same `prefix+/` picker, the same menus, focus and policy. *If a plugin ends up with a poorer version
of a shipped feature, that phase has failed regardless of what loads.*

### The three ways a plugin puts something on screen

Decided in §2.7 / R6 below, and it predates the one-tree work:

| shape | who owns it |
|---|---|
| **Requested** — a modal or dropdown that asks a question and returns a typed answer | the host owns stacking, focus trap, Escape, click-outside, placement |
| **Contributed into a region** — a panel, toolbar or status segment seated in the chrome | the plugin offers, the host mounts |
| **Its own named surface** — a panel of its own design, addressed by name and opened by an action, the way `heca.expose` is | the plugin offers, the host attaches |

A plugin never puts a surface in the tree itself. It offers one, exactly as it offers a container.

### Adding a dock to a region

A region — a sidebar, a bar — is a **place reached by name that holds a list**. A plugin adds to it
with one line, and never builds, finds or registers anything:

```rust
regions("sidebar.left").append(Docker::new("docker"));     // at the end
regions("sidebar.left").prepend(Outline::new("outline"));  // at the start
regions("sidebar.right").remove("workspaces");             // take out a built-in, by name
regions("sidebar.left").retain(|id| id != "notes");        // keep only what you say yes to
regions("sidebar.left").gap("md");                         // air between the docks
```

| names | `"sidebar.left"`, `"sidebar.right"`, `"bar.top"`, `"bar.bottom"` (the older `"left-sidebar"` / `"left"` also read) |
|---|---|
| **size** | said by the dock, on the body it builds: `.flex(3.0)` — or by whoever adds it: `.append(Docker::new("d").flex(3.0))`. See [`layout.md` → Shares](layout.md#shares) |
| **position** | the order it was added, or `.order(..)` on its body, or by whoever adds it: `.append(Docker::new("d").order(-1))`. See [`layout.md` → Order](layout.md#order) |
| **who wins** | whoever **adds** the dock, over what the dock's own body says — like a style written where an element is used beating the one its component shipped with |
| a name nobody knows | said out loud, lists the real ones, and changes nothing |
| a call after startup | said out loud and dropped. Regions are set up once, as the app starts |

**Why `retain` and not `filter`.** In Rust, `filter` builds a new list and leaves the old one as it
was; `retain` changes the list in place — the containers it says no to leave the region. That is
what this does, so it takes the name Rust already uses for it (`Vec::retain`).

**The region holds no sizes and no indexes.** It is a list anyone may append to: a size list written
there would be wrong the moment one more dock arrived, and an index would shift under everyone after
a plugin's insert with nothing failing. The ends are stable, and each dock carries its own size and,
if it needs one, its own place.

### What a named surface inherits, and therefore does not have to build

Only *attaching* was ever missing. Everything after it already works, and the exposé is the proof:

- **Opening** is an ordinary action carrying the surface's name — `toggle_layer { name = "heca.expose" }`,
  bound in `keybindings.default.toml`. Rebindable by the user, and reachable from the palette and RPC,
  which is the project rule that a capability must not be trapped behind one surface.
- **Its own keys** are declared under its name in a `[[keys.surface]]` block, consulted *before* the
  global map.
- **The way out** is free and cannot be removed: the `layer` floor binds Escape — asserted,
  unremovable — plus `q` / `Ctrl+q` to `close_overlay`, declared once for every layer rather than
  copied per surface. Those keys do not exist while no layer is up, so `:q` still quits vim.
- **Input, hint letters, layout, paint and z-order** follow from being a node in the one tree. Z is tree position; a surface passes the pointer through where it covers nothing.

### How a plugin writes what goes inside

**Built, and the answer to "surely it isn't hand-written JSON":**

```
heca-view the MODEL — serde and nothing else. A plugin depends on this alone.
heca-view/src/build.rs the TYPED SDK — one Rust type per kind, so the compiler refuses what the
                        widget cannot do. This is what an author writes.
heca-view-realize the ONE bridge — realize(&ViewNode, theme, emit, forms), below `heca`.
```

The showcase renders a described tree beside its hand-built twin, which is what proves the bridge
does not need the app.

**Why it crosses as data at all** — not a limit of WASM, but of any sandbox: a plugin cannot hand us
a closure, a signal or a pointer to a live widget, because those mean something only inside our
memory. So the interface crosses as a description and a press comes back as an `Intent` — a name and
arguments. The consequence to design around: our own chrome updates in place through signals, while
a plugin's change is a new description. Fine for a panel or a dialog; not for something changing
many times a second, which stays ours.

### What is missing, and where it is tracked

- **Other languages.** Generated type definitions from the same closed `WidgetKind::ALL` list. Today a Rust author gets editor help and a JavaScript or Python author gets
  nothing. That is the gap between "a plugin *can* do this" and "anyone can write a plugin".
- **The vocabulary's own holes.** 13 of 46 widgets have no described form, and a context menu has no
  described declaration. ⚠️ A menu is a **declaration
  attached to a node**, the way a press or a hint is, never a widget kind an author assembles and
  positions. The native side already proves the shape — one builder on any widget, no id, no path,
  no anchor.
- **The proof.** Rebuild one of our own overlays using only what a plugin can write. ⚠️ Its text still names `LayerContent`; read it as
  "the described path". Until this lands, "a plugin can add an overlay" is an intention, not a fact.
- **No host-only exceptions.** The rule is compile-enforced per builder; the audit proving the
  host-only list is *only* the legitimate cases has not been done.
- **The boundary itself.** Loading someone else's code, the event bus, region contributions and
  action registration across it — , which says outright it has no real spec yet and needs
  a discovery pass before it can be planned. It is the least defined thing in the feature.

### The plugin that needs no code at all

 — plugins from `config.toml`. A large share of plugins add a command that runs a
program, a panel in a region, a menu entry and a key. None of that is logic, and none of it should
need a sandbox or a compiler: it is a text file declaring what exists and where it appears, in the
same shapes our own defaults already use, merged by the same per-key rules so a user can override
any of it. The compiled part joins only when there is behaviour to run.

Two things follow from doing it this way. Declaring an action in that file makes it reachable by key,
by mouse and by script at once, so a plugin cannot accidentally trap a feature behind its own button.
And everything declared is known *before* the plugin is loaded — the palette can list it and the
keymap can bind it without running anything.

---

# Part I — The declarative UI model

*This is the current model, and it supersedes anything in
[`chrome-and-ui.md`](chrome-and-ui.md) that contradicts it.*

## 0. What all of this is for — read this before the rules

**heca is meant to be extended by other people.** The target, stated by Antonio and written as
⭐⭐ RULE ZERO in `AGENTS.md`, is that someone writing a plugin — or contributing to the project —
authors UI the way they would in **Flutter or SwiftUI**:

- a **declarative tree** of typed widgets, composed to any depth;
- **behaviour attached to the widget itself**, in one line, on the thing it belongs to;
- and **exactly what the app's own chrome gets** — the same widgets, the same keyboard picker, the
  same menus, the same focus and policy — with no host-private type, no registry to pre-register
  with, and no second-class path.

Everything in this file is downstream of that sentence. Use it as the test when a rule looks like
ceremony:

> Write the line a **plugin author** would type. If getting the behaviour needs a registry, an id, a
> `pub(crate)` type, or a rule they must remember, **the API is the bug** — not their code.

Two things this rules out permanently, and they have both been tried:

- **A registry parameter on the bridge.** `realize` once took a `HintTargets` sink so the leader-key
  picker could reach a described node; the native side used a *different*, host-private registry for
  the same feature. Two doors, and the plugin's was the poorer one: a
  node declares `hint` and the framework collects it out of the laid-out tree.
- **A host-private composite as the answer to "and also do X first".** `FocusPaneThenAction` /
  `FocusContainerThenAction` are `pub(crate)`, so a plugin cannot say them. If the behaviour is
  needed it gets a **name** a plugin can name, like `focus_pane` and `unfocus_dock` have.


## 1. What the model is

A screen is described as a tree of nodes:

```
ViewNode {
  kind: which widget           // Column, Row, Button, Input, Scroll, …
  props: name → value           // padding, placeholder, variant, colour, …
  events: name → Intent          // "press" → an action id + arguments
  children: [ ViewNode, … ]        // any depth
}
```

The same tree works for the app's own screens, for anything sent in over RPC, and for a plugin.
It is plain data, so it survives being turned into bytes.

`realize` turns a tree into real widgets. It is the only path from a description to a widget.

⚠️ **It moved below the app.** It lived in the app (`heca/src/chrome/realize.rs`) and the
model beside it (`heca/src/chrome/view.rs`); everything below that says so is out of date. They are
now two crates **below** `heca`: **`heca-view`** (the model — serde and nothing else, no widget
library) and **`heca-view-realize`** (the bridge — it owns the `heca-grid-ui` dependency). A plugin
can therefore name the vocabulary without compiling the renderer, and `heca-renderer`'s showcase —
which is below the app — renders a described tree beside its hand-built twin.

**And there is a typed SDK on top of the model** (`heca-view/src/build.rs`): one
Rust type per widget kind, so an author writes `VStack::new().gap(8).child(Button::new("Restart"))`
and the compiler refuses what the widget cannot do. It lowers to a `ViewNode` and adds no
capability. **That is the surface a plugin author is meant to use** — see §0 below.

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
heca (the app)  ──depends on──▶ heca-grid-ui (the widget library)
```

Never the other way round. The library does not know that plugins, descriptions or `ViewNode`
exist. It only knows widgets.

| Owned by the app | Owned by the library | Owned by the plugin |
|---|---|--- |
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

### R2. Appearance comes from the theme, and code can override it

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
guarded by a test that fails when an icon is added on one side only.

**This was broken once, while the doc said it worked.** `border` and
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

A percentage travels as `"100%"` / `"50%"`, which is one of the spellings `Length` accepts (a number
is px, `"200px"` the same, `"auto"` content-sized). **They are not a plugin dialect** — one parser
reads them, and the app's own builders take the identical strings, so what you write is what native
code gets. Full reference, including the `"50"`-is-pixels trap:
[`docs/widgets.md` → Writing a size](widgets.md).

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
|---|---|--- |
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
which has no title, so this example described something that could not be built. 
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
  thickness from the theme. It reached the vocabulary in ; before that a plugin had to
  fake the line with a thin sized `Surface` that hardcoded both.
- **The clickable row.** `Row` used to name the plain horizontal box, so this example described
  behaviour — press, selection, hover — against a kind that had none of it, and no reader could
  tell. renamed the boxes to `VStack` / `HStack` and gave `Row` to the interactive
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
leader key** (`prefix+/`): the host assigns the letters, draws them and runs the pick.

**And a pick is not a click**. They are different gestures and a node may answer
them differently — heca's own sidebar row activates the pane and *leaves* the sidebar on a click,
and stays in it on a hint. Bind `hint` when they differ; leave it unbound and a pick does what a
press does:

```rust
Row::new()
    .on_press(intent("docker.select", { "id": id }))   // go there
    .on_hint(intent("docker.reveal", { "id": id }))    // look at it, stay where I am
```

**`.hintable(false)` is the opt-out**. Anything you can act on — a click, a double
click, a key — wears a letter with nothing declared, so the only thing left to say is "not me":

```rust
Button::new("×").hintable(false)   // a close button on every row would eat a letter each
```

`hintable(true)` is the default and changes nothing on a widget nobody can act on: there would be
nothing for the letter to run.

> **Correction.** This paragraph used to say `.hintable(false)` *"was written here as the
> opt-out and never existed. There is nothing to opt out of."* That was wrong, and how the decision
> was lost the first time: automatic-plus-opt-out was the original documented design (quoted in
> `BACKLOG.md:1490`), the native side only ever implemented explicit opt-in, and a later session
> rewrote **the documentation to match the code** rather than the other way round. When the code and
> a decision disagree, the code is what changes. Full record: `docs/hint-architecture.md` § 2.

---

## 6. Values

- Numbers, text and true/false as themselves.
- Enums as their **name**, lower case with underscores: `"space_between"`, `"small"`, `"both"`.
  The enum's own variants are the accepted list, so adding a variant accepts it with no list to
  update anywhere.
- A size as a plain number (pixels), `"auto"`, or a percentage string like `"50%"`.
- A space — `gap`, `padding`, `margin`, and their per-axis and per-side forms — as a plain number
  (pixels), `"8px"`, or a **step of the theme's rhythm** by name: `"none"` / `"xs"` / `"sm"` /
  `"md"` / `"lg"`.

  **Prefer the step.** It is a fraction of the inherited font, resolved when the tree is laid out,
  so your panel breathes like the rest of the app and follows a font, theme or zoom change with
  nothing rewritten. A pixel count is tuned for one font size and wrong at every other. Use the
  steps to group — a tight `"xs"` inside a label-and-control couple, a roomier `"md"` between
  couples — which is a form layout with no arithmetic.

  One parser reads these and the app's own builders take the identical spellings, so what you write
  is what native code gets. They are **not a plugin dialect**. Full reference:
  [`widgets.md` → Writing a space](widgets.md#writing-a-space).

  ⚠️ The retired names `gap_spacing`, `pad_spacing_x` and `pad_spacing_y` are still read so older
  trees keep working. Nothing writes them; do not use them in anything new.
- A **grid track** as `"auto"`, `"1fr"`, `"200px"`, a bare number (pixels), `"min-content"` /
  `"max-content"`, or `"repeat(3, 1fr)"`, in the `columns` and `rows` lists of a `Grid`. A child
  places itself with `area`, or `col`/`row` plus `col_span`/`row_span` — properties of the **child**,
  as CSS has them, resolved against the grid that holds it. Same rule as everything above:
  one parser, and the app's own `Grid::template("auto 1fr")` reads the identical words — the
  vocabulary used to live only on this side, which meant native code could not say `"1fr"` at all.
- A **layout keyword** — `direction`, `justify`, `align` — as the word a stylesheet would write:
  `"row"` / `"column"`, `"start"` / `"center"` / `"end"` / `"space-between"` / `"space-around"` /
  `"space-evenly"`, `"stretch"` / `"baseline"`. A hyphen and an underscore are the same character
  here, so `"space-between"` and `"space_between"` both land.
- A colour as `"#rrggbb"`, `"#rrggbbaa"`, or a theme name.
- `accent` — **the accent this node and everything inside it paints its chrome with**: focus rings,
  hover fills, selected washes, scrollbar thumbs, a caret. A panel with a hue of its own sets it
  once and every control inside follows, with nothing told twice.

  ```jsonc
  { "kind": "surface", "props": { "accent": "danger" },
    "children": [ { "kind": "button", "props": { "text": "Delete" } } ] }
  ```

  **The theme is always first**: set nothing and everything reads the theme's accent. The order is
  the node's own → the nearest ancestor that set one → the theme. Prefer a **theme name** over a
  literal, as with every colour here: a name follows a theme reload, a literal does not.

  ⚠️ It does not redefine a declared meaning — a badge's `accent` variant, an alert's `info`, a
  destructive button. Those keep the colour their variant names.

Anything the model does not understand is ignored and the widget keeps its own default. A single
bad value costs only itself — the good properties on the same node still apply. The model is
untrusted input and is treated that way.

---

## 7. Still open

- **Two-value builders** (`Overlay::panel_size(width, height)`). Either split them or let a
  property carry a pair.
- **A registry instead of the fixed widget list** (R5). Allowed by the rules; not built.
- ~~**Where the plugin-facing types live.**~~ **Done.** The model is
  `heca-view` and the bridge is `heca-view-realize`, both below the app; a Rust plugin author
  depends on `heca-view` alone. The typed SDK on top of it (`heca-view/src/build.rs`,
  ) is what gives the editor help this row was asking for.
- **Generated type definitions** for plugins in other languages, from the same widget list. **This is the gap between "a Rust plugin can do this" and "anyone
  can write a plugin"** — the Rust author is served, a JS or Python author is not.
- **No host-only exceptions in the vocabulary.** : the rule that a widget driven by
  a live host signal is host-only (R9) is written and enforced per builder; the audit proving the
  host-only list is *only* those is not done.
- **Nothing loads a plugin yet.** (WASM runtime). Everything above is the authoring
  model, proven by the app and the showcase authoring against it — not by a third party.

---

## 8. Settled decisions

| Decision |
|---|
| The description model exists and covers the whole widget library, not just plugins. |
| Behaviour crosses as an action id, never a function, so the tree stays sendable. |
| Layout and appearance become separate types. |
| Layout properties are read from the layout type itself — no list in the app. |
| Widget properties are generated from each widget's builders; every builder must declare whether a description can set it. |
| Order never matters, and the marker that would have reintroduced it is rejected at compile time. |
| **Appearance is overridable.** The theme is the default, not a wall. Replaces the old "styling is not a property" rule in `AGENTS.md` and in the chrome plan §2.6.1 rule C. |
| A plugin written in another language may use functions; the plugin kit turns them into references. No change to the boundary. |
| The closed widget list is about plugins not inventing widgets. It does not require a fixed enum — a host-filled registry satisfies it. |
| A context target **names** a row (container + `nav_key`); the component that wrote the key resolves it. The host enumerates no row kinds. (§2.11) |
| A row's click, double-click and right-click are **named intents**, declared per item kind — never closures — so click, picker, menu, key and RPC are one path. (§2.11) |
| **The model and the bridge move below the app** (`heca-view`, `heca-view-realize`), so a plugin can depend on the vocabulary without the renderer, and anything that can build widgets can render a description. |
| ⭐⭐ **RULE ZERO**: a capability is **one builder on the widget**. If getting it needs a registry, an id or a `pub(crate)` type, the API is the bug. Outranks the architecture rules. (`AGENTS.md`) |
| **A pick is not a click.** `hint` is its own event, defaulting to `press`. The hint registry — host-side *and* the `HintTargets` sink `realize` took — is deleted; a node declares what a pick does and the framework collects it out of the laid-out tree. |
| **A gesture names the seating it was declared in.** A widget built inside a mounted container carries that mount on the intent it emits, so the same container seated twice has rows that each answer for themselves — nothing is resolved back to an instance. `owning_mount` answers only for a call with no element behind it (a keybinding, a palette entry, RPC without `--dock`). |
