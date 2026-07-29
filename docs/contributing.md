# Contributing

## Dev setup

Install the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS
(Rust toolchain; on Linux the WebKitGTK/AppIndicator dev packages), then:

```sh
npm ci
npm run tauri:dev      # run the app with hot-reload
```

## Project layout

- `src/` — React + TypeScript frontend (Chakra UI v3, Storybook).
- `src-tauri/` — Rust backend (see [architecture.md](architecture.md) for the module map).
- `docs/` — this documentation.
- `packaging/` — Homebrew cask and release assets.
- `.github/workflows/` — CI and Release pipelines.

## npm scripts

| Script | Does |
|---|---|
| `npm run tauri:dev` | Run the desktop app in dev. |
| `npm run tauri:build` | Build the packaged app. |
| `npm run typecheck` | `tsc --noEmit`. |
| `npm run check` | Biome check **and write** fixes. |
| `npm run test` | Vitest (React Testing Library). |
| `npm run test:coverage` | Vitest with a v8 coverage report. |
| `npm run storybook` | Storybook dev server. |
| `npm run build-storybook` | Build the static Storybook. |

## What CI enforces

[`ci.yml`](../.github/workflows/ci.yml) runs on every push/PR and must be green:

**Frontend** — `typecheck`, `biome check .`, `vitest`, `build-storybook`.
**Rust** — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check`.

Run the equivalents locally before pushing:

```sh
npm run typecheck && npx biome check . && npm run test && npm run build-storybook
cd src-tauri && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Note clippy runs with `-D warnings`, so warnings fail the build. Biome (not ESLint) is the
JS/TS linter/formatter.

### Coverage

Coverage is **reported, not gated** (no failing threshold yet — we're growing it).

- **Frontend:** `npm run test:coverage` (Vitest + v8) → summary in the terminal, HTML in
  `coverage/`.
- **Rust:** CI uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov); locally,
  `cargo install cargo-llvm-cov` then `cd src-tauri && cargo llvm-cov --summary-only`.

Unit tests live next to the code: Rust in `#[cfg(test)] mod tests` blocks at the bottom of
each module; frontend as `*.test.ts(x)` beside the file under test.

## Adding a new tool

Tools are dispatched by `ToolKind` in `src-tauri/src/tools.rs`. To add one:

1. **Backend** — add the operations (an `Op` enum with `name`, `is_write`, `summary`,
   `args`, `description`), and wire them into `execute` in `src-tauri/src/tools.rs`. For the
   capability itself, add a vertical slice (see [hexagonal.md](hexagonal.md)): a
   `domain/` type if it has business rules, an `application/ports/` contract + `usecases/`,
   an `infrastructure/driven/` adapter implementing the port, an `infrastructure/driving/`
   adapter that `execute` calls, and a `compose/` builder wiring them — mirroring the
   existing web / drive / knowledge slices.
2. **Frontend** — register the tool class in `src/lib/config.ts` (`TOOL_CLASSES`) and its
   operations in `TOOL_OPS` (mark writes with `write: true` so they default to *ask*). Add
   any config fields to `ToolInstance` and the settings UI.
3. **Approvals** — because policies are derived from `TOOL_OPS`, the per-bot approval
   editor picks up the new operations automatically.
4. Keep read ops `allow` by default and writes `ask`, and give each op a friendly
   `summary` (it drives the activity feed and the live Discord status message).

## Style

- Match the surrounding code — comment density, naming, and idioms.
- Rust: keep it `clippy`-clean under `-D warnings`; prefer inlined format args.
- TS/React: let Biome format; keep components small and typed.
