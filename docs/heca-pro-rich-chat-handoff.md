# Heca foundations for rich chat, mixed panes and previews

Developer handoff from heca-pro · 2026-10-10

References and pinning ownership corrected on 2026-10-12. This file in heca is the canonical
handoff. Its requirements are stated here; the heca-pro links are optional background reading.

**Status:** the product direction below was agreed with Antonio. The API details, additions to
existing task scope, and delivery order are proposals for review with the heca maintainers.
Heca's planner remains the source of truth for implementation and priority.

## 1. Purpose

Provide the open heca foundations for a workspace where people can discuss work with agents,
inspect what they did, and keep using ordinary terminals and other apps alongside them.

The main agent interface should be a **rich conversation like Paseo's**: streamed replies,
formatted text, code, expandable commands and output, changes, reports and attachments. A basic
room message log is useful, but does not satisfy this requirement.

Heca-pro adds rooms, members, orchestration and workflows. Open heca supplies the reusable layout,
content hosting, widgets, agent adapters and resource viewers. These capabilities should also
work for a single agent or a plugin without heca-pro.

## 2. Agreed working view

Daily work happens in the normal workspace, through columns, panes and docks.

```text
Room workspace
├── Pinned column
│   └── Orchestrator conversation
├── Scrollable columns
│   ├── Worker conversation
│   ├── Worker conversation + another pane in the same column
│   ├── Ordinary terminal
│   └── Other content, including a future browser
└── User-placed docks
    ├── Rooms, members, progress and requests needing attention
    └── Tabbed previews: images, files, changes and browser pages
```

This is an example arrangement. A workflow can describe an initial layout; the user can change it.
It does not assign permanent positions to roles, content types or docks.

- **Column pinning** keeps selected columns visible while the other columns scroll horizontally.
  The user chooses which columns to pin. Pinning is generic heca work, but no dedicated
  implementation task was found in the planner review; see sections 4 and 5.2.
- Each conversation owns its vertical scrolling, selection and composer.
- A workspace containing a room remains a normal workspace. It can contain unrelated terminals,
  split panes and other apps. Room membership does not restrict its content.
- A progress dock shows current activity, tasks, reports and requests. Selecting a member can
  focus its conversation. Worker activity and planner task status remain separate fields.
- A preview dock can be placed in the right sidebar or elsewhere by the user. Several resources
  can remain open in tabs. The same viewers can also be placed in ordinary panes.
- Chat and terminal views should refer to the same agent session. The exact terminal view that
  each agent supports must be verified; see section 5.4.
- An optional overlay provides a general team or room overview. It is not the primary chat view.

On narrow windows, offer a configurable way to focus one conversation and reveal the other areas
when needed. A narrow desktop layout is useful independently of a future phone client.

### Relationship to earlier heca-pro decisions

