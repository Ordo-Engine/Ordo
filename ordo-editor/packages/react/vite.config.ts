import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';
import dts from 'vite-plugin-dts';

export default defineConfig({
  plugins: [
    react(),
    dts({
      include: ['src/**/*'],
      outDir: 'dist',
    }),
  ],
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.ts'),
      name: 'OrdoEditorReact',
      formats: ['es', 'cjs'],
      fileName: (format) => `index.${format === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: ['react', 'react-dom', '@ordo/editor-core'],
      output: {
        globals: {
          react: 'React',
          'react-dom': 'ReactDOM',
          '@ordo/editor-core': 'OrdoEditorCore',
        },
        assetFileNames: (assetInfo) => {
          if (assetInfo.name === 'style.css') {
            return 'styles/index.css';
          }
          return assetInfo.name || 'assets/[name][extname]';
        },
      },
    },
    sourcemap: true,
    cssCodeSplit: false,
  },
});

