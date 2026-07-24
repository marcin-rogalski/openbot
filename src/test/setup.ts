import "@testing-library/jest-dom/vitest"
import { vi } from "vitest"

// jsdom can't parse Chakra v3's modern CSS (color-mix, @layer) and logs a noisy
// "Could not parse CSS stylesheet" error for each rule. It's harmless (styling
// isn't asserted in tests), so drop just those messages.
const realConsoleError = console.error.bind(console)
console.error = (...args: unknown[]) => {
  if (typeof args[0] === "string" && args[0].includes("Could not parse CSS stylesheet")) {
    return
  }
  realConsoleError(...args)
}

// jsdom doesn't implement matchMedia; next-themes (via Chakra's Provider) needs
// it. Provide a minimal stub so components mount in tests.
if (!window.matchMedia) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }))
}
