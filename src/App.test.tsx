import { render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import App from "./App"

describe("App", () => {
  it("renders the placeholder heading", () => {
    render(<App />)
    expect(
      screen.getByRole("heading", { name: "openbot" }),
    ).toBeInTheDocument()
  })
})
