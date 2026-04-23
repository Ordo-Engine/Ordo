<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter, useRoute } from 'vue-router';
import { useProjectStore } from '@/stores/project';
import { useOrgStore } from '@/stores/org';
import { useAuthStore } from '@/stores/auth';
import { rulesetDraftApi, rulesetHistoryApi } from '@/api/platform-client';
import { buildCheckpointVersionEntries, extractRulesetVersion } from '@/utils/ruleset-version';
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next';
import type { RulesetHistoryEntry } from '@/api/types';

const projectStore = useProjectStore();
const orgStore = useOrgStore();
const auth = useAuthStore();
const { t, locale } = useI18n();
const router = useRouter();
const route = useRoute();
const orgId = computed(() => route.params.orgId as string);

const canAdmin = computed(() => (auth.user ? orgStore.canAdmin(auth.user.id) : false));

const selectedRuleset = ref<string | null>(null);
const versionData = ref<{
  name: string;
  current_version: string;
  current_display_version: string;
  versions: Array<{
    seq: number;
    version: string;
    display_version: string;
    created_at: string;
    entry: RulesetHistoryEntry;
  }>;
} | null>(null);
const historyEntries = ref<RulesetHistoryEntry[]>([]);
const loading = ref(false);
const rollingBack = ref<number | null>(null);

async function loadVersions(name: string) {
  if (!auth.token || !projectStore.currentProject) return;
  loading.value = true;
  try {
    const history = await rulesetHistoryApi.list(auth.token, projectStore.currentProject.id, name);
    historyEntries.value = history.entries;
    const checkpoints = buildCheckpointVersionEntries(history.entries);
    const meta = projectStore.draftMetas.find((entry) => entry.name === name) ?? null;
    const currentVersion =
      meta?.draft_version ?? extractRulesetVersion(history.entries[0]?.snapshot);
    const currentDisplayVersion =
      checkpoints.find((item) => item.version === currentVersion)?.display_version ??
      currentVersion;
    versionData.value = {
      name,
      current_version: currentVersion,
      current_display_version: currentDisplayVersion,
      versions: checkpoints.map((item, index) => ({
        seq: checkpoints.length - index,
        version: item.version,
        display_version: item.display_version,
        created_at: item.entry.created_at,
        entry: item.entry,
      })),
    };
  } catch (e: any) {
    MessagePlugin.error(e.message || t('versions.loadFailed'));
    versionData.value = null;
    historyEntries.value = [];
  } finally {
    loading.value = false;
  }
}

watch(selectedRuleset, (name) => {
  if (name) loadVersions(name);
  else versionData.value = null;
});

// Auto-select first ruleset
watch(
  () => projectStore.rulesets,
  (list) => {
    if (list.length > 0 && !selectedRuleset.value) {
      selectedRuleset.value = list[0].name;
    }
  },
  { immediate: true }
);

function handleRollback(seq: number) {
  if (!selectedRuleset.value) return;
  const name = selectedRuleset.value;
  const dlg = DialogPlugin.confirm({
    header: t('versions.rollbackDialog'),
    body: t('versions.rollbackConfirm', { name, seq }),
    confirmBtn: { content: t('versions.rollbackConfirmBtn'), theme: 'warning' },
    cancelBtn: t('versions.rollbackCancel'),
    onConfirm: async () => {
      if (!auth.token || !projectStore.currentProject) return;
      rollingBack.value = seq;
      try {
        const org = orgStore.currentOrg;
        if (!org) throw new Error('No active org');
        const target = versionData.value?.versions.find((item) => item.seq === seq)?.entry;
        if (!target) throw new Error('Version not found');
        const currentDraft = await rulesetDraftApi.get(
          auth.token,
          org.id,
          projectStore.currentProject.id,
          name
        );
        const restored = await rulesetDraftApi.save(
          auth.token,
          org.id,
          projectStore.currentProject.id,
          name,
          {
            ruleset: target.snapshot,
            expected_seq: currentDraft.draft_seq,
          }
        );
        if ('conflict' in restored) {
          throw new Error(t('versions.rollbackFailed'));
        }
        await rulesetHistoryApi.append(auth.token, projectStore.currentProject.id, name, [
          {
            id: crypto.randomUUID(),
            action: `restore version #${seq}`,
            source: 'restore',
            snapshot: target.snapshot,
          },
        ]);
        dlg.hide();
        MessagePlugin.success(t('versions.rollbackSuccess', { seq }));
        await projectStore.fetchRulesets();
        await loadVersions(name);
      } catch (e: any) {
        MessagePlugin.error(e.message || t('versions.rollbackFailed'));
      } finally {
        rollingBack.value = null;
      }
    },
  });
}

function formatDate(dateStr: string) {
  return new Date(dateStr).toLocaleString(
    locale.value === 'zh-TW' ? 'zh-TW' : locale.value === 'zh-CN' ? 'zh-CN' : 'en-US',
    {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }
  );
}
</script>

