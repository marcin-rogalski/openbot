import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { MetricsData } from "../lib/bot"
import { StatusBar } from "./StatusBar"
import { Provider } from "./ui/provider"

function renderSB(props: {
  running: boolean
  thinking?: boolean
  metrics?: MetricsData
}) {
  return render(
    <Provider>
      <StatusBar {...props} />
    </Provider>,
  )
}

describe("StatusBar", () => {
  it("shows Thinking when thinking", () => {
    renderSB({ running: true, thinking: true })
    expect(screen.getByText("Thinking")).toBeInTheDocument()
    expect(screen.queryByText("Running")).not.toBeInTheDocument()
  })

  it("shows Running when running and not thinking", () => {
    renderSB({ running: true })
    expect(screen.getByText("Running")).toBeInTheDocument()
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
})
