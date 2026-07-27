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
| **read** | Reads a file's text (PDF via Google's converter). |
| **create** / **create_folder** | Creates a file or folder. |
| **update** / **delete** | Updates a file, or moves it to trash. |
| **reindex** | Walks the Drive subtree, downloading, parsing, embedding, and indexing every supported file it hasn't already indexed. Emits progress; safe to re-run. |
| **backfill_attachments** | On-demand scan of recent channel history for attachments to ingest (see below). |

Typical usage in Discord: *"reindex the knowledge base"* → then ask a cross-file question
and the bot answers with citations. If the index is empty, `ask` says so and suggests
reindexing.

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
