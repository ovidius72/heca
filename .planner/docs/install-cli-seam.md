# CLI ↔ installer seam — `heca … integration <program>` (T544 C6 ↔ T174)

Agreed boundary: **P099 owns the CLI entry point and formatting; P102 owns the installer.** The CLI runs before the window — no session server, no socket. The same installer API backs the future settings UI (one implementation).

## CLI commands P099 adds (cli.rs `COMMANDS`, extended to accept subcommand words)

| command | calls |
| --- | --- |
| `heca install integration <program> [--json]`   | `apply(program, IntegrationOperation::Install)` |
| `heca update integration <program> [--json]`    | `apply(program, IntegrationOperation::Update)` |
| `heca uninstall integration <program> [--json]` | `apply(program, IntegrationOperation::Uninstall)` |
| `heca inspect integration <program> [--json]`   | `inspect(program)` |
| `heca list integrations [--json]`               | `list()` |

Each formats the returned `InstallationStatus` to text, or `serde_json` when `--json`.

## P102 API the CLI calls (already present, already serde — good)

```rust
Integrations::list(&self)                         -> io::Result<Vec<InstallationStatus>>
Integrations::inspect(&self, id: &str)            -> io::Result<InstallationStatus>
Integrations::apply(&self, id: &str, op: IntegrationOperation) -> io::Result<InstallationStatus>
// InstallationStatus / InstallationState / InstallationIssue / IntegrationOperation: Serialize ✓
```

## The one thing P099 needs from P102

A constructor the CLI (and the settings UI) can build with no P102 internals (`AgentDrivers` / `IntegrationContext` / config path):

```rust
impl Integrations {
    /// Default machine-local installer: registered drivers + the user's integration
    /// dirs + user config path. No session server or window.
    pub fn machine_local() -> std::io::Result<Self>;
}
```
(Name negotiable: `machine_local` / `for_this_machine` / `default_local`.)

## Open question for P102

Grammar: P099 proposes `heca <verb> integration <program>` (verb ∈ install/inspect/update/uninstall) + `heca list integrations`. If `heca integration <verb> <program>` reads better against your model, say so — P099 owns the grammar but it should fit the installer.

Status: **agreed 2026-10-22.** Codex accepted the grammar and **implemented** `Integrations::machine_local()` (P102 worktree: `heca/src/agent/integration/mod.rs:29`). P099 implements the CLI side in T544 C6, calling `Integrations::machine_local()?` then `apply`/`inspect`/`list`. Note: C6 compiles only once P102's `agent::integration` module is on this branch / `next`. No C3 impact.
