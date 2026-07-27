# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
