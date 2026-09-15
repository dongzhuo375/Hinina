---
name: Competitive Arena UI
colors:
  surface: '#0f141b'
  surface-dim: '#0f141b'
  surface-bright: '#343941'
  surface-container-lowest: '#090f15'
  surface-container-low: '#171c23'
  surface-container: '#1b2027'
  surface-container-high: '#252a32'
  surface-container-highest: '#30353d'
  on-surface: '#dee2ec'
  on-surface-variant: '#c9c4d8'
  inverse-surface: '#dee2ec'
  inverse-on-surface: '#2c3138'
  outline: '#938ea1'
  outline-variant: '#484555'
  surface-tint: '#cabeff'
  primary: '#cabeff'
  on-primary: '#31009a'
  primary-container: '#947dff'
  on-primary-container: '#2a0088'
  inverse-primary: '#603ce2'
  secondary: '#b4c5ff'
  on-secondary: '#002a78'
  secondary-container: '#0053db'
  on-secondary-container: '#cdd7ff'
  tertiary: '#39dcd2'
  on-tertiary: '#003734'
  tertiary-container: '#00a29a'
  on-tertiary-container: '#00302d'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e6deff'
  primary-fixed-dim: '#cabeff'
  on-primary-fixed: '#1c0062'
  on-primary-fixed-variant: '#4816cb'
  secondary-fixed: '#dbe1ff'
  secondary-fixed-dim: '#b4c5ff'
  on-secondary-fixed: '#00174b'
  on-secondary-fixed-variant: '#003ea8'
  tertiary-fixed: '#61f9ef'
  tertiary-fixed-dim: '#39dcd2'
  on-tertiary-fixed: '#00201e'
  on-tertiary-fixed-variant: '#00504c'
  background: '#0f141b'
  on-background: '#dee2ec'
  surface-variant: '#30353d'
  status-ac: '#22C55E'
  status-wa: '#EF4444'
  status-tle: '#F59E0B'
  status-pending: '#00C7BE'
  bg-canvas-dark: '#0D1117'
  bg-surface-dark: '#161B22'
  bg-surface-elevated: '#21262D'
  border-subtle-dark: '#30363D'
  text-primary-dark: '#F0F6FC'
  text-secondary-dark: '#8B949E'
  text-muted-dark: '#6E7681'
  bg-canvas-light: '#F8FAFC'
  bg-surface-light: '#FFFFFF'
  border-subtle-light: '#E2E8F0'
  text-primary-light: '#0F172A'
  text-secondary-light: '#64748B'
typography:
  display-hero:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: '700'
    lineHeight: 40px
  headline-lg:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '700'
    lineHeight: 32px
  headline-md:
    fontFamily: Inter
    fontSize: 18px
    fontWeight: '600'
    lineHeight: 26px
  headline-sm:
    fontFamily: Inter
    fontSize: 15px
    fontWeight: '600'
    lineHeight: 22px
  body-lg:
    fontFamily: Inter
    fontSize: 15px
    fontWeight: '400'
    lineHeight: 24px
  body-md:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: '400'
    lineHeight: 20px
  body-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 18px
  label-code-lg:
    fontFamily: JetBrains Mono
    fontSize: 14px
    fontWeight: '500'
    lineHeight: 22px
  label-code-md:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '500'
    lineHeight: 18px
  label-code-sm:
    fontFamily: JetBrains Mono
    fontSize: 11px
    fontWeight: '600'
    lineHeight: 16px
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  gutter: 0.75rem
  margin: 1rem
  space-xs: 0.25rem
  space-sm: 0.5rem
  space-md: 0.75rem
  space-lg: 1rem
  space-xl: 1.5rem
---

## Brand & Style

### Brand Personality & Core Purpose
This design system is crafted for high-stakes, distraction-free competitive programming (ACM/ICPC, XCPC, and collegiate algorithmic contests). The emotional tone balances intense focus, professional telemetry, and decisive clarity. It eliminates ambient visual noise to prioritize algorithmic reasoning, real-time feedback loops, and rapid code submission under strict time constraints.

### Aesthetic Movement
The visual architecture blends **Modern IDE Functionalism** with **Dark Telemetry & Glass-Refined Engineering**:
- **Deep-space canvas**: In competitive dark mode, the interface uses low-luminance slate neutrals (`#0D1117` and `#161B22`) to minimize eye fatigue during multi-hour contests while elevating high-contrast syntax highlighting.
- **Vibrant Accent Energy**: Electric Violet (`#7C5CFF`) serves as the primary system accent for active states and critical calls-to-action, complemented by crisp tech blues (`#2563EB`) in prep/setup phases.
- **Crisp Structural Borders**: Thin, low-opacity hairline dividers (`rgba(255, 255, 255, 0.08)` in dark mode; `rgba(0, 0, 0, 0.08)` in light mode) preserve precise window and panel boundaries without heavy dropped shadows.
- **Dual Mode Intent**: 
  - **Arena Dark Mode (Default / Active Match)**: Full workspace concentration, multi-pane docking, Monaco code editor, real-time judge telemetry.
  - **Preparation Light Mode (Check-in / Setup)**: Clear legibility, clean card elevations, and frictionless authentication flows.

