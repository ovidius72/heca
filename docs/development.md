# Developing heca — running, testing and tuning

Run these commands from the repository root. There is one base application and two development
programs, each serving a different purpose:

| Program | Command | What it runs |
| --- | --- | --- |
| **Heca** | `cargo run -p heca` | The base application: workspaces, columns, panes and terminals. |
| **Extension example** | `cargo run -p heca --example extend_heca` | Heca with example contributions: a **Notes** dock with a terminal in the right sidebar, actions and a named layer. These additions belong to the example. |
| **Widget showcase** | `cargo run -p heca-renderer --example showcase` | An interactive gallery of the UI library's widgets and compositions. |

The base program calls `heca::run()`. The extension example registers its contributions first,
then calls the same `heca::run()`. Its complete source and usage are in
[`heca/examples/extend_heca.rs`](../heca/examples/extend_heca.rs) and
[Building on Heca](extending-heca.md). The showcase source is
[`heca-renderer/examples/showcase/main.rs`](../heca-renderer/examples/showcase/main.rs), with widget
instructions in the [widget reference](widgets.md).

## Manual app tests without attaching to the usual server

The base application and extension example accept `--standalone`:

```bash
# Base application, in a session that ends when its window closes
cargo run -p heca -- --standalone

# Extension example, including its right-sidebar terminal
cargo run -p heca --example extend_heca -- --standalone
```

The `--` separates Cargo's options from arguments passed to the program. Standalone mode does
not attach to the background server; see [Sessions — detach and reattach](sessions.md)
for normal server-backed use.

For UI changes, test the affected widgets in the showcase and check the base application's
behavior. For Provider, dock, layer or extension API changes, also run the extension example.
Check input, focus, scrolling, resizing and opening/closing the affected surfaces. Record manual
runtime results alongside automated test results.

Cargo discovers these programs from the package manifests and example sources. When adding or
renaming a runnable program, update this table and its linked usage guide.

## Continuous integration

Every pull request, and every push to `main`, runs [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)
on Linux, macOS and Windows. Each system runs, with the Rust version pinned in
[`rust-toolchain.toml`](../rust-toolchain.toml):

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

A warning fails the build. One system failing does not stop the other two. To reproduce a run
locally, use those same two commands. They build the whole workspace and take a long time, so
day to day scope cargo to the crates you changed, for example `./scripts/lint-changed.sh` and
`./scripts/lint-changed.sh test`.

## Frame timing

For tuning the render path, run heca with `HECA_FRAME_TIMES` set (any value):

```sh
HECA_FRAME_TIMES=1 cargo run --release -p heca
```

Every 120 frames that drew, it prints one line on stderr — the average time `render_frame` took on the
CPU and the average number of render passes it began:

```text
[frame-times] 120 frames: 1.84 ms CPU, 9.0 render passes per frame (GPU time is not measured)
```

It is CPU time only: the GPU runs after the frame is submitted, so work moved from the CPU to the GPU
looks like a win here. Off unless the variable is set.
