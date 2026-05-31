# Redraw Loop

> RedrawState machine, frame scheduling, VBlank handling.

## State Machine

```
Idle → Queued → WaitingForVBlank → Idle
                → WaitingForEstimatedVBlank → Idle
```

1. `Idle` — output doesn't need repaint
2. `Queued` — `queue_redraw()` called; pending redraw
3. `WaitingForVBlank` — frame submitted, waiting for VBlank to submit next
4. `WaitingForEstimatedVBlank` — no damage, but throttling frame callbacks to max once per refresh cycle

## Key Behavior

- Only one frame can be submitted to an output at a time (TTY limitation)
- Must wait for VBlank before submitting the next
- Without damage: timer fires at estimated next VBlank → return to Idle
- Throttling prevents applications from continuously redrawing without actual damage
