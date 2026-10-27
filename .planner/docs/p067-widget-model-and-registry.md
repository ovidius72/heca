Serializable UI descriptions, typed authoring and the realization bridge. Existing code uses WidgetKind; P067(F003)/T398 now owns replacing that closed enum with a host-populated widget registry and eliminating Box/cast exposure from public component authoring APIs. Behavior remains Intent-based across serialized boundaries; native components retain their closures and live state.
---

# The declarative UI model

*Transferred from `docs/chrome-and-ui.md` Part I, updated 2026-08-11. Current, and it supersedes anything elsewhere that contradicts it. The architecture around it — the layers, the dependency rule, the settled questions — is in **F003's own description**.*

## Current implementation ownership

Approved 2026-10-25: P067 is the shared UI authoring foundation. P067/T517 moved here intact from P052 and owns generic template/part composition first. P067/T398 then owns registry/model migration, all public constructor/factory ergonomics and native/described acceptance, including standalone Terminal::new("shell") and binding. It was moved from P084, preserving its id/history; it is planned, not implemented. T245/T249/T251, T474, T501 and T502 remain completed foundations.

Canonical phase decision: 92371b2a-9cf6-4fb4-98ae-b10b223f468a. Public authoring signatures follow project decision 5c7d85c7-b0f5-4ed0-b8ca-2f06f5cbbad4: ordinary constructors/children, no exposed Box parameters/returns, manual boxing, casts or explicit type-erasure conversions. Runtime allocation remains internal.

T360's state/binding work stays with its reviewer. P094 owns terminal-header composition (T450), managed creation/lifecycle/restore consuming T398 (T452), float/input audits, second-content proof and picker convergence. P106/T550 owns multi-document tabs and retains its T452 task prerequisite. F012 owns library integration; P022/T075 owns actual loaded-plugin startup parity. T398 must not wait back on T452 or the whole WASM runtime. The project decision and P067/P094 verified handoffs carry the complete approved ownership/resume map.

## 0. What all of this is for — read this before the rules

**heca is meant to be extended by other people.** The target, written as ⭐⭐ RULE ZERO in `AGENTS.md`, is that someone writing a plugin — or contributing to the project — authors UI the way they would in **Flutter or SwiftUI**:

- a **declarative tree** of typed widgets, composed to any depth;
- **behaviour attached to the widget itself**, in one line, on the thing it belongs to;
- and **exactly what the app's own chrome gets** — the same widgets, the same keyboard picker, the same menus, the same focus and policy — with no host-private type, no registry to pre-register with, and no second-class path.

Everything below is downstream of that sentence. Use it as the test when a rule looks like ceremony:

> Write the line a **plugin author** would type. If getting the behaviour needs a registry, an id, a `pub(crate)` type, or a rule they must remember, **the API is the bug** — not their code.

Two things this rules out permanently, and both have been tried:

- **A registry parameter on the bridge.** `realize` once took a `HintTargets` sink so the leader-key picker could reach a described node; the native side used a *different*, host-private registry for the same feature. Two doors, and the plugin's was the poorer one — its rows could not say "focus my container first" and were refused by `ActionPolicy::ContainerFocused`. Deleted in F004/P084/T399: a node declares `peek` and the framework collects it out of the laid-out tree.
- **A host-private composite as the answer to "and also do X first".** `FocusPaneThenAction` / `FocusContainerThenAction` are `pub(crate)`, so a plugin cannot say them. If the behaviour is needed it gets a **name** a plugin can name, like `focus_pane` and `unfocus_dock` have.

## 1. What the model is

A screen is described as a tree of nodes:

```
ViewNode {
  kind:     which widget           // VStack, HStack, Row, Button, Input, Scroll, …
  props:    name → value           // padding, placeholder, variant, colour, …
  events:   name → Intent          // "press" → an action id + arguments
  children: [ ViewNode, … ]        // any depth
}
```

