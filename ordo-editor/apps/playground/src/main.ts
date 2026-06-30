import { createApp } from 'vue';
import { initWasm } from '@ordo-engine/editor-vue';
import App from './App.vue';

// Import local styles (includes theme)
import './styles/main.css';

// Initialize analytics
import { initAnalytics } from './utils/analytics';
initAnalytics();

// Initialize the WASM module (single source of truth for studio↔engine conversion)
// before mounting, so the synchronous converter call-sites are ready.
async function bootstrap() {
  try {
    await initWasm();
  } catch (err) {
    console.error('[ordo] WASM init failed:', err);
  }
  createApp(App).mount('#app');
}

bootstrap();
