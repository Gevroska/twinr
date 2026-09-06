import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";
// import devtools from 'solid-devtools/vite';

export default defineConfig({
  esbuild: {
    jsx: "automatic",
    jsxImportSource: "solid-js",
  },
  plugins: [
    /* 
    Uncomment the following line to enable solid-devtools.
    For more info see https://github.com/thetarnav/solid-devtools/tree/main/packages/extension#readme
    */
    // devtools(),
    solidPlugin(),
  ],
  server: {
    port: 3100,
  },
  build: {
    target: "esnext",
    minify: "esbuild",
    cssMinify: true,
    // The complete HLS engine preserves alternate audio and container support.
    // Cache it independently from application edits, with a 600 kB size budget.
    chunkSizeWarningLimit: 600,
    rollupOptions: {
      output: {
        manualChunks: { hls: ["hls.js"] },
      },
    },
  },
});
