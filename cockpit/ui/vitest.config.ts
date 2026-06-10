import { defineConfig } from 'vitest/config'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { svelteTesting } from '@testing-library/svelte/vite'

// Dashboard build (Lane A) test config. Kept separate from vite.config.ts so the
// build config stays vite-only and the project's existing `tsc` config-typecheck is
// unaffected by the vitest⇄vite type skew.
export default defineConfig({
  plugins: [
    svelte(),
    // Resolves the `browser` export condition so @testing-library/svelte mounts the
    // client (not SSR) build under vitest, and auto-cleans the DOM between tests.
    svelteTesting(),
  ],
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['src/**/*.{test,spec}.{ts,svelte.ts}'],
  },
})
