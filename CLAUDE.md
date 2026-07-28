# openbot — Claude Code guide

Tauri v2 (Rust, `src-tauri/`) + React 18 / TypeScript / Chakra UI v3 (`src/`) — a desktop app
running local-LLM-powered Discord bots.

## Verify (before every commit)

Run **`./scripts/verify.sh`** — Rust `fmt --check` + `clippy --all-targets -D warnings` +
`cargo test`, then frontend `tsc --noEmit` + `biome check .` + `vitest`. Everything must be green.
(`CMAKE_POLICY_VERSION_MINIMUM=3.5` is set in `.cargo/config.toml` — needed for songbird's Opus
build; no need to prefix it manually.)

## Run in the app

Verify runtime changes with the **packaged** build, not `tauri dev`: `npm run tauri:build`, then
open `src-tauri/target/release/bundle/macos/openbot.app`. Bots can be driven via the localhost
control API on `127.0.0.1:8787` — `GET /bots`, `POST /bots/<id>/{start,stop,toggle}`.

## Git

Commit/push only when asked. Branch off `main`, `merge --ff-only`, push. End commit messages with
the `Co-Authored-By:` + `Claude-Session:` trailers.

## Conventions

- Generated test/sample files → `dist/` (gitignored, wiped by builds).
- Architecture is migrating to **hexagonal** — see `docs/hexagonal.md` and `docs/adr/`. Keep
  formats, algorithms, and IO out of `domain/`.
- After a plan is signed off, proceed autonomously end-to-end; ask only on genuinely ambiguous
  design forks.
