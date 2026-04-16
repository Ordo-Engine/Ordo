<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { MessagePlugin } from 'tdesign-vue-next'
import { useAuthStore } from '@/stores/auth'
import { usePreferencesStore } from '@/stores/preferences'
import { setLocale, LOCALE_OPTIONS, i18n, type Locale } from '@/i18n'

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const prefs = usePreferencesStore()

// Profile form
const displayName = ref(auth.user?.display_name ?? '')
const savingProfile = ref(false)

async function saveProfile() {
  if (!displayName.value.trim()) return
  savingProfile.value = true
  try {
    await auth.updateProfile({ display_name: displayName.value.trim() })
    MessagePlugin.success(t('settings.profileSaved'))
  } catch {
    MessagePlugin.error(t('settings.profileSaveFailed'))
  } finally {
    savingProfile.value = false
  }
}

// Password form
const currentPassword = ref('')
const newPassword = ref('')
const confirmNewPassword = ref('')
const changingPassword = ref(false)

async function changePassword() {
  if (newPassword.value !== confirmNewPassword.value) {
    MessagePlugin.warning(t('settings.passwordMismatch'))
    return
  }
  changingPassword.value = true
  try {
    await auth.changePassword(currentPassword.value, newPassword.value)
    MessagePlugin.success(t('settings.passwordChanged'))
    currentPassword.value = ''
    newPassword.value = ''
    confirmNewPassword.value = ''
  } catch (e: any) {
    MessagePlugin.error(e.message || t('settings.passwordChangeFailed'))
  } finally {
    changingPassword.value = false
  }
}

// Appearance
const currentLocale = ref((i18n.global.locale as any).value as Locale)

function onLocaleChange(val: Locale) {
  currentLocale.value = val
  setLocale(val)
}
</script>

<template>
  <div class="settings-page">
    <!-- Breadcrumb -->
    <t-breadcrumb class="breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{ t('breadcrumb.home') }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('settings.title') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <h1 class="page-title">{{ t('settings.title') }}</h1>

    <div class="settings-layout">
      <!-- Profile -->
      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('settings.profile') }}</h2>
        <t-form label-align="top" :colon="false" class="settings-form">
          <t-form-item :label="t('settings.displayNameLabel')">
            <t-input
              v-model="displayName"
              :placeholder="t('settings.displayNamePlaceholder')"
            />
          </t-form-item>
          <t-form-item :label="t('settings.emailLabel')">
            <t-input :value="auth.user?.email" disabled />
          </t-form-item>
          <t-form-item>
            <t-button
              theme="primary"
              :loading="savingProfile"
              @click="saveProfile"
            >
              {{ t('settings.saveProfile') }}
            </t-button>
          </t-form-item>
        </t-form>
      </t-card>

      <!-- Change password -->
      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('settings.changePassword') }}</h2>
        <t-form label-align="top" :colon="false" class="settings-form">
          <t-form-item :label="t('settings.currentPassword')">
            <t-input
              v-model="currentPassword"
              type="password"
              :placeholder="t('settings.currentPasswordPlaceholder')"
            />
          </t-form-item>
          <t-form-item :label="t('settings.newPassword')">
            <t-input
              v-model="newPassword"
              type="password"
              :placeholder="t('settings.newPasswordPlaceholder')"
            />
          </t-form-item>
          <t-form-item :label="t('settings.confirmNewPassword')">
            <t-input
              v-model="confirmNewPassword"
              type="password"
              :placeholder="t('settings.confirmNewPasswordPlaceholder')"
            />
          </t-form-item>
          <t-form-item>
            <t-button
              theme="primary"
              :loading="changingPassword"
              :disabled="!currentPassword || !newPassword || !confirmNewPassword"
              @click="changePassword"
            >
              {{ t('settings.changePasswordBtn') }}
            </t-button>
          </t-form-item>
        </t-form>
      </t-card>

      <!-- Appearance -->
      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('settings.appearance') }}</h2>
        <t-form label-align="top" :colon="false" class="settings-form">
          <t-form-item :label="t('settings.themeLabel')">
            <t-radio-group :value="prefs.theme" @change="(v: any) => prefs.setTheme(v)">
              <t-radio value="light">{{ t('settings.themeLight') }}</t-radio>
              <t-radio value="dark">{{ t('settings.themeDark') }}</t-radio>
              <t-radio value="system">{{ t('settings.themeSystem') }}</t-radio>
            </t-radio-group>
          </t-form-item>
          <t-form-item :label="t('settings.languageLabel')">
            <t-select
              :value="currentLocale"
              style="width: 200px"
              @change="(v: any) => onLocaleChange(v)"
            >
              <t-option
                v-for="opt in LOCALE_OPTIONS"
                :key="opt.value"
                :value="opt.value"
                :label="opt.label"
              />
            </t-select>
          </t-form-item>
        </t-form>
      </t-card>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  padding: 32px;
  overflow-y: auto;
  height: 100%;
}

.breadcrumb {
  margin-bottom: 16px;
}

.page-title {
  margin: 0 0 24px;
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.settings-layout {
  display: flex;
  flex-direction: column;
  gap: 20px;
  max-width: 600px;
}

.settings-card {
  background: var(--ordo-bg-panel);
  border-radius: 8px;
}

.card-title {
  margin: 0 0 20px;
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.settings-form {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
</style>