## Colors

### Primary and Brand Semantics
- **Electric Violet (`#7C5CFF`)**: The signature interactive accent. Used for primary CTA buttons (e.g., Code Submit), active problem navigation items, focused input rings, and active tabs.
- **Tech Blue (`#2563EB` / `#3B82F6`)**: Secondary brand color, prominent in pre-contest screens, authentication, step-wizard completed nodes, and platform connectivity badges.
- **Cyan Spark (`#00C7BE`)**: Tertiary telemetry color, utilized for pending judge states, memory profiling meters, and interactive badges.

### OJ Judgement Semantic Tokens
Strict, uncompromised color coding communicates submission verdicts immediately:
- **Accepted (AC)**: `#22C55E` — Vivid green for correct submissions and passing test cases.
- **Wrong Answer (WA)**: `#EF4444` — Crisp red for incorrect logic, runtime crashes, and system faults.
- **Time Limit Exceeded / Warning (TLE/PE/OLE)**: `#F59E0B` — Vibrant amber-orange for timeouts, memory warnings, and non-fatal anomalies.
- **Compiling / Judging (PD/CI)**: `#00C7BE` — Pulsing cyan indicating evaluation in progress.

### Surface and Depth Architecture
- **Dark Mode Surfaces**: Base backdrop is `#0D1117`, panel backgrounds use `#161B22`, and elevated cards or active floating panels use `#21262D`. Hairline borders use `#30363D` with no heavy box shadows.
- **Light Mode Surfaces**: Canvas rests on `#F8FAFC`, cards on pure `#FFFFFF`, and borders on `#E2E8F0`.

## Typography

### Font Hierarchy & Roles
- **UI & Structural Type (`Inter`, fallbacks: `-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei"`)**: Handles application chrome, dialogs, problem statements, countdown labels, and tables. Clean geometric glyphs ensure swift reading at compact desktop sizes.
- **Monospace Code & Telemetry (`JetBrains Mono`, fallbacks: `"Fira Code", monospace`)**: Dedicated to code panes, Monaco editor buffers, memory and runtime badges, test case I/O boxes, and the status table (AC, WA, TLE, CE). Ligatures and distinct character glyphs (like `0` vs `O`, `l` vs `1`) ensure absolute programming precision.

### Typographic Treatment
- Problem descriptions and editorial text maintain a readable line height (`1.6` to `1.7`) with comfortable paragraph margins.
- Telemetry figures, line numbers, memory counters, and timestamps are fixed to tabular figures (`font-variant-numeric: tabular-nums`) to prevent layout shifts during live clock ticking and status polling.

## Layout & Spacing

### Desktop Client Layout Architecture
The desktop client conforms to a strict dense-docking layout tailored for 1080p, 2K, and 4K displays:
- **Titlebar Header (Windows Native Frame)**: Fixed height `40px` (or `48px`), accommodating drag regions, brand crest, breadcrumb route, global contest countdown widget, and custom window control buttons (`min`, `max/restore`, `close`).
- **Activity Bar (Leftmost Nav)**: Fixed width `56px` hosting vertical icon tabs (`Contest`, `Problems`, `Submissions`, `Standings`, `Workspace`, `Settings`).
- **Main Arena (Adaptive Split Grid)**:
  - **Navigation Sidebar (Problem List / Tree)**: Fixed width `240px`–`280px` collapsible panel.
  - **Work Area**: Resizable 2-column or 3-column split-pane layout:
    - *Left Work Pane*: Markdown problem statement, sample test case copy blocks, contest announcement drawer.
    - *Right Work Pane*: Monaco Editor header (Language selector, code template switcher, submit action), code canvas, and collapsible bottom panel (Judge results, test case runner, console output).

### Compact Spacing Scale
Because competitive programming demands high information density, spacing relies on a strict 4px/8px modular base:
- `space-xs` (4px): Micro gaps between status badges, tags, and icon-to-label pairs.
- `space-sm` (8px): Button internal padding, input padding, list item vertical rhythm.
- `space-md` (12px): Panel header padding, card gaps, test case container padding.
- `space-lg` (16px): Outer pane separation, modal padding.
- `space-xl` (24px): Login card outer margins and setup wizard steps.