The diagram is conceptual: the current Rust field still uses WidgetKind until T398 lands. Its replacement is an open serializable registry identifier; public authors use typed constructors rather than manually supplying that tag.

The same tree works for the app's own screens, for anything sent in over RPC, and for a plugin. It is plain data, so it survives being turned into bytes.

`realize` turns a tree into real widgets. It is the only path from a description to a widget.

⚠️ **Moved 2026-07-27 (F003/P017/T009).** It lived in the app (`heca/src/chrome/realize.rs`) and the model beside it (`heca/src/chrome/view.rs`); anything that still says so is out of date. They are now two crates **below** `heca`: **`heca-view`** (the model — serde and nothing else, no widget library) and **`heca-view-realize`** (the bridge — it owns the `heca-grid-ui` dependency). A plugin can therefore name the vocabulary without compiling the renderer, and `heca-renderer`'s showcase — which is below the app — renders a described tree beside its hand-built twin.

**And there is a typed SDK on top of the model** (`heca-view/src/build/`, F003/P011/T006): one Rust type per widget kind, so an author writes `VStack::new().gap(8).child(Button::new("Restart"))` and the compiler refuses what the widget cannot do. It lowers to a `ViewNode` and adds no capability. **That is the surface a plugin author is meant to use.**

Two ways exist to build a screen and they produce the same widgets:

```rust
// Native components retain their own runtime behavior.
let native = heca_grid_ui::widgets::Button::destructive("Restart");

// Described authoring uses typed constructors too.
use heca_view::build::*;
use heca_view::{Intent, ViewVariant};
let described = heca_view::build::Button::new("Restart")
    .variant(ViewVariant::Destructive)
    .on_press(Intent::new("docker.restart").arg("id", id));
```

## 2. Who owns what

The model lives in heca-view (serialization-only); the bridge lives in heca-view-realize (depends on the model and grid-ui). The app and renderer consume those crates. heca-grid-ui stays independent of ViewNode and owns widgets, layout, painting and framework input/focus behavior.

The host supplies runtime services and bootstraps the widget registry. Described/WASM plugins send data and action references; native extensions may compose Component implementations directly. Neither path requires per-placement registry registration or public type-erasure ceremony.

Factories/metadata must reuse the existing native property surface. The concrete registration/discovery design belongs to T398 and must preserve these dependency boundaries.

## 3. The rules

### R1. A widget's own builders decide what a description can set

The list of settable properties is not written anywhere. Each widget publishes it, generated from its own builder methods:

```rust
#[props]
impl Input {
    #[prop] pub fn placeholder(mut self, s: impl Into<String>) -> Self { .. }
    #[host_only("behaviour crosses as a reference, never a function")]
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self { .. }
}
```

**Every builder must say which it is.** A builder that is neither `#[prop]` nor `#[host_only]` fails the build. Being left out quietly is exactly how `Input::placeholder` and `ScrollRegion`'s second axis stayed unreachable for months while both worked perfectly in the library.

A second check fails the build when a whole widget has no published list at all.

### R2. Appearance comes from the theme, and code can override it — CHANGED 2026-07-26

**Old rule, now dead:** "styling is not a property; you pass semantic intent and the host resolves the pixels."

**New rule:** the theme supplies the default. Code can override it, with a string, the same way it sets any other property.

```rust
.prop("color", PropValue::Color("#ff8800".into()))   // a literal
.prop("color", PropValue::Color("muted".into()))     // a theme name
```

Set nothing and you get the theme, which is what almost everything should do — a widget that does not override follows a theme reload. A literal colour does not follow a theme reload; that is the trade for control, and it is the author's choice to make.

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