This direction refines decisions 9, 45 and 46 in
[heca-pro decisions](https://github.com/ovidius72/heca-pro/blob/develop/docs/decisions.md). In particular,
decision 46's earlier terminal-first interaction for visible agents is being replaced by rich
conversation content in normal panes, with terminal access retained.

Room and Member remain components. A pane remains a generic container. A room is not a new pane
class, and a room's workspace does not become a chat-only workspace. Instance-level rooms also
remain part of the design; every room need not have its own workspace.

The visibility of direct worker messages to the orchestrator and the effect of selecting a room
on workspace focus remain open decisions. Neither should be assumed during implementation.

## 3. Ownership

| Capability | Owner | What heca-pro consumes or adds |
|---|---|---|
| Columns, panes, splits, pinning, focus and layout persistence | Open heca | Workflow layout and room/member associations |
| Public content hosting and reusable terminal content | Open heca | Conversation components mounted through the same API |
| Rich text, code, diffs, message lists, multi-line input and generic block presentation | `heca-grid-ui`, with rendering support where needed | Room/member labels, recipients and reports bound to pro data |
| Agent session contract and provider adapters | Open heca | Joining teams, role permissions, task assignment and orchestration |
| Resource opening, image/file viewers and browser content | Open heca | Links and attachments originating in room conversations |
| Generic server, socket, event stream and extension state hooks | Open heca | Shared room state and room history |
| Team messages and agent-comms integration | heca-pro's existing helper boundary | Room delivery, membership and reports |
| Planner display | heca-pro | Read-only access through the planner API |

Heca-pro is a separate application depending on open heca's crates. It contributes through
`heca::extension("pro")`, the same extension API used by plugins. Open heca's generic behaviour
must remain usable without room or role concepts.

The helper remains the single agent-comms boundary. Open heca's generic content and agent APIs
should not depend on agent-comms or the planner.

### What heca-pro will build on these foundations

The existing pro consumers are P003(F001)/T012–T014 for the helper, room state and command routing,
and P004(F001)/T015–T018 for rooms, activity, chat and notifications. These are heca-pro references,
separate from the heca references below.

T017 currently describes room/member messages through agent-comms, using P103's input and message
list. Reconcile its scope and dependencies with this richer conversation direction before starting
implementation. Pro will compose the upstream widgets, bind member/session identity and recipients,
and connect the progress and preview views. It will consume the richer agent contract rather than
build a private provider adapter or renderer.

## 4. Verified starting point

The following is a snapshot, checked against the live heca planner and local source on 2026-10-10.
Task status is separate from release availability.

| Existing work | State at review | Relevance |
|---|---|---|
| P098(F012), library entry point | Done | External crates can contribute before `heca::run()` |
| P101(F012), per-project settings | Done | Configuration for layouts, actions and user preferences |
| P094(F011), a pane hosts any app | In progress | Owns the common content foundation |
| P094(F011)/T448 and T451 | Done | External rendering support and the generic pane component |
| P094(F011)/T449 | In progress | Terminal becomes reusable content |
| P094(F011)/T450, T452, T453, T454 | Planned | Content-owned header, public content contract, floating reuse and non-terminal proof |
| P082(F003)/T474 | Done | Columns own their panes and identity; its future Pinned Column requirement is not pinning implementation |
| P102(F012), including T171 and T174 | Planned | Agent contract and adapters; current scope covers recognition, launch and status |
| P103(F012)/T524 | Planned | Multi-line input and message list, plus lines/arrows and tree layout |
| P037(F007)/T172 | Planned | Native rich Markdown preview, currently associated with Neovim |
| P044(F007)/T170 | Planned | Reuse the terminal image renderer, currently associated with Neovim |
| P016(F002)/T039 | Done | Terminal image rendering exists; a generic image viewer still needs integration |
| P104, P099, P100 and P105 in F012 | Planned | Server/client split, socket/events, sessions and save/restore |

The widget catalog already includes `Flex`, `Grid`, `Surface`, `Pane`, `ScrollRegion`, `Input`,
`Tabs`, `Choice`, `DockFrame`, `Button`, `IconButton`, `Label`, `Tag`, `Badge` and `StatusDot`.
`Tabs` supplies a selector; a tabbed viewer still needs content and lifecycle composition.

The pane shell has a generic content slot, but the current workspace model still stores terminal
content. The public entry API supplies actions, docks, layers and decorations; the reviewed
extension API also has reusable named terminals. The remaining public API for arbitrary pane
content is explicitly owned by T452, with proof in T454.

The reviewed heca checkout was `feat/t449-terminal-named`, at commit
`369a5fa7a8419929056a7b5927e6f537fc95f4d2`. Heca-pro currently pins
`58f0f2b221ea8d0238671a213b625d15998b9a4a`. APIs found on the development branch must be merged and
included in an agreed dependency revision before pro relies on them.

**Pinning review, 2026-10-12:** P082(F003)/T474 is DONE. Its final completion summary confirms the
column-parent work shipped. Antonio corrected its future-work wording to **Pinned Column**:
the user chooses a whole column to keep visible while other columns scroll. Its panes stay inside
that column. Independent sticky or pinned panes are outside this requirement. Pinning has not
shipped as part of T474.

No dedicated column-pinning implementation task was found in the reviewed task inventory,
descriptions or Ideas Inbox. T474 is a related foundation, not the pinning owner. A maintainer
must confirm whether another reference exists; otherwise agree a separate task with Antonio
before creating it. The earlier handoff's claim that pinning was already tracked was incorrect.

### Implementation rules

- Search the widget catalog, app components and surface components before adding anything.
- Compose with `Flex`, `Grid`, `Surface` and existing widgets. Follow SOLID and keep one clear job
  per component. A generic component belongs in the library that can own it.
- Put sizes, spacing, colours, wording, keys, limits and defaults in theme, config, language files
  or the workflow. This document introduces no fixed production values or bindings.
- Fix wrapping, scrolling, selection and lifecycle behaviour in the owning widget or framework.
  Every placement should receive the fix automatically.
- Use framework event targeting and bubbling, including keys. Content owns its interaction;
  docks and pane hosts supply placement, not custom input forwarding.
- Register user actions once through the existing action system. Expose capabilities through
  public APIs usable by the app, external crates and the normal RPC route where applicable.
- Keep content types, providers and pro roles extensible. Public APIs must work without hidden
  host types, manual registry lookups or requiring a caller to supply a widget key for basic behaviour.

## 5. What we need from heca

### 5.1. Finish the public content foundation

**Existing owner: P094(F011), especially T452 and T454.**

Provide a public way to register content and create a pane containing it. A plugin author should
implement and use it from another crate without a host-private type, manual registry lookup or
extra event wiring at the call site.

The contract should cover:

- Stable content identity and attachment to a view.
- A content-owned widget tree, including an optional header and its own scroll regions.
- Bounds and clipping supplied by the host, with content layout expressed through library widgets.
- Framework-delivered pointer, keyboard, focus, selection and clipboard behaviour.
- Mounting, detaching, disposal and an explicit distinction between closing a view and stopping
  its underlying program or session.
- State needed to restore the correct registered content type.
- Native widget content and content rendered by another engine through the existing host surface.

Keep the two existing responsibilities clear: `PaneBackend` is the process/IO contract; the
content contract is the UI side. A browser or editor may have its own program protocol. Its
adapter handles that protocol and implements the relevant public contracts.

Content types must be extensible. Terminal, browser, editor and conversation are examples, not a
closed application list. A component moved between a pane and a dock should keep its behaviour;
the framework resolves events through the tree.

**Proof required:** an external crate provides ordinary widget content with its own scrolling,
mounts it in a split pane and a dock, then exercises focus, selection, resize, zoom and floating.
Complete T454's non-terminal proof before treating this API as ready for pro.

### 5.2. Deliver column pinning through the layout engine

**Related foundation: P082(F003)/T474, DONE. Pinning implementation has no confirmed task.**

This section proposes the missing capability, named **Pinned Column** in T474's corrected future
requirement. Pinning means the user chooses whole columns that stay visible while other columns
scroll. This does not add independent pane pinning. Agree the implementation owner, scope and
priority before adding or changing planner work.

Pinned columns reserve real space. Scrolling columns are laid out and clipped in the remaining
area. Focus and reveal operations must use that same available area.

Pinning should preserve column and pane identities, processes, selection and content state.
It must work for arbitrary content and be available through the normal action catalog, bindings,
menus, palette and RPC. Widths, limits and interaction preferences come from theme or config.

Specify the behaviour for resizing, multiple pinned columns, moving a pinned column, workspace
changes and narrow windows. Also specify which pinning properties are shared workspace state and
which are per-window view state. These choices require review; the illustrative layout above
does not settle them.

**Proof required:** pinned conversation and terminal columns remain usable while other mixed
columns scroll, split, resize and receive focus. Several windows with different widths remain
usable without changing each other's view unexpectedly.

### 5.3. Provide reusable rich conversation building blocks

**Existing owners to reconcile: P103(F012)/T524 and P037(F007)/T172.**

T524 covers basic message lists and multi-line input. T172 already proposes native rich Markdown
preview. Agree a shared rendering implementation with that owner, keeping editor-specific
integration separate. Additional rich conversation scope requires maintainer agreement.

| Building block | Required behaviour |
|---|---|
| Rich text | Streaming Markdown, paragraphs, lists, tables, links, wrapped text and selection |
| Code | Language indication, readable highlighting, horizontal overflow where needed and copy |
| Command/output block | Command, execution folder, state, exit result and expandable output |
| Changes | File identity, readable additions/removals and an action to inspect the resource |
| Attachment | Image/file information, loading or error state, and an open action |
| Request block | A visible request state and actions bound to the existing permission/action route |
| Message list | Stable entries, incremental updates, history loading and responsive long histories |
| Composer | Multi-line editing, selection, clipboard, IME and configurable submit/newline behaviour |

Compose blocks from existing widgets wherever possible. Generic block presentation belongs in
the library; provider data interpretation belongs in adapters; pro-specific reports bind pro data.

Auto-follow new content only while the reader is following the latest messages. Reading older
content must preserve the visible position while streaming text reflows below it. Collapsed
output, selection and expansion state should survive ordinary updates.

Display and copying are separate from execution. An executable action must identify its target
and use the normal command and permission route. Copying a displayed command must not run it.

**Proof required:** showcase entries with streamed text, long output, code, diffs, attachments and
multi-line composition. Run them in the GPU app at narrow and wide sizes and with changed font,
theme and key settings. Performance limits must be configured and verified, not invented here.

### 5.4. Extend the agent contract to full conversations

**Existing owner to review: P102(F012), particularly T171 and T174.**

Its current scope is recognition, launch and status. Rich chat needs a supported conversation
interface as well. Status escape codes and terminal output alone do not define a reliable
conversation or tool history.

An adapter should declare and implement the capabilities its agent supports:

- Start, attach to and resume an identifiable conversation/session.
- Supply streamed conversation text and structured work events.
- Accept user input with an acknowledged result.
- Report whether input while working is queued, steering or interrupting, and expose supported
  controls explicitly.
- Supply tool lifecycle, output, completion/error, changes, attachments and permission requests.
- Supply historical data or a clear history capability when reconnecting.
- Identify supported terminal access and how it relates to the same conversation.

Built-in adapters and plugin adapters must use the same public interface. The UI reads declared
capabilities and normalized data; it does not branch on a program's name.

The event model should provide the following information. Exact Rust names and wire spelling are
for the heca API review, not declarations of an implemented API:

| Information | Why it is needed |
|---|---|
| Session/conversation identity | Associate views with the same running agent |
| Turn, message and block identity | Update streamed content without duplicating entries |
| Ordering/replay information | Reconnect and reconstruct a consistent history |
| Request/tool correlation | Match output and replies to the operation that produced them |
| Author, event kind and time | Explain the origin and sequence of activity |
| Text update semantics | Distinguish appends, replacements and completed content |
| Operation state and result | Present running, finished, interrupted and failed work accurately |
| Resource origin | Resolve attachments in the correct machine, project and worktree |
| Permission request identity | Resolve one request once through the existing checks |
| Declared capabilities and version | Support plugins and evolving agent protocols |

Use supported provider interfaces and document the evidence for each adapter. Check Claude Code,
Codex, pi and OpenCode individually; these are the current product targets, not the complete set
the API permits. Record protocol, streaming, tools, history, permissions, interrupt and terminal
support in an adapter capability matrix.

**Important feasibility check:** a native interactive CLI and a structured conversation API may
offer different modes. Verify whether they can share a live session before promising seamless
switching. A terminal view could mean the native CLI or the session's raw process output; those
are different capabilities. Agree that behaviour for each adapter. Any limitation affecting the
rich chat requirement must be raised explicitly rather than silently substituting a plain log.

**Proof required:** a real agent produces a streamed response, a tool operation, output, a change
and a permission request through the public contract. Input, reconnect and resume preserve the
same session. Test every provider claimed as supported, plus an external adapter.

### 5.5. Provide resource opening and reusable viewers

**Existing work to reuse: terminal image rendering and P044(F007)/T170.**

Provide one resource-opening API used by conversation attachments, file links and other apps.
A resource identifies its origin, type, location and optional revision or content identity.
A path originating on another computer must not be interpreted as a local file.

Viewers register the resource types they support. The user or calling action chooses a pane,
preview tab or other supported destination through normal configuration and actions. Respect an
explicit destination; report unavailable resources and unsupported viewers clearly.

Image, text/code, diff and browser viewers should share the generic content contract. Reuse image
rendering already present in heca. Review T170's ownership before adding another implementation.
Opening a Git change must preserve repository, worktree and revision context.

A browser also needs an actual engine integration, including navigation, clipping, focus, input
and lifecycle. The generic host and T454's demonstration are foundations, not that implementation.
Engine selection, platform support and browser-specific behaviour need a separate reviewed plan.

**Proof required:** open a chat attachment in a preview tab and a normal pane using the same
viewer implementation. Exercise loading, failure, switching and resizing. Verify resource origin
with different worktrees and a remote resource supplied through the agreed transport.

### 5.6. Compose a tabbed preview dock

**Reuse: `DockFrame`, `Tabs`, `Choice` and the shared content/viewer APIs.**

The dock owns its open-resource collection and selected tab. Each viewer owns its own input,
scrolling and content state. Preserve each tab's position and state when selecting another tab.

Users can move or hide the dock. Opening a resource should reuse an existing matching tab where
the agreed identity rules say it is the same resource. Closing a preview releases that view;
the lifetime of an underlying browser or process follows its explicit content policy.

Keep progress and preview independently placeable. A shared DockView selector displays one
selected provider, so progress is only continuously visible if its dock remains mounted in a
visible region alongside the preview. Compose this using the existing region/provider model.

**Proof required:** image, change and text tabs retain their state; the same content works in a
pane. Add a browser tab when browser integration is ready. Keyboard navigation comes from the
existing widget key configuration.

### 5.7. Supply shared state, events and restore hooks

**Existing owners: P104(F012), P099(F012), P100(F012) and P105(F012).**

The server holds running sessions and shared content state. Clients draw from snapshots and
changes. Each window keeps its own focus and scroll positions. Specify ownership for selected
tabs and drafts in the view-state review; draft sharing has not been decided.

Provide extension hooks for shared state, change notifications and versioned save/restore.
Pro owns its room history; generic agent session data and pro room messages should have distinct
ownership and be linked by identity where appropriate.

Define snapshot-to-live ordering, replay, duplicate handling, input acknowledgements and recovery
after a connection loss. Closing a client window leaves its agents running. Restarting a server
uses the existing restore/resume policy; it is different from reattaching to a live server.

Every client command uses the existing RPC action route and checks. Provider adapters can use
their program protocols behind those actions. Open heca's socket remains local; remote room and
resource traffic follows the agreed pro transport. A new browser or chat view is not a new
network entry point into heca.

**Proof required:** two windows show one conversation with independent view state. Detach and
reattach without starting another agent or duplicating messages. Restart and resume through the
documented policy, and verify pending requests remain consistent.

## 6. Conversation and room data flow

The rich agent transcript and the team conversation both contribute to the experience:

1. An agent adapter produces structured conversation and work events for its session.
2. The heca server stores and publishes the data required by attached views.
3. A conversation component renders that data through the generic rich widgets.
4. User input identifies a recipient/session and enters the normal action route.
5. Native conversation input reaches the registered agent adapter. Room/member messages use the
   existing pro helper and agent-comms route where appropriate.
6. Pro associates sessions, members, tasks and reports to build the progress dock and overview.

Room messages do not automatically contain every tool event from an agent. The current helper
protocol defines room/direct text messages and generic delivery/state events, not a typed full
transcript. Carrying transcript data across computers will need an agreed protocol addition at
that single helper boundary.

Link related room messages and agent turns by identity. Define delivery, human visibility and
inclusion in an agent's context separately. Showing a worker's output to the user does not by
itself instruct another agent or put that output into its conversation.

Generic blocks should accept registered extensions for additional reports and resource types.
Decide version compatibility and unknown-block presentation in the API review; preserve enough
information to explain unsupported content.

## 7. Proposed delivery order

This order expresses dependencies and usable checkpoints. Maintainers should reconcile it with
the current planner, active work and accepted priorities before scheduling changes.

| Checkpoint | Work | Evidence that unblocks the next consumer |
|---|---|---|
| A. Content API | Complete P094's public content path and T454 proof | An external crate mounts working non-terminal content in a pane and dock |
| B. Data and rendering contracts | Review P102 additions, adapter feasibility, and shared Markdown/message work with P103 and T172 | A documented event model, capability matrix and runnable rich widget examples |
| C. First real rich conversation | Connect a supported live agent to the reusable conversation components | Real streaming, work details, input and permissions through the normal action path |
| D. Mixed pinned workspace | Deliver column pinning after agreeing its task, then compose pro rooms and members | Orchestrator/worker chats alongside ordinary terminals and split panes |
| E. Previews | Resource opening and tabbed image, text and change viewers; browser on its reviewed implementation track | Resources open through one API in both docks and panes |
| F. Durable sessions | Complete the applicable server/socket/session/restore integrations | Multiple windows, detach/attach and restart/resume demonstrated |

Widget work and adapter feasibility can proceed alongside the content and server foundations.
These checkpoints are not a strictly serial schedule. A, B and the applicable P104 server/action
integration underpin C. D adds the pinning and pro integration. E can begin once the host and
resource contracts are ready. P104 precedes P099 in the existing server plan; socket-based worker
control and attached clients require that connection work. F depends on the applicable P099,
P100 and P105 work. A local widget demonstration proves rendering; the usable pro conversation
additionally needs its real agent and server integrations.

Browser delivery is a future capability in this direction. Its engine integration can follow
the shared host API. Rich chat remains the target of the first usable conversation, rather than
being deferred until all browser work is complete.

Review task dependencies as well as prose. For example, T452 and T454 describe preceding steps,
while their compact planner views currently list no dependency edges. Keep actual dependencies
consistent with the agreed delivery checkpoints.

## 8. Integrated acceptance demonstration

Deliver a reproducible demo setup and let Antonio exercise it in the running GPU application:

1. Open a workspace with an agent conversation, worker conversations, an ordinary terminal and
   a split column containing different content types.
2. Pin the orchestrator column. Scroll and focus worker columns while the pinned chat remains
   usable. Resize the window and change the font and theme settings.
3. Receive a streamed response with code, command output, a file change and an attachment.
   Expand output, select text, copy code and open the attachment in a preview tab.
4. Read older history while new events arrive. Confirm the reader's position is preserved.
5. Send a multi-line message, including IME input. Exercise the documented behaviour while the
   agent is busy, then a permission request, interruption and resume where supported.
6. Switch preview tabs, move the dock, and open the same kind of resource in a normal pane.
   Input and scrolling continue to belong to the content.
7. Inspect another worker. Its identity and the composer's recipient remain clear. Exercise the
   chosen room/workspace navigation policy after that decision has been made.
8. Attach a second window. Use different widths and independent scroll positions. Disconnect and
   reconnect one window, then exercise the server restart/resume policy.
9. Add content and an agent adapter from an external crate. Confirm that the public contracts
   work without changing host-private code or adding caller-specific workarounds.

Use focused tests for event ordering, duplicate handling, lifecycle, permissions, resource origin
and API access from an external crate. Add widget/showcase coverage and follow heca's existing
lint and changed-code gates. Visual behaviour requires the running demo, not only green tests.
Record the tested revision, provider versions, capability results and remaining limitations.

## 9. Decisions for the joint review

Settle these together before implementing the affected behaviour:

| Decision | Review needed |
|---|---|
| Direct worker conversation | Whether instructions, messages or summaries are visible to the orchestrator |
| Room selection | Whether selecting a room only selects its conversation or also activates its workspace |
| Pinning behaviour | Placement, multiple pins, narrow-window handling and shared versus per-window state |
| Chat/terminal relationship | Supported terminal mode and shared-session behaviour for each provider |
| Busy-agent input | Queue, steering and interruption semantics, exposed through declared capabilities |
| Composer controls | Configurable submit/newline actions, attachments and draft ownership |
| Resource opening | Default destination, focus behaviour, matching-tab identity and unavailable remote content |
| Browser integration | Engine, platforms, browser lifetime and support for the shared content contract |
| Compatibility and retention | Event/schema evolution, unknown blocks, history limits and saved-state versions |

Existing role permission rules continue to apply. Decisions about message visibility should not
silently change who can execute an action or bypass the established approval route.

## 10. Requested response from the heca developers

Please provide:

1. The owner and exact planner reference for each foundation. For pinning, confirm an existing
   implementation task or propose one for Antonio's approval; P082(F003)/T474 is related completed
   work, not that task.
2. Confirmation of which requirements fit existing tasks and which need approved scope changes.
   In particular, reconcile Markdown with T172 and image reuse with T170.
3. A reviewable public content API and structured agent contract, with external-crate examples.
   Identify unresolved provider limitations before finalizing the user interaction.
4. An agreed delivery order with explicit dependencies and the checkpoints above mapped to tasks.
5. A testable revision for each checkpoint, its demonstration instructions and supported adapters.
   Supply the upstream revision that pro can pin after the work is merged.

Create or reprioritize planner work only after scope and priority are agreed with Antonio. This
handoff supplies requirements and acceptance evidence. Its proposed additions become scheduled
work when the agreed changes are recorded in the planner.

## 11. References

- Optional background in the private heca-pro repository (repository access required):
  [architecture](https://github.com/ovidius72/heca-pro/blob/develop/docs/architecture.md),
  [components](https://github.com/ovidius72/heca-pro/blob/develop/docs/components.md),
  [open heca work](https://github.com/ovidius72/heca-pro/blob/develop/docs/open-heca-work.md)
  and [earlier decisions](https://github.com/ovidius72/heca-pro/blob/develop/docs/decisions.md).
- Heca planner: P094(F011), P102/P103/P104/P099/P100/P105(F012), P037(F007)/T172 and
  P044(F007)/T170. Read full task and phase context before executing; these are heca references.
- Pinning's related foundation: P082(F003)/T474, DONE. No dedicated pinning implementation task
  was found in the 2026-10-12 review; see sections 4 and 5.2 before scheduling.
  Its [full description and completion history](../.planner/docs/t474-column-identity-and-authoring-api.md)
  preserve the corrected future requirement.
- [Heca's P094 design](../.planner/docs/p094-pane-hosts-any-app.md).
- [Reviewed extension API](https://github.com/ovidius72/heca/blob/369a5fa7a8419929056a7b5927e6f537fc95f4d2/heca/src/entry/extension.rs),
  [pane shell](https://github.com/ovidius72/heca/blob/369a5fa7a8419929056a7b5927e6f537fc95f4d2/heca/src/chrome/pane/shell.rs),
  [workspace content model](https://github.com/ovidius72/heca/blob/369a5fa7a8419929056a7b5927e6f537fc95f4d2/heca/src/chrome/workspace/model.rs)
  and [widget catalog](https://github.com/ovidius72/heca/blob/369a5fa7a8419929056a7b5927e6f537fc95f4d2/docs/widgets.md).
- [Paseo](https://github.com/getpaseo/paseo) is the UI reference. Its conversation separates
  readable messages from inspectable work details. The reviewed source has structured renderers
  for shell output, edits and other tool results, plus copy controls for code. See its
  [tool detail renderer](https://github.com/getpaseo/paseo/blob/0ba4d2526ebed49e30271b16018b5e9464a79d21/packages/app/src/components/tool-call-details.tsx)
  and [code block component](https://github.com/getpaseo/paseo/blob/0ba4d2526ebed49e30271b16018b5e9464a79d21/packages/app/src/components/highlighted-code-block.tsx).
  These are references for the interaction; the implementation requested here uses heca's own
  content and widget APIs.
