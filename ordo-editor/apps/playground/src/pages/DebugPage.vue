<script setup lang="ts">
/**
 * Debug Page - VM Visualization and Expression/RuleSet Debugging
 * VS Code style integrated debugger
 */
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { OrdoIcon, convertToEngineFormat } from '@ordo/editor-vue';
import type { RuleSet } from '@ordo/editor-core';

// Props
const props = defineProps<{
  externalRuleset?: RuleSet | null;
  trigger?: number;
}>();

// Debug mode: 'expression' or 'ruleset'
const debugMode = ref<'expression' | 'ruleset'>('expression');

// Server connection state
const serverEndpoint = ref('http://localhost:8080');
const connectionStatus = ref<'disconnected' | 'connecting' | 'connected' | 'error'>('disconnected');
const isDebugMode = ref(false);
const serverInfo = ref<any>(null);
const errorMessage = ref('');

// Expression mode state
const expression = ref('age > 18 && status == "active"');
const contextJson = ref('{\n  "age": 25,\n  "status": "active"\n}');
const evalResult = ref<any>(null);
const isEvaluating = ref(false);
const activeExprTab = ref<'ast' | 'bytecode' | 'trace'>('ast');

// RuleSet mode state
const rulesetSource = ref<'json' | 'editor' | 'server'>('json');
const rulesetJson = ref('');
const rulesetInputJson = ref('{\n  "user": {\n    "age": 25,\n    "level": "vip"\n  }\n}');
const rulesetResult = ref<any>(null);
const isExecutingRuleset = ref(false);
const activeRulesetTab = ref<'overview' | 'steps' | 'variables' | 'expressions'>('overview');
const availableRulesets = ref<any[]>([]);
const selectedRulesetName = ref('');

// Function to load ruleset from external source
function loadExternalRuleset(ruleset: RuleSet) {
  debugMode.value = 'ruleset';
  rulesetSource.value = 'editor';
  // Convert to engine format (backend-compatible JSON)
  try {
    const engineFormat = convertToEngineFormat(ruleset);
    rulesetJson.value = JSON.stringify(engineFormat, null, 2);
  } catch (e) {
    errorMessage.value =
      e instanceof Error ? e.message : 'Failed to convert ruleset to engine format';
  }
}

// Watch for trigger changes (when Debug button is clicked)
watch(
  () => props.trigger,
  () => {
    if (props.externalRuleset) {
      loadExternalRuleset(props.externalRuleset);
    }
  }
);

// Also watch for initial load
watch(
  () => props.externalRuleset,
  (newRuleset) => {
    if (newRuleset) {
      loadExternalRuleset(newRuleset);
    }
  },
  { immediate: true }
);

// Panel sizes
const leftPanelWidth = ref(280);
const bottomPanelHeight = ref(200);
const showBottomPanel = ref(true);

// Resizing state
const isResizingLeft = ref(false);
const isResizingBottom = ref(false);

function startResizeLeft(e: MouseEvent) {
  isResizingLeft.value = true;
  e.preventDefault();
}

function startResizeBottom(e: MouseEvent) {
  isResizingBottom.value = true;
  e.preventDefault();
}

function handleMouseMove(e: MouseEvent) {
  if (isResizingLeft.value) {
    const newWidth = e.clientX;
    leftPanelWidth.value = Math.max(200, Math.min(500, newWidth));
  }
  if (isResizingBottom.value) {
    const container = document.querySelector('.debug-main');
    if (container) {
      const rect = container.getBoundingClientRect();
      const newHeight = rect.bottom - e.clientY;
      bottomPanelHeight.value = Math.max(100, Math.min(500, newHeight));
    }
  }
}

function handleMouseUp() {
  isResizingLeft.value = false;
  isResizingBottom.value = false;
}

