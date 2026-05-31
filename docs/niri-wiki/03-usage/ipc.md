# IPC (niri msg)

> JSON-RPC over Unix socket for external control.

## Socket

The IPC socket lives at `$NIRI_SOCKET` (typically `/run/user/$UID/niri.$WAYLAND_DISPLAY.$PID.sock`).

## Usage

```sh
# List outputs
niri msg outputs
niri msg --json outputs   # Machine-readable JSON

# Execute action
niri msg action focus-workspace 2
niri msg action set-column-width "50%"

# Interactive pick
niri msg pick-window   # Click a window → get its ID and metadata

# Event stream (continuous updates)
niri msg event-stream         # Debug
niri msg --json event-stream  # JSON events

# Find key names
niri msg --json action
```

## Programmatic Access

Direct socket communication:

```sh
# Test with socat
socat STDIO "$NIRI_SOCKET"
# Send: "FocusedWindow"
# Receive: {"Ok":{"FocusedWindow":{...}}}

# Discover request format
socat STDIO UNIX-LISTEN:temp.sock
# Then in another terminal:
NIRI_SOCKET=./temp.sock niri msg action focus-workspace 2
# Then look at socat output: {"Action":{"FocusWorkspace":{"reference":{"Index":2}}}}
```

Requests are single JSON lines. Responses are single JSON lines wrapping `Ok` or `Err`.

## JSON Backwards Compatibility

- Existing fields and enum variants will not be renamed
- Non-optional existing fields will not be removed
- New fields and variants WILL be added
- Human-readable output (without `--json`) is NOT stable

## Event Stream

Since 0.1.9. Continuous state updates. Gives complete current state up-front, then incremental updates. Users implement bars/indicators without polling.
