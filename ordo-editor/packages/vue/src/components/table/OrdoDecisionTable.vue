<script setup lang="ts">
/**
 * OrdoDecisionTable - Spreadsheet-style decision table editor
 * 决策表编辑器主组件
 */
import { ref, computed } from 'vue';
import type {
  DecisionTable,
  DecisionTableRow,
  CellValue,
  InputColumn,
  SchemaFieldType,
  HitPolicy,
} from '@ordo-engine/editor-core';
import { DecisionTable as DTFactory, cellValueToString } from '@ordo-engine/editor-core';
import type { SchemaField } from '@ordo-engine/editor-core';
import OrdoTableCellEditor from './OrdoTableCellEditor.vue';
import OrdoTableToolbar from './OrdoTableToolbar.vue';
import { useI18n } from '../../locale';

export interface Props {
  modelValue: DecisionTable;
  schema?: SchemaField[];
  disabled?: boolean;
  traceInput?: Record<string, unknown> | null;
  traceResultCode?: string | null;
  traceOutput?: Record<string, unknown> | null;
}

const props = withDefaults(defineProps<Props>(), {
  schema: () => [],
  disabled: false,
  traceInput: null,
  traceResultCode: null,
  traceOutput: null,
});

const emit = defineEmits<{
  'update:modelValue': [value: DecisionTable];
  change: [value: DecisionTable];
  showAsFlow: [];
}>();

const { t } = useI18n();

// Currently editing cell: { rowId, columnId }
const editingCell = ref<{ rowId: string; columnId: string } | null>(null);
const dragRowId = ref<string | null>(null);
const dropTargetRowId = ref<string | null>(null);

// Column header inline editing
const editingColId = ref<string | null>(null);
const editingColField = ref('');

function startEditCol(colId: string, currentField: string) {
  if (props.disabled) return;
  editingColId.value = colId;
  editingColField.value = currentField;
}

function commitEditCol() {
  const colId = editingColId.value;
  if (!colId) return;
  const field = editingColField.value.trim();
  if (!field) {
    editingColId.value = null;
    return;
  }

  const inputIdx = props.modelValue.inputColumns.findIndex((c) => c.id === colId);
  if (inputIdx !== -1) {
    const cols = [...props.modelValue.inputColumns];
    cols[inputIdx] = { ...cols[inputIdx], fieldPath: field, label: field };
    emitTable({ ...props.modelValue, inputColumns: cols });
  } else {
    const outputIdx = props.modelValue.outputColumns.findIndex((c) => c.id === colId);
    if (outputIdx !== -1) {
      const cols = [...props.modelValue.outputColumns];
      cols[outputIdx] = { ...cols[outputIdx], fieldName: field, label: field };
      emitTable({ ...props.modelValue, outputColumns: cols });
    }
  }
  editingColId.value = null;
}

const allColumns = computed(() => [
  ...props.modelValue.inputColumns.map((c) => ({ ...c, kind: 'input' as const })),
  ...props.modelValue.outputColumns.map((c) => ({
    ...c,
    kind: 'output' as const,
    fieldPath: c.fieldName,
  })),
]);

const hasColumns = computed(() => allColumns.value.length > 0);
const hasRows = computed(() => props.modelValue.rows.length > 0);
const sortedRows = computed(() =>
  [...props.modelValue.rows].sort((a, b) => a.priority - b.priority)
);
const hasTrace = computed(() => !!props.traceInput);

type TraceVerdict = true | false | null;

function normalizePath(path: string): string {
  return path.startsWith('$.') ? path.slice(2) : path.startsWith('$') ? path.slice(1) : path;
}

function resolvePath(input: Record<string, unknown> | null | undefined, path: string): unknown {
  if (!input) return undefined;
  const normalized = normalizePath(path);
  if (!normalized) return input;

  return normalized.split('.').reduce<unknown>((current, segment) => {
    if (current && typeof current === 'object' && segment in (current as Record<string, unknown>)) {
      return (current as Record<string, unknown>)[segment];
    }
    return undefined;
  }, input);
}

function formatTraceValue(value: unknown): string {
  if (value === undefined) return '—';
  if (value === null) return 'null';
  if (typeof value === 'string') return value;
  return JSON.stringify(value);
}