// Connect to server
async function connect() {
  if (connectionStatus.value === 'connecting') return;

  connectionStatus.value = 'connecting';
  errorMessage.value = '';

  try {
    const response = await fetch(`${serverEndpoint.value}/health`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const info = await response.json();
    serverInfo.value = info;
    isDebugMode.value = info.debug_mode;
    connectionStatus.value = 'connected';
  } catch (e) {
    connectionStatus.value = 'error';
    errorMessage.value = e instanceof Error ? e.message : 'Connection failed';
  }
}

function disconnect() {
  connectionStatus.value = 'disconnected';
  serverInfo.value = null;
  isDebugMode.value = false;
  evalResult.value = null;
  rulesetResult.value = null;
}

// Load available rulesets from server
async function loadAvailableRulesets() {
  if (!isDebugMode.value) return;

  try {
    const response = await fetch(`${serverEndpoint.value}/api/v1/rulesets`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    availableRulesets.value = await response.json();
  } catch (e) {
    errorMessage.value = e instanceof Error ? e.message : 'Failed to load rulesets';
  }
}

// Load a specific ruleset by name
async function loadRuleset(name: string) {
  if (!isDebugMode.value) return;

  try {
    const response = await fetch(`${serverEndpoint.value}/api/v1/rulesets/${name}`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const ruleset = await response.json();
    rulesetJson.value = JSON.stringify(ruleset, null, 2);
    selectedRulesetName.value = name;
  } catch (e) {
    errorMessage.value = e instanceof Error ? e.message : 'Failed to load ruleset';
  }
}

// Execute ruleset
async function executeRuleset() {
  if (!isDebugMode.value || isExecutingRuleset.value) return;

  isExecutingRuleset.value = true;
  rulesetResult.value = null;
  errorMessage.value = '';

  try {
    let ruleset = {};
    let input = {};

    // Parse ruleset JSON
    try {
      ruleset = JSON.parse(rulesetJson.value || '{}');
    } catch {
      throw new Error('Invalid RuleSet JSON');
    }

    // Parse input JSON
    try {
      input = JSON.parse(rulesetInputJson.value || '{}');
    } catch {
      throw new Error('Invalid Input JSON');
    }

    // Execute inline
    const response = await fetch(`${serverEndpoint.value}/api/v1/debug/execute-inline`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        ruleset,
        input,
        trace_level: 'full',
      }),
    });

    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.message || `HTTP ${response.status}`);
    }

    rulesetResult.value = await response.json();
  } catch (e) {
    errorMessage.value = e instanceof Error ? e.message : 'Execution failed';
  } finally {
    isExecutingRuleset.value = false;
  }
}

// Watch for debug mode changes to load rulesets
watch([isDebugMode, debugMode], ([debug, mode]) => {
  if (debug && mode === 'ruleset' && rulesetSource.value === 'server') {
    loadAvailableRulesets();
  }
});

// Evaluate expression
async function evaluate() {
  if (!isDebugMode.value || isEvaluating.value) return;

  isEvaluating.value = true;
  evalResult.value = null;

  try {
    let context = {};
    try {
      context = JSON.parse(contextJson.value || '{}');
    } catch {
      throw new Error('Invalid JSON context');
    }

    const response = await fetch(`${serverEndpoint.value}/api/v1/debug/eval`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        expression: expression.value,
        context,
        trace_level: 'full',
      }),
    });

    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.message || `HTTP ${response.status}`);
    }

    evalResult.value = await response.json();
  } catch (e) {
    errorMessage.value = e instanceof Error ? e.message : 'Evaluation failed';
  } finally {
    isEvaluating.value = false;
  }
}

function formatValue(value: any): string {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (typeof value === 'string') return `"${value}"`;
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  return JSON.stringify(value);
}

function formatDuration(ns: number): string {
  if (ns < 1000) return `${ns}ns`;
  if (ns < 1000000) return `${(ns / 1000).toFixed(2)}µs`;
  return `${(ns / 1000000).toFixed(2)}ms`;
}

// Status color
const statusColor = computed(() => {
  switch (connectionStatus.value) {
    case 'connected':
      return isDebugMode.value ? 'var(--ordo-success, #4ec969)' : 'var(--ordo-accent)';
    case 'connecting':
      return 'var(--ordo-warning, #e8a835)';
    case 'error':
      return 'var(--ordo-danger, #e51400)';
    default:
      return 'var(--ordo-text-tertiary)';
  }
});

// Lifecycle hooks
onMounted(() => {
  // Add resize event listeners
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', handleMouseUp);
  // Auto-connect
  connect();
});

onUnmounted(() => {
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', handleMouseUp);
});
</script>

