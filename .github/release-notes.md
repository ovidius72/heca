## Download

| System | Installer | Archive |
|---|---|---|
| macOS (Apple Silicon) | the `.dmg` for aarch64 | `heca-<tag>-aarch64-apple-darwin.zip` |
| macOS (Intel) | the `.dmg` for x86_64 | `heca-<tag>-x86_64-apple-darwin.zip` |
| Linux (x86_64) | the `.deb` or the `.AppImage` | `heca-<tag>-x86_64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64) | the `.msi` | `heca-<tag>-x86_64-pc-windows-msvc.zip` |

The archive holds the `heca` program, the licences and the README. Everything heca needs is inside
the program.

## These builds are not signed yet

- **macOS** says the app "cannot be opened because the developer cannot be verified". Open it once
  with right-click → **Open**, or run `xattr -dr com.apple.quarantine /Applications/heca.app`.
- **Windows** SmartScreen may say "Windows protected your PC". Choose **More info** → **Run anyway**.
- **Linux AppImage**: make it executable first — `chmod +x heca-*.AppImage`.
