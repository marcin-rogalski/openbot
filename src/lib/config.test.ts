import { describe, expect, it } from "vitest"
import { defaultPolicy, newBot, newToolInstance, toolIcon, toolLabel } from "./config"

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
