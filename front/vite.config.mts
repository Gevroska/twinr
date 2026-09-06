import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";

export default defineConfig({
  plugins: [solidPlugin()],
  server: {
    port: 3100,
  },
  build: {
    manifest: true,
    target: "esnext",
    minify: "oxc",
    cssMinify: "lightningcss",
    // The complete HLS engine preserves alternate audio and container support.
    // Cache it independently from application edits, with a 600 kB size budget.
    chunkSizeWarningLimit: 600,
    rolldownOptions: {
      output: {
        // Match the Rust server's immutable asset-cache detection.
        hashCharacters: "hex",
        codeSplitting: { groups: [{ name: "hls", test: /node_modules[/\\]hls\.js/ }] },
      },
    },
  },
});
