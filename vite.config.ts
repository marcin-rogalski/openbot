/// <reference types="vitest/config" />

import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// Tauri expects a fixed port and will fail if it's in use.
const TAURI_DEV_PORT = 1420

export default defineConfig(async () => ({
  plugins: [react()],

  // Prevent vite from obscuring rust errors.
  clearScreen: false,
  server: {
    port: TAURI_DEV_PORT,
    strictPort: true,
    watch: {
      // Tell vite to ignore watching `src-tauri`.
      ignored: ["**/src-tauri/**"],
    },
  },

  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
  },
}))
