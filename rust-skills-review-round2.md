# Rust-Skills Review Round 2

**Date:** 2026-05-31  
**Scope:** `actions.rs`, `sidebar.rs`, `input.rs`, `app_state.rs`, `main.rs` (focus/tracking), `session.rs` (remove_workspace)  
**Tests:** 44 passing (32 heca + 7 heca-config + 5 heca-core)  
**Clippy:** 0 warnings

---

## Critical Issues (0)

No critical safety or correctness bugs found.

---

## High Issues (1)

### HI-1: `collect_all_pane_candidates` / `collect_all_column_candidates` — ASCII overflow with >26 panes

**File:** `heca/src/main.rs`  
**Code:**
```rust
let ch = (b'a' + candidates.len() as u8) as char;
```

**Problem:** With more than 25 panes/columns, `b'a' + 25 = b'z'`. The 26th pane would produce `{` (ASCII 123), which is not a letter. Beyond 230 panes, this panics in debug mode on `u8` overflow.

**Impact:** PaneSelect/Swap mode breaks with large sessions.  
**Fix:** Use a wrapping or extended alphabet:
```rust
const ALPHABET: &[char] = &[
    'a','b','c','d','e','f','g','h','i','j','k','l','m',
    'n','o','p','q','r','s','t','u','v','w','x','y','z',
    'A','B','C','D','E','F','G','H','I','J','K','L','M',
    'N','O','P','Q','R','S','T','U','V','W','X','Y','Z',
];
let ch = ALPHABET[candidates.len() % ALPHABET.len()];
```

---

## Medium Issues (4)

### MED-1: `sidebar_hit_test` expanded mode doesn't cap at visible lines

**File:** `heca/src/sidebar.rs` — `sidebar_hit_test()`  
**Problem:** In expanded mode, `fi = scroll_offset + line_index` is computed without checking `line_index < visible_lines`. If the user clicks in the empty space below the last rendered item (but still within sidebar bounds), and `fi < flat_items.len()`, it returns a non-visible item.

**Fix:** Add visible lines check:
```rust
let visible_lines = (sidebar_height / ITEM_HEIGHT) as usize;
let fi = tree.scroll_offset + line_index;
if fi < tree.flat_items.len() && line_index < visible_lines {
    return Some(fi);
}
```

---

### MED-2: `remove_workspace` missing "active == removed" test case

**File:** `heca-core/src/layout/session.rs`  
**Problem:** No test for when `active_workspace_idx == removed_idx`. The code handles it correctly (falls through both conditions, active stays pointing at the workspace that shifted into the removed slot), but this behavior is untested.

**Fix:** Add test:
```rust
#[test]
fn test_remove_workspace_active_is_removed() {
    let mut session = make_session_with_workspaces(3);
    session.active_workspace_idx = 1;
    assert!(session.remove_workspace(1));
    assert_eq!(session.workspaces.len(), 2);
    // active stays at 1, which now points to the former workspace 2
    assert_eq!(session.active_workspace_idx, 1);
}
```

---

### MED-3: `WmAction` missing `Eq` + `Hash` derives

**File:** `heca/src/input.rs`  
**Problem:** `WmAction` derives `PartialEq` but not `Eq` or `Hash`. Since all variants are unit types, it could safely derive both. This limits use in `HashMap`/`HashSet` and requires `PartialEq` bounds where `Eq` would suffice.

**Fix:** `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]`

---

### MED-4: `ActionDescriptor` could derive `Copy`

**File:** `heca/src/actions.rs`  
**Problem:** `ActionDescriptor` contains only `&'static str` and `ActionCategory` (which is `Copy`). Deriving `Copy` would eliminate `.clone()` costs and allow passing by value without ownership concerns.

**Fix:** `#[derive(Debug, Clone, Copy)]` on `ActionDescriptor`.

---

## Low Issues (6)

### LOW-1: `SidebarColEntry` / `SidebarWsEntry` empty impl blocks

**File:** `heca/src/sidebar.rs`  
**Fix:** Remove empty `impl` blocks or add `#[derive(...)]` and drop the impl.

---

### LOW-2: `render_sidebar_collapsed` unused `_height` parameter

**File:** `heca/src/sidebar.rs`  
**Note:** The underscore prefix suppresses the warning, but the parameter is genuinely unused. Consider removing it from the signature if scrolling is never needed in collapsed mode. However, keeping it for API symmetry is reasonable.

---

### LOW-3: `action_from_name` and `action_priority` are private but untested at module boundary

**File:** `heca/src/input.rs`  
**Note:** Tests cover these via `KeyBindings` integration, but direct unit tests for `action_from_name("focus_left")` and `action_priority` ordering exist and are sufficient.

---

### LOW-4: `focus_pane_by_id` calls `sync_focus` even when pane not found

**File:** `heca/src/main.rs`  
**Note:** This is by design — it ensures `AppState.focused_pane` stays in sync with `Session` even on miss. The behavior is correct, but a code comment explaining this would help future readers.

---

### LOW-5: `AppState` has all-public fields — invariant enforcement relies on convention

**File:** `heca/src/app_state.rs`  
**Note:** The `focus_pane_by_id` / `sync_focus` / `switch_workspace_tracked` canonical functions maintain invariants, but nothing prevents direct mutation. For a binary this is acceptable, but consider `pub(crate)` for fields that should only be touched by `main.rs`.

---

### LOW-6: `ActionCategory` derives `Hash` but is never used as a HashMap key

**File:** `heca/src/actions.rs`  
**Note:** Harmless, but unnecessary. Could be removed to reduce generated code. Not worth fixing unless clippy complains.

---

## Positive Findings

1. **No `unsafe` code** in any of the reviewed modules.
2. **No `unwrap`/`expect` in production code** (only in tests, which is fine).
3. **Canonical focus functions are respected** — all focus paths go through `focus_pane_by_id()`.
4. **Clippy clean** — 0 warnings across workspace.
5. **Good test coverage** — 44 tests covering action registry, sidebar tree, input parsing, session operations, and app state.
6. **`remove_workspace` index fix is correct** — properly decrements `active_workspace_idx` when it's after the removed index.
7. **Prefix timeout is sound** — uses `Instant::elapsed()` which is monotonic and handles system time changes correctly.
8. **`sidebar_hit_test` handles both expanded and collapsed modes** — good separation of concerns.

---

## Recommended Fixes (Priority Order)

| Priority | Issue | File | Effort |
|----------|-------|------|--------|
| High | HI-1: ASCII overflow in candidate generation | `main.rs` | Small |
| Medium | MED-1: Hit test visible lines cap | `sidebar.rs` | Small |
| Medium | MED-2: Missing active==removed test | `session.rs` | Small |
| Medium | MED-3: Add Eq+Hash to WmAction | `input.rs` | Tiny |
| Medium | MED-4: Add Copy to ActionDescriptor | `actions.rs` | Tiny |
| Low | LOW-1: Remove empty impl blocks | `sidebar.rs` | Tiny |
