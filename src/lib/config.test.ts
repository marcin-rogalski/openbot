import { beforeAll, describe, expect, it, vi } from "vitest"

// The tool schema now comes from the backend; seed the manifest cache so the
// icon/label/instance helpers resolve known types.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => [
    {
      kind: "google_drive",
      label: "Google Drive",
      icon: "📁",
      oauth: true,
      configCaption: null,
      configFields: [],
      ops: [],
    },
    {
      kind: "web_search",
      label: "Web Search",
      icon: "🔎",
      oauth: false,
      configCaption: null,
      configFields: [],
      ops: [],
    },
  ]),
}))

import {
  defaultPolicy,
  loadManifests,
  newBot,
  newToolInstance,
  toolIcon,
  toolLabel,
} from "./config"

beforeAll(async () => {
  await loadManifests()
})

describe("config helpers", () => {
  it("defaultPolicy: reads allow, writes ask", () => {
    expect(defaultPolicy(false)).toBe("allow")
    expect(defaultPolicy(true)).toBe("ask")
  })

  it("toolIcon/toolLabel resolve known types and fall back", () => {
    expect(toolLabel("google_drive")).toBe("Google Drive")
    expect(toolIcon("web_search")).toBe("🔎")
    expect(toolLabel("unknown")).toBe("unknown")
    expect(toolIcon("unknown")).toBe("🔧")
  })

  it("newBot carries audio + attachment defaults", () => {
    const b = newBot(0)
    expect(b.transcriptionEnabled).toBe(true)
    expect(b.attachmentsEnabled).toBe(true)
    expect(b.model.transcriptionModel).toBe("whisper-1")
    expect(b.model.embeddingModel).toBe("nomic-embed-text")
    expect(b.id).toBeTruthy()
  })

  it("newToolInstance sets type + label and a fresh id", () => {
    const t = newToolInstance("web_search")
    expect(t.type).toBe("web_search")
    expect(t.name).toBe("Web Search")
    expect(t.id).toBeTruthy()
  })
})
