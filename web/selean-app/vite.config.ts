import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    // WASM package is built by wasm-pack before `vite build` runs.
    // Vite bundles the JS glue; the .wasm file is emitted as an asset.
  },
  server: {
    port: 3000,
    proxy: {
      "/api/ws": {
        target: "ws://localhost:8080",
        ws: true,
      },
      "/api": {
        target: "http://localhost:8080",
        changeOrigin: true,
      },
    },
  },
});
