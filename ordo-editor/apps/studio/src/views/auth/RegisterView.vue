<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { MessagePlugin } from 'tdesign-vue-next'

const router = useRouter()
const auth = useAuthStore()
const { t } = useI18n()

const displayName = ref('')
const email = ref('')
const password = ref('')
const password2 = ref('')
const loading = ref(false)

async function handleRegister() {
  if (!email.value || !password.value || !displayName.value) {
    MessagePlugin.warning(t('auth.fillRequired'))
    return
  }
  if (password.value !== password2.value) {
    MessagePlugin.warning(t('auth.passwordMismatch'))
    return
  }
  if (password.value.length < 8) {
    MessagePlugin.warning(t('auth.passwordTooShort'))
    return
  }
  loading.value = true
  try {
    await auth.register(email.value, password.value, displayName.value)
    router.push('/')
  } catch (e: any) {
    MessagePlugin.error(e.message || t('auth.registerFailed'))
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
        <h1 class="auth-title">{{ t('auth.createAccount') }}</h1>
        <p class="auth-subtitle">{{ t('auth.joinStudio') }}</p>
      </div>

      <div class="auth-form">
        <div class="form-field">
          <label class="form-label">{{ t('auth.displayNameLabel') }}</label>
          <t-input v-model="displayName" :placeholder="t('auth.displayNamePlaceholder')" size="large" clearable />
        </div>
        <div class="form-field">
          <label class="form-label">{{ t('auth.emailLabel') }}</label>
          <t-input v-model="email" placeholder="your@email.com" type="email" size="large" clearable />
        </div>
        <div class="form-field">
          <label class="form-label">{{ t('auth.passwordLabel') }}</label>
          <t-input v-model="password" :placeholder="t('auth.passwordMinPlaceholder')" type="password" size="large" />
        </div>
        <div class="form-field">
          <label class="form-label">{{ t('auth.confirmPasswordLabel') }}</label>
          <t-input
            v-model="password2"
            :placeholder="t('auth.confirmPasswordPlaceholder')"
            type="password"
            size="large"
            @keyup.enter="handleRegister"
          />
        </div>

        <t-button
          theme="primary"
          block
          size="large"
          :loading="loading"
          class="submit-btn"
          @click="handleRegister"
        >
          {{ t('auth.registerBtn') }}
        </t-button>
      </div>

      <div class="auth-footer">
        {{ t('auth.hasAccount') }}
        <router-link to="/login" class="auth-link">{{ t('auth.loginLink') }}</router-link>
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
</style>
