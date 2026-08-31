import { defineConfig } from 'vitest/config'

// Node environment, not jsdom: this is a Worker, there is no DOM. Node 20 provides
// the same WebCrypto globals (crypto.subtle) the Workers runtime does, so the pure
// modules and the crypto ones both run unmodified here.
export default defineConfig({
  test: {
    environment: 'node',
    globals: true,
    include: ['test/**/*.test.ts'],
  },
})
