# Plugin Authoring (ViewNode)

> **STATUS: DESIGN — target Phase 9 (`plugin-08`). NOT yet available.**
> This document describes the *planned* plugin authoring model so the API we ship
> stays compatible with it. None of the SDK types below exist yet; the code
> samples are illustrative pseudo-Rust of the host SDK, not a working API. Track
> progress under the `plugin-ui` and `plugin-08` phases in `BACKLOG.md`.
> Source of truth for the design: `pluggable-chrome-plugin-plan.md` §2.6–2.7, §3.

## Mental model

A heca plugin never draws pixels and never hands the host a native widget object.
It does three things:

1. **observes** app state (events + read selectors),
2. **contributes UI** as a declarative **`ViewNode`** tree, and
3. **dispatches intents** (string action id + args) to change state.

The **host** owns everything visual: it maps your `ViewNode` to real
`heca-grid-ui` widgets, resolves colors/fonts from the active **Theme**, and owns
focus, clipping, and overlay z-order. You describe *what*; the host decides *how*.

- A **`Provider`** is the unit that mounts into a chrome region (left/right
  sidebar, top/bottom bar). Built-in providers are Rust; WASM plugins are reached
  through a host-owned `WasmProviderAdapter` that implements the same `Provider`
  trait — so everything below is identical for both.