<template>
  <div class="debug-page" :class="{ 'resizing-h': isResizingLeft, 'resizing-v': isResizingBottom }">
    <!-- Left Sidebar: Connection & Context -->
    <aside class="debug-sidebar" :style="{ width: leftPanelWidth + 'px' }">
      <div class="sidebar-header">
        <span>DEBUG</span>
      </div>

      <div class="sidebar-content">
        <!-- Mode Selector -->
        <div class="sidebar-section">
          <div class="section-title">MODE</div>
          <div class="mode-selector">
            <button
              class="mode-btn"
              :class="{ active: debugMode === 'expression' }"
              @click="debugMode = 'expression'"
            >
              Expression
            </button>
            <button
              class="mode-btn"
              :class="{ active: debugMode === 'ruleset' }"
              @click="debugMode = 'ruleset'"
            >
              RuleSet
            </button>
          </div>
        </div>

        <!-- Connection Section -->
        <div class="sidebar-section">
          <div class="section-title">SERVER CONNECTION</div>

          <div class="connection-form">
            <div class="input-row">
              <input
                v-model="serverEndpoint"
                type="text"
                placeholder="http://localhost:8080"
                :disabled="connectionStatus === 'connected'"
                @keyup.enter="connect"
              />
            </div>

            <div class="button-row">
              <button
                v-if="connectionStatus !== 'connected'"
                class="btn primary"
                :disabled="connectionStatus === 'connecting'"
                @click="connect"
              >
                {{ connectionStatus === 'connecting' ? 'Connecting...' : 'Connect' }}
              </button>
              <button v-else class="btn danger" @click="disconnect">Disconnect</button>
            </div>

            <!-- Status -->
            <div class="status-row">
              <span class="status-dot" :style="{ background: statusColor }"></span>
              <span class="status-text">
                {{
                  connectionStatus === 'connected'
                    ? isDebugMode
                      ? 'Debug Mode'
                      : 'Connected (No Debug)'
                    : connectionStatus
                }}
              </span>
            </div>

            <div v-if="errorMessage" class="error-message">{{ errorMessage }}</div>
          </div>
        </div>

        <!-- Server Info -->
        <div v-if="serverInfo" class="sidebar-section">
          <div class="section-title">SERVER INFO</div>
          <div class="info-list">
            <div class="info-row">
              <span class="info-label">Version</span>
              <span class="info-value">{{ serverInfo.version }}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Storage</span>
              <span class="info-value">{{ serverInfo.storage?.mode }}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Rules</span>
              <span class="info-value">{{ serverInfo.storage?.rules_count }}</span>
            </div>
          </div>

          <div v-if="isDebugMode" class="debug-badge">
            <OrdoIcon name="check" :size="12" />
            Debug Mode Active
          </div>
          <div v-else class="warning-badge">
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <path
                d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"
              />
              <line x1="12" y1="9" x2="12" y2="13" />
              <line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
            Start with --debug-mode
          </div>
        </div>

        <!-- RuleSet Source (RuleSet mode only) -->
        <div v-if="debugMode === 'ruleset'" class="sidebar-section">
          <div class="section-title">RULESET SOURCE</div>
          <div class="source-selector">
            <button
              class="source-btn"
              :class="{ active: rulesetSource === 'json' }"
              @click="rulesetSource = 'json'"
            >
              JSON
            </button>
            <button
              class="source-btn"
              :class="{ active: rulesetSource === 'editor' }"
              @click="rulesetSource = 'editor'"
            >
              Editor
            </button>
            <button
              class="source-btn"
              :class="{ active: rulesetSource === 'server' }"
              @click="
                rulesetSource = 'server';
                loadAvailableRulesets();
              "
            >
              Server
            </button>
          </div>

          <!-- Server source: select from list -->
          <div v-if="rulesetSource === 'server'" class="ruleset-list">
            <select
              v-model="selectedRulesetName"
              class="ruleset-select"
              @change="loadRuleset(selectedRulesetName)"
            >
              <option value="">Select a ruleset...</option>
              <option v-for="rs in availableRulesets" :key="rs.name" :value="rs.name">
                {{ rs.name }} (v{{ rs.version }})
              </option>
            </select>
          </div>

          <!-- Editor source: show info or JSON -->
          <div v-if="rulesetSource === 'editor'">
            <div v-if="!rulesetJson" class="info-message">
              Click "Debug Current" in the editor to send the ruleset here
            </div>
            <div v-else class="info-message success">
              ✓ Ruleset loaded from editor ({{ Math.round(rulesetJson.length / 1024) }}KB)
            </div>
          </div>
        </div>

        <!-- RuleSet JSON (RuleSet mode with JSON/Server/Editor source) -->
        <div
          v-if="debugMode === 'ruleset' && rulesetJson && rulesetSource !== 'json'"
          class="sidebar-section"
        >
          <div class="section-title">RULESET JSON (READ-ONLY)</div>
          <textarea
            :value="rulesetJson"
            class="context-input"
            placeholder='{"config": {...}, "steps": {...}}'
            rows="10"
            readonly
          ></textarea>
        </div>

        <!-- RuleSet JSON (RuleSet mode with JSON source - editable) -->
        <div v-if="debugMode === 'ruleset' && rulesetSource === 'json'" class="sidebar-section">
          <div class="section-title">RULESET JSON</div>
          <textarea
            v-model="rulesetJson"
            class="context-input"
            placeholder='{"config": {...}, "steps": {...}}'
            rows="10"
          ></textarea>
        </div>

        <!-- Input Data (RuleSet mode) -->
        <div v-if="debugMode === 'ruleset'" class="sidebar-section">
          <div class="section-title">INPUT (JSON)</div>
          <textarea
            v-model="rulesetInputJson"
            class="context-input"
            placeholder='{"user": {"age": 25}}'
            rows="6"
          ></textarea>
        </div>

        <!-- Context Section (Expression mode only) -->
        <div v-if="debugMode === 'expression'" class="sidebar-section">
          <div class="section-title">CONTEXT (JSON)</div>
          <textarea
            v-model="contextJson"
            class="context-input"
            placeholder='{"key": "value"}'
            rows="8"
          ></textarea>
        </div>
      </div>

      <!-- Resize handle -->
      <div class="resize-handle right" @mousedown="startResizeLeft"></div>
    </aside>

    <!-- Main Editor Area -->
    <main class="debug-main">
      <!-- Expression Input Bar (Expression mode) -->
      <div v-if="debugMode === 'expression'" class="expression-bar">
        <div class="expression-label">Expression:</div>
        <input
          v-model="expression"
          type="text"
          class="expression-input"
          placeholder="age > 18 && status == 'active'"
          @keyup.enter="evaluate"
        />
        <button class="eval-btn" :disabled="!isDebugMode || isEvaluating" @click="evaluate">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
            <path d="M8 5v14l11-7z" />
          </svg>
          {{ isEvaluating ? 'Running...' : 'Evaluate' }}
        </button>
      </div>

      <!-- RuleSet Execute Bar (RuleSet mode) -->
      <div v-if="debugMode === 'ruleset'" class="expression-bar">
        <div class="expression-label">RuleSet:</div>
        <div class="ruleset-info">
          {{ rulesetJson ? 'Ready' : 'Enter RuleSet JSON' }}
        </div>
        <button
          class="eval-btn"
          :disabled="!isDebugMode || isExecutingRuleset || !rulesetJson"
          @click="executeRuleset"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
            <path d="M8 5v14l11-7z" />
          </svg>
          {{ isExecutingRuleset ? 'Executing...' : 'Execute' }}
        </button>
      </div>

      <!-- Expression Result Header -->
      <div v-if="debugMode === 'expression' && evalResult" class="result-header">
        <div class="result-value">
          <span class="label">Result:</span>
          <span class="value" :class="typeof evalResult.result">{{
            formatValue(evalResult.result)
          }}</span>
        </div>
        <div class="timing-info">
          <span>Parse: {{ formatDuration(evalResult.parse_duration_ns || 0) }}</span>
          <span v-if="evalResult.compile_duration_ns"
            >Compile: {{ formatDuration(evalResult.compile_duration_ns) }}</span
          >
          <span>Eval: {{ formatDuration(evalResult.eval_duration_ns || 0) }}</span>
        </div>
      </div>

      <!-- RuleSet Result Header -->
      <div v-if="debugMode === 'ruleset' && rulesetResult" class="result-header">
        <div class="result-value">
          <span class="label">Code:</span>
          <span class="value">{{ rulesetResult.result.code }}</span>
        </div>
        <div class="timing-info">
          <span>Duration: {{ (rulesetResult.result.duration_us / 1000).toFixed(2) }}ms</span>
          <span v-if="rulesetResult.rule_trace"
            >Steps: {{ rulesetResult.rule_trace.steps.length }}</span
          >
        </div>
      </div>

      <!-- Expression Mode Tabs -->
      <div v-if="debugMode === 'expression'" class="debug-tabs">
        <div
          class="tab"
          :class="{ active: activeExprTab === 'ast' }"
          @click="activeExprTab = 'ast'"
        >
          AST
        </div>
        <div
          class="tab"
          :class="{ active: activeExprTab === 'bytecode' }"
          @click="activeExprTab = 'bytecode'"
        >
          Bytecode
        </div>
        <div
          class="tab"
          :class="{ active: activeExprTab === 'trace' }"
          @click="activeExprTab = 'trace'"
        >
          Trace
          <span v-if="evalResult?.eval_steps" class="tab-badge">{{
            evalResult.eval_steps.length
          }}</span>
        </div>
      </div>

      <!-- RuleSet Mode Tabs -->
      <div v-if="debugMode === 'ruleset'" class="debug-tabs">
        <div
          class="tab"
          :class="{ active: activeRulesetTab === 'overview' }"
          @click="activeRulesetTab = 'overview'"
        >
          Overview
        </div>
        <div
          class="tab"
          :class="{ active: activeRulesetTab === 'steps' }"
          @click="activeRulesetTab = 'steps'"
        >
          Steps
          <span v-if="rulesetResult?.rule_trace?.steps" class="tab-badge">{{
            rulesetResult.rule_trace.steps.length
          }}</span>
        </div>
        <div
          class="tab"
          :class="{ active: activeRulesetTab === 'variables' }"
          @click="activeRulesetTab = 'variables'"
        >
          Variables
        </div>
        <div
          class="tab"
          :class="{ active: activeRulesetTab === 'expressions' }"
          @click="activeRulesetTab = 'expressions'"
        >
          Expressions
        </div>
      </div>

      <!-- Tab Content - Expression Mode -->
      <div v-if="debugMode === 'expression'" class="tab-content">
        <!-- AST Tab -->
        <div v-if="activeExprTab === 'ast'" class="ast-panel">
          <div v-if="evalResult?.ast" class="ast-tree">
            <pre>{{ JSON.stringify(evalResult.ast, null, 2) }}</pre>
          </div>
          <div v-else class="empty-state">
            <OrdoIcon name="terminal" :size="32" />
            <p>Evaluate an expression to see the AST</p>
          </div>
        </div>

        <!-- Bytecode Tab -->
        <div v-if="activeExprTab === 'bytecode'" class="bytecode-panel">
          <div v-if="evalResult?.bytecode" class="bytecode-content">
            <div class="bytecode-stats">
              <span>{{ evalResult.bytecode.instruction_count }} instructions</span>
              <span>{{ evalResult.bytecode.constant_count }} constants</span>
              <span>{{ evalResult.bytecode.field_count }} fields</span>
            </div>
            <div class="instruction-list">
              <div
                v-for="(inst, idx) in evalResult.bytecode.instructions"
                :key="idx"
                class="instruction-item"
              >
                <span class="inst-idx">{{ idx.toString().padStart(3, '0') }}</span>
                <span class="inst-text">{{ inst }}</span>
              </div>
            </div>
          </div>
          <div v-else class="empty-state">
            <OrdoIcon name="action" :size="32" />
            <p>Evaluate an expression to see bytecode</p>
          </div>
        </div>

        <!-- Trace Tab -->
        <div v-if="activeExprTab === 'trace'" class="trace-panel">
          <div v-if="evalResult?.eval_steps?.length" class="trace-list">
            <div v-for="step in evalResult.eval_steps" :key="step.step" class="trace-item">
              <span class="step-num">{{ step.step }}</span>
              <span class="step-desc">{{ step.description }}</span>
              <span class="step-result">→ {{ formatValue(step.result) }}</span>
            </div>
          </div>
          <div v-else class="empty-state">
            <OrdoIcon name="decision" :size="32" />
            <p>Evaluate an expression to see the trace</p>
          </div>
        </div>
      </div>

      <!-- Tab Content - RuleSet Mode -->
      <div v-if="debugMode === 'ruleset'" class="tab-content">
        <!-- Overview Tab -->
        <div v-if="activeRulesetTab === 'overview'" class="ruleset-panel">
          <div v-if="rulesetResult" class="overview-content">
            <div class="overview-section">
              <h3>Execution Result</h3>
              <div class="info-grid">
                <div class="info-item">
                  <span class="info-label">Code:</span>
                  <span class="info-value">{{ rulesetResult.result.code }}</span>
                </div>
                <div class="info-item">
                  <span class="info-label">Message:</span>
                  <span class="info-value">{{ rulesetResult.result.message }}</span>
                </div>
                <div class="info-item">
                  <span class="info-label">Duration:</span>
                  <span class="info-value"
                    >{{ (rulesetResult.result.duration_us / 1000).toFixed(2) }}ms</span
                  >
                </div>
              </div>
            </div>
            <div class="overview-section">
              <h3>Output</h3>
              <pre class="output-json">{{
                JSON.stringify(rulesetResult.result.output, null, 2)
              }}</pre>
            </div>
          </div>
          <div v-else class="empty-state">
            <OrdoIcon name="terminal" :size="32" />
            <p>Execute a ruleset to see the overview</p>
          </div>
        </div>

        <!-- Steps Tab -->
        <div v-if="activeRulesetTab === 'steps'" class="ruleset-panel">
          <div v-if="rulesetResult?.rule_trace?.steps?.length" class="steps-list">
            <div
              v-for="(step, idx) in rulesetResult.rule_trace.steps"
              :key="step.id"
              class="step-trace-item"
            >
              <div class="step-header">
                <span class="step-number">{{ (idx as number) + 1 }}</span>
                <span class="step-name">{{ step.name || step.id }}</span>
                <span class="step-duration">{{ (step.duration_us / 1000).toFixed(2) }}ms</span>
              </div>
              <div class="step-details">
                <span class="step-type">Type: {{ step.step_type }}</span>
              </div>
            </div>
          </div>
          <div v-else class="empty-state">
            <OrdoIcon name="decision" :size="32" />
            <p>Execute a ruleset to see step trace</p>
          </div>
        </div>

        <!-- Variables Tab -->
        <div v-if="activeRulesetTab === 'variables'" class="ruleset-panel">
          <div v-if="rulesetResult?.rule_trace?.variables" class="variables-content">
            <div
              v-if="Object.keys(rulesetResult.rule_trace.variables).length > 0"
              class="variable-list"
            >
              <div
                v-for="(value, name) in rulesetResult.rule_trace.variables"
                :key="name"
                class="variable-item"
              >
                <span class="var-name">{{ name }}</span>
                <span class="var-value">{{ formatValue(value) }}</span>
              </div>
            </div>
            <div v-else class="info-message">No variables were set during execution</div>
          </div>
          <div v-else class="empty-state">
            <OrdoIcon name="action" :size="32" />
            <p>Execute a ruleset to see variables</p>
          </div>
        </div>

        <!-- Expressions Tab -->
        <div v-if="activeRulesetTab === 'expressions'" class="ruleset-panel">
          <div v-if="rulesetResult?.expr_traces?.length" class="expr-traces-list">
            <div
              v-for="(trace, idx) in rulesetResult.expr_traces"
              :key="idx"
              class="expr-trace-item"
            >
              <pre>{{ JSON.stringify(trace, null, 2) }}</pre>
            </div>
          </div>
          <div v-else class="empty-state">
            <OrdoIcon name="terminal" :size="32" />
            <p>Expression traces will appear here</p>
          </div>
        </div>
      </div>

      <!-- Bottom Panel: Console/Output -->
      <div
        v-if="showBottomPanel"
        class="bottom-panel"
        :style="{ height: bottomPanelHeight + 'px' }"
      >
        <!-- Resize handle for bottom panel -->
        <div class="resize-handle top" @mousedown="startResizeBottom"></div>
        <div class="panel-header">
          <span>OUTPUT</span>
          <button class="panel-close" @click="showBottomPanel = false">
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
        <div class="panel-content">
          <div v-if="evalResult" class="output-json">
            <pre>{{ JSON.stringify(evalResult, null, 2) }}</pre>
          </div>
          <div v-else class="output-placeholder">
            No output yet. Evaluate an expression to see results.
          </div>
        </div>
      </div>
    </main>
  </div>
