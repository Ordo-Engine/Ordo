<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin } from 'tdesign-vue-next';
import {
  RuleSet,
  Step as StepFactory,
  type RuleSet as StudioRuleSet,
} from '@ordo-engine/editor-core';
import { subRuleApi } from '@/api/platform-client';
import type { SubRuleAssetMeta, SubRuleScope } from '@/api/types';
import { useAuthStore } from '@/stores/auth';

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const { t } = useI18n();

const orgId = computed(() => route.params.orgId as string);
const projectId = computed(() => route.params.projectId as string);

const assets = ref<SubRuleAssetMeta[]>([]);
const selectedAssetId = ref<string | null>(null);
const loading = ref(false);
const creating = ref(false);
const isCreating = ref(false);
const searchQuery = ref('');

const form = ref({
  name: '',
  displayName: '',
  description: '',
  scope: 'project' as SubRuleScope,
});

const filteredAssets = computed(() => {
  const keyword = searchQuery.value.trim().toLowerCase();
  const sorted = [...assets.value].sort((a, b) => {
    if (a.scope !== b.scope) return a.scope === 'project' ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  if (!keyword) return sorted;

  return sorted.filter((asset) =>
    [asset.name, asset.display_name ?? '', asset.description ?? '', asset.scope].some((value) =>
      value.toLowerCase().includes(keyword)
    )
  );
});

const selectedAsset = computed(
  () => assets.value.find((asset) => asset.id === selectedAssetId.value) ?? null
);

const projectCount = computed(
  () => assets.value.filter((asset) => asset.scope === 'project').length
);
const orgCount = computed(() => assets.value.filter((asset) => asset.scope === 'org').length);

async function loadAssets() {
  if (!auth.token || !orgId.value || !projectId.value) return;
  loading.value = true;
  try {
    assets.value = await subRuleApi.listProject(auth.token, orgId.value, projectId.value);
    if (!selectedAsset.value && assets.value.length > 0) {
      selectedAssetId.value = assets.value[0].id;
    }
  } catch (e: any) {
    MessagePlugin.error(e.message || t('subRules.loadFailed'));
  } finally {
    loading.value = false;
  }
}

function createDefaultDraft(name: string): StudioRuleSet {
  const terminal = StepFactory.terminal({
    id: 'return_result',
    name: t('subRules.defaultTerminalName'),
    code: 'OK',
    message: {
      type: 'literal',
      value: '',
      valueType: 'string',
    },
    output: [],
    position: { x: 160, y: 120 },
  });

  return RuleSet.create({
    name,
    version: '0.1.0',
    description: t('subRules.defaultDescription'),
    startStepId: terminal.id,
    steps: [terminal],
    enableTrace: true,
  });
}

function openCreate(scope: SubRuleScope = 'project') {
  isCreating.value = true;
  selectedAssetId.value = null;
  form.value = {
    name: '',
    displayName: '',
    description: '',
    scope,
  };
}

function openAsset(asset: SubRuleAssetMeta) {
  isCreating.value = false;
  selectedAssetId.value = asset.id;
}

async function createAsset() {
  if (!auth.token) return;
  const name = form.value.name.trim();
  if (!name) {
    MessagePlugin.warning(t('subRules.nameRequired'));
    return;
  }

  creating.value = true;
  try {
    const payload = {
      name,
      display_name: form.value.displayName.trim() || name,
      description: form.value.description.trim(),
      draft: createDefaultDraft(name),
      input_schema: [],
      output_schema: [],
    };

    const asset =
      form.value.scope === 'org'
        ? await subRuleApi.saveOrg(auth.token, orgId.value, name, payload)
        : await subRuleApi.saveProject(auth.token, orgId.value, projectId.value, name, payload);

    MessagePlugin.success(t('subRules.createSuccess'));
    await loadAssets();
    selectedAssetId.value = asset.id;
    isCreating.value = false;
  } catch (e: any) {
    MessagePlugin.error(e.message || t('subRules.saveFailed'));
  } finally {
    creating.value = false;
  }
}

function cancelCreate() {
  isCreating.value = false;
  if (!selectedAssetId.value && assets.value.length > 0) {
    selectedAssetId.value = assets.value[0].id;
  }
}

function openInEditor(asset: SubRuleAssetMeta) {
  const tabParam = encodeURIComponent(`§${asset.name}`);
  router.push(`/orgs/${orgId.value}/projects/${projectId.value}/editor/${tabParam}`);
}

function formatTime(value: string | null) {
  if (!value) return '—';
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
}

watch(
  () => [orgId.value, projectId.value],
  () => {
    selectedAssetId.value = null;
    isCreating.value = false;
    void loadAssets();
  }
);

onMounted(loadAssets);
</script>

<template>
  <div class="sub-rules-view">
    <t-breadcrumb class="asset-breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">
        {{ t('breadcrumb.home') }}
      </t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">
        {{ t('breadcrumb.projects') }}
      </t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('projectNav.subRules') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <div class="asset-header">
      <div class="asset-header__info">
        <h2 class="asset-header__title">{{ t('subRules.title') }}</h2>
        <p class="asset-header__desc">{{ t('subRules.desc') }}</p>
      </div>
      <t-button theme="primary" @click="openCreate('project')">
        <t-icon name="add" />
        {{ t('subRules.create') }}
      </t-button>
    </div>

    <div class="sub-rule-body">
      <aside class="sub-rule-list">
        <div class="sub-rule-list__toolbar">
          <t-input
            v-model="searchQuery"
            clearable
            size="small"
            :placeholder="t('subRules.searchPlaceholder')"
          />
        </div>

        <div class="sub-rule-list__stats">
          <span>{{ t('subRules.scopeProject') }} {{ projectCount }}</span>
          <span>{{ t('subRules.scopeOrg') }} {{ orgCount }}</span>
        </div>

        <div v-if="loading" class="asset-loading">
          <t-loading size="small" />
        </div>
        <div v-else-if="assets.length === 0" class="asset-empty">
          <t-icon name="git-branch" size="32px" style="opacity: 0.3" />
          <p>{{ t('subRules.empty') }}</p>
        </div>
        <template v-else>
          <button
            v-for="asset in filteredAssets"
            :key="asset.id"
            class="sub-rule-item"
            :class="{ 'is-active': selectedAsset?.id === asset.id && !isCreating }"
            @click="openAsset(asset)"
          >
            <span class="sub-rule-item__main">
              <strong>{{ asset.display_name || asset.name }}</strong>
              <small>{{ asset.name }}</small>
            </span>
            <span class="sub-rule-item__meta">
              <t-tag
                size="small"
                variant="light"
                :theme="asset.scope === 'project' ? 'primary' : 'default'"
              >
                {{
                  asset.scope === 'project' ? t('subRules.scopeProject') : t('subRules.scopeOrg')
                }}
              </t-tag>
            </span>
          </button>
        </template>
      </aside>

      <main class="sub-rule-detail">
        <section v-if="isCreating" class="sub-rule-panel">
          <div class="sub-rule-panel__header">
            <div>
              <h3>{{ t('subRules.createTitle') }}</h3>
              <p>{{ t('subRules.createDesc') }}</p>
            </div>
          </div>

          <t-form label-align="top" colon>
            <t-form-item :label="t('subRules.assetScope')" required>
              <t-radio-group v-model="form.scope" variant="default-filled">
                <t-radio-button value="project">{{ t('subRules.scopeProject') }}</t-radio-button>
                <t-radio-button value="org">{{ t('subRules.scopeOrg') }}</t-radio-button>
              </t-radio-group>
            </t-form-item>
            <t-form-item :label="t('subRules.name')" required>
              <t-input v-model="form.name" :placeholder="t('subRules.namePlaceholder')" />
            </t-form-item>
            <t-form-item :label="t('subRules.displayName')">
              <t-input
                v-model="form.displayName"
                :placeholder="t('subRules.displayNamePlaceholder')"
              />
            </t-form-item>
            <t-form-item :label="t('subRules.description')">
              <t-textarea
                v-model="form.description"
                :rows="4"
                :placeholder="t('subRules.descriptionPlaceholder')"
              />
            </t-form-item>
          </t-form>

          <div class="sub-rule-panel__footer">
            <t-button theme="primary" :loading="creating" @click="createAsset">
              {{ t('common.create') }}
            </t-button>
            <t-button variant="outline" @click="cancelCreate">{{ t('common.cancel') }}</t-button>
          </div>
        </section>

        <section v-else-if="selectedAsset" class="sub-rule-panel">
          <div class="sub-rule-panel__header">
            <div>
              <div class="sub-rule-panel__eyebrow">
                {{
                  selectedAsset.scope === 'project'
                    ? t('subRules.scopeProject')
                    : t('subRules.scopeOrg')
                }}
              </div>
              <h3>{{ selectedAsset.display_name || selectedAsset.name }}</h3>
              <p>{{ selectedAsset.description || t('subRules.noDescription') }}</p>
            </div>
            <div class="sub-rule-panel__header-actions">
              <t-button
                size="small"
                theme="primary"
                variant="outline"
                @click="openInEditor(selectedAsset)"
              >
                {{ t('subRules.openInEditor') }}
              </t-button>
            </div>
          </div>

          <div class="sub-rule-stats">
            <div class="sub-rule-stat">
              <span>{{ t('subRules.name') }}</span>
              <strong>{{ selectedAsset.name }}</strong>
            </div>
            <div class="sub-rule-stat">
              <span>{{ t('subRules.updatedAt') }}</span>
              <strong>{{ formatTime(selectedAsset.draft_updated_at) }}</strong>
            </div>
          </div>

          <div class="sub-rule-usage">
            <h4>{{ t('subRules.usageTitle') }}</h4>
            <p>{{ t('subRules.usageDesc') }}</p>
            <code>
              {{ `{ "scope": "${selectedAsset.scope}", "name": "${selectedAsset.name}" }` }}
            </code>
          </div>
        </section>

        <div v-else class="sub-rule-placeholder">
          <t-icon name="git-branch" size="40px" style="opacity: 0.16" />
          <p>{{ t('subRules.placeholder') }}</p>
          <t-button theme="primary" variant="outline" @click="openCreate('project')">
            {{ t('subRules.create') }}
          </t-button>
        </div>
      </main>
    </div>
  </div>
</template>

<style scoped>
.sub-rules-view {
  padding: 20px 24px;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.asset-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 16px;
  gap: 16px;
  flex-shrink: 0;
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

.sub-rule-body {
  flex: 1;
  display: flex;
  gap: 16px;
  overflow: hidden;
}

.sub-rule-list {
  width: 300px;
  flex-shrink: 0;
  overflow-y: auto;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-panel);
}

.sub-rule-list__toolbar {
  padding: 8px;
  border-bottom: 1px solid var(--ordo-border-light);
}

.sub-rule-list__stats {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  padding: 8px 10px;
  border-bottom: 1px solid var(--ordo-border-light);
  color: var(--ordo-text-tertiary);
  font-size: 11px;
}

.sub-rule-item {
  width: 100%;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 11px 12px;
  border: 0;
  border-bottom: 1px solid var(--ordo-border-light);
  background: transparent;
  cursor: pointer;
  text-align: left;
}

.sub-rule-item:hover {
  background: var(--ordo-hover-bg);
}

.sub-rule-item.is-active {
  background: var(--ordo-active-bg);
}

.sub-rule-item__main {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.sub-rule-item__main strong {
  color: var(--ordo-text-primary);
  font-size: 13px;
  font-weight: 600;
}

.sub-rule-item__main small {
  overflow: hidden;
  color: var(--ordo-text-tertiary);
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sub-rule-item__meta {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 5px;
  color: var(--ordo-text-tertiary);
  font-size: 11px;
}

.sub-rule-detail {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
}

.sub-rule-panel,
.sub-rule-placeholder {
  min-height: 100%;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-panel);
}

.sub-rule-panel {
  padding: 18px;
}

.sub-rule-panel__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--ordo-border-light);
}

.sub-rule-panel__header h3 {
  margin: 0 0 6px;
  color: var(--ordo-text-primary);
  font-size: 18px;
  font-weight: 600;
}

.sub-rule-panel__header p {
  margin: 0;
  max-width: 760px;
  color: var(--ordo-text-secondary);
  font-size: 13px;
  line-height: 1.6;
}

.sub-rule-panel__eyebrow {
  margin-bottom: 6px;
  color: var(--ordo-text-tertiary);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.sub-rule-panel__header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.sub-rule-panel__footer {
  display: flex;
  gap: 8px;
  margin-top: 18px;
}

.sub-rule-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: 16px;
  border: 1px solid var(--ordo-border-light);
  border-radius: var(--ordo-radius-md);
  overflow: hidden;
}

.sub-rule-stat {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 14px;
  border-right: 1px solid var(--ordo-border-light);
}

.sub-rule-stat:last-child {
  border-right: 0;
}

.sub-rule-stat span {
  color: var(--ordo-text-tertiary);
  font-size: 11px;
}

.sub-rule-stat strong {
  color: var(--ordo-text-primary);
  font-size: 13px;
  font-weight: 600;
  word-break: break-all;
}

.sub-rule-usage {
  margin-top: 16px;
  padding: 16px;
  border: 1px solid var(--ordo-border-light);
  border-radius: var(--ordo-radius-md);
}

.sub-rule-usage h4 {
  margin: 0 0 6px;
  color: var(--ordo-text-primary);
  font-size: 14px;
}

.sub-rule-usage p {
  margin: 0 0 12px;
  color: var(--ordo-text-secondary);
  font-size: 13px;
}

.sub-rule-usage code {
  display: block;
  padding: 10px 12px;
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-code, rgba(15, 23, 42, 0.04));
  color: var(--ordo-text-primary);
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
  white-space: normal;
  word-break: break-all;
}

.sub-rule-placeholder,
.asset-loading,
.asset-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
}

.sub-rule-placeholder {
  min-height: 280px;
}

.asset-loading,
.asset-empty {
  height: 140px;
}

@media (max-width: 900px) {
  .sub-rule-body {
    flex-direction: column;
  }

  .sub-rule-list {
    width: 100%;
    max-height: 280px;
  }

  .sub-rule-stats {
    grid-template-columns: 1fr;
  }

  .sub-rule-stat {
    border-right: 0;
    border-bottom: 1px solid var(--ordo-border-light);
  }

  .sub-rule-stat:last-child {
    border-bottom: 0;
  }
}
</style>
