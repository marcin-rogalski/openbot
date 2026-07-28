# Hexagonal architecture in openbot

How the backend (`src-tauri/src`) is being organized. This is the **target**; migration is
incremental and test-guarded (see the [ADRs](adr/README.md), especially
[0006](adr/0006-module-layout.md)). The current, still-tangled code is described in
[architecture.md](architecture.md).

## The one rule

**Dependencies point inward.** `domain` → (nothing); `application` → `domain`; `adapters` and
`tools` → `application`'s ports; `app.rs` (composition root) → everything. Nothing in
`domain`/`application` may import serenity, reqwest, rusqlite, tauri, symphonia, or any vendor
client. "Pure" = std + serde only.

```
                 ┌─────────── adapters/driving ───────────┐
   Discord in ──▶│ gateway · tauri commands · http api ·  │──┐
                 │ voice receiver · OS input               │  │ call usecases
                 └─────────────────────────────────────────┘  ▼
   tools/  ────────────────────────────────────▶  application (ports · services · usecases)
   (plugins, over ports)                                   │  ▲
                 ┌─────────── adapters/driven ────────────┐ │  │ implement ports
   Discord out ◀─│ openai · gdrive · sqlite · discord     │◀┘  │
   Drive/DB/…    │ sender · codec · store · tauri events   │────┘
                 └─────────────────────────────────────────┘
                              ▲ dto/ maps wire ⇆ domain
                       domain (business data + invariants)
```

## Layers

### `domain/` — business representation + its rules
Value objects and the operations *on that representation*: validation, sanitization, invariants.
No formats, no algorithms, no IO.
- `bot.rs` — `BotConfig`/`ModelConfig` + `is_ready` (business rule: is a bot runnable).
- `binding.rs` — `ToolInstance`, `Binding` (which tools a bot uses + policy) — see
  [ADR-0005](adr/0005-config-layering.md).
- `policy.rs` — `Policy` + resolution (writes→ask).
- `memory.rs` — `Memory` (note/rule) value object + budget invariant + note sanitization.

### `application/` — the hexagon core (provider- and platform-agnostic)
- **`ports/`** — the contracts the app owns (`#[async_trait]`, dyn-compatible):
  `ChatModel` · `Embedder` · `Transcriber` · `KnowledgeIndex` · `FileStore` · `WebSearch` ·
  `ChatPlatform` · `ConfigStore` · `EventSink` · `Bus` · `ApprovalGate` · `Tool`.
- **`services/`** — reusable, injected into usecases:
  - `context.rs` — the **`Ctx`** handed to tools/usecases: a struct of `Arc<dyn Port>` (this is
    the "Host" from [ADR-0002](adr/0002-host-tool-sdk.md), realized as a struct, not a mega-trait).
  - `tool_registry.rs` — register · dispatch · gather manifests + business prompt content.
  - `prompt.rs` — build the **business** prompt (identity, memory rules/notes, conversation).
  - `jobs.rs` — async job runner + proactive-turn helper ([ADR-0004](adr/0004-async-jobs-and-proactive-turns.md)).
- **`usecases/`** — one operation each: `handle_message` (the ReAct reply loop), `run_tool`,
  `ingest_document`, `reindex`, `transcribe_media`, `run_meeting`, `manage_bot`.

### `adapters/` — format, protocol, and IO only
- **`driving/`** — call *into* the app: `discord_gateway` (inbound), `tauri_commands`,
  `http_api` (control API), `voice_receiver` (songbird receive), `os/` (tray/window/macos input).
- **`driven/`** — the app calls *out* (each implements a port): `openai` (ChatModel/Embedder/
  Transcriber — **owns the `TOOL_CALL` text convention**, see below), `gdrive`, `keenable`,
  `sqlite` (KnowledgeIndex — **cosine/RRF/FTS live here**), `discord_sender` (ChatPlatform —
  **message splitting here**), `codec` (audio decode/wav — symphonia/audiopus), `store`,
  `tauri_events`.
- **`dto/`** — boundary structs: `openai`, `gdrive`, `events`, `http` — mapped to/from domain so
  serde never touches the core.