<template>
  <div class="version-view">
    <t-breadcrumb class="asset-breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{
        t('breadcrumb.home')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{
        t('breadcrumb.projects')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('projectNav.versions') }}</t-breadcrumb-item>
    </t-breadcrumb>
    <div class="asset-header">
      <div class="asset-header__info">
        <h2 class="asset-header__title">{{ t('versions.title') }}</h2>
        <p class="asset-header__desc">{{ t('versions.desc') }}</p>
      </div>
    </div>

    <div class="version-body">
      <!-- Left: ruleset selector -->
      <div class="version-selector">
        <div class="version-selector__title">{{ t('versions.rulesets') }}</div>
        <div v-if="projectStore.rulesets.length === 0" class="asset-empty">
          <p>{{ t('versions.noRulesets') }}</p>
        </div>
        <div
          v-for="rs in projectStore.rulesets"
          :key="rs.name"
          class="ruleset-item"
          :class="{ 'is-active': rs.name === selectedRuleset }"
          @click="selectedRuleset = rs.name"
        >
          <t-icon name="file-code" size="13px" style="opacity: 0.6; flex-shrink: 0" />
          <span class="ruleset-item__name">{{ rs.name }}</span>
          <span class="ruleset-item__version">v{{ rs.version }}</span>
        </div>
      </div>

      <!-- Right: version timeline -->
      <div class="version-timeline">
        <div v-if="!selectedRuleset" class="version-placeholder">
          <t-icon name="history" size="36px" style="opacity: 0.15" />
          <p>{{ t('versions.placeholder') }}</p>
        </div>

        <div v-else-if="loading" class="asset-loading">
          <t-loading />
        </div>

        <template v-else-if="versionData">
          <div class="version-timeline__header">
            <span class="version-timeline__name">{{ versionData.name }}</span>
            <t-tag size="small" theme="primary" variant="light">
              {{ t('versions.current') }} {{ versionData.current_display_version }}
            </t-tag>
            <t-button size="small" variant="outline" @click="loadVersions(selectedRuleset!)">
              <t-icon name="refresh" />
            </t-button>
          </div>

          <div v-if="versionData.versions.length === 0" class="asset-empty">
            <p>{{ t('versions.noVersions') }}</p>
          </div>

          <div class="version-list">
            <div
              v-for="v in versionData.versions"
              :key="v.seq"
              class="version-entry"
              :class="{ 'is-current': v.display_version === versionData.current_display_version }"
            >
              <div class="version-entry__seq">#{{ v.seq }}</div>
              <div class="version-entry__body">
                <div class="version-entry__version">v{{ v.display_version }}</div>
                <div class="version-entry__time">{{ formatDate(v.created_at) }}</div>
              </div>
              <div class="version-entry__actions">
                <t-tag
                  v-if="v.display_version === versionData.current_display_version"
                  size="small"
                  theme="success"
                  variant="light"
                  >{{ t('versions.currentTag') }}</t-tag
                >
                <t-button
                  v-else-if="canAdmin"
                  size="small"
                  variant="outline"
                  theme="warning"
                  :loading="rollingBack === v.seq"
                  @click="handleRollback(v.seq)"
                >
                  {{ t('versions.rollbackBtn') }}
                </t-button>
              </div>
            </div>
          </div>
        </template>

        <div v-else class="asset-empty">
          <p>{{ t('versions.loadError') }}</p>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.version-view {
  padding: 20px 24px;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.asset-header {
  flex-shrink: 0;
  margin-bottom: 16px;
}

.asset-header__title {
  font-size: 16px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin: 0 0 4px;
}

.asset-header__desc {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  margin: 0;
}

.version-body {
  flex: 1;
  display: flex;
  gap: 16px;
  overflow: hidden;
}

.version-selector {
  width: 220px;
  flex-shrink: 0;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  overflow-y: auto;
}

.version-selector__title {
  padding: 8px 12px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--ordo-text-tertiary);
  border-bottom: 1px solid var(--ordo-border-light);
}

.ruleset-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  cursor: pointer;
  border-bottom: 1px solid var(--ordo-border-light);
}

.ruleset-item:hover {
  background: var(--ordo-hover-bg);
}
.ruleset-item.is-active {
  background: var(--ordo-active-bg);
}

.ruleset-item__name {
  flex: 1;
  font-size: 12px;
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ruleset-item__version {
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  font-family: 'JetBrains Mono', monospace;
}

.version-timeline {
  flex: 1;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  overflow-y: auto;
  padding: 16px;
}

.version-timeline__header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;
}

.version-timeline__name {
  font-family: 'JetBrains Mono', monospace;
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  flex: 1;
}

.version-list {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.version-entry {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid var(--ordo-border-light);
}

.version-entry.is-current {
  background: var(--ordo-active-bg);
  margin: 0 -16px;
  padding: 10px 16px;
}

.version-entry__seq {
  width: 36px;
  font-size: 11px;
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-tertiary);
  text-align: right;
}

.version-entry__body {
  flex: 1;
}

.version-entry__version {
  font-size: 13px;
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-primary);
}

.version-entry__time {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  margin-top: 2px;
}

.version-entry__actions {
  flex-shrink: 0;
}

.version-placeholder {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--ordo-text-tertiary);
  font-size: 13px;
}

.asset-loading,
.asset-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 120px;
  color: var(--ordo-text-tertiary);
  font-size: 13px;
}
</style>
