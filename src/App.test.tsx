import { render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve({ running: false })),
}))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))

import App from "./App"
import { Provider } from "./components/ui/provider"

describe("App", () => {
  it("renders the app shell with the Chat tab and run toggle", () => {
    render(
      <Provider>
        <App />
      </Provider>,
    )
    expect(screen.getByRole("heading", { name: "openbot" })).toBeInTheDocument()
    expect(screen.getByRole("button", { name: "Chat" })).toBeInTheDocument()
    expect(screen.getByText("Run bot")).toBeInTheDocument()
  })
})
