# The Widget Architecture — `ViewNode`, `realize`, and the widgets

**Status: SETTLED. This is the architecture. Do not re-propose alternatives to it.**

If you are an agent starting a session on heca UI work: read this file **before** proposing
anything about widgets, `ViewNode`, plugins, or the declarative model. Every question in §5 has
already been decided — repeatedly. Re-litigating them wastes the maintainer's time.

---

## 1. The two layers, and the one bridge

heca's UI is a declarative tree over a retained widget tree — the same shape SwiftUI and Flutter
use (`Widget` → `Element`/`RenderObject`).

```
   ViewNode                     the DESCRIPTION   (heca/src/chrome/view.rs — the APP crate)
   { kind, props, events, children }              pure data · serde · no closures · no signals
        │
        │   realize(&ViewNode, emit, hints, forms) -> Box<dyn Component>
        │   the ONE bridge     (heca/src/chrome/realize.rs — the APP crate)
        ▼
   Component tree               the LIVE WIDGETS  (heca-grid-ui — the LIBRARY crate)
   Base + children: Vec<Box<dyn Component>>       builder API · closures · signals
        │
        ▼
   taffy layout → paint → Scene → heca-renderer → GPU
```

**Layer 1 — `ViewNode` (description).** A serializable node: `kind` (the closed `WidgetKind`
vocabulary), `props` (scalars + semantic enums — *never* raw colors/pixels), `events`
(`press`/`change` → an `Intent` = action id + args, **never a closure**), and `children`
(`Vec<ViewNode>`, recursive). This is what a WASM plugin ships over the boundary, what RPC can
send, and what native code authors when it wants a declarative body (modals, menus, panels).

**Layer 2 — the widgets (`heca-grid-ui`).** Real retained components: embed `Base`, hold
`children: Vec<Box<dyn Component>>`, own paint/event/tick, expose a builder API with **closures**
(`.on_click(move || …)`) and **signals** (`.state()`, `.text_signal()`). The native chrome
(sidebar, pane headers) is built directly here.

**The bridge — `realize`.** The single, host-side mapper. It owns the theme lookup, the intent
emitter, and the hint registry, so neither layer has to. Both authoring paths converge on the
**same retained tree**.

---

## 2. The dependency rule (this is why the model looks the way it does)

```
heca (app)  ──depends on──▶  heca-grid-ui (library)
```

**`heca-grid-ui` NEVER depends on `heca`.** Same boundary as `heca-core`: the library is
headless, GPU-free, app-free, unit-testable.

`ViewNode` lives in the **app**. Therefore:

> ### ❌ `Button::new(ViewNode)` is impossible. Never propose it.
>
> A widget constructor cannot take a `ViewNode`, because that would make the library depend on
> the app's model — and then the library would also need `realize`, which needs the app's
> `InteractionIntent` / `HintTargetRegistry` / theme wiring. It inverts the crate graph.

**And moving `ViewNode` down into `heca-grid-ui` is also rejected** — not just for layering, but
because `ViewNode` **cannot carry closures or signals** (it must serialize for WASM). The native
chrome depends on both:

```rust
Row::new().on_activate(move || select(i))   // a closure — impossible in ViewNode
row.state().set(true)                       // flip active IN PLACE, no rebuild
label.text_signal().set(new_name)           // live text, no rebuild
```

If every native tree had to be authored as `ViewNode`, we would lose in-place signal updates and
callbacks: every state change becomes a full rebuild through `realize`. That fights the reactive
chrome store the whole architecture is built on. **So the library keeps its builder API.**

---

## 3. How a widget holds content (THE RULE)

> **A widget's *content* is COMPOSED from child `Component`s — never hand-drawn in `paint`.**
> A widget paints only its own **chrome** (background, border, glow, focus ring, press flash),
> all from `Theme`. Its content (labels, icons, rows) are children the layout engine positions
> and the children paint themselves.

This is what makes a widget:
- **`ViewNode`-realizable** — `realize` has somewhere to attach `node.children`; and
- **extensible by composition** — a new affordance is a child, not a hand-positioned `cx.text` /
  `cx.icon` call.

**When you touch a widget, you refactor it toward this.** A leaf that hand-draws its content is a
refactor target, not something to bolt more hand-drawing onto. (`Button` drawing its own label,
`Item` drawing its own label: both are targets.)

