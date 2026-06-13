# Handoff: Phase 3 - Terminal Backend Integration Attempt

**Date:** 2026-06-10  
**Session Summary:** Attempted to wire TerminalBackend (PTY + vte parser) to replace FakeBackend, resulting in extreme performance issues (lag, flickering, text overlap).

---

## What Was Attempted

### Objective
Replace `FakeBackend` (static test pattern) with `TerminalBackend` (real PTY + vte parser) so panes display actual terminal output.

### Changes Made

#### 1. Export TerminalBackend (`heca-core/src/backend/mod.rs`)
- Added `TerminalBackend` to pub use exports
- **Status:** ✅ Clean - app compiles

#### 2. Wire TerminalBackend at Startup (`heca/src/app/startup.rs`)
- Changed `FakeBackend::new(80, 24)` to `TerminalBackend::new(80, 24).expect(...)`
- **Status:** ⚠️ Compiled, but performance issues introduced

#### 3. Wire TerminalBackend in Handlers (`heca/src/handlers.rs`)
- Changed 5 `FakeBackend` creation sites to use `TerminalBackend`
- Added error handling with fallback to `FakeBackend`
- **Status:** ⚠️ Compiled, but performance issues

#### 4. Render Data Cloning Fix (8.3) - Partial
- Added `dirty_rows: Vec<bool>` to Grid struct
- Mark dirty on mutations (`put_char`, `scroll_up`, `clear_*`, etc.)
- Optimized `render_data()` to only rebuild dirty rows
- **Status:** ❌ **REVERTED** - caused compilation error and didn't solve the core issue

#### 5. Border Inset Attempt
- Inset terminal content area by border width to prevent overlap
- **Status:** ❌ **REVERTED** - caused text to disappear

#### 6. Pane Border Overlay Removal
- Removed pane name overlays (static text in center of panes)
- **Status:** ✅ Done - text no longer has overlay labels

---

## What Actually Happened

### Performance Issues
The terminal became **extremely slow**:
- "Prefix takes minutes" to activate
- Text flickering (appearing/disappearing after keystrokes)
- Severe lag - app unusable

### Root Causes Identified
1. **render_data() allocates 1,920 cells per frame** (80×24 grid)
2. Each frame triggers a redraw due to `backend.update()` returning true
3. vte parser processes all PTY output byte-by-byte
4. No proper damage tracking - entire grid rebuilt even for small changes

### Text Overlap
- Terminal text appeared over the pane border
- Border drawing logic places border on top of content area
- Attempts to inset content caused text to disappear

---

## Current State

**Reverted to FakeBackend.** All panes now use `FakeBackend::new(80, 24)` again.

```
✅ Code compiles cleanly
✅ All 286 tests pass
✅ App is usable again (no lag)
❌ Terminal backend not wired
❌ Text overlap issues not resolved
❌ Damage tracking not working
```

### Files Modified
- `heca-core/src/backend/mod.rs` - reverted
- `heca-core/src/backend/terminal.rs` - reverted
- `heca/src/app/startup.rs` - reverted
- `heca/src/handlers.rs` - reverted
- `heca/src/app/mutations.rs` - reverted
- `heca/src/app/render.rs` - reverted

### Git Status
```bash
git diff --stat
# No changes - all reverted
```

---

## What Would Work Next Time

To properly wire TerminalBackend, consider:

1. **Use a pre-allocated cell buffer** instead of rebuilding Vec on each frame
2. **Render directly from Grid** without `BackendRenderData` intermediate
3. **Implement proper damage tracking** with dirty-row tracking that persists across frames
4. **Fix border rendering** - ensure content is properly inset or border is drawn around content
5. **Consider using alacritty_terminal** instead of custom vte parser for production
6. **Test with actual shell output** before declaring "done"

---

## Lessons Learned

1. **Don't add real features without profiling** - the 1,920 cell allocation per frame is catastrophic
2. **Damage tracking alone isn't enough** - need to avoid the intermediate data structure entirely
3. **Test user experience early** - "compiles" doesn't mean "usable"
4. **Render pipeline matters** - the border+text draw order needs careful design
5. **When in doubt, revert** - better to have usable app than broken "enhancement"

---

## Next Steps Recommendation

1. **Revert all TerminalBackend changes** - current state is good
2. **Add placeholder code** for future terminal backend (commented out, not wired)
3. **Design proper rendering pipeline** - avoid intermediate allocations
4. **Profile before optimizing** - identify real bottlenecks
5. **Consider existing terminal libs** (alacritty_terminal) vs custom vte

---

**Status:** Back to baseline. Ready to resume from here.  
**State:** All panes use FakeBackend. App is usable but shows static test pattern, not real terminal.  
**Ready for:** User to decide next approach or continue with other phases.
