import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@tauri-apps/plugin-network": path.resolve(
        __dirname,
        "./src/tauri-apps/plugin-network.ts",
      ),
    },
  },
  // Dev proxy: forward API and WebSocket calls to the running Axum server
  server: {
    proxy: {
      "/api": "http://localhost:7779",
      "/ws": { target: "ws://localhost:7779", ws: true },
    },
  },
});
