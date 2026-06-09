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

## Phosphor Duotone (`Phosphor-Duotone.ttf`)

- **Family:** Phosphor-Duotone (the icon glyph font)
- **License:** MIT — see `PHOSPHOR-LICENSE.txt` (free to embed & redistribute)
- **Source:** https://phosphoricons.com — `@phosphor-icons/web` (`src/duotone/`)

The embedded **icon** font. The renderer registers it as a second family and
selects it for `FontRole::Icon` text runs; the [`Icon`](../src/widgets/icon.rs)
widget renders a glyph from it. Duotone icons are **consecutive codepoint
pairs** — a secondary (`:before`) layer drawn dimmed and a primary (`secondary +
1`) layer on top; `Icon` stacks both. The dim factor is the theme-configurable
`Theme.icon_secondary_alpha`, and glyph colors come from the theme — nothing is
hardcoded in the widget.
