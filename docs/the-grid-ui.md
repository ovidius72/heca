# The GridCN — Design System & Component Library Analysis

> Full documentation of [thegridcn.com](https://thegridcn.com), a Tron: Ares inspired shadcn/ui theme system by [@educlopez](https://github.com/educlopez/thegridcn-ui).
>
> Retrieved 2026-06-03. ~304 stars, MIT license, TypeScript, Next.js 16, React 19.

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Theme System](#theme-system)
4. [Component Catalog](#component-catalog)
5. [Key Design Patterns](#key-design-patterns)
6. [Component Implementation Patterns](#component-implementation-patterns)
7. [CSS Variable System](#css-variable-system)
8. [Registry & Install System](#registry--install-system)
9. [3D Effects Pipeline](#3d-effects-pipeline)
10. [Lessons for heca-ui](#lessons-for-heca-ui)

---

## Overview

The GridCN transforms standard shadcn/ui into a neon, glass-HUD design system modeled after *Tron: Ares*. It ships:

- **55+ base UI components** (shadcn/ui re-exports with Tron styling)
- **90+ Tron-flavored components** (HUDs, radars, timers, maps, data cards, 3D grids)
- **6 themes** built on `oklch()` CSS variables with `data-theme` attribute switching
- **3D effects** via Three.js (animated grid, tunnel, particles, light beams)
- **shadcn CLI-native registry** — each component is a single `npx shadcn add` away

### Stack

| Layer | Technology |
|-------|-----------|
| Framework | Next.js 16 (App Router) |
| UI Library | React 19 |
| Styling | Tailwind CSS 4 + CSS custom properties |
| Unstyled Components | Radix UI primitives |
| Theming | `data-theme` attribute + `oklch()` CSS variables |
| 3D | Three.js via `@react-three/fiber` |
| Component Variants | `class-variance-authority` (cva) |
| CLI Integration | shadcn registry format |
| Package Manager | pnpm |

### Design Philosophy

- **Copy-paste ownership**: Every component is raw TSX you own. Nothing is locked behind a package.
- **Theme-first**: All colors come from CSS variables. Components never hardcode colors.
- **Grid-native**: Corner brackets, scanlines, glowing borders, and monospace/HUD typography throughout.
- **Progressive intensity**: Tron effects scale from "Off" (standard shadcn) to "Heavy" (full Tron aesthetic with animations).

---

## Architecture

### Project Layout

```
project-ares/
├── src/
│   ├── app/                      # Next.js App Router
│   │   ├── components/           # Component showcase page
│   │   ├── page.tsx              # Homepage
│   │   └── globals.css           # Global styles + all 6 theme definitions
│   ├── components/
│   │   ├── ui/                   # 55+ shadcn base components (re-exported with Tron styling)
│   │   ├── thegridcn/            # 90+ Tron-flavored components + 3D + effects + movie UI
│   │   ├── theme/                # ThemeProvider, useTheme, ThemeSwitcher, TronIntensitySwitcher
│   │   ├── showcase/             # Component showcase sections (demo pages)
│   │   └── layout/               # Layout components (header, sidebar, footer)
│   ├── hooks/                    # Custom React hooks
│   ├── lib/                      # cn() utility, registry helpers
│   └── registry/                 # Component registry configuration
├── public/
│   ├── r/                        # Generated registry JSON (one per component)
│   └── tokens/                   # Generated theme tokens (CSS + JSON per theme)
├── scripts/                      # Registry + token build scripts
├── docs/install.md               # shadcn CLI install guide
└── components.json               # shadcn/ui configuration
```

### Two-Tier Component Architecture

The library splits into two component tiers:

**`src/components/ui/`** — Base shadcn/ui components (55+):
`accordion`, `alert`, `badge`, `button`, `card`, `checkbox`, `dialog`, `dropdown-menu`, `form`, `input`, `label`, `select`, `separator`, `sheet`, `sidebar`, `switch`, `table`, `tabs`, `textarea`, `tooltip`, etc.

These are standard shadcn/ui components that work exactly as documented — they pick up theme variables automatically through CSS.

**`src/components/thegridcn/`** — Tron-specific components (90+):
`data-card`, `hud`, `radar`, `reticle`, `uplink-header`, `status-bar`, `derez-timer`, `identity-disc`, `grid-scan-overlay`, `grid-floor`, `tunnel`, `boot-sequence`, `terminal`, `coordinate-display`, `energy-meter`, `signal-indicator`, `gauge`, `waveform`, `beam-marker`, `map`, `light-cycle-engine`, etc.

These are original components with Tron aesthetic built in.

---

## Theme System

### How It Works

Themes use a `data-theme` attribute on `<html>` combined with CSS custom properties (`oklch()` color space).

```html
<html data-theme="ares">
```

```css
[data-theme="ares"] {
  --background: oklch(0.08 0.03 25);
  --foreground: oklch(0.95 0.01 25);
  --primary: oklch(0.6 0.25 25);
  --border: oklch(0.3 0.12 25);
  --glow: oklch(0.6 0.25 25);
}
```

All components use Tailwind's `bg-background`, `text-foreground`, `border-border`, etc., which resolve through the CSS cascade.

### Theme Variants (6 + 1 hidden)

| ID | Name | God/Inspiration | Primary Color | Vibe |
|----|------|-----------------|---------------|------|
| `tron` | Tron | User | Cyan `oklch(0.75 0.18 195)` | Classic Tron blue |
| `ares` | Ares | God of War | Red `oklch(0.6 0.25 25)` | War room, danger |
| `clu` | Clu | Program | Orange `oklch(0.68 0.22 55)` | Rogue program |
| `athena` | Athena | Goddess of Wisdom | Gold `oklch(0.75 0.18 85)` | Divine, regal |
| `aphrodite` | Aphrodite | Goddess of Love | Pink `oklch(0.65 0.22 350)` | Neon romance |
| `poseidon` | Poseidon | God of Sea | Blue `oklch(0.6 0.2 250)` | Deep ocean |
| `creator` | Creator | Architect | White `oklch(1 0 0)` | Hidden, system-only |

### Tron Intensity System

Four levels controlling glow/border effects via `data-tron-intensity`:

| Level | Description |
|-------|-------------|
| `none` | Standard shadcn style (no Tron effects) |
| `light` | Subtle glows, enhanced borders |
| `medium` | Glowing borders with corner brackets |
| `heavy` | Full Tron aesthetic with animations |

### ThemeProvider

```tsx
// Wrap root layout
<ThemeProvider defaultTheme="ares">{children}</ThemeProvider>

// Use anywhere
const { theme, setTheme, tronIntensity, setTronIntensity } = useTheme();
```

- Persists to `localStorage`
- Sets `data-theme` and `data-tron-intensity` on `<html>`
- Uses `React.createContext` + `React.useCallback` for stable references

### Token Export System

Themes can be exported as standalone CSS/JSON for use outside the library:

```
public/tokens/
  index.json         · manifest of all themes
  tron.css           · copy-paste CSS block
  tron.json          · flat vars object { "--primary": "oklch(...)", ... }
  ares.{css,json}
  clu.{css,json}
  athena.{css,json}
  aphrodite.{css,json}
  poseidon.{css,json}
```

Usage:
```css
@import "https://thegridcn.com/tokens/ares.css";
```

### CSS Variable Categories

| Category | Variables | Examples |
|----------|-----------|---------|
| Core surfaces | `--background`, `--foreground` | Dark bg, light text |
| Primary | `--primary`, `--primary-foreground` | Theme accent color |
| Secondary | `--secondary`, `--secondary-foreground` | Supporting color |
| Muted | `--muted`, `--muted-foreground` | Subtle UI |
| Accent | `--accent`, `--accent-foreground` | Highlight |
| Destructive | `--destructive` | Red error |
| Borders | `--border`, `--input`, `--ring` | Outline colors |
| Surfaces | `--card`, `--popover` | Elevated surfaces |
| Glow | `--glow`, `--glow-muted` | Neon glow effects |
| Chart | `--chart-1` through `--chart-5` | Data viz |
| Sidebar | `--sidebar*` (8 vars) | Sidebar surfaces |
| Misc | `--radius` | Border radius |

All use `oklch()` color space for perceptual uniformity and easy hue shifting.

---

## Component Catalog

### Base UI Components (55+ in `src/components/ui/`)

| Component | Features |
|-----------|----------|
| `accordion` | Collapsible sections with Radix primitives |
| `alert` | Variants: default, destructive |
| `badge` | Variants: default, secondary, destructive, outline |
| `button` | 6 variants + 5 sizes, supports `asChild` (Slot) |
| `button-group` | Grouped button layout |
| `card` | Header, content, footer sub-components |
| `checkbox` | Radix-based with label |
| `dialog` | Modal dialog with Radix |
| `dropdown-menu` | Popup menu with Radix |
| `form` | React Hook Form integration |
| `input` | Styled text input |
| `label` | Form label with Radix |
| `select` | Native/styled select |
| `separator` | Horizontal/vertical divider |
| `sheet` | Slide-in panel with Radix |
| `sidebar` | Full sidebar navigation system |
| `switch` | Toggle switch with Radix |
| `table` | Data table with sort examples |
| `tabs` | Tab navigation with Radix |
| `textarea` | Multi-line input |
| `toggle` | Press/toggle state button |
| `tooltip` | Hover tooltip with Radix |
| `spinner` | Loading indicator |

Plus: `alert-dialog`, `aspect-ratio`, `avatar`, `breadcrumb`, `calendar`, `carousel`, `chart` (recharts), `collapsible`, `command` (cmdk), `context-menu`, `drawer` (vaul), `empty`, `field`, `hover-card`, `input-group`, `input-otp`, `item`, `kbd`, `menubar`, `navigation-menu`, `pagination`, `popover`, `progress`, `radio-group`, `scroll-area`, `skeleton`, `slider`, `sonner` (toast), `tabs`, `toggle-group`, `tooltip`.

### Tron-Specific Components (90+ in `src/components/thegridcn/`)

#### HUD / Overlay

| Component | Description |
|-----------|-------------|
| `hud` | Heads-up display frame with scanlines |
| `hud-frame` | Individual HUD panel frame |
| `hud-corner-frame` | Corner bracket decorations |
| `uplink-header` | System status bar with left/right text |
| `status-bar` | Animated status bar with progress |
| `status-dot` | Status indicator dot (green/red/yellow) |
| `reticle` | Scanning reticle overlay |
| `grid-scan-overlay` | Animated scanning grid effect |
| `crt-effect` | CRT monitor scanline + curvature effect |
| `glow-container` | Wrapper with neon glow on hover |

#### Data Display

| Component | Description |
|-----------|-------------|
| `data-card` | Movie-accurate dossier card with corner brackets |
| `stat` | Single statistic display |
| `stat-card` | Card with metrics and sparkline |
| `metric-row` | Row of key performance metrics |
| `coordinate-display` | Grid coordinates readout |
| `energy-meter` | Power/energy level bar |
| `gauge` | Circular speed/power gauge |
| `signal-indicator` | Signal strength bars |
| `waveform` | Audio/wave display |
| `sparkline` | Mini chart line |

#### Navigation & Structure

| Component | Description |
|-----------|-------------|
| `sidebar-nav` | Sidebar navigation with active states |
| `breadcrumb-nav` | Breadcrumb path with separators |
| `tabs` | Tron-styled tab navigation |
| `pagination` | Page navigation |
| `accordion` | Tron-styled accordion |
| `stepper` | Multi-step progress indicator |

#### Feedback & Alerts

| Component | Description |
|-----------|-------------|
| `alert` | Tron-styled alert with icon variants |
| `toast` | Notification toast |
| `notification` | Slide-in notification |
| `anomaly-banner` | Alert banner for system anomalies |
| `boot-sequence` | Animated boot/startup sequence |
| `derez-timer` | De-resolution countdown timer |
| `countdown` | General countdown display |
| `progress-bar` | Horizontal progress |
| `progress-ring` | Circular progress |

#### Cards & Containers

| Component | Description |
|-----------|-------------|
| `card` | Tron-styled card with glow |
| `bento-grid` | Grid layout for dashboards |
| `floating-panel` | Floating/glass panel |
| `feature-card` | Feature showcase card |
| `pricing-card` | Pricing tier card |
| `testimonial-card` | Quote/testimonial card |
| `stat-card` | Statistics/metrics card |

#### Forms & Input

| Component | Description |
|-----------|-------------|
| `text-input` | Tron-styled text input |
| `search-input` | Search with icon |
| `select` | Tron-styled select |
| `toggle` | Tron toggle switch |
| `slider` | Tron range slider |
| `date-picker` | Date selection |
| `tag-input` | Multi-tag input |
| `alias-input` | Identity alias input |
| `file-upload` | Drag-and-drop upload |
| `number-input` | Stepper number input |
| `rating` | Star rating input |
| `chip` | Small tag/chip |

#### 3D / Three.js

| Component | Description |
|-----------|-------------|
| `grid` | Interactive 3D grid (three.js) |
| `grid-floor` | 3D grid floor plane |
| `tunnel` | 3D tunnel/wormhole effect |
| `light-cycle-engine` | Light cycle game engine (canvas) |
| `light-cycle-game` | Playable light cycle game |
| `identity-disc` | Rotating 3D identity disc |
| `agent-avatar` | 3D character avatar |

#### Dashboard / Layout

| Component | Description |
|-----------|-------------|
| `kanban-board` | Drag-and-drop kanban |
| `data-table` | Interactive data table |
| `heatmap` | Calendar heatmap |
| `timeline` | Vertical timeline |
| `timeline-bar` | Horizontal timeline/roadmap |
| `leaderboard` | Ranked list |
| `activity-feed` | Scrollable activity stream |
| `changelog` | Version changelog |
| `faq` | Accordion FAQ section |
| `comparison-table` | Feature comparison |
| `marquee` | Scrolling marquee |
| `hero-section` | Full-width hero |
| `cta-banner` | Call-to-action banner |
| `newsletter-form` | Email signup form |
| `footer` | Page footer |
| `logo-cloud` | Client logo grid |
| `stats-counter` | Animated counter |
| `video-player` | Video player wrapper |
| `video-progress` | Video progress bar |

#### Code / Developer

| Component | Description |
|-----------|-------------|
| `code-block` | Syntax-highlighted code with copy |
| `copy-button` | Copy-to-clipboard button |
| `install-command` | CLI command display with copy |
| `terminal` | Terminal emulator display |
| `command-menu` | Cmd+K command palette |

#### Utility

| Component | Description |
|-----------|-------------|
| `divider` | Section divider with label |
| `empty-state` | Empty state placeholder |
| `skeleton` | Loading skeleton |
| `avatar-group` | Stacked avatars |
| `droppable` | Drag-and-drop zone |
| `beam-marker` | Map/radar beam marker |
| `map-marker` | Map location marker |
| `map` | Interactive map |
| `location-display` | Location coordinates |
| `arrival-panel` | Arrival/departure board |
| `regen-indicator` | Regeneration/health indicator |
| `template-previewer` | Template preview in modal |
| `speed-indicator` | Speed/signal meter |

### Templates (5 full-page layouts)

| Template | Contents |
|----------|----------|
| Dashboard | Collapsible sidebar, metric cards, charts, data table, activity feed, widget cluster |
| Landing Page | Hero, feature grid, pricing cards, testimonials, comparison table, FAQ, newsletter |
| Blog | Two-column layout, TOC sidebar, code blocks, author bio, newsletter CTA, related articles |
| Login | Split-panel with decorative branding, email/password form, social login buttons, circuit background |
| Analytics | Tab navigation, KPI cards, area/bar/pie charts, heatmap, data tables, real-time metrics |

---

## Key Design Patterns

### 1. CSS Variable-Driven Theming

Every color comes from CSS variables. Components never hardcode colors.

```tsx
// ✅ Good — uses CSS variables
<div className="bg-card text-card-foreground border-border" />

// ❌ Bad — hardcoded color
<div className="bg-[#1a1a2e] text-white" />
```

### 2. Variant System via `class-variance-authority`

```tsx
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/80",
        destructive: "bg-destructive text-white hover:bg-destructive/90",
        outline: "border bg-background shadow-xs hover:bg-muted",
        ghost: "hover:bg-muted",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 rounded-md px-3",
        lg: "h-10 rounded-md px-6",
        icon: "size-9",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  }
);
```

### 3. Radix UI Primitive Composition

Interactive components build on Radix UI unstyled primitives:

```tsx
import * as TabsPrimitive from "@radix-ui/react-tabs";

const Tabs = TabsPrimitive.Root;
const TabsList = React.forwardRef(/*...*/);
const TabsTrigger = React.forwardRef(/*...*/);
const TabsContent = React.forwardRef(/*...*/);
```

### 4. Corner Bracket Decorations

A signature Tron visual — L-shaped corner brackets in the primary color:

```tsx
<div className="pointer-events-none absolute left-0 top-0 h-4 w-4 border-l-2 border-t-2 border-primary/50" />
<div className="pointer-events-none absolute right-0 top-0 h-4 w-4 border-r-2 border-t-2 border-primary/50" />
<div className="pointer-events-none absolute bottom-0 left-0 h-4 w-4 border-b-2 border-l-2 border-primary/50" />
<div className="pointer-events-none absolute bottom-0 right-0 h-4 w-4 border-b-2 border-r-2 border-primary/50" />
```

### 5. Scanline Overlay

CSS gradient "scanlines" for CRT/HUD authenticity:

```css
bg-[repeating-linear-gradient(0deg,transparent,transparent_2px,rgba(0,0,0,0.03)_2px,rgba(0,0,0,0.03)_4px)]
```

### 6. `data-slot` Attribute Convention

Every component exposes a `data-slot` attribute for testing/styling:

```tsx
<div data-slot="tron-data-card" data-status={status}>
```

### 7. `cn()` Utility for Class Merging

Built on `clsx` + `tailwind-merge`:

```tsx
import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

### 8. `asChild` Prop (Slot Pattern)

Components that wrap semantic elements support Radix's `Slot` pattern:

```tsx
const Comp = asChild ? Slot : "button";
<Comp className={...} {...props} />
```

### 9. Glow Effects

Two-tier glow CSS variables + Tailwind utilities:

```css
--glow: oklch(0.75 0.18 195);
--glow-muted: oklch(0.5 0.12 195);
```

```tsx
<div className="shadow-[0_0_15px_var(--glow)]" />
```

The `tron-intensity` attribute controls whether glow effects are visible.

### 10. Typography Pattern

- **Uppercase labels**: `text-[10px] uppercase tracking-widest text-foreground/80`
- **Monospace values**: `font-mono text-sm uppercase tracking-wide`
- **Pipes as separators**: `<span className="text-primary">|</span>`
- **Fonts**: Orbitron (headings), Rajdhani (body) — loaded from Google Fonts

---

## Component Implementation Patterns

### Pattern A: Simple Stateless Presentation

```tsx
export function StatusDot({ status = "active" }: { status?: "active" | "inactive" | "alert" }) {
  const colors = {
    active: "bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]",
    inactive: "bg-gray-500",
    alert: "bg-red-500 shadow-[0_0_8px_rgba(239,68,68,0.6)]",
  };
  return <span className={cn("inline-block h-2 w-2 rounded-full", colors[status])} />;
}
```

### Pattern B: Composable Sub-components

```tsx
// Card pattern: Card.Header, Card.Content, Card.Footer
<Card>
  <Card.Header>
    <Card.Title>Title</Card.Title>
    <Card.Description>Description</Card.Description>
  </Card.Header>
  <Card.Content>Body</Card.Content>
  <Card.Footer>Actions</Card.Footer>
</Card>
```

Implemented via exported sub-components:
```tsx
const Card = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("rounded-xl border bg-card text-card-foreground shadow", className)} {...props} />
  )
);

const CardHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(/*...*/);
const CardTitle = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(/*...*/);
const CardContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(/*...*/);
const CardFooter = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(/*...*/);

export { Card, CardHeader, CardTitle, CardContent, CardFooter };
```

### Pattern C: Interactive with Context

Dropdown menu uses Radix primitives + compound components:

```tsx
<DropdownMenu>
  <DropdownMenuTrigger>Open</DropdownMenuTrigger>
  <DropdownMenuContent>
    <DropdownMenuItem>Item 1</DropdownMenuItem>
    <DropdownMenuItem>Item 2</DropdownMenuItem>
    <DropdownMenuSeparator />
    <DropdownMenuItem>Item 3</DropdownMenuItem>
  </DropdownMenuContent>
</DropdownMenu>
```

### Pattern D: Theme-Consuming with Variants

```tsx
export function Badge({ variant = "default", className, ...props }) {
  return (
    <div
      className={cn(
        "inline-flex items-center rounded-md border px-2.5 py-0.5 text-xs font-semibold transition-colors",
        variant === "default" && "border-transparent bg-primary text-primary-foreground",
        variant === "secondary" && "border-transparent bg-secondary text-secondary-foreground",
        variant === "destructive" && "border-transparent bg-destructive text-destructive-foreground",
        variant === "outline" && "text-foreground",
        className
      )}
      {...props}
    />
  );
}
```

---

## CSS Variable System (Complete)

### Per-Theme Variables (16-17 vars per theme)

Each theme defines the same set of variables, only the `oklch()` values differ:

| Variable | Purpose | Tron Example |
|----------|---------|-------------|
| `--background` | Main page bg | `oklch(0.06 0.02 250)` |
| `--foreground` | Main text color | `oklch(0.95 0.02 220)` |
| `--primary` | Theme accent | `oklch(0.75 0.18 195)` |
| `--primary-foreground` | Text on primary | `oklch(0.1 0 0)` |
| `--card` | Card bg | `oklch(0.1 0.02 250)` |
| `--card-foreground` | Card text | `oklch(0.92 0.02 220)` |
| `--popover` | Popup bg | `oklch(0.1 0.02 250)` |
| `--popover-foreground` | Popup text | `oklch(0.92 0.02 220)` |
| `--secondary` | Secondary surfaces | `oklch(0.18 0.05 200)` |
| `--secondary-foreground` | Secondary text | `oklch(0.95 0 0)` |
| `--muted` | Muted surfaces | `oklch(0.15 0.03 200)` |
| `--muted-foreground` | Muted text | `oklch(0.65 0 0)` |
| `--accent` | Highlight surfaces | `oklch(0.7 0.15 195)` |
| `--accent-foreground` | Highlight text | `oklch(0.1 0 0)` |
| `--destructive` | Error/danger | `oklch(0.55 0.25 30)` |
| `--border` | Borders | `oklch(0.3 0.1 195)` |
| `--input` | Input borders | `oklch(0.2 0.06 195)` |
| `--ring` | Focus ring | `oklch(0.75 0.18 195)` |
| `--radius` | Border radius | `0.5rem` |
| `--glow` | Glow effect color | `oklch(0.75 0.18 195)` |
| `--glow-muted` | Subtle glow | `oklch(0.5 0.12 195)` |
| `--chart-1` through `--chart-5` | Chart colors | — |
| `--sidebar*` | Sidebar (8 vars) | — |

### Global (non-theme) Variables

```css
:root {
  --font-orbitron: 'Orbitron', sans-serif;
  --font-rajdhani: 'Rajdhani', sans-serif;
}
```

---

## Registry & Install System

### shadcn-Compatible Registry

The site exposes a shadcn-compatible registry at `https://thegridcn.com/r/{name}.json`.

Register once in `components.json`:

```json
{
  "registries": {
    "@thegridcn": "https://thegridcn.com/r/{name}.json"
  }
}
```

Then install:

```bash
npx shadcn@latest add @thegridcn/button
npx shadcn@latest add @thegridcn/data-card
npx shadcn@latest add @thegridcn/theme-ares
```

### Registry Item Schema

Each JSON file at `public/r/{name}.json` follows the shadcn registry schema:

```json
{
  "name": "data-card",
  "type": "registry:component",
  "registryDependencies": [],
  "dependencies": [],
  "files": ["src/components/thegridcn/data-card.tsx"]
}
```

Theme items use `"type": "registry:style"` and write a CSS file.

### Token Export Build

```bash
pnpm tokens:build    # regenerates public/tokens/
```

---

## 3D Effects Pipeline

### Components

- **`TronGrid3D`** — Interactive 3D grid with particles and light beams
- **`TronTunnel`** — 3D tunnel/wormhole effect  
- **`TronGrid`** — 3D grid floor plane

### Technical Approach

- Built on `@react-three/fiber` (React renderer for Three.js) + `@react-three/drei` (utility helpers)
- SSR-safe: 3D components only render on client via dynamic imports
- Optimizations: instanced meshes, hoisted shaders, GPU buffer reuse, DPR limiting
- `R3F` Suspense boundaries for loading states

### Key Rendering Pattern

```tsx
"use client";
import dynamic from "next/dynamic";

const Grid3D = dynamic(() => import("@/components/thegridcn/grid"), { ssr: false });
```

---

## Lessons for heca-ui

### What The GridCN does well

1. **Theme-first architecture**: Zero hardcoded colors. Every component reads from CSS variables.
2. **Consistent variant system**: `cva` makes adding variants mechanical and predictable.
3. **Compound components**: Card.Header, Card.Content, Card.Footer patterns enable composition without CSS config.
4. **Progressive enhancement**: TronIntensity lets users dial effects from subtle to full neon.
5. **Copy-paste distribution**: No package dependency — every component is owned by the consumer.
6. **95+ components**: Covers everything from simple buttons to full-page templates.
7. **CLI-first install**: `npx shadcn add` is the only install path.
8. **Typography consistency**: Monospace for data, uppercase for labels, specific HUD fonts.

### What to adopt for heca-ui

1. **CSS variable theme system** — We already have `Theme` structs. Add CSS variable export for web.
2. **Variant system** — Our `from_theme()` pattern is similar but variant handling is manual.
3. **Compound components** — TabBar with Tab sub-components, Card with Header/Content/Footer.
4. **Corner decorations** — Visual signature element (corner brackets, scanlines).
5. **Progressive effects** — Allow users to opt into animations/glows at different intensities.
6. **Template system** — Full-page demo layouts showing real component composition.

### What not to copy

1. **Radix UI dependency** — heca-ui renders raw geometry, no DOM. We need our own accessibility/interaction layer.
2. **Tailwind CSS** — GPU-rendered UI uses different layout primitives.
3. **Next.js lock-in** — heca-ui is a window manager, not a web app.
4. **Three.js for chrome** — 3D effects in a compositor are too heavy. Keep for content panes.

---

# heca-grid-ui: Design Specification

> New crate in the heca-ui worktree (`heca-grid-ui/`). GPU-rendered TRON/neon HUD
> component library for the heca compositor dashboard.
>
> Reference visual: The GridCN dashboard template + install-command component.
> See `https://thegridcn.com/templates/dashboard` and
> `https://thegridcn.com/components`.

## Visual Identity

- **TRON/neon HUD** — dark backgrounds, glowing cyan/red/orange borders, corner
  brackets, scanline overlays, monospace/HUD typography
- **Design language**: glass-HUD meets grid-native UI — everything has a slight
  glow, corner decorations, and a "system status" feel
- **Typography**: uppercase labels, monospace values, pipe (`|`) separators,
  tracking-widest for data labels

## Crate

```
heca-grid-ui/          ← workspace member (Cargo.toml at workspace root)
├── Cargo.toml
├── src/
│   ├── lib.rs         ← module re-exports
│   ├── base.rs        ← ComponentBase, Component trait, Rect, Style, ComponentState
│   ├── theme.rs       ← GridTheme with TRON/ARES/CLU palettes
│   ├── pane.rs        ← Pane + Tab (titled container with tab bar)
│   ├── sidebar.rs     ← Sidebar (nav panel, left/right)
│   ├── kpi.rs         ← KPI metric card
│   ├── table.rs       ← Data table
│   ├── header.rs      ← Uplink header (system status bar)
│   ├── activity.rs    ← Activity feed
│   └── statusbar.rs   ← Bottom status bar
├── demo/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs    ← Dashboard demo (wgpu + winit)
```

Dependencies: `heca_ui` for `DrawList`, `Color`, `TextRasterizer`.

## Base Component Architecture

### Core Types

```rust
/// Position and size.
struct Rect { x: f32, y: f32, w: f32, h: f32 }

/// Interaction state.
enum ComponentState { Normal, Hovered, Pressed, Disabled, Focused, Selected }

/// Common visual properties — shared by every component.
struct Style {
    bg: Color, fg: Color,
    border: Color, border_width: f32, border_radius: f32,
    font_size: f32,
    glow: Color, glow_radius: f32,
}

/// Base data embedded in every component.
struct ComponentBase {
    rect: Rect, style: Style, state: ComponentState, visible: bool,
}
```

### Trait System (no repeated fields)

Every concrete component embeds `ComponentBase` as its first field and uses the
`impl_component!` macro to generate the delegation — never repeating accessors:

```rust
// Trait that every component implements
trait AsComponent {
    fn component_base(&self) -> &ComponentBase;
    fn component_base_mut(&mut self) -> &mut ComponentBase;
}

// Generates AsComponent impl — one macro call per type
macro impl_component!($ty);

// Core render/layout trait (object-safe)
trait Component: AsComponent {
    fn render(&self, dl: &mut DrawList, rast: &mut TextRasterizer);
    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32);  // default sets rect
    fn hit_test(&self, mx: f32, my: f32) -> bool;            // default uses rect
    fn z_index(&self) -> i32 { 0 }
}

// Concrete component — only adds domain fields
struct Pane {
    base: ComponentBase,     // ← common properties via base
    title: String,           // ← pane-specific
    tabs: Vec<Tab>,
    active_tab: usize,
    ...
}
impl_component!(Pane);       // ← generates AsComponent
impl Component for Pane {    // ← implements render/layout
    fn render(&self, dl, rast) { ... }
}
```

**No repeating common fields.** Every component gets `rect`, `style`, `state`,
`visible` through `ComponentBase`, accessed via `.base.rect` or through trait
methods from `Component` (`.set_position()`, `.hit_test()`, etc.).

### Layout vs. Rendering Separation

- `layout(x, y, w, h)` sets the rect. Some components may also lay out children.
- `render(dl, rast)` emits geometry using the current rect + style.
- Consumer calls `layout()` once per frame, then `render()` to generate draw
  commands.

### TRON Decorations (shared utilities in base.rs)

```rust
/// Four L-shaped corner brackets
fn draw_corner_brackets(dl, x, y, w, h, size, color, thickness);

/// Horizontal scanline overlay
fn draw_scanlines(dl, x, y, w, h, color, spacing);

/// Glowing border with outer glow pass + solid core
fn draw_neon_border(dl, x, y, w, h, color, glow_color, thickness);
```

## Theme

### GridTheme (domain-specific, not heca_ui Theme)

```rust
struct GridTheme {
    name: &'static str,
    // Core surfaces: bg, bg_elevated, bg_float, bg_sunken, bg_overlay
    // Text: fg, fg_muted, fg_dim, fg_inverse
    // Primary: primary, primary_muted, accent, accent_muted
    // Semantic: success, warning, danger, info (+ muted variants)
    // Borders: border, border_focus, border_subtle
    // Interactive: hover_bg, hover_fg, active_bg, active_fg
    // Glow: glow, glow_muted
    // Misc: radius, font_size
}
```

6 theme constructors: `tron()`, `ares()`, `clu()`, `athena()`, `aphrodite()`,
`poseidon()`.

### TRON Palette (default)

| Token | RGBA | Use |
|-------|------|-----|
| `bg` | `#0d0e1a` | Main background |
| `bg_elevated` | `#161b2d` | Pane/card surfaces |
| `bg_float` | `#1e243d` | Overlays, popups |
| `primary` | `#00d4ff` | Accent, glows, borders |
| `border` | `#00d4ff` @ 24% | Borders |
| `fg` | `#e6e9f4` | Primary text |
| `fg_muted` | `#828caa` | Secondary text |
| `glow` | `#00d4ff` @ 47% | Neon glow around borders |

## Component Catalog

### Core Components

| Component | File | Description |
|-----------|------|-------------|
| `Pane` | `pane.rs` | Titled container with optional tab bar. Renders bg, border glow, title, tabs, corner brackets, scanlines. Returns `content_bounds()` for content rendering. |
| `Tab` | `pane.rs` | Tab data struct (label, id, alert flag). Sized by pane width. Active tab has glowing underline indicator. |
| `Sidebar` | `sidebar.rs` | Vertical navigation panel. Renders logo/title area at top, nav items with icons, active indicator, optional collapse. |
| `SidebarItem` | `sidebar.rs` | Nav item with label, optional icon, active/hover state. |
| `KpiCard` | `kpi.rs` | Metric card showing value, label, delta (▲/▼ percentage), background glow. Compact, designed for row layout. |
| `DataTable` | `table.rs` | Scrollable data table with header row, aligned columns, row hover, status cells. |
| `UplinkHeader` | `header.rs` | Top system status bar — left text (system ID), right text (status/version). Thin, monospace, uppercase. |
| `ActivityFeed` | `activity.rs` | Vertical list of activity entries with timestamp, description, color-coded status dot. |
| `StatusBar` | `statusbar.rs` | Bottom bar with left/right section labels. Thin, bordered top. |

### Reference: The GridCN Dashboard Template

```
┌──────────────────────────────────────────────────────────────────┐
│ [UPLINK HEADER]    GRID CONTROL v4.2  SECTOR 7-G // ONLINE      │
├─────────┬────────────────────────────────────────┬───────────────┤
│         │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐  │               │
│ SIDEBAR │  │ KPI  │ │ KPI  │ │ KPI  │ │ KPI  │  │ RIGHT         │
│ (nav)   │  │ card │ │ card │ │ card │ │ card │  │ SIDEBAR       │
│         │  └──────┘ └──────┘ └──────┘ └──────┘  │ (activity     │
│ Home    │  ┌─────────────────────────────┐       │  feed +       │
│ Dashboard│  │     CHART PANE              │       │  system load) │
│ Users   │  │  ┌─┬─┬─┬─┐                  │       │               │
│ Network │  │  │T│a│b│s│  (active tab)    │       │               │
│ Alerts  │  │  └─┴─┴─┴─┘                  │       │               │
│ Systems │  │  ┌──────────────────────┐   │       │               │
│ Settings│  │  │  Chart content       │   │       │               │
│         │  │  └──────────────────────┘   │       │               │
│         │  └─────────────────────────────┘       │               │
│         │  ┌─────────────────────────────┐       │               │
│         │  │     DATA TABLE PANE         │       │               │
│         │  │  ┌─┬─┬─┬─┬─┬─┐             │       │               │
│         │  │  │H│e│a│d│e│r│             │       │               │
│         │  │  ├─┼─┼─┼─┼─┼─┤             │       │               │
│         │  │  │r│o│w│s│.│.│             │       │               │
│         │  │  └─┴─┴─┴─┴─┴─┘             │       │               │
│         │  └─────────────────────────────┘       │               │
├─────────┴────────────────────────────────────────┴───────────────┤
│ [STATUS BAR]  Connected · 12 nodes online        v4.2.1          │
└──────────────────────────────────────────────────────────────────┘
```

**See**: `https://thegridcn.com/templates/dashboard` for the full rendered
reference.

### Reference: Pane with Tab Bar (install-command style)

```
┌──────────────────────────────────────────────────┐
│ ┌──────┬──────┬──────┬──────┐                    │
│ │ Tab1 │ Tab2 │ Tab3 │ Tab4 │  ← tab bar        │
│ └──────┴──────┴──────┴──────┘                    │
│ ┌────────────────────────────────────────────┐   │
│ │                                            │   │
│ │         Content for active tab             │   │
│ │         (grid tiles / charts / tables)     │   │
│ │                                            │   │
│ └────────────────────────────────────────────┘   │
│                                                    │
│ ┌────────────────────────────────────────────┐   │
│ │  $ npm install @heca/grid-ui              │   │
│ └────────────────────────────────────────────┘   │
│                                                    │
│  ┌───────────────────────────────────────────┐    │
│  │  Corner bracket decoration (4 corners)   │    │
│  └───────────────────────────────────────────┘    │
│  (subtle scanline overlay across the pane)        │
└──────────────────────────────────────────────────┘
```

**See**: The GridCN install-command component at `https://thegridcn.com/components`
for the exact tab-bar+content layout.

The pane has:
1. **Tab bar**: horizontal, sticky to top, each tab has centered label + optional
   alert dot. Active tab has a glowing underline.
2. **Content area**: fills remaining space. Renders whatever the active tab needs
   (KPI cards, charts, tables, grid tiles).
3. **Corner brackets**: 4 L-shaped brackets at each corner in the primary color.
4. **Scanlines**: subtle repeating horizontal lines at 4px spacing, extremely
   low opacity (white at ~2%).
5. **Border glow**: optional outer glow pass (wider, transparent) + solid border.

## Pane and Grid Tiles

Instead of generic dashboard content, each pane contains **grid tiles** —
compact widgets that display data. Each tile has:
- A small title bar
- Content (number, sparkline, icon, status)
- Border with corner brackets

Tile types:
- **Metric tile** — single number + label + delta
- **Status tile** — green/yellow/red status indicator
- **Chart tile** — placeholder for sparkline/chart area
- **Table tile** — small inline data table
- **Activity tile** — recent events list

## Layout

```
┌──────────────────────────────────────────────────────┐
│  UplinkHeader                                        │
├────────┬───────────────────────────────┬─────────────┤
│        │  Main Content Area            │  Right       │
│ Left   │  ┌──Pane──┐ ┌──Pane──┐       │  Sidebar     │
│ Sidebar│  │tabs+tile│ │tabs+tile│       │             │
│        │  └────────┘ └────────┘       │  Activity    │
│ (nav)  │  ┌──────Pane────────────────┐│  Feed        │
│        │  │    Data Table with tabs  ││             │
│        │  └──────────────────────────┘│  System Load │
├────────┴───────────────────────────────┴─────────────┤
│  StatusBar                                           │
└──────────────────────────────────────────────────────┘
```

- **UplinkHeader**: Full width, thin (~32px). Shows "GRID CONTROL v4.2 SECTOR 7-G // ONLINE"
  or similar system status info.
- **Left Sidebar**: ~200px wide. Navigation items with hover/active states. Logo
  at top.
- **Main Content**: Flex layout with wrap. Panes fill available space (responsive).
- **Right Sidebar**: ~260px wide. Activity feed on top, system load bars below.
- **StatusBar**: Full width, thin (~28px). Connection status left, version right.

## Implementation Order

| Phase | What | Depends On |
|-------|------|------------|
| 1 | `base.rs` — ComponentBase, Component trait, Rect, Style, decorations | nothing |
| 2 | `theme.rs` — GridTheme with TRON palette | base |
| 3 | `pane.rs` — Pane + Tab (layout, render, hit testing) | base, theme |
| 4 | `sidebar.rs` — Sidebar navigation | base, theme |
| 5 | `kpi.rs` — KPI metric card | base, theme |
| 6 | `header.rs` — Uplink header | base, theme |
| 7 | `table.rs` — Data table | base, theme |
| 8 | `activity.rs` — Activity feed | base, theme |
| 9 | `statusbar.rs` — Status bar | base, theme |
| 10 | `lib.rs` — Module exports | all above |
| 11 | Demo app — Full dashboard with all components | crate done |
