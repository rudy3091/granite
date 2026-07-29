import { defineConfig } from "vite";

export default defineConfig({
  build: {
    lib: {
      entry: "src/main.ts",
      name: "GraniteEditor",
      fileName: () => "main.js",
      formats: ["es"],
    },
    outDir: "dist",
    cssCodeSplit: false,
    emptyOutDir: true,
  },
});
