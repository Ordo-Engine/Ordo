<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useAuthStore } from '@/stores/auth';
import { useOrgStore } from '@/stores/org';
import { memberApi } from '@/api/platform-client';
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next';
import type { Role } from '@/api/types';

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const orgStore = useOrgStore();
const { t, locale } = useI18n();

// Sub-org management
const showCreateSubOrg = ref(false);
const creatingSubOrg = ref(false);
const newSubOrgName = ref('');
const newSubOrgDesc = ref('');
const assignAdmin = ref(false);
const assignAdminUserId = ref('');
const assignAdminRole = ref<Role>('admin');

// Sub-org member panel
const expandedSubOrgId = ref<string | null>(null);
const showAddMemberDialog = ref(false);
const addMemberTargetSubOrgId = ref('');
const addMemberUserId = ref('');
const addMemberRole = ref<Role>('editor');
const addingMember = ref(false);

const isRootOrg = computed(() => (orgStore.currentOrg?.depth ?? 0) === 0);
const parentOrgId = computed(() => orgStore.currentOrg?.parent_org_id ?? null);
const parentOrg = computed(() =>
  parentOrgId.value ? orgStore.orgs.find((o) => o.id === parentOrgId.value) ?? null : null
);
const currentSubOrgs = computed(() => (orgId.value ? orgStore.subOrgs[orgId.value] ?? [] : []));

// Parent org members not yet in the target sub-org (for "Add from parent" dropdown)
const parentMembersNotInSubOrg = computed(() => {
  const subMembers = new Set(
    (orgStore.subOrgMembers[addMemberTargetSubOrgId.value] ?? []).map((m) => m.user_id)
  );
  return orgStore.members.filter((m) => !subMembers.has(m.user_id));
});

// For "designate admin on create": exclude the current user (they become Owner automatically)
const parentMembersForAssign = computed(() =>
  orgStore.members.filter((m) => m.user_id !== auth.user?.id)
);

const orgId = computed(() => route.params.orgId as string);
const saving = ref(false);
const deleting = ref(false);
const leaving = ref(false);

const name = ref('');
const description = ref('');

const isAdmin = computed(() => {
  if (!auth.user) return false;
  return orgStore.canAdmin(auth.user.id);
});

const isMember = computed(() => {
  if (!auth.user) return false;
  return orgStore.currentOrg?.members.some((m) => m.user_id === auth.user!.id) ?? false;
});

const createdAtFormatted = computed(() => {
  const raw = orgStore.currentOrg?.created_at;
  if (!raw) return '—';
  return new Intl.DateTimeFormat(locale.value, {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  }).format(new Date(raw));
});

onMounted(async () => {
  if (!orgStore.currentOrg || orgStore.currentOrg.id !== orgId.value) {
    await orgStore.selectOrg(orgId.value);
  }
  name.value = orgStore.currentOrg?.name ?? '';
  description.value = orgStore.currentOrg?.description ?? '';
  // Pre-load sub-orgs if this is a root org
  if (isRootOrg.value) {
    await orgStore.fetchSubOrgs(orgId.value);
  }
});

async function handleCreateSubOrg() {
  if (!newSubOrgName.value.trim()) {
    MessagePlugin.warning(t('org.nameRequired'));
    return;
  }
  creatingSubOrg.value = true;
  try {
    const org = await orgStore.createSubOrg(
      orgId.value,
      newSubOrgName.value.trim(),
      newSubOrgDesc.value || undefined
    );
    // Optionally designate an admin from the parent org
    if (assignAdmin.value && assignAdminUserId.value) {
      try {
        await orgStore.addSubOrgMember(
          orgId.value,
          org.id,
          assignAdminUserId.value,
          assignAdminRole.value
        );
      } catch (e: any) {
        // If the user is already a member (e.g. was auto-added as creator), ignore
        if (!e.message?.toLowerCase().includes('already a member')) throw e;
      }
    }
    showCreateSubOrg.value = false;
    newSubOrgName.value = '';
    newSubOrgDesc.value = '';
    assignAdmin.value = false;
    assignAdminUserId.value = '';
    assignAdminRole.value = 'admin';
    MessagePlugin.success(t('org.createSubOrgSuccess'));
  } catch (e: any) {
    MessagePlugin.error(e.message);
  } finally {
    creatingSubOrg.value = false;
  }
}

async function toggleSubOrgMembers(subOrgId: string) {
  if (expandedSubOrgId.value === subOrgId) {
    expandedSubOrgId.value = null;
    return;
  }
  expandedSubOrgId.value = subOrgId;
  await orgStore.fetchSubOrgMembers(orgId.value, subOrgId);
}

