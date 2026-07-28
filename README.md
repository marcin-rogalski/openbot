# openbot

**Local-LLM-powered Discord bots on your desktop.** openbot is a macOS/Windows/Linux
app that runs one or more Discord bots backed by a model server *you* control — so
your conversations, files, and API keys stay on your machine. Each bot can search
and build a knowledge base from your Google Drive, search the web, and remember
facts and rules you give it, with per-tool approval so nothing happens behind your back.

> Built with [Tauri](https://tauri.app) (Rust) + React + TypeScript.

![CI](https://github.com/marcin-rogalski/openbot/actions/workflows/ci.yml/badge.svg)

## Features

- **Multi-bot** — run several independently-configured bots from one window, each with
  its own token, model, system prompt, and tools.
- **Bring your own model** — points at any OpenAI-compatible server (llama.cpp, LM
  Studio, Ollama's OpenAI endpoint, vLLM…) for both chat and embeddings.
- **Google Drive knowledge base** — a NotebookLM-style RAG: Drive is cold storage;
  parsing, embedding, and the search index live locally. Ask questions across your
  files and get grounded, cited answers.
- **Web search** — via [Keenable](https://keenable.ai).
- **Memory & rules** — the bot saves facts and follows guidance you give it, injected
  into its prompt.
- **Per-tool approval** — every tool operation is `allow` / `ask` / `deny`; writes
  default to *ask*, reads to *allow*.
- **Live status in Discord** — a single message shows the bot's progress
  (💭 Thinking… → 🔎 Searched… → the answer) and resolves into the final reply.
- **Local & private** — runs from the menu-bar/tray; your data never leaves your machine
  except the calls *you* configure (your model server, Google, Keenable, Discord).

## Quick start

```sh
# 1. Install (macOS, via the tap — see docs/releasing.md to set it up)
brew tap marcin-rogalski/openbot
brew install --cask openbot

# 2. Start a local OpenAI-compatible model server that serves chat + /embeddings
#    (default the app expects: http://127.0.0.1:8080/v1)

# 3. Launch openbot, add a bot, paste its Discord token, point it at your model,
#    enable the tools you want, and invite it to your server.
```

Prefer to build from source, or not on macOS? See **[docs/getting-started.md](docs/getting-started.md)**.

## How it works

The React UI configures bots and watches them work; a Rust backend runs each bot's
Discord gateway connection, drives an OpenAI-compatible model in a ReAct tool loop,
and executes tools (Google Drive, web, memory) under your approval policies. The
knowledge base is a local SQLite index (FTS5 keyword + brute-force cosine over
embeddings) built from files that live in Drive. See
**[docs/architecture.md](docs/architecture.md)** for the full picture.

## Documentation

- **[Getting started](docs/getting-started.md)** — prerequisites, install, first run
- **[Configuration](docs/configuration.md)** — every setting explained
- **[Tools](docs/tools.md)** — Google Drive knowledge base, web search, memory
- **[Architecture](docs/architecture.md)** — how it's built, for contributors
- **[Releasing](docs/releasing.md)** — tagging, multi-platform builds, Homebrew
- **[Contributing](docs/contributing.md)** — dev setup and how to extend it
- **[Architecture decisions](docs/adr/README.md)** — ADRs for the target core + tool-plugin design

## License

[MIT](LICENSE) © Marcin Rogalski