`ViewOrientation`, `ViewScrollAxes`, `ViewSeverity` (which serves both toast severity and alert variant), `ViewLabelSide`, `ViewMarker`, `ViewTextAlign`, and `ViewGlyph` for the icon names. They **do not** get a `PropValue` variant and neither should the next one — a fixed set travels as its name, so `Text` already carries it and the type belongs in the authoring layer. `Size`, `Variant` and `Align` predate that rule. `ViewGlyph` is generated from `heca_grid_ui::Glyph` and guarded by a test that fails when an icon is added on one side only (F003/P011/T019).

**This did not work until 2026-07-27 (F003/P011/T018), and the doc said it did.** `border` and `glow` are structs; a property value could only be a scalar, so neither could be written from a description — whatever serde derives the types carried. Adding serde to a type is not the same as having a value that can carry it, and because nothing tested the two, nothing failed. `Map` is the extension point: a future struct-shaped property needs no new variant.

### R3. Order never matters

Properties are applied after children are attached. That removes the only real dependency there was — `Select` and `Tabs` clamp `selected` against the number of options, so they need the options to exist first.

Nobody sequences anything: not an author, not a caller, not an agent. `#[prop]` **rejects any argument**, so a hint like `late` or `after = "x"` cannot be reintroduced one widget at a time. If two properties on a widget ever do depend on each other, that widget's setters are the bug.

### R4. Behaviour crosses as a reference, not a function

A plugin is a separate program. You cannot send it a function — there is nothing to send. So you send a **reference**: a name the other side can look up. That is what `Intent` is.

For a plugin written in TypeScript or another language, the plugin kit hides this. The author writes a function; the kit stores it, invents a reference, and puts the reference on the wire:

```ts
Button({ text: "Restart", onClick: () => restart(id) })
// the kit stores the function and sends a reference; the author never sees it
```

One real limit either way: the function runs when the message arrives, not while drawing.

The app's own code has no boundary to cross, so it passes real functions directly through the builder API. Same widgets, same result.

### R5. An open host-populated widget registry replaces the closed enum

A described plugin composes widgets whose implementations are available to the host. This is a runtime availability constraint, not a requirement for a fixed enum. Native compiled components remain open through the existing Component contract.

P067(F003)/T398 replaces WidgetKind, the central widget-type realization match and ALL lists with a host-populated registry. A single library-side definition supplies a registered widget's identity, factory and metadata; authors keep typed constructors and ordinary children. Adding a supported widget must not mean editing three parallel closed inventories.

Registration belongs to library/bootstrap wiring, never to a caller placing a component. The exact serializable identifier, discovery mechanism, public return types and compatibility plan are design deliverables of T398. Existing built-in spellings and data/event semantics must be preserved through an explicit migration.

### R6. Overlays are requested, not mounted

A modal, dropdown or context menu is not something a plugin puts in its tree. It asks the host and waits for a typed answer. The host owns stacking, focus trapping, Escape, click-outside and placement.

Panels, toolbars and status segments go **into** a region. Modals and dropdowns are **requested**.

### R7. Layout and appearance are separate types

```rust
pub struct Style {
    pub layout: Layout,   // arrangement + the size variant
    pub visual: Visual,   // fill, border, glow, radius, font size and scale
}
```

Two peers. Layout can be read and written as data; appearance is the same under R2. Grouping them separately keeps "what the caller asked for" apart from "what the theme decided", which stays useful even now that both are settable.

### R8. Sizing follows CSS flexbox — unset means stretch

Set no width and a node **fills its parent across the cross axis**, exactly as CSS `align-items: stretch` does. A panel with no width in a 600px column is 600px wide, and the row inside it fills the panel's content box in turn.

```rust
build::Panel::new()                    // fills the parent
build::Panel::new().width_pct(1.0)     // the same thing, said out loud
build::Panel::new().width_pct(0.5)     // half — what a percentage is actually for
build::Panel::new().width(240.0)       // fixed; this is what keeps two panels side by side
```

A percentage travels as `"100%"` / `"50%"`, which is one of the three spellings `Length` accepts (a number is px, `"auto"` is content-sized).

