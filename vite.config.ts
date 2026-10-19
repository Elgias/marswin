import { resolve } from "node:path";
import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  // Cargo locks files under src-tauri/target while building; watching them crashes Vite.
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "chrome120",
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        settings: resolve(__dirname, "settings.html"),
      },
    },
  },
});
