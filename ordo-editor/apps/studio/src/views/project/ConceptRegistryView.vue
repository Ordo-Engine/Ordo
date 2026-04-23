<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter, useRoute } from 'vue-router';
import { useCatalogStore } from '@/stores/catalog';
import { useOrgStore } from '@/stores/org';
import { useAuthStore } from '@/stores/auth';
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next';
import type { ConceptDefinition, FactDataType } from '@/api/types';

const catalog = useCatalogStore();
const orgStore = useOrgStore();
const auth = useAuthStore();
const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const orgId = computed(() => route.params.orgId as string);

const canEdit = computed(() => (auth.user ? orgStore.canAdmin(auth.user.id) : false));

// ── Edit panel ────────────────────────────────────────────────────────────────
const selected = ref<ConceptDefinition | null>(null);
const saving = ref(false);
const isNew = ref(false);

const emptyForm = (): Omit<ConceptDefinition, 'created_at' | 'updated_at'> => ({
  name: '',
  data_type: 'number',
  expression: '',
  dependencies: [],
  description: '',
});

const form = ref(emptyForm());

function openNew() {
  isNew.value = true;
  selected.value = null;
  form.value = emptyForm();
}

function openEdit(concept: ConceptDefinition) {
  isNew.value = false;
  selected.value = concept;
  form.value = {
    name: concept.name,
    data_type: concept.data_type,
    expression: concept.expression,
    dependencies: [...concept.dependencies],
    description: concept.description ?? '',
  };
}

// Simple cycle detection: DFS from new concept's dependencies
function hasCycle(name: string, deps: string[]): boolean {
  const visited = new Set<string>();
  function dfs(node: string): boolean {
    if (node === name) return true;
    if (visited.has(node)) return false;
    visited.add(node);
    const concept = catalog.concepts.find((c) => c.name === node);
    if (!concept) return false;
    return concept.dependencies.some(dfs);
  }
  return deps.some(dfs);
}

async function handleSave() {
  if (!form.value.name.trim()) {
    MessagePlugin.warning(t('concepts.nameRequired'));
    return;
  }
  if (!form.value.expression.trim()) {
    MessagePlugin.warning(t('concepts.exprRequired'));
    return;
  }
  if (hasCycle(form.value.name, form.value.dependencies)) {
    MessagePlugin.error(t('concepts.cycleError'));
    return;
  }
  saving.value = true;
  try {
    await catalog.upsertConcept({
      ...form.value,
      name: form.value.name.trim(),
      expression: form.value.expression.trim(),
      description: form.value.description?.trim() || undefined,
    });
    MessagePlugin.success(isNew.value ? t('concepts.createSuccess') : t('concepts.updateSuccess'));
    isNew.value = false;
    selected.value = catalog.concepts.find((c) => c.name === form.value.name) ?? null;
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.saveFailed'));
  } finally {
    saving.value = false;
  }
}

function handleDelete(concept: ConceptDefinition) {
  const dlg = DialogPlugin.confirm({
    header: t('concepts.deleteDialog'),
    body: t('concepts.deleteConfirm', { name: concept.name }),
    confirmBtn: { content: t('common.delete'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await catalog.deleteConcept(concept.name);
        if (selected.value?.name === concept.name) {
          selected.value = null;
        }
        dlg.hide();
        MessagePlugin.success(t('concepts.deleteSuccess'));
      } catch (e: any) {
        MessagePlugin.error(e.message);
      }
    },
  });
}

const dataTypeLabels = computed<Record<FactDataType, string>>(() => ({
  string: t('facts.typeString'),
  number: t('facts.typeNumber'),
  boolean: t('facts.typeBoolean'),
  date: t('facts.typeDate'),
  object: t('facts.typeObject'),
}));
</script>