function openAddMemberDialog(subOrgId: string) {
  addMemberTargetSubOrgId.value = subOrgId;
  addMemberUserId.value = '';
  addMemberRole.value = 'editor';
  showAddMemberDialog.value = true;
}

async function handleAddSubOrgMember() {
  if (!addMemberUserId.value) {
    MessagePlugin.warning(t('org.subOrgMember.selectRequired'));
    return;
  }
  addingMember.value = true;
  try {
    await orgStore.addSubOrgMember(
      orgId.value,
      addMemberTargetSubOrgId.value,
      addMemberUserId.value,
      addMemberRole.value
    );
    showAddMemberDialog.value = false;
    MessagePlugin.success(t('org.subOrgMember.addSuccess'));
  } catch (e: any) {
    MessagePlugin.error(e.message);
  } finally {
    addingMember.value = false;
  }
}

function handleRemoveSubOrgMember(subOrgId: string, userId: string, userName: string) {
  const dlg = DialogPlugin.confirm({
    header: t('org.subOrgMember.removeTitle'),
    body: t('org.subOrgMember.removeConfirm', { name: userName }),
    confirmBtn: { content: t('common.remove'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await orgStore.removeSubOrgMember(orgId.value, subOrgId, userId);
        dlg.hide();
        MessagePlugin.success(t('org.subOrgMember.removeSuccess'));
      } catch (e: any) {
        MessagePlugin.error(e.message);
      }
    },
  });
}

function handleDeleteSubOrg(subOrgId: string, subOrgName: string) {
  const dlg = DialogPlugin.confirm({
    header: t('org.deleteSubOrg'),
    body: t('org.deleteSubOrgConfirm', { name: subOrgName }),
    confirmBtn: { content: t('org.settings.deleteBtn'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await orgStore.deleteSubOrg(subOrgId, orgId.value);
        dlg.hide();
        MessagePlugin.success(t('org.deleteSubOrgSuccess'));
      } catch (e: any) {
        MessagePlugin.error(e.message);
      }
    },
  });
}

async function handleSave() {
  if (!name.value.trim()) {
    MessagePlugin.warning(t('org.settings.nameRequired'));
    return;
  }
  saving.value = true;
  try {
    await orgStore.updateOrg(orgId.value, {
      name: name.value.trim(),
      description: description.value || undefined,
    });
    MessagePlugin.success(t('org.settings.saveSuccess'));
  } catch (e: any) {
    MessagePlugin.error(e.message);
  } finally {
    saving.value = false;
  }
}

function copyOrgId() {
  navigator.clipboard.writeText(orgId.value).then(() => {
    MessagePlugin.success(t('org.settings.orgIdCopied'));
  });
}

function handleLeave() {
  if (!auth.token || !auth.user?.id) return;
  const token = auth.token;
  const userId = auth.user.id;
  const orgName = orgStore.currentOrg?.name ?? '';
  const dialog = DialogPlugin.confirm({
    header: t('org.settings.leaveOrgDialog'),
    body: t('org.settings.leaveOrgConfirm', { name: orgName }),
    confirmBtn: { content: t('org.settings.leaveOrgBtn'), theme: 'danger', loading: leaving.value },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      leaving.value = true;
      try {
        await memberApi.remove(token, orgId.value, userId);
        dialog.hide();
        MessagePlugin.success(t('org.settings.leaveOrgSuccess'));
        router.push('/orgs');
      } catch (e: any) {
        MessagePlugin.error(e.message);
      } finally {
        leaving.value = false;
      }
    },
  });
}

const roleTheme: Record<string, string> = {
  owner: 'primary',
  admin: 'warning',
  editor: 'success',
  viewer: 'default',
};

