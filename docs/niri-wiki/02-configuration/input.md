# Input Configuration

> Complete reference for the `input {}` section.

```kdl
input {
    keyboard { xkb { layout "us"; variant "colemak_dh_ortho"; options "compose:ralt,ctrl:nocaps"; } }
    touchpad { tap; natural-scroll; }
    mouse { accel-speed 0.2; }
    trackpoint { scroll-method "on-button-down"; }
    trackball { scroll-method "on-button-down"; }
    tablet { map-to-output "eDP-1"; }
    touch { map-to-output "eDP-1"; }
    disable-power-key-handling
    warp-mouse-to-focus
    focus-follows-mouse max-scroll-amount="0%"
    workspace-auto-back-and-forth
    mod-key "Super"
    mod-key-nested "Alt"
}
```

## Keyboard

- `xkb { layout, variant, options, model, rules }` — standard xkbcommon
- `xkb { file "~/.config/keymap.xkb" }` — custom keymap file (since 25.02)
- Empty `xkb {}` → fetch from systemd-localed (since 25.08)
- `repeat-delay 600`, `repeat-rate 25`
- `track-layout "global"` or `"window"` — per-window layout tracking
- `numlock` — enable numlock at startup (since 25.05)

## Touchpad

- `tap` — tap-to-click
- `dwt` — disable-when-typing
- `dwtp` — disable-when-trackpointing
- `drag` — tap-and-drag (since 25.05)
- `drag-lock` — lift finger briefly without dropping drag (since 25.02)
- `natural-scroll`
- `accel-speed -1.0..1.0`, `accel-profile "adaptive"` or `"flat"`
- `scroll-factor 1.0`, can be separate: `scroll-factor vertical=1.0 horizontal=-2.0`
- `scroll-method "two-finger"`, `"edge"`, `"on-button-down"`, `"no-scroll"`
- `scroll-button 273`, `scroll-button-lock` (since 25.08)
- `tap-button-map "left-right-middle"` or `"left-middle-right"`
- `click-method "button-areas"` or `"clickfinger"`
- `left-handed`
- `disabled-on-external-mouse`
- `middle-emulation`

## Mouse / Trackpoint / Trackball

Same acceleration/natural-scroll/scroll-* options. Trackpoint defaults to `scroll-method "on-button-down"`.

## Tablet / Touch

- `map-to-output "eDP-1"` — absolute mapping to specific output
- `map-to-focused-output` — map to focused output (since 26.04)
- `map-to-focused-window` — map to focused window (since next)
- `calibration-matrix 1.0 0.0 0.0 0.0 1.0 0.0`
- `left-handed` (tablets)

## General Settings

- `disable-power-key-handling` — let logind handle power button
- `warp-mouse-to-focus` — warp cursor to focused window. Optional `mode="center-xy"` or `"center-xy-always"` (since 25.05)
- `focus-follows-mouse` — auto-focus on hover. Optional `max-scroll-amount="10%"` (since 0.1.8) to limit scrolling.
- `workspace-auto-back-and-forth` — switching to same workspace goes back
- `mod-key "Super"`, `mod-key-nested "Alt"` — customize Mod (since 25.05)
