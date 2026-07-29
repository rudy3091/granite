import { defineConfig } from "vite";

export default defineConfig({
  build: {
    lib: {
      entry: "src/editor.ts",
      name: "GraniteEditor",
      fileName: () => "editor.js",
      formats: ["iife"],
    },
    outDir: "dist",
    cssCodeSplit: false,
    emptyOutDir: true,
  },
});
