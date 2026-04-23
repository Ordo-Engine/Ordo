<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter, useRoute } from 'vue-router';
import { useCatalogStore } from '@/stores/catalog';
import { useOrgStore } from '@/stores/org';
import { useAuthStore } from '@/stores/auth';
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next';
import type { FactDataType, FactDefinition, NullPolicy } from '@/api/types';

const catalog = useCatalogStore();
const orgStore = useOrgStore();
const auth = useAuthStore();
const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const orgId = computed(() => route.params.orgId as string);

const canEdit = computed(() => (auth.user ? orgStore.canAdmin(auth.user.id) : false));

const selected = ref<FactDefinition | null>(null);
const saving = ref(false);
const isNew = ref(false);
const searchQuery = ref('');
const prefillHint = ref('');

const emptyForm = (name = ''): Omit<FactDefinition, 'created_at' | 'updated_at'> => ({
  name,
  data_type: 'string',
  source: '',
  null_policy: 'error',
  latency_ms: undefined,
  description: '',
  owner: '',
});

const form = ref(emptyForm());

const filteredFacts = computed(() => {
  const keyword = searchQuery.value.trim().toLowerCase();
  const facts = [...catalog.facts].sort((a, b) => a.name.localeCompare(b.name));
  if (!keyword) return facts;

  return facts.filter((fact) =>
    [fact.name, fact.source, fact.owner ?? '', fact.description ?? ''].some((value) =>
      value.toLowerCase().includes(keyword)
    )
  );
});

const dataTypeLabel = computed<Record<FactDataType, string>>(() => ({
  string: t('facts.typeString'),
  number: t('facts.typeNumber'),
  boolean: t('facts.typeBoolean'),
  date: t('facts.typeDate'),
  object: t('facts.typeObject'),
}));

const nullPolicyLabel = computed<Record<NullPolicy, string>>(() => ({
  error: t('facts.policyError'),
  default: t('facts.policyDefault'),
  skip: t('facts.policySkip'),
}));

watch(
  () => catalog.facts,
  (facts) => {
    if (isNew.value) return;
    if (!selected.value && facts.length > 0) {
      openEdit([...facts].sort((a, b) => a.name.localeCompare(b.name))[0]);
      return;
    }
    if (selected.value) {
      const next = facts.find((fact) => fact.name === selected.value?.name) ?? null;
      selected.value = next;
    }
  },
  { immediate: true, deep: true }
);

watch(
  () => route.query.createFact,
  (value) => {
    const requestedName = typeof value === 'string' ? value.trim() : '';
    if (!requestedName) return;

    const existing = catalog.facts.find((fact) => fact.name === requestedName);
    if (existing) {
      openEdit(existing);
    } else {
      openNew(requestedName);
      prefillHint.value = t('facts.prefillHint', { name: requestedName });
    }

    const nextQuery = { ...route.query };
    delete nextQuery.createFact;
    void router.replace({ query: nextQuery });
  },
  { immediate: true }
);

function openNew(prefillName = '') {
  isNew.value = true;
  selected.value = null;
  form.value = emptyForm(prefillName);
  prefillHint.value = prefillName ? t('facts.prefillHint', { name: prefillName }) : '';
}

function openEdit(fact: FactDefinition) {
  isNew.value = false;
  selected.value = fact;
  prefillHint.value = '';
  form.value = {
    name: fact.name,
    data_type: fact.data_type,
    source: fact.source,
    null_policy: fact.null_policy,
    latency_ms: fact.latency_ms,
    description: fact.description ?? '',
    owner: fact.owner ?? '',
  };
}

