<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next'

const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const { t } = useI18n()

const orgId = computed(() => route.params.orgId as string)
const saving = ref(false)
const deleting = ref(false)

const name = ref('')
const description = ref('')

const isAdmin = computed(() => {
  if (!auth.user) return false
  return orgStore.canAdmin(auth.user.id)
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
    <t-breadcrumb class="page-breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{ t('breadcrumb.home') }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push('/orgs')">{{ t('breadcrumb.orgs') }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ orgStore.currentOrg?.name }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('breadcrumb.orgSettings') }}</t-breadcrumb-item>
    </t-breadcrumb>
    <div class="page-header">
      <div>
        <h2 class="page-title">{{ t('org.settings.title') }}</h2>
        <p class="page-subtitle">{{ orgStore.currentOrg?.name }}</p>
      </div>
    </div>

    <div class="settings-body">
      <!-- General -->
      <section class="settings-section">
        <h3 class="section-title">{{ t('org.settings.general') }}</h3>
        <t-form label-align="top" class="settings-form">
          <t-form-item :label="t('org.settings.nameLabel')">
            <t-input v-model="name" :placeholder="t('org.settings.nameLabel')" :disabled="!isAdmin" />
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
            <t-button theme="primary" :loading="saving" @click="handleSave">{{ t('common.save') }}</t-button>
          </t-form-item>
        </t-form>
      </section>

      <!-- Members quick link -->
      <section class="settings-section">
        <h3 class="section-title">{{ t('org.settings.members') }}</h3>
        <p class="section-desc">{{ t('org.settings.membersDesc') }}</p>
        <t-button variant="outline" @click="router.push(`/orgs/${orgId}/members`)">
          <t-icon name="usergroup" />
          {{ t('org.settings.viewMembers') }}
        </t-button>
      </section>

      <!-- Danger zone -->
      <section class="settings-section settings-section--danger" v-if="isAdmin">
        <h3 class="section-title section-title--danger">{{ t('org.settings.danger') }}</h3>
        <div class="danger-item">
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
</template>

<style scoped>
.view-page {
  padding: 32px;
  height: 100%;
  overflow-y: auto;
  max-width: 680px;
}

.page-header {
  margin-bottom: 32px;
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
  gap: 32px;
}

.settings-section {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  padding: 24px;
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

.section-desc {
  font-size: 13px;
  color: var(--ordo-text-secondary);
  margin: 0 0 16px;
}

.settings-form {
  max-width: 480px;
}

.danger-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
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
</style>
