# openbot

Minimal Tauri v2 + React + TypeScript boilerplate: menu bar tray icon, a
hide-to-tray window, and full tooling (lint, typecheck, test, CI, release).
Deliberately almost no app code — that's for you to add. This just makes sure
the plumbing and tooling are solid.

## What's already wired up

- **Tray icon** (`src-tauri/src/tray.rs`): left-click toggles the window,
  menu has Show / Hide / Quit. Icon renders as a template image so it adapts
  to light/dark menu bars on macOS.
- **Window lifecycle** (`src-tauri/src/window.rs`): closing the window hides
  it instead of quitting the app (standard tray-app behavior). Two commands
  are exposed to the frontend: `show_main_window`, `hide_main_window`.
- **Icons**: a full desktop icon set (`32x32.png` … `icon.icns`/`icon.ico`)
  is already generated in `src-tauri/icons/` from a placeholder blue square.
  Swap it out with your own before shipping — see "Replacing the icon" below.
- **Tooling**: ESLint (flat config) + Prettier + strict TypeScript + Vitest +
  React Testing Library, all verified to run clean out of the box.
- **CI** (`.github/workflows/ci.yml`): typecheck/lint/test/build on every
  push, plus a `cargo check` on macOS runners.
- **Release** (`.github/workflows/release.yml`): push a `v*` tag to build a
  universal macOS binary and attach it to a draft GitHub Release via
  `tauri-action`. Code-signing/notarization env vars are stubbed in but
  commented out — uncomment once you have an Apple Developer account.

## Prerequisites (macOS)

You need the Rust toolchain and Tauri's system deps, which npm alone can't
install:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
xcode-select --install   # if not already installed
```

## Scripts

| Command               | What it does                                                         |
| --------------------- | -------------------------------------------------------------------- |
| `npm start`           | Alias for `tauri:dev` — runs the real desktop app                    |
| `npm run dev`         | Vite dev server only (browser, no native shell)                      |
| `npm run tauri:dev`   | Full Tauri dev mode: native window + tray + hot reload               |
| `npm run build`       | Typecheck + production frontend bundle (`dist/`)                     |
| `npm run tauri:build` | Full native app bundle (`.app` / `.dmg`) for your Mac                |
| `npm run publish`     | Same as `tauri:build` — kept as a separate name for CI/muscle memory |
| `npm run test`        | Run Vitest once                                                      |
| `npm run test:watch`  | Vitest in watch mode                                                 |
| `npm run lint`        | ESLint                                                               |
| `npm run typecheck`   | `tsc --noEmit`                                                       |
| `npm run format`      | Prettier, write mode                                                 |
| `npm run ci`          | typecheck + lint + test + build, in sequence (what CI runs)          |

## Project layout

```
src/                   React app (currently just a placeholder)
src-tauri/
  src/
    main.rs            Wires everything together, ~20 lines
    tray.rs            Tray icon + menu
    window.rs          Show/hide commands + the shared window label constant
  tauri.conf.json       Window size, bundle targets, icon paths
  icons/                Generated icon set
.github/workflows/      ci.yml, release.yml
```

## Replacing the icon

```bash
npx tauri icon path/to/your-1024x1024-source.png
```

This regenerates every size (`32x32.png` through `icon.icns`/`icon.ico`) in
`src-tauri/icons/` in one shot — no macOS-only tooling required, it runs
anywhere.

## Where to plug in your Discord/MCP logic

- Frontend UI → `src/App.tsx` and onward.
- Anything that needs Rust-side system access (spawning your sidecar
  process, reading local files, etc.) → new `#[tauri::command]` functions,
  same pattern as `window.rs`. Register them in the `invoke_handler!` list in
  `main.rs`.
- If the Discord bot / MCP client runs as a separate Node process rather
  than in Rust, `tauri-plugin-shell` (already a dependency) can spawn and
  manage it as a sidecar binary.
