# Identity — what a widget is called, and who has to say it

*Decided 2026-08-17. Replaces `nav_key` and `scope_key`.*

Four things need to name a widget: the **keyboard cursor** (which row am I on), the **right-click**
(what did I land on), **drag**, and the **hint picker** (which target wears which letter, and does it
keep it next time). Today they read `Base::nav_key` and `Base::scope_key`.

Those names are the problem. They describe *how the framework uses the string* rather than what it
is, and a developer adding a button has no reason to guess that either exists.

Antonio, 2026-08-17: *"i don't want plugin authors or developers to have to add this strange and
confusing name… if i were a developer adding a button i will forget to add that."* And on the first
answer, which still made every widget declare a role: *"it's the same for any widget, a developer has
to think to add those unusual properties. It's uncommon."*

---

## 1. The rule

**Two cases, and only two.**

| what you are building | what you write |
|---|---|
| anything at all — a button, an icon, a card, a label | **nothing** |
| an item in a collection you are iterating | **`.key(…)`** — the item's own id, from your data |

```rust
IconButton::new(Glyph::ChevronDown).on_click(move || expand(id))   // nothing

for pane in &column.panes {
    Item::new(&pane.name).key(pane.id)                             // its own id
}
```

That is the whole surface. There is no role to declare, no region to name, and nothing to remember
on an ordinary widget.

---

## 2. `key` is React's `key`, and means the same thing

Every developer who has written React, Vue or Svelte already knows this concept: when you render a
collection, each child carries the identity of *the thing it represents*, so the framework can tell
"this row again" from "a different row" after a rebuild.

That is exactly what is needed here, for exactly the same reason. A chrome tree is rebuilt for
reasons that have nothing to do with navigation — a pane's git status changing is enough — and a
cursor, a letter or a right-click target that resets every rebuild is not one.

### You never count

**`key` is never a position and never a counter.** It comes from the data you are already iterating:

```rust
Item::new(&pane.name).key(pane.id)            // heca — a PaneId
Row::new().key(&container.id)                 // a plugin listing docker containers
```

Antonio, 2026-08-17: *"what does it mean `pane:7`? Should the developer count the number of panes
they are adding?"* No. If you are reaching for a counter, the key is wrong — an index is exactly the
thing that changes when the list changes, which is what identity is for.

### Where it is required

**In a collection, and nowhere else** — the same rule React uses, and the same place a developer
already expects to think about it. Outside one, identity is derived (§4) and you write nothing.

---

## 3. Nesting is structure, not a second concept

The sidebar is a **tree**: a workspace row holds column rows, which hold pane rows. Every level is
both *a row the cursor stops on* and *a container of the next level*.

```rust
DockFrame::new(&ws.name).key(ws.id)                 // a row, and a container
    .child(MarkerGroup::new().key(col.id)           // a row, and a container
        .child(Item::new(&pane.name).key(pane.id))) // a leaf row
```

One property at every level. Nesting is expressed by the tree, exactly as it is in React's nested
lists.

### This is why `scope_key` disappears

`scope_key` answered *"which region did this press land in"* — a second declaration for the same
point in the tree, and the reason two fields existed at all.

With a key at every level it is **derived**: the region is the **nearest keyed ancestor**. Nobody
declares it, and it cannot be forgotten or fall out of step with the row it encloses.

> The old `Base::scope_key` doc argued the two must stay separate, because folding them would make a
> region turn up in `collect_nav_keys` as a steppable row. That held while identity and role were the
> same declaration. Once every node is keyed, "region" is a question you *ask* of the tree rather
> than something a widget asserts.

---

## 4. Everything else gets an identity anyway

A widget that declares no `key` still needs one, or a hint letter cannot stay with it between
openings. It is derived, in three levels — each used only when the one above is ambiguous:

1. **its name, within the nearest keyed ancestor** — `pane:7 / ×`
2. **name + index among identically-named siblings in that scope** — `topbar / ×[1]`
3. nothing else; that is the floor

The name comes from `Component::text_summary()` (`heca-grid-ui/src/component.rs`), which already
computes one *"from the contents, like the web's accessible-name algorithm"*.

⚠️ **Derive from content, never from position.** `Flex/Row[2]/Button[0]` looks automatic and drifts on
every tree change — which is the bug this exists for: expanding a pane moved a button's letter from
`k` to `j`. Content-based identity does not move. The level-2 index counts only identically-named
siblings *in one scope*, so it shifts when a `×` is added beside other `×`s and never because
something changed elsewhere.

**Known limit:** a derived identity changes if the label changes. Fine for a remembered letter; not
fine for anything durable, which is what an explicit `key` is for.

---

## 5. Forcing a `key` — a warning, not a type

Considered and rejected: a typestate builder where an item in a collection does not compile without a
key. It puts the rule in the compiler, but it is noise on every widget, and a plugin sending JSON
never meets the Rust compiler.

What is built instead, and it is again what React does:

- **a warning when a collection's children have no keys** — the moment it can be seen and fixed;
- **a test** over heca's own chrome, so our rows stay keyed;
- **an error at realize time** for a plugin's node that declares a `press` inside a collection with
  no `key` — the only mechanism that reaches a plugin author.

Demanding a name from everyone up front is how you get `"btn1"`, which is worse than no name.

---

## 6. Naming

**`key`, not `id`.** `id` suggests global uniqueness across a document, the way HTML means it. This
is scoped to its collection — two lists may both have a `key("1")` — which is precisely React's
meaning.

**No `Hintable…` prefix on anything.** The identity serves the cursor, the right-click, drag *and*
the picker; naming it after one of four readers would be wrong, and `HintableItem` would suggest a
second kind of `Item` rather than a property of the one that exists.

---

## 7. What this replaces

| gone | replaced by |
|---|---|
| `Base::nav_key`, `ComponentExt::nav_key` | `Base::key`, `ComponentExt::key` |
| `Base::scope_key`, `ComponentExt::scope_key` | derived — the nearest keyed ancestor |
| an author remembering either exists on an ordinary widget | nothing to write |

Readers that must keep working: `nav::collect_nav_keys`, `nav::nav_key_at`, `nav::scope_at`, the
cursor, right-click resolution, drag, and `hint::offer_hint_by_key` — which matched *either* field and
now matches one.

Scale at the time of writing: 170 `nav_key` sites, 15 `scope_key`.

See also `docs/hint-architecture.md` § 2d–2e, where this came from: a letter has to stay with its
target between openings of the picker, and that is impossible without an identity.
