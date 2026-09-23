# The Widget Architecture — `ViewNode`, `realize`, and the widgets

**Status: SETTLED. This is the architecture. Do not re-propose alternatives to it.**

If you are an agent starting a session on heca UI work: read this file **before** proposing
anything about widgets, `ViewNode`, plugins, or the declarative model. Every question in §5 has
already been decided — repeatedly. Re-litigating them wastes the maintainer's time.

> **Why this shape at all — the one sentence that explains every rule below.** heca is meant to be
> extended by *other people*. The target is that someone writing a plugin, or contributing to the
> project, authors UI the way they would in **Flutter or SwiftUI**: a declarative tree of typed
> widgets, composed, with behaviour attached to the widget itself — and gets exactly what the app's
> own chrome gets, with no host-private type, no registry, and no second-class path. That is
> `AGENTS.md` → ⭐⭐ RULE ZERO, and it outranks everything in this file. When a rule below looks
> like ceremony, check it against that sentence: if a plugin author would have to know it, the API
> is the bug.

---

## 1. The two layers, and the one bridge

heca's UI is a declarative tree over a retained widget tree — the same shape SwiftUI and Flutter
use (`Widget` → `Element`/`RenderObject`).

```
   build::VStack / build::Button …   the TYPED SDK   (heca-view/src/build.rs)
   SwiftUI-shaped builders that lower to a ViewNode  — `Label::new(..).title(..)` will not compile
        │
        ▼
   ViewNode                     the DESCRIPTION   (heca-view — its OWN crate, serde and nothing else)
   { kind, props, events, children }              pure data · serde · no closures · no signals
        │
        │   realize(&ViewNode, theme, emit, forms) -> Box<dyn Component>
        │   the ONE bridge     (heca-view-realize — its OWN crate, BELOW the app)
        ▼
   Component tree               the LIVE WIDGETS  (heca-grid-ui — the LIBRARY crate)
   Base + children: Vec<Box<dyn Component>>       builder API · closures · signals
        │
        ▼
   taffy layout → paint → Scene → heca-renderer → GPU
```

> ⚠️ **Moved 2026-07-27 (F003/P017/T009).** `ViewNode` and `realize` used to live in the app
> (`heca/src/chrome/view.rs` and `chrome/realize.rs`); older text everywhere still says so. They are
> now two crates **below** `heca`: `heca-view` (the model, serde only, no widget library at all) and
> `heca-view-realize` (the bridge, which owns the `heca-grid-ui` dependency). That is what lets a
> plugin depend on the vocabulary without compiling the thing that draws it — and why
> `heca-renderer`'s showcase, which sits below the app, can render a described tree beside its
> hand-built twin.

**Layer 0 — the typed SDK (`heca-view::build`).** The SwiftUI-shaped surface an author actually
writes: `VStack::new().gap(8).child(Button::new("Restart").on_press(intent))`. Each kind is its own
type, so the compiler refuses what the widget cannot do (`Label::new(..).title(..)` is an error;
`Separator::new().orientation("horizonal")` will not take the typo). It lowers to a `ViewNode` and
adds no capability — it is ergonomics and type-safety over the same data, exactly as Flutter's typed
`Widget` classes lower to `Element`/`RenderObject`.

**Layer 1 — `ViewNode` (description).** A serializable node: `kind` (the closed `WidgetKind`
vocabulary), `props` (scalars, semantic enums, and appearance — a colour is a hex literal or a
theme token **name**, resolved by `realize` against the theme the tree is built with), `events`
(`press` / `hint` / `change` / `toggle` / `action` / `dismiss` → an `Intent` = action id + args,
**never a closure**), and `children` (`Vec<ViewNode>`, recursive). This is what a WASM plugin ships
over the boundary, what RPC can send, and what native code authors when it wants a declarative body
(modals, menus, panels).

**Layer 2 — the widgets (`heca-grid-ui`).** Real retained components: embed `Base`, hold
`children: Vec<Box<dyn Component>>`, own paint/event/tick, expose a builder API with **closures**
(`.on_click(move || …)`) and **signals** (`.state()`, `.text_signal()`). The native chrome
(sidebar, pane headers) is built directly here.

**The bridge — `realize`.** The single mapper. It owns the theme lookup and the intent emitter, so
neither layer has to. Both authoring paths converge on the **same retained tree**.

It used to own a third thing — a **hint registry** — and that is gone (F004/P084/T399, 2026-08-11).
A pickable node now writes its own `hint` behaviour into the widget's `Base::hint` slot, and the
framework collects the declarations out of the laid-out tree. There is no sink to hand in, nothing
to un-register, and nothing a plugin cannot reach. **When a bridge grows a registry parameter, that
is the smell**: it means the capability behind it is host-private.

---

## 2. The dependency rule (this is why the model looks the way it does)

```
heca (app)  ──depends on──▶  heca-grid-ui (library)
```

**`heca-grid-ui` NEVER depends on `heca`.** Same boundary as `heca-core`: the library is
headless, GPU-free, app-free, unit-testable.