function handleDelete() {
  const orgName = orgStore.currentOrg?.name ?? '';
  const dialog = DialogPlugin.confirm({
    header: t('org.settings.deleteDialog'),
    body: t('org.settings.deleteConfirm', { name: orgName }),
    confirmBtn: { content: t('org.settings.deleteBtn'), theme: 'danger', loading: deleting.value },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      deleting.value = true;
      try {
        await orgStore.deleteOrg(orgId.value);
        dialog.hide();
        MessagePlugin.success(t('org.settings.deleteSuccess'));
        router.push('/orgs');
      } catch (e: any) {
        MessagePlugin.error(e.message);
      } finally {
        deleting.value = false;
      }
    },
  });
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

        <!-- Parent org info (sub-orgs only) -->
        <section v-if="!isRootOrg && parentOrg" class="settings-section">
          <h3 class="section-title">{{ t('org.parentOrg') }}</h3>
          <div class="parent-org-row" @click="router.push(`/orgs/${parentOrg.id}/settings`)">
            <div class="org-mini-icon">{{ parentOrg.name[0]?.toUpperCase() }}</div>
            <div class="parent-org-info">
              <div class="parent-org-name">{{ parentOrg.name }}</div>
              <div class="parent-org-meta">
                {{ t('org.memberCount', { count: parentOrg.member_count }) }}
              </div>
            </div>
            <t-icon name="chevron-right" class="parent-org-arrow" />
          </div>
        </section>

        <!-- Sub-organizations (root orgs only) -->
        <section v-if="isRootOrg" class="settings-section">
          <div class="section-header-row">
            <div>
              <h3 class="section-title">{{ t('org.subOrgs') }}</h3>
              <p class="section-desc">{{ t('org.subOrgsDesc') }}</p>
            </div>
            <t-button
              v-if="isAdmin"
              size="small"
              variant="outline"
              @click="showCreateSubOrg = true"
            >
              <t-icon name="add" />
              {{ t('org.addSubOrg') }}
            </t-button>
          </div>

          <div v-if="currentSubOrgs.length === 0" class="suborg-empty">
            {{ t('org.noSubOrgs') }}
          </div>
          <div v-else class="suborg-list">
            <template v-for="sub in currentSubOrgs" :key="sub.id">
              <div class="suborg-row">
                <div class="org-mini-icon org-mini-icon--sub">{{ sub.name[0]?.toUpperCase() }}</div>
                <div class="suborg-info">
                  <div class="suborg-name">{{ sub.name }}</div>
                  <div class="suborg-meta">
                    {{ t('org.memberCount', { count: sub.member_count }) }}
                    <span v-if="sub.project_count > 0" class="suborg-meta-sep">·</span>
                    <span v-if="sub.project_count > 0">{{
                      t('org.projectCount', { count: sub.project_count })
                    }}</span>
                  </div>
                </div>
                <div class="suborg-actions">
                  <t-button
                    v-if="isAdmin"
                    size="small"
                    variant="text"
                    @click="toggleSubOrgMembers(sub.id)"
                  >
                    {{
                      expandedSubOrgId === sub.id
                        ? t('common.collapse')
                        : t('org.subOrgMember.manage')
                    }}
                  </t-button>
                  <t-button
                    size="small"
                    variant="text"
                    @click="router.push(`/orgs/${sub.id}/settings`)"
                  >
                    {{ t('org.manage') }}
                  </t-button>
                  <t-button
                    v-if="isAdmin"
                    size="small"
                    variant="text"
                    theme="danger"
                    @click="handleDeleteSubOrg(sub.id, sub.name)"
                  >
                    {{ t('common.delete') }}
                  </t-button>
                </div>
              </div>
              <!-- Expandable member panel -->
              <div v-if="expandedSubOrgId === sub.id" class="suborg-member-panel">
                <div class="suborg-member-panel__header">
                  <span>{{ t('org.subOrgMember.panelTitle', { name: sub.name }) }}</span>
                  <t-button
                    v-if="isAdmin"
                    size="small"
                    variant="outline"
                    @click="openAddMemberDialog(sub.id)"
                  >
                    <t-icon name="add" />
                    {{ t('org.subOrgMember.addFromParent') }}
                  </t-button>
                </div>
                <div v-if="!orgStore.subOrgMembers[sub.id]?.length" class="suborg-member-empty">
                  {{ t('org.subOrgMember.empty') }}
                </div>
                <div v-else class="suborg-member-list">
                  <div
                    v-for="m in orgStore.subOrgMembers[sub.id]"
                    :key="m.user_id"
                    class="suborg-member-row"
                  >
                    <span class="suborg-member-name">{{ m.display_name }}</span>
                    <t-tag :theme="roleTheme[m.role]" size="small">{{ m.role }}</t-tag>
                    <t-button
                      v-if="isAdmin"
                      size="small"
                      variant="text"
                      theme="danger"
                      @click="handleRemoveSubOrgMember(sub.id, m.user_id, m.display_name)"
                    >
                      {{ t('common.remove') }}
                    </t-button>
                  </div>
                </div>
              </div>
            </template>
          </div>
        </section>

        <!-- Create sub-org dialog -->
        <t-dialog
          v-model:visible="showCreateSubOrg"
          :header="t('org.createSubOrgDialog')"
          :confirm-btn="{ content: t('common.create'), loading: creatingSubOrg }"
          @confirm="handleCreateSubOrg"
          @close="showCreateSubOrg = false"
          width="440px"
        >
          <t-form label-align="top">
            <t-form-item :label="t('org.nameLabel')" required>
              <t-input
                v-model="newSubOrgName"
                :placeholder="t('org.namePlaceholder')"
                autofocus
                @keyup.enter="handleCreateSubOrg"
              />
            </t-form-item>
            <t-form-item :label="t('org.descLabel')">
              <t-input v-model="newSubOrgDesc" :placeholder="t('org.descPlaceholder')" />
            </t-form-item>
            <t-form-item :label="t('org.assignAdmin')">
              <t-switch v-model="assignAdmin" />
            </t-form-item>
            <t-form-item v-if="assignAdmin" :label="t('org.assignAdminMember')">
              <t-select
                v-model="assignAdminUserId"
                :placeholder="t('org.assignAdminPlaceholder')"
                clearable
              >
                <t-option
                  v-for="m in parentMembersForAssign"
                  :key="m.user_id"
                  :value="m.user_id"
                  :label="`${m.display_name} (${m.email})`"
                />
              </t-select>
            </t-form-item>
            <t-form-item v-if="assignAdmin" :label="t('org.subOrgMember.role')">
              <t-select v-model="assignAdminRole">
                <t-option value="admin" label="Admin" />
                <t-option value="editor" label="Editor" />
              </t-select>
            </t-form-item>
          </t-form>
        </t-dialog>

        <!-- Add member from parent org dialog -->
        <t-dialog
          v-model:visible="showAddMemberDialog"
          :header="t('org.subOrgMember.addDialogTitle')"
          :confirm-btn="{ content: t('common.add'), loading: addingMember }"
          @confirm="handleAddSubOrgMember"
          @close="showAddMemberDialog = false"
          width="400px"
        >
          <t-form label-align="top">
            <t-form-item :label="t('org.subOrgMember.selectMember')" required>
              <t-select
                v-model="addMemberUserId"
                :placeholder="t('org.subOrgMember.selectPlaceholder')"
                filterable
              >
                <t-option
                  v-for="m in parentMembersNotInSubOrg"
                  :key="m.user_id"
                  :value="m.user_id"
                  :label="`${m.display_name} (${m.email})`"
                />
              </t-select>
            </t-form-item>
            <t-form-item :label="t('org.subOrgMember.role')">
              <t-select v-model="addMemberRole">
                <t-option value="admin" label="Admin" />
                <t-option value="editor" label="Editor" />
                <t-option value="viewer" label="Viewer" />
              </t-select>
            </t-form-item>
          </t-form>
        </t-dialog>

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

