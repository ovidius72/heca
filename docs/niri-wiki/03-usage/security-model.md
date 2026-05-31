# Security Model

> Trust model, sandboxing, lock screen behavior.

## Trust Model

niri assumes unsandboxed programs on the host are **trusted**. Rationale: host programs can already access everything through LD_PRELOAD, PATH replacement, socket interposition, filesystem access, etc.

## What Access to the Wayland Socket Gives

- Screen recording (wlr-screencopy)
- Input emulation (virtual-pointer, virtual-keyboard)
- Clipboard access (wlr-data-control)
- Fullscreen overlay surfaces (layer-shell)
- Lock screen bypass (create lock surface, tell niri to unlock)

## What Access to the IPC Socket Gives

- Spawn Wayland clients (which have full access above)

## What D-Bus Access Gives

- Screen recording (portal interface)
- Full keyboard monitoring and emulation (accessibility interface)
- X11 `$DISPLAY` access → all X11 window content and input

## Running Untrusted Clients

Require proper sandbox that:
- Removes niri's IPC socket
- Prevents D-Bus access to host services
- Uses filtered Wayland socket (via `security-context-v1` protocol)

Flatpak satisfies all criteria. Filtering Wayland alone is NOT enough.

## Lock Screen

- Most actions disabled when locked
- `spawn` blocked except binds with `allow-when-locked=true`
- Quit is ALWAYS allowed (display manager catches you)
- If lock screen crashes: session remains locked (red screen) — spawn fresh lock screen to recover
