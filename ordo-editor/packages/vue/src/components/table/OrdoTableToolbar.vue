<script setup lang="ts">
/**
 * OrdoTableToolbar - Toolbar for the decision table editor
 * 决策表工具栏
 */
import type { HitPolicy } from '@ordo-engine/editor-core';
import type { SchemaField } from '@ordo-engine/editor-core';
import { useI18n } from '../../locale';

export interface Props {
  hitPolicy: HitPolicy;
  schema?: SchemaField[];
  disabled?: boolean;
  hasSchema?: boolean;
}

withDefaults(defineProps<Props>(), {
  schema: () => [],
  disabled: false,
  hasSchema: false,
});

const emit = defineEmits<{
  addRow: [];
  addInputColumn: [];
  addOutputColumn: [];
  'update:hitPolicy': [value: HitPolicy];
  importFromSchema: [];
  exportJson: [];
  showAsFlow: [];
}>();

const { t } = useI18n();

const hitPolicyOptions: { value: HitPolicy; labelKey: string }[] = [
  { value: 'first', labelKey: 'table.hitPolicyFirst' },
  { value: 'all', labelKey: 'table.hitPolicyAll' },
  { value: 'collect', labelKey: 'table.hitPolicyCollect' },
];

function onHitPolicyChange(e: Event) {
  const value = (e.target as HTMLSelectElement).value as HitPolicy;
  emit('update:hitPolicy', value);
}
</script>

<template>
  <div class="ordo-table-toolbar" :class="{ disabled }">
    <div class="ordo-table-toolbar__left">
      <!-- Add Row -->
      <button
        type="button"
        class="ordo-table-toolbar__btn ordo-table-toolbar__btn--primary"
        :disabled="disabled"
        @click="emit('addRow')"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
        </svg>
        {{ t('table.addRow') }}
      </button>

      <!-- Separator -->
      <span class="ordo-table-toolbar__sep" />

      <!-- Add Input Column -->
      <button
        type="button"
        class="ordo-table-toolbar__btn ordo-table-toolbar__btn--input"
        :disabled="disabled"
        @click="emit('addInputColumn')"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
        </svg>
        {{ t('table.addInputColumn') }}
      </button>

      <!-- Add Output Column -->
      <button
        type="button"
        class="ordo-table-toolbar__btn ordo-table-toolbar__btn--output"
        :disabled="disabled"
        @click="emit('addOutputColumn')"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
        </svg>
        {{ t('table.addOutputColumn') }}
      </button>

      <!-- Import from Schema -->
      <button
        v-if="hasSchema"
        type="button"
        class="ordo-table-toolbar__btn"
        :disabled="disabled"
        @click="emit('importFromSchema')"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="7 10 12 15 17 10" />
          <line x1="12" y1="15" x2="12" y2="3" />
        </svg>
        {{ t('table.importFromSchema') }}
      </button>
    </div>

    <div class="ordo-table-toolbar__right">
      <!-- Hit Policy selector -->
      <div class="ordo-table-toolbar__hit-policy">
        <label class="ordo-table-toolbar__label">{{ t('table.hitPolicy') }}</label>
        <select
          :value="hitPolicy"
          :disabled="disabled"
          class="ordo-table-toolbar__select"
          @change="onHitPolicyChange"
        >
          <option v-for="opt in hitPolicyOptions" :key="opt.value" :value="opt.value">
            {{ t(opt.labelKey) }}
          </option>
        </select>
      </div>

      <!-- Separator -->
      <span class="ordo-table-toolbar__sep" />

      <!-- Export JSON -->
      <button
        type="button"
        class="ordo-table-toolbar__btn"
        :disabled="disabled"
        @click="emit('exportJson')"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="17 8 12 3 7 8" />
          <line x1="12" y1="3" x2="12" y2="15" />
        </svg>
        {{ t('table.exportJson') }}
      </button>

      <!-- Show as Flow -->
      <button
        type="button"
        class="ordo-table-toolbar__btn"
        :disabled="disabled"
        @click="emit('showAsFlow')"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="12 2 22 8.5 22 15.5 12 22 2 15.5 2 8.5 12 2" />
          <line x1="12" y1="22" x2="12" y2="15.5" />
          <polyline points="22 8.5 12 15.5 2 8.5" />
        </svg>
        {{ t('table.showAsFlow') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.ordo-table-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ordo-space-md);
  padding: 8px 12px;
  background: var(--ordo-bg-secondary);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  flex-wrap: wrap;
}

.ordo-table-toolbar.disabled {
  opacity: 0.6;
  pointer-events: none;
}

.ordo-table-toolbar__left,
.ordo-table-toolbar__right {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.ordo-table-toolbar__sep {
  display: block;
  width: 1px;
  height: 20px;
  background: var(--ordo-border-color);
  margin: 0 2px;
}

.ordo-table-toolbar__btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-card);
  color: var(--ordo-text-secondary);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

.ordo-table-toolbar__btn:hover:not(:disabled) {
  border-color: var(--ordo-primary-400);
  color: var(--ordo-primary-600);
  background: var(--ordo-primary-50);
}

.ordo-table-toolbar__btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.ordo-table-toolbar__btn--primary {
  background: var(--ordo-primary-600);
  border-color: var(--ordo-primary-600);
  color: white;
}

.ordo-table-toolbar__btn--primary:hover:not(:disabled) {
  background: var(--ordo-primary-700);
  border-color: var(--ordo-primary-700);
  color: white;
}

.ordo-table-toolbar__btn--input {
  border-color: color-mix(in srgb, var(--ordo-warning) 40%, var(--ordo-border-color));
  color: var(--ordo-warning);
}

.ordo-table-toolbar__btn--input:hover:not(:disabled) {
  background: color-mix(in srgb, var(--ordo-warning) 10%, var(--ordo-bg-card));
  border-color: var(--ordo-warning);
}

.ordo-table-toolbar__btn--output {
  border-color: color-mix(in srgb, var(--ordo-success) 40%, var(--ordo-border-color));
  color: var(--ordo-success);
}

.ordo-table-toolbar__btn--output:hover:not(:disabled) {
  background: color-mix(in srgb, var(--ordo-success) 10%, var(--ordo-bg-card));
  border-color: var(--ordo-success);
}

.ordo-table-toolbar__hit-policy {
  display: flex;
  align-items: center;
  gap: 6px;
}

.ordo-table-toolbar__label {
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-tertiary);
  white-space: nowrap;
}

.ordo-table-toolbar__select {
  appearance: none;
  padding: 4px 24px 4px 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-card);
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-primary);
  cursor: pointer;
  transition: var(--ordo-transition-base);
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23999' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'%3E%3C/polyline%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 6px center;
}

.ordo-table-toolbar__select:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-table-toolbar__select:hover:not(:disabled) {
  border-color: var(--ordo-border-hover);
}
</style>
