import { render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === "get_running_bots") return Promise.resolve([])
    return Promise.resolve(undefined)
  }),
}))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/plugin-store", () => ({
  LazyStore: class {
    get() {
      return Promise.resolve(null)
    }
    set() {
      return Promise.resolve()
    }
    save() {
      return Promise.resolve()
    }
  },
}))

import App from "./App"
import { Provider } from "./components/ui/provider"

describe("App", () => {
  it("renders the bot list shell", async () => {
    render(
      <Provider>
        <App />
      </Provider>,
    )
    expect(screen.getByText("Bots")).toBeInTheDocument()
    expect(screen.getByText("General settings")).toBeInTheDocument()
    expect(await screen.findByText(/No bots yet/)).toBeInTheDocument()
  })
})
