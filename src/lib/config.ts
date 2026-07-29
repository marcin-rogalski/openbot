import { invoke } from "@tauri-apps/api/core"
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
  // Memory: consolidation budget + the (sharable) store key.
  memoryMaxNotes: number
  memoryCharBudget: number
  storeId: string
}

// --- Tool manifest (single source of truth, from the Rust backend) ----------
// Tool classes, their config fields, and their ops are owned by the Rust tool
// modules and fetched via `tool_manifests`. The frontend renders every tool
// editor and approval list from these — no hardcoded tables here.

export type ManifestField = {
  key: string
  label: string
  secret: boolean
  number: boolean
}
export type ManifestOp = { op: string; label: string; write: boolean }
export type ToolManifest = {
  kind: string
  label: string
  icon: string
  oauth: boolean
  configCaption: string | null
  configFields: ManifestField[]
  ops: ManifestOp[]
}

/** Cache populated once at boot by `loadManifests`, read by the sync helpers
 * below (which run during render, after the cache is filled). */
let MANIFESTS: ToolManifest[] = []

export async function loadManifests(): Promise<ToolManifest[]> {
  MANIFESTS = await invoke<ToolManifest[]>("tool_manifests")
  return MANIFESTS
}

export function manifestFor(type: string): ToolManifest | undefined {
  return MANIFESTS.find((m) => m.kind === type)
}

/** The default policy for an op when no explicit per-bot policy is set. */
export function defaultPolicy(write: boolean): ToolPolicy {
  return write ? "ask" : "allow"
}

export function toolIcon(type: string): string {
  return manifestFor(type)?.icon ?? "🔧"
}

export function toolLabel(type: string): string {
  return manifestFor(type)?.label ?? type
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
    memoryMaxNotes: 40,
    memoryCharBudget: 2000,
    storeId: "",
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
