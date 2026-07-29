# Architecture Decision Records

Short records of significant architecture decisions — the context, the decision, and its
consequences. Newest decisions may supersede older ones (noted in **Status**).

These ADRs record the "core + tool plugins" / ports-and-adapters direction. The Rust backend
has since been **implemented** on this architecture (with some details refined during the
build — e.g. adapters live in `infrastructure/{driven,driving}`, the composition root is
`compose/`, and the `TOOL_CALL` convention sits in `tools.rs` outside the hexagon rather than
in the AI adapter). The ADRs are kept as point-in-time decisions; the **current, as-built**
layout is the source of truth in [../hexagonal.md](../hexagonal.md) and
[../architecture.md](../architecture.md).

| ADR | Title | Status |
|---|---|---|
| [0001](0001-tool-plugin-architecture.md) | Core + tool-plugin architecture (ports & adapters) | Accepted |
| [0002](0002-host-tool-sdk.md) | The Host — the tool SDK / capability object | Accepted |
| [0003](0003-prompt-results-events.md) | Prompt contribution, tool results, and the event bus | Accepted |
| [0004](0004-async-jobs-and-proactive-turns.md) | Async jobs, deterministic vs model-driven paths, proactive turns | Accepted |
| [0005](0005-config-layering.md) | Config layering: definition · instance · binding | Accepted |
| [0006](0006-module-layout.md) | Module layout & migration | Accepted |

The contributor walk-through of the layout is [../hexagonal.md](../hexagonal.md).

## Format

Each ADR has: **Status**, **Context**, **Decision**, **Consequences**, and where useful a
**Sketch** (illustrative Rust, not final) and **Alternatives**. Keep them short.
