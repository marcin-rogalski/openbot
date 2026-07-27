# Getting started

## Prerequisites

**1. A model server.** openbot doesn't ship a model — it talks to any
OpenAI-compatible HTTP server that provides:

- **Chat completions** (streaming) at `POST {baseUrl}/chat/completions`
- **Embeddings** at `POST {baseUrl}/embeddings` (for the Google Drive knowledge base)

Anything that speaks the OpenAI API works: [llama.cpp](https://github.com/ggml-org/llama.cpp)
(`llama-server`), [LM Studio](https://lmstudio.ai), [Ollama](https://ollama.com) (its
`/v1` endpoint), [vLLM](https://github.com/vllm-project/vllm), etc. The app defaults to
`http://127.0.0.1:8080/v1` and the embedding model `nomic-embed-text` — change both in the
bot's **Model** settings if yours differ.

**2. A Discord bot token.** Create an application at the
[Discord Developer Portal](https://discord.com/developers/applications) → **Bot** → copy
the token. Under **Privileged Gateway Intents**, enable **Message Content Intent** (the bot
reads message text to respond). Then invite it to your server with the **Send Messages** and
**Read Message History** permissions.

**3. (Build from source only)** Rust and Tauri's system dependencies (see below), plus a
**CMake + Opus toolchain** for voice transcription — on macOS `brew install cmake opus`, on
Debian/Ubuntu `sudo apt-get install cmake libopus-dev`. If the bundled Opus fails to
configure with a modern CMake, set `CMAKE_POLICY_VERSION_MINIMUM=3.5` in the build
environment (the CI workflows already do).

## Install

### macOS — Homebrew (recommended)

```sh
brew tap marcin-rogalski/openbot
brew install --cask openbot
```

openbot is currently **unsigned**, so on first launch macOS may block it. If so:

```sh
xattr -dr com.apple.quarantine "/Applications/openbot.app"
```

…or right-click the app in Finder and choose **Open**. (Setting up the tap and future
signing is covered in [releasing.md](releasing.md).)

### Any platform — build from source

Install the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS
(Rust toolchain, and on Linux the WebKitGTK/AppIndicator dev packages). Then:

```sh
git clone https://github.com/marcin-rogalski/openbot.git
cd openbot
npm ci
npm run tauri:build     # produces a packaged app in src-tauri/target/release/bundle/
```

For live development use `npm run tauri:dev`.

## First run

1. **Add a bot** with the **+** above the bots list. Give it a name.
2. On the **Model** tab, set your server's **Base URL**, **Model name**, and (if you use
   the knowledge base) **Embedding model**. Add an API key only if your server needs one.
3. On the **General**/bot settings, paste the **Discord token** and tweak the **system
   prompt** if you like.
4. On the **Tools** tab, add and enable the tools you want, and set their approval
   policies (see [configuration.md](configuration.md#tools--approvals)).
5. Flip the bot **on** (the switch by its name). The status bar shows *Running*.
6. In your Discord server, **@mention** the bot or reply to it. Watch the single status
   message go **💭 Thinking… → tool steps → the answer**, and follow along live in the
   app's activity feed (toggle **Verbose** for the raw tool calls).

## Troubleshooting

- **Bot won't come online** — double-check the token and that **Message Content Intent**
  is enabled in the Developer Portal.
- **"Ask" / knowledge base returns nothing** — the index is empty; run **reindex** first
  (ask the bot to "reindex the knowledge base"), and confirm your server serves
  `/embeddings`.
- **No reply / it seems stuck** — open the app and toggle **Verbose** to see the tool
  loop; check that the Base URL is reachable from your machine.
