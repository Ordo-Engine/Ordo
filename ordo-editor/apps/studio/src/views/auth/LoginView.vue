<script setup lang="ts">
import { ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { MessagePlugin } from 'tdesign-vue-next'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()
const { t } = useI18n()

const email = ref('')
const password = ref('')
const loading = ref(false)

async function handleLogin() {
  if (!email.value || !password.value) return
  loading.value = true
  try {
    await auth.login(email.value, password.value)
    const redirect = (route.query.redirect as string) || '/'
    router.push(redirect)
  } catch (e: any) {
    MessagePlugin.error(e.message || t('auth.loginFailed'))
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="auth-page">
    <div class="auth-card">
      <div class="auth-header">
        <svg class="auth-logo" width="32" height="32" viewBox="0 0 24 24" fill="none">
          <rect x="3" y="3" width="8" height="8" rx="1" fill="#0066b8" />
          <rect x="13" y="3" width="8" height="8" rx="1" fill="#0066b8" opacity=".6" />
          <rect x="3" y="13" width="8" height="8" rx="1" fill="#0066b8" opacity=".6" />
          <rect x="13" y="13" width="8" height="8" rx="1" fill="#0066b8" opacity=".3" />
        </svg>
        <h1 class="auth-title">Ordo Studio</h1>
        <p class="auth-subtitle">{{ t('auth.subtitle') }}</p>
      </div>

      <div class="auth-form">
        <div class="form-field">
          <label class="form-label">{{ t('auth.emailLabel') }}</label>
          <t-input
            v-model="email"
            placeholder="your@email.com"
            type="email"
            autocomplete="email"
            size="large"
            clearable
          />
        </div>

        <div class="form-field">
          <label class="form-label">{{ t('auth.passwordLabel') }}</label>
          <t-input
            v-model="password"
            :placeholder="t('auth.passwordPlaceholder')"
            type="password"
            autocomplete="current-password"
            size="large"
            @keyup.enter="handleLogin"
          />
        </div>

        <t-button
          theme="primary"
          block
          size="large"
          :loading="loading"
          class="submit-btn"
          @click="handleLogin"
        >
          {{ t('auth.loginBtn') }}
        </t-button>
      </div>

      <div class="auth-footer">
        {{ t('auth.noAccount') }}
        <router-link to="/register" class="auth-link">{{ t('auth.registerLink') }}</router-link>
      </div>
    </div>
  </div>
</template>

<style scoped>
.auth-page {
  min-height: 100vh;
  background: var(--ordo-bg-app);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.auth-card {
  width: 100%;
  max-width: 400px;
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-xl);
  padding: 40px;
  box-shadow: var(--ordo-shadow-md);
}

.auth-header {
  text-align: center;
  margin-bottom: 32px;
}

.auth-logo {
  display: block;
  margin: 0 auto 12px;
}

.auth-title {
  font-size: 22px;
  font-weight: 700;
  color: var(--ordo-text-primary);
  margin: 0 0 4px;
}

.auth-subtitle {
  font-size: 13px;
  color: var(--ordo-text-secondary);
  margin: 0;
}

.auth-form {
  display: flex;
  flex-direction: column;
  gap: 20px;
  margin-bottom: 24px;
}

.form-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
}

.submit-btn {
  margin-top: 4px;
}

.auth-footer {
  text-align: center;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.auth-link {
  color: var(--ordo-accent);
  text-decoration: none;
  font-weight: 500;
}
.auth-link:hover {
  text-decoration: underline;
}
</style>
