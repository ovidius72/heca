# Plan 03: Configurable Prefix, Global Keys, Chords, and Command Spawning

**Created:** 2026-06-01
**Status:** In Progress
**Depends on:** Plan 02 (Action Registry + RPC Foundation)
**Goal:** Make the prefix key configurable, add non-prefix global bindings, support multi-key chord sequences, and spawn programs from keybindings.

---

## Feature 1: Configurable Prefix Key

**Current:** `Ctrl+B` is hardcoded in the keyboard handler via multiple checks (logical key text `\u{2}`, Character `"\u{2}"`, physical key `KeyB` + ctrl modifier).

**Target:** User sets `prefix_key = "Ctrl+b"` in `config.toml`.

**Config format:**
```toml
[general]
prefix_key = "Ctrl+b"    # default
# prefix_key = "Alt+Space"
# prefix_key = "Ctrl+a"    # tmux refugees
# prefix_key = "Super+b"   # macOS-friendly
```

**Implementation:**
1. Add `prefix_key: String` to `GeneralConfig` (default `"Ctrl+b"`)
2. Parse prefix key string into a `KeyCombo` at init time
3. Store `prefix_combo: KeyCombo` in `AppState`
4. Replace all hardcoded `is_prefix` checks with `combo == state.prefix_combo`

**Files:** `heca-config/src/theme.rs`, `heca/src/app_state.rs`, `heca/src/main.rs`

---

## Feature 2: Non-Prefix Global Keys

**Current:** In `Normal` mode, every key except `Ctrl+B` is forwarded to the focused pane's terminal backend.

**Target:** Certain keys trigger WM actions directly without requiring prefix.

**Config format:**
```toml
[global]
# Workspace switching with Alt+digit (like i3/sway)
focus_workspace_1 = "Alt+1"
focus_workspace_2 = "Alt+2"
focus_workspace_3 = "Alt+3"

# Sidebar toggle with Super (macOS-friendly)
toggle_sidebar = "Super+b"

# Fullscreen with F11
# toggle_fullscreen = "F11"
```

**Resolution order in Normal mode:**
```
Keypress in Normal mode:
  1. Is it the prefix key? → Enter Prefix mode
  2. Is it in global keymap? → Execute action, stay Normal
  3. Forward to focused pane's terminal backend
```

**Warning:** Global keys are **stolen from terminal apps**. If `Alt+1` is bound, vim/emacs in a pane never sees it. Users must pick modifiers that don't conflict (Alt, Super, F-keys).

**Files:** `heca-config/src/theme.rs`, `heca/src/keymap.rs`, `heca/src/main.rs`

---

## Feature 3: Chord Sequences

**Current:** Prefix mode accepts a single key, then exits.

**Target:** Support sequences like `prefix → w → 1` (switch to workspace 1), `prefix → w → 2`, etc.

**Config format:**
```toml
[[chord]]
sequence = ["w", "1"]
action = "focus_workspace"
args = { ws_idx = 0 }

[[chord]]
sequence = ["w", "2"]
action = "focus_workspace"
args = { ws_idx = 1 }

[[chord]]
sequence = ["w", "c"]
action = "create_workspace"
```

**State machine:**
```
Normal ──prefix──► Prefix ──w──► Chord { matched: ["w"], candidates: [...] }
                                     │
                                     ├── 1 ──► execute FocusWorkspace(0) ──► Normal
                                     ├── 2 ──► execute FocusWorkspace(1) ──► Normal
                                     ├── c ──► execute CreateWorkspace ──► Normal
                                     ├── Esc ──► cancel ──► Normal
                                     └── timeout (500ms) ──► cancel ──► Normal
```

**Implementation:**
1. `ChordRegistry` with a trie (prefix tree) of key sequences → actions
2. `InputMode::Chord { matched: Vec<KeyCombo>, candidates: Vec<...> }`
3. After prefix key, check if the next key starts any chord
4. If exact match (single candidate, sequence complete) → execute
5. If partial match (multiple candidates share prefix) → stay in chord mode
6. If no match → cancel, optionally forward keys to terminal

**Simplification for MVP:** Hardcode `prefix → w → digit` for workspace switching. Configurable chords come later.

**Files:** `heca/src/chords.rs` (NEW), `heca/src/input.rs`, `heca/src/main.rs`

---

## Feature 4: Command Spawning from Keybindings

**Current:** All panes use `FakeBackend` (no real PTY). No way to spawn external programs.

**Target:** Keybindings can spawn programs in new panes.

**Config format (herdr-inspired):**
```toml
[[keys.command]]
key = "prefix+Shift+g"
command = "lazygit"

[[keys.command]]
key = "prefix+Shift+f"
command = "fish"
```

**Or integrated into existing keybindings:**
```toml
[commands]
lazygit = "Shift+g"    # prefix+Shift+g spawns lazygit
fish = "Shift+f"       # prefix+Shift+f spawns fish
```

**Implementation:**
1. New `WmAction::SpawnCommand { command: String }` variant
2. Handler uses `portable-pty` (already in Cargo.toml) to spawn a PTY
3. Creates a `TerminalBackend` (not `FakeBackend`) for the new pane
4. Wire into keybinding system — action names like `spawn_lazygit` mapped to `WmAction::SpawnCommand`

**Note:** Currently `heca` only has `FakeBackend` wired. Adding real PTY support means:
- Creating `TerminalBackend` that wraps `portable_pty::MasterPty`
- Integrating `alacritty_terminal` VTE parser for output
- This is significant — basically "Phase 3 — The Content" from the roadmap

**Simplification for this plan:** Add the `WmAction::SpawnCommand` variant and handler skeleton. Full PTY wiring is a separate plan. For now, the handler can log the command and create a FakeBackend with the command name as title.

**Files:** `heca/src/input.rs`, `heca/src/handlers.rs`

---

## Phase Breakdown

| Phase | Feature | Effort | Files |
|-------|---------|--------|-------|
| 1 | Configurable prefix key | Small | `theme.rs`, `app_state.rs`, `main.rs` |
| 2 | Global (non-prefix) keys | Small | `theme.rs`, `keymap.rs`, `main.rs` |
| 3 | Chord sequences (hardcoded workspace chords) | Medium | `chords.rs` (NEW), `input.rs`, `main.rs` |
| 4 | Command spawning skeleton | Small | `input.rs`, `handlers.rs` |
| 5 | Integration & tests | Medium | All above |

---

## Config Example (all features)

```toml
[general]
prefix_key = "Ctrl+b"

[keybindings]
focus_left = ["h", "Left"]
focus_right = ["l", "Right"]

[global]
focus_workspace_1 = "Alt+1"
focus_workspace_2 = "Alt+2"
toggle_sidebar = "Super+b"

[chords]
# prefix → w → digit switches to workspace
workspace_1 = ["w", "1"]
workspace_2 = ["w", "2"]
workspace_new = ["w", "c"]

[commands]
lazygit = "Shift+g"
fish = "Shift+f"
```

---

## Test Plan

- `test_prefix_key_parse` — "Ctrl+b" → KeyCombo { key: "b", ctrl: true }
- `test_global_binding` — Alt+1 resolves to FocusWorkspace(0) in Normal mode
- `test_chord_workspace_switch` — prefix+w+1 triggers FocusWorkspace(0)
- `test_chord_timeout` — prefix+w, wait 500ms → cancels to Normal
- `test_chord_cancel` — prefix+w+Esc → cancels to Normal
- `test_command_spawn` — WmAction::SpawnCommand creates pane with correct title
