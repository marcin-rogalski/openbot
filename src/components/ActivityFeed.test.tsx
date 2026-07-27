import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { ActivityEvent } from "../lib/bot"
import { ActivityFeed } from "./ActivityFeed"
import { Provider } from "./ui/provider"

let seq = 0
const ev = (over: Partial<ActivityEvent>): ActivityEvent => ({
  botId: "b",
  id: `e${seq++}`,
  ts: Date.now(),
  kind: "message",
  content: "",
  ...over,
})

function renderFeed(events: ActivityEvent[], verbose = false) {
  return render(
    <Provider>
      <ActivityFeed events={events} verbose={verbose} />
    </Provider>,
  )
}

describe("ActivityFeed", () => {
  it("shows the empty state with no events", () => {
    renderFeed([])
    expect(screen.getByText(/Nothing yet/)).toBeInTheDocument()
  })

  it("renders a reply's content", () => {
    renderFeed([ev({ kind: "reply", content: "the answer" })])
    expect(screen.getByText("the answer")).toBeInTheDocument()
  })

  it("hides log/model_call unless verbose", () => {
    const events = [ev({ kind: "log", content: "debug line" })]
    const { rerender } = renderFeed(events, false)
    expect(screen.queryByText("debug line")).not.toBeInTheDocument()
    rerender(
      <Provider>
        <ActivityFeed events={events} verbose={true} />
      </Provider>,
    )
    expect(screen.getByText("debug line")).toBeInTheDocument()
  })

  it("groups consecutive same-summary tool calls with a count", () => {
    renderFeed(
      [
        ev({ kind: "tool_call", content: "a", summary: "🔎 Searched the web" }),
        ev({ kind: "tool_call", content: "b", summary: "🔎 Searched the web" }),
      ],
      false,
    )
    expect(screen.getByText(/×2/)).toBeInTheDocument()
  })
})