async function handleSave() {
  if (!form.value.name.trim()) {
    MessagePlugin.warning(t('facts.nameRequired'));
    return;
  }
  if (!form.value.source.trim()) {
    MessagePlugin.warning(t('facts.sourceRequired'));
    return;
  }

  saving.value = true;
  try {
    const saved = await catalog.upsertFact({
      ...form.value,
      name: form.value.name.trim(),
      source: form.value.source.trim(),
      description: form.value.description?.trim() || undefined,
      owner: form.value.owner?.trim() || undefined,
    });
    if (!saved) {
      throw new Error(t('facts.saveFailed'));
    }

    MessagePlugin.success(isNew.value ? t('facts.createSuccess') : t('facts.updateSuccess'));
    openEdit(saved);
  } catch (e: any) {
    MessagePlugin.error(e.message || t('facts.saveFailed'));
  } finally {
    saving.value = false;
  }
}

function handleDelete(fact: FactDefinition) {
  const dlg = DialogPlugin.confirm({
    header: t('facts.deleteDialog'),
    body: t('facts.deleteConfirm', { name: fact.name }),
    confirmBtn: { content: t('common.delete'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await catalog.deleteFact(fact.name);
        if (selected.value?.name === fact.name) {
          selected.value = null;
          const next = [...catalog.facts]
            .filter((item) => item.name !== fact.name)
            .sort((a, b) => a.name.localeCompare(b.name))[0];
          if (next) {
            openEdit(next);
          } else {
            isNew.value = false;
            form.value = emptyForm();
          }
        }
        dlg.hide();
        MessagePlugin.success(t('facts.deleteSuccess'));
      } catch (e: any) {
        MessagePlugin.error(e.message || t('facts.saveFailed'));
      }
    },
  });
}

function cancelEdit() {
  prefillHint.value = '';
  if (selected.value) {
    openEdit(selected.value);
  } else {
    isNew.value = false;
    form.value = emptyForm();
  }
}
</script>

