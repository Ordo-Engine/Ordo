<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next'
import type { Role } from '@/api/types'

const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const { t, locale } = useI18n()

const orgId = computed(() => route.params.orgId as string)

const showInvite = ref(false)
const inviting = ref(false)
const inviteEmail = ref('')
const inviteRole = ref<Role>('editor')

const isAdmin = computed(() => {
  if (!auth.user) return false
  return orgStore.canAdmin(auth.user.id)
})

const roleOptions = [
  { label: 'Owner', value: 'owner' },
  { label: 'Admin', value: 'admin' },
  { label: 'Editor', value: 'editor' },
  { label: 'Viewer', value: 'viewer' },
]

const roleLabel: Record<Role, string> = {
  owner: 'Owner',
  admin: 'Admin',
  editor: 'Editor',
  viewer: 'Viewer',
}

const roleTheme: Record<Role, string> = {
  owner: 'primary',
  admin: 'warning',
  editor: 'success',
  viewer: 'default',
}

const columns = computed(() => {
  const cols: any[] = [
    { colKey: 'display_name', title: t('member.colName'), minWidth: 160 },
    { colKey: 'email', title: t('member.colEmail'), minWidth: 200 },
    { colKey: 'role', title: t('member.colRole'), width: 120 },
    { colKey: 'invited_at', title: t('member.colJoinedAt'), width: 160 },
  ]
  if (isAdmin.value) {
    cols.push({ colKey: 'actions', title: t('member.colActions'), width: 120, fixed: 'right' })
  }
  return cols
})

onMounted(async () => {
  if (!orgStore.currentOrg || orgStore.currentOrg.id !== orgId.value) {
    await orgStore.selectOrg(orgId.value)
  }
})

async function handleInvite() {
  if (!inviteEmail.value.trim()) {
    MessagePlugin.warning(t('member.emailRequired'))
    return
  }
  inviting.value = true
  try {
    await orgStore.inviteMember(orgId.value, inviteEmail.value.trim(), inviteRole.value)
    showInvite.value = false
    inviteEmail.value = ''
    inviteRole.value = 'editor'
    MessagePlugin.success(t('member.inviteSuccess'))
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    inviting.value = false
  }
}

async function handleRoleChange(userId: string, role: Role) {
  try {
    await orgStore.updateMemberRole(orgId.value, userId, role)
    MessagePlugin.success(t('member.roleUpdated'))
  } catch (e: any) {
    MessagePlugin.error(e.message)
  }
}

function handleRemove(userId: string, displayName: string) {
  const dlg = DialogPlugin.confirm({
    header: t('member.removeDialog'),
    body: t('member.removeConfirm', { name: displayName }),
    confirmBtn: { content: t('member.removeConfirmBtn'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await orgStore.removeMember(orgId.value, userId)
        dlg.hide()
        MessagePlugin.success(t('member.removeSuccess'))
      } catch (err: any) {
        MessagePlugin.error(err.message)
      }
    },
  })
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(locale.value === 'zh-TW' ? 'zh-TW' : locale.value === 'zh-CN' ? 'zh-CN' : 'en-US', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  })
}
</script>

<template>
  <div class="view-page">
    <t-breadcrumb class="page-breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{ t('breadcrumb.home') }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push('/orgs')">{{ t('breadcrumb.orgs') }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ orgStore.currentOrg?.name }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('breadcrumb.members') }}</t-breadcrumb-item>
    </t-breadcrumb>
    <div class="page-header">
      <div>
        <h2 class="page-title">{{ t('member.title') }}</h2>
        <p class="page-subtitle">
          {{ t('member.subtitle', { org: orgStore.currentOrg?.name ?? '', count: orgStore.members.length }) }}
        </p>
      </div>
      <t-button v-if="isAdmin" theme="primary" @click="showInvite = true">
        <t-icon name="add" />
        {{ t('member.invite') }}
      </t-button>
    </div>

    <t-table
      :data="orgStore.members"
      :columns="columns"
      row-key="user_id"
      hover
      size="medium"
      :loading="orgStore.loading"
      :empty="t('member.empty')"
    >
      <template #display_name="{ row }">
        <div class="member-cell">
          <div class="member-avatar">{{ row.display_name?.[0]?.toUpperCase() ?? '?' }}</div>
          <span class="member-name">{{ row.display_name }}</span>
          <t-tag v-if="row.user_id === auth.user?.id" size="small" theme="primary" variant="light">
            {{ t('member.me') }}
          </t-tag>
        </div>
      </template>

      <template #role="{ row }">
        <t-select
          v-if="isAdmin && row.user_id !== auth.user?.id && row.role !== 'owner'"
          :value="row.role"
          :options="roleOptions.filter(o => o.value !== 'owner')"
          size="small"
          @change="(v: any) => handleRoleChange(row.user_id, v)"
        />
        <t-tag v-else :theme="roleTheme[row.role as Role]" variant="light" size="small">
          {{ roleLabel[row.role as Role] }}
        </t-tag>
      </template>

      <template #invited_at="{ row }">
        <span class="text-secondary">{{ formatDate(row.invited_at) }}</span>
      </template>

      <template #actions="{ row }">
        <t-button
          v-if="row.user_id !== auth.user?.id && row.role !== 'owner'"
          variant="text"
          theme="danger"
          size="small"
          @click="handleRemove(row.user_id, row.display_name)"
        >
          {{ t('member.remove') }}
        </t-button>
      </template>
    </t-table>

    <!-- Invite dialog -->
    <t-dialog
      v-model:visible="showInvite"
      :header="t('member.inviteDialog')"
      :confirm-btn="{ content: t('member.invite'), loading: inviting }"
      @confirm="handleInvite"
      @close="showInvite = false"
      width="440px"
    >
      <t-form label-align="top">
        <t-form-item :label="t('member.emailLabel')" required>
          <t-input
            v-model="inviteEmail"
            :placeholder="t('member.emailPlaceholder')"
            type="email"
            autofocus
            @keyup.enter="handleInvite"
          />
        </t-form-item>
        <t-form-item :label="t('member.roleLabel')">
          <t-select v-model="inviteRole" :options="roleOptions.filter(o => o.value !== 'owner')" />
        </t-form-item>
      </t-form>
    </t-dialog>
  </div>
</template>

<style scoped>
.view-page {
  padding: 32px;
  height: 100%;
  overflow-y: auto;
}

.page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 24px;
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

.member-cell {
  display: flex;
  align-items: center;
  gap: 10px;
}

.member-avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--ordo-accent-bg);
  color: var(--ordo-accent);
  font-size: 12px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.member-name {
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.text-secondary {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}
</style>