</template>

<style scoped>
.debug-page {
  display: flex;
  height: 100%;
  width: 100%;
  background: var(--ordo-bg-editor);
  color: var(--ordo-text-primary);
  font-family: var(--ordo-font-sans);
}

.debug-page.resizing-h {
  cursor: col-resize;
  user-select: none;
}

.debug-page.resizing-v {
  cursor: row-resize;
  user-select: none;
}

/* Sidebar */
.debug-sidebar {
  background: var(--ordo-bg-panel);
  border-right: 1px solid var(--ordo-border-color);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  position: relative;
}

.sidebar-header {
  padding: 8px 12px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  letter-spacing: 0.5px;
  border-bottom: 1px solid var(--ordo-border-color);
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
}

.sidebar-section {
  padding: 8px 0;
  border-bottom: 1px solid var(--ordo-border-light);
}

.section-title {
  padding: 4px 12px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  letter-spacing: 0.5px;
}

/* Connection Form */
.connection-form {
  padding: 8px 12px;
}

.input-row {
  margin-bottom: 8px;
}

.input-row input {
  width: 100%;
  padding: 6px 10px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-primary);
  font-size: 12px;
  font-family: var(--ordo-font-mono);
}

.input-row input:focus {
  outline: none;
  border-color: var(--ordo-accent);
}