### R9. Live state needs a described bridge, not a permanent widget exclusion

Closures and Rust Signal objects do not travel directly as serialized data. A described capability instead needs a defined data/action/binding seam. Missing that seam is an access gap to identify; it is not a permanent reason to classify an entire generic widget as host-only.

T360 owns reactive values and bindings and is under another agent's review. T398 consumes that settled contract and audits remaining adapters without inventing a second signal store or changing T360. A builder binding an externally-owned signal is a different question from a widget with framework-managed internal state or an initial literal value.

## 4. Capability access must be proved per operation

The former hand census of settable/host-only builders is obsolete. T398 checks the current exported capabilities and their usable public paths rather than preserving those historical counts.

- A function crosses the described boundary as an action/reference through the existing behavior seam.
- Composed content uses ordinary children; it is not a reason to exclude a whole widget.
- A no-value convenience builder can map to an explicit property value.
- Live values need the existing owner's binding seam; absent adapters are recorded as concrete gaps.
- A multi-value builder needs an explicit supported data shape and validation.
- Public constructors/children and their return types must meet the project's Box/cast-free contract.

A registry entry, an allowlist reason or a passing enum census alone does not prove that a capability works. Input, layout, data updates and lifecycle must be verified where they are part of the capability.

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

`Panel` is a **titled section container** — a plugin's slice of a region. It is quiet by default (no frame of its own, lighter padding than a `Card`); give it a `fill` or a `border` when it should stand apart. The heading used to be fiction: `WidgetKind::Panel` realized to a plain `Surface`, which has no title, so this example described something that could not be built. F003/P017/T008 made `Panel` a real widget.

### A table

There is no `Table` widget. A table is built from the pieces, and this is not a workaround — composing is the model:

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

Two things in this example only became writable as a description recently, and both were wrong here for a while:

- **The rule** between the header and the body is a real widget, so it takes its colour and thickness from the theme. It reached the vocabulary in F003/P017/T5; before that a plugin had to fake the line with a thin sized `Surface` that hardcoded both.
- **The clickable row.** `Row` used to name the plain horizontal box, so this example described behaviour — press, selection, hover — against a kind that had none of it, and no reader could tell. F003/P017/T6 renamed the boxes to `VStack` / `HStack` and gave `Row` to the interactive widget it always meant in `heca-grid-ui`. A `Row` with a press intent is focusable, activates on click and on Enter/Space, and is reachable by `prefix+/` like any other actionable node.

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

The result carries the chosen action **and** whatever the form collected. A widget with a `name` joins that data.

### Menus and keyboard hints

A context menu is a host-owned dropdown you attach to a node; the host returns the chosen entry as an intent.

You never build a keyboard hint. **Any node with a press intent is automatically reachable by the leader key** (`prefix+/`): the host assigns the letters, draws them and runs the pick.

**And a pick is not a click** (F004/P084/T399). They are different gestures and a node may answer them differently — heca's own sidebar row activates the pane and *leaves* the sidebar on a click, and stays in it on a peek. Bind `peek` when they differ; leave it unbound and a pick does what a press does:

```rust
Row::new()
    .on_press(intent("docker.select", { "id": id }))   // go there
    .on_peek(intent("docker.reveal", { "id": id }))    // look at it, stay where I am
```

*(`.hintable(false)` was once written here as the opt-out and never existed. There is nothing to opt out of: a node with neither `press` nor `peek` is not a pick target.)*

**And a gesture names the seating it was declared in.** A widget built inside a mounted container carries that mount on the intent it emits, so the same container mounted twice has rows that each answer for themselves — with two docks on screen, `prefix+/` lights a letter per row *per dock*, and picking one puts the cursor and the keyboard in that dock. `owning_mount` resolves only calls with no element behind them: a keybinding, a palette entry, RPC without `--dock`. (F004/P084/T399.)

## 6. Values

