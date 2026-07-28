# Tools

Bots act through tools. Every tool call runs the ReAct loop:
the model emits `TOOL_CALL {json}`, the backend runs it under your
[approval policy](configuration.md#tools--approvals), and the result is fed
back as `TOOL_RESULT` until the model produces a final answer (capped at 10
tool iterations per reply).

## Google Drive knowledge base

Google Drive is treated as **cold storage / backup** — the source of truth for your
original files. Parsing, embedding, and the search index all happen **locally**; Drive is
only downloaded from when needed. This is the "NotebookLM via Discord" model: your files
live in Drive, but semantic search runs on your machine.

### Setup

Add a **Google Drive** tool (globally or per bot) with a Google OAuth **Client ID** +
**Client secret** and a target **Folder ID**. On first use the bot walks you through the
OAuth consent flow; tokens are stored locally.

### The local index

Each Drive tool instance has its own SQLite database (`<app_data>/knowledge/<id>.db`)
containing the file sources and their embedded text chunks. Retrieval is **hybrid**: FTS5
keyword search (BM25) unioned with brute-force cosine similarity over the chunk
embeddings, merged by reciprocal-rank fusion. The `.db` is a rebuildable cache — delete
it and `reindex` restores it from Drive.

### Operations

| Operation | What it does |
|---|---|
| **search** | Filename/keyword search across the Drive subtree. |
| **ask** | Semantic Q&A: embeds the question, retrieves the most relevant passages, and returns them cited (`### <name>`) for the model to synthesize an answer from. |
| **list_sources** | Lists what's currently in the local index. |
| **list** | Lists files/folders in Drive. |
| **read** | Reads a file's text — by id **or a pasted Drive share link** (PDF via Google's converter). Use to summarize a linked doc without saving it. |
| **save_link** | Copies a file from a pasted Drive link into the folder and indexes it. |
| **transcribe_link** | Downloads an audio/video file from a Drive link, transcribes it, and saves a transcript + summary. |
| **create** / **create_folder** | Creates a file or folder. |
| **update** / **delete** | Updates a file, or moves it to trash. |
| **reindex** | Walks the Drive subtree, downloading, parsing, embedding, and indexing every supported file it hasn't already indexed. Emits progress; safe to re-run. |
| **backfill_attachments** | On-demand scan of recent channel history for attachments to ingest (see below). |

Typical usage in Discord: *"reindex the knowledge base"* → then ask a cross-file question
and the bot answers with citations. If the index is empty, `ask` says so and suggests
reindexing.

### Google Drive links

Paste a Google Drive link and tell the bot what to do with it:

- *"summarize this"* → `read` fetches the doc's text and the bot summarizes it (nothing saved).
- *"save this to our drive"* → `save_link` copies the file into the tool's folder and indexes it.
- *"transcribe this"* (audio **or video** link) → `transcribe_link` streams the file to disk,
  splits it into ~5-minute WAV chunks, transcribes each, and saves a transcript + summary into
  the folder. This handles **long recordings** (meetings, conference audio) with bounded
  memory — including video files, whose audio track is extracted. Files up to ~2 GB.
  It runs **in the background**: the bot acknowledges immediately, shows live progress
  ("chunk N/M"), and posts the transcript + summary to the channel when finished — so it isn't
  blocked for the whole (possibly multi-hour) recording.

> **Speaker diarization** ("who said what" in a recording) is not yet supported for files —
> Whisper alone can't separate speakers, and it needs a separate diarization model. It's a
> planned optional hook (a configurable diarization endpoint). Live *voice channels* are
> already speaker-labelled by Discord user.

It recognizes the usual link shapes (`/file/d/<id>`, `/document/d/<id>`, `?id=<id>`,
`/drive/folders/<id>`). **Access requirement:** the bot reads with *its own* Google account,
so a link only works if it's shared with that account or set to **"anyone with the link."**
If it isn't, the bot reports that the link needs sharing.

### Attachment ingestion

When **attachments** are enabled, files posted in a watched channel pass through an
*attachment gate* that tools can subscribe to. The Drive tool's sink:

