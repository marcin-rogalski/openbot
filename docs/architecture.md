# Architecture

openbot is a [Tauri](https://tauri.app) app: a **Rust backend** that does the real work
(Discord gateways, model calls, tools, indexing) and a **React + TypeScript** UI that
configures bots and watches them work. They communicate over Tauri commands and events.

```
┌───────────────────────────── openbot (desktop app) ──────────────────────────────┐
│                                                                                    │
│   React UI  ──commands/events──  Rust backend                                      │
│   (src/)                          (src-tauri/src/)                                  │
│                                        │                                            │
│                                        ├── Discord gateway (per bot)  ──►  Discord  │
│                                        ├── ReAct tool loop                          │
│                                        │     ├── model server  ──►  chat + embeds   │
│                                        │     ├── Google Drive  ──►  Drive API       │
│                                        │     ├── web search    ──►  Keenable        │
│                                        │     └── memory                             │
│                                        └── local knowledge index (SQLite)           │
└────────────────────────────────────────────────────────────────────────────────────┘
```

## Backend modules (`src-tauri/src/`)

| Module | Responsibility |
|---|---|
| `main.rs` | App entry: sets up Tauri, tray, window, registers commands, starts the control API. |
| `bot.rs` | `BotManager` (bots keyed by id) and the per-bot event model emitted to the UI. |
| `discord.rs` | Per-bot serenity client: receives messages, runs the reply/tool loop, maintains the live status message and rolling channel memory. |
| `model.rs` | OpenAI-compatible client: streaming `chat()`, `embed()`, plus small classifier calls (relevance, foldering, summarization). |
| `tools.rs` | Tool dispatch (`ToolKind`), the Drive/Web operations, approval gating, and the attachment sink. |
| `knowledge.rs` | The local SQLite knowledge store: sources, chunks, FTS5, and hybrid search. |
| `ingest.rs` | Text extraction (text + PDF) and chunking. |
| `gdrive/` | Google Drive REST client + OAuth (`auth.rs`): list, read, upload, folders, recursive search. |
| `websearch.rs` | Keenable search + page fetch. |
| `voice.rs` | Live voice-channel transcription: receive per-speaker PCM (songbird), segment on silence, transcribe each utterance, assemble a meeting transcript + summary. |
| `memory.rs` | Per-bot memory notes: save, consolidate, and produce prompt guidance. |
| `config.rs` | Config types and persistence, plus migration of the legacy single-bot config. |
| `api.rs` | The localhost control API (`GET /bots`, start/stop/toggle). |
| `tray.rs`, `window.rs`, `macos.rs` | Menu-bar tray, hide-to-tray window lifecycle, and macOS-only quit interception (all `#[cfg(target_os = "macos")]`-gated). |

## The reply / tool loop

When a bot is addressed, `discord.rs`:

1. Builds the message list — a **raw window** of recent messages plus a rolling
   **summary** of older ones (compacted periodically), any inline attachment text, and the
   memory guidance and system prompt.
2. Calls `model::chat()` (streaming). Tokens stream into the app as a collapsible
   "Thinking" block; a repetition guard aborts pathological infinite generations.
3. If the model emits `TOOL_CALL {json}`, the call is routed through `tools.rs` under its
   approval policy, executed, and the `TOOL_RESULT` is appended — looping up to
   `MAX_TOOL_ITERS` (10).
4. The final answer is sent to Discord (split across messages if over Discord's 2000-char
   limit), with any sources cited at the top.

Throughout, a single **live status message** in the channel is edited in place
(💭 Thinking… → each tool's summary → the final answer), so the user sees progress in one
tidy message.

## Local RAG design

The knowledge index deliberately avoids a native vector extension (which would complicate
signing/packaging). Instead:

- **Storage:** `rusqlite` with the `bundled` SQLite (FTS5 compiled in). One `.db` per Drive
  tool instance. Embeddings are stored as little-endian `f32` BLOBs.
- **Retrieval:** FTS5 BM25 keyword candidates **∪** brute-force cosine similarity over chunk
  embeddings, normalized and fused (reciprocal-rank fusion), top-k returned.
- **Scale:** ample for a personal knowledge base, with no extra dependencies to load or sign.
- **Derived cache:** the index is fully rebuildable from Drive via `reindex`.

All DB and cosine work runs inside `tokio::task::spawn_blocking` so it never blocks the
async runtime.

## Voice transcription

`voice.rs` uses **songbird** (serenity's voice library) with `DecodeMode::Decode`, so each
20 ms tick delivers decoded 48 kHz stereo PCM per speaker (SSRC). A `Meeting` accumulates
audio per speaker, closes an utterance after ~0.8 s of silence (or a 30 s cap), downmixes
to mono, wraps it in a WAV, and transcribes it off the receive path via
`model::transcribe`. Utterances are ordered and speaker-labelled (SSRC → user id from
speaking-state updates); on leave they're assembled into a transcript + summary. This pulls
a native Opus dependency (`audiopus`), which is why the build needs an Opus/CMake toolchain
and CI sets `CMAKE_POLICY_VERSION_MINIMUM`.

## Events

The backend emits per-bot, id-scoped events the UI subscribes to:

`bot://status`, `bot://activity`, `bot://stream`, `bot://thinking`, `bot://metrics`,
`bot://tool-approval`. This is how the activity feed, the streaming "Thinking" block, the
status bar, and the approval prompts stay live.

## Frontend (`src/`)

React + [Chakra UI v3](https://chakra-ui.com) with a Discord-inspired design system
(`src/theme.ts`) and Storybook (`src/stories/`) covering the component library. Key pieces:
the shell (`App.tsx`), the per-bot view and tabs (`components/BotView.tsx`), the activity
feed (`components/ActivityFeed.tsx`), and the settings/section components. Bot control and
event subscriptions live in `src/lib/bot.ts`; config types mirror the backend in
`src/lib/config.ts`.

## Planned architecture alignment

The code today is layered but tangled (orchestration modules call concrete infrastructure
directly). The agreed target — a **core + tool-plugin** architecture — is specified in the
[Architecture Decision Records](adr/README.md): the ports/adapters split and the `Tool` plugin
model ([ADR-0001](adr/0001-tool-plugin-architecture.md)), the `Host` tool SDK
([ADR-0002](adr/0002-host-tool-sdk.md)), the prompt/results/event-bus mechanisms
([ADR-0003](adr/0003-prompt-results-events.md)), async jobs + proactive turns
([ADR-0004](adr/0004-async-jobs-and-proactive-turns.md)), and config layering
([ADR-0005](adr/0005-config-layering.md)). Being migrated incrementally, tests first:

**Rust → hexagonal (ports & adapters):**
`domain/` (pure types + rules, no IO) · `ports/` (traits: `ChatModel`, `Embedder`,
`Transcriber`, `KnowledgeIndex`, `FileStore`, `WebSearch`, `ChatPlatform`, `ConfigStore`,
`EventSink`) · `application/` (use-cases over ports: reply loop, ingest, reindex, meeting
lifecycle) · `adapters/` (openai, gdrive, keenable, sqlite, discord, voice, store,
tauri-events, os) · `interface/` (tauri commands + the localhost HTTP API). This makes the
domain + application layers unit-testable against fake ports.

**Frontend → layered / FSD-lite:**
`shared/` (design-system primitives + utils) · `entities/` (domain types + pure helpers) ·
`features/` (cohesive slices with their own components + hooks) · `ipc/` (the Tauri
boundary — the frontend's port to the Rust core) · `app/` (shell, layout, providers).

**Testing:** coverage is reported (not gated). Rust uses `cargo-llvm-cov`; the frontend uses
Vitest's v8 provider. Unit tests target pure logic today; application-layer tests with fake
adapters follow the ports migration.
