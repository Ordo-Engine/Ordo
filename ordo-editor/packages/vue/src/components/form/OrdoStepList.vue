<script setup lang="ts">
/**
 * OrdoStepList - Business Stage & Step Flow Editor
 * 业务阶段与步骤流程编辑器
 *
 * Steps are organized into Stages (Groups) representing business phases
 * 步骤按照业务阶段（分组）组织
 */
import { ref, computed } from 'vue';
import type { Step, RuleSet, StepGroup } from '@ordo-engine/editor-core';
import {
  Step as StepFactory,
  StepGroup as StepGroupFactory,
  generateId,
  GROUP_COLORS,
} from '@ordo-engine/editor-core';
import OrdoStepEditor from '../step/OrdoStepEditor.vue';
import OrdoIcon from '../icons/OrdoIcon.vue';
import { useI18n } from '../../locale';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';

export interface Props {
  /** RuleSet data */
  modelValue: RuleSet;
  /** Field suggestions for expressions */
  suggestions?: FieldSuggestion[];
  /** Whether the editor is disabled */
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  suggestions: () => [],
  disabled: false,
});

const emit = defineEmits<{
  'update:modelValue': [value: RuleSet];
  change: [value: RuleSet];
}>();

const { t } = useI18n();

// Track expanded groups and steps
const expandedGroups = ref<Set<string>>(new Set(['ungrouped']));
const expandedSteps = ref<Set<string>>(new Set());

// Get groups from ruleset
const groups = computed(() => props.modelValue.groups || []);

// Get steps organized by group
const stepsInGroups = computed(() => {
  const map = new Map<string, Step[]>();
  const groupedStepIds = new Set<string>();

  // Initialize groups
  for (const group of groups.value) {
    map.set(group.id, []);
    for (const stepId of group.stepIds) {
      groupedStepIds.add(stepId);
    }
  }

  // Assign steps to groups
  for (const step of props.modelValue.steps) {
    let found = false;
    for (const group of groups.value) {
      if (group.stepIds.includes(step.id)) {
        map.get(group.id)!.push(step);
        found = true;
        break;
      }
    }
    if (!found) {
      // Ungrouped steps
      if (!map.has('ungrouped')) {
        map.set('ungrouped', []);
      }
      map.get('ungrouped')!.push(step);
    }
  }

  return map;
});

// Get ungrouped steps
const ungroupedSteps = computed(() => stepsInGroups.value.get('ungrouped') || []);

// Toggle group expansion
function toggleGroup(groupId: string) {
  if (expandedGroups.value.has(groupId)) {
    expandedGroups.value.delete(groupId);
  } else {
    expandedGroups.value.add(groupId);
  }
}

function isGroupExpanded(groupId: string) {
  return expandedGroups.value.has(groupId);
}

// Toggle step expansion
function toggleStep(stepId: string) {
  if (expandedSteps.value.has(stepId)) {
    expandedSteps.value.delete(stepId);
  } else {
    expandedSteps.value.add(stepId);
  }
}

function isStepExpanded(stepId: string) {
  return expandedSteps.value.has(stepId);
}

// ============ Group Operations ============

