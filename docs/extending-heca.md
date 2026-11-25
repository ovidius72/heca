# Building on heca — adding to it from another crate

## ⭐ THE EVENTS — one way to listen, everywhere

A state change is heard with one line, the same in heca's own components, in an extension and in a
plugin (WASM included):

```rust
cx.on("pane.cwd.changed", |ev| …)        // a component or provider, in a window
rooms.on("session.changed", |rooms, cx, ev| …)   // an extension's service, on the server
// a plugin: the same on("name", …) through the event bridge (F003/T79)
```

`"*"` listens for every event. Names are dotted (`pane.status.changed`, `session.changed`). Where
changes are rare and a listener wants them together, one event carries a `kind` saying what happened
(`session.changed`: `created` · `renamed` · `terminated` · `restarted` · `deleted`) and the data a
listener needs. Each event is raised from ONE place, and the scripts' stream (`heca rpc --events`)
carries the same event. **Never** a signal, a polling loop, a dedicated trait method or a
per-consumer callback in its place; a change nobody can listen to is not done. (Project decision
f639e662; the contract is P054(F006)'s "Pane runtime state", section 0.1.)

---

`heca` is a library plus a one-line program. Plain heca is:

```rust
fn main() { heca::run("heca"); }
```

A program built on heca says who it is, adds what it wants through heca's **existing doors**, and
calls the same `heca::run(..)` with its own fixed name — `heca::run("Heca Pro")` — which titles its
windows (`Heca Pro - ~/projects/x`). Its own icon, optional, goes with the name —
`heca::App::new("Heca Pro").icon(include_bytes!("../assets/icon.png")).run()` — and is the window's
and the Dock's while it runs (a user's `icon.png` beside the config still wins); the installed icon is
the program's own package's. Nothing here is a second mechanism: each door fills the one heca's own
parts use, so what is added is reachable from a key, a menu, the palette and RPC with nothing written
for any of them. A worked, runnable example: `heca/examples/extend_heca.rs`
(`cargo run -p heca --example extend_heca`).

```rust
fn main() {
    let demo = heca::extension("demo");
    // Add actions, layers and pane decorations through `demo`.
    heca::run("heca");
}
```

