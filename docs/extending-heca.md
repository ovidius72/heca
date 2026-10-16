# Building on heca — adding to it from another crate

`heca` is a library plus a one-line program. Plain heca is:

```rust
fn main() { heca::run(); }
```

A program built on heca says who it is, adds what it wants through heca's **existing doors**, and
calls the same `heca::run()`. Nothing here is a second mechanism: each door fills the one heca's own
parts use, so what is added is reachable from a key, a menu, the palette and RPC with nothing written
for any of them. A worked, runnable example: `heca/examples/extend_heca.rs`
(`cargo run -p heca --example extend_heca`).

```rust
fn main() {
    let demo = heca::extension("demo");
    // Add actions, layers and pane decorations through `demo`.
    heca::run();
}
```

A dock is contributed separately through `heca::regions(..)` and a `Provider`; its `id()` names
the placement and its `kind()` names the provider type. The complete compiling example shows both
APIs: [`heca/examples/extend_heca.rs`](../heca/examples/extend_heca.rs).

## The rules

- **The name is the program's, said once.** `heca::extension("demo")`. `heca` is reserved for the app;
  a name with a dot is refused; a second extension with the same name is refused. A short name with a
  dot is refused too. Every refusal is *said* (once, to whoever is writing the program), never silent.
- **What is added is a default.** Where a dock sits and whether an item shows is the user's to decide.
  A dock they move stays where they put it. Pane buttons, chips and lines are listed **by name** in
  `[appearance.pane] title_actions`, `title_segments` and `[appearance.sidebar] pane_lines`: while a
  list is still heca's own, what you added shows after it; once the user writes their own list only
  their list counts, and they are told **once** that an unlisted item exists.
- **Before `run()`.** Every door queues; the app takes the queues once, as it starts. A call after
  that is said and dropped.
- **Words are an English fallback** (`.label(..)`, `.description(..)`) until the app-wide text system
  lands.

## The doors

| you add | with | side | notes |
|---|---|---|---|
| a dock | `heca::regions("sidebar.left").append(dock)` | client | a `Provider`; placed by the user like any dock |
| an action | `demo.action("s")…run(f)` | server | policy defaults to `AlwaysAllowed` (refused while a floating pane is active); say `.policy(..)` otherwise; `.destructive()` asks first |
| an overlay | `demo.layer("s").view(f)` | client | opened with `toggle_layer name=demo.s` from a key, menu, palette or RPC; built again each time it opens |
| a pane header button | `demo.pane_button("s")` | client | runs the action `demo.s`; icon, words, tooltip and danger come from it; if it declares an integer `pane` argument (`heca::PANE_ARG`) it receives the pane's id |
| a terminal | `demo.terminal("s")` | client | the terminal `demo.s` in any tree you build (a dock, an overlay): `.command(..)`, `.cwd(..)`, `.title(..)`; the same name is the same running process, and closing the dock never ends it |
| a pane header chip | `demo.pane_chip("s", f)` | client | `f(&PaneFacts) -> Option<PaneChip>`; re-run when the pane's facts change |
| a sidebar row line | `demo.pane_line("s", f)` | client | `f(&PaneFacts) -> Option<PaneLine>` |

`PaneFacts` is plain data about one pane (`#[non_exhaustive]`: fields may be added, so read them by
name). An action changes shared state; a dock, layer or pane decoration is drawn by a window.
The server/client boundary carries those responsibilities without changing these extension calls.

## Where the code lives

`heca/src/entry/` (the doors), `heca/src/chrome/panes/pane_items/` (buttons, chips, lines and the listing
rules they share), `heca/src/lib.rs` (what is public).
