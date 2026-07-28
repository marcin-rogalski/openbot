# ADR-0003: Prompt contribution, tool results, and the event bus

- **Status:** Accepted (2026-07-28)
- **Related:** [0001](0001-tool-plugin-architecture.md), [0004](0004-async-jobs-and-proactive-turns.md)

## Context

The proposal was "unified event formats — one for the bot to pick up, one for other tools,"
plus tools registering and injecting data (memory rules/notes) into the system prompt. In
practice these are **three different mechanisms** with different timing and shape. Unifying them
into one "event" would be a mistake.

## Decision

Keep three distinct flows.

### 1. System-prompt contribution — a synchronous *pull*, not an event

At prompt-build time the application asks each registered tool for its contribution:
`Tool::system_prompt_section(&host, bot) -> Option<String>`. Re-pulled **every turn**, because
it changes (memory rules/notes; a tool introducing its ops). This is how a tool "registers" its
presence to the model and injects data.

- **Token budget: generous for now.** No hard trimming initially — assemble everything with a
  large cap and revisit with tiering/priority later (tool one-liners always; full schemas/notes
  on demand). Memory's existing note/char budget stays as-is.

### 2. Tool → model results — the ReAct contract, as readable text

`Tool::execute` returns a `ToolResult`: **readable text** (what the model reads as
`TOOL_RESULT`) plus **optional structured metadata** (e.g. `sources` for the reply header,
already done today). Do **not** force a rigid JSON envelope — LLMs handle described-text results
better; the system prompt tells the model how to read them.

### 3. Tool ↔ tool — a pub/sub event bus

Generalises today's "attachment gate" (`AttachmentSink`/`deliver_attachment`) into a typed bus:
tools `host.emit(Event)` and receive `Tool::on_event`. Example: an **Attachments** tool emits
`AttachmentPosted`; **Drive** and **Transcription** subscribe.

- **Guards:** define event types up front (no stringly-typed free-for-all); **serialize**
  delivery through the bus for now (the ReAct loop is already sequential); cap fan-out depth to
  prevent **loops** (A emits → B emits → A…).
- **Only build it because we have real consumers** (attachments → drive + transcription). Don't
  ship a generic bus with one publisher.

## Consequences

- Clear separation: *registration/data* = pull at prompt time; *results* = ReAct text;
  *tool coordination* = bus. Each is simple on its own.
- The bus makes attachments/transcription first-class tools (ADR-0005) instead of a bespoke gate.
- Prompt size can grow with many tools — accepted for now (generous budget); tiering is a
  follow-up when it actually hurts.