.input-row input:disabled {
  opacity: 0.6;
}

.button-row {
  margin-bottom: 8px;
}

.btn {
  width: 100%;
  padding: 6px 12px;
  border: none;
  border-radius: var(--ordo-radius-sm);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.btn.primary {
  background: var(--ordo-accent);
  color: #fff;
}

.btn.primary:hover:not(:disabled) {
  opacity: 0.9;
}

.btn.primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn.danger {
  background: var(--ordo-danger, #e51400);
  color: #fff;
}

.btn.danger:hover {
  opacity: 0.9;
}

.status-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--ordo-text-secondary);
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.error-message {
  margin-top: 8px;
  padding: 6px 8px;
  background: rgba(229, 20, 0, 0.1);
  border-radius: var(--ordo-radius-sm);
  font-size: 11px;
  color: var(--ordo-danger, #e51400);
}

/* Info List */
.info-list {
  padding: 4px 12px;
}

.info-row {
  display: flex;
  justify-content: space-between;
  padding: 2px 0;
  font-size: 11px;
}

.info-label {
  color: var(--ordo-text-tertiary);
}

.info-value {
  color: var(--ordo-text-secondary);
  font-family: var(--ordo-font-mono);
}

.debug-badge {
  margin: 8px 12px;
  padding: 6px 10px;
  background: rgba(78, 201, 105, 0.15);
  border: 1px solid var(--ordo-success, #4ec969);
  border-radius: var(--ordo-radius-sm);
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-success, #4ec969);
  display: flex;
  align-items: center;
  gap: 6px;
}

.warning-badge {
  margin: 8px 12px;
  padding: 6px 10px;
  background: rgba(232, 168, 53, 0.15);
  border: 1px solid var(--ordo-warning, #e8a835);
  border-radius: var(--ordo-radius-sm);
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-warning, #e8a835);
  display: flex;
  align-items: center;
  gap: 6px;
}

/* Context Input */
.context-input {
  width: calc(100% - 24px);
  margin: 8px 12px;
  padding: 8px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-primary);
  font-size: 11px;
  font-family: var(--ordo-font-mono);
  resize: vertical;
}

.context-input:focus {
  outline: none;
  border-color: var(--ordo-accent);
}

/* Main Area */
.debug-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}

/* Expression Bar */
.expression-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-color);
}

