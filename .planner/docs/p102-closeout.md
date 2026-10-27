# P102 closeout — 2026-10-24

Code: `/Users/antonio/projects/heca-p102`, branch `feat/p102-agent`, base `8fab80e7`.
Changes are local and uncommitted. The eventual PR targets `next`.

This audit uses each task's current contract and Accepted Decisions. Phase decision
`db6feb71-3c67-45cd-9c50-2fba4feb19ca` permits completing the review here while
Claude/Opus is unavailable. It preserves explicit native-test limitations.

## Current verification

- The full scoped test command found the already documented `focus_door` scanner
  failure: it mistakes Input's private `View.focused` paint data for `Base.focused`.
  Both `heca/tests/focus_door.rs` and `heca-grid-ui/src/widgets/input/component.rs`
  match HEAD. Their sources and the guard were left unchanged.
- Running the remaining scoped suites passed **2,320 tests in 57 suites**, with
  three optional manual checks ignored and that one known scanner test excluded.
  Command: `cargo test -p heca-core -p heca-config -p heca-grid-ui -p heca-theme -p heca --lib --tests --locked -- --skip only_the_focus_door_writes_the_focused_flag`.
- Scoped Clippy passed with `-D warnings` for core, config, grid-ui, theme,
  renderer and heca, including their library and test targets.
- The actual installer CLI suite passed all four tests, covering every registered
  bundle and install/repeat/inspect/update/uninstall, text/JSON, errors and settings
  preservation. Real PTY and reporting-helper suites are included in the scoped run.
- Logs: `target/p102/closeout-tests.log`,
  `closeout-tests-with-baseline-excluded.log`, `closeout-clippy.log`, and
  `integration-cli-recheck.log` in the P102 worktree.

## Checklist evidence

### T171 — shared contract: 7/7 verified

| Item | Evidence |
| --- | --- |
| C1 Existing mechanisms and ownership | `CODEX-P102.md`, earlier checkpoint reviews, `heca-core/src/agent/driver.rs`, `heca-core/src/runtime/mod.rs`; contract belongs to open Heca. |
| C2 Contract proposed and agreed | The public `AgentDriver` trait and `Extension::agent_driver` follow the agreed recognition/prepare/read contract. Phase decision `227957d5-d092-4e5d-880a-ea72e37747ed` adds driver-owned artwork and reported session identity. The later declarative integration builder/plugin runtime remains T549. |
| C3 Agent state and change events | `PaneRuntime.agent` is separate from `ProcessStatus`; `ChromeEvent::PaneAgentChanged` carries previous/current shared `AgentState`; store and launch tests cover transitions and clearing. |
| C4 Notification escape codes | `heca-core/src/backend/terminal/osc.rs` emits notification input for OSC 9/99/777; the registered driver interprets it. Existing tests cover ordinary notifications and unrelated/invalid reports. |
| C5 Side channel | `heca-core/src/agent/channel/` and `session/`; bounded records, partial lines and closed endpoints are tested. Pi uses its installed extension and the same per-terminal channel. |
| C6 Added driver | `heca-core/src/agent/tests.rs`, `heca/src/agent/registration/tests.rs` and `heca/tests/agent_launch.rs`: external registration, scoping, recognition, preparation and reports through real PTY launch. |
| C7 Documentation | `docs/extending-heca.md` covers registration, installation, metadata, native constraints and settings. |

### T173 — pane row: 4/4 verified

| Item | Evidence |
| --- | --- |
| C1 Reuse existing widgets | `providers/workspaces/agent_line` composes Image, Grid, Flex, Icon, Label, PaneName and Visibility; `pane_row` binds pane facts. |
| C2 Configured agent segment | Existing pane-lines configuration controls placement. Reactive identity/status/task fields stay in the shared component. Row tests cover narrow layouts, identity and optional fields. |
| C3 Configured colours/icons | Status labels/glyphs/tones come from agent settings and the theme. Driver SVG marks use the shared Image path. Header, tooltip, theme, row and renderer tests pass. |
| C4 Native GPU use | Previous isolated native Heca checks exercised Claude waiting/working/error, two Codex waiting/working/finished sessions and Pi waiting/working/finished. Claude's successful response is separately unverified under T174 C2. Antonio approved the current icon/status/dialog/header styling, grouping and flash, including the corrected prefix+Enter startup sequence. |

The startup-flash regression reproduces the actual order: initial agent report,
then process-name/status discovery. Unchanged explicit agent work state stays
quiet during discovery. One pane-owned observer follows canonical pane focus.

### T174 — persistent integrations: 13/13 accepted

