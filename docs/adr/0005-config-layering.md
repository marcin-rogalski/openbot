# ADR-0005: Config layering — definition · instance · binding

- **Status:** Accepted (2026-07-28)
- **Related:** [0001](0001-tool-plugin-architecture.md), [0002](0002-host-tool-sdk.md)

## Context

Config already spans three ideas but they aren't named, and two capabilities sit outside the
tool config entirely:

- `GlobalConfig.tools` holds credentialed **tool instances** (a Drive OAuth + folder, a Keenable
  key).
- `BotConfig.enabled_tool_ids` + `tool_policies` bind instances to a bot with approval policy.
- **Attachments** (`attachments_enabled`) and **transcription** (`transcription_enabled` +
  `transcription_model`) are bot-native flags, not tool config.

The base **general + model config** (`base_url`, `model_name`, `api_key`, `embedding_model`,
`transcription_model`) is the rigid core the Host builds `infer`/`embed`/`transcribe` from.

## Decision

Name and keep three distinct layers; never collapse them.

1. **Definition** — the plugin itself (code): the `Tool` implementation + its `ToolManifest`
   (id, ops, arg schemas, default policies). No user data.
2. **Instance** — a configured, usually credentialed occurrence, global and shareable across
   bots (e.g. "Case files" Drive = OAuth + folder id; a transcription config = model + chunk
   size + size caps; a web-search key). Lives in `GlobalConfig`.
3. **Binding** — per-bot: which instances are enabled + per-op approval policy
   (`allow`/`ask`/`deny`). Lives in `BotConfig`.

Consequences of this framing:

- **Attachments and transcription become tool instances** with their own config (as desired):
  the bot-native `attachments_enabled` / `transcription_*` flags migrate to instance config +
  bindings, so they gain approval policies and per-bot settings like any other tool.
- The **base config stays the rigid core**: the Host derives the AI SDK from it; tools read
  everything through `Host::config()` (a `ResolvedConfig` = base ⊕ instance ⊕ binding), never
  `base_url`/keys directly.
- One Drive instance shared by several bots keeps **independent policies** per bot — only the
  three-layer split makes that correct.

## Consequences

- The frontend `TOOL_CLASSES` / `TOOL_OPS` / instance editors map cleanly onto
  definition/instance, and the per-bot Tools tab onto binding — mostly a renaming +
  generalisation of what exists.
- Migration note: moving attachments/transcription into instances needs a `settings.json`
  migration (like the earlier legacy-config migration in `config.rs`).