<template>
  <div class="fact-view">
    <t-breadcrumb class="asset-breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{
        t('breadcrumb.home')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{
        t('breadcrumb.projects')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('projectNav.facts') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <div class="asset-header">
      <div class="asset-header__info">
        <h2 class="asset-header__title">{{ t('facts.title') }}</h2>
        <p class="asset-header__desc">{{ t('facts.desc') }}</p>
      </div>
      <t-button v-if="canEdit" theme="primary" @click="openNew()">
        <t-icon name="add" />
        {{ t('facts.add') }}
      </t-button>
    </div>

    <div class="fact-body">
      <div class="fact-list">
        <div class="fact-list__toolbar">
          <t-input
            v-model="searchQuery"
            clearable
            size="small"
            :placeholder="t('facts.searchPlaceholder')"
          />
        </div>

        <div v-if="catalog.loading" class="asset-loading">
          <t-loading size="small" />
        </div>
        <div v-else-if="catalog.facts.length === 0" class="asset-empty">
          <t-icon name="data" size="32px" style="opacity: 0.3" />
          <p>{{ t('facts.empty') }}</p>
        </div>
        <div
          v-for="fact in filteredFacts"
          :key="fact.name"
          class="fact-item"
          :class="{ 'is-active': selected?.name === fact.name && !isNew }"
          @click="openEdit(fact)"
        >
          <div class="fact-item__name">{{ fact.name }}</div>
          <div class="fact-item__meta">
            <t-tag size="small" variant="light">{{ dataTypeLabel[fact.data_type] }}</t-tag>
            <span class="fact-item__policy">{{ nullPolicyLabel[fact.null_policy] }}</span>
          </div>
          <div class="fact-item__source">{{ fact.source }}</div>
          <button v-if="canEdit" class="fact-item__del" @click.stop="handleDelete(fact)">
            <t-icon name="close" size="12px" />
          </button>
        </div>

        <div v-if="isNew" class="fact-item is-active">
          <div class="fact-item__name" style="opacity: 0.5">
            {{ form.name || t('facts.newPlaceholder') }}
          </div>
          <div class="fact-item__source">{{ prefillHint || t('facts.editorNew') }}</div>
        </div>
      </div>

      <div class="fact-editor" v-if="selected || isNew">
        <div v-if="prefillHint" class="fact-prefill">{{ prefillHint }}</div>
        <t-form label-align="top" colon>
          <t-form-item :label="t('facts.nameLabel')" required>
            <t-input
              v-model="form.name"
              :placeholder="t('facts.namePlaceholder')"
              :disabled="!isNew"
            />
          </t-form-item>
          <t-form-item :label="t('facts.typeLabel')" required>
            <t-select v-model="form.data_type">
              <t-option value="string" :label="t('facts.typeString')" />
              <t-option value="number" :label="t('facts.typeNumber')" />
              <t-option value="boolean" :label="t('facts.typeBoolean')" />
              <t-option value="date" :label="t('facts.typeDate')" />
              <t-option value="object" :label="t('facts.typeObject')" />
            </t-select>
          </t-form-item>
          <t-form-item :label="t('facts.sourceLabel')" required>
            <t-input v-model="form.source" :placeholder="t('facts.sourcePlaceholder')" />
          </t-form-item>
          <t-form-item :label="t('facts.nullPolicyLabel')">
            <t-select v-model="form.null_policy">
              <t-option value="error" :label="t('facts.policyError')" />
              <t-option value="default" :label="t('facts.policyDefault')" />
              <t-option value="skip" :label="t('facts.policySkip')" />
            </t-select>
          </t-form-item>
          <t-form-item :label="t('facts.latencyLabel')">
            <t-input-number
              v-model="form.latency_ms"
              :min="0"
              :placeholder="t('facts.latencyPlaceholder')"
            />
          </t-form-item>
          <t-form-item :label="t('facts.ownerLabel')">
            <t-input v-model="form.owner" :placeholder="t('facts.ownerPlaceholder')" />
          </t-form-item>
          <t-form-item :label="t('facts.descLabel')">
            <t-textarea
              v-model="form.description"
              :rows="4"
              :placeholder="t('facts.descPlaceholder')"
            />
          </t-form-item>
        </t-form>

        <div class="fact-editor__footer">
          <t-button v-if="canEdit" theme="primary" :loading="saving" @click="handleSave">
            {{ isNew ? t('common.create') : t('facts.save') }}
          </t-button>
          <t-button variant="outline" @click="cancelEdit">{{ t('facts.cancel') }}</t-button>
        </div>
      </div>

      <div v-else class="fact-placeholder">
        <t-icon name="data" size="36px" style="opacity: 0.15" />
        <p>{{ t('facts.placeholder') }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.fact-view {
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

.fact-body {
  flex: 1;
  display: flex;
  gap: 16px;
  overflow: hidden;
}

.fact-list {
  width: 260px;
  flex-shrink: 0;
  overflow-y: auto;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
}

.fact-list__toolbar {
  padding: 8px;
  border-bottom: 1px solid var(--ordo-border-light);
}

.fact-item {
  padding: 10px 12px;
  cursor: pointer;
  border-bottom: 1px solid var(--ordo-border-light);
  position: relative;
}

.fact-item:hover {
  background: var(--ordo-hover-bg);
}
.fact-item.is-active {
  background: var(--ordo-active-bg);
}

.fact-item__name {
  font-size: 13px;
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-primary);
  margin-bottom: 4px;
}

.fact-item__meta {
  display: flex;
  align-items: center;
  gap: 6px;
}

.fact-item__policy {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.fact-item__source {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  margin-top: 4px;
}

.fact-item__del {
  position: absolute;
  top: 8px;
  right: 8px;
  display: none;
  width: 18px;
  height: 18px;
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  border-radius: 3px;
  align-items: center;
  justify-content: center;
}

.fact-item:hover .fact-item__del {
  display: flex;
}
.fact-item__del:hover {
  background: rgba(255, 80, 80, 0.15);
  color: #e34d59;
}

.fact-editor {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
}

.fact-prefill {
  margin-bottom: 12px;
  padding: 8px 10px;
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-active-bg);
  color: var(--ordo-text-secondary);
  font-size: 12px;
}

.fact-editor__footer {
  display: flex;
  gap: 8px;
  margin-top: 16px;
}

.fact-placeholder {
  flex: 1;
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
  font-size: 12px;
}
</style>
