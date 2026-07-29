# Hexagonal architecture in openbot

How the backend (`src-tauri/src`) is organized. The Rust backend has been migrated to
ports-and-adapters; this is the contributor walk-through. The design intent is recorded in
the [ADRs](adr/README.md) (the implementation refined some of their details — noted below).

## The one rule

**Dependencies point inward.** `domain` → (nothing); `application` → `domain`; adapters and
`tools` → `application`'s ports; `compose/` (composition root) → everything. Nothing in
`domain`/`application` may import serenity, reqwest, rusqlite, tauri, songbird, or any vendor
client. "Pure" = std (+ a serde derive only where unavoidable, e.g. `ChatMessage`).

```
                 ┌─────────── infrastructure/driving ─────────┐
   Discord in ──▶│ discord.rs · voice.rs · tauri commands ·   │──┐
                 │ control API · web/drive/knowledge/memory/  │  │ call usecases / ports
                 │ ingestion/transcription driving adapters   │  ▼
   tools.rs ───────────────────────────────▶  application (ports · services · usecases)
   (out of hex; parses TOOL_CALL)                          │  ▲
                 ┌─────────── infrastructure/driven ──────────┐ │  │ implement ports
   model/Drive ◀─│ model_chat · embeddings · keenable ·       │◀┘  │
   DB / codec…   │ gdrive_storage · knowledge_index · memory_ │────┘
                 │ store/consolidator · symphonia_codec · …    │
                 └─────────── dto/ maps wire ⇆ domain ─────────┘
                       domain (business data + invariants)
```

## Layers

### `domain/` — business representation + its rules
Value objects and the operations *on that representation*: validation, sanitization,
invariants. No formats, no algorithms, no IO.
- `conversation.rs` — `ChatMessage` (the dialogue turn).
- `memory.rs` — `Memory`/`MemoryKind` + budget invariant (`over_budget`, `fifo_trim`) + sanitization.
- `drive.rs` — `DriveEntry` (a stored file).
- `knowledge.rs` — `SourceRef`, `KnowledgePassage`, `SourceSummary`.
- `page.rs` / `search.rs` — `PageUrl`/`PageContent`, `SearchQuery`/`SearchHit`.
- `transcript.rs` — `Segment`/`Transcript` (offset assembly, plain text).

### `application/` — the hexagon core (provider- and platform-agnostic)
- **`ports/`** — the contracts the app owns (`#[async_trait]` where they do IO):
  `chat_model` · `websearch` · `webfetch` · `drive` · `knowledge` (`KnowledgeIndex`,
  `Embeddings`) · `memory` (`MemoryStore`, `MemoryConsolidator`) · `ingestion`
  (`ArchivePolicy`, `TextExtractor`, `FileFetcher`) · `transcription` (`Transcriber`,
  `AudioCodec`, `Summarizer`).
- **`services/`** — reusable pure logic injected into usecases: `chunking` (text → embed
  windows), `foldering` (semantic subfolder choice over the drive + policy ports).
- **`usecases/`** — one operation each: `search_web`, `fetch_page`, `save_memory`,
  `ask_knowledge`, `reindex_knowledge`, `index_document`, `archive_attachment`,
  `transcribe_clip`.

### `infrastructure/` — format, protocol, and IO only
- **`driven/`** — the app calls *out* (each implements a port): `model_chat` /
  `model_transcriber` / `model_summarizer` / `model_archive_policy` / `embeddings` (the model
  server), `keenable` (web), `gdrive_storage`, `knowledge_index` (**cosine/RRF/FTS live
  here**), `memory_store` / `memory_consolidator`, `symphonia_codec` (audio decode/split),
  `ingest_extractor`, `http_fetcher`.
- **`driving/`** — call *into* the app: the per-capability adapters (`web`, `drive`,
  `knowledge`, `memory`, `ingestion`, `transcription`) that `tools.rs` / `discord.rs` /
  `voice.rs` invoke. (The serenity gateway and tauri commands themselves live at the crate
  root / `main.rs` and depend only on these.)
- **`dto/`** — boundary structs (`keenable`, `memory`) mapped to/from domain so serde never
  touches the core.
- **`shared/`** — cross-cutting infra wired first: `http` (shared client), `time`.

### `tools.rs` — the model↔app boundary, *outside* the hexagon
Tool dispatch (`ToolKind`, `execute`), approval gating, the attachment sink, and
**`TOOL_CALL` parsing** (`parse_tool_call`). Tools are partly declarative and partly
reactive, so they're kept out of the hexagon; `execute` calls into the driving adapters
([ADR-0001](adr/0001-tool-plugin-architecture.md)).

### `compose/` — composition root
The only place that names concrete adapters: `compose_*` builders wire driven adapters into
usecases and hand them to the driving side. `compose::shared::compose_shared()` warms the
shared infra (HTTP client) first, from `main.rs`'s Tauri `setup`.

## The provider-swap boundary

Swapping a vendor is an adapter change only. The `ChatModel` port hands the driving side a
clean reply:

```rust
trait ChatModel {
    async fn chat(&self, messages: Vec<ChatMessage>, on_delta: &(dyn Fn(&str) + Sync))
        -> Result<ChatReply, String>;
    async fn should_engage(&self, context: &str, message: &str) -> bool;
    async fn summarize_conversation(&self, previous: &str, new: &str) -> Result<String, String>;
}
```

Everything provider-specific — the wire protocol, streaming SSE, throughput parsing — lives
in `infrastructure/driven/model_chat` (wrapping `model.rs`). The same holds for every other
vendor: swap Keenable, the Drive REST client, the SQLite store, or the embedding/transcription
model and only an `infrastructure/driven/*` file changes.

> **Refinement vs [ADR-0002/0003](adr/README.md):** the ADRs sketched a structured
> `ChatRequest{tools}` → `ModelReply{tool_call}` with tool-call parsing inside the AI adapter.
> As built, the `TOOL_CALL {json}` convention lives in `tools.rs` (the model↔app boundary,
> outside the hexagon) rather than the adapter, because tools sit outside the hexagon. The
> boundary still holds — a vendor swap touches only `model_chat`.

## Where does X go? (quick rules)

| If it… | It's… |
|---|---|
| represents business data or enforces a business invariant | `domain/` |
| orchestrates a business operation | `application/usecases/` |
| is reusable logic with no IO, injected into usecases | `application/services/` |
| is a contract the app needs | `application/ports/` |
| parses/serializes an external format, or does IO | `infrastructure/{driven,driving}/` (+ `dto/`) |
| is a codec / ranking / search algorithm / wire-limit split | the relevant **driven adapter** |
| is the `TOOL_CALL` convention or tool dispatch | `tools.rs` (outside the hexagon) |
| names concrete adapters and wires them | `compose/` |

## Still at the crate root (by design)

`main.rs`/`api.rs`/`tray`/`window`/`macos` are entrypoints; `bot.rs` is the bot registry +
UI event model; `config.rs` is config persistence; `discord.rs`/`voice.rs` are the serenity
driving adapters; `model.rs`/`gdrive/`/`knowledge.rs`/`audio.rs`/`ingest.rs` are vendor/IO
helpers wrapped by the driven adapters. These depend only on `application` ports + the
driving/driven adapters — the dependency rule holds.
