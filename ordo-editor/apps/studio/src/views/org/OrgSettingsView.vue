<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { memberApi } from '@/api/platform-client'
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next'

const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const { t, locale } = useI18n()

const orgId = computed(() => route.params.orgId as string)
const saving = ref(false)
const deleting = ref(false)
const leaving = ref(false)

const name = ref('')
const description = ref('')

const isAdmin = computed(() => {
  if (!auth.user) return false
  return orgStore.canAdmin(auth.user.id)
})

const isMember = computed(() => {
  if (!auth.user) return false
  return orgStore.currentOrg?.members.some((m) => m.user_id === auth.user!.id) ?? false
})

const createdAtFormatted = computed(() => {
  const raw = orgStore.currentOrg?.created_at
  if (!raw) return '—'
  return new Intl.DateTimeFormat(locale.value, {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  }).format(new Date(raw))
})

onMounted(async () => {
  if (!orgStore.currentOrg || orgStore.currentOrg.id !== orgId.value) {
    await orgStore.selectOrg(orgId.value)
  }
  name.value = orgStore.currentOrg?.name ?? ''
  description.value = orgStore.currentOrg?.description ?? ''
})

async function handleSave() {
  if (!name.value.trim()) {
    MessagePlugin.warning(t('org.settings.nameRequired'))
    return
  }
  saving.value = true
  try {
    await orgStore.updateOrg(orgId.value, {
      name: name.value.trim(),
      description: description.value || undefined,
    })
    MessagePlugin.success(t('org.settings.saveSuccess'))
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    saving.value = false
  }
}

function copyOrgId() {
  navigator.clipboard.writeText(orgId.value).then(() => {
    MessagePlugin.success(t('org.settings.orgIdCopied'))
  })
}

function handleLeave() {
  const orgName = orgStore.currentOrg?.name ?? ''
  const dialog = DialogPlugin.confirm({
    header: t('org.settings.leaveOrgDialog'),
    body: t('org.settings.leaveOrgConfirm', { name: orgName }),
    confirmBtn: { content: t('org.settings.leaveOrgBtn'), theme: 'danger', loading: leaving.value },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      leaving.value = true
      try {
        await memberApi.remove(auth.token, orgId.value, auth.user!.id)
        dialog.hide()
        MessagePlugin.success(t('org.settings.leaveOrgSuccess'))
        router.push('/orgs')
      } catch (e: any) {
        MessagePlugin.error(e.message)
      } finally {
        leaving.value = false
      }
    },
  })
}

function handleDelete() {
  const orgName = orgStore.currentOrg?.name ?? ''
  const dialog = DialogPlugin.confirm({
    header: t('org.settings.deleteDialog'),
    body: t('org.settings.deleteConfirm', { name: orgName }),
    confirmBtn: { content: t('org.settings.deleteBtn'), theme: 'danger', loading: deleting.value },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      deleting.value = true
      try {
        await orgStore.deleteOrg(orgId.value)
        dialog.hide()
        MessagePlugin.success(t('org.settings.deleteSuccess'))
        router.push('/orgs')
      } catch (e: any) {
        MessagePlugin.error(e.message)
      } finally {
        deleting.value = false
      }
    },
  })
}
</script>

