import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { MetricsData } from "../lib/bot"
import { StatusBar } from "./StatusBar"
import { Provider } from "./ui/provider"

function renderSB(props: {
  running: boolean
  busy?: string | null
  detail?: string | null
  metrics?: MetricsData
}) {
  return render(
    <Provider>
      <StatusBar {...props} />
    </Provider>,
  )
}

describe("StatusBar", () => {
  it("shows the busy label (sans trailing ellipsis) when busy", () => {
    renderSB({ running: true, busy: "🔎 Searching the web…" })
    expect(screen.getByText("🔎 Searching the web")).toBeInTheDocument()
    expect(screen.queryByText("Idle")).not.toBeInTheDocument()
  })

  it("shows Idle when running and not busy", () => {
    renderSB({ running: true })
    expect(screen.getByText("Idle")).toBeInTheDocument()
  })

  it("ignores a busy label when not running", () => {
    renderSB({ running: false, busy: "🔎 Searching the web…" })
    expect(screen.getByText("Stopped")).toBeInTheDocument()
  })

  it("shows Stopped when not running", () => {
    renderSB({ running: false })
    expect(screen.getByText("Stopped")).toBeInTheDocument()
  })

  it("formats inference speed when running", () => {
    renderSB({ running: true, metrics: { prefillTps: null, inferenceTps: 42 } })
    expect(screen.getByText(/42 tok\/s/)).toBeInTheDocument()
  })

  it("shows em dash for speed when stopped", () => {
    renderSB({ running: false, metrics: { prefillTps: null, inferenceTps: 42 } })
    expect(screen.getByText(/—/)).toBeInTheDocument()
  })

  it("shows tool progress on the right, over speed, when a detail is set", () => {
    renderSB({
      running: true,
      busy: "🎙️ Transcribing…",
      detail: "42%",
      metrics: { prefillTps: null, inferenceTps: 42 },
    })
    expect(screen.getByText("progress")).toBeInTheDocument()
    expect(screen.getByText("42%")).toBeInTheDocument()
    expect(screen.queryByText(/tok\/s/)).not.toBeInTheDocument()
  })
})
