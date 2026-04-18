import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

const packagesPath = resolve(__dirname, '../../packages')
const platformProxyTarget =
  process.env.ORDO_PLATFORM_PROXY_TARGET || 'http://localhost:3001'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@ordo-engine/editor-core': resolve(packagesPath, 'core/src/index.ts'),
      '@ordo-engine/editor-vue': resolve(packagesPath, 'vue/src/index.ts'),
    },
  },
  server: {
    host: '0.0.0.0',
    port: 3002,
    strictPort: true, // Always use 3002 — kill any process holding it first
    proxy: {
      // All /api traffic goes through ordo-platform (:3001)
      // ordo-platform proxies engine calls to ordo-server internally
      '/api': {
        target: platformProxyTarget,
        changeOrigin: true,
      },
    },
    fs: {
      allow: [
        resolve(__dirname, '..'),
        packagesPath,
        resolve(__dirname, '../../node_modules'),
      ],
    },
  },
  optimizeDeps: {
    include: [
      '@vue-flow/core',
      '@vue-flow/background',
      '@vue-flow/controls',
      '@vue-flow/minimap',
      '@vue-flow/node-resizer',
      'dagre',
      'tdesign-vue-next',
    ],
  },
})