- A **`ViewNode`** is a recursive widget tree (like Flutter's `Widget` or
  SwiftUI's `View`): a *container* node holds a **vector of child widgets**, each
  of which may itself be a container.

```
ViewNode {
  kind:     WidgetKind,             // Column|Row|Grid|Card|Scroll|Panel (containers)
                                    //  · Label|Button|Badge|Icon|Input|Toggle|StatusDot|… (leaves)
  props:    PropMap,                // serializable data + semantic enums (Variant, WidgetSize)
  events:   { on_press: Intent, …}, // Intent = (action_id, args) — NOT a native callback
  children: [ ViewNode, … ],        // recursive; empty for leaves
}
```

**Two rules that never change:**

- **Styling is not a prop.** You pass *semantic intent* (`Variant::Danger`,
  `WidgetSize::Small`); the host resolves the actual pixels from the Theme. You
  cannot pass raw colors/fonts. This keeps every plugin visually consistent.
- **Interaction is an intent, not a callback.** `on_press` carries a string action
  id + serializable args. The host routes it (dispatch a registered action, or
  deliver it back to you as an event). No plugin code runs during paint.

---

## Example 1 — a simple panel

A provider that mounts a small panel with a label and a button into the right
sidebar.

```rust
struct HelloProvider;

impl Provider for HelloProvider {
    fn id(&self) -> &str { "example.hello" }
    fn title(&self) -> &str { "Hello" }
    fn supported_regions(&self) -> RegionSet { RegionSet::sidebars() }
    fn default_region(&self) -> RegionId { RegionId::RightSidebar }

    fn build_contribution(&self, ctx: &ChromeCtx) -> Contribution {
        let name = ctx.state().active_pane_title().unwrap_or_default();

        Contribution::container("example.hello", "Hello",
            Panel::new()
                .title("Hello")
                .child(Column::new().gap(6).padding(10)
                    .child(Label::new(format!("Active pane: {name}")))
                    .child(Button::new("Refresh")
                        .variant(Variant::Accent)
                        .size(WidgetSize::Small)
                        .on_press(intent("example.hello.refresh", {})))))
    }

    // React to app events; the returned handles are kept alive while mounted.
    fn on_activate(&mut self, ctx: &ChromeCtx) -> ProviderHandles {
        let mut h = ProviderHandles::default();
        h.keep(ctx.on("pane.active.changed", |_e| { /* mark dirty → host re-builds */ }));
        h
    }
}
```

---

## Example 2 — a complex widget (a table)

A container is just a `ViewNode` with children, so you compose arbitrarily. Here a
Docker-style list. (Until a first-class `Table` widget lands, a table is composed
from `Grid`/`Row`/`Label`/`Badge` — see `plugin-task-ui-5`.)

```rust
fn containers_table(rows: &[ContainerRow]) -> ViewNode {
    let header = Row::new().gap(12)
        .child(Label::new("Name").variant(Variant::Muted))
        .child(Label::new("Status").variant(Variant::Muted))
        .child(Label::new("CPU").variant(Variant::Muted));

    let body = rows.iter().fold(Column::new().gap(2), |col, r| {
        col.child(Row::new().gap(12)
            .child(Label::new(&r.name))
            .child(Badge::new(&r.status).variant(status_variant(&r.status)))
            .child(Label::new(&r.cpu))
            // whole-row intent: selecting a row dispatches with the row id
            .on_press(intent("plugin.docker.select", { "id": &r.id })))
    });

    Scroll::vertical().child(
        Column::new().gap(6)
            .child(header)
            .child(Separator::new())
            .child(body))
}
```

`status_variant` returns a **semantic** `Variant` (e.g. `Accent` for running,
`Danger` for exited); the host maps it to Theme colors.

---

## Example 3 — a modal with a form

Overlays are **host-owned**: you *request* one and *await* a typed result. The
modal `body` is a full `ViewNode`, so it can contain the table above, a form, or
any composition. The result carries the chosen action id **plus data the body
collected** (selected row, form fields).

```rust
async fn restart_dialog(ctx: &ChromeCtx, rows: &[ContainerRow]) {
    let result = ctx.overlay.open_modal(ModalSpec {
        title: "Restart a container".into(),
        body: Column::new().gap(10)
            .child(Label::new("Pick a container to restart:"))
            .child(containers_table(rows))                 // ← the table from Example 2
            .child(Input::new("filter").placeholder("filter…")
                .on_change(intent("plugin.docker.filter", {}))),
        actions: vec![
            ModalAction::danger("restart", "Restart"),
            ModalAction::new("cancel", "Cancel"),
        ],
        danger: false,
        dismissible: true,
    }).await;

    // The host owns z-order, focus trap, Esc, click-outside, positioning.
    if let ModalResult::Action { id, data } = result {
        if id == "restart" {
            if let Some(container_id) = data.get("selected_id") {
                ctx.actions.dispatch("plugin.docker.restart", { "id": container_id });
            }
        }
    }
}
```

Under WASM (Phase 9) the `await` marshals as a request-id plus a resolve event —
you never block the UI thread and never touch the overlay's rendering.

---

## Context menus & KeyHint — overlays and hints from a plugin

These are **host-owned capabilities**, not widgets you nest in a `ViewNode`. A plugin
never draws a context menu nor manages a KeyHint; it **requests** the overlay or
**declares** an intent, and the host owns z-order, focus, keyboard routing, and dismissal.

**Context menu = a host-owned dropdown anchored at the right-click.** Two equivalent ways:

*Declarative* — attach a context handler in the `ViewNode`:

```rust
Row::new()
    .child(Label::new(&container.name))
    .on_context([ item("restart", "Restart"), item("remove", "Remove") ])
```

On right-click the host opens the menu, owns z-order / focus / click-outside / Esc, and
returns the chosen entry to you as an **intent** (`plugin.docker.restart`, …).

*Imperative* — the same, in response to the event:

```rust
let choice = ctx.overlay.open_dropdown(DropdownSpec {
    anchor: click_rect,          // a context menu is just a dropdown anchored at the click
    side: OverlaySide::Below,
    items: vec![ item("restart", "Restart"), item("remove", "Remove") ],
}).await;
if let DropdownResult::Picked(id) = choice { ctx.actions.dispatch(&id, args) }
```

Items are data (id + label); one item may carry a `ViewNode` if it needs rich content, but
the *menu itself* stays host-owned.

**KeyHint is different: it's the host's universal leader/vimium overlay, not a widget.**
You never create a KeyHint. Instead, **any plugin widget that exposes an `on_press` intent
is automatically "hintable"** — when the user triggers the leader, the host assigns letters
to *every* clickable target (app + plugin) and, on the keypress, emits the intent:

```rust
Button::new("Restart").on_press(intent("plugin.docker.restart", { "id": id }))
// → the host assigns it a leader letter and routes it. No KeyHint code in the plugin.
```

Opt a widget out with `.hintable(false)`; the default is "has an intent ⇒ is a target". One
system covers app and plugin widgets alike.

**Rule of thumb:** `ViewNode` = *mounted content*; **overlays** (context menu / dropdown /
modal) = *requested* from the host; **KeyHint** = *enabled* by exposing an intent.

## The closed vocabulary (and how to extend it)

Plugins compose from the host's **closed** `WidgetKind` set. You get unlimited
*instances and compositions*, but you cannot invent a new *rendering primitive* —
that would be something the host can't draw or theme. Adding a new widget (e.g. a
first-class `Table`) is **host-side** work: the widget in `heca-grid-ui`, its
showcase demo + `docs/widgets.md`, and a mapper arm. This is the guardrail that
keeps the host the single owner of rendering.

## What's shipped vs planned

- **Shipped:** the `heca-grid-ui` widgets (`Modal`, `Select`, `Button`, `Row`,
  `Label`, `Badge`, …) and the read/observe `App` facade (`app.on`/`app.state`).
- **Planned (this document's model):** the `ViewNode` type + typed builder SDK +
  host mapper (`plugin-ui`), the rich-`body` overlays (`plugin-task-ui-4`), and the
  WASM runtime + host-adapter (`plugin-08`).
