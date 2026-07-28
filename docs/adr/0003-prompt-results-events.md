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

### 1. Tool availability + business context — a synchronous *pull* at prompt time

Two different contributions, both gathered per turn (not events):

- **Tool availability** = each tool's **structured `ToolManifest`** (name, ops, arg schemas,
  description). The application passes these to the `ChatModel` port; the **AI adapter** renders
  them into the provider's format — a `TOOL_CALL {json}` convention in the system prompt for
  OpenAI-compatible servers, or a native `tools` parameter for Anthropic (see
  [ADR-0002](0002-host-tool-sdk.md)/[ADR-0006](0006-module-layout.md)). So the *format* is infra;
  the *manifest* is declarative and tool-owned.
- **Business context** = `Tool::system_prompt_section(&ctx, bot) -> Option<String>`, plain
  provider-agnostic prompt **text** (memory's rules/notes, the bot's identity). Built by
  `application/services/prompt.rs` and injected as messages. This is business data, not protocol.

- **Token budget: generous for now.** No hard trimming initially — assemble everything with a
  large cap and revisit with tiering/priority later. Memory's existing note/char budget stays.

### 2. Tool → model results — readable text (format owned by the adapter)

`Tool::execute` returns a `ToolResult`: **readable text** + **optional structured metadata**
(e.g. `sources` for the reply header). The application treats it as text; *how* it's handed back
to the model (a `TOOL_RESULT:` line for the text convention, or a `tool_result` block for a
native API) is the **AI adapter's** job. Do **not** force a rigid JSON envelope on the text —
LLMs read described text better.

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
