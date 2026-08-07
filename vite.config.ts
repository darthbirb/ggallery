/// <reference types="vitest/config" />
import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwind from "@tailwindcss/vite";

// Tauri drives the dev server; the port is fixed and must not wander.
export default defineConfig({
  plugins: [react(), tailwind()],
  // `@/` is the alias `components.json` declares, so anything added later
  // with `npx shadcn@latest add …` resolves without editing its imports.
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "chrome110",
    sourcemap: false,
  },
  worker: {
    format: "es",
  },
  // Interaction tests, not appearance tests — see ROADMAP.md §M2.5 "Build
  // notes". `lib/ipc` is mocked per test file, so nothing here ever reaches
  // Tauri, and jsdom is enough.
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    css: false,
  },
});