## Elevation & Depth

### Tonal Tiers & Structural Hairlines
In competitive dark mode, depth is established through tonal stepping and crisp borders rather than diffuse drop shadows:
- **Tier 0 (Desktop Canvas)**: `#0D1117` — Deepest shell background behind draggable panes and window gutters.
- **Tier 1 (Surface Panels)**: `#161B22` with a 1px border of `#30363D` — Problem viewer, code editor background, file navigator.
- **Tier 2 (Floating & Active Toolbars)**: `#21262D` — Active tab items, dropdown menus, context menus, and toolbars.
- **Tier 3 (Modals & Overlays)**: `#1F242C` with `box-shadow: 0 16px 36px rgba(0, 0, 0, 0.65)` and `border: 1px solid rgba(255, 255, 255, 0.12)`.

### Light Mode Elevation
In the preparation and login states:
- Cards rest on pure `#FFFFFF` with a soft ambient elevation: `0 4px 20px -2px rgba(15, 23, 42, 0.06), 0 1px 3px rgba(15, 23, 42, 0.04)`.
- Input fields and step cards utilize a crisp 1px neutral border (`#E2E8F0`) transitioning to `#7C5CFF` on focus with a matching 3px focus ring (`rgba(124, 92, 255, 0.2)`).

## Shapes

### Roundedness Philosophy
A balanced `8px` (`0.5rem`) base corner radius provides a modern software feel while maintaining the structural sharpness demanded by tabular and multi-pane developer interfaces:
- **Panels & Docked Windows**: Sharp outer seam (`0px`) where docked against window frames; inner contained cards use `rounded-md` (6px) or `rounded-lg` (8px).
- **Interactive Controls (Inputs, Buttons, Dropdowns)**: Uniform `6px` to `8px` corner radius.
- **Verdict Badges & Status Chips**: Pill-shaped (`9999px`) or soft badge (`4px`) with monospaced text to emphasize categorical outcomes.
- **Problem Letter Identifiers (A, B, C...)**: Squircle cards (`6px`–`8px`) with centered bold typography for rapid visual indexing.

## Components

### 1. Window Frame & Titlebar
- **Native Header Drag Region**: Includes `-webkit-app-region: drag` with excluded interactive elements (controls, search, profile).
- **Contest Indicator**: Houses contest status (`In Progress` in `#22C55E`, `Pending` in `#F59E0B`), live clock, and time remaining counter with fixed-width mono numerals.
- **Window Controls**: Minimize, Maximize/Restore, and Close buttons positioned flush at the top-right corner, matching Windows desktop affordances with subtle hover states (Close button turns red `#E81123` on hover).

### 2. Authentication & Pre-Contest Screen
- **Focused Inputs**: Directly asks for username/email and credentials. OJ platform select dropdown has been removed in favor of direct credential authentication configured through unified target settings.
- **Step Process Indicator**: Visual 4-step wizard (`Server Connect` → `Auth Verification` → `Fetch Contest Data` → `Ready`) with completed states in tech-blue / AC green, connecting lines, and status icons.
- **Pre-Contest Countdown Card**: Prominent multi-segment timer card (`Days : Hours : Mins : Secs`) with large bold numbers (`36px`) and subtle gray labels.

### 3. Problem Directory & Nav Rail
- **Problem Card**: Compact list item showing problem index badge (e.g. `A`, `B`, `C`), localized title, point score, difficulty chip, submission status (green check for AC, amber retry for attempted, plain for unread), and star bookmark button.
- **Active Problem Selection**: High-contrast left indicator line (`#7C5CFF`, 3px wide) with an elevated surface `#21262D`.

### 4. Problem Description & Test Case Runner
- **Markdown Viewer**: High readability styles for math formulas (KaTeX), image scaling, copyable code samples with single-click clipboard confirmation feedback.
- **Sample I/O Block**: Dual-pane or stacked light-contrast code containers with quick "Copy Input" and "Run against sample" action buttons.

### 5. Monaco Code Editor & Submissions
- **Editor Header**: Language selector dropdown (C++17, Python 3, Java 21, Rust), file tab strip, auto-save status indicator, and prominent "Submit Code" CTA.
- **Submit Button**: High-visibility `#7C5CFF` fill, hover brightness boost, loading spinner on IPC transmit, and keyboard shortcut hint (`Ctrl+Enter`).
- **Judgement Verdict Table**: Real-time polling list displaying Verdict pill (`AC`, `WA`, `TLE`, `CE`), submission timestamp, problem label, runtime (`ms`), memory footprint (`MB`), and drill-down link for test case breakdown.