`ViewNode` lives **above** the library, in `heca-view`. (It lived in the app until F003/P017/T009;
moving it into its own crate changed *who may depend on it*, not the direction of the arrow.)
Therefore:

> ### ❌ `Button::new(ViewNode)` is impossible. Never propose it.
>
> A widget constructor cannot take a `ViewNode`, because that would make the library depend on the
> model — and then the library would also need `realize`, which needs the theme and intent wiring
> above it. It inverts the crate graph.

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

**A realized subtree goes in through the slot's own builder.** `realize` returns
`Box<dyn Component>`, which is not itself `Component` — so every child-taking builder used to come
in twos, sixteen of them, and a caller had to know which spelling to reach for. They take
`impl IntoComponent` now, which covers a widget and a box alike and hands an already-boxed subtree
through rather than wrapping it again. **A new slot takes `impl IntoComponent`; do not add a
seventeenth `*_boxed`.**

---

## 4. The two authoring paths (both legal, same result)

**Declarative — the typed SDK** (what a plugin author writes; `heca-view::build`):

```rust
use heca_view::build::*;

Button::new("Delete")
    .variant(ViewVariant::Destructive)
    .on_press(Intent::new("confirm_ok"))
    .child(HStack::new()
        .child(Icon::new("trash"))
        .child(Label::new("Delete")))
```

**Declarative — the raw node** (what that lowers to, and what crosses the wire):

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
| Is `realize` the only ViewNode→widget path? | **Yes.** | One bridge, in `heca-view-realize`, owns theme + intent wiring. (It was app-side until F003/P017/T009 moved it below the app; it never owned a *second* path.) |
| Does the bridge need a registry handed in (hints, ids, sinks)? | **No — and a new one is a bug.** | F004/P084/T399 deleted the last of them. A capability a described node cannot express without host plumbing is a capability a plugin only has a second-class version of. |
| Does a plugin author write raw `ViewNode`? | **They may, but the typed SDK is the surface.** | `heca-view::build` — one type per kind, so the compiler refuses what the widget cannot do. Same relationship Flutter's typed `Widget`s have to `Element`. |
| Do widgets hold children, or hand-draw content? | **Children.** | The rule in §3. Hand-drawn content is a refactor target. |
| Can any component go inside a widget's slot? | **Yes.** | Slots are `impl Component`. Never narrow to a closed enum. |
| How does a realized subtree get into a widget? | **The slot's own builder.** | Slots take `impl IntoComponent`; the `*_boxed` twins are deleted. |
| Does behaviour cross the plugin boundary as a callback? | **No — an `Intent`** (action id + args). | Keeps the model serializable; click, KeyHint pick, and RPC all fire the same intent. |
| Is styling a prop? | **Yes — changed 2026-07-26, built 2026-07-27.** | The `Theme` gives the default; code may override it with a string (`"#ff8800"` or a theme name). The old "No" is dead — do not restore it. All of `Visual` is settable; a token name resolves against the theme the tree is built with. |

---

## 6. Where the code lives

| Concern | File |
|---|---|
| `ViewNode` / `WidgetKind` / `PropValue` / `Intent` | `heca-view/src/lib.rs` |
| The typed SDK a plugin author writes | `heca-view/src/build.rs` |
| `realize(&ViewNode, theme, emit, forms) -> Box<dyn Component>` | `heca-view-realize/src/lib.rs` |
| Overlay: `ModalSpec` → `Dialog` (a realized subtree entering a slot) | `heca/src/chrome/overlay.rs` |
| The widgets | `heca-grid-ui/src/widgets/` |
| `Base` / `Component` / `PaintCx` | `heca-grid-ui/src/component.rs` |
| Widget catalog (human docs) | `docs/widgets.md` |
| Long-arc plugin design | `chrome-and-ui.md` §2.6.2, §2.7.2 |
| Plugin authoring model | `plugins.md` |

---

## 7. Open (genuinely not yet decided)

These are *not* settled, and are the live design work — everything above is.

~~`realize` coverage~~ — **done.** Every kind maps to a live widget, guarded by a test.
~~Composing the leaf widgets~~ — **done** (F003/P015, F003/P016).
~~Where the plugin-facing types live~~ — **done** (F003/P017/T009). `heca-view` is its own crate,
depending on serde and nothing else, so a plugin can name the vocabulary without compiling the
renderer.
~~Typed builder SDK~~ — **done.** `heca-view/src/build.rs`; a drift guard in `heca-view-realize`
fails the build when a widget grows a property the SDK cannot set.

- **Generated type definitions** for plugins in *other* languages, from the same widget list —
  **F003/P001/T006**. The Rust author is served; a JS or Python author is not, and that is the gap
  between "a plugin can do this" and "anyone can write a plugin".
- **Every widget buildable from a description, with no host-only exceptions** — **F004/P084/T398**.
  The rule is already written (a widget whose state is a live host signal is host-only, §5); the
  audit that proves the list is *only* those is not done.
- **The WASM boundary itself** — **F003/P022**. Everything above is the authoring model; nothing
  yet loads a plugin.
