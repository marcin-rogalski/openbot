# ADR-0002: The Host — the tool SDK / capability object

- **Status:** Accepted (2026-07-28)
- **Related:** [0001](0001-tool-plugin-architecture.md), [0004](0004-async-jobs-and-proactive-turns.md)

## Context

The initial framing was an "SDK" of `infer` / `embed` / `transcribe` built on the base config.
But tools need more than model calls: they emit progress (the Discord live-status message + the
app activity feed), post messages/attachments, read/write the knowledge index, subscribe to
events, and run under the approval policy. And they should not know they're talking to *Discord*
specifically (Slack, etc. later).

## Decision

Pass every tool a single **`Host`** — the ports the application layer implements. It is the tool
SDK. Concrete servers (OpenAI-compatible model, Google Drive, SQLite, serenity/Discord) are
**adapters behind it**.

### Sketch (illustrative, not final)

```rust
trait Host: Send + Sync {
    // --- AI (the "SDK": today's model.rs, behind ports) ---
    async fn infer(&self, req: InferRequest) -> Result<InferResponse>;   // chat/completions
    async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>>;   // /embeddings
    async fn transcribe(&self, audio: Audio) -> Result<Vec<Segment>>;    // /audio/transcriptions

    // --- Runtime ---
    fn emit(&self, event: Event);                 // tool↔tool bus (ADR-0003)
    fn status(&self, update: Status);             // progress → Discord status + app preview
    fn reply(&self) -> &dyn ReplyChannel;         // platform-agnostic: post / attach / edit / delete
    fn store(&self) -> &dyn Store;                // kv + the knowledge index
    fn config(&self) -> &ResolvedConfig;          // base + instance + binding (ADR-0005)
    async fn approve(&self, op: &OpRef, args: &Value) -> Decision;  // policy gate
    fn spawn_job<F>(&self, job: F);               // async job lifecycle (ADR-0004)
}
```

- **`ReplyChannel`** is the platform boundary: `say`, `attach(files)`, `edit_status`,
  `delete` — the Discord adapter implements it. This is what makes tools portable and is the
  natural home for the "status reflected on Discord + the chat preview" behaviour.
- **`status()`** unifies what we already do ad-hoc: the live status message edits and the
  activity-feed events become one `Status { state: Started|Progress|Done|Failed, text }`.
- The **base config (general + model)** is rigid: the Host builds `infer`/`embed`/`transcribe`
  from it, so tools never touch `base_url`/keys directly.

## Consequences

- Tools are decoupled from serenity and from concrete servers; a fake `Host` makes them
  unit-testable (assert emitted events, statuses, and model calls).
- New platforms = a new `ReplyChannel`/driver adapter, tools unchanged.
- The Host is a **contract**: adding a method is a breaking change across tools, so keep it
  small and cohesive; prefer capabilities behind sub-traits (`Store`, `ReplyChannel`) over a
  fat interface.
