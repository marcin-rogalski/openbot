# Architecture Decision Records

Short records of significant architecture decisions — the context, the decision, and its
consequences. Newest decisions may supersede older ones (noted in **Status**).

These ADRs describe the **target** "core + tool plugins" architecture we're migrating toward.
They are design, not yet implemented; the migration is incremental and test-guarded (see
ADR-0001). The current code is described in [../architecture.md](../architecture.md).

| ADR | Title | Status |
|---|---|---|
| [0001](0001-tool-plugin-architecture.md) | Core + tool-plugin architecture (ports & adapters) | Accepted |
| [0002](0002-host-tool-sdk.md) | The Host — the tool SDK / capability object | Accepted |
| [0003](0003-prompt-results-events.md) | Prompt contribution, tool results, and the event bus | Accepted |
| [0004](0004-async-jobs-and-proactive-turns.md) | Async jobs, deterministic vs model-driven paths, proactive turns | Accepted |
| [0005](0005-config-layering.md) | Config layering: definition · instance · binding | Accepted |

## Format

Each ADR has: **Status**, **Context**, **Decision**, **Consequences**, and where useful a
**Sketch** (illustrative Rust, not final) and **Alternatives**. Keep them short.
