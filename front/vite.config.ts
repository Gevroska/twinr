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
    minify: false,
    cssMinify: false,
  },
});
