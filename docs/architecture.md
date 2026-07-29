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

## Backend layout (`src-tauri/src/`)

The backend follows **hexagonal / ports-and-adapters** (see [hexagonal.md](hexagonal.md)).
Dependencies point inward: `domain` → nothing, `application` → `domain`, adapters +
entrypoints → `application`'s ports. Every capability is a vertical slice through these
layers.

| Layer | What lives there |
|---|---|
| `domain/` | Business types + rules, pure (std only): `conversation`, `memory`, `drive`, `knowledge`, `page`, `search`, `transcript`. |
| `application/ports/` | The trait contracts the app needs: `chat_model`, `websearch`, `webfetch`, `drive`, `knowledge`, `memory`, `ingestion`, `transcription`. |
| `application/services/` | Reusable pure logic injected into usecases: `chunking`, `foldering`. |
| `application/usecases/` | One business operation each: `search_web`, `fetch_page`, `save_memory`, `ask_knowledge`, `reindex_knowledge`, `index_document`, `archive_attachment`, `transcribe_clip`. |
| `infrastructure/driven/` | Outbound adapters implementing ports: `model_chat`/`model_transcriber`/`model_summarizer`/`model_archive_policy`/`embeddings` (model server), `keenable` (web), `gdrive_storage`, `knowledge_index` (SQLite), `memory_store`/`memory_consolidator`, `symphonia_codec`, `ingest_extractor`, `http_fetcher`. |
| `infrastructure/driving/` | Inbound adapters translating requests into usecase calls: `web`, `drive`, `knowledge`, `memory`, `ingestion`, `transcription`. |
| `infrastructure/dto/` | Boundary (de)serialization structs (`keenable`, `memory`), mapped to/from domain. |
| `infrastructure/shared/` | Cross-cutting infra wired first: `http` (shared client), `time`. |
| `compose/` | The composition root — `compose_*` builders that name every concrete adapter and inject it into usecases. |

The remaining **crate-root modules** are entrypoints, cross-cutting app services, and
adapter-level helpers that the driven adapters wrap:

| Module | Responsibility |
|---|---|
| `main.rs` | App entry: Tauri setup, tray, window, command registration, control API; calls `compose::shared` first. |
| `bot.rs` | `BotManager` (bots keyed by id) + the per-bot event model emitted to the UI. |
| `discord.rs` | Per-bot serenity driving adapter: receives messages, runs the reply/tool loop over the `ChatModel` port, maintains the live status message and rolling channel memory. |
| `voice.rs` | Voice-channel driving adapter: per-speaker PCM → utterances → the transcription engine → a meeting transcript. |
| `tools.rs` | Tool dispatch (`ToolKind`, `execute`), approval gating, the attachment sink, and `TOOL_CALL` parsing. Tools are the model↔app boundary, so they sit outside the hexagon and call into driving adapters. |
| `model.rs` | OpenAI-compatible client (streaming `chat`, `embed`, transcription, classifier calls) — wrapped by the driven model adapters. |
| `gdrive/` | Google Drive REST client + OAuth — wrapped by `gdrive_storage`. |
| `knowledge.rs` | SQLite store (sources, chunks, FTS5, hybrid search) — wrapped by `knowledge_index`. |
| `ingest.rs` | Text extraction (text + PDF) — wrapped by `ingest_extractor` (chunking now lives in `application/services/chunking`). |
| `audio.rs` | Symphonia/audiopus decode + WAV chunking — wrapped by `symphonia_codec`. |
| `config.rs` | Config types and persistence, plus migration of the legacy single-bot config. |
| `api.rs` | The localhost control API (`GET /bots`, start/stop/toggle). |
| `tray.rs`, `window.rs`, `macos.rs` | Menu-bar tray, hide-to-tray window lifecycle, macOS quit interception. |

## The reply / tool loop

When a bot is addressed, `discord.rs`:

1. Builds the message list — a **raw window** of recent messages plus a rolling
   **summary** of older ones (compacted periodically via the `ChatModel` port), any inline
   attachment text, and the memory guidance and system prompt.
2. Calls the **`ChatModel` port** (streaming). Tokens stream into the app as a collapsible
   "Thinking" block; a repetition guard aborts pathological infinite generations.
