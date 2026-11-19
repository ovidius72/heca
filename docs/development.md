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

## Releasing

A release is made **only** by pushing a version tag: `vX.Y.Z`, equal to the workspace version in
`Cargo.toml`. A pre-release takes a **number** after the dash — `v0.1.0-1`, not `v0.1.0-rc1` —
because the Windows installer accepts nothing else; it needs that exact version in `Cargo.toml` too.

```bash
git tag v0.1.0
git push origin v0.1.0
```

[`.github/workflows/release.yml`](../.github/workflows/release.yml) then checks the tag against
`Cargo.toml` (and stops if they differ), builds heca for macOS (Apple Silicon and Intel), Linux and
Windows, makes the installers with [cargo-packager](https://github.com/crabnebula-dev/cargo-packager)
— `.dmg` and `.app`, `.deb` and `.AppImage`, `.msi` — and an archive per system with the program,
the licences and the README, and publishes them all in a GitHub Release with the notes in
[`.github/release-notes.md`](../.github/release-notes.md). A tag with a `-` is published as a
pre-release. The installer settings are in `heca/Cargo.toml` under `[package.metadata.packager]`.

To try the installers locally (macOS shown):

```bash
cargo install cargo-packager --locked
cargo build -p heca --release
cargo packager -p heca --release -f app,dmg -o dist
```



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