### `tools/` — plugins, *outside* the hexagon
Each capability is a cohesive module (`tools/drive/`, `websearch`, `memory`, `attachments`,
`transcription`) holding its **manifest + config (declarative)** and its **behaviour
(`execute` / `on_event`, reactive)** together, depending only on the `Ctx` ports. Tools are
kept out of the hexagon so they don't smear declarative + reactive concerns across
domain/application/adapters ([ADR-0001](adr/0001-tool-plugin-architecture.md)).

### `app.rs` — composition root
The only place that names concrete types: build the driven adapters → assemble the `Ctx` →
register the tools → start the driving adapters. `main.rs` just calls `app::run()`.

## The provider-swap boundary (why tool-calling lives in the AI adapter)

The `ChatModel` port is **structured on both ends**, so switching model providers is an adapter
change only:

```rust
trait ChatModel {
    async fn infer(&self, req: ChatRequest, on_token: &mut dyn FnMut(&str)) -> Result<ModelReply>;
}
struct ChatRequest { messages: Vec<Message>, tools: Vec<ToolManifest>, /* … */ }
struct ModelReply { text: String, tool_call: Option<ToolCall> }
```

The application passes messages + the available tools' **manifests**; it gets back a clean
`ModelReply`. *How* tools are advertised and *how* a call is read is provider-specific and lives
entirely in the driven AI adapter:
- OpenAI-compatible (e.g. oMLX today): inject a `TOOL_CALL {json}` text convention into the
  prompt and parse it back out (plus reply sanitizing).
- Anthropic: pass structured `tools`, receive native `tool_use` blocks — no text convention.

Consequently there are **two kinds of prompt contribution** ([ADR-0003](adr/0003-prompt-results-events.md)):
tool **availability** = a structured manifest the *adapter* renders; **business context**
(memory rules/notes, identity) = plain prompt text from `services/prompt.rs`.

## Where does X go? (quick rules)

| If it… | It's… |
|---|---|
| represents business data or enforces a business invariant | `domain/` |
| orchestrates a business operation | `application/usecases/` |
| is reusable logic with no IO, injected into usecases | `application/services/` |
| is a contract the app needs | `application/ports/` |
| parses/serializes an external format, or does IO | `adapters/{driven,driving}/` (+ `dto/`) |
| is a codec / ranking / search algorithm / wire-limit split | the relevant **driven adapter** |
| is a bot capability (declarative manifest + reactive behaviour) | `tools/` |

## Current → target map

| Today | Target |
|---|---|
| `model.rs` | `adapters/driven/openai.rs` (owns `TOOL_CALL` parse/sanitize) |
| `gdrive/`, `websearch.rs` | `adapters/driven/gdrive/`, `keenable.rs` |
| `knowledge.rs` | `domain`? no — split: ranking → `adapters/driven/sqlite.rs`; the store is the adapter |
| `ingest.rs`, `audio.rs` | `application/services/chunking.rs` (text) + `adapters/driven/codec.rs` (audio) |
| `discord.rs` | `adapters/driving/discord_gateway.rs` + `adapters/driven/discord_sender.rs` + `application/usecases/{handle_message,run_meeting}.rs` |
| `voice.rs` | `adapters/driving/voice_receiver.rs` + `application/usecases/run_meeting.rs` |
| `bot.rs` | `application/usecases/manage_bot.rs` + `ports/{approvals,events}` + `adapters/driven/tauri_events.rs` + `adapters/driving/tauri_commands.rs` |
| `config.rs` | `domain/*` (types) + `adapters/driven/store.rs` (persistence + migration) |
| `tools.rs` | `application/services/tool_registry.rs` + `tools/*` |
| `api.rs`, `main.rs`, `tray/window/macos` | `adapters/driving/{http_api,os}` + `app.rs` |

## Migration order (green at every step)

1. Carve `domain/` (the already-pure types + tests) — no behaviour change.
2. Define `application/ports/` (signatures only).
3. Wrap existing modules as driven adapters implementing the ports (delegate to today's code).
4. Add the `Ctx` + `Tool` + registry; **migrate one tool first (Web search)** as the proof,
   guarded by the tests already in place.
5. Move the reply loop into `application/usecases/handle_message.rs` behind `ChatPlatform`.
6. Port the rest; fold in Attachments + Transcription as tools (with the config migration).