**Docks are contributed through Providers.** The canonical
[Provider and startup region reference](widgets.md#providers-and-regions) documents adding,
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
| `demo.layer("chat").arg("room", ArgKind::Text, true, "…").view(build)` | a layer that shows one of many things takes arguments; `cx.arg("room")` reads them as it builds |
| `demo.defaults(config, keys)` | ship the extension's own `config.default.toml` and `keybindings.default.toml` — see [An extension's own defaults](#an-extensions-own-defaults) |

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

### An extension's own defaults

An extension ships its defaults the way heca does — a `config.default.toml` and a
`keybindings.default.toml` beside its code — and hands them over once, before `heca::run(..)`:

```rust
let rooms = heca::extension("rooms").defaults(
    include_str!("../config.default.toml"),       // [extensions.rooms] helper = "auto" …
    include_str!("../keybindings.default.toml"),  // "rooms.focus" = "prefix+Shift+a", [[keys.surface]] name = "rooms" …
);
```

They are layered between heca's defaults and the user's files — **heca's defaults → each
extension's → `config.toml` / `keybindings.toml` → the project's `.heca/config.toml`** — so they are
defaults like heca's own: the extension reads its settings with `Config::extension::<T>("rooms")`;
its keys bind like heca's (plain bindings for any of its actions, not only a dock's, and its
`[[keys.surface]]` entries beside heca's); `--show-config` lists its settings as coming from
"(defaults of extension rooms)"; and the user overrides any of it in the same files, the same way.

They speak **only for that extension**: the config file may hold only `[extensions.<name>]`, the
keys file only bindings of its own actions (`<name>.…`) and its own surfaces (`[[keys.surface]]`
named `<name>` or `<name>.…`). Anything else — a heca setting, a heca key, another extension's — is
refused and said, and those defaults are dropped. A WASM plugin will hand over the same two files
from its package, through the same handle. Code: `heca-config/src/extension_defaults.rs`.

## Providers and docks at startup

The canonical [Provider and region API reference](widgets.md#providers-and-regions) explains
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
  `[appearance.pane] title_actions`, `title_segments` and the sidebar's view lists
  (`[appearance.sidebar] pane_<view>_view`): while a
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
| an overlay | `demo.layer("s").view(f)` | client | opened with `toggle_layer name=demo.s` from a key, menu, palette or RPC; built again each time it opens. One that shows one of many things declares its arguments (`.arg("room", ArgKind::Text, true, "…")`) and reads them with `cx.arg("room")`; `toggle_layer name=demo.s room=auth` passes them. A missing required argument, or one it does not take, refuses the action with a sentence. The last arguments are kept for rebuilds, and a toggle given other arguments shows them instead of hiding |
| a pane header button | `demo.pane_button("s")` | client | runs the action `demo.s`; icon, words, tooltip and danger come from it; if it declares an integer `pane` argument (`heca::PANE_ARG`) it receives the pane's id |
| a terminal | `demo.terminal("s")` | client | the terminal `demo.s` in any tree you build (a dock, an overlay): `.command(..)`, `.cwd(..)`, `.title(..)`; the same name is the same running process, and closing the dock never ends it |
| a pane header chip | `demo.pane_chip("s", f)` | client | `f(&PaneFacts) -> Option<PaneChip>`; re-run when the pane's facts change |
| a sidebar row line | `demo.pane_line("s", f)` | client | `f(&PaneFacts) -> Option<PaneLine>` |
| a list of the palette, or something added to one of heca's lists | `demo.palette_list("containers").sigil('%').items(..).render_item(..)` | client | items are `PaletteItem`s (title, description, icon, keys, `on_choose(Intent)`); `render_item(\|item, row\| row.trailing(..))` adds to the row (a widget, or a description from a plugin); heca's lists are `command`, `pane`, `workspace`, `session`, `follow`, `notification`; see [The palette's lists](#the-palettes-lists) |
| a service | `demo.service("s", factory)` | server | something that lives inside the server, once; see [A service inside the server](#a-service-inside-the-server) |
| a listener | `service.on("session.changed", \|svc, cx, ev\| …)` | server | THE EVENTS on a service; see [Sessions and their projects](#sessions-and-their-projects) |
| a folder line | `heca::FolderLine::new().path(p)` | client | the line heca shows a pane's directory with — see [Sessions and their projects](#sessions-and-their-projects) |
| a client of a running server (a test, a probe, a tool) | `heca::socket::request(path, line, &limits)`; `heca::socket::Attached::open(path, &limits)` then `.send(..)` / `.receive()` | outside | the same code heca's window and `heca rpc` run, so it moves with the protocol version; never write the attach preface or the frames by hand |
| a terminal agent driver | `demo.agent_driver(MyDriver::new())` | terminal owner | implements `heca::agent::AgentDriver`; scoped as `demo.<driver id>` and used by the same launch/report path as built-in drivers |

Dock and layer factories return ordinary components. For a dock, implement
`Provider::build_contribution` with `ContainerContribution::new(self, |cx, build| widget)`;
for a named layer use `demo.layer("details").view(|cx| widget)`. Neither needs author-side boxing,
casts or storage conversions. The complete compiling program is
[`extend_heca.rs`](../heca/examples/extend_heca.rs); the shared reference is
[Providers and regions](widgets.md#providers-and-regions).

## Terminal agent drivers

Implement `heca::agent::AgentDriver`: declare executable names, prepare a temporary reporting
connection, and decode explicit `AgentInput` reports. Register it
before `heca::run(..)`:

```rust,ignore
let demo = heca::extension("demo");
demo.agent_driver(MyDriver::new());
heca::run("heca");
```

**An agent's icon, by its driver id** — to draw the same harness icon heca draws, in your own dock:

```rust,ignore
let icon = heca::agent::icon(&cx.config().config.settings.agents, "claude"); // Option<AgentIcon>
```

The id is the driver's (`claude`, `codex`, `pi`, `opencode`, or `<extension>.<id>` for an added
driver). The icon is there whether or not the user switched the integration on, and the user's own
`icon` setting wins, as in heca's sidebar. `None` for an id heca does not know: show the generic
terminal glyph, as the agent line does.

`AgentSetup` adds arguments and environment for that invocation; the original arguments remain.
Read current command settings from `AgentLaunchContext.environment` and `cwd`. Launch setup never edits
personal settings. A side-channel driver writes newline-delimited reports to the supplied `endpoint`; an OSC
driver reads `TerminalNotification` inputs. Reports use open `AgentStatus` identifiers. The configured
byte and queue limits apply to custom drivers too.

To let a saved session continue the program's own conversation, implement
`AgentDriver::resume(original, session_id)`: return the launch that continues the native session
`session_id` the program reported (`original` may already be an earlier resume). The default returns
`None`, and then heca says the harness was not resumed and gives the pane the user's shell; it never
starts a new conversation in its place.

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

**A brand mark carries its own background.** The theme is the user's, so a mark drawn in one colour
on a transparent background disappears on some theme (a black mark on a dark one). Ship the mark the
way the brand itself draws its app icon: on its own solid tile, in the brand's colours. heca's Claude
mark is white on Claude's coral tile, and the Codex mark is OpenAI's black symbol on a white tile.
Only brand marks keep their own colours this way; everything else heca draws follows the theme.

**A session whose work runs outside heca still reports to its pane.** A program can run a session
in a process heca never started — Claude Code resumes a background session in its own daemon, and
only the front-end runs in the pane — so that process's hooks have no `HECA_AGENT_*` variables. When a
hook that has them reports a native `session_id`, heca remembers the pane's reporting connection under
that id (an owner-only folder in the user's temporary folder); a hook without them looks its session
up and reports to the same pane. A pane that is gone has no file to write to, and its entry goes.

The report reader attaches the registered mark after decoding a native report. Lifecycle templates
do not repeat SVG contents in every event or in the launch environment. A user can override a mark
with `icon = "/path/to/logo.svg"` under that program's configuration; this works for added drivers too.

`AgentState` also accepts optional `model`, `session_id` and `session_title` values supplied by a
driver. The row shows the program's icon and the model it answers with, or its configured name when
it reports none. A driver keeps the model current by naming it on its events; for a hook that does
not, heca reads it from the conversation log the hook names (`transcript_path`, the model of the last
answer — at the end of a turn it waits a moment for that answer to be written), and a model-switch
hook names the new one (`to_model`, Claude Code's `PostModelSwitch`, installed with the integration),
so Claude Code's row follows `/model` at once. Under it, the reported session title, as `[appearance.sidebar] agent_session_line` says
(by default only a title the program reported that is not the pane's name already). It never
invents a title. Omitted model/title metadata is retained
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
`pane.agent.changed` carries changes independently of process status. A component outside heca
reads a window event through its accessors, not by matching it: `ev.pane()` (the pane of any
`pane.*` event), `ev.agent()` (the report now, for `pane.agent.changed`) and `ev.feedback()` (the
granted status, for `pane.feedback`):

```rust
fn on_activate(&mut self, ctx: &heca::ChromeCtx<'_>) -> heca::ProviderHandles {
    // The dock's own store of the latest report per pane, read by its rows.
    let reports = self.reports.clone();
    heca::ProviderHandles {
        subscriptions: vec![ctx.on("pane.agent.changed", move |ev| {
            if let Some(pane) = ev.pane() {
                reports.record(pane, ev.agent().cloned());
            }
        })],
        ..Default::default()
    }
}
```

Each status can set `label`, `icon` (a shared glyph name), `tone` and `sound`.
Tones use the current theme: `accent`, `muted`, `warning`, `success` or `danger`.
The row shows the icon in that tone, with the label as its tooltip, including for plugin-defined
statuses:

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

Playback is on by default (`[settings] notification_sound = true`; `false` mutes it). The bundled defaults are `completed` for Finished,
`error` for Error, `locked` for Waiting for permission and `attention` for Waiting for input.
`success` is also available for custom selection. Working/compacting/subagent progress stays silent.

```toml
[settings]
notification_sound = true   # the one switch; false mutes every sound

[settings.agents.sounds]
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
`source` replaces the choice while preserving other controls. `[settings] notification_sound` overrides all
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

Register before `heca::run(..)`. `heca::audio::library().names()` provides the available choices,
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

## A service inside the server

**When to use one.** When something has to keep running whatever windows do: a connection to another
program, a watcher, a queue of work. A dock or layer is drawn by a window and goes with it; a service
lives in the server, so it is still there when the window closes and when another one opens.

```rust
let demo = heca::extension("demo");

// 1. Declare it. The factory runs once, when a server starts.
let rooms = demo.service("rooms", |cx| Ok(Rooms::open(cx.config(), cx.inbox())?));

// 2. An action on it. It is an ordinary action: a key, a menu, the palette and RPC all find it.
//    Its arguments reach the handler in the types it declares — a declared `Int` is an `Int`
//    whether a key, a script line or code sent it — so the handler parses nothing itself.
rooms.action(
    demo.action("join").label("Join room"),
    |rooms, cx, intent| rooms.join(cx, intent), // returns an `ActionResult`
);

// 3. What a window shows. `apply` folds each snapshot or delta into the value.
let shown = rooms.client(RoomsView::default(), |view, update| view.apply(update));

// In a component: a list in it is children that follow it — the rows of the rooms still there are
// kept, new ones made, gone ones dropped. A single fact is a widget bound to it. Reading it once while
// a dock is built shows what it was then, and nothing rebuilds that dock when it changes.
Flex::column().gap(Spacing::Sm).child(shown.map(|view| &view.rooms, RoomRow::new))
```

`Rooms` implements `Service`:

- `receive` takes one message from the inbox. Call `cx.republish()` when what windows see changed.
- `project` says what one window is shown: a whole snapshot, a delta on the revision that window
  already has, "unchanged", or nothing.
- `stop` is called at shutdown. The default has nothing to wait for.
- `SCHEMA_VERSION` goes up when the snapshot or delta shape changes. A window on another version
  shows a notification instead of misreading the data.

A worker the service starts sends it messages through the `ServiceInbox` (`cx.inbox()`). It is
the only part that crosses threads, so the service itself need not be `Send`. The inbox is bounded:
when it is full, `send` returns an error to the sender and never blocks.

An action answers `Done`, `Refused(why)` or `Later(ticket)`. For `Later`, take the ticket with
`cx.later()` and call `cx.complete(ticket, result)` when the work is done.

**A folder of its own.** `cx.data_dir()` is the folder for what the service writes to disk. It
exists before the factory runs, only your user can read it, and no other service shares it. It is the
same on every start of this server, so a service finds its files again after a restart. The folder is
`<data folder>/heca/servers/<server>/<extension>.<service>/`. `<server>` is `default` for the
background server (`heca --server`). A standalone window's server is `standalone-1`, or the lowest
`standalone-<n>` no other running server holds: two windows open at once never share files, and a
window opened again after one closed gets its folder back. `ServiceCx` and so every action have
`data_dir()` too, so a service need not keep the path. Where the platform has no data folder, the
service does not start and says so in a notification; the server keeps running.

**Which actions exist — `cx.action_catalog()`.** The factory can ask what actions this server has,
to check that an action it will name is there. The answer has every action of heca, every extension's
plain action, every service's action (its own and other services') and `kill_server`. It does not
have what only a window declares, such as a dock's own actions. It is complete when the factory runs:
the server adds all of these before it starts any service. It is a list of names (`contains(name)`,
`names()`), cheap to clone, and safe to send to a worker thread. It describes this run of the server
only and is not saved, so ask again on every start. It grants nothing: whether an action may run is
still decided by the action policy when it is asked.

**Which session — `session()` or `session_key()`.** `cx.session()` is a number for this run of the
server: use it to say "the session this call is in" and nothing more, because a server that starts
again may number its sessions differently (it keeps the numbers of sessions it restores, but not when
saving is off). To store something against a session, use
`cx.session_key()`. It is a `SessionKey`, which heca makes once when the session is created. It stays
the same when the session is renamed, terminated, restarted or has windows come and go, and it does
not depend on the session's name, folder or a window's connection. It ends only when the session is
deleted. Compare it, hash it and store it (it implements `Serialize` and `Deserialize`); you cannot
read anything out of it or make one. `ActionCx` and `ProjectCx` both have it, and it is always the
key of the session the call or the window is in. A session the server saved when it stopped comes back
with the same key, so data stored against it is found again after a restart; with `save_sessions = false`
(or a file that cannot be read) the sessions are new and so are their keys.

**Which workspace — `workspace()`.** `cx.workspace()` is the workspace the action was asked from:
the one the asking window shows; for a script, the one a window of the session showed last; `None`
before any window has shown one. It is a `WorkspaceKey` — the session's key and the number heca gave
the workspace when it was made — so, like `SessionKey`, it can be stored and still names the same
workspace after the workspaces are reordered, renamed or folded and after the server restarts. It
ends when the workspace is deleted. Its position (`ws_idx`) is a different thing: it changes on a
reorder, so never store it.

```rust
// A room belongs to the workspace it was made in — no index passed by the caller.
fn create_room(rooms: &mut Rooms, cx: &mut ActionCx<'_, Msg>, intent: &Intent) -> ActionResult {
    let Some(workspace) = cx.workspace() else { return ActionResult::Refused("no workspace yet".into()) };
    rooms.add(Room { workspace, .. });
    ActionResult::Done(None)
}
```

A dock reads the same key for the workspace **its window** shows, as a value to follow:
`cx.state().active_workspace()` (a signal, so a binding on it follows workspace switches with nothing
rebuilt), and the event `workspace.focused` is raised when it changes (THE EVENTS):

```rust
let shown = cx.state().active_workspace();
RoomRow::new(&room).hidden(move || shown.get() != Some(room.workspace))   // only this workspace's rooms
```

**A table of its own.** `cx.extension_config::<T>()` reads `[extensions.<name>]`, where `<name>` is
the extension the service was declared under, as any type that implements `serde::Deserialize`. A
table that does not fit `T` is an error that names the table and says what is wrong
(`ExtensionConfigError`), never a quiet default. A missing table reads as an empty one, so a type with
`#[serde(default)]` is its own defaults. An extension can read no table but its own.

```toml
# ~/.config/heca/config.toml — a trusted project's .heca/config.toml can set the same keys
[extensions.demo]
server = "wss://rooms.example.com"
max_rooms = 20
```

```rust
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Settings { server: String, max_rooms: usize }

let rooms = demo.service("rooms", |cx| {
    let settings: Settings = cx.extension_config()?;      // the server's configuration
    Ok(Rooms::open(cx.data_dir(), settings, cx.inbox())?) // its own folder
});
// In an action or a projection, `cx.extension_config()` reads the session's configuration — the
// user's, with a trusted project's on top — the same one `cx.config()` returns.
```

A trusted project can override any of your keys, as it can every other setting. If a key is
sensitive — a command to run, a path to write to — check it before you use it, or read it only from
the server's configuration (`cx.extension_config()` in the factory).

**Who may run what — `action_policy`.** Every action the server runs is first put to every policy any
service declared: an action from a window, a script, a helper or a request another service handed in,
and each follow-up of a chain.

```rust
rooms.action_policy(|rooms, _cx, request| match request.requester() {
    Requester::External { sender, .. } if !rooms.may(sender, request.action()) =>
        Permit::Deny(format!("{sender} may not run {}", request.action())),
    _ => Permit::Allow,
});
```

- `request.requester()` is `Requester::Local { window }` (the user, from a window or a script) or
  `Requester::External { service, sender }` (a request a service handed in for `sender`).
  `request.action()`, `request.args()`, `request.session()` (`None` for what belongs to no session, such
  as listing or creating sessions) and `request.parent()` (the action that asked for this follow-up)
  say the rest. An action reads who it was run for with `cx.sender()` (`None` for the user).
- **Every declared policy is asked about every request, local or external, and the first `Deny` wins.**
  The reason goes to whoever asked: a notification in a window, `error: …` to a script, the outcome of
  a request a service handed in.
- It is asked when the action is about to run, never earlier, so a rule that changed applies to what
  was already waiting. It covers every action in the catalog: built-ins, extension actions, service
  actions and the follow-ups of a chain, which are asked with the same requester as the action that
  started them. A chain cannot do what its requester could not.
- **With no policy declared anywhere**, the user's requests go through as before and a request handed
  in by a service is refused. A request handed in by a service that declares no policy of its own is
  refused even when another service declares one.
- There is no way around it: heca keeps no exception for any requester.
- **Stopping the server (`kill_server`) is covered for the user** — from a window, `heca --kill-server`
  or `heca rpc kill_server` — so a policy can forbid it and the server stays up, with the reason
  reported as for any other refusal. For a request handed in by a service it is **never runnable**,
  whatever any policy says, also as a follow-up of a chain.
- **What it does not cover.** An action a window runs by itself, with no help from the server (moving
  focus, scrolling, opening a menu), never reaches the server, so no policy sees it. Everything that
  changes the server's state does.

**Security limit.**

Any process of the same user can open the socket directly. Without effective isolation, such as a different OS user, permissions limit requests received through cooperating integrations, not a hostile local process. Nothing more is promised. `Sender::new(...)` authenticates no one.

**Requests handed in for someone else — `cx.submit`.** A service that relays what other people ask
for (a chat room, a webhook) hands each request in from where it gets control (`receive`, an action):

```rust
let sender = Sender::new(who_the_integration_says_this_is); // from the integration, never from message text
match cx.submit(ExternalRequest { sender, session, request_id, line }) {
    Ok(request) => self.waiting.insert(request),            // a `RequestRef`
    Err(SubmitError::QueueFull | SubmitError::DuplicateId) => { /* try later, or say so */ }
    Err(other) => { /* UnknownSession, or Refused(why) */ }
}

// Later, once per request:
fn request_done(&mut self, cx: &mut ServiceCx<'_, Msg>, request: &RequestRef, outcome: RequestOutcome) {
    match outcome {
        RequestOutcome::Done(line) => { /* … */ }
        RequestOutcome::Refused(why) | RequestOutcome::Failed(why) => { /* … */ }
        RequestOutcome::Unknown => { /* the server stopped part way; never run again */ }
    }
}
```

- `line` is exactly what `heca rpc` takes, and it is read, checked and run the way a script's line is,
  in the session `session` names. heca never guesses a session and never uses another one:
  `SubmitError::UnknownSession` if no session has that key. A line is at most `max_frame_bytes` long.
  An action that **declares** a confirmation (a built-in like `close`, or one an extension marks
  `.destructive()`) is always refused for a sender, even when the user turned that confirmation off
  for themselves in `[confirm]`; the same holds for a chain's follow-up. A query (`list-actions`,
  `describe-action`) is not an action and is refused. An action that needs a window, or that makes,
  lists or removes sessions, is not run: a request handed in is never sent to a window.
- `submit` answers at once and runs the request on the server's next turn, with every policy asked
  first. The answer is one of `Ok(RequestRef)`, `QueueFull` (the service already has
  `request_queue_capacity` open), `UnknownSession`, `DuplicateId` (a request of this service with the
  same id is still open) or `Refused(why)` (empty or too long a line, no policy, server stopping).
  heca does not remember an id once its request has ended; remembering them is the service's job.
- `request_done` says how it ended. **`Done(line)`**: every action of it ran and every deferred answer
  came in. **`Refused(why)`**: nothing ran. **`Failed(why)`**: it ended with an error after something
  may have run, including a refusal part way through a chain; what ran before is not undone.
  **`Unknown`**: it was started and the server stopped before it ended.
- **Restarts.** Nothing is saved. When the server stops, a request that never started is
  `Refused("server stopped, not executed")` and one that started and did not finish is `Unknown`, and
  the service is told while it is still running. Nothing is run again by the next server.

**What heca guarantees**

- **One per server.** The factory runs once when a server starts, not per window or per session.
  Closing a window or switching session never restarts it.
- **Its folder is its own.** Never shared with another service or another running server, never
  readable by other users, and the same after a restart.
- **A window always gets a snapshot** when it attaches, reconnects, meets a server that has started
  again, or misses a revision (it asks for a fresh one). Between those it gets deltas.
- **A deferred answer is never run again and never cancelled.** If the caller times out or goes
  away, the work still finishes. A script waiting on it is answered; a window is only told if it was
  refused.
- **Stopping waits for the service.** At shutdown heca calls `stop` until it answers `Done`,
  delivering inbox messages in between, for at most `stop_timeout_ms`. Callers still waiting are then
  answered "the server stopped".
- **Every action is judged by the same policies, wherever it comes from**, and a request handed in
  by a service is run in the session it names or not at all.
- **Failures are shown, not fatal.** A factory, message or projection that fails, or a service that
  is slow to stop, becomes a notification (and a line on stderr); the server keeps running.

**Settings** (`[settings.services]`):

| setting | default | meaning |
|---|---|---|
| `inbox_capacity` | `256` | messages one service's inbox holds before senders are told it is full |
| `stop_timeout_ms` | `3000` | the longest the server waits for services to stop at shutdown |
| `request_queue_capacity` | `64` | requests one service may have open at once, waiting to run or waiting on a deferred answer |
| `saved_entry_max_bytes` | `65536` | the largest value one saved-state entry may hold, in bytes of JSON |

Declare services and their actions before `heca::run(..)`; later ones are reported and dropped. Code: `heca/src/entry/service/` (what you write), `heca/src/server/services/` (the host).

### State an extension saves

What an extension wants back after a restart — the order of its rooms, a choice the user made — is
**saved state**, kept by heca and written by the server only (the user's TOML stays theirs, read
only). An entry is found by a key and an id (`""` for a single value); heca puts the extension's
name in front of the key, so no extension can read or overwrite another's. The value is your own
type, stored as JSON:

```rust
// In an action: this session's room order.
let order: Vec<RoomId> = cx.saved(Scope::Session, "room-order", "").unwrap_or_default();
cx.save(Scope::Session, "room-order", "", &order)?;           // Result<(), SavedError>
cx.forget(Scope::Session, "room-color", &room_id)?;           // one entry per room
```

- **Scopes.** `Scope::Global` — one value for the whole server, in a file in the server's own folder
  (`heca.saved-state.json`), written at once and atomically. `Scope::Session` — the session the call
  is in (an action's, a projection's): saved with the session, restored with it after a restart,
  deleted with it. A message handler is in no session: it names one, `Scope::SessionOf(key)`.
- **`save_sessions = false`:** the sessions are not kept across a restart, and neither are their
  entries. Global entries are kept either way.
- **Reading.** `cx.saved::<T>(scope, key, id)` is `None` when the entry is not there **or no longer
  fits `T`** (you changed the type): start from your default. A missing, corrupt or older global file
  is a first run, never a failed start.
- **Limits.** A value larger than `[settings.services] saved_entry_max_bytes` is refused with
  `SavedError::TooLarge`, never cut short.
- **Who has it.** `ActionCx` (read and write), `ServiceCx` (read and write; message handlers),
  `ProjectCx` (read only: a projection shows state, it does not change it).
- **A dock shows it and changes it — nothing else to write.** The dock reads what its service shows
  (`shown`), and the projection reads the saved entry; a drag or a key runs one of the extension's
  actions, which saves. **After a save, heca shows that extension's services again in every
  window**, so the new value reaches every dock without the service saying its view changed:

  ```rust
  DockFrame::new("Rooms")
      .grip(true)
      .child(
          Flex::column()
              .gap(Spacing::Sm)
              .child(shown.map(|view| &view.rooms, |room| {         // the order the projection read
                  RoomRow::new(room)
                      .draggable()
                      .on_drop(|ev| ev.dispatch(Intent::new("heca-pro.reorder")))   // the action saves
              })),
      )
  ```
- **Listening (THE EVENTS).** Every save and every real forget raises **`state.changed`**, with the
  session (`None` for global), the full key (`heca-pro.room-order`) and the id —
  `rooms.on("state.changed", |rooms, cx, ev| { let change = ev.as_state(); … })`. The scripts'
  stream (`heca rpc --events`) carries the same event.

### Notifications with a button of their own

A service raises a notification with `heca::Notification`, and a button on it can run a closure — no
public action to declare for a one-off "Open pane" or "Retry":

```rust
heca::Notification::info("Agent finished")
    .body("room design · bob")
    .action(NotificationAction::run("Open pane", move || inbox.send(RoomWake::OpenPane(pane))))
    .send();
```

The closure stays on the server, so the button works in every window of the session, and it lives as
long as the notification does: a notification the server drops (a duplicate, `mode = "none"`) drops
it, and one that leaves the screen lets it go — nothing to register or unregister. A button made in
window code has no closure on the server; use a named action there
(`NotificationAction::new(label, Intent)`). Code: `heca/src/notification/model/action.rs`.

## Sessions and their projects

Every context a service is handed — `ServiceCx` (and so `ActionCx`) and `ProjectCx` — reads
**every session the server holds** with `cx.sessions()`: its key (the same for its whole life), its
name, its project folder and whether it is running. Terminated sessions are listed; the caller
decides what to show.

```rust
let mine = cx.session_key();
let folder = cx.sessions().folder_of(mine);                      // None: made without a folder
let siblings = cx.sessions().same_project(mine);                 // this session and the others on its project
let on_heca = cx.sessions().of_project(Path::new("~/projects/heca"));
```

Folders are compared as the file system resolves them, so a project reached through a symbolic link
or written with a trailing slash is the same project.

Each session also says how many windows show it and how many agents run in it — `entry.windows()`,
`entry.agents()`.

**In a window, the same list and the same event.** A dock reads every session with
`cx.state().sessions()` — a value to follow, so a list bound to it keeps up as sessions are made,
renamed and deleted — and hears `cx.on("session.changed", |ev| …)` with the same kinds:

```rust
let sessions = cx.state().sessions();
Flex::column().child(sessions.map(SessionRow::new))
```

**Something of yours at the end of a session row** — the session list is one of the palette's
lists, so it is [`palette_list("session").render_item(..)`](#the-palettes-lists):

```rust
let pro = heca::extension("heca-pro");
pro.palette_list("session").render_item(move |item, row| {
    match item.session().filter(|s| rooms.with(|r| r.is_pro(s.key()))) {
        Some(_) => row.trailing(Badge::accent("pro")),
        None => row,
    }
});
```

**To hear when sessions change, listen** (THE EVENTS): `session.changed` carries what happened
(`ev.kind()`: `Created`, `Renamed`, `Terminated`, `Restarted`, `Deleted`), the session it happened to
(as it was just before, for a deleted one) and every session after the change.

```rust
rooms.on("session.changed", |rooms, cx, ev| {
    let Some(ev) = ev.as_session() else { return };
    match ev.kind() {
        SessionChange::Created | SessionChange::Deleted => rooms.regroup(ev.sessions()),
        SessionChange::Renamed => rooms.rename(ev.session()),
        _ => return,
    }
    cx.republish();
});
```

A name heca has no event for is refused out loud at start-up. `heca rpc --events` streams the same
event to scripts, as `{"SessionChanged":{"kind":"created","session":{…}}}`.

**Showing a folder the way heca does** is `heca::FolderLine` — a folder glyph and the path, cut from
the front so the end survives, the whole path on hover. Place it like any widget; its path follows a
signal, and an empty path hides the line and its gap:

```rust
let line = heca::FolderLine::new().path("~/projects/heca").size(WidgetSize::Caption).indent(Spacing::Sm);
let path = line.path_signal();
let card = Flex::column().child(Label::new(member)).child(line);
path.set(new_folder);           // in place, no rebuild
```

## Where the code lives

`heca/src/entry/` (the doors), `heca/src/chrome/panes/pane_items/` (buttons, chips, lines and the listing
rules they share), `heca/src/lib.rs` (what is public).

## The palette's lists

The palette is a set of **lists**, each opened by a character typed first (its *sigil*): heca's own
are `command` (`:`, the default), `pane` (`@`), `workspace` (`$`), `session` (`#`), `follow` (`~`)
and `notification` (`!`). An extension adds a list of its own, or adds to one that exists, through
the one door heca's own lists use — `palette_list(name)`. *Client side.*

```rust
let demo = heca::extension("demo");
demo.palette_list("demo.containers")
    .sigil('%')
    .placeholder("Search containers")
    .search_descriptions(true)
    .items(move || containers().into_iter().map(|c| {
        PaletteItem::new(c.name())
            .description(c.image())
            .icon(Glyph::Terminal)
            .keys("prefix+shift+c")
            .on_choose(Intent::new("demo.attach").arg("id", PropValue::Text(c.id())))
    }).collect())
    .render_item(|item, row| row.trailing(Badge::new("up")));
```

- **A list is named**, and every call on the same name adds to the same list, from any extension and
  in any order. Name yours after your extension (`"demo.containers"`). `sigil`, `placeholder`,
  `search_descriptions` and `items` say what the list is; the last call wins.
- **`items`** takes a list, a function returning one, or a signal holding one, read each time the
  palette opens. An item is a `PaletteItem`: a title (what is matched as you type), a description,
  an icon, its bindings (`keys`, written as in `keybindings.toml`), and what choosing it sends —
  always an `Intent`, so a choice passes the same action gate (policy, confirmation) every other act
  does.
- **`render_item(|item, row| …)`** shows an item: it gets the item and the palette's standard row
  (icon, title with the match drawn, description, keys) and returns the row. `row.trailing(widget)`
  puts something at the end of the title's line; `PaletteRow::custom(widget)` is a row of your own.
  Each `render_item` on a list receives the row the one before returned, so several extensions'
  additions add up, in registration order.
- **Opening a list** is the palette's own action: `command_palette mode=demo.containers` (a key, RPC,
  a script) opens the palette on it.
- heca's lists keep their order and their rows whoever names them first; adding to one
  (`palette_list("session").render_item(..)`) changes only its rows.

**A plugin** writes the same calls with data: its items are `PaletteItem`s and its choices
`Intent`s already; what it adds to a row is a description — `row.trailing_described(node)`,
`PaletteRow::custom_described(node)` — realized by heca into the same widget.

`palette(mode).to_raw(..)` / `.to_description(..)` are the earlier spelling of
`palette_list(mode).render_item(|item, row| row.trailing(..))` and still work, deprecated.