1. **Relevance gate** — a short model check decides whether the file is worth keeping
   (guided by your [memory rules](#memory), e.g. *"always store PDFs"*).
2. **Ingest** — download → extract text → chunk → embed → add to the local index.
3. **Semantic foldering** — a brief model classification picks the best existing subfolder,
   then uploads the original there (under your write-approval policy). Unsupported types
   are still archived to Drive, just without an index entry.

Files posted *before* the tool was watching aren't lost — run **backfill_attachments** to
sweep recent history on demand.

### Supported file types & limits

- **Text-ish** files → read directly.
- **PDF** → text extraction (`pdf-extract`; scanned/image-only PDFs need OCR, which is
  **deferred**).
- **docx/xlsx** parsing is **deferred**.
- Attachment size/count caps apply per message; long extracted text is truncated for the
  prompt.

## Web search

Backed by **[Keenable](https://keenable.ai)** (formerly Tavily). Add a **Web Search** tool
with your Keenable API key.

| Operation | What it does |
|---|---|
| **search** | Runs a web search and returns ranked results. |
| **fetch** | Fetches and extracts the readable content of a page. |

Both are reads, so they `allow` by default. When used, sources are surfaced at the top of
the reply.

## Memory

When **memory** is enabled, the bot can save facts and rules ("notes") that are injected
into its system prompt on subsequent turns — so it remembers preferences, context, and
guidance you give it (including rules that steer the attachment gate, like *"store all
`.pdf` files"*).

Memory is bounded by **max notes** and a **char budget**
([configuration](configuration.md#memory)). When it overflows, the model **consolidates**
older notes into a tighter summary; if that fails, it falls back to dropping the oldest
(FIFO). Memories are stored locally per bot.

## Audio & voice transcription

Transcription is a built-in capability (toggled per bot, on by default), backed by your
model server's `/audio/transcriptions` endpoint (the **Transcription model** setting —
e.g. whisper.cpp, faster-whisper, LM Studio, or OpenAI Whisper). It works in two modes.

### Audio files

Post an audio file (`.mp3`, `.m4a`, `.wav`, `.ogg`, `.flac`, …) in a channel the bot is in.
The bot transcribes it and replies with two attachments — `*.transcript.md` and
`*.summary.md` (overview + key points + action items) — and feeds the transcript into its
answer so it can act on the audio the same turn. With a **Google Drive** tool enabled, both
files are saved to Drive and indexed into the knowledge base, so you can `ask` about them
later. Files up to ~25 MB are transcribed.

Compressed audio is **decoded to WAV inside openbot** (pure-Rust `symphonia`, plus
`audiopus` for Opus) before it's sent, so the transcription server needs no extra codecs
(no ffmpeg): mp3, m4a/AAC, FLAC, Ogg Vorbis, **and Opus** (Discord's built-in *voice
messages*) all work against a plain Whisper endpoint. Live voice channels are unaffected
(openbot already sends WAV).

Transcripts are **timestamped** — each line is prefixed with `[MM:SS]` (using the model's
segment times), so long recordings are easy to navigate.

### Live voice channels

The bot can join a voice channel and transcribe the conversation in real time:

- **Join** — while you're in a voice channel, mention the bot with something like
  *"join the call and take notes"*. It joins, **announces that it's transcribing**
  (consent — it never records silently), and starts listening.
- **During** — it receives each speaker's audio, segments it on silence, and transcribes
  each utterance, building a running, speaker-labelled transcript.
- **Leave** — say *"stop"* or *"leave"*. The bot posts a `meeting-<time>.transcript.md` +
  `.summary.md` to the channel and (with a Drive tool) saves + indexes them.

**Scheduled meetings** — when a Discord *scheduled event* tied to a voice channel goes
live, the bot posts in the server's system channel asking whether it should join and
transcribe; confirm with a *"join the call"* mention.

Live voice needs the `GUILD_VOICE_STATES` and `GUILD_SCHEDULED_EVENTS` gateway intents
(both non-privileged, on by default). Building from source requires an Opus toolchain —
see [getting-started](getting-started.md#prerequisites).