function valuesEqual(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function evaluateCell(cell: CellValue, actual: unknown): { verdict: TraceVerdict; reason: string } {
  switch (cell.type) {
    case 'any':
      return { verdict: true, reason: t('table.traceAnyCell') };
    case 'exact':
      return {
        verdict: valuesEqual(actual, cell.value),
        reason: `${formatTraceValue(actual)} ${
          valuesEqual(actual, cell.value) ? '==' : '!='
        } ${formatTraceValue(cell.value)}`,
      };
    case 'in': {
      const matched = cell.values.some((value) => valuesEqual(actual, value));
      return {
        verdict: matched,
        reason: `${formatTraceValue(actual)} ${matched ? 'in' : 'not in'} [${cell.values
          .map(formatTraceValue)
          .join(', ')}]`,
      };
    }
    case 'range': {
      if (typeof actual !== 'number') {
        return { verdict: false, reason: `${formatTraceValue(actual)} is not a number` };
      }
      const minOk =
        cell.min === undefined ||
        (cell.minInclusive !== false ? actual >= cell.min : actual > cell.min);
      const maxOk =
        cell.max === undefined ||
        (cell.maxInclusive !== false ? actual <= cell.max : actual < cell.max);
      return {
        verdict: minOk && maxOk,
        reason: `${actual} in ${cellValueToString(cell)}`,
      };
    }
    case 'expression':
      return { verdict: null, reason: t('table.traceExprCell') };
  }
}

const traceRowState = computed(() => {
  const traceInput = props.traceInput;
  return sortedRows.value.map((row) => {
    const cellChecks = props.modelValue.inputColumns.map((col) => {
      const cell = getCellValue(row, col.id, 'input');
      const actual = resolvePath(traceInput, col.fieldPath);
      const evaluation = evaluateCell(cell, actual);
      return {
        columnId: col.id,
        actual,
        verdict: evaluation.verdict,
        reason: evaluation.reason,
      };
    });

    const decisiveMismatch = cellChecks.find((cell) => cell.verdict === false);
    const matched = !decisiveMismatch;

    return {
      rowId: row.id,
      matched,
      cellChecks,
      reason: decisiveMismatch?.reason ?? t('table.traceAllMatched'),
    };
  });
});

const traceActiveRowId = computed(
  () => traceRowState.value.find((row) => row.matched)?.rowId ?? null
);

function traceCellState(rowId: string, columnId: string): TraceVerdict {
  return (
    traceRowState.value
      .find((row) => row.rowId === rowId)
      ?.cellChecks.find((cell) => cell.columnId === columnId)?.verdict ?? null
  );
}

function traceCellReason(rowId: string, columnId: string): string {
  return (
    traceRowState.value
      .find((row) => row.rowId === rowId)
      ?.cellChecks.find((cell) => cell.columnId === columnId)?.reason ?? ''
  );
}

function traceCellActual(column: InputColumn): unknown {
  return resolvePath(props.traceInput, column.fieldPath);
}

function traceRowStatus(rowId: string): 'matched' | 'unmatched' | 'unknown' {
  if (!hasTrace.value) return 'unknown';
  const row = traceRowState.value.find((entry) => entry.rowId === rowId);
  if (!row) return 'unknown';
  return row.matched && traceActiveRowId.value === rowId ? 'matched' : 'unmatched';
}

function traceRowReason(rowId: string): string {
  return traceRowState.value.find((entry) => entry.rowId === rowId)?.reason ?? '';
}

function traceOutputValue(fieldName: string): unknown {
  return props.traceOutput?.[fieldName];
}

function traceResultClass(verdict: TraceVerdict): string {
  if (verdict === true) return 'ordo-decision-table__trace-actual--match';
  if (verdict === false) return 'ordo-decision-table__trace-actual--mismatch';
  return 'ordo-decision-table__trace-actual--neutral';
}

// ============================
// Mutation helpers
// ============================

function emitTable(table: DecisionTable) {
  emit('update:modelValue', table);
  emit('change', table);
}

function updateHitPolicy(policy: HitPolicy) {
  emitTable({ ...props.modelValue, hitPolicy: policy });
}

// ---- Row CRUD ----

function addRow() {
  const maxPriority = props.modelValue.rows.reduce((max, r) => Math.max(max, r.priority), 0);
  const row = DTFactory.createRow(maxPriority + 1);

  for (const col of props.modelValue.inputColumns) {
    row.inputValues[col.id] = DTFactory.anyCell();
  }
  for (const col of props.modelValue.outputColumns) {
    row.outputValues[col.id] = DTFactory.anyCell();
  }

  emitTable({ ...props.modelValue, rows: [...props.modelValue.rows, row] });
}

function deleteRow(rowId: string) {
  emitTable({
    ...props.modelValue,
    rows: props.modelValue.rows.filter((r) => r.id !== rowId),
  });
}

function duplicateRow(rowId: string) {
  const source = props.modelValue.rows.find((r) => r.id === rowId);
  if (!source) return;

  const maxPriority = props.modelValue.rows.reduce((max, r) => Math.max(max, r.priority), 0);
  const newRow: DecisionTableRow = {
    ...JSON.parse(JSON.stringify(source)),
    id: DTFactory.createRow(0).id,
    priority: maxPriority + 1,
  };

  emitTable({ ...props.modelValue, rows: [...props.modelValue.rows, newRow] });
}

// ---- Column CRUD ----

function addInputColumn(fieldPath?: string, label?: string, type?: SchemaFieldType) {
  const col = DTFactory.createInputColumn(
    fieldPath || '$.field',
    label || 'New Input',
    type || 'string'
  );

  const rows = props.modelValue.rows.map((r) => ({
    ...r,
    inputValues: { ...r.inputValues, [col.id]: DTFactory.anyCell() },
  }));

  emitTable({
    ...props.modelValue,
    inputColumns: [...props.modelValue.inputColumns, col],
    rows,
  });
}

function addOutputColumn(fieldName?: string, label?: string, type?: SchemaFieldType) {
  const col = DTFactory.createOutputColumn(
    fieldName || 'output',
    label || 'New Output',
    type || 'string'
  );

  const rows = props.modelValue.rows.map((r) => ({
    ...r,
    outputValues: { ...r.outputValues, [col.id]: DTFactory.anyCell() },
  }));

  emitTable({
    ...props.modelValue,
    outputColumns: [...props.modelValue.outputColumns, col],
    rows,
  });
}

function deleteColumn(columnId: string, kind: 'input' | 'output') {
  if (kind === 'input') {
    const rows = props.modelValue.rows.map((r) => {
      const { [columnId]: _, ...rest } = r.inputValues;
      return { ...r, inputValues: rest };
    });
    emitTable({
      ...props.modelValue,
      inputColumns: props.modelValue.inputColumns.filter((c) => c.id !== columnId),
      rows,
    });
  } else {
    const rows = props.modelValue.rows.map((r) => {
      const { [columnId]: _, ...rest } = r.outputValues;
      return { ...r, outputValues: rest };
    });
    emitTable({
      ...props.modelValue,
      outputColumns: props.modelValue.outputColumns.filter((c) => c.id !== columnId),
      rows,
    });
  }
}

function importFromSchema() {
  if (!props.schema || props.schema.length === 0) return;

  const existingPaths = new Set(props.modelValue.inputColumns.map((c) => c.fieldPath));
  const newCols: InputColumn[] = [];

  function flattenFields(fields: SchemaField[], prefix: string) {
    for (const f of fields) {
      const path = prefix ? `${prefix}.${f.name}` : `$.${f.name}`;
      if (f.type === 'object' && f.fields) {
        flattenFields(f.fields, path);
      } else if (!existingPaths.has(path)) {
        newCols.push(DTFactory.createInputColumn(path, f.name, f.type));
      }
    }
  }

  flattenFields(props.schema, '');

  if (newCols.length === 0) return;

  const rows = props.modelValue.rows.map((r) => {
    const extra: Record<string, CellValue> = {};
    for (const col of newCols) extra[col.id] = DTFactory.anyCell();
    return { ...r, inputValues: { ...r.inputValues, ...extra } };
  });

  emitTable({
    ...props.modelValue,
    inputColumns: [...props.modelValue.inputColumns, ...newCols],
    rows,
  });
}

function exportJson() {
  const blob = new Blob([JSON.stringify(props.modelValue, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${props.modelValue.name || 'decision-table'}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

// ---- Cell editing ----

function startEditing(rowId: string, columnId: string) {
  if (props.disabled) return;
  editingCell.value = { rowId, columnId };
}

function stopEditing() {
  editingCell.value = null;
}

function isEditing(rowId: string, columnId: string): boolean {
  return editingCell.value?.rowId === rowId && editingCell.value?.columnId === columnId;
}

function getCellValue(
  row: DecisionTableRow,
  columnId: string,
  kind: 'input' | 'output'
): CellValue {
  const map = kind === 'input' ? row.inputValues : row.outputValues;
  return map[columnId] ?? { type: 'any' };
}

function updateCellValue(
  rowId: string,
  columnId: string,
  kind: 'input' | 'output',
  value: CellValue
) {
  const rows = props.modelValue.rows.map((r) => {
    if (r.id !== rowId) return r;
    if (kind === 'input') {
      return { ...r, inputValues: { ...r.inputValues, [columnId]: value } };
    }
    return { ...r, outputValues: { ...r.outputValues, [columnId]: value } };
  });
  emitTable({ ...props.modelValue, rows });
}

function updateRowField(rowId: string, field: 'resultCode' | 'resultMessage', value: string) {
  const rows = props.modelValue.rows.map((r) =>
    r.id === rowId ? { ...r, [field]: value || undefined } : r
  );
  emitTable({ ...props.modelValue, rows });
}

// ---- Drag-to-reorder ----

function onDragStart(rowId: string) {
  dragRowId.value = rowId;
}

function onDragOver(rowId: string, e: DragEvent) {
  e.preventDefault();
  dropTargetRowId.value = rowId;
}

function onDragLeave() {
  dropTargetRowId.value = null;
}

function onDrop(targetRowId: string) {
  if (!dragRowId.value || dragRowId.value === targetRowId) {
    dragRowId.value = null;
    dropTargetRowId.value = null;
    return;
  }

  const rows = [...props.modelValue.rows];
  const fromIdx = rows.findIndex((r) => r.id === dragRowId.value);
  const toIdx = rows.findIndex((r) => r.id === targetRowId);
  if (fromIdx === -1 || toIdx === -1) return;

  const [moved] = rows.splice(fromIdx, 1);
  rows.splice(toIdx, 0, moved);

  const reindexed = rows.map((r, i) => ({ ...r, priority: i + 1 }));
  emitTable({ ...props.modelValue, rows: reindexed });

  dragRowId.value = null;
  dropTargetRowId.value = null;
}

function onDragEnd() {
  dragRowId.value = null;
  dropTargetRowId.value = null;
}

function cellDisplayText(cell: CellValue): string {
  return cellValueToString(cell);
}

function cellTypeClass(cell: CellValue): string {
  return `cell-type--${cell.type}`;
}
</script>

<template>
  <div class="ordo-decision-table" :class="{ disabled, 'has-trace': hasTrace }">
    <!-- Toolbar -->
    <OrdoTableToolbar
      v-if="!disabled"
      :hit-policy="modelValue.hitPolicy"
      :schema="schema"
      :disabled="disabled"
      :has-schema="!!schema && schema.length > 0"
      @add-row="addRow"
      @add-input-column="addInputColumn()"
      @add-output-column="addOutputColumn()"
      @update:hit-policy="updateHitPolicy"
      @import-from-schema="importFromSchema"
      @export-json="exportJson"
      @show-as-flow="$emit('showAsFlow')"
    />

    <!-- Empty state -->
    <div v-if="!hasColumns" class="ordo-decision-table__empty">
      <svg
        width="48"
        height="48"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
      >
        <rect x="3" y="3" width="18" height="18" rx="2" />
        <line x1="3" y1="9" x2="21" y2="9" />
        <line x1="3" y1="15" x2="21" y2="15" />
        <line x1="9" y1="3" x2="9" y2="21" />
        <line x1="15" y1="3" x2="15" y2="21" />
      </svg>
      <p>{{ t('table.noColumns') }}</p>
    </div>

    <!-- Table -->
    <div v-else class="ordo-decision-table__scroll">
      <table class="ordo-decision-table__table">
        <thead>
          <!-- Section group row -->
          <tr class="ordo-decision-table__group-row">
            <th class="ordo-decision-table__group-spacer"></th>
            <th
              v-if="hasTrace"
              class="ordo-decision-table__group-th ordo-decision-table__group-th--trace"
            >
              {{ t('table.traceStatus') }}
            </th>
            <th
              v-if="modelValue.inputColumns.length > 0"
              :colspan="modelValue.inputColumns.length"
              class="ordo-decision-table__group-th ordo-decision-table__group-th--input"
            >
              {{ t('table.groupInput') }}
            </th>
            <th
              v-if="modelValue.outputColumns.length > 0"
              :colspan="modelValue.outputColumns.length"
              class="ordo-decision-table__group-th ordo-decision-table__group-th--output"
            >
              {{ t('table.groupOutput') }}
            </th>
            <th
              colspan="2"
              class="ordo-decision-table__group-th ordo-decision-table__group-th--result"
            >
              {{ t('table.groupResult') }}
            </th>
            <th v-if="!disabled" class="ordo-decision-table__group-spacer"></th>
          </tr>

          <tr>
            <th class="ordo-decision-table__th ordo-decision-table__th--handle">#</th>
            <th v-if="hasTrace" class="ordo-decision-table__th ordo-decision-table__th--trace">
              {{ t('table.traceStatus') }}
            </th>
            <!-- Input columns -->
            <th
              v-for="col in modelValue.inputColumns"
              :key="col.id"
              class="ordo-decision-table__th ordo-decision-table__th--input"
            >
              <div class="ordo-decision-table__col-header">
                <span class="ordo-decision-table__col-badge ordo-decision-table__col-badge--input"
                  >IN</span
                >
                <span class="ordo-decision-table__col-label">{{ col.label }}</span>
                <span class="ordo-decision-table__col-type">{{ col.type }}</span>
                <button
                  v-if="!disabled"
                  type="button"
                  class="ordo-decision-table__col-delete"
                  :title="t('table.deleteColumn')"
                  @click="deleteColumn(col.id, 'input')"
                >
                  <svg
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                  >
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              </div>
              <div
                class="ordo-decision-table__col-path"
                @click="startEditCol(col.id, col.fieldPath)"
              >
                <input
                  v-if="editingColId === col.id"
                  class="ordo-decision-table__col-input"
                  v-model="editingColField"
                  @blur="commitEditCol"
                  @keydown.enter="commitEditCol"
                  @keydown.esc="editingColId = null"
                  @click.stop
                  autofocus
                />
                <span v-else>{{ col.fieldPath }}</span>
              </div>
            </th>

            <!-- Output columns -->
            <th
              v-for="(col, index) in modelValue.outputColumns"
              :key="col.id"
              class="ordo-decision-table__th ordo-decision-table__th--output"
              :class="{ 'col-group-start--output': index === 0 }"
            >
              <div class="ordo-decision-table__col-header">
                <span class="ordo-decision-table__col-badge ordo-decision-table__col-badge--output"
                  >OUT</span
                >
                <span class="ordo-decision-table__col-label">{{ col.label }}</span>
                <span class="ordo-decision-table__col-type">{{ col.type }}</span>
                <button
                  v-if="!disabled"
                  type="button"
                  class="ordo-decision-table__col-delete"
                  :title="t('table.deleteColumn')"
                  @click="deleteColumn(col.id, 'output')"
                >
                  <svg
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                  >
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              </div>
              <div
                class="ordo-decision-table__col-path"
                @click="startEditCol(col.id, col.fieldName)"
              >
                <input
                  v-if="editingColId === col.id"
                  class="ordo-decision-table__col-input"
                  v-model="editingColField"
                  @blur="commitEditCol"
                  @keydown.enter="commitEditCol"
                  @keydown.esc="editingColId = null"
                  @click.stop
                  autofocus
                />
                <span v-else>{{ col.fieldName }}</span>
              </div>
            </th>

            <!-- Result columns -->
            <th
              class="ordo-decision-table__th ordo-decision-table__th--result col-group-start--result"
            >
              <div class="ordo-decision-table__col-header">
                <span class="ordo-decision-table__col-badge ordo-decision-table__col-badge--result"
                  >CODE</span
                >
                <span class="ordo-decision-table__col-label">{{ t('table.resultCode') }}</span>
              </div>
            </th>
            <th class="ordo-decision-table__th ordo-decision-table__th--result">
              <div class="ordo-decision-table__col-header">
                <span class="ordo-decision-table__col-badge ordo-decision-table__col-badge--result"
                  >MSG</span
                >
                <span class="ordo-decision-table__col-label">{{ t('table.resultMessage') }}</span>
              </div>
            </th>

            <!-- Actions -->
            <th
              v-if="!disabled"
              class="ordo-decision-table__th ordo-decision-table__th--actions"
            ></th>
          </tr>

          <tr v-if="hasTrace" class="ordo-decision-table__trace-input-row">
            <th class="ordo-decision-table__th ordo-decision-table__th--handle">
              {{ t('table.traceInputRow') }}
            </th>
            <th class="ordo-decision-table__th ordo-decision-table__th--trace">
              {{ t('table.traceActual') }}
            </th>
            <th
              v-for="col in modelValue.inputColumns"
              :key="`trace-input-${col.id}`"
              class="ordo-decision-table__th ordo-decision-table__th--input"
            >
              <div
                class="ordo-decision-table__trace-actual ordo-decision-table__trace-actual--neutral"
              >
                {{ formatTraceValue(traceCellActual(col)) }}
              </div>
            </th>
            <th
              v-for="(col, index) in modelValue.outputColumns"
              :key="`trace-output-${col.id}`"
              class="ordo-decision-table__th ordo-decision-table__th--output"
              :class="{ 'col-group-start--output': index === 0 }"
            >
              <div
                class="ordo-decision-table__trace-actual ordo-decision-table__trace-actual--neutral"
              >
                {{ formatTraceValue(traceOutputValue(col.fieldName)) }}
              </div>
            </th>
            <th
              class="ordo-decision-table__th ordo-decision-table__th--result col-group-start--result"
            >
              <div
                class="ordo-decision-table__trace-actual ordo-decision-table__trace-actual--neutral"
              >
                {{ formatTraceValue(traceResultCode) }}
              </div>
            </th>
            <th class="ordo-decision-table__th ordo-decision-table__th--result">—</th>
          </tr>
        </thead>

        <tbody>
          <tr v-if="!hasRows">
            <td
              :colspan="allColumns.length + (hasTrace ? 5 : 4)"
              class="ordo-decision-table__empty-row"
            >
              {{ t('table.noRows') }}
            </td>
          </tr>

          <tr
            v-for="row in sortedRows"
            :key="row.id"
            class="ordo-decision-table__row"
            :class="{
              'ordo-decision-table__row--trace-match': hasTrace && traceActiveRowId === row.id,
              'ordo-decision-table__row--dragging': dragRowId === row.id,
              'ordo-decision-table__row--drop-target':
                dropTargetRowId === row.id && dragRowId !== row.id,
            }"
            :draggable="!disabled"
            @dragstart="onDragStart(row.id)"
            @dragover="onDragOver(row.id, $event)"
            @dragleave="onDragLeave"
            @drop="onDrop(row.id)"
            @dragend="onDragEnd"
          >
            <!-- Priority handle -->
            <td class="ordo-decision-table__td ordo-decision-table__td--handle">
              <span class="ordo-decision-table__drag-handle" :title="t('table.priority')">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
                  <circle cx="9" cy="6" r="1.5" />
                  <circle cx="15" cy="6" r="1.5" />
                  <circle cx="9" cy="12" r="1.5" />
                  <circle cx="15" cy="12" r="1.5" />
                  <circle cx="9" cy="18" r="1.5" />
                  <circle cx="15" cy="18" r="1.5" />
                </svg>
              </span>
              <span class="ordo-decision-table__priority">{{ row.priority }}</span>
            </td>

            <td v-if="hasTrace" class="ordo-decision-table__td ordo-decision-table__td--trace">
              <div
                class="ordo-decision-table__trace-status"
                :class="`ordo-decision-table__trace-status--${traceRowStatus(row.id)}`"
              >
                {{
                  traceRowStatus(row.id) === 'matched'
                    ? t('table.traceMatched')
                    : traceRowStatus(row.id) === 'unmatched'
                      ? t('table.traceNotMatched')
                      : t('table.traceUnknown')
                }}
              </div>
              <div class="ordo-decision-table__trace-reason">
                {{ traceRowReason(row.id) }}
              </div>
            </td>

            <!-- Input cells -->
            <td
              v-for="col in modelValue.inputColumns"
              :key="col.id"
              class="ordo-decision-table__td ordo-decision-table__td--input"
              :class="hasTrace ? traceResultClass(traceCellState(row.id, col.id)) : ''"
              @click="startEditing(row.id, col.id)"
            >
              <OrdoTableCellEditor
                v-if="isEditing(row.id, col.id)"
                :model-value="getCellValue(row, col.id, 'input')"
                :field-type="col.type"
                :disabled="disabled"
                @update:model-value="updateCellValue(row.id, col.id, 'input', $event)"
                @confirm="stopEditing"
                @cancel="stopEditing"
              />
              <div
                v-else
                class="ordo-decision-table__cell-display"
                :class="cellTypeClass(getCellValue(row, col.id, 'input'))"
                :title="hasTrace ? traceCellReason(row.id, col.id) : undefined"
              >
                <div class="ordo-decision-table__cell-stack">
                  <span>{{ cellDisplayText(getCellValue(row, col.id, 'input')) }}</span>
                  <span v-if="hasTrace" class="ordo-decision-table__cell-actual">
                    {{ t('table.traceActual') }}: {{ formatTraceValue(traceCellActual(col)) }}
                  </span>
                </div>
              </div>
            </td>

            <!-- Output cells -->
            <td
              v-for="(col, index) in modelValue.outputColumns"
              :key="col.id"
              class="ordo-decision-table__td ordo-decision-table__td--output"
              :class="{ 'col-group-start--output': index === 0 }"
              @click="startEditing(row.id, col.id)"
            >
              <OrdoTableCellEditor
                v-if="isEditing(row.id, col.id)"
                :model-value="getCellValue(row, col.id, 'output')"
                :field-type="col.type"
                :disabled="disabled"
                @update:model-value="updateCellValue(row.id, col.id, 'output', $event)"
                @confirm="stopEditing"
                @cancel="stopEditing"
              />
              <div
                v-else
                class="ordo-decision-table__cell-display"
                :class="cellTypeClass(getCellValue(row, col.id, 'output'))"
              >
                {{ cellDisplayText(getCellValue(row, col.id, 'output')) }}
              </div>
            </td>

            <!-- Result Code -->
            <td
              class="ordo-decision-table__td ordo-decision-table__td--result col-group-start--result"
            >
              <input
                :value="row.resultCode || ''"
                class="ordo-decision-table__inline-input"
                :disabled="disabled"
                placeholder="CODE"
                @input="
                  updateRowField(row.id, 'resultCode', ($event.target as HTMLInputElement).value)
                "
              />
            </td>

            <!-- Result Message -->
            <td class="ordo-decision-table__td ordo-decision-table__td--result">
              <input
                :value="row.resultMessage || ''"
                class="ordo-decision-table__inline-input"
                :disabled="disabled"
                placeholder="Message"
                @input="
                  updateRowField(row.id, 'resultMessage', ($event.target as HTMLInputElement).value)
                "
              />
            </td>

            <!-- Row actions -->
            <td v-if="!disabled" class="ordo-decision-table__td ordo-decision-table__td--actions">
              <div class="ordo-decision-table__row-actions">
                <button
                  type="button"
                  class="ordo-decision-table__row-btn"
                  :title="t('table.duplicateRow')"
                  @click.stop="duplicateRow(row.id)"
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                  >
                    <rect x="9" y="9" width="13" height="13" rx="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                  </svg>
                </button>
                <button
                  type="button"
                  class="ordo-decision-table__row-btn ordo-decision-table__row-btn--danger"
                  :title="t('table.deleteRow')"
                  @click.stop="deleteRow(row.id)"
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                  >
                    <polyline points="3 6 5 6 21 6" />
                    <path
                      d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                    />
                  </svg>
                </button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.ordo-decision-table {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-sm);
  width: 100%;
  font-size: var(--ordo-font-size-sm);
}

.ordo-decision-table.disabled {
  opacity: 1;
}

.ordo-decision-table__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--ordo-space-md);
  padding: 48px 24px;
  color: var(--ordo-text-tertiary);
  text-align: center;
}

.ordo-decision-table__empty svg {
  opacity: 0.4;
}

.ordo-decision-table__scroll {
  overflow-x: auto;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
}

.ordo-decision-table__table {
  width: 100%;
  border-collapse: collapse;
  table-layout: auto;
}

/* ---- Section group row ---- */

.ordo-decision-table__group-row {
  border-bottom: none;
}

.ordo-decision-table__group-spacer {
  background: var(--ordo-bg-secondary);
  border-bottom: 1px solid var(--ordo-border-color);
  padding: 0;
}

.ordo-decision-table__group-th {
  padding: 5px 12px;
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.6px;
  text-align: center;
  border-bottom: 1px solid var(--ordo-border-color);
}

.ordo-decision-table__group-th--input {
  background: color-mix(in srgb, var(--ordo-warning) 14%, var(--ordo-bg-secondary));
  color: var(--ordo-warning);
  border-right: 3px solid var(--ordo-warning);
}

.ordo-decision-table__group-th--output {
  background: color-mix(in srgb, var(--ordo-success) 14%, var(--ordo-bg-secondary));
  color: var(--ordo-success);
  border-right: 3px solid var(--ordo-success);
}

.ordo-decision-table__group-th--result {
  background: color-mix(in srgb, var(--ordo-info) 14%, var(--ordo-bg-secondary));
  color: var(--ordo-info);
}

.ordo-decision-table__group-th--trace {
  background: color-mix(in srgb, var(--ordo-primary-500, #2563eb) 14%, var(--ordo-bg-secondary));
  color: var(--ordo-primary-500, #2563eb);
  border-right: 3px solid var(--ordo-primary-500, #2563eb);
}

/* ---- Column group separators ---- */

.col-group-start--output {
  border-left: 3px solid color-mix(in srgb, var(--ordo-success) 50%, var(--ordo-border-color)) !important;
}

.col-group-start--result {
  border-left: 3px solid color-mix(in srgb, var(--ordo-info) 50%, var(--ordo-border-color)) !important;
}

/* ---- Header ---- */

.ordo-decision-table__th {
  padding: 8px 12px;
  text-align: left;
  font-weight: 600;
  font-size: 11px;
  white-space: nowrap;
  border-bottom: 2px solid var(--ordo-border-color);
  background: var(--ordo-bg-secondary);
  vertical-align: top;
  position: sticky;
  top: 0;
  z-index: 1;
}

.ordo-decision-table__th--handle {
  width: 56px;
  text-align: center;
}

.ordo-decision-table__th--input {
  background: color-mix(in srgb, var(--ordo-warning) 8%, var(--ordo-bg-secondary));
  border-bottom-color: var(--ordo-warning);
}

.ordo-decision-table__th--output {
  background: color-mix(in srgb, var(--ordo-success) 8%, var(--ordo-bg-secondary));
  border-bottom-color: var(--ordo-success);
}

.ordo-decision-table__th--result {
  min-width: 100px;
  background: color-mix(in srgb, var(--ordo-info) 8%, var(--ordo-bg-secondary));
  border-bottom-color: var(--ordo-info);
}

.ordo-decision-table__th--actions {
  width: 68px;
}

.ordo-decision-table__th--trace {
  min-width: 150px;
  background: color-mix(in srgb, var(--ordo-primary-500, #2563eb) 8%, var(--ordo-bg-secondary));
  border-bottom-color: var(--ordo-primary-500, #2563eb);
}

.ordo-decision-table__col-header {
  display: flex;
  align-items: center;
  gap: 4px;
}

.ordo-decision-table__col-badge {
  font-size: 9px;
  font-weight: 700;
  text-transform: uppercase;
  padding: 1px 4px;
  border-radius: var(--ordo-radius-sm);
  letter-spacing: 0.5px;
}

.ordo-decision-table__col-badge--input {
  background: color-mix(in srgb, var(--ordo-warning) 20%, transparent);
  color: var(--ordo-warning);
}

.ordo-decision-table__col-badge--output {
  background: color-mix(in srgb, var(--ordo-success) 20%, transparent);
  color: var(--ordo-success);
}

.ordo-decision-table__col-badge--result {
  background: color-mix(in srgb, var(--ordo-info) 20%, transparent);
  color: var(--ordo-info);
}

.ordo-decision-table__col-label {
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.ordo-decision-table__col-type {
  font-size: 9px;
  font-weight: 500;
  color: var(--ordo-text-tertiary);
  text-transform: uppercase;
  margin-left: auto;
}

.ordo-decision-table__col-delete {
  padding: 2px;
  border: none;
  background: none;
  color: var(--ordo-text-tertiary);
  cursor: pointer;
  border-radius: var(--ordo-radius-sm);
  opacity: 0;
  transition: all 0.15s;
}

.ordo-decision-table__th:hover .ordo-decision-table__col-delete {
  opacity: 1;
}

.ordo-decision-table__col-delete:hover {
  background: var(--ordo-error-bg);
  color: var(--ordo-error);
}

.ordo-decision-table__col-path {
  font-size: 10px;
  font-family: var(--ordo-font-mono);
  color: var(--ordo-text-tertiary);
  margin-top: 2px;
  cursor: text;
  min-height: 14px;
}

.ordo-decision-table__col-input {
  width: 100%;
  font-size: 10px;
  font-family: var(--ordo-font-mono);
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-input);
  border: 1px solid var(--ordo-border-focus);
  border-radius: 2px;
  padding: 1px 4px;
  outline: none;
  box-sizing: border-box;
}

/* ---- Body ---- */

.ordo-decision-table__empty-row {
  padding: 32px;
  text-align: center;
  color: var(--ordo-text-tertiary);
  font-style: italic;
}

.ordo-decision-table__row {
  transition: background 0.15s;
}

.ordo-decision-table__row:hover {
  background: var(--ordo-bg-secondary);
}

.ordo-decision-table__row--trace-match {
  background: color-mix(in srgb, var(--ordo-success) 9%, transparent);
}

.ordo-decision-table__row--dragging {
  opacity: 0.5;
}

.ordo-decision-table__row--drop-target {
  box-shadow: inset 0 -2px 0 0 var(--ordo-primary-500);
}

.ordo-decision-table__td {
  padding: 4px 8px;
  border-bottom: 1px solid var(--ordo-border-color);
  vertical-align: middle;
  position: relative;
}

.ordo-decision-table__td--handle {
  text-align: center;
  width: 56px;
}

.ordo-decision-table__drag-handle {
  display: inline-flex;
  cursor: grab;
  color: var(--ordo-text-tertiary);
  padding: 2px;
  border-radius: var(--ordo-radius-sm);
  transition: color 0.15s;
}

.ordo-decision-table__drag-handle:hover {
  color: var(--ordo-text-secondary);
}

.ordo-decision-table__priority {
  display: inline-block;
  min-width: 20px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  text-align: center;
}

.ordo-decision-table__td--input {
  background: color-mix(in srgb, var(--ordo-warning) 3%, transparent);
  cursor: pointer;
  min-width: 120px;
}

.ordo-decision-table__td--output {
  background: color-mix(in srgb, var(--ordo-success) 3%, transparent);
  cursor: pointer;
  min-width: 120px;
}

.ordo-decision-table__td--result {
  min-width: 100px;
}

.ordo-decision-table__td--actions {
  width: 68px;
}

.ordo-decision-table__td--trace {
  width: 150px;
  min-width: 150px;
  background: color-mix(in srgb, var(--ordo-primary-500, #2563eb) 3%, transparent);
}

/* ---- Cell display ---- */

.ordo-decision-table__cell-display {
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  padding: 4px 6px;
  border-radius: var(--ordo-radius-sm);
  min-height: 28px;
  display: flex;
  align-items: center;
  transition: background 0.15s;
  word-break: break-word;
}

.ordo-decision-table__cell-stack {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.ordo-decision-table__cell-actual {
  font-size: 10px;
  color: var(--ordo-text-tertiary);
}

.ordo-decision-table__trace-status {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
}

.ordo-decision-table__trace-status--matched {
  color: var(--ordo-success);
  background: color-mix(in srgb, var(--ordo-success) 14%, transparent);
}

.ordo-decision-table__trace-status--unmatched {
  color: var(--ordo-error);
  background: color-mix(in srgb, var(--ordo-error) 12%, transparent);
}

.ordo-decision-table__trace-status--unknown {
  color: var(--ordo-text-tertiary);
  background: var(--ordo-bg-secondary);
}

.ordo-decision-table__trace-reason {
  margin-top: 6px;
  font-size: 11px;
  line-height: 1.4;
  color: var(--ordo-text-tertiary);
}

.ordo-decision-table__trace-input-row .ordo-decision-table__th {
  top: 46px;
  z-index: 1;
}

.ordo-decision-table__trace-actual {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  line-height: 1.45;
  word-break: break-word;
}

.ordo-decision-table__trace-actual--match {
  color: var(--ordo-success);
}

.ordo-decision-table__trace-actual--mismatch {
  color: var(--ordo-error);
}

.ordo-decision-table__trace-actual--neutral {
  color: var(--ordo-text-secondary);
}

.ordo-decision-table__td--input.ordo-decision-table__trace-actual--match {
  background: color-mix(in srgb, var(--ordo-success) 8%, transparent);
}

.ordo-decision-table__td--input.ordo-decision-table__trace-actual--mismatch {
  background: color-mix(in srgb, var(--ordo-error) 7%, transparent);
}

.ordo-decision-table__td:hover .ordo-decision-table__cell-display {
  background: var(--ordo-bg-tertiary);
}

.cell-type--any {
  color: var(--ordo-text-tertiary);
  font-weight: 700;
  font-size: 14px;
}

.cell-type--exact {
  color: var(--ordo-text-primary);
}

.cell-type--range {
  color: var(--ordo-primary-600);
}

.cell-type--in {
  color: var(--ordo-warning);
}

.cell-type--expression {
  color: var(--ordo-info);
  font-style: italic;
}

/* ---- Inline inputs ---- */

.ordo-decision-table__inline-input {
  width: 100%;
  height: 28px;
  padding: 0 6px;
  border: 1px solid transparent;
  border-radius: var(--ordo-radius-sm);
  font-size: 12px;
  font-family: var(--ordo-font-mono);
  background: transparent;
  color: var(--ordo-text-primary);
  transition: var(--ordo-transition-base);
}

.ordo-decision-table__inline-input:hover:not(:disabled) {
  border-color: var(--ordo-border-color);
}

.ordo-decision-table__inline-input:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
  background: var(--ordo-bg-input);
}

/* ---- Row actions ---- */

.ordo-decision-table__row-actions {
  display: flex;
  gap: 2px;
}

.ordo-decision-table__row-btn {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: var(--ordo-text-tertiary);
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
}

.ordo-decision-table__row-btn:hover {
  background: var(--ordo-bg-tertiary);
  color: var(--ordo-text-primary);
}

.ordo-decision-table__row-btn--danger:hover {
  background: var(--ordo-error-bg);
  color: var(--ordo-error);
}
</style>
