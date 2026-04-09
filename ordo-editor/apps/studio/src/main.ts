import { createApp } from 'vue'
import { createPinia } from 'pinia'
import TDesign from 'tdesign-vue-next'
import router from './router'
import App from './App.vue'
import { i18n } from './i18n'

// Styles
import 'tdesign-vue-next/es/style/index.css'
import './styles/main.css'

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(TDesign)
app.use(i18n)
app.mount('#app')
