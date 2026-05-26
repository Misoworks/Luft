import tailwindcss from "@tailwindcss/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

export default defineConfig({
  base: "./",
  plugins: [svelte(), tailwindcss(), viteSingleFile()],
  build: {
    assetsDir: "assets",
    emptyOutDir: true,
    outDir: "dist",
    sourcemap: false,
    target: "es2022",
  },
});
