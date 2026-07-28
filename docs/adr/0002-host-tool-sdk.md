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

Pass every tool a single capability object, the **`Ctx`** ("Host") — a **struct of ports**, not a
mega-trait (a fat async trait is object-safety-hostile and awkward to wire). It lives in
`application/services/context.rs`; each capability is its own port (`application/ports/`),
implemented by a driven adapter. Concrete servers (OpenAI-compatible model, Drive, SQLite,
serenity) are adapters behind those ports.

> **Amendment (ADR-0006):** the "Host" is the `Ctx` struct below, and the AI capability is the
> structured `ChatModel` port — the model's tool-calling *format* (advertising tools, parsing
> calls) lives in the AI adapter, so switching provider changes only infra.

### Sketch (illustrative, not final)

```rust
struct Ctx {
    chat: Arc<dyn ChatModel>,          // structured infer (see below)
    embed: Arc<dyn Embedder>,
    transcribe: Arc<dyn Transcriber>,
    knowledge: Arc<dyn KnowledgeIndex>,
    files: Arc<dyn FileStore>,
    web: Arc<dyn WebSearch>,
    reply: Arc<dyn ChatPlatform>,      // platform-agnostic: say / attach / edit / delete / status
    store: Arc<dyn ConfigStore>,
    events: Arc<dyn Bus>,              // tool↔tool bus (ADR-0003)
    approvals: Arc<dyn ApprovalGate>,  // policy gate
    jobs: Arc<dyn Jobs>,               // async job lifecycle (ADR-0004)
    config: ResolvedConfig,            // base + instance + binding (ADR-0005)
}

// The AI port is structured on both ends, so tool-calling is provider-specific *infra*:
trait ChatModel: Send + Sync {
    async fn infer(&self, req: ChatRequest, on_token: &mut dyn FnMut(&str)) -> Result<ModelReply>;
}
struct ChatRequest { messages: Vec<Message>, tools: Vec<ToolManifest> /* … */ }
struct ModelReply { text: String, tool_call: Option<ToolCall> }
```

- **`ReplyChannel`** is the platform boundary: `say`, `attach(files)`, `edit_status`,
  `delete` — the Discord adapter implements it. This is what makes tools portable and is the
  natural home for the "status reflected on Discord + the chat preview" behaviour.
- **`status()`** unifies what we already do ad-hoc: the live status message edits and the
  activity-feed events become one `Status { state: Started|Progress|Done|Failed, text }`.
- The **base config (general + model)** is rigid: the Host builds `infer`/`embed`/`transcribe`
  from it, so tools never touch `base_url`/keys directly.

## Consequences

- Tools are decoupled from serenity and from concrete servers; a fake `Ctx` (ports stubbed)
  makes them unit-testable (assert emitted events, statuses, and model calls).
- New platform = a new `ChatPlatform` driven adapter, tools unchanged. New model vendor = a new
  `ChatModel` adapter (owning that vendor's tool-calling format), application/domain unchanged.
- Each **port** is a contract: adding a new port is cheap (a `Ctx` field); changing a port's
  method signature is the breaking change — so keep ports small and cohesive.
