import { createApp } from 'vue';
import App from './App.vue';

// Import local styles (includes theme)
import './styles/main.css';

// Initialize analytics
import { initAnalytics } from './utils/analytics';
initAnalytics();

createApp(App).mount('#app');
