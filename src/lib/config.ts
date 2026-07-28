import { LazyStore } from "@tauri-apps/plugin-store"

// Mirrors src-tauri/src/config.rs. settings.json holds two keys: `global`
// (Google sign-in + reusable tool instances) and `bots` (list of bot configs).
// Backend migrates the old single `config` into one bot on startup.

export type ToolPolicy = "allow" | "ask" | "deny"

export type ToolInstance = {
  id: string
  name: string
  /** Tool class, e.g. "google_drive". */
  type: string
  // Google Drive: self-contained integration + folder.
  clientId: string
  clientSecret: string
  folderId: string
  // Web Search (Keenable): a single API key.
  apiKey: string
}

/** Available tool classes the "+" menu offers. Add new providers here. */
export type ToolClass = { type: string; label: string; icon: string }
export const TOOL_CLASSES: ToolClass[] = [
  { type: "google_drive", label: "Google Drive", icon: "📁" },
  { type: "web_search", label: "Web Search", icon: "🔎" },
]

/** The callable ops each tool class exposes, mirroring the backend `DriveOp`/
 * `WebOp` suffixes. `write` sets the default policy (writes → ask, reads → allow)
 * and drives the per-bot approval editor. */
export type ToolOp = { op: string; label: string; write: boolean }
export const TOOL_OPS: Record<string, ToolOp[]> = {
  google_drive: [
    { op: "search", label: "Search files", write: false },
    { op: "ask", label: "Ask (knowledge base)", write: false },
    { op: "list_sources", label: "List indexed sources", write: false },
    { op: "list", label: "List files", write: false },
    { op: "read", label: "Read a file or link", write: false },
    { op: "save_link", label: "Save a Drive link", write: true },
    { op: "transcribe_link", label: "Transcribe a Drive link", write: true },
    { op: "create", label: "Create file", write: true },
    { op: "create_folder", label: "Create folder", write: true },
    { op: "update", label: "Update file", write: true },
    { op: "delete", label: "Delete (trash) file", write: true },
    { op: "reindex", label: "Rebuild the index", write: true },
    { op: "backfill_attachments", label: "Backfill attachments", write: true },
  ],
  web_search: [
    { op: "search", label: "Web search", write: false },
    { op: "fetch", label: "Fetch a page", write: false },
  ],
}

/** The default policy for an op when no explicit per-bot policy is set. */
export function defaultPolicy(write: boolean): ToolPolicy {
  return write ? "ask" : "allow"
}

export function toolIcon(type: string): string {
  return TOOL_CLASSES.find((c) => c.type === type)?.icon ?? "🔧"
}

export function toolLabel(type: string): string {
  return TOOL_CLASSES.find((c) => c.type === type)?.label ?? type
}

export type GlobalConfig = {
  tools: ToolInstance[]
  mcpServers: unknown[]
}

export type ModelConfig = {
  baseUrl: string
  modelName: string
  apiKey: string
  embeddingModel: string
  transcriptionModel: string
}

export type BotConfig = {
  id: string
  name: string
  color: string
  discordToken: string
  model: ModelConfig
  systemPrompt: string
  followupWindowMessages: number
  followupWindowSecs: number
  enabledToolIds: string[]
  toolPolicies: Record<string, ToolPolicy>
  memoryEnabled: boolean
  memoryMaxNotes: number
  memoryCharBudget: number
  attachmentsEnabled: boolean
  transcriptionEnabled: boolean
}

export const BOT_COLORS = [
  "#4c8bf5",
  "#22a06b",
  "#e0603b",
  "#9b5de5",
  "#f2a900",
  "#e5487f",
  "#00b8d9",
]

export const DEFAULT_GLOBAL: GlobalConfig = {
  tools: [],
  mcpServers: [],
}

export function newBot(index: number): BotConfig {
  return {
    id: crypto.randomUUID(),
    name: "New bot",
    color: BOT_COLORS[index % BOT_COLORS.length],
    discordToken: "",
    model: {
      baseUrl: "http://127.0.0.1:8080/v1",
      modelName: "",
      apiKey: "",
      embeddingModel: "nomic-embed-text",
      transcriptionModel: "whisper-1",
    },
    systemPrompt:
      "You are a helpful assistant in a Discord server. Keep replies concise.",
    followupWindowMessages: 5,
    followupWindowSecs: 180,
    enabledToolIds: [],
    toolPolicies: {},
    memoryEnabled: false,
    memoryMaxNotes: 40,
    memoryCharBudget: 2000,
    attachmentsEnabled: true,
    transcriptionEnabled: true,
  }
}

export function newToolInstance(type: string): ToolInstance {
  return {
    id: crypto.randomUUID(),
    name: toolLabel(type),
    type,
    clientId: "",
    clientSecret: "",
    folderId: "",
    apiKey: "",
  }
}

const store = new LazyStore("settings.json")

export async function loadGlobal(): Promise<GlobalConfig> {
  const g = await store.get<Partial<GlobalConfig>>("global")
  return { ...DEFAULT_GLOBAL, ...(g ?? {}) }
}

export async function saveGlobal(global: GlobalConfig): Promise<void> {
  await store.set("global", global)
  await store.save()
}

export async function loadBots(): Promise<BotConfig[]> {
  return (await store.get<BotConfig[]>("bots")) ?? []
}

export async function saveBots(bots: BotConfig[]): Promise<void> {
  await store.set("bots", bots)
  await store.save()
}