.expression-label {
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-tertiary);
  white-space: nowrap;
}

.expression-input {
  flex: 1;
  padding: 6px 10px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-primary);
  font-size: 12px;
  font-family: var(--ordo-font-mono);
}

.expression-input:focus {
  outline: none;
  border-color: var(--ordo-accent);
}

.eval-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--ordo-success, #4ec969);
  border: none;
  border-radius: var(--ordo-radius-sm);
  color: #fff;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.eval-btn:hover:not(:disabled) {
  opacity: 0.9;
}

.eval-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Result Header */
.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: var(--ordo-bg-selected);
  border-bottom: 1px solid var(--ordo-border-color);
}

.result-value {
  display: flex;
  align-items: center;
  gap: 8px;
}

.result-value .label {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.result-value .value {
  font-size: 13px;
  font-weight: 600;
  font-family: var(--ordo-font-mono);
}

.result-value .value.boolean {
  color: var(--ordo-node-action, #007acc);
}

.result-value .value.number {
  color: var(--ordo-warning, #e8a835);
}

.result-value .value.string {
  color: var(--ordo-success, #4ec969);
}

.timing-info {
  display: flex;
  gap: 12px;
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  font-family: var(--ordo-font-mono);
}

/* Tabs */
.debug-tabs {
  display: flex;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-color);
}

.tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.15s;
}

.tab:hover {
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-item-hover);
}

.tab.active {
  color: var(--ordo-text-primary);
  border-bottom-color: var(--ordo-accent);
}

.tab-badge {
  font-size: 10px;
  padding: 1px 5px;
  background: var(--ordo-accent);
  color: #fff;
  border-radius: 8px;
}

/* Tab Content */
.tab-content {
  flex: 1;
  overflow: auto;
  background: var(--ordo-bg-editor);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  min-height: 200px;
  color: var(--ordo-text-tertiary);
  gap: 12px;
}

.empty-state p {
  font-size: 13px;
}

/* AST Panel */
.ast-panel {
  height: 100%;
}

.ast-tree {
  padding: 12px;
}

.ast-tree pre {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  white-space: pre-wrap;
}

/* Bytecode Panel */
.bytecode-panel {
  height: 100%;
}

.bytecode-content {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.bytecode-stats {
  display: flex;
  gap: 16px;
  padding: 8px 12px;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-color);
  font-size: 10px;
  color: var(--ordo-text-tertiary);
}

.instruction-list {
  flex: 1;
  overflow-y: auto;
}

.instruction-item {
  display: flex;
  gap: 12px;
  padding: 4px 12px;
  font-size: 11px;
  font-family: var(--ordo-font-mono);
  border-left: 2px solid transparent;
  transition: all 0.15s;
}

.instruction-item:hover {
  background: var(--ordo-bg-item-hover);
  border-left-color: var(--ordo-accent);
}

.inst-idx {
  color: var(--ordo-text-tertiary);
  min-width: 24px;
}

.inst-text {
  color: var(--ordo-text-primary);
}

/* Trace Panel */
.trace-panel {
  height: 100%;
}

.trace-list {
  padding: 8px 0;
}

.trace-item {
  display: grid;
  grid-template-columns: 32px 1fr auto;
  gap: 12px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border-light);
  transition: background 0.15s;
}

