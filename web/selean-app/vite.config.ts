import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      // The WASM package is built by wasm-pack and served at runtime.
      // Externalize it so Vite doesn't fail when the pkg dir is absent.
      external: [/selean_wasm/],
    },
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
