# Neovim GUI — first plan (idea stage)

> **Status: idea stage.** This document seeds the direction and records the first
> decisions and observations. Details come later — do not treat anything here as
> locked beyond the three decisions explicitly marked as taken.

## Vision

heca embeds **Neovim as the text engine** and renders the editor itself as a
native GUI (a Neovim GUI client, in the spirit of Neovide / goneovim). The goal
that drives this is **rich images and markdown handled in the GUI**, without the
terminal inline-image protocols.

## Why this architecture (vs. terminal-escape images)

Going through Neovim's UI protocol instead of terminal escape sequences removes
the whole class of problems we hit with inline terminal images:

- No Kitty / iTerm2 / Sixel / base64 / Unicode-placeholder limitations.
- heca **draws** the editor, so it knows the exact geometry — windows, splits,
  scroll position, folds, cursor. That geometric knowledge is exactly what lets
  us place native images and rich content precisely and **track them as the user
  scrolls** (the part that is always fragile with terminal escapes).
- It reuses what heca already has: the GPU cell renderer, the `ImageRenderer`,
  and the overlay/popup widgets.

## How a Neovim GUI works (concretely)

- Launch `nvim --embed`; communicate over **msgpack-RPC** (Rust: `nvim-rs`).
- `nvim_ui_attach` makes Neovim emit **redraw** events describing the grid
  (cells, highlight groups, cursor, scrolling). The client renders them.
- heca renders that grid by reusing its GPU cell-rendering stack (glyph atlas,
  damage, ligatures). The grid is conceptually close to the terminal grid heca
  already draws — the **new** part is the "driver" that interprets the msgpack
  redraw protocol instead of terminal escape sequences.

## Decisions taken (this session)

1. **Editor = a pane type.** The editor is a heca pane, alongside terminal panes.
   Multiple editor panes can be open at once, each backed by its own embedded
   `nvim --embed` instance (same way multiple terminals work).
2. **`ext_multigrid` = ENABLED.** See the recommendation below.
3. **Images = dedicated RPC channel.** Image placement does not piggyback on the
   redraw stream; it flows over a separate, dedicated channel.

## `ext_multigrid` — recommendation (ENABLE)

**Recommendation: enable `ext_multigrid`.**

- **Why:** with multigrid, each window gets its own grid plus explicit geometry
  (`win_pos` / `win_float_pos` for position+size) and a per-window viewport
  (`win_viewport`: topline / botline / curline). That per-window geometry +
  viewport is precisely what we need to **anchor an image to a buffer/window
  position and follow it across scroll**, and it also enables smooth per-window
  scrolling (Neovide-style). Without multigrid, Neovim sends one global grid and
  we'd have to infer window boundaries and buffer-line mapping ourselves — fragile
  for image anchoring.
- **Cost:** more complex rendering — we manage N grids, float z-ordering, and
  compositing instead of one flat grid. Accepted: it is the enabler for the
  image/markdown goal, which is the whole point.
- **Pair it with:** `ext_linegrid` (the modern line-based redraw protocol, the
  default we want) and the `win_viewport` events (for image anchoring / scroll
  tracking). `ext_messages` / `ext_cmdline` / `ext_popupmenu` are optional and can
  come later if we want a fully native cmdline/messages UI.

## Images — dedicated channel

- A small **Lua component inside the embedded Neovim** sends image commands to
  heca over a dedicated RPC channel: **place / move / delete**, carrying the
  anchor (buffer id, window/grid, line, column), the source (path or bytes), and
  sizing.
- heca decodes the image (reusing the existing `ImageRenderer` + decode pipeline)
  and draws it **natively over the editor grid**, repositioning it on
  `win_viewport` / scroll / redraw and **clipping it to the owning window**.
- Keeping this off the redraw stream gives a clean separation and room to evolve
  the image feature independently.

## Markdown (later)

- heca reads the **buffer content via RPC** and renders a native rich preview
  (overlay or split), or renders markdown buffers inline with native widgets.
- Trigger mechanism (action / keybinding) to be decided later.

## What carries over from the terminal image work

- `ImageRenderer` + the image decode pipeline → direct reuse for editor images.
- GPU cell-rendering know-how (glyph atlas, damage-based redraw).
- grid-ui overlay/popup widgets → reuse for markdown and image viewers.
- **New work:** the redraw-protocol driver (msgpack redraw vs. terminal escapes).

## Rough phases (high level — to detail later)

1. Embed `nvim --embed` + RPC + `ui_attach`; render a single grid end-to-end.
2. Editor-pane integration: multiple instances, lifecycle, focus, input routing.
3. `ext_multigrid`: per-window grids, geometry, `win_viewport`, float z-order.
4. Input: full key encoding to `nvim_input` (modifiers, special keys), mouse.
5. Image layer: dedicated RPC channel + native draw + scroll tracking + clipping.
6. Markdown layer.

## Open questions (revisit when detailing)

- Float / popup z-ordering and the compositing model with multigrid.
- Mapping a buffer line to a grid row for image anchors (use `win_viewport`
  topline as the reference).
- Keyboard: complete key encoding to `nvim_input`; how editor input coexists with
  heca's tmux-like prefix bindings.
- Config: per-pane Neovim config vs. shared; how user `init.lua` is loaded.
- Performance: many editor instances + images on screen at once.
- Relationship to terminal panes: shared chrome, switching, addressing.
- The Lua side: ship a small built-in heca Neovim plugin, or require user setup?

## Note

The terminal inline-image work (capture + `ImageRenderer` + the PTY pixel-size /
WezTerm-identity fixes) stays valuable for **terminal** panes (e.g. Yazi in a
terminal pane) and feeds this effort. It is not superseded by the editor pane.
