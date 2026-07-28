# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **More visible progress on Discord** — the live status message now shows what the bot is
  doing *while* a tool runs (present-tense labels like "🎙️ Transcribing… ", "🔎 Searching
  the web…"), streams real progress for long operations (transcription: downloading →
  decoding → "chunk N/M"; reindex: "file N/M"), says "⏳ Waiting for your approval…" when a
  tool needs approval, and shows a persistent "Transcribing…" note for posted audio (instead
  of a typing indicator that times out). Status edits are throttled to respect rate limits.

- **Google Drive links** — paste a Drive link and the bot can read/summarize it (`read` now
  accepts a link or id), copy it into the tool's folder + index it (`save_link`), or, for an
  audio/video link, download + transcribe it and save a transcript + summary
  (`transcribe_link`). Recognizes the common link shapes; the file must be shared with the
  bot's Google account (or public). `transcribe_link` streams large files to disk and
  transcribes them in ~5-minute chunks, so long meeting/conference recordings (and the audio
  track of **video** files) work with bounded memory (up to ~2 GB).
- **Client-side audio decoding** — posted mp3 / m4a / FLAC / Ogg-Vorbis are decoded to WAV
  inside openbot (pure-Rust `symphonia`, new `audio.rs`) before transcription, so the model
  server needs no extra codecs (no ffmpeg). Opus (Discord voice messages) still needs a
  server that decodes it. Shared `pcm_to_wav` with the live-voice path.

- **Audio transcription** — audio attachments are transcribed via the model server's
  `/audio/transcriptions` endpoint (new **Transcription model** setting); the bot posts a
  transcript + summary `.md`, feeds the transcript into its reply, and (with a Drive tool)
  saves + indexes both into the knowledge base. Per-bot toggle.
- **Live voice-channel transcription** — the bot can join a voice channel (via a "join the
  call" mention), receive per-speaker audio (songbird), transcribe each utterance, and on
  leave post a speaker-labelled meeting transcript + summary (also indexed with a Drive
  tool). It announces that it's transcribing when it joins (consent).
- **Scheduled-meeting awareness** — when a Discord scheduled event tied to a voice channel
  goes live, the bot offers to join and transcribe it.
- Collapsible long tool-call rows in the activity feed.
- **Test coverage (phase 1):** unit tests for pure logic on both sides — Rust
  `#[cfg(test)]` modules (42 tests: config, policies, chunking, tool-call parsing, RRF/cosine,
  WAV/downmix, memory trimming, message splitting, voice commands…) and Vitest/RTL component
  + helper tests on the frontend. Coverage reporting wired into CI (`cargo-llvm-cov` +
  Vitest v8), report-only for now.

### Notes

- Voice transcription pulls a native Opus dependency; building needs a CMake/Opus toolchain
  and CI sets `CMAKE_POLICY_VERSION_MINIMUM=3.5`.

## [0.1.0] - 2026-07-27

First release: openbot runs local-LLM-powered Discord bots from a desktop app.

### Added

- **Multi-bot desktop app** — Tauri v2 (Rust) + React + Chakra UI v3, running from the
  menu-bar/tray with a hide-to-tray window. Configure and run several independent bots.
- **Bring-your-own model** — points at any OpenAI-compatible server for streaming chat and
  embeddings, driven by a ReAct tool loop with a repetition guard.
- **Google Drive knowledge base** — NotebookLM-style RAG with Drive as cold storage and a
  local SQLite index (FTS5 keyword + brute-force cosine). Operations for search, ask,
  reindex, list/read/create/update/delete, folders, and attachment backfill.
- **Attachment ingestion gate** — relevance-checked archiving of channel attachments into
  Drive with semantic foldering and local indexing (text + PDF; OCR/docx deferred).
- **Web search** — Keenable-backed search and page fetch, with sources surfaced atop replies.
- **Memory & rules** — per-bot notes injected into the prompt, with model-based
  consolidation (FIFO fallback).
- **Per-tool approvals** — `allow` / `ask` / `deny` per operation; reads default to allow,
  writes to ask.
- **Live Discord status message** — a single message shows progress (💭 Thinking… → tool
  steps → the answer) and resolves into the final reply.
- **Live activity feed** — streaming "Thinking" block, grouped/foldable tool calls,
  collapsible long output, and a Verbose mode.
- **Theme toggle** — Auto / Light / Dark, persisted.
- **Design system + Storybook** — Discord-inspired Chakra theme and component stories.
- **Localhost control API** — `GET /bots` and start/stop/toggle on `127.0.0.1:8787`.
- **CI/CD** — GitHub Actions CI (typecheck, Biome, tests, Storybook, rustfmt, clippy,
  cargo check) and a multi-platform Release workflow (macOS universal, Windows, Linux) via
  `tauri-action`, plus a Homebrew cask for macOS.

[Unreleased]: https://github.com/marcin-rogalski/openbot/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/marcin-rogalski/openbot/releases/tag/v0.1.0
