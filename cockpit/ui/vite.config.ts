import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vite.dev/config/
// NOTE: vitest config lives in vitest.config.ts so this build config stays vite-only
// (the project's `tsc -p tsconfig.node.json` typechecks this file against vite's types).
export default defineConfig({
  plugins: [svelte()],
})