**Children are open.** A slot takes `impl Component` — any widget. Do **not** narrow it to a
closed enum (`Icon | Label`); that is a narrow patch against a general problem. Widgets with an
intrinsic semantic color (`Badge::danger`, `StatusDot::online`) keep it; unstyled text/glyphs
inherit the parent's state color.

**A realized subtree goes in via `*_boxed`.** `realize` returns `Box<dyn Component>`, which is not
itself `Component`, so it cannot go through `Parent::child`. The established seam is an explicit
boxed setter — `Dialog::body_boxed(Box<dyn Component>)` is the precedent; new widgets follow it.

---

## 4. The two authoring paths (both legal, same result)

**Declarative** (plugins, RPC, modal/menu bodies) — arbitrary tree, arbitrary depth:

```rust
ViewNode::new(WidgetKind::Button)
    .prop("variant", PropValue::Variant(ViewVariant::Destructive))
    .on_press(Intent::new("confirm_ok"))
    .child(ViewNode::new(WidgetKind::HStack)
        .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("trash".into())))
        .child(ViewNode::new(WidgetKind::Label).text("Delete")))
```

**Native builder** (chrome, sidebar, pane headers — needs closures/signals):

```rust
Button::destructive("Delete")
    .child(Icon::new(Glyph::Trash))
    .on_click(move || emit(intent))
```

`realize` turns the first into the second. **Same widget, same retained tree.**

Extending the vocabulary (a new `WidgetKind`) is **host-side** work — the widget in
`heca-grid-ui`, its `realize` arm, its showcase demo, its `docs/widgets.md` entry. Plugins only
*compose* existing kinds; they never invent a rendering primitive.

---

## 5. SETTLED — do not re-propose these

| Question | Answer | Why |
|---|---|---|
| Should `heca-grid-ui` own `ViewNode`? | **No.** | Inverts the crate graph; and `ViewNode` can't carry the closures/signals native chrome needs. |
| Can a widget constructor take a `ViewNode` (`Button::new(ViewNode)`)? | **No.** | Same reason. The library never sees the app's model. |
| Is `realize` the only ViewNode→widget path? | **Yes.** | One bridge, app-side, owns theme + intent + hint wiring. |
| Do widgets hold children, or hand-draw content? | **Children.** | The rule in §3. Hand-drawn content is a refactor target. |
| Can any component go inside a widget's slot? | **Yes.** | Slots are `impl Component`. Never narrow to a closed enum. |
| How does a realized subtree get into a widget? | **`*_boxed` setter.** | `Dialog::body_boxed` is the precedent. |
| Does behaviour cross the plugin boundary as a callback? | **No — an `Intent`** (action id + args). | Keeps the model serializable; click, KeyHint pick, and RPC all fire the same intent. |
| Is styling a prop? | **Yes — changed 2026-07-26.** | The `Theme` gives the default; code may override it with a string (`"#ff8800"` or a theme name). The old "No" is dead — do not restore it. |

---

## 6. Where the code lives

| Concern | File |
|---|---|
| `ViewNode` / `WidgetKind` / `PropValue` / `Intent` | `heca/src/chrome/view.rs` |
| `realize(&ViewNode) -> Box<dyn Component>` | `heca/src/chrome/realize.rs` |
| Overlay: `ModalSpec` → `Dialog` (the `*_boxed` seam in action) | `heca/src/chrome/overlay.rs` |
| The widgets | `heca-grid-ui/src/widgets/` |
| `Base` / `Component` / `PaintCx` | `heca-grid-ui/src/component.rs` |
| Widget catalog (human docs) | `docs/widgets.md` |
| Long-arc plugin design | `chrome-and-ui.md` §2.6.2, §2.7.2 |
| Plugin authoring model | `chrome-and-ui.md` |

---

## 7. Open (genuinely not yet decided)

These are *not* settled, and are the live design work — everything above is.

~~`realize` coverage~~ — **done.** Every kind maps to a live widget, guarded by a test.
~~Composing the leaf widgets~~ — **done** (F003/P015, F003/P016).

- **Typed builder SDK** over `ViewNode` (`VStack::new().gap(8).child(…)`) — **F003/P011/T006**,
  waiting on **F003/P017**.
