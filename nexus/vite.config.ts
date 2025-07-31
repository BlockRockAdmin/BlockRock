import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { nodePolyfills } from 'vite-plugin-node-polyfills';

export default defineConfig({
  plugins: [
    vue(),
    nodePolyfills({
      globals: {
        Buffer: true,
        global: true,
        process: true,
      },
      include: ['buffer', 'crypto', 'stream', 'util'],
      protocolImports: true,
    }),
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      crypto: 'crypto-browserify',
      buffer: 'buffer/',
      stream: 'readable-stream',
      util: fileURLToPath(new URL('./node_modules/util', import.meta.url)),
    },
  },
  define: {
    'process.env': {},
    global: 'window',
    buffer: ['buffer', 'buffer'],
    Buffer: ['buffer', 'Buffer'],
  },
  server: {
    host: '0.0.0.0',
    port: 5173,
    hmr: {
      host: '192.168.8.236',
      port: 5173,
    },
    headers: {
      'Content-Type': 'text/html; charset=utf-8',
      'Cache-Control': 'public, max-age=0',
      'X-Content-Type-Options': 'nosniff',
    },
  },
  base: '/', // Aggiunto per coerenza
});
