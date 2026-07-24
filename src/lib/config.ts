import { LazyStore } from "@tauri-apps/plugin-store"

// Mirrors BotConfig in src-tauri/src/config.rs. Persisted as a single "config"
// object in settings.json (app data dir), shared with the backend.

export type ToolPolicy = "allow" | "ask" | "deny"

export type BotConfig = {
  discordToken: string
  modelBaseUrl: string
  modelName: string
  apiKey: string
  systemPrompt: string
  followupWindowMessages: number
  followupWindowSecs: number
  googleClientId: string
  googleClientSecret: string
  driveFolderId: string
  toolPolicies: Record<string, ToolPolicy>
}

export const DEFAULT_CONFIG: BotConfig = {
  discordToken: "",
  modelBaseUrl: "http://127.0.0.1:8080/v1",
  modelName: "",
  apiKey: "",
  systemPrompt:
    "You are openbot, a helpful assistant in a Discord server. Keep replies concise.",
  followupWindowMessages: 5,
  followupWindowSecs: 180,
  googleClientId: "",
  googleClientSecret: "",
  driveFolderId: "",
  toolPolicies: {},
}

const STORE_FILE = "settings.json"
const CONFIG_KEY = "config"

const store = new LazyStore(STORE_FILE)

export async function loadConfig(): Promise<BotConfig> {
  const saved = await store.get<Partial<BotConfig>>(CONFIG_KEY)
  return { ...DEFAULT_CONFIG, ...(saved ?? {}) }
}

export async function saveConfig(config: BotConfig): Promise<void> {
  await store.set(CONFIG_KEY, config)
  await store.save()
}
