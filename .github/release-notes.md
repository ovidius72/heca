## Download

| System | Installer | Archive |
|---|---|---|
| macOS (Apple Silicon) | the `.dmg` for aarch64 | `heca-<tag>-aarch64-apple-darwin.zip` |
| macOS (Intel) | the `.dmg` for x86_64 | `heca-<tag>-x86_64-apple-darwin.zip` |
| Linux (x86_64) | the `.deb` or the `.AppImage` | `heca-<tag>-x86_64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64) | the `.msi` | `heca-<tag>-x86_64-pc-windows-msvc.zip` |

The archive holds the `heca` program, the licences and the README. Everything heca needs is inside
the program.

## These builds are not notarized yet

- **macOS** says Apple cannot verify that the `.dmg` (or the app) is free of malware. Choose
  **Done**, then open **System Settings → Privacy & Security** and press **Open Anyway** next to the
  message about heca; do the same for the app the first time it starts. Or, in Terminal, remove the
  download mark before opening it: `xattr -d com.apple.quarantine ~/Downloads/heca_*.dmg` (and
  `xattr -dr com.apple.quarantine /Applications/heca.app` for the installed app). The app carries an
  ad-hoc signature, so it is never reported as damaged.
- **Windows** SmartScreen may say "Windows protected your PC". Choose **More info** → **Run anyway**.
- **Linux AppImage**: make it executable first — `chmod +x heca-*.AppImage`.
