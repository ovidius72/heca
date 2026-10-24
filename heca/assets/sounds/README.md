# Default sound pack

These MP3 files were supplied by the maintainer on 2026-10-23 and are copied unchanged.
They are embedded in the Heca library, so packaged apps and embeddings need no checkout path.

| Asset | File | Default agent status |
| --- | --- | --- |
| `completed` | `completed1.mp3` | Finished |
| `error` | `error1.mp3` | Error |
| `locked` | `locked.mp3` | Waiting for permission |
| `attention` | `attention1.mp3` | Waiting for input |
| `success` | `success1.mp3` | Available for custom selection |

The event mapping lives in `config.default.toml`. Playback is disabled by default.
Users can select another asset or file, mute an individual status, and override its volume.
Additional assets use the same public `Extension::sound` registration path as the built-in pack.
