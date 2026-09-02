import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Tauri has no Node server: prerender to static files (frontendDist ../dist)
    adapter: adapter({
      pages: "dist",
      assets: "dist",
      fallback: "index.html",
    }),
  },
};

export default config;