function addGroup() {
  const id = generateId('stage');
  const newGroup = StepGroupFactory.create({
    id,
    name: `Stage ${(groups.value.length || 0) + 1}`,
    description: '',
    color: Object.values(GROUP_COLORS)[groups.value.length % Object.values(GROUP_COLORS).length],
  });

  const newRuleset: RuleSet = {
    ...props.modelValue,
    groups: [...(props.modelValue.groups || []), newGroup],
  };

  expandedGroups.value.add(id);
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

function updateGroup(group: StepGroup) {
  const newGroups = (props.modelValue.groups || []).map((g) => (g.id === group.id ? group : g));
  const newRuleset: RuleSet = { ...props.modelValue, groups: newGroups };
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

function deleteGroup(groupId: string) {
  const newGroups = (props.modelValue.groups || []).filter((g) => g.id !== groupId);
  const newRuleset: RuleSet = { ...props.modelValue, groups: newGroups };
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

// ============ Step Operations ============

function addStepToGroup(type: 'decision' | 'action' | 'terminal', groupId?: string) {
  let newStep: Step;
  const id = generateId('step');

  switch (type) {
    case 'decision':
      newStep = StepFactory.decision({
        id,
        name: t('step.decision'),
        branches: [],
        defaultNextStepId: '',
      });
      break;
    case 'action':
      newStep = StepFactory.action({ id, name: t('step.action'), nextStepId: '' });
      break;
    case 'terminal':
      newStep = StepFactory.terminal({ id, name: t('step.terminal'), code: 'RESULT' });
      break;
  }

  let newGroups = props.modelValue.groups || [];

  // If adding to a group, update group's stepIds
  if (groupId && groupId !== 'ungrouped') {
    newGroups = newGroups.map((g) =>
      g.id === groupId ? { ...g, stepIds: [...g.stepIds, id] } : g
    );
  }

  const newRuleset: RuleSet = {
    ...props.modelValue,
    steps: [...props.modelValue.steps, newStep],
    groups: newGroups,
    startStepId: props.modelValue.startStepId || id,
  };

  expandedSteps.value.add(id);
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

function updateStep(step: Step) {
  const newSteps = props.modelValue.steps.map((s) => (s.id === step.id ? step : s));
  const newRuleset: RuleSet = { ...props.modelValue, steps: newSteps };
  emit('update:modelValue', newRuleset);
}

function handleStepChange(step: Step) {
  const newSteps = props.modelValue.steps.map((s) => (s.id === step.id ? step : s));
  const newRuleset: RuleSet = { ...props.modelValue, steps: newSteps };
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

function deleteStep(stepId: string) {
  const newSteps = props.modelValue.steps.filter((s) => s.id !== stepId);
  // Also remove from groups
  const newGroups = (props.modelValue.groups || []).map((g) => ({
    ...g,
    stepIds: g.stepIds.filter((id) => id !== stepId),
  }));

  const newRuleset: RuleSet = {
    ...props.modelValue,
    steps: newSteps,
    groups: newGroups,
    startStepId: props.modelValue.startStepId === stepId ? '' : props.modelValue.startStepId,
  };
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

function setAsStart(stepId: string) {
  const newRuleset: RuleSet = { ...props.modelValue, startStepId: stepId };
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

function moveStepToGroup(stepId: string, targetGroupId: string) {
  // Remove from all groups first
  let newGroups = (props.modelValue.groups || []).map((g) => ({
    ...g,
    stepIds: g.stepIds.filter((id) => id !== stepId),
  }));

  // Add to target group (if not ungrouped)
  if (targetGroupId !== 'ungrouped') {
    newGroups = newGroups.map((g) =>
      g.id === targetGroupId ? { ...g, stepIds: [...g.stepIds, stepId] } : g
    );
  }

  const newRuleset: RuleSet = { ...props.modelValue, groups: newGroups };
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

// Get step type label
function getTypeLabel(type: string): string {
  switch (type) {
    case 'decision':
      return 'DEC';
    case 'action':
      return 'ACT';
    case 'terminal':
      return 'END';
    default:
      return type.toUpperCase();
  }
}

// Get step summary
function getStepSummary(step: Step): string {
  switch (step.type) {
    case 'decision':
      return `${step.branches?.length || 0} branches`;
    case 'action':
      return `${step.assignments?.length || 0} vars`;
    case 'terminal':
      return step.code || 'END';
    default:
      return '';
  }
}

// Get group color style
function getGroupColorStyle(group: StepGroup) {
  return {
    borderLeftColor: group.color || GROUP_COLORS.gray,
    '--group-color': group.color || GROUP_COLORS.gray,
  };
}
</script>

<template>
  <div class="ordo-step-list" :class="{ disabled }">
    <!-- Header Toolbar -->
    <div class="ordo-step-list__header">
      <div class="header-title">
        <OrdoIcon name="terminal" :size="16" />
        <span>{{ t('flow.steps') }}</span>
        <span class="step-count">{{ modelValue.steps.length }} {{ t('flow.steps') }}</span>
      </div>
      <div class="header-actions">
        <button class="btn-add-stage" @click="addGroup">
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="4 2" />
            <path d="M12 8v8M8 12h8" />
          </svg>
          {{ t('flow.createGroup') }}
        </button>
      </div>
    </div>

    <!-- Stages & Steps -->
    <div class="ordo-step-list__content">
      <!-- User-defined Stages (Groups) -->
      <div
        v-for="group in groups"
        :key="group.id"
        class="stage-container"
        :style="getGroupColorStyle(group)"
      >
        <!-- Stage Header -->
        <div class="stage-header" @click="toggleGroup(group.id)">
          <div class="stage-header-left">
            <OrdoIcon
              :name="isGroupExpanded(group.id) ? 'chevron-down' : 'chevron-right'"
              :size="14"
            />
            <div class="stage-color-dot" :style="{ background: group.color }"></div>
            <input
              class="stage-name-input"
              :value="group.name"
              @input="updateGroup({ ...group, name: ($event.target as HTMLInputElement).value })"
              @click.stop
            />
            <span class="stage-step-count"
              >{{ stepsInGroups.get(group.id)?.length || 0 }} {{ t('flow.steps') }}</span
            >
          </div>
          <div class="stage-header-right" @click.stop>
            <div class="stage-add-buttons">
              <button
                class="btn-add-mini type-decision"
                :title="t('step.decision')"
                @click="addStepToGroup('decision', group.id)"
              >
                <OrdoIcon name="decision" :size="12" />
              </button>
              <button
                class="btn-add-mini type-action"
                :title="t('step.action')"
                @click="addStepToGroup('action', group.id)"
              >
                <OrdoIcon name="action" :size="12" />
              </button>
              <button
                class="btn-add-mini type-terminal"
                :title="t('step.terminal')"
                @click="addStepToGroup('terminal', group.id)"
              >
                <OrdoIcon name="terminal" :size="12" />
              </button>
            </div>
            <button
              class="btn-delete-stage"
              @click="deleteGroup(group.id)"
              :title="t('flow.deleteGroup')"
            >
              <OrdoIcon name="delete" :size="14" />
            </button>
          </div>
        </div>

        <!-- Stage Description -->
        <div v-if="isGroupExpanded(group.id)" class="stage-description">
          <input
            class="stage-desc-input"
            :value="group.description || ''"
            :placeholder="t('common.description') + '...'"
            @input="
              updateGroup({ ...group, description: ($event.target as HTMLInputElement).value })
            "
          />
        </div>

        <!-- Steps in this Stage -->
        <div v-if="isGroupExpanded(group.id)" class="stage-steps">
          <div
            v-for="step in stepsInGroups.get(group.id) || []"
            :key="step.id"
            class="step-item"
            :class="[`type-${step.type}`, { expanded: isStepExpanded(step.id) }]"
          >
            <div class="step-header" @click="toggleStep(step.id)">
              <div class="step-header-left">
                <OrdoIcon
                  :name="isStepExpanded(step.id) ? 'chevron-down' : 'chevron-right'"
                  :size="12"
                />
                <span class="step-type-badge" :class="step.type">{{
                  getTypeLabel(step.type)
                }}</span>
                <span class="step-name">{{ step.name }}</span>
                <span v-if="step.id === modelValue.startStepId" class="start-badge">START</span>
              </div>
              <div class="step-header-right" @click.stop>
                <span class="step-summary">{{ getStepSummary(step) }}</span>
                <select
                  class="move-select"
                  :title="t('flow.moveTo')"
                  @change="moveStepToGroup(step.id, ($event.target as HTMLSelectElement).value)"
                >
                  <option value="">{{ t('flow.moveTo') }}</option>
                  <option value="ungrouped">{{ t('flow.ungroupedSteps') }}</option>
                  <option
                    v-for="g in groups"
                    :key="g.id"
                    :value="g.id"
                    :disabled="g.id === group.id"
                  >
                    {{ g.name }}
                  </option>
                </select>
                <button
                  v-if="step.id !== modelValue.startStepId"
                  class="btn-icon"
                  title="Set as Start"
                  @click="setAsStart(step.id)"
                >
                  <OrdoIcon name="start" :size="12" />
                </button>
                <button class="btn-icon danger" title="Delete" @click="deleteStep(step.id)">
                  <OrdoIcon name="delete" :size="12" />
                </button>
              </div>
            </div>

            <!-- Step Editor (Expanded) -->
            <div v-if="isStepExpanded(step.id)" class="step-body">
              <OrdoStepEditor
                :model-value="step"
                :available-steps="modelValue.steps"
                :suggestions="suggestions"
                :disabled="disabled"
                :show-delete="false"
                @update:model-value="updateStep"
                @change="handleStepChange"
              />
            </div>
          </div>

          <!-- Empty Stage -->
          <div v-if="(stepsInGroups.get(group.id) || []).length === 0" class="stage-empty">
            <span>{{ t('flow.noSteps') }}</span>
            <div class="quick-add">
              <button @click="addStepToGroup('decision', group.id)">
                + {{ t('step.decision') }}
              </button>
              <button @click="addStepToGroup('action', group.id)">+ {{ t('step.action') }}</button>
              <button @click="addStepToGroup('terminal', group.id)">
                + {{ t('step.terminal') }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Ungrouped Steps Section -->
      <div
        v-if="ungroupedSteps.length > 0 || groups.length === 0"
        class="stage-container ungrouped"
      >
        <div class="stage-header" @click="toggleGroup('ungrouped')">
          <div class="stage-header-left">
            <OrdoIcon
              :name="isGroupExpanded('ungrouped') ? 'chevron-down' : 'chevron-right'"
              :size="14"
            />
            <span class="stage-name">{{
              groups.length > 0 ? t('flow.ungroupedSteps') : t('flow.allSteps')
            }}</span>
            <span class="stage-step-count">{{ ungroupedSteps.length }} {{ t('flow.steps') }}</span>
          </div>
          <div class="stage-header-right" @click.stop>
            <div class="stage-add-buttons">
              <button
                class="btn-add-mini type-decision"
                :title="t('step.decision')"
                @click="addStepToGroup('decision', 'ungrouped')"
              >
                <OrdoIcon name="decision" :size="12" />
              </button>
              <button
                class="btn-add-mini type-action"
                :title="t('step.action')"
                @click="addStepToGroup('action', 'ungrouped')"
              >
                <OrdoIcon name="action" :size="12" />
              </button>
              <button
                class="btn-add-mini type-terminal"
                :title="t('step.terminal')"
                @click="addStepToGroup('terminal', 'ungrouped')"
              >
                <OrdoIcon name="terminal" :size="12" />
              </button>
            </div>
          </div>
        </div>

        <div v-if="isGroupExpanded('ungrouped')" class="stage-steps">
          <div
            v-for="step in ungroupedSteps"
            :key="step.id"
            class="step-item"
            :class="[`type-${step.type}`, { expanded: isStepExpanded(step.id) }]"
          >
            <div class="step-header" @click="toggleStep(step.id)">
              <div class="step-header-left">
                <OrdoIcon
                  :name="isStepExpanded(step.id) ? 'chevron-down' : 'chevron-right'"
                  :size="12"
                />
                <span class="step-type-badge" :class="step.type">{{
                  getTypeLabel(step.type)
                }}</span>
                <span class="step-name">{{ step.name }}</span>
                <span v-if="step.id === modelValue.startStepId" class="start-badge">START</span>
              </div>
              <div class="step-header-right" @click.stop>
                <span class="step-summary">{{ getStepSummary(step) }}</span>
                <select
                  v-if="groups.length > 0"
                  class="move-select"
                  title="Move to stage"
                  @change="moveStepToGroup(step.id, ($event.target as HTMLSelectElement).value)"
                >
                  <option value="">Move to...</option>
                  <option v-for="g in groups" :key="g.id" :value="g.id">{{ g.name }}</option>
                </select>
                <button
                  v-if="step.id !== modelValue.startStepId"
                  class="btn-icon"
                  title="Set as Start"
                  @click="setAsStart(step.id)"
                >
                  <OrdoIcon name="start" :size="12" />
                </button>
                <button class="btn-icon danger" title="Delete" @click="deleteStep(step.id)">
                  <OrdoIcon name="delete" :size="12" />
                </button>
              </div>
            </div>

            <div v-if="isStepExpanded(step.id)" class="step-body">
              <OrdoStepEditor
                :model-value="step"
                :available-steps="modelValue.steps"
                :suggestions="suggestions"
                :disabled="disabled"
                :show-delete="false"
                @update:model-value="updateStep"
                @change="handleStepChange"
              />
            </div>
          </div>

          <!-- Empty State -->
          <div v-if="ungroupedSteps.length === 0" class="stage-empty">
            <OrdoIcon name="terminal" :size="24" />
            <span>{{ t('flow.noStepsYet') }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ordo-step-list {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  font-family: var(--ordo-font-sans);
  background: var(--ordo-bg-editor);
  overflow: hidden;
}

/* Header */
.ordo-step-list__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
}

.header-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.step-count {
  font-weight: 400;
  color: var(--ordo-text-tertiary);
  font-size: 11px;
}

.header-actions {
  display: flex;
  gap: 8px;
}

.btn-add-stage {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border: 1px dashed var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: transparent;
  color: var(--ordo-text-secondary);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-add-stage:hover {
  border-color: var(--ordo-accent);
  color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
}

/* Content */
.ordo-step-list__content {
  flex: 1 1 0;
  overflow-y: auto;
  padding: 16px;
}

.ordo-step-list__content > * + * {
  margin-top: 16px;
}

/* Stage Container */
.stage-container {
  border: 1px solid var(--ordo-border-color);
  border-left: 4px solid var(--group-color, var(--ordo-border-color));
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-item);
  overflow: hidden;
}

.stage-container.ungrouped {
  border-left-color: var(--ordo-text-tertiary);
}

/* Stage Header */
.stage-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  cursor: pointer;
  user-select: none;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-light);
}

.stage-header:hover {
  background: var(--ordo-bg-item-hover);
}

.stage-header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.stage-color-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.stage-name-input {
  background: transparent;
  border: none;
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  padding: 2px 4px;
  margin: -2px 0;
  border-radius: 3px;
}

.stage-name-input:hover {
  background: var(--ordo-bg-item);
}

.stage-name-input:focus {
  background: var(--ordo-bg-editor);
  outline: 1px solid var(--ordo-accent);
}

.stage-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.stage-step-count {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  padding: 2px 8px;
  background: var(--ordo-bg-item);
  border-radius: 10px;
}

.stage-header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.stage-add-buttons {
  display: flex;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.15s;
}

.stage-header:hover .stage-add-buttons {
  opacity: 1;
}

.btn-add-mini {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-item);
  cursor: pointer;
  transition: all 0.15s;
}

.btn-add-mini.type-decision:hover {
  background: rgba(183, 110, 0, 0.1);
  border-color: var(--ordo-node-decision);
  color: var(--ordo-node-decision);
}

.btn-add-mini.type-action:hover {
  background: rgba(0, 122, 204, 0.1);
  border-color: var(--ordo-node-action);
  color: var(--ordo-node-action);
}

.btn-add-mini.type-terminal:hover {
  background: rgba(40, 167, 69, 0.1);
  border-color: var(--ordo-node-terminal);
  color: var(--ordo-node-terminal);
}

.btn-delete-stage {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: var(--ordo-radius-sm);
  background: transparent;
  color: var(--ordo-text-tertiary);
  cursor: pointer;
  opacity: 0;
  transition: all 0.15s;
}

.stage-header:hover .btn-delete-stage {
  opacity: 1;
}

.btn-delete-stage:hover {
  background: var(--ordo-error-bg);
  color: var(--ordo-error);
}

/* Stage Description */
.stage-description {
  padding: 0 16px 8px 16px;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-light);
}

.stage-desc-input {
  width: 100%;
  background: transparent;
  border: none;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  padding: 4px;
  margin: -4px;
  border-radius: 3px;
}

.stage-desc-input:hover {
  background: var(--ordo-bg-item);
}

.stage-desc-input:focus {
  background: var(--ordo-bg-editor);
  outline: 1px solid var(--ordo-accent);
}

/* Stage Steps */
.stage-steps {
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

/* Step Item */
.step-item {
  border: 1px solid var(--ordo-border-light);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-editor);
  overflow: hidden;
}

.step-item.expanded {
  border-color: var(--ordo-border-color);
}

.step-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  cursor: pointer;
  user-select: none;
}

.step-header:hover {
  background: var(--ordo-bg-item-hover);
}

.step-header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.step-type-badge {
  font-family: var(--ordo-font-mono);
  font-size: 9px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 3px;
  text-transform: uppercase;
}

.step-type-badge.decision {
  background: rgba(183, 110, 0, 0.15);
  color: #e8a835;
}

.step-type-badge.action {
  background: rgba(0, 122, 204, 0.15);
  color: #3794ff;
}

.step-type-badge.terminal {
  background: rgba(40, 167, 69, 0.15);
  color: #4ec969;
}

.step-name {
  font-size: 12px;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.start-badge {
  font-size: 9px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 3px;
  background: var(--ordo-accent);
  color: #fff;
}

.step-header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.step-summary {
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  font-family: var(--ordo-font-mono);
}

.move-select {
  font-size: 10px;
  padding: 2px 4px;
  border: 1px solid var(--ordo-border-light);
  border-radius: 3px;
  background: var(--ordo-bg-item);
  color: var(--ordo-text-secondary);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s;
}

.step-header:hover .move-select {
  opacity: 1;
}

.btn-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: var(--ordo-radius-sm);
  background: transparent;
  color: var(--ordo-text-tertiary);
  cursor: pointer;
  opacity: 0;
  transition: all 0.15s;
}

.step-header:hover .btn-icon {
  opacity: 1;
}

.btn-icon:hover {
  background: var(--ordo-bg-item);
  color: var(--ordo-text-primary);
}

.btn-icon.danger:hover {
  background: var(--ordo-error-bg);
  color: var(--ordo-error);
}

/* Step Body */
.step-body {
  padding: 12px;
  border-top: 1px solid var(--ordo-border-light);
  background: var(--ordo-bg-item);
}

/* Empty State */
.stage-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px;
  color: var(--ordo-text-tertiary);
  text-align: center;
  gap: 12px;
  font-size: 12px;
}

.quick-add {
  display: flex;
  gap: 8px;
}

.quick-add button {
  padding: 4px 12px;
  border: 1px dashed var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: transparent;
  color: var(--ordo-text-secondary);
  font-size: 11px;
  cursor: pointer;
}

.quick-add button:hover {
  border-color: var(--ordo-accent);
  color: var(--ordo-accent);
}
</style>