3. If the model emits `TOOL_CALL {json}` (parsed by `tools::parse_tool_call`), the call is
   routed through `tools.rs` under its approval policy, executed, and the `TOOL_RESULT` is
   appended — looping up to `MAX_TOOL_ITERS` (10).
4. The final answer is sent to Discord (split across messages if over Discord's 2000-char
   limit), with any sources cited at the top.

Throughout, a single **live status message** in the channel is edited in place
(💭 Thinking… → each tool's label/progress → the final answer), and the same label + a
right-side progress/speed detail drive the app status bar.

## Local RAG design

The knowledge index deliberately avoids a native vector extension (which would complicate
signing/packaging). Instead:

- **Storage:** `rusqlite` with the `bundled` SQLite (FTS5 compiled in). One `.db` per Drive
  tool instance. Embeddings are stored as little-endian `f32` BLOBs.
- **Retrieval:** FTS5 BM25 keyword candidates **∪** brute-force cosine similarity over chunk
  embeddings, normalized and fused (reciprocal-rank fusion), top-k returned.
- **Scale:** ample for a personal knowledge base, with no extra dependencies to load or sign.
- **Derived cache:** the index is fully rebuildable from Drive via `reindex`.

The `knowledge` module (SQLite + cosine) is wrapped by the `KnowledgeIndex` driven adapter;
the `ask_knowledge` / `reindex_knowledge` / `index_document` usecases orchestrate it with the
`Embeddings` port and the `chunking` service. All DB and cosine work runs inside
`tokio::task::spawn_blocking` so it never blocks the async runtime.

## Voice transcription

`voice.rs` uses **songbird** (serenity's voice library) with `DecodeMode::Decode`, so each
20 ms tick delivers decoded 48 kHz stereo PCM per speaker (SSRC). A `Meeting` accumulates
audio per speaker, closes an utterance after ~0.8 s of silence (or a 30 s cap), downmixes to
mono, wraps it in a WAV, and transcribes it off the receive path through the **transcription
engine** (`Transcriber` port). Utterances are ordered and speaker-labelled; on leave they're
assembled into a transcript + summary (via the `Summarizer` port). This pulls a native Opus
dependency (`audiopus`), which is why the build needs an Opus/CMake toolchain and CI sets
`CMAKE_POLICY_VERSION_MINIMUM`.

## Events

The backend emits per-bot, id-scoped events the UI subscribes to:

`bot://status`, `bot://activity`, `bot://stream`, `bot://busy` (activity label + optional
progress detail), `bot://metrics`, `bot://tool-approval`. This is how the activity feed, the
streaming "Thinking" block, the status bar, and the approval prompts stay live.

## Frontend (`src/`)

React + [Chakra UI v3](https://chakra-ui.com) with a Discord-inspired design system
(`src/theme.ts`) and Storybook (`src/stories/`) covering the component library. Key pieces:
the shell (`App.tsx`), the per-bot view and tabs (`components/BotView.tsx`), the activity
feed (`components/ActivityFeed.tsx`), and the settings/section components. Bot control and
event subscriptions live in `src/lib/bot.ts`; config types mirror the backend in
`src/lib/config.ts`.

## Architecture decisions

The **core + tool-plugin** direction and its ports/adapters split are recorded in the
[Architecture Decision Records](adr/README.md): the plugin model
([ADR-0001](adr/0001-tool-plugin-architecture.md)), the `Host`/`Ctx` tool SDK
([ADR-0002](adr/0002-host-tool-sdk.md)), prompt/results/event mechanisms
([ADR-0003](adr/0003-prompt-results-events.md)), async jobs + proactive turns
([ADR-0004](adr/0004-async-jobs-and-proactive-turns.md)), config layering
([ADR-0005](adr/0005-config-layering.md)), and the module layout
([ADR-0006](adr/0006-module-layout.md)). The Rust backend has been migrated to the
hexagonal layout above (ports in `application/`, adapters in `infrastructure/{driven,driving}`,
composition in `compose/`); the [hexagonal.md](hexagonal.md) walk-through is the contributor
guide. The frontend remains a component/hook layout under `src/`.

**Testing:** coverage is reported (not gated). Rust uses `cargo-llvm-cov`; the frontend uses
Vitest's v8 provider. Domain rules and usecases are unit-tested against fake ports.
