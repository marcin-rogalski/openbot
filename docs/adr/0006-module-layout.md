# ADR-0006: Module layout & migration

- **Status:** Accepted (2026-07-28)
- **Related:** [0001](0001-tool-plugin-architecture.md), [0002](0002-host-tool-sdk.md),
  [0003](0003-prompt-results-events.md)
- **Guide:** the fuller contributor walk-through is [../hexagonal.md](../hexagonal.md).

## Context

ADR-0001 set the direction (ports & adapters + tool plugins) but not the concrete module layout.
We refined it in review; the sharp points:

- **Domain** is the *business representation* of data + operations on it (validation,
  sanitization, invariants) — **not** formats, algorithms, or IO.
- **Application** = exactly `ports/` + `services/` + `usecases/`; **ports live in application**.
- **Adapters** split **driving** (into the app) vs **driven** (out of the app), plus a **`dto/`**.
- **Tools are plugins outside the hexagon** — cohesive modules (declarative manifest/config +
  reactive behaviour together), depending only on the application's ports. Not usecases.
- The provider-swap test: switching the model/DB/platform vendor must touch **adapters only**.

## Decision

```
src-tauri/src/
  main.rs            # entry → app::run()
  app.rs             # composition root (builds adapters, assembles Ctx, registers tools)
  domain/            # business data + invariants (std + serde only)
  application/
    ports/           # ChatModel, Embedder, Transcriber, KnowledgeIndex, FileStore, WebSearch,
                     #   ChatPlatform, ConfigStore, EventSink, Bus, ApprovalGate, Tool
    services/        # context (Ctx = struct of Arc<dyn Port>), tool_registry, prompt, jobs, chunking
    usecases/        # handle_message, run_tool, ingest_document, reindex,
                     #   transcribe_media, run_meeting, manage_bot
  adapters/
    driving/         # discord_gateway, tauri_commands, http_api, voice_receiver, os/
    driven/          # openai, gdrive, keenable, sqlite, discord_sender, codec, store, tauri_events
    dto/             # openai, gdrive, events, http  (boundary structs; mapped to/from domain)
  tools/             # drive, websearch, memory, attachments, transcription  (plugins over Ctx)
```

Rationale in [../hexagonal.md](../hexagonal.md); the current→target map and migration order too.
Key placements that fall out of the rules:

- **Ranking** (cosine/RRF/FTS) → `adapters/driven/sqlite.rs`; **message splitting** →
  `adapters/driven/discord_sender.rs`; **audio codec** → `adapters/driven/codec.rs`; the
  **`TOOL_CALL` text convention** (advertise tools + parse/sanitize calls) →
  `adapters/driven/openai.rs`. None of these are domain.
- **Text chunking** → `application/services/chunking.rs` (reusable, no IO).
- The **"Host"** is the `Ctx` struct-of-ports in `services/`, not a trait (amends ADR-0002).

## Migration

Incremental, compiling + green at every step (see the guide for the full sequence): carve
`domain/` → define `ports/` → wrap today's modules as driven adapters → add `Ctx`/`Tool`/registry
and **migrate one tool (Web search) first** → move the reply loop behind `ChatPlatform` → port the
rest → fold Attachments + Transcription in as tools.

## Consequences

- Adding a capability = a new module under `tools/`; changing a vendor = a new driven adapter.
- The core (`domain` + `application`) is unit-testable against fake ports — the coverage work
  already landed is the safety net for the migration.
- One-time cost: moving files + a `settings.json` migration when attachments/transcription become
  tool instances.
