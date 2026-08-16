import { defineConfig } from "vite"

export default defineConfig({
  build: {
    outDir: "dist",
    emptyOutDir: false,
    minify: false,
    lib: {
      entry: "src/main/code.ts",
      formats: ["es"],
      fileName: () => "code.js",
    },
    rollupOptions: {
      output: {
        inlineDynamicImports: true,
      },
    },
  },
})