- Numbers, text and true/false as themselves.
- Enums as their **name**, lower case with underscores: `"space_between"`, `"small"`, `"both"`. The enum's own variants are the accepted list, so adding a variant accepts it with no list to update anywhere.
- A size as a plain number (pixels), `"auto"`, or a percentage string like `"50%"`.
- A colour as `"#rrggbb"`, `"#rrggbbaa"`, or a theme name.

Anything the model does not understand is ignored and the widget keeps its own default. A single bad value costs only itself — the good properties on the same node still apply. The model is untrusted input and is treated that way.

## 7. Still open

- **Two-value builders** (`Overlay::panel_size(width, height)`). Either split them or let a property carry a pair.
- **Replace the fixed widget list with the host-populated registry** (R5). Planned and owned by P067(F003)/T398; not implemented.
- ~~**Where the plugin-facing types live.**~~ **Done, F003/P017/T009 (2026-07-27).** The model is `heca-view` and the bridge is `heca-view-realize`, both below the app; a Rust plugin author depends on `heca-view` alone. The typed SDK on top of it (`heca-view/src/build/`, F003/P011/T006) is what gives the editor help this row was asking for.
- **Generated type definitions** for plugins in other languages, from the same widget list. F003/P001/T006. **This is the gap between "a Rust plugin can do this" and "anyone can write a plugin"** — the Rust author is served, a JS or Python author is not.
- **Residual native/described capability parity.** Included in P067(F003)/T398 as acceptance of the registry migration. Generic widgets are not excluded wholesale; specific live-state prerequisites retain their existing owner.
- **Nothing loads a plugin yet.** F003/P022 (WASM runtime). Everything above is the authoring model, proven by the app and the showcase authoring against it — not by a third party.

## 8. Decisions, with dates

| Date | Decision |
|---|---|
| 2026-07-06 | The description model exists and covers the whole widget library, not just plugins. |
| 2026-07-06 | Behaviour crosses as an action id, never a function, so the tree stays sendable. |
| 2026-07-25 | Layout and appearance become separate types. |
| 2026-07-25 | Layout properties are read from the layout type itself — no list in the app. |
| 2026-07-26 | Widget properties are generated from each widget's builders; every builder must declare whether a description can set it. |
| 2026-07-26 | Order never matters, and the marker that would have reintroduced it is rejected at compile time. |
| 2026-07-26 | **Appearance is overridable.** The theme is the default, not a wall. Replaces the old "styling is not a property" rule. |
| 2026-07-26 | A plugin written in another language may use functions; the plugin kit turns them into references. No change to the boundary. |
| 2026-07-26 | The closed widget list is about plugins not inventing widgets. It does not require a fixed enum — a host-filled registry satisfies it. |
| 2026-07-27 | **The model and the bridge move below the app** (`heca-view`, `heca-view-realize`, F003/P017/T009), so a plugin can depend on the vocabulary without the renderer, and anything that can build widgets can render a description. |
| 2026-07-30 | A context target **names** a row (container + `nav_key`); the component that wrote the key resolves it. The host enumerates no row kinds. (P086) |
| 2026-07-30 | A row's click, double-click and right-click are **named intents**, declared per item kind — never closures — so click, picker, menu, key and RPC are one path. (P086) |
| 2026-08-07 | ⭐⭐ **RULE ZERO**: a capability is **one builder on the widget**. If getting it needs a registry, an id or a `pub(crate)` type, the API is the bug. Outranks the architecture rules. |
| 2026-08-11 | **A pick is not a click.** `peek` is its own event, defaulting to `press`. The hint registry — host-side *and* the `HintTargets` sink `realize` took — is deleted; a node declares what a pick does and the framework collects it out of the laid-out tree. (F004/P084/T399) |
| 2026-08-11 | **A gesture names the seating it was declared in**, so the same container mounted twice has rows that each answer for themselves; nothing is resolved back to an instance. (F004/P084/T399) |