/* ── Section header with action button ───────────────────────────────────── */
.section-header-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
}

.section-header-row .section-title {
  margin-bottom: 2px;
}

.section-desc {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  margin: 0;
}

/* ── Parent org row ───────────────────────────────────────────────────────── */
.parent-org-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md, 8px);
  cursor: pointer;
  transition: border-color 0.15s;
}

.parent-org-row:hover {
  border-color: var(--ordo-accent);
}

.parent-org-info {
  flex: 1;
}

.parent-org-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.parent-org-meta {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.parent-org-arrow {
  color: var(--ordo-text-tertiary);
}

/* ── Sub-org list ─────────────────────────────────────────────────────────── */
.suborg-empty {
  font-size: 13px;
  color: var(--ordo-text-secondary);
  padding: 16px 0 4px;
}

.suborg-list {
  display: flex;
  flex-direction: column;
  gap: 0;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md, 8px);
  overflow: hidden;
}

.suborg-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  transition: background 0.12s;
}

.suborg-row:last-child {
  border-bottom: none;
}

.suborg-row:hover {
  background: var(--ordo-bg-app);
}

.org-mini-icon {
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: var(--ordo-accent-bg);
  color: var(--ordo-accent);
  font-size: 13px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.org-mini-icon--sub {
  background: #f0fdf4;
  color: #16a34a;
}

.suborg-meta-sep {
  margin: 0 2px;
  color: var(--ordo-text-tertiary);
}

/* ── Sub-org member panel ─────────────────────────────────────────────────── */
.suborg-member-panel {
  background: var(--ordo-bg-app);
  border-bottom: 1px solid var(--ordo-border-color);
  padding: 12px 14px 14px 58px;
}

.suborg-member-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.suborg-member-panel__header span {
  font-size: 12px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
}

.suborg-member-empty {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.suborg-member-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.suborg-member-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.suborg-member-name {
  flex: 1;
  font-size: 13px;
  color: var(--ordo-text-primary);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.suborg-info {
  flex: 1;
  min-width: 0;
}

.suborg-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.suborg-meta {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.suborg-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

@media (max-width: 900px) {
  .view-page {
    padding: 20px;
  }

  .danger-item {
    flex-direction: column;
    align-items: flex-start;
  }

  .section-header-row {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
