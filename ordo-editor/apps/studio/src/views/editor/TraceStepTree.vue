<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { TestExecutionTraceStep } from '@/api/types';

defineOptions({ name: 'TraceStepTree' });

const props = withDefaults(
  defineProps<{
    steps: TestExecutionTraceStep[];
    level?: number;
  }>(),
  {
    level: 0,
  }
);

const emit = defineEmits<{
  (e: 'open-sub-rule', step: TestExecutionTraceStep): void;
}>();

const { t } = useI18n();
const expanded = ref<Set<string>>(new Set());

function stepKey(step: TestExecutionTraceStep, index: number) {
  return `${props.level}:${step.id}:${index}`;
}

function isExpanded(key: string) {
  return expanded.value.has(key);
}

function toggle(key: string) {
  const next = new Set(expanded.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  expanded.value = next;
}

function durationMs(us: number): string {
  return (us / 1000).toFixed(1) + 'ms';
}

function fmtJson(v: unknown): string {
  return JSON.stringify(v, null, 2);
}

function hasDetails(step: TestExecutionTraceStep) {
  return (
    !!step.input_snapshot ||
    !!step.variables_snapshot ||
    !!step.sub_rule_input ||
    !!step.sub_rule_outputs?.length ||
    !!step.sub_rule_frames?.length
  );
}
</script>

<template>
  <div class="trace-tree" :class="{ 'trace-tree--nested': level > 0 }">
    <div v-for="(step, index) in steps" :key="stepKey(step, index)" class="trace-step">
      <div class="trace-step__row" :style="{ '--depth': level }">
        <button
          class="trace-step__toggle"
          :class="{ 'is-hidden': !hasDetails(step) }"
          @click="toggle(stepKey(step, index))"
        >
          <t-icon :name="isExpanded(stepKey(step, index)) ? 'chevron-down' : 'chevron-right'" />
        </button>

        <div class="trace-step__marker" :class="{ 'is-sub-rule': step.sub_rule_ref }">
          {{ index + 1 }}
        </div>

        <div class="trace-step__main">
          <div class="trace-step__title">
            <span class="trace-step__name">{{ step.name }}</span>
            <span class="trace-step__id">{{ step.id }}</span>
          </div>
          <div class="trace-step__meta">
            <span>{{ durationMs(step.duration_us) }}</span>
            <span v-if="step.next_step">→ {{ step.next_step }}</span>
            <span v-if="step.is_terminal" class="trace-chip trace-chip--terminal">
              {{ t('test.trace.terminal') }}
            </span>
            <span v-if="step.sub_rule_ref" class="trace-chip trace-chip--subrule">
              {{ t('test.trace.subRule') }} · {{ step.sub_rule_ref }}
            </span>
          </div>
        </div>

        <button
          v-if="step.sub_rule_ref && step.sub_rule_frames?.length"
          class="trace-step__open"
          @click="emit('open-sub-rule', step)"
        >
          {{ t('test.trace.openSubRule') }}
        </button>
      </div>

      <div v-if="isExpanded(stepKey(step, index))" class="trace-step__details">
        <div v-if="step.sub_rule_input" class="trace-detail">
          <div class="trace-detail__label">{{ t('test.trace.callInput') }}</div>
          <pre>{{ fmtJson(step.sub_rule_input) }}</pre>
        </div>

        <div v-if="step.sub_rule_outputs?.length" class="trace-detail">
          <div class="trace-detail__label">{{ t('test.trace.outputMappings') }}</div>
          <div class="trace-output-list">
            <div
              v-for="output in step.sub_rule_outputs"
              :key="`${output.parent_var}:${output.child_var}`"
              class="trace-output"
              :class="{ 'is-missing': output.missing }"
            >
              <span class="trace-output__map"
                >{{ output.child_var }} → {{ output.parent_var }}</span
              >
              <code v-if="!output.missing">{{ fmtJson(output.value) }}</code>
              <span v-else>{{ t('test.trace.missingOutput') }}</span>
            </div>
          </div>
        </div>

        <div v-if="step.input_snapshot" class="trace-detail">
          <div class="trace-detail__label">{{ t('test.project.inputSnapshot') }}</div>
          <pre>{{ fmtJson(step.input_snapshot) }}</pre>
        </div>

        <div v-if="step.variables_snapshot" class="trace-detail">
          <div class="trace-detail__label">{{ t('test.project.variablesSnapshot') }}</div>
          <pre>{{ fmtJson(step.variables_snapshot) }}</pre>
        </div>

        <div v-if="step.sub_rule_frames?.length" class="trace-detail trace-detail--frames">
          <div class="trace-detail__label">{{ t('test.trace.innerFrames') }}</div>
          <TraceStepTree
            :steps="step.sub_rule_frames"
            :level="level + 1"
            @open-sub-rule="emit('open-sub-rule', $event)"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.trace-tree {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.trace-tree--nested {
  padding-left: 10px;
  border-left: 1px solid rgba(148, 163, 184, 0.2);
}

.trace-step {
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 8px;
  background: rgba(15, 23, 32, 0.36);
  overflow: hidden;
}

.trace-step__row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 9px;
}

.trace-step__toggle {
  width: 18px;
  height: 18px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--ordo-text-tertiary);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
}

.trace-step__toggle:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.trace-step__toggle.is-hidden {
  visibility: hidden;
}

.trace-step__marker {
  width: 22px;
  height: 22px;
  border-radius: 999px;
  background: rgba(99, 102, 241, 0.12);
  color: #818cf8;
  font-size: 10px;
  font-weight: 800;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.trace-step__marker.is-sub-rule {
  background: rgba(74, 138, 89, 0.18);
  color: #9bd39f;
}

.trace-step__main {
  flex: 1;
  min-width: 0;
}

.trace-step__title {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.trace-step__name {
  color: var(--ordo-text-primary);
  font-size: 12px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trace-step__id {
  color: var(--ordo-text-tertiary);
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trace-step__meta {
  margin-top: 3px;
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--ordo-text-tertiary);
  font-size: 10px;
  font-family: 'JetBrains Mono', monospace;
  flex-wrap: wrap;
}

.trace-chip {
  border-radius: 999px;
  padding: 1px 6px;
  font-family: inherit;
}

.trace-chip--terminal {
  background: rgba(99, 102, 241, 0.12);
  color: #818cf8;
}

.trace-chip--subrule {
  background: rgba(74, 138, 89, 0.18);
  color: #9bd39f;
}

.trace-step__open {
  border: 1px solid rgba(126, 191, 132, 0.24);
  background: rgba(74, 138, 89, 0.12);
  color: #9bd39f;
  border-radius: 999px;
  padding: 4px 8px;
  font-size: 11px;
  cursor: pointer;
  flex-shrink: 0;
}

.trace-step__open:hover {
  background: rgba(74, 138, 89, 0.2);
}

.trace-step__details {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 0 10px 10px 57px;
}

.trace-detail {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.trace-detail__label {
  color: var(--ordo-text-tertiary);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.trace-detail pre,
.trace-output code {
  margin: 0;
  padding: 6px 8px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.14);
  color: var(--ordo-text-primary);
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-all;
}

.trace-output-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.trace-output {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 6px 8px;
  border-radius: 6px;
  background: rgba(74, 138, 89, 0.1);
  color: var(--ordo-text-secondary);
  font-size: 11px;
}

.trace-output.is-missing {
  background: rgba(239, 68, 68, 0.1);
  color: #ef4444;
}

.trace-output__map {
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-primary);
  flex-shrink: 0;
}
</style>
