import react from "@vitejs/plugin-react";
import path from "node:path";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
    },
  },
  server: {
    port: 17282,
    proxy: {
      "/api": "http://127.0.0.1:17280",
      "/healthz": "http://127.0.0.1:17280",
    },
  },
});
