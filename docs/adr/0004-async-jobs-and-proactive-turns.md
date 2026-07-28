# ADR-0004: Async jobs, deterministic vs model-driven paths, proactive turns

- **Status:** Accepted (2026-07-28)
- **Related:** [0002](0002-host-tool-sdk.md), [0003](0003-prompt-results-events.md)

## Context

Two realities the request-response model doesn't cover, both already hit in the current code:

- **Background transcription** starts a long job, reports progress, and delivers a result
  *later, out of band* (it posts the transcript to the channel when done — `spawn_transcription`
  in `discord.rs`).
- **Attachment archiving** is deterministic — download → archive → index with **no model call**.

"The bot picks up an event automatically" implies a *push*, but the bot today only acts on
inbound Discord messages. That gap is a real new capability, not free.

## Decision

### Async jobs are first-class

A tool can start a **Job** via the Host: `start → status(Progress) → complete(result) | fail(err)`.
The job runs independently of the triggering turn; progress flows through `Host::status`
(ADR-0002) and results are delivered per the proactive-turn rule below.

### Two execution paths, both first-class

- **Model-driven** — the ReAct tool call (`Tool::execute`), for anything the LLM should decide
  or narrate.
- **Deterministic / event-driven** — gates, subscriptions, and jobs (`Tool::on_event`) that run
  **without a model turn**. The design must bless this; not everything routes through the LLM
  ("user/tool → model → back" is only half the picture).

### Proactive turns — choose per event

When a job/event needs to reach the channel, the emitter chooses:

- **Post directly** (default) — deterministic and cheap; what background transcription does now.
- **Wake the model** — request the bot compose a message (costs a model turn). Use sparingly,
  where phrasing/decision matters (e.g. "a scheduled meeting started — should I join?").

A proactive turn needs an explicit trigger + a target channel; it is a deliberate mechanism the
Host exposes, not an implicit side effect.

## Consequences

- Long work no longer blocks the reply loop (already true for transcription; generalised here).
- Deterministic flows stay fast and model-free; only opt into a model turn when it earns its cost.
- Concurrency stays bounded: jobs run on tasks, but bus delivery and the ReAct loop remain
  serial per conversation for now (revisit if throughput demands it).
- Failure is surfaced consistently: to the model as a `TOOL_RESULT` error (model path), and to
  the user via `Host::status`/`reply` (deterministic path).
