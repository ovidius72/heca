# Embedded fonts

## Geist Mono (`GeistMono-Regular.ttf`, `GeistMono-Bold.ttf`)

- **Family:** Geist Mono (both weights register under the same family name)
- **License:** SIL Open Font License 1.1 — see `OFL.txt` (free to embed & redistribute)
- **Source:** https://github.com/vercel/geist-font (`packages/next/dist/fonts/geist-mono/`)

The **default** mono font for the Grid UI, embedded so the look renders
identically without a system install. Defaults, not hardcoded overrides —
`Theme.font_family` / `Theme.font_size` remain fully configurable; weight is
selected at shaping time via `Attrs::weight`. Geist Mono ships no italic face,
so italic is synthesized as an oblique skew.
