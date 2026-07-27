# Configuration

Settings are stored locally by `tauri-plugin-store` in `settings.json` (in the app's
data directory), under two keys: **`global`** (reusable tool instances) and **`bots`**
(the list of bot configs). You edit everything from the UI — this page explains what
each setting does.

## Global settings

Opened from the gear icon in the title bar.

- **Tools** — reusable *tool instances* (e.g. a Google Drive connection, a Keenable
  key) that any bot can enable. Defining a tool here once lets multiple bots share it.
  Deleting one is under its **Danger zone**.

## Per-bot settings

Each bot is configured on its own tabs.

### Model

Points the bot at your OpenAI-compatible server.

| Field | Default | Notes |
|---|---|---|
| **Base URL** | `http://127.0.0.1:8080/v1` | Chat + embeddings live under this. |
| **Model name** | — | The chat model id your server expects. |
| **Embedding model** | `nomic-embed-text` | Used for the knowledge base; must be served at `/embeddings`. |
| **API key** | — | Only if your server requires auth (sent as a bearer token). |

### Behavior

- **System prompt** — the bot's persona/instructions. Default: *"You are a helpful
  assistant in a Discord server. Keep replies concise."*
- **Follow-up window** — after replying, the bot keeps responding to the same
  conversation without needing a mention for a short window: **5 messages** or **180
  seconds** by default (whichever comes first).

### Tools & approvals

On the **Tools** tab, a switch to the left of each tool enables it for this bot; the
collapsible row holds that tool's config, including **Approvals**.

Every tool *operation* has a policy:

| Policy | Meaning |
|---|---|
| **allow** | Runs automatically. |
| **ask** | Prompts you for approval in the app before running. |
| **deny** | The bot can't use it. |

**Defaults:** read operations → `allow`, write operations → `ask`. Only policies you
change from the default are stored. The operations per tool:

**Google Drive**

| Op | Kind | Default |
|---|---|---|
| Search files | read | allow |
| Ask (knowledge base) | read | allow |
| List indexed sources | read | allow |
| List files | read | allow |
| Read a file | read | allow |
| Create file | write | ask |
| Create folder | write | ask |
| Update file | write | ask |
| Delete (trash) file | write | ask |
| Rebuild the index (reindex) | write | ask |
| Backfill attachments | write | ask |

**Web Search**

| Op | Kind | Default |
|---|---|---|
| Web search | read | allow |
| Fetch a page | read | allow |

### Memory

- **Enable memory** — lets the bot save facts/rules that get injected into its prompt.
  Off by default.
- **Max notes** (default **40**) and **char budget** (default **2000**) — when memory
  grows past these, older notes are consolidated by the model (FIFO fallback).

See [tools.md](tools.md#memory) for how the bot uses it.

### Attachments

- **Enable attachments** (on by default) — when someone posts a file in a channel the bot
  watches, tools that subscribe to the attachment gate can react to it (e.g. Google Drive
  archives + indexes relevant files). See [tools.md](tools.md#attachment-ingestion).

## The tools you can add

New tool *classes* are offered by the **+** menu:

- **Google Drive** — needs a Google OAuth **Client ID** + **Client secret**, and a target
  **Folder ID**. See [tools.md](tools.md#google-drive-knowledge-base).
- **Web Search** — needs a single **Keenable API key**.

## Remote control API

While the app is running it exposes a tiny **localhost-only** HTTP API on
`127.0.0.1:8787` for listing and toggling bots (handy for scripts):

```
GET  /bots                → { "bots": [{ id, name, running, ready }] }
POST /bots/<id>/start
POST /bots/<id>/stop
POST /bots/<id>/toggle
```

It binds to loopback only and has no auth — it's meant for local use.
