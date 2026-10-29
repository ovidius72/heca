# Building on heca — adding to it from another crate

`heca` is a library plus a one-line program. Plain heca is:

```rust
fn main() { heca::run(); }
```

A program built on heca says who it is, adds what it wants through heca's **existing doors**, and
calls the same `heca::run()`. Nothing here is a second mechanism: each door fills the one heca's own
parts use, so what is added is reachable from a key, a menu, the palette and RPC with nothing written
for any of them. A worked, runnable example: `heca/examples/extend_heca.rs`
(`cargo run -p heca --example extend_heca`).

```rust
fn main() {
    let demo = heca::extension("demo");
    // Add actions, layers and pane decorations through `demo`.
    heca::run();
}
```

**Docks are contributed through Providers.** The canonical
[Provider and startup region reference](widgets.md#providers-and-startup-regions) documents adding,
removing and ordering them from native Heca/library code, plus the sandbox-plugin startup parity
requirement and current availability.

## Extension identity, registrations and shared terminals

`heca::extension("demo")` **declares the extension's identity** and returns its handle. It creates
no terminal or widget. Declare it once; pass or clone that handle wherever it is needed. Calling
`heca::extension("demo")` again does not retrieve the first handle: duplicate declarations are
refused. Its items use qualified names such as `demo.shell` or `demo.build`, so different extensions
can use the same short name without sharing an item accidentally.

The handle's methods have different jobs; a shared namespace does not give them all the same
creation or lifetime behavior:

| call | what it does |
|---|---|
| `Label::new("Notes")`, `Pane::new("console")` | construct UI components to put in a tree; they do not look up an extension-wide instance by that text |
| `Terminal::new("shell")` | get or create the app's named terminal `heca.shell`, without an extension handle |
| `demo.terminal("shell")` | **get or create** the named terminal declaration; repeated calls return handles sharing that terminal's session |
| `demo.action("build")…run(handler)` | register a named action and its handler; it does not retrieve an existing widget or execute the action |
| `demo.layer("console").view(build)` | register a named surface builder; the host calls the builder again when rebuilding that surface, rather than retaining one cached widget tree |

For terminals, the first call creates the declaration and the returned component handle; the host
starts the shell or command from that declaration. The native constructor and extension method
share this behavior, with separate owner namespaces:

```rust
let demo = heca::extension("demo"); // identity only; no shell is created
let a = demo.terminal("shell");    // first call: declares demo.shell
let b = demo.terminal("shell");    // another handle sharing a's session
let c = demo.terminal("logs");     // a different name: a separate terminal
let another_view = a.clone();      // also shares a's session; no new process
let native = heca::Terminal::new("shell"); // heca.shell, separate from demo.shell
```

The name identifies the session; `.title(...)` only changes its display title. Configure
`.command(...)` and `.cwd(...)` before the process starts. Applying those builders to an already
running terminal does not restart it or change that process's command or working directory.
Rebuilding a dock or reopening a layer can therefore construct fresh UI components while
`demo.terminal("shell")` or `Terminal::new("shell")` reconnects to the same respective session.
Hiding or removing that UI does not end the named terminal; a killed terminal stays ended when
requested again under the same owner and name.
See the canonical [Terminal reference](widgets.md#terminal) for builders, input and lifetime rules,
and [Terminal processes](#terminal-processes) for host startup and managed-pane ownership.

This is named lookup and sharing, rather than a per-component memoized reference: different UI
components asking for `demo.shell` share it. Ordinary widgets still have their own identity and
state; they do not require an extension handle merely to be constructed or placed in a tree.
These calls describe the native extension API; see [the plugin guide](plugins.md) for the
described/sandbox path and its current availability.

## Providers and docks at startup

The canonical [Provider and region API reference](widgets.md#providers-and-startup-regions) explains
how to add, remove and order docks, the required `Provider` methods, placement identity, startup
cutoff and native/plugin availability. Use that reference for the operation contract.

A complete native implementation is [`heca/examples/extend_heca.rs`](../heca/examples/extend_heca.rs):
its `Notes` provider contributes a dock containing a label and a Terminal to `sidebar.right`.

## The rules

- **The name is the program's, said once.** `heca::extension("demo")`. `heca` is reserved for the app;
  a name with a dot is refused; a second extension with the same name is refused. A short name with a
  dot is refused too. Every refusal is *said* (once, to whoever is writing the program), never silent.
- **What is added is a default.** Where a dock sits and whether an item shows is the user's to decide.
  A dock they move stays where they put it. Pane buttons, chips and lines are listed **by name** in
  `[appearance.pane] title_actions`, `title_segments` and `[appearance.sidebar] pane_lines`: while a
  list is still heca's own, what you added shows after it; once the user writes their own list only
  their list counts, and they are told **once** that an unlisted item exists.
- **Register before `run()`.** Startup registrations (actions, layer builders, decorations,
  drivers and Provider/region changes) must be made before the app takes their queues; late
  additions are reported and dropped. A registered dock or layer may subsequently call
  `demo.terminal("shell")` while building its content to get or create its named terminal.
- **Words are an English fallback** (`.label(..)`, `.description(..)`) until the app-wide text system
  lands.

## The doors

| you add | with | side | notes |
|---|---|---|---|
| a dock | `heca::regions("sidebar.left").append(dock)` | client | a `Provider`; placed by the user like any dock |
| an action | `demo.action("s")…run(f)` | server | policy defaults to `AlwaysAllowed` (refused while a floating pane is active); say `.policy(..)` otherwise; `.destructive()` asks first |
| an overlay | `demo.layer("s").view(f)` | client | opened with `toggle_layer name=demo.s` from a key, menu, palette or RPC; built again each time it opens |
| a pane header button | `demo.pane_button("s")` | client | runs the action `demo.s`; icon, words, tooltip and danger come from it; if it declares an integer `pane` argument (`heca::PANE_ARG`) it receives the pane's id |
| a terminal | `demo.terminal("s")` | client | the terminal `demo.s` in any tree you build (a dock, an overlay): `.command(..)`, `.cwd(..)`, `.title(..)`; the same name is the same running process, and closing the dock never ends it |
| a pane header chip | `demo.pane_chip("s", f)` | client | `f(&PaneFacts) -> Option<PaneChip>`; re-run when the pane's facts change |
| a sidebar row line | `demo.pane_line("s", f)` | client | `f(&PaneFacts) -> Option<PaneLine>` |
| a terminal agent driver | `demo.agent_driver(MyDriver::new())` | terminal owner | implements `heca::agent::AgentDriver`; scoped as `demo.<driver id>` and used by the same launch/report path as built-in drivers |

Dock and layer factories return ordinary components. For a dock, implement
`Provider::build_contribution` with `ContainerContribution::new(self, |cx, build| widget)`;
for a named layer use `demo.layer("details").view(|cx| widget)`. Neither needs author-side boxing,
casts or storage conversions. The complete compiling program is
[`extend_heca.rs`](../heca/examples/extend_heca.rs); the shared reference is
[Providers and startup regions](widgets.md#providers-and-startup-regions).

## Terminal agent drivers

Implement `heca::agent::AgentDriver`: declare executable names, prepare a temporary reporting
connection, and decode explicit `AgentInput` reports. Register it
before `heca::run()`:

```rust,ignore
let demo = heca::extension("demo");
demo.agent_driver(MyDriver::new());
heca::run();
```

`AgentSetup` adds arguments and environment for that invocation; the original arguments remain.
Read current command settings from `AgentLaunchContext.environment` and `cwd`. Launch setup never edits
personal settings. A side-channel driver writes newline-delimited reports to the supplied `endpoint`; an OSC
driver reads `TerminalNotification` inputs. Reports use open `AgentStatus` identifiers. The configured
byte and queue limits apply to custom drivers too.

For persistent setup, implement `AgentDriver::integration(&IntegrationContext)`. Return an
`AgentIntegration` with a version and `IntegrationResource` entries: owned files, appended JSON/TOML
array entries, an owned TOML value, or a native TOML feature switch. This method describes the installation; it does not
write files. The shared installer checks ownership, preserves existing entries, writes a recoverable
batch, and implements inspection, explicit updates and uninstall. Returning `None` means the driver
needs no persistent files.

`heca::agent::integration::Integrations::machine_local()` builds that service from normal Heca
configuration, registered drivers and user locations, including disabled drivers. Construction
installs nothing and returns configuration errors. `heca::agent::integrations(&config.settings.agents)`
accepts an existing configuration; `integrations_in` also accepts explicit locations for embeddings
and isolated tests. These operations work without a window or session server. An
installation update does not re-enable reporting the user disabled. Native hook trust remains the
native program's responsibility.

The command line calls this same installer before opening a window:

```sh
heca list integrations
heca inspect integration <program>
heca install integration <program>
heca update integration <program>
heca uninstall integration <program>
```

Use `list integrations` to discover registered program ids, including an embedding's added
drivers. Every command accepts `--json` for structured output. Invalid arguments, settings and
failed changes return a nonzero exit code; they never open the GUI. These local operations need
no running session server. Installation is always explicit.

For a program with session-local MCP configuration,
`context.reporter.mcp_server(context.endpoint.as_deref(), &reports)` describes the shared stdio
reporting helper. `reports` maps native event names to `AgentState` values. The helper supplies
the connection's environment; adapters do not need to know its environment-variable names.
An installed registration passes `None` for the endpoint and stays inactive outside Heca. Native hooks call
`ReportingHelper::TOOL` with `{ "event": "Stop", "turn_id": "…" }`; `turn_id` is optional.
Disabled events are ignored, and a closed terminal's report file is never recreated.

Codex uses this connection so its shared background server cannot reuse another pane's reporting
environment. Its persistent MCP hooks keep the normal native trust review. Codex does not support
MCP hooks for `SessionEnd`; normal process exit clears the pane's agent state. Its `SessionStart`
hook may run before MCP is ready, so the first status may arrive with the first prompt instead.
See the [native hook lifecycle](https://learn.chatgpt.com/docs/hooks#execution-and-lifecycle).

### Driver logos and session identity

Every driver can supply its own mark through the same optional method:

```rust,ignore
fn icon(&self) -> Option<heca::agent::AgentIcon> {
    Some(heca::agent::AgentIcon::svg(include_str!("assets/logo.svg")))
}
```

`AgentIcon::svg` takes SVG contents; `AgentIcon::file` takes a local SVG or bitmap path. A native
driver can embed artwork with `include_str!`, whose path is relative to its Rust source file. A
packaged driver resolves a file against its installed asset directory. The shared `Image` widget
owns loading, fitting, rendering and load-error fallback. There is no harness-name switch in the row.
The four built-in marks use this same contract; their sources and licenses are beside the assets.
Use explicit SVG fills for brand artwork. `currentColor` deliberately follows the surrounding
text color; fixed brand fills and gradients keep their own colors, including in selected rows.

The report reader attaches the registered mark after decoding a native report. Lifecycle templates
do not repeat SVG contents in every event or in the launch environment. A user can override a mark
with `icon = "/path/to/logo.svg"` under that program's configuration; this works for added drivers too.

`AgentState` also accepts optional `model`, `session_id` and `session_title` values supplied by a
driver. The row shows a reported model or the configured program name, then the reported session
title or the pane name. It never invents a model or title. Omitted model/title metadata is retained
only when the driver and an explicit, nonempty session identity match; a different session clears
it. Missing fields remain compatible with older reports. Native adapters must forward the metadata
their program actually supplies.

Claude command hooks forward the native `model`, `session_id` and custom `session_title`. Codex
MCP hooks forward `model` and `session_id`; they do not invent a title. Pi uses its extension
context and session manager. OpenCode uses the active client's session, including its model id
and title, and ignores other sessions' events.

Command-hook drivers can use `context.reporter.command_hook(driver_id, native_event)` in their
installed command registration. The host reads bounded JSON from stdin and selects only `turn_id`,
`model`, `session_id` and `session_title`; it does not read transcripts or retain prompts. The
configured event map supplies the status and driver identity. Missing reporting environments,
disabled events and closed terminal endpoints do not produce reports. MCP and command hooks share
the same validation and append path, and neither changes the program's permission decisions.
The configured `max_report_bytes` bounds native command input and the resulting report.

Bundled integration changes require an explicit update through the installer. Rebuilding Heca
does not rewrite an installed integration or personal settings.

The registered id is scoped to the extension. Configure its name, executable aliases or enabled state
under `[settings.agents.programs."demo.custom"]`. Registration does not enable reporting: the user opts in
with `enabled = true`, as for built-in programs. Status labels, icons and optional sounds use
`[settings.agents.statuses.<status>]`. `PaneFacts.agent` and `PaneRuntime.agent` carry the current report;
`pane.agent.changed` carries changes independently of process status.

Each status can set `label`, `icon` (a shared glyph name), `tone` and `sound`.
Tones use the current theme: `accent`, `muted`, `warning`, `success` or `danger`.
The icon and label follow that tone together, including for plugin-defined statuses:

```toml
[settings.agents.statuses.review_ready]
label = "Ready for review"
icon = "check_circle"
tone = "success"
```

Finished means the turn ended; Success explicitly reports a positive result. Their
default symbols differ, and adding presentation does not invent lifecycle events.

Pane header chips also support semantic colour, without fixed RGB values:
`PaneChip::new(Glyph::GitBranch, branch).tone(Tone::Warning)`. Their labels shorten
inside the header while their hover tooltips retain the complete value.

Reporting opt-in also applies to terminals already open when project settings are
trusted or reloaded. Each terminal reserves an empty private connection at startup;
no program wrapper, watcher or integration runs while reporting is disabled. On
enable, the same connection publishes only the chosen launchers and accepts the
current program's latest bounded report. On disable, it removes those launchers
and clears the displayed agent state. Running shells and programs are preserved.

### Configurable sounds

Playback is off by default. When enabled, the bundled defaults are `completed` for Finished,
`error` for Error, `locked` for Waiting for permission and `attention` for Waiting for input.
`success` is also available for custom selection. Working/compacting/subagent progress stays silent.

```toml
[settings.agents.sounds]
enabled = true
volume = 0.7

[settings.agents.statuses.finished]
sound = { source = "asset:completed", volume = 0.4 }

[settings.agents.statuses.error]
sound = { source = "/full/path/to/my-error.mp3", volume = 0.5 }

[settings.agents.statuses.waiting_input.sound]
enabled = false
```

Each status can select a registered asset (`asset:name`) or a local filename, set its own volume
from 0 to 1, or disable playback. An omitted per-status volume inherits the global one. Existing
`sound = "/path/to/file.wav"` settings still work. The normal config/project merge applies; changing
`source` replaces the choice while preserving other controls. The global switch overrides all
per-status choices. Repeated reports, title/model updates and agent clearing do not replay a sound.

The pack is embedded in the library; no runtime path points at a source checkout or Downloads.
File and embedded audio use one shared decoder on the background playback thread. A missing file,
bad format or unknown asset reports an error without interrupting the agent or render loop.

Native embeddings add assets through their ordinary extension handle:

```rust,ignore
use heca::audio::{Audio, Sound};

let demo = heca::extension("demo");
demo.sound("review-ready", Audio::from_bytes(include_bytes!("assets/review-ready.mp3")));
demo.sound("warning", Audio::load(package_directory.join("warning.mp3")));

// The same choice/control type is used by config and a future settings UI.
let choice = Sound::asset("demo.review-ready").volume(0.4).enabled(true);
// Configure a custom status with sound = { source = "asset:demo.review-ready" }.
```

Register before `heca::run()`. `heca::audio::library().names()` provides the available choices,
including added assets. A caller building an independent catalog can use
`SoundLibrary::new().sound("name", Audio::load(path))`. No harness or status name is built into
the loader. This is native extension registration; it does not add a dynamic plugin loader.

## Terminal processes

**How a terminal's process starts.** Every terminal process — the shell or command behind a
terminal — starts through one door, from a plain description of what to run and where; the door
starts it, or keeps the one already running. A terminal you place (`demo.terminal("s")`) asks the
door itself from its own settings, and the same name is the same process. A pane's terminal is
started by the server when the pane is made, from the pane's description, and the pane's terminal
component shows it: the server holds the panes, so a pane's shell runs with no window drawing it —
a pane made over RPC with no window open, a restored session, an agent on a workspace nobody is
looking at.

`PaneFacts` is plain data about one pane (`#[non_exhaustive]`: fields may be added, so read them by
name). An action changes shared state; a dock, layer or pane decoration is drawn by a window.
The server/client boundary carries those responsibilities without changing these extension calls.

## Where the code lives

`heca/src/entry/` (the doors), `heca/src/chrome/panes/pane_items/` (buttons, chips, lines and the listing
rules they share), `heca/src/lib.rs` (what is public).
