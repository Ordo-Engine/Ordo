import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { resolve } from 'path';

const packagesPath = resolve(__dirname, '../../packages');

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      // In dev mode, point to source files for hot reload
      '@ordo/editor-core': resolve(packagesPath, 'core/src/index.ts'),
      '@ordo/editor-vue': resolve(packagesPath, 'vue/src/index.ts'),
    },
  },
  server: {
    port: 3001,
    open: true,
    strictPort: false,
    fs: {
      // Allow serving files from the monorepo root
      allow: [
        resolve(__dirname, '..'),  // apps folder
        packagesPath,               // packages folder
        resolve(__dirname, '../../node_modules'), // root node_modules
      ],
    },
  },
  // Optimize deps to avoid issues with Vue Flow
  optimizeDeps: {
    include: [
      '@vue-flow/core',
      '@vue-flow/background',
      '@vue-flow/controls',
      '@vue-flow/minimap',
      '@vue-flow/node-resizer',
      'dagre',
    ],
  },
});
