# Animation Timing

> LazyClock, AdjustableClock, and VBlank-synchronized rendering.

## Requirements

1. Get animation state at a specific future time (for VBlank-predicted rendering)
2. Animations start at the moment of user action (not the next frame)
3. Within a single action processing, all time queries return the same value
4. Easy implementation of global slowdown

## Solution: LazyClock

```rust
struct LazyClock {
    timestamp: Option<Instant>,
}
```

- First query in an iteration: fetch system time, store it, return it
- Subsequent queries: return stored value
- Cleared at end of event loop iteration (before going to sleep)
- User action → fresh timestamp → all processing sees the same time

### AdjustableClock

Wrapper that applies slowdown factor to the timestamps. `set_unadjusted()` sets the raw underlying timestamp (e.g., for rendering at predicted VBlank time). `now_unadjusted()` gets the raw value.

## Shared Clock

Animations share an `Rc<Clock>` so overriding the time applies everywhere. Tests use independent clocks per test.
