import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  root: resolve(import.meta.dirname, 'ui'),
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      usePolling: false,
    },
  },
  clearScreen: false,
});
