# heca documentation

Start with the [README](../README.md) to install heca and open it. Everything else is here.

## Using heca

| Page | What is in it |
| --- | --- |
| [What heca is](overview.md) | What a workspace compositor is, how it differs from tmux, and the NIRI ideas it follows. |
| [Keyboard commands](keyboard-commands.md) | Every default key, the docks' keys, scrollback and selection, the command palette. |
| [Sessions](sessions.md) | The background server, detach and reattach, following another window, saved sessions. |
| [Configuration](configuration.md) | `config.toml`: per-project settings, theme, appearance, fonts, notifications, agent status, the program catalog. |
| [Keybindings](keybindings.md) | How to bind a key: the five shapes, modes, surfaces, precedence. |
| [Actions](actions.md) | How an action is dispatched, how to add one, and the interaction policy. |

## Building heca and building on it

| Page | What is in it |
| --- | --- |
| [Developing heca](development.md) | Running the app and the examples, manual tests, continuous integration, frame timing. |
| [Architecture](architecture.md) | The technology stack, the crate map, how input becomes an action, the design decisions. |
| [Server and client](server-client-architecture.md) | The model behind sessions: the server, windows, the socket and RPC. |
| [Building on heca](extending-heca.md) | Adding to heca from another crate: providers, docks, agent drivers, services. |
| [Writing a plugin](plugins.md) | The declarative UI model and the plugin story. |
| [Widgets](widgets.md) | The catalog of every `heca-grid-ui` widget. Search it before building any UI. |
| [Layout](layout.md) | Which container to reach for, and how to say a size, a space, a track and an alignment. |
| [Chrome and UI](chrome-and-ui.md) | The pluggable chrome: regions, containers, the declarative UI. |
| [Hint architecture](hint-architecture.md), [Overlay design](overlay-design.md), [Surface compositor](surface-compositor.md), [Widget architecture](widget-architecture.md) | The design of one subsystem each. |

## Work in progress

What is planned, in progress or decided lives in the project planner (`.planner/`), loaded with
`/planner load`. There is no roadmap file.