.trace-item:hover {
  background: var(--ordo-bg-item-hover);
}

.step-num {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--ordo-bg-item);
  border-radius: 50%;
  font-size: 10px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
}

.step-desc {
  font-size: 12px;
  font-family: var(--ordo-font-mono);
  color: var(--ordo-text-primary);
}

.step-result {
  font-size: 12px;
  font-family: var(--ordo-font-mono);
  color: var(--ordo-success, #4ec969);
}

/* Bottom Panel */
.bottom-panel {
  border-top: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  display: flex;
  flex-direction: column;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 12px;
  background: var(--ordo-bg-item);
  border-bottom: 1px solid var(--ordo-border-color);
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
}

.panel-close {
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  padding: 2px;
  border-radius: 3px;
}

.panel-close:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.panel-content {
  flex: 1;
  overflow: auto;
  padding: 8px 12px;
}

.output-json pre {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  white-space: pre-wrap;
}

.output-placeholder {
  color: var(--ordo-text-tertiary);
  font-size: 12px;
  font-style: italic;
}

/* Resize handles */
.resize-handle {
  position: absolute;
  z-index: 100;
  transition: background 0.15s;
}

.resize-handle:hover,
.resize-handle:active {
  background: var(--ordo-accent);
}

.resize-handle.right {
  top: 0;
  bottom: 0;
  right: -2px;
  width: 4px;
  cursor: col-resize;
}

.resize-handle.top {
  left: 0;
  right: 0;
  top: -2px;
  height: 4px;
  cursor: row-resize;
}

/* Bottom panel needs relative positioning for resize handle */
.bottom-panel {
  position: relative;
}

/* Mode Selector */
.mode-selector {
  display: flex;
  gap: 4px;
  padding: 8px 12px;
}

.mode-btn {
  flex: 1;
  padding: 6px 12px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-secondary);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.mode-btn:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.mode-btn.active {
  background: var(--ordo-accent);
  color: #fff;
  border-color: var(--ordo-accent);
}

/* Source Selector */
.source-selector {
  display: flex;
  gap: 4px;
  padding: 8px 12px;
}

.source-btn {
  flex: 1;
  padding: 4px 8px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-secondary);
  font-size: 10px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.source-btn:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.source-btn.active {
  background: var(--ordo-bg-selected);
  color: var(--ordo-text-primary);
  border-color: var(--ordo-accent);
}

/* RuleSet List */
.ruleset-list {
  padding: 8px 12px;
}

.ruleset-select {
  width: 100%;
  padding: 6px 10px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-primary);
  font-size: 12px;
  font-family: var(--ordo-font-mono);
}

