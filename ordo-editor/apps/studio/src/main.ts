import { createApp } from 'vue';
import { createPinia } from 'pinia';
import TDesign from 'tdesign-vue-next';
import { initWasm } from '@ordo-engine/editor-core';
import router from './router';
import App from './App.vue';
import { i18n } from './i18n';

// Styles
import 'tdesign-vue-next/es/style/index.css';
import './styles/main.css';

// Initialize the WASM module (single source of truth for studio↔engine conversion)
// before mounting, so the synchronous converter used by normalizeRuleset is ready.
// A failure here is non-fatal — the app still mounts.
async function bootstrap() {
  try {
    await initWasm();
  } catch (err) {
    console.error('[ordo] WASM init failed:', err);
  }

  const app = createApp(App);
  app.use(createPinia());
  app.use(router);
  app.use(TDesign);
  app.use(i18n);
  app.mount('#app');
}

bootstrap();
