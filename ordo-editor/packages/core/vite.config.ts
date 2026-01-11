import { defineConfig } from 'vite';
import { resolve } from 'path';
import dts from 'vite-plugin-dts';

export default defineConfig({
  plugins: [
    dts({
      include: ['src/**/*'],
      outDir: 'dist',
    }),
  ],
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.ts'),
      name: 'OrdoEditorCore',
      formats: ['es', 'cjs'],
      fileName: (format) => `index.${format === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      // @ordo/wasm 是通过 Rust 编译的 WASM 包，需要单独构建
      // 在运行时动态加载，所以标记为外部依赖
      external: ['@ordo/wasm', /^@ordo\/wasm\/.*/],
    },
    sourcemap: true,
  },
});

