import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import license from "rollup-plugin-license";
import path from "node:path";

// `npm run build` writes dist/, which is committed and embedded in the okbase binary.
// `npm run dev` proxies the API to a running `okbase view` (default port 7332).
export default defineConfig({
  plugins: [
    svelte(),
    license({
      thirdParty: {
        output: path.resolve(import.meta.dirname, "dist/LICENSES.txt"),
        allow: "(MIT OR Apache-2.0 OR ISC OR BSD-2-Clause OR BSD-3-Clause)",
      },
    }),
  ],
  base: "/",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    assetsDir: "assets",
    sourcemap: false,
    chunkSizeWarningLimit: 600,
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:7332",
      "/files": "http://127.0.0.1:7332",
    },
  },
});