.ruleset-select:focus {
  outline: none;
  border-color: var(--ordo-accent);
}

/* Info Message */
.info-message {
  padding: 8px 12px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  font-style: italic;
}

.info-message.success {
  color: #4ec9b0;
  font-weight: 500;
  font-style: normal;
}

/* RuleSet Info */
.ruleset-info {
  flex: 1;
  padding: 6px 10px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-secondary);
  font-size: 12px;
  font-family: var(--ordo-font-mono);
}

/* RuleSet Panel */
.ruleset-panel {
  height: 100%;
  overflow-y: auto;
}

.overview-content {
  padding: 16px;
}

.overview-section {
  margin-bottom: 24px;
}

.overview-section h3 {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin: 0 0 12px 0;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--ordo-border-color);
}

.info-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
}

.info-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.info-label {
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.info-value {
  font-size: 13px;
  color: var(--ordo-text-primary);
  font-family: var(--ordo-font-mono);
}

.output-json {
  padding: 12px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  white-space: pre-wrap;
  overflow-x: auto;
}

/* Steps List */
.steps-list {
  padding: 8px 0;
}

.step-trace-item {
  padding: 12px 16px;
  border-bottom: 1px solid var(--ordo-border-light);
  transition: background 0.15s;
}

.step-trace-item:hover {
  background: var(--ordo-bg-item-hover);
}

.step-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 4px;
}

.step-number {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--ordo-bg-item);
  border-radius: 50%;
  font-size: 10px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
}

.step-name {
  flex: 1;
  font-size: 12px;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.step-duration {
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  font-family: var(--ordo-font-mono);
}

.step-details {
  margin-left: 36px;
  font-size: 11px;
  color: var(--ordo-text-secondary);
}

.step-type {
  padding: 2px 6px;
  background: var(--ordo-bg-item);
  border-radius: 3px;
  font-family: var(--ordo-font-mono);
}

/* Variables Content */
.variables-content {
  padding: 16px;
}

.variable-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.variable-item {
  display: flex;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
}

.var-name {
  font-size: 12px;
  font-weight: 500;
  color: var(--ordo-text-primary);
  font-family: var(--ordo-font-mono);
}

.var-value {
  font-size: 12px;
  color: var(--ordo-success, #4ec969);
  font-family: var(--ordo-font-mono);
}

/* Expression Traces */
.expr-traces-list {
  padding: 16px;
}

.expr-trace-item {
  margin-bottom: 16px;
  padding: 12px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
}

.expr-trace-item pre {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  white-space: pre-wrap;
}
</style>
