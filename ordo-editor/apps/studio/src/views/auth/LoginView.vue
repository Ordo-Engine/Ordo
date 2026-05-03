<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useAuthStore } from '@/stores/auth';
import { useSystemStore } from '@/stores/system';
import { MessagePlugin } from 'tdesign-vue-next';
import platformLogo from '@/assets/platform-logo.png';

const router = useRouter();
const route = useRoute();
const auth = useAuthStore();
const systemStore = useSystemStore();
const { t } = useI18n();

const email = ref('');
const password = ref('');
const loading = ref(false);

onMounted(() => {
  systemStore.fetchConfig();
});

async function handleLogin() {
  if (!email.value || !password.value) return;
  loading.value = true;
  try {
    await auth.login(email.value, password.value);
    const redirect = (route.query.redirect as string) || '/';
    router.push(redirect);
  } catch (e: any) {
    MessagePlugin.error(e.message || t('auth.loginFailed'));
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="auth-page">
    <!-- Left brand area -->
    <div class="brand-area">
      <div class="brand-logo">
        <img :src="platformLogo" alt="Ordo" class="brand-logo-img" />
        <span class="brand-name">Ordo</span>
      </div>

      <div class="brand-copy">
        <h1 class="brand-headline">{{ t('auth.subtitle') }}</h1>
        <p class="brand-tagline">{{ t('auth.tagline') }}</p>
      </div>

      <ul class="brand-features">
        <li>
          <svg class="feat-check" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="8" fill="#22c55e" fill-opacity="0.15" />
            <path
              d="M4.5 8l2.5 2.5 4.5-4.5"
              stroke="#16a34a"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          {{ t('auth.feature1') }}
        </li>
        <li>
          <svg class="feat-check" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="8" fill="#22c55e" fill-opacity="0.15" />
            <path
              d="M4.5 8l2.5 2.5 4.5-4.5"
              stroke="#16a34a"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          {{ t('auth.feature2') }}
        </li>
        <li>
          <svg class="feat-check" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="8" fill="#22c55e" fill-opacity="0.15" />
            <path
              d="M4.5 8l2.5 2.5 4.5-4.5"
              stroke="#16a34a"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          {{ t('auth.feature3') }}
        </li>
      </ul>
    </div>

    <!-- Right: floating form card -->
    <div class="form-card">
      <div class="form-header">
        <h2 class="form-title">{{ t('auth.loginBtn') }}</h2>
      </div>

      <div class="form-fields">
        <div class="field">
          <label class="field-label">{{ t('auth.emailLabel') }}</label>
          <t-input
            v-model="email"
            placeholder="you@company.com"
            type="email"
            autocomplete="email"
            size="large"
            clearable
          />
        </div>

        <div class="field">
          <div class="field-label-row">
            <label class="field-label">{{ t('auth.passwordLabel') }}</label>
            <router-link to="/forgot-password" class="forgot-link">{{
              t('auth.forgotPassword')
            }}</router-link>
          </div>
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

      <div class="divider">
        <span class="divider-text">{{ t('auth.orLoginWith') }}</span>
      </div>

      <div class="alt-login">
        <button class="alt-btn" @click="MessagePlugin.info(t('auth.ssoNotConfigured'))">
          <!-- GitHub mark -->
          <svg viewBox="0 0 24 24" width="18" height="18" fill="#1a1714">
            <path
              d="M12 2C6.477 2 2 6.477 2 12c0 4.418 2.865 8.166 6.839 9.489.5.092.682-.217.682-.482 0-.237-.008-.866-.013-1.7-2.782.603-3.369-1.34-3.369-1.34-.454-1.155-1.11-1.462-1.11-1.462-.908-.62.069-.608.069-.608 1.003.07 1.531 1.03 1.531 1.03.892 1.529 2.341 1.087 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.11-4.555-4.943 0-1.091.39-1.984 1.029-2.683-.103-.253-.446-1.27.098-2.647 0 0 .84-.269 2.75 1.025A9.578 9.578 0 0 1 12 6.836c.85.004 1.705.114 2.504.336 1.909-1.294 2.747-1.025 2.747-1.025.546 1.377.202 2.394.1 2.647.64.699 1.028 1.592 1.028 2.683 0 3.842-2.339 4.687-4.566 4.935.359.309.678.919.678 1.852 0 1.336-.012 2.415-.012 2.743 0 .267.18.578.688.48C19.138 20.163 22 16.418 22 12c0-5.523-4.477-10-10-10z"
            />
          </svg>
          GitHub
        </button>
        <button class="alt-btn" @click="MessagePlugin.info(t('auth.ssoNotConfigured'))">
          <!-- Google G logo (official colors) -->
          <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
            <path
              d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
              fill="#4285F4"
            />
            <path
              d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
              fill="#34A853"
            />
            <path
              d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l3.66-2.84z"
              fill="#FBBC05"
            />
            <path
              d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
              fill="#EA4335"
            />
          </svg>
          Google
        </button>
      </div>

      <div v-if="systemStore.allowRegistration" class="form-footer">
        {{ t('auth.noAccount') }}
        <router-link to="/register" class="form-link">{{ t('auth.registerLink') }}</router-link>
      </div>
    </div>
  </div>
</template>

<style scoped>
.auth-page {
  min-height: 100vh;
  background: #f2f0ea;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 48px 64px;
  gap: 80px;
}

/* ── Left brand area ──────────────────────────────────────────────────────── */
.brand-area {
  flex: 1;
  max-width: 460px;
  display: flex;
  flex-direction: column;
  gap: 36px;
}

.brand-logo {
  display: flex;
  align-items: center;
  gap: 10px;
}

.brand-logo-img {
  width: 32px;
  height: 32px;
  object-fit: contain;
}

.brand-name {
  font-size: 20px;
  font-weight: 700;
  color: #1a1714;
  letter-spacing: -0.3px;
}

.brand-copy {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.brand-headline {
  font-size: 32px;
  font-weight: 700;
  color: #1a1714;
  line-height: 1.2;
  margin: 0;
}

.brand-tagline {
  font-size: 15px;
  color: #5a534a;
  line-height: 1.7;
  margin: 0;
}

.brand-features {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.brand-features li {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  color: #3d3830;
  line-height: 1.5;
}

.feat-check {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
}

/* ── Right form card ──────────────────────────────────────────────────────── */
.form-card {
  width: 400px;
  flex-shrink: 0;
  background: #ffffff;
  border-radius: 16px;
  box-shadow:
    0 4px 6px rgba(0, 0, 0, 0.04),
    0 10px 40px rgba(0, 0, 0, 0.08);
  padding: 40px;
  display: flex;
  flex-direction: column;
  gap: 28px;
}

.form-header {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.form-title {
  font-size: 22px;
  font-weight: 700;
  color: #0f172a;
  margin: 0;
}

.form-sub {
  font-size: 13px;
  color: #8a8178;
  margin: 0;
}

.form-fields {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field-label {
  font-size: 13px;
  font-weight: 500;
  color: #374151;
}

.field-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.forgot-link {
  font-size: 12px;
  color: var(--ordo-accent);
  text-decoration: none;
  font-weight: 500;
}

.forgot-link:hover {
  text-decoration: underline;
}

.submit-btn {
  margin-top: 6px;
}

.divider {
  display: flex;
  align-items: center;
  gap: 12px;
  color: #c4bfb7;
  font-size: 12px;
}

.divider::before,
.divider::after {
  content: '';
  flex: 1;
  height: 1px;
  background: #e8e4dc;
}

.divider-text {
  white-space: nowrap;
  color: #8a8178;
}

.alt-login {
  display: flex;
  gap: 10px;
}

.alt-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 42px;
  border: 1px solid #e4e1d9;
  border-radius: 10px;
  background: #ffffff;
  color: #1a1714;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition:
    border-color 0.15s,
    box-shadow 0.15s,
    background 0.15s;
}

.alt-btn:hover {
  border-color: #bfbab1;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.07);
  background: #fafaf9;
}

.form-footer {
  text-align: center;
  font-size: 13px;
  color: #8a8178;
  padding-top: 4px;
  border-top: 1px solid #f0ede6;
}

.form-link {
  color: var(--ordo-accent);
  text-decoration: none;
  font-weight: 500;
  margin-left: 4px;
}

.form-link:hover {
  text-decoration: underline;
}

/* ── Responsive ─────────────────────────────────────────────────────────── */
@media (max-width: 860px) {
  .auth-page {
    flex-direction: column;
    padding: 40px 24px;
    gap: 40px;
    justify-content: flex-start;
    padding-top: 60px;
  }

  .brand-area {
    max-width: 100%;
    gap: 16px;
  }

  .brand-headline {
    font-size: 24px;
  }

  .brand-features {
    display: none;
  }

  .form-card {
    width: 100%;
    max-width: 420px;
    padding: 32px 28px;
  }
}
</style>