Antonio approved closing T174 on 2026-10-24 using Claude's real quota-error
cycle instead of requiring a successful model response. This changes the
acceptance criterion; it does not claim a successful Claude response.
In the existing HecaP102Audio GPU window, Claude Code v2.1.310 ran with the
installed hooks from `target/p102/live-ui/native/.claude`. Antonio accepted
the native workspace trust prompt and confirmed the row flash. Sending
`Reply with exactly OK. Do not use tools or change files.` visibly changed
the row from Waiting to Working to Error. Claude reported the weekly usage
limit, resetting Oct 26 at 7am Europe/Rome. No paid-credit option was selected.

C1 and C3–C9 and C11 were already checked and are retained. Current code and
tests confirm the installed bundles, owned registration, inspection, repeatable
installation, explicit update/uninstall, rollback, settings preservation and
normal native trust. Reporting and audio remain disabled until selected.

| Item | Result and evidence |
| --- | --- |
| C2 Native cycles, with accepted Claude quota-error outcome | **ACCEPTED by Antonio, 2026-10-24.** Codex, Pi and OpenCode have native completed-cycle evidence. The fresh Claude GUI test verified Waiting, Working and Error through the installed integration; Antonio confirmed the flash. A successful Claude model response remains unverified and is no longer required for this closeout under the explicit acceptance change above. Earlier evidence: `target/p102/reporting-review.md`, `native-metadata-review.md`, `native-metadata-live-{codex,claude}.json` and `live-native-cycles.json`. |
| C10 Persistent installation replaces temporary hooks | Driver prepare paths supply only the temporary connection/environment; Codex also supplies its session-specific MCP settings. Persistent native hooks/extensions remain installed. Real PTY tests cover typed commands, Heca launches and inactivity outside Heca. Native adapters and installed-hook/helper fixtures exercise the same paths. |
| C12 Shared transition contract coordinated with P099 | The agreed shape is `PaneAgentChanged { pane, previous, agent }` using the existing serializable `AgentState`; snapshots reuse `PaneRuntime.agent`. Backend updates are drained individually before the runtime mirror; the real PTY test preserves several transitions delivered in one write, and exit/disable tests verify clearing. P099 owns adding/transporting its Change variant; that consumer remains P099 work. No duplicate wire model or transport implementation is claimed here. |
| C13 Lifecycle/reporting verification, docs and review | Installer, external-driver, PTY/helper, config and adapter suites pass. Existing reviews plus this review cover the current contract, configuration/trust boundaries, ownership/recovery, background audio, shared UI components and the subsequent startup fixes. No additional blocking code findings were identified. Native Claude's completion limitation remains C2; socket transport remains P099. Review follows the phase's permission to proceed without Opus. |

The CLI connection itself is implemented and tested in
`heca/src/app/cli/{mod,catalog,integrations,queries}.rs` and
`heca/tests/integration_cli.rs`. See `install-cli-seam.md` for the P099 merge note.
An unfinished T544 does not mean these five installer commands are missing.
The historical T174/T544 coordination is recorded in the seam document. The
current canonical P099 snapshot does not contain T544; this audit does not claim
a live dependency edge or completion of the broader P099 transport work.

### T175 — sounds: DONE, 5/5 verified

| Item | Result and evidence |
| --- | --- |
| C1 Background playback | `heca/src/agent/sounds/worker.rs` owns the bounded queue and audio device on a separate thread. Status updates use nonblocking enqueue. |
| C2 Configurable and off by default | Shared config/source types provide per-status enabled/volume/file-or-asset choices and normal project overrides. Config and sound-policy tests pass. |
| C3 Changes only | AgentSounds subscribes once to the state-change event. Duplicate states, metadata-only updates, clearing and disabled playback are tested as silent. |
| C4 Bundled defaults | Five supplied MP3 assets are embedded and decoded by the shared audio library. Default mappings live in config; external sound registration uses the existing extension path. No runtime Downloads dependency. |
| C5 Heard in the app with a real agent | **VERIFIED.** In the isolated native Heca P102 Audio GPU window, real Pi v1.4.0 was started with `pi --no-session` through the ordinary zsh/shared-launcher path. Its installed integration reported `waiting_input`; configured audio used the bundled attention sound at volume 0.25. After a repeat launch Antonio explicitly confirmed: “Sì, l’ho sentito”. This is the real installed-agent/app/audio path, not the sound simulation. No model request was submitted. Runtime setup/log: `target/p102/HecaP102Audio.app`, `native-audio-zsh/`, `native-audio-runtime.log`. |

## Final acceptance

Final ledger: **29/29 accepted**, including the explicitly revised T174 C2.
T171, T173 and T175 were already DONE; T174 is approved for completion.
No implementation work remains within the agreed P102 scope. A successful
Claude native response is still unverified because of the account quota.
The approval accepts the real error outcome; it does not turn it into a success.

Antonio asked to leave the current focused-pane flash behavior unchanged. This
closeout changes planner records and evidence only. Existing code/test results
are reused; there were no production source edits requiring another test run.