<template>
  <div class="view-page">
    <div class="settings-shell">
      <div class="page-header">
        <h2 class="page-title">{{ t('org.settings.title') }}</h2>
        <p class="page-subtitle">{{ orgStore.currentOrg?.name }}</p>
      </div>

      <div class="settings-body">
        <!-- General -->
        <section class="settings-section">
          <h3 class="section-title">{{ t('org.settings.general') }}</h3>
          <t-form label-align="top" class="settings-form">
            <t-form-item :label="t('org.settings.nameLabel')">
              <t-input
                v-model="name"
                :placeholder="t('org.settings.nameLabel')"
                :disabled="!isAdmin"
              />
            </t-form-item>
            <t-form-item :label="t('org.settings.descLabel')">
              <t-textarea
                v-model="description"
                :placeholder="t('org.settings.descPlaceholder')"
                :autosize="{ minRows: 2, maxRows: 4 }"
                :disabled="!isAdmin"
              />
            </t-form-item>
            <t-form-item v-if="isAdmin">
              <t-button theme="primary" :loading="saving" @click="handleSave">
                {{ t('common.save') }}
              </t-button>
            </t-form-item>
          </t-form>
        </section>

        <!-- Org info -->
        <section class="settings-section">
          <h3 class="section-title">{{ t('org.settings.infoSection') }}</h3>
          <div class="info-grid">
            <div class="info-row">
              <span class="info-label">{{ t('org.settings.orgId') }}</span>
              <div class="info-value-row">
                <code class="info-code">{{ orgId }}</code>
                <t-tooltip :content="t('org.settings.orgIdCopied')" trigger="click" placement="top">
                  <t-button size="small" variant="text" @click="copyOrgId">
                    <t-icon name="file-copy" />
                  </t-button>
                </t-tooltip>
              </div>
              <p class="info-hint">{{ t('org.settings.orgIdDesc') }}</p>
            </div>
            <div class="info-row">
              <span class="info-label">{{ t('org.settings.createdAt') }}</span>
              <span class="info-value">{{ createdAtFormatted }}</span>
            </div>
          </div>
        </section>

        <!-- Danger zone -->
        <section class="settings-section settings-section--danger">
          <h3 class="section-title section-title--danger">{{ t('org.settings.danger') }}</h3>

          <!-- Leave org — for non-admin members -->
          <div v-if="!isAdmin && isMember" class="danger-item">
            <div>
              <div class="danger-item__label">{{ t('org.settings.leaveOrg') }}</div>
              <div class="danger-item__desc">{{ t('org.settings.leaveOrgDesc') }}</div>
            </div>
            <t-button theme="danger" variant="outline" :loading="leaving" @click="handleLeave">
              {{ t('org.settings.leaveOrgBtn') }}
            </t-button>
          </div>

          <!-- Delete org — for admins -->
          <div v-if="isAdmin" class="danger-item">
            <div>
              <div class="danger-item__label">{{ t('org.settings.deleteLabel') }}</div>
              <div class="danger-item__desc">{{ t('org.settings.deleteDesc') }}</div>
            </div>
            <t-button theme="danger" variant="outline" :loading="deleting" @click="handleDelete">
              {{ t('org.settings.deleteBtn') }}
            </t-button>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.view-page {
  padding: 32px;
  height: 100%;
  width: 100%;
  overflow-y: auto;
  box-sizing: border-box;
}

.settings-shell {
  max-width: 680px;
}

.page-header {
  margin-bottom: 28px;
}

.page-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin: 0 0 4px;
}

.page-subtitle {
  font-size: 13px;
  color: var(--ordo-text-secondary);
  margin: 0;
}

.settings-body {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.settings-section {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  padding: 20px 24px;
}

.settings-section--danger {
  border-color: var(--td-error-color, #e34d59);
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin: 0 0 16px;
}

.section-title--danger {
  color: var(--td-error-color, #e34d59);
}

.settings-form {
  max-width: 480px;
}

/* Info grid */
.info-grid {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.info-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.info-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
}

.info-value-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.info-code {
  font-family: var(--td-font-family-code, monospace);
  font-size: 13px;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-subtle, #f3f4f6);
  padding: 2px 8px;
  border-radius: 4px;
}

.info-value {
  font-size: 13px;
  color: var(--ordo-text-primary);
}

.info-hint {
  font-size: 12px;
  color: var(--ordo-text-tertiary, #9ca3af);
  margin: 0;
}

/* Danger items */
.danger-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.danger-item + .danger-item {
  margin-top: 16px;
  padding-top: 16px;
  border-top: 1px solid var(--ordo-border-color);
}

.danger-item__label {
  font-size: 14px;
  font-weight: 500;
  color: var(--ordo-text-primary);
  margin-bottom: 4px;
}

.danger-item__desc {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

@media (max-width: 900px) {
  .view-page {
    padding: 20px;
  }

  .danger-item {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
