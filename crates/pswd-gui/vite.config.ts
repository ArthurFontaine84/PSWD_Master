/// <reference types="vitest/config" />
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";
import { defineConfig } from "vite";

// Adresse fournie par Tauri pour le développement sur un appareil mobile.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts"],
      // `main.ts` ne fait que monter l'application dans la page : il n'y a rien à tester.
      exclude: ["src/main.ts", "src/**/*.test.ts", "src/test-setup.ts"],
      thresholds: { lines: 100 },
    },
  },
});
