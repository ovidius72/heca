# Laying things out

How to arrange widgets in heca: which container to reach for, how to say a size, and the handful of
rules that decide whether an arrangement survives a font change, a resize and a fourth child.

Every value here is written the way a stylesheet writes it, and **one parser reads each kind** —
shared with the wire, so the line you write in Rust is the line a plugin writes in JSON. The enum
variants (`Track::Fr`, `Justify::SpaceBetween`, `Length::Percent`, `Space::Step`) still exist and are
what the strings parse *to*; you should not need to type one.

- [The one rule](#the-one-rule)
- [Grid](#grid)
- [Flex](#flex)
- [Surface](#surface)
- [Children — one or many](#children--one-or-many)
- [Sizes](#sizes)
- [Space — gap, padding, margin](#space--gap-padding-margin)
- [Alignment](#alignment)
- [Shares](#shares)
- [Order](#order)
- [Traps](#traps)

---

## The one rule

> **Anything with more than one PART is a `Grid` with a track template.**

A header and a body. A header, a body and a footer. A toolbar above a list. Two parts or more, and
the arrangement is a template — `auto` for what sizes itself, `1fr` for what takes the rest:

```rust
Grid::new()
    .template_row("auto 1fr auto")   // header · body · footer
    .child([header, body, footer])   // auto-placed, in order
```

Never a stack of `Flex` children with `grow` weights and heights tuned by hand, and **never a parent
that counts its children** to work out which one is which.

**Why.** The template says the whole arrangement in one line a reader can check against the picture.
A flex stack says it in as many tuned numbers as there are children, spread across the file, and each
is right only for the child count it was written for — add a footer and every other term needs
revisiting.

**The counting failure is the one to watch for.** A pane holds `[header, content]`, so the host asks
*"does this pane have two children?"* to decide whether it has a header. Put two things in the body
and the first is mistaken for a header. A template has no such question: a part is in the row it was
placed in, and a missing part is a missing row.

**A part is optional by being absent**, not by a flag — build the template from the parts you have.

Use a [`Flex`](#flex) for a *list of like things* — a row of buttons, a column of rows — where the
children are interchangeable and none has a job the others don't.

---

## Grid

CSS-grid layout: explicit row/column tracks, named areas, per-child placement. Pure layout, no
styling.

### Tracks

Each axis is named, because a bare `template` cannot say which axis it means and a reader should
never have to remember:

```rust
Grid::new()
    .template_row("auto 1fr")            // rows: header sizes itself, body takes the rest
    .template_column("22px 1fr auto")    // columns: icon · title · badge
```

Both take **whichever shape you hold**:

```rust
.template_row("auto 1fr")                    // one stylesheet line
.template_row(["auto", "1fr"])               // a list of strings
.template_row([200, 100])                    // numbers are pixels
.template_row([Track::Auto, Track::Fr(1.0)]) // the variants, if you have them already
```

A track is spelled as CSS spells it:

| spelling | means |
| --- | --- |
| `"auto"` | sized to fit its content |
| `"1fr"`, `"2.5fr"` | a fraction of the leftover free space |
| `"200px"`, `"200"` | fixed logical pixels |
| `"min-content"` / `"min"` | as small as the content allows |
| `"max-content"` / `"max"` | as large as the content wants |
| `"repeat(3, 1fr)"` | three equal tracks — expands as CSS expands it |
| `"auto repeat(2, 1fr) auto"` | a repeat among ordinary tracks |

Hyphen and underscore are the same character, so `"min-content"` and `"min_content"` both land.

⚠️ **An unreadable track is `auto`, never a panic** — these arrive from config and from plugins, so a
typo costs its author a track size, not the host. Use `"1fr".parse::<Track>()` when you want to be
told instead.

### Placing children

**Add them and let the grid place them**, exactly as in HTML. Auto-placement fills the tracks in
order and wraps to the next row:

```rust
Grid::new()
    .template_column("1fr 1fr")
    .child([a, b, c, d])       // two columns, so: a b / c d
```

**A child that needs a particular place says so about itself** — CSS's `grid-column` and `grid-row`,
on the item:

```rust
Label::new("title").column(2)             // column 2
header.column("1 / -1")                   // the whole width, however many columns there are
wide.column("1 / span 2")                 // two columns from the first
body.row(2).column("1 / -1")              // second row, full width
sidebar.row_span(2)                       // two rows tall, wherever it lands
```

| spelling | means |
| --- | --- |
| `2` / `"2"` | start at line 2, one track |
| `"1 / -1"` | line 1 to the end — **every track there is** |
| `"1 / span 2"` | two tracks from line 1 |
| `"2 / 4"` | line 2 up to line 4 |

`"1 / -1"` is the one worth knowing: it stays true when a column is added later, where a hard-coded
span does not.

**Named areas**, when the shape repeats or a part spans. The grid names the regions and the child
says which one it belongs in — CSS's `grid-template-areas` and `grid-area`:

```rust
Grid::new()
    .template_column("22px 1fr auto")
    .template_row("auto auto")
    .template_area(["dot title tag",
                    ".   sub   ."])
    .gap("xs")
    .child([
        Icon::new(Glyph::Terminal).area("dot"),
        Label::new("nvim").area("title"),
        Badge::success("RUN").area("tag"),
        Label::new("~/src").area("sub"),
    ])
```

`.` or `_` marks an empty cell, and a name covering several cells is the box around them.

**Both are resolved during layout**, against whichever grid ends up holding the child. So a child
can be built before its parent exists, and changing a template re-places the children without
rebuilding any of them. A child of something that is not a grid simply ignores what it said.

### Every Grid builder

**On the grid** — it holds the template and nothing else:

| builder | takes |
| --- | --- |
| `.template_row(..)` | a track line (`"auto 1fr"`, `"repeat(3, 1fr)"`) or a list |
| `.template_column(..)` | the same, for columns |
| `.template_area(..)` | one area row (`"dot title"`) or a list of them |
| `.child(..)` | one child or many — auto-placed unless the child says otherwise |
| `.gap(..)` | space between tracks — see [Space](#space--gap-padding-margin) |
| `.align(..)` / `.justify_items(..)` | how items sit inside their cells, vertically / horizontally |

**On any child** — every widget has these, because placement is the item's own property:

| builder | means |
| --- | --- |
| `.column(..)` / `.row(..)` | CSS `grid-column` / `grid-row` |
| `.column_span(..)` / `.row_span(..)` | how many tracks it covers; `Span::All` for every one |
| `.area("title")` | CSS `grid-area` — the named region it belongs in |
| `.align_self(..)` / `.justify_self(..)` | where it sits inside its own cell |

⚠️ There is **no `.cell(child, …)` and no `.row(child)` on the grid.** A parent writing a position
into its child is the inversion this replaced — five positional numbers at the call site, and a
child that could not say anything about where it goes.

---

## Flex

A row or a column of **like things**. Reach for it when the children are interchangeable — a row of
buttons, a column of rows — and for anything else reach for [`Grid`](#grid).

```rust
Flex::row().gap("sm").align("center").child(icon).child(Label::new("nvim"))
Flex::column().gap("xs").child(rows)
```

- `Flex::row()` / `Flex::column()` — the direction, chosen at construction.
- `.gap(..)`, `.padding(..)`, `.align(..)`, `.justify(..)` — see below.
- `.child(..)` — one child or many; see [Children](#children--one-or-many).

**Baseline alignment is automatic.** A row of text runs puts them on one line even at different font
sizes — a name beside a dimmed `(program)` suffix, a heading beside a count. This looks *through*
transparent wrappers (`Visibility`, `KeyHint`, `Tooltip`) to find the run, and deliberately stops
there: a child that is a whole layout is not a word on the line. A row whose children are columns of
cards is arranged by its tracks, not by where some label inside happens to sit.

---

## Surface

A styled box: background, border, radius, glow. It owns **appearance**, not arrangement — put a
`Flex` or a `Grid` inside it and let that do the laying out.

```rust
Surface::new()
    .background(theme.colors.panel)
    .border(theme.colors.border, 1.0)
    .radius(6.0)
    .padding("sm")
    .child(Grid::new().template_row("auto 1fr").child([header, body]))
```

Read every value from the `Theme`. A literal colour will not follow a theme reload.

---

## Children — one or many

**One builder takes either shape**, everywhere children go:

```rust
Flex::column().child(header).child(body)          // one at a time
Flex::column().child([header, body])              // an array
Flex::column().child(rows.collect::<Vec<_>>())    // a Vec
Grid::new().row([icon, title, badge])             // same rule on a grid row
```

There is no plural spelling to discover. A `child` / `children` pair is two names for one idea: the
tests exercise one, the caller reaches for whichever they saw first, and the two drift. The
declarative side matches — `ViewNode::child(..)` takes one node or a list of them.

---

## Sizes

`width`, `height`, `min_width`, `max_width`, `min_height`, `max_height` all take any spelling:

```rust
.width("200px")     .width(200)        .width(Length::Px(200.0))
.width("50%")       .width(Length::HALF)
.width("auto")
```

| spelling | means |
| --- | --- |
| `"200px"`, `"200"`, `200` | fixed logical pixels |
| `"50%"` | a fraction of the parent |
| `"auto"` | the widget decides |

⚠️ **`Percent` is a fraction, `0.0..=1.0`** — half is `Percent(0.5)`, not 50.
⚠️ **Unset already stretches**, as in CSS. `.width(Length::FULL)` on a child that already fills says
nothing; deleting such a call is a judgement about its parent, not a mechanical sweep.

Named fractions: `Length::FULL`, `HALF`, `THIRD`, `QUARTER`.

---

## Space — gap, padding, margin

One property, every spelling:

```rust
.gap("sm")      .gap(8)      .gap("8px")
.padding("md")  .padding_x("sm")  .padding_top("xs")
.margin("lg")   .margin_y("sm")
```

| spelling | means |
| --- | --- |
| `"none"` `"xs"` `"sm"` `"md"` `"lg"` | a step of the theme's rhythm |
| `8`, `"8"`, `"8px"` | fixed logical pixels |

**Prefer the step.** It is a fraction of the inherited font, resolved when the tree is laid out, so
the arrangement breathes with a font, theme or zoom change and follows one with nothing rewritten. A
pixel count is tuned for one font size and wrong at every other. Use steps to group — a tight `"xs"`
inside a label-and-control couple, a roomier `"md"` between couples.

⚠️ **An unreadable space is none**, never a panic.
⚠️ **A gap is added OUTSIDE a percentage.** Siblings whose widths are percentages summing to 100%
overflow by exactly their gaps, so air between percentage-sized siblings belongs in their **padding**
(inside the border box). Between `grow` siblings a gap is exact, because grow divides what is left
after gaps.

---

## Alignment

`justify` is the main axis, `align` the cross axis — as in CSS. Both take the word a stylesheet
writes:

```rust
Flex::row().justify("space-between").align("center")
```

| `justify` | `align` |
| --- | --- |
| `"start"` `"center"` `"end"` | `"start"` `"center"` `"end"` |
| `"space-between"` `"space-around"` `"space-evenly"` | `"stretch"` `"baseline"` |

`"space-between"` and `"space_between"` both land. An unreadable keyword falls back to the default
rather than panicking.

Also: `.align_self(..)` overrides the parent's `align` for one child; `.justify_items(..)` and
`.justify_self(..)` are the grid's horizontal equivalents inside a cell.

---

## Shares

**A child that should take a part of its parent says so with `.flex(n)`** — CSS `flex: <n>`, said
on the child, exactly where CSS says it.

```rust
Flex::column()
    .child(docker.flex(1.0))       // ┐
    .child(workspaces.flex(3.0))   // ├ 1 : 3 : 1 of the column, whatever each one holds
    .child(notes.flex(1.0))        // ┘
    .child(git.flex(0.0))          //   as tall as its content, and no more
```

| written | means |
|---|---|
| `.flex(1.0)` beside `.flex(3.0)` | a quarter and three quarters — **whatever each holds** |
| `.flex(0.0)` | as big as its content, no part of the rest (CSS `flex: none`) |
| nothing | its own size — the parent gives it no part |

**It runs along the parent's direction**: height in a column, width in a row. One number either way.

**It belongs to the child, not the parent.** The parent writes no size per child and never counts
them, so a child added later — a plugin's dock appended to a sidebar — brings its own part and the
others keep theirs. A parent-side list of sizes (a template of `"1fr 3fr 1fr"`) is written for a
fixed number of children and is wrong the moment one more arrives.

**A child holding more than its part keeps its part.** It does not grow past it or push its
neighbours out; a scroll area inside it scrolls, anything else clips.

**In a `Grid` it does nothing**, as in CSS. The grid's own tracks size its cells, and a grid child is
placed with `.row` / `.column` / `.row_span` / `.column_span` / `.area` — see [Grid](#grid). None of
those size a child either: they choose which tracks it covers, and the template decides how big the
tracks are.

Declarative, the same word as a property:

```rust
use heca_view::build::*;
VStack::new().child(Label::new("docker")).flex(1.0)      // → { "flex": 1.0 }
```

### Why not `grow`?

`.grow(n)` is CSS `flex-grow` **alone**, and alone it does something else: it divides only the space
left over after every child took its content. Two children asked for 2:1 but holding different
amounts land at 606/294 in a 900px box, not 600/300, and a column of `grow(1.0)` children collapses to
its content instead of splitting the box.

`.flex(n)` is the three parts that make "n parts" true, applied for you against the real parent:

- **grow by `n`** — the weight;
- **start from zero** (`flex-basis: 0`) — so the ratio is of the whole box, not of the leftover;
- **allowed to shrink**, with its minimum size along the parent's direction at zero — so content
  larger than the part cannot hold it open.

Reach for `grow` only when you mean the leftover — a spacer that pushes its siblings to the ends.

⚠️ **Do not write the three by hand** (`.grow(n).basis(0.0).shrink(1.0)`). In a grid cell a zero
base size is a *definite zero height*, so a hand-written trio drew each dock as its title row and
nothing else. `.flex` knows which parent it is in; the trio does not.

⚠️ **"A share of the widest sibling" is not a flex part** — parts always fill their parent. That is a
**percentage** against a denominator chosen once at the top.

## Order

**A child that must sit somewhere other than where it was added says so with `.order(..)`** — CSS
`order`, said on the child.

```rust
Flex::column()
    .child(notes)                  // 0 — as added
    .child(docker.order(-1))       // before everything that said nothing
    .child(git.order([0, 5]))      // after every plain 0, before any 1
```

| written | means |
|---|---|
| nothing | `0` — laid out in the order it was added |
| `.order(-1)` / `.order(3)` | lower first; ties keep the order they were added |
| `.order([0, 5])` | compared a place at a time: after `0`, before `1` |

**Why a list.** Two siblings you do not own at `0` and `1` leave no whole number between them.
`[0, 5]` fits there without renumbering anyone. A missing place counts as `0`, so `1` and `[1, 0]` are
the same. Up to four places.

**One parser**, like sizes and spaces: `3`, `[0, 5]`, `"0 5"`, `"0, 5"` all read the same, in Rust, in
a plugin's description (`{"order": [0, 5]}`) and in config. An unreadable spelling is `0`, never a
panic.

**Visual only**, as in CSS. It moves where the child is laid out, not where it sits in the tree: paint
order, Tab order and the order the letter picker hands out letters stay the order children were
added.

**In a `Grid`** it orders what the grid places itself. A child that said where it goes (`.row`,
`.column`, `.area`) stays there.

**Menus use the same order.** A context-menu entry says where it sits with
`DropdownItem::order(..)` — the same number-or-list, compared the same way — and a whole block of
entries with `ContextMenuContribution::order`. (It was called `weight`; a description still using
that name reads.)

⚠️ **There is no index anywhere.** Nothing takes "put this at position 2": a sibling inserted before
it would silently move it. The ends (`append` / `prepend` on a region) and `.order` are stable.

---

## Traps

Each of these cost a real defect.

**Counting children to tell them apart.** `children.len() == 2` to decide there is a header. Two
things in the body and the first is mistaken for one. Use a template, a named area, or a `key`.

**Computing pixels in a composition.** The moment a composition multiplies model numbers by a scale
of its own it has taken over the layout engine's job — and then it owns every term: the window, the
overlay margin, the panel padding, the gaps, the row count, each row's height, the scroll centring.
Miss one and everything is wrong by exactly that term. Express it as a percentage of a shared
denominator and `grow` weights, and the engine answers it exactly at every window size.

**A gap outside a percentage.** See [Space](#space--gap-padding-margin).

**Naming a vertical box `Column`.** `Column` already means a column of panes in `heca-core`. Use
`Flex::column()`, or name the component for what it holds.

**Assuming a test covers a shape it never builds.** The map's overflow guard gave every column one
pane, so a column of two beside a column of one — the shape that actually broke — could not appear
in it. When a guard says "never exceeds its box", check which shapes it builds.

---

See also: [`widgets.md`](widgets.md) for the full widget catalog, and [`plugins.md`](plugins.md) for
the same values as a plugin writes them.