<template>
  <div class="concept-view">
    <t-breadcrumb class="asset-breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{
        t('breadcrumb.home')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{
        t('breadcrumb.projects')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('projectNav.concepts') }}</t-breadcrumb-item>
    </t-breadcrumb>
    <div class="asset-header">
      <div class="asset-header__info">
        <h2 class="asset-header__title">{{ t('concepts.title') }}</h2>
        <p class="asset-header__desc">{{ t('concepts.desc') }}</p>
      </div>
      <t-button v-if="canEdit" theme="primary" @click="openNew">
        <t-icon name="add" />{{ t('concepts.add') }}
      </t-button>
    </div>

    <div class="concept-body">
      <!-- Left: concept list -->
      <div class="concept-list">
        <div v-if="catalog.loading" class="asset-loading"><t-loading size="small" /></div>
        <div v-else-if="catalog.concepts.length === 0" class="asset-empty">
          <t-icon name="share" size="32px" style="opacity: 0.3" />
          <p>{{ t('concepts.empty') }}</p>
        </div>
        <div
          v-for="c in catalog.concepts"
          :key="c.name"
          class="concept-item"
          :class="{ 'is-active': selected?.name === c.name }"
          @click="openEdit(c)"
        >
          <div class="concept-item__name">{{ c.name }}</div>
          <div class="concept-item__meta">
            <t-tag size="small" variant="light">{{ dataTypeLabels[c.data_type] }}</t-tag>
            <span v-if="c.dependencies.length" class="concept-item__deps">
              {{ t('concepts.depCount', { count: c.dependencies.length }) }}
            </span>
          </div>
          <button v-if="canEdit" class="concept-item__del" @click.stop="handleDelete(c)">
            <t-icon name="close" size="12px" />
          </button>
        </div>

        <!-- New concept placeholder -->
        <div v-if="isNew" class="concept-item is-active">
          <div class="concept-item__name" style="opacity: 0.5">
            {{ t('concepts.newPlaceholder') }}
          </div>
        </div>
      </div>

      <!-- Right: edit panel -->
      <div class="concept-editor" v-if="selected || isNew">
        <t-form label-align="top" colon>
          <t-form-item :label="t('concepts.nameLabel')" required>
            <t-input
              v-model="form.name"
              :placeholder="t('concepts.namePlaceholder')"
              :disabled="!isNew"
            />
          </t-form-item>
          <t-form-item :label="t('concepts.typeLabel')">
            <t-select v-model="form.data_type">
              <t-option value="string" :label="t('facts.typeString')" />
              <t-option value="number" :label="t('facts.typeNumber')" />
              <t-option value="boolean" :label="t('facts.typeBoolean')" />
              <t-option value="date" :label="t('facts.typeDate')" />
              <t-option value="object" :label="t('facts.typeObject')" />
            </t-select>
          </t-form-item>
          <t-form-item :label="t('concepts.exprLabel')" required>
            <t-textarea
              v-model="form.expression"
              :rows="4"
              :placeholder="t('concepts.exprPlaceholder')"
              style="font-family: 'JetBrains Mono', monospace; font-size: 12px"
            />
          </t-form-item>
          <t-form-item :label="t('concepts.depsLabel')">
            <t-select
              v-model="form.dependencies"
              multiple
              :options="catalog.allFieldNames.map((n) => ({ label: n, value: n }))"
              :placeholder="t('concepts.depsPlaceholder')"
            />
          </t-form-item>
          <t-form-item :label="t('concepts.descLabel')">
            <t-textarea
              v-model="form.description"
              :rows="2"
              :placeholder="t('concepts.descPlaceholder')"
            />
          </t-form-item>
        </t-form>
        <div class="concept-editor__footer">
          <t-button v-if="canEdit" theme="primary" :loading="saving" @click="handleSave">
            {{ isNew ? t('concepts.createBtn') : t('concepts.saveBtn') }}
          </t-button>
          <t-button
            variant="outline"
            @click="
              selected = null;
              isNew = false;
            "
            >{{ t('concepts.cancel') }}</t-button
          >
        </div>
      </div>

      <div v-else class="concept-placeholder">
        <t-icon name="share" size="36px" style="opacity: 0.15" />
        <p>{{ t('concepts.placeholder') }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.concept-view {
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

.concept-body {
  flex: 1;
  display: flex;
  gap: 16px;
  overflow: hidden;
}

.concept-list {
  width: 240px;
  flex-shrink: 0;
  overflow-y: auto;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
}

.concept-item {
  padding: 10px 12px;
  cursor: pointer;
  border-bottom: 1px solid var(--ordo-border-light);
  position: relative;
}

.concept-item:hover {
  background: var(--ordo-hover-bg);
}
.concept-item.is-active {
  background: var(--ordo-active-bg);
}

.concept-item__name {
  font-size: 13px;
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-primary);
  margin-bottom: 4px;
}

.concept-item__meta {
  display: flex;
  align-items: center;
  gap: 6px;
}

.concept-item__deps {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.concept-item__del {
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

.concept-item:hover .concept-item__del {
  display: flex;
}
.concept-item__del:hover {
  background: rgba(255, 80, 80, 0.15);
  color: #e34d59;
}

.concept-editor {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
}

.concept-editor__footer {
  display: flex;
  gap: 8px;
  margin-top: 16px;
}

.concept-placeholder {
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
