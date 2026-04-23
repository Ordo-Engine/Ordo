<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useAuthStore } from '@/stores/auth';
import { useSystemStore } from '@/stores/system';
import { MessagePlugin } from 'tdesign-vue-next';
import platformLogo from '@/assets/platform-logo.png';

const router = useRouter();
const auth = useAuthStore();
const systemStore = useSystemStore();
const { t } = useI18n();

const displayName = ref('');
const email = ref('');
const password = ref('');
const password2 = ref('');
const loading = ref(false);

onMounted(() => {
  systemStore.fetchConfig();
});

async function handleRegister() {
  if (!email.value || !password.value || !displayName.value) {
    MessagePlugin.warning(t('auth.fillRequired'));
    return;
  }
  if (password.value !== password2.value) {
    MessagePlugin.warning(t('auth.passwordMismatch'));
    return;
  }
  if (password.value.length < 8) {
    MessagePlugin.warning(t('auth.passwordTooShort'));
    return;
  }
  loading.value = true;
  try {
    await auth.register(email.value, password.value, displayName.value);
    router.push('/');
  } catch (e: any) {
    MessagePlugin.error(e.message || t('auth.registerFailed'));
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
      <!-- Loading skeleton -->
      <div v-if="systemStore.loading" class="loading-state">
        <t-skeleton
          theme="paragraph"
          animation="gradient"
          :row-col="[{ width: '40%' }, { width: '100%' }, { width: '100%' }, { width: '100%' }]"
        />
      </div>

      <!-- Registration disabled -->
      <template v-else-if="!systemStore.allowRegistration">
        <div class="disabled-header">
          <div class="disabled-icon">
            <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
              <path
                d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"
                fill="currentColor"
              />
            </svg>
          </div>
          <h2 class="form-title">{{ t('auth.registrationDisabled') }}</h2>
        </div>
        <p class="disabled-desc">{{ t('auth.registrationDisabledDesc') }}</p>
        <router-link to="/login" class="back-btn"> ← {{ t('auth.backToLogin') }} </router-link>
      </template>

      <!-- Registration form -->
      <template v-else>
        <div class="form-header">
          <h2 class="form-title">{{ t('auth.createAccount') }}</h2>
        </div>

        <div class="form-fields">
          <div class="field">
            <label class="field-label">{{ t('auth.displayNameLabel') }}</label>
            <t-input
              v-model="displayName"
              :placeholder="t('auth.displayNamePlaceholder')"
              size="large"
              clearable
            />
          </div>
          <div class="field">
            <label class="field-label">{{ t('auth.emailLabel') }}</label>
            <t-input
              v-model="email"
              placeholder="you@company.com"
              type="email"
              size="large"
              clearable
            />
          </div>
          <div class="field">
            <label class="field-label">{{ t('auth.passwordLabel') }}</label>
            <t-input
              v-model="password"
              :placeholder="t('auth.passwordMinPlaceholder')"
              type="password"
              size="large"
            />
          </div>
          <div class="field">
            <label class="field-label">{{ t('auth.confirmPasswordLabel') }}</label>
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

        <div class="form-footer">
          {{ t('auth.hasAccount') }}
          <router-link to="/login" class="form-link">{{ t('auth.loginLink') }}</router-link>
        </div>
      </template>
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

.loading-state {
  padding: 8px 0;
}

/* Disabled state */
.disabled-header {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.disabled-icon {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  background: #fef3c7;
  color: #d97706;
  display: flex;
  align-items: center;
  justify-content: center;
}

.disabled-desc {
  font-size: 14px;
  color: #5a534a;
  line-height: 1.65;
  margin: 0;
}

.back-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 500;
  color: var(--ordo-accent);
  text-decoration: none;
}

.back-btn:hover {
  text-decoration: underline;
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

.submit-btn {
  margin-top: 6px;
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
