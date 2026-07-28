# ADR-0001: Core + tool-plugin architecture (ports & adapters)

- **Status:** Accepted (2026-07-28)
- **Related:** [0002](0002-host-tool-sdk.md), [0003](0003-prompt-results-events.md),
  [0004](0004-async-jobs-and-proactive-turns.md), [0005](0005-config-layering.md)

## Context

Capabilities are hard-wired today:

- `tools.rs` dispatches on a fixed `ToolKind` enum (`Drive`/`Web`/`Memory`); adding a tool
  means editing the enum, `execute`, `summary`, `active_label`, `catalog`, the frontend
  `TOOL_OPS`, and often special-casing it in `discord.rs` (`is_backfill`, `is_transcribe_link`).
- **Attachments** and **audio transcription** are bot-native, not tools — different code paths
  for the same idea (react to input, do work, report status).
- Tools call concrete infrastructure directly (`gdrive::`, `knowledge::`, `model::`, and
  serenity in `discord.rs`), so nothing is swappable or unit-testable in isolation.

We want new capabilities to be **plugins** with one uniform lifecycle, and the model server to
be the stable core they build on.

## Decision

Adopt **hexagonal layering** and model every capability as a **`Tool` plugin**. The concrete
module layout is in [ADR-0006](0006-module-layout.md); in outline:

```
domain/        business data + invariants (no formats, algorithms, or IO)
application/   ports/ · services/ · usecases/   (the hexagon core; ports live here)
adapters/      driving/ (into the app) · driven/ (out of the app) · dto/
tools/         the plugins — cohesive, outside the hexagon, over the ports
app.rs         composition root
```

- A `Tool` is a Rust **trait**; tools are registered at startup into a `Vec<Box<dyn Tool>>`
  ("registry"). No dynamic loading — see Alternatives.
- **Tools live outside the hexagon** so their declarative (manifest/config) and reactive
  (`execute`/`on_event`) halves stay together instead of smearing across domain/application/
  adapters. They depend only on the application's ports via the `Ctx` (ADR-0002), never on
  concrete adapters or serenity.
- Attachments and transcription **become tools** with their own config (ADR-0005).
- The three data flows (prompt / results / bus) are defined in ADR-0003; the job + proactive
  paths in ADR-0004.

### Sketch (illustrative, not final)

```rust
trait Tool: Send + Sync {
    /// Stable id, human name, and the ops it exposes (drives the catalog + policy keys).
    fn manifest(&self) -> ToolManifest;

    /// Optional system-prompt contribution, pulled fresh each turn (ADR-0003).
    async fn system_prompt_section(&self, host: &dyn Host, bot: &BotId) -> Option<String> { None }

    /// A model-invoked operation (the ReAct path). Returns readable text (+ optional
    /// structured metadata) for TOOL_RESULT.
    async fn execute(&self, host: &dyn Host, call: &ToolCall) -> ToolResult;

    /// React to bus events (the deterministic path). Default: ignore. (ADR-0003/0004)
    async fn on_event(&self, host: &dyn Host, event: &Event) {}
}
```

## Consequences

- Adding a capability = implement `Tool` + register; no cross-file surgery.
- The domain + application layers become unit-testable against a **fake `Host`** — building on
  the coverage work already in place.
- One uniform notion of "capability" replaces the tool/gate/native split.
- Migration is **incremental and test-guarded**: keep the current dispatch working; port one
  clean tool first (**Web search** or **Memory**) behind the new trait+Host, prove it, then
  move the rest, and finally fold attachments/transcription in as tools.

## Alternatives

- **Dynamic / WASM / dlopen third-party plugins** — rejected. Huge scope (ABI, sandboxing,
  versioning) for a personal multi-bot app; compile-time traits give the whole design with none
  of the cost. Revisit only if third-party distribution becomes a goal.
