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
    // A dock. It carries its own name (its `kind()`), so it needs no extension.
    heca::regions("sidebar.right").append(Notes::new("notes"));

    // Who this program is — once. Everything below is named `<name>.<short>`.
    let pro = heca::extension("pro");

    pro.action("start").label("Start").icon(Glyph::Play).run(|cx, _| { /* … */ Handled::Yes });
    pro.layer("planner").view(|cx| Overlay::new().panel(planner(cx)).default_open(false));
    pro.pane_button("start");                       // a button in every pane's header
    pro.pane_chip("status", |facts| Some(PaneChip::new(Glyph::Terminal, facts.app_name.clone())));
    pro.pane_line("folder", |facts| Some(PaneLine::new(Glyph::Folder, "…")));

    heca::run();
}
```

## The rules

- **The name is the program's, said once.** `heca::extension("pro")`. `heca` is reserved for the app;
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
| an action | `pro.action("s")…run(f)` | server | policy defaults to `AlwaysAllowed` (refused while a floating pane is active); say `.policy(..)` otherwise; `.destructive()` asks first |
| an overlay | `pro.layer("s").view(f)` | client | opened with `toggle_layer name=pro.s` from a key, menu, palette or RPC; built again each time it opens |
| a pane header button | `pro.pane_button("s")` | client | runs the action `pro.s`; icon, words, tooltip and danger come from it; if it declares an integer `pane` argument (`heca::PANE_ARG`) it receives the pane's id |
| a pane header chip | `pro.pane_chip("s", f)` | client | `f(&PaneFacts) -> Option<PaneChip>`; re-run when the pane's facts change |
| a sidebar row line | `pro.pane_line("s", f)` | client | `f(&PaneFacts) -> Option<PaneLine>` |

`PaneFacts` is plain data about one pane (`#[non_exhaustive]`: fields are added, so read them by
name). *Server* and *client* is the line the server/client split (F012/P104) will route along: an
action changes state, so it runs where the state is; everything drawn lives in a window.

## Where the code lives

`heca/src/entry/` (the doors), `heca/src/chrome/pane_items/` (buttons, chips, lines and the listing
rules they share), `heca/src/lib.rs` (what is public).
