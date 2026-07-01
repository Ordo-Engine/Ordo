<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useAiStore, type ToolActivity } from '@/stores/ai-agent';
import { renderMarkdown } from '@/utils/ai-markdown';

const props = defineProps<{ orgId: string; projectId: string }>();
const emit = defineEmits<{ (e: 'close'): void }>();

const { t } = useI18n();
const ai = useAiStore();
const input = ref('');

onMounted(() => ai.init(props.orgId, props.projectId));

// ── model picker (a self-styled dark combobox; a framework select would drop a
// clashing light popover onto this dark surface) ──
const modelOptions = computed(() =>
  ai.providers.flatMap((p) => p.models.map((m) => ({ value: m.id, label: m.label })))
);
const modelLabel = computed(() => {
  const hit = modelOptions.value.find((o) => o.value === ai.modelId);
  return hit?.label ?? ai.modelId ?? t('ai.selectModel');
});
const showModels = ref(false);
const modelFilter = ref('');
const filteredModels = computed(() => {
  const q = modelFilter.value.trim().toLowerCase();
  const opts = modelOptions.value;
  if (!q) return opts;
  return opts.filter((o) => o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q));
});
function providerForModel(modelId: string): string {
  for (const p of ai.providers) {
    if (p.models.some((m) => m.id === modelId)) return p.id;
  }
  return ai.provider || ai.providers[0]?.id || 'openai';
}
function chooseModel(value: string) {
  const v = value.trim();
  if (v) ai.selectModel(providerForModel(v), v);
  showModels.value = false;
  modelFilter.value = '';
}

// Index of the assistant message currently streaming (for the live cursor).
const streamingIdx = computed(() => {
  if (!ai.running) return -1;
  const last = ai.messages.length - 1;
  return last >= 0 && ai.messages[last].role === 'assistant' ? last : -1;
});

const pendingTitle = computed(() => {
  const c = ai.pending?.call;
  if (!c) return '';
  if (c.name === 'publish') return t('ai.confirmPublish');
  if (c.name === 'delete_file') return t('ai.confirmDelete');
  return c.name;
});

// Human action verb — present-continuous while running, past tense when done.
const VERB_FORMS: Record<string, [string, string]> = {
  read_file: ['Reading', 'Read'],
  write_file: ['Editing', 'Edited'],
  delete_file: ['Deleting', 'Deleted'],
  list_files: ['Listing files', 'Listed files'],
  grep: ['Searching', 'Searched'],
  validate: ['Validating', 'Validated'],
  run_tests: ['Running tests', 'Ran tests'],
  publish: ['Publishing', 'Published'],
};
function verbOf(tool: { name: string; status: string }): string {
  const f = VERB_FORMS[tool.name];
  if (!f) return tool.name;
  return tool.status === 'running' ? f[0] : f[1];
}
/** The plan checklist renders itself — hide its raw tool line from the flow. */
function visibleTools(tools: ToolActivity[]): ToolActivity[] {
  return tools.filter((tl) => tl.name !== 'update_plan');
}

const planActive = computed(() => ai.plan.length > 0);
const planDone = computed(() => ai.plan.filter((p) => p.status === 'completed').length);
const planOpen = ref(true);

// Mode pill cycles agent -> ask -> agent.
function toggleMode() {
  ai.setMode(ai.mode === 'agent' ? 'ask' : 'agent');
}

// @ context picker
const showPin = ref(false);
const projectFiles = ref<string[]>([]);
const pinFilter = ref('');
const filteredFiles = computed(() => {
  const q = pinFilter.value.trim().toLowerCase();
  const list = projectFiles.value.filter((f) => !ai.contextFiles.includes(f));
  if (!q) return list;
  return list.filter((f) => f.toLowerCase().includes(q));
});
async function openPin() {
  projectFiles.value = await ai.listProjectFiles();
  pinFilter.value = '';
  showPin.value = !showPin.value;
}
function pick(path: string) {
  ai.pinFile(path);
  showPin.value = false;
}

function submit() {
  const text = input.value;
  input.value = '';
  ai.send(text);
}
</script>

<template>
  <aside class="ai-sidebar">
    <header class="ai-header">
      <span class="ai-title">{{ t('ai.title') }}</span>
      <div class="ai-header-actions">
        <button class="ai-hbtn" :title="t('ai.clear')" @click="ai.reset">
          {{ t('ai.clear') }}
        </button>
        <button class="ai-hbtn ai-hbtn-icon" :title="'Close'" @click="emit('close')">✕</button>
      </div>
    </header>

    <!-- Plan header — a sticky summary block at the top -->
    <div v-if="planActive" class="ai-plan">
      <button class="ai-plan-head" @click="planOpen = !planOpen">
        <span class="ai-plan-glyph">≡</span>
        <span class="ai-plan-name">{{ t('ai.planTitle') }}</span>
        <span class="ai-plan-count">{{ planDone }}/{{ ai.plan.length }}</span>
        <span class="ai-plan-caret" :class="{ open: planOpen }">›</span>
      </button>
      <div v-if="planOpen" class="ai-plan-list">
        <div v-for="(item, k) in ai.plan" :key="k" class="ai-plan-item" :class="item.status">
          <span class="ai-plan-dot" :class="item.status">
            <span v-if="item.status === 'in_progress'" class="ai-plan-spin" />
            <span v-else-if="item.status === 'completed'" class="ai-plan-check">✓</span>
          </span>
          <span class="ai-plan-text">{{ item.content }}</span>
        </div>
      </div>
    </div>

    <div class="ai-messages">
      <div v-if="!ai.messages.length" class="ai-empty">
        <div class="ai-empty-mark">✦</div>
        <p class="ai-empty-title">{{ t('ai.emptyTitle') }}</p>
        <p class="ai-empty-hint">{{ t('ai.emptyHint') }}</p>
      </div>

      <div v-for="(msg, i) in ai.messages" :key="i" class="ai-msg" :class="msg.role">
        <div v-if="msg.role === 'user'" class="ai-user">{{ msg.text }}</div>
        <template v-else>
          <div v-if="msg.text || i === streamingIdx" class="ai-assistant">
            <span class="ai-md" v-html="renderMarkdown(msg.text)"></span
            ><span v-if="i === streamingIdx" class="ai-cursor">▍</span>
          </div>
          <div
            v-for="tool in visibleTools(msg.tools)"
            :key="tool.id"
            class="ai-tool-line"
            :class="tool.status"
          >
            <span v-if="tool.status === 'running'" class="ai-tool-spin" />
            <span v-else class="ai-tool-tick" :class="tool.status">{{
              tool.status === 'error' ? '✕' : '✓'
            }}</span>
            <span class="ai-tool-verb">{{ verbOf(tool) }}</span>
            <code v-if="tool.target" class="ai-tool-target">{{ tool.target }}</code>
          </div>
        </template>
      </div>

      <div v-if="ai.running && streamingIdx === -1" class="ai-running">
        <span class="ai-tool-spin" />{{ t('ai.thinking') }}
      </div>
      <t-alert v-if="ai.error" theme="error" :message="ai.error" class="ai-error" />
    </div>

    <!-- Human-in-the-loop question (ask_question tool) -->
    <div v-if="ai.pendingQuestion" class="ai-card ai-question">
      <p class="ai-question-text">{{ ai.pendingQuestion.question }}</p>
      <div class="ai-question-options">
        <button
          v-for="(opt, k) in ai.pendingQuestion.options"
          :key="k"
          class="ai-opt-btn"
          @click="ai.answerQuestion(opt)"
        >
          {{ opt }}
        </button>
      </div>
    </div>

    <!-- High-risk confirmation card -->
    <div v-if="ai.pending" class="ai-card ai-confirm">
      <p class="ai-confirm-title">⚠ {{ pendingTitle }}</p>
      <pre class="ai-confirm-input">{{ JSON.stringify(ai.pending.call.input, null, 2) }}</pre>
      <div class="ai-confirm-actions">
        <button class="ai-opt-btn ghost" @click="ai.rejectPending">{{ t('ai.reject') }}</button>
        <button class="ai-opt-btn" @click="ai.approvePendingAlways">
          {{ t('ai.approveAlways') }}
        </button>
        <button class="ai-opt-btn danger" @click="ai.approvePending">{{ t('ai.approve') }}</button>
      </div>
    </div>

    <!-- Changed-file review -->
    <div v-if="ai.touchedFiles.length" class="ai-changed">
      <div v-for="f in ai.touchedFiles" :key="f" class="ai-changed-row">
        <span class="ai-changed-dot" />
        <code class="ai-changed-file">{{ f }}</code>
        <span class="ai-changed-actions">
          <button v-if="ai.canRevert(f)" class="ai-changed-btn revert" @click="ai.revertFile(f)">
            {{ t('ai.revert') }}
          </button>
          <button class="ai-changed-btn keep" @click="ai.keepFile(f)">{{ t('ai.keep') }}</button>
        </span>
      </div>
    </div>

    <!-- Unified composer -->
    <footer class="ai-composer" :class="{ disabled: !ai.ready }">
      <div v-if="ai.contextFiles.length" class="ai-pills">
        <span v-for="p in ai.contextFiles" :key="p" class="ai-pill">
          <span class="ai-pill-at">@</span>{{ p }}
          <button class="ai-pill-x" @click="ai.unpinFile(p)">✕</button>
        </span>
      </div>

      <!-- @ context picker popover -->
      <div v-if="showPin" class="ai-popover ai-pin-pop">
        <input
          v-model="pinFilter"
          class="ai-pop-filter"
          :placeholder="t('ai.addContext')"
          autofocus
        />
        <div class="ai-pop-list">
          <button v-for="f in filteredFiles" :key="f" class="ai-pop-item" @click="pick(f)">
            {{ f }}
          </button>
          <div v-if="!filteredFiles.length" class="ai-pop-empty">—</div>
        </div>
      </div>

      <textarea
        v-model="input"
        class="ai-input"
        :placeholder="t('ai.placeholder')"
        :disabled="ai.running || !ai.ready"
        rows="1"
        @keydown.enter.exact.prevent="submit"
      ></textarea>

      <div class="ai-toolbar">
        <button class="ai-mode" :class="ai.mode" :title="t('ai.modeHint')" @click="toggleMode">
          <span class="ai-mode-dot" />{{
            ai.mode === 'agent' ? t('ai.modeAgent') : t('ai.modeAsk')
          }}
        </button>

        <div class="ai-model">
          <button v-if="modelOptions.length" class="ai-model-btn" @click="showModels = !showModels">
            <span class="ai-model-label">{{ modelLabel }}</span>
            <span class="ai-model-caret">⌄</span>
          </button>
          <span v-else class="ai-no-provider">{{ t('ai.noProvider') }}</span>
          <div v-if="showModels" class="ai-popover ai-model-pop">
            <input
              v-model="modelFilter"
              class="ai-pop-filter"
              :placeholder="t('ai.selectModel')"
              autofocus
              @keydown.enter="chooseModel(modelFilter)"
            />
            <div class="ai-pop-list">
              <button
                v-for="o in filteredModels"
                :key="o.value"
                class="ai-pop-item"
                :class="{ active: o.value === ai.modelId }"
                @click="chooseModel(o.value)"
              >
                {{ o.label }}
              </button>
            </div>
          </div>
        </div>

        <span class="ai-toolbar-spacer" />

        <button
          class="ai-icon-btn"
          :class="{ active: showPin }"
          :disabled="!ai.ready"
          :title="t('ai.addContext')"
          @click="openPin"
        >
          @
        </button>
        <button v-if="ai.running" class="ai-send stop" :title="t('ai.stop')" @click="ai.stop">
          <span class="ai-stop-glyph" />
        </button>
        <button
          v-else
          class="ai-send"
          :disabled="!ai.ready || !input.trim()"
          :title="t('ai.send')"
          @click="submit"
        >
          ↑
        </button>
      </div>
    </footer>
  </aside>
</template>

<style scoped>
/* Follows the studio's own design tokens — a light, on-brand surface that
   adapts with the app theme (the --ordo-* set flips in dark mode). */
.ai-sidebar {
  --c-bg: var(--ordo-bg-panel, #ffffff);
  --c-elev: var(--ordo-bg-panel, #ffffff);
  --c-elev-2: var(--ordo-bg-secondary, #f4f5f7);
  --c-hover: var(--ordo-hover-bg, rgba(0, 0, 0, 0.05));
  --c-border: var(--ordo-border-light, #eceef1);
  --c-border-2: var(--ordo-border-color, #e1e4e8);
  --c-text: var(--ordo-text-primary, #24292e);
  --c-dim: var(--ordo-text-secondary, #586069);
  --c-faint: var(--ordo-text-tertiary, #8b929c);
  --c-accent: var(--ordo-accent, #0066b8);
  --c-accent-hover: var(--ordo-accent-hover, #005ba1);
  --c-accent-soft: var(--ordo-accent-bg, #e6f1fc);
  --c-green: var(--ordo-success, #388a34);
  --c-red: var(--ordo-error, #e51400);
  --c-warn: var(--ordo-warning, #b76e00);
  --c-code-bg: var(--ordo-bg-secondary, #f6f8fa);
  --c-mono: var(--ordo-font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);

  display: flex;
  flex-direction: column;
  width: 372px;
  height: 100%;
  border-left: 1px solid var(--c-border);
  background: var(--c-bg);
  color: var(--c-text);
  font-size: 13px;
}
.ai-sidebar button {
  font-family: inherit;
}

/* ── header ── */
.ai-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 40px;
  padding: 0 8px 0 14px;
  flex-shrink: 0;
}
.ai-title {
  font-weight: 600;
  font-size: 13px;
  color: var(--c-text);
}
.ai-header-actions {
  display: flex;
  gap: 2px;
}
.ai-hbtn {
  border: none;
  background: transparent;
  color: var(--c-dim);
  font-size: 12px;
  padding: 4px 8px;
  border-radius: 5px;
  cursor: pointer;
  transition:
    background 0.1s,
    color 0.1s;
}
.ai-hbtn:hover {
  background: var(--c-hover);
  color: var(--c-text);
}
.ai-hbtn-icon {
  padding: 4px 7px;
  font-size: 11px;
}

/* ── plan header ── */
.ai-plan {
  margin: 2px 10px 6px;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  background: var(--c-elev);
  overflow: hidden;
  flex-shrink: 0;
}
.ai-plan-head {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 10px;
  border: none;
  background: transparent;
  color: var(--c-text);
  cursor: pointer;
  text-align: left;
}
.ai-plan-glyph {
  color: var(--c-dim);
  font-size: 13px;
}
.ai-plan-name {
  font-weight: 600;
  font-size: 12.5px;
}
.ai-plan-count {
  margin-left: auto;
  font-size: 11px;
  color: var(--c-dim);
  font-variant-numeric: tabular-nums;
}
.ai-plan-caret {
  color: var(--c-faint);
  transition: transform 0.15s;
  transform: rotate(90deg);
}
.ai-plan-caret.open {
  transform: rotate(-90deg);
}
.ai-plan-list {
  padding: 2px 10px 9px;
  display: flex;
  flex-direction: column;
  gap: 7px;
  border-top: 1px solid var(--c-border);
  margin-top: -1px;
  padding-top: 9px;
}
.ai-plan-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 12.5px;
  line-height: 1.5;
}
.ai-plan-dot {
  flex-shrink: 0;
  width: 13px;
  height: 13px;
  margin-top: 2px;
  border-radius: 50%;
  border: 1.5px solid var(--c-border-2);
  display: flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
}
.ai-plan-dot.in_progress {
  border-color: var(--c-accent);
}
.ai-plan-dot.completed {
  border-color: var(--c-green);
  background: var(--c-green);
}
.ai-plan-check {
  color: #ffffff;
  font-size: 9px;
  line-height: 1;
  font-weight: 700;
}
.ai-plan-spin {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  border: 1.5px solid var(--c-accent);
  border-top-color: transparent;
  animation: ai-spin 0.8s linear infinite;
}
.ai-plan-text {
  color: var(--c-text);
}
.ai-plan-item.completed .ai-plan-text {
  color: var(--c-dim);
  text-decoration: line-through;
}
.ai-plan-item.pending .ai-plan-text {
  color: var(--c-dim);
}

/* ── message flow ── */
.ai-messages {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 8px 14px 4px;
  display: flex;
  flex-direction: column;
  gap: 11px;
}
.ai-empty {
  margin: auto;
  text-align: center;
  color: var(--c-dim);
  padding-bottom: 40px;
}
.ai-empty-mark {
  font-size: 20px;
  color: var(--c-accent);
  opacity: 0.8;
  margin-bottom: 10px;
}
.ai-empty-title {
  font-size: 13px;
  color: var(--c-text);
  margin-bottom: 4px;
}
.ai-empty-hint {
  font-size: 12px;
  color: var(--c-faint);
  line-height: 1.5;
}
.ai-msg {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
/* User turn — a compact bubble aligned to the trailing edge. */
.ai-user {
  align-self: flex-end;
  max-width: 90%;
  padding: 7px 11px;
  border-radius: 12px 12px 3px 12px;
  background: var(--c-elev-2);
  border: 1px solid var(--c-border);
  color: var(--c-text);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
/* Assistant turn — rendered markdown, full width. */
.ai-assistant {
  color: var(--c-text);
  line-height: 1.62;
  word-break: break-word;
}
.ai-md :deep(.ai-code) {
  background: var(--c-code-bg);
  border: 1px solid var(--c-border-2);
  border-radius: 7px;
  padding: 9px 11px;
  margin: 7px 0;
  overflow-x: auto;
  font-family: var(--c-mono);
  font-size: 12px;
  line-height: 1.5;
  white-space: pre;
  color: var(--c-text);
}
.ai-md :deep(.ai-inline-code) {
  background: var(--c-code-bg);
  border: 1px solid var(--c-border-2);
  border-radius: 4px;
  padding: 0.5px 4px;
  font-family: var(--c-mono);
  font-size: 0.88em;
  color: var(--c-text);
}
.ai-md :deep(.ai-list) {
  margin: 4px 0;
  padding-left: 18px;
}
.ai-md :deep(a) {
  color: var(--c-accent);
}
.ai-md :deep(h3),
.ai-md :deep(h4),
.ai-md :deep(h5) {
  font-size: 13px;
  font-weight: 600;
  margin: 9px 0 4px;
  color: var(--c-text);
}
/* A tool call as an understated dimmed line: verb + code target. */
.ai-tool-line {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  line-height: 1.5;
  overflow: hidden;
  white-space: nowrap;
  color: var(--c-dim);
}
.ai-tool-verb {
  flex-shrink: 0;
}
.ai-tool-target {
  color: var(--c-text);
  font-family: var(--c-mono);
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ai-tool-line.error,
.ai-tool-line.error .ai-tool-target {
  color: var(--c-red);
}
.ai-tool-tick {
  flex-shrink: 0;
  width: 12px;
  text-align: center;
  font-size: 10px;
  color: var(--c-green);
  opacity: 0.9;
}
.ai-tool-tick.error {
  color: var(--c-red);
}
.ai-tool-spin {
  flex-shrink: 0;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  border: 1.5px solid var(--c-dim);
  border-top-color: transparent;
  animation: ai-spin 0.8s linear infinite;
}
@keyframes ai-spin {
  to {
    transform: rotate(360deg);
  }
}
.ai-cursor {
  display: inline-block;
  animation: ai-blink 1s step-end infinite;
  color: var(--c-accent);
}
@keyframes ai-blink {
  50% {
    opacity: 0;
  }
}
.ai-running {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  color: var(--c-dim);
}
.ai-error {
  font-size: 12px;
}

/* ── inline cards (question / confirm) ── */
.ai-card {
  margin: 0 12px 8px;
  padding: 11px;
  border-radius: 9px;
  border: 1px solid var(--c-border);
  background: var(--c-elev);
}
.ai-question-text {
  font-size: 13px;
  line-height: 1.5;
  margin-bottom: 9px;
  color: var(--c-text);
}
.ai-question-options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.ai-opt-btn {
  border: 1px solid var(--c-border-2);
  background: var(--c-elev-2);
  color: var(--c-text);
  font-size: 12px;
  padding: 5px 11px;
  border-radius: 7px;
  cursor: pointer;
  transition:
    background 0.1s,
    border-color 0.1s;
}
.ai-opt-btn:hover {
  background: var(--c-hover);
  border-color: var(--c-accent);
}
.ai-opt-btn.ghost {
  border-color: transparent;
  background: transparent;
  color: var(--c-dim);
}
.ai-opt-btn.danger {
  border-color: color-mix(in srgb, var(--c-red) 45%, transparent);
  color: var(--c-red);
}
.ai-confirm {
  border-color: color-mix(in srgb, var(--c-warn) 40%, transparent);
  background: color-mix(in srgb, var(--c-warn) 8%, transparent);
}
.ai-confirm-title {
  font-size: 12.5px;
  font-weight: 600;
  margin-bottom: 7px;
  color: var(--c-warn);
}
.ai-confirm-input {
  font-size: 11px;
  max-height: 120px;
  overflow: auto;
  background: var(--c-code-bg);
  border: 1px solid var(--c-border-2);
  color: var(--c-text);
  padding: 7px;
  border-radius: 6px;
  margin-bottom: 9px;
  font-family: var(--c-mono);
}
.ai-confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* ── changed-file review ── */
.ai-changed {
  display: flex;
  flex-direction: column;
  gap: 1px;
  margin: 0 12px 8px;
  padding: 5px 4px;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  background: var(--c-elev);
}
.ai-changed-row {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 4px 8px;
  border-radius: 5px;
  font-size: 12px;
}
.ai-changed-row:hover {
  background: var(--c-hover);
}
.ai-changed-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--c-accent);
  flex-shrink: 0;
}
.ai-changed-file {
  font-family: var(--c-mono);
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--c-text);
  flex: 1;
}
.ai-changed-actions {
  display: flex;
  gap: 10px;
  flex-shrink: 0;
}
.ai-changed-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 12px;
  padding: 0;
}
.ai-changed-btn.keep {
  color: var(--c-green);
}
.ai-changed-btn.revert {
  color: var(--c-red);
}

/* ── composer ── */
.ai-composer {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 7px;
  margin: 6px 12px 12px;
  padding: 9px 10px 7px;
  border: 1px solid var(--c-border-2);
  border-radius: 12px;
  background: var(--c-elev);
  transition: border-color 0.12s;
}
.ai-composer:focus-within {
  border-color: rgba(74, 158, 255, 0.6);
}
.ai-composer.disabled {
  opacity: 0.55;
}
.ai-pills {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.ai-pill {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 20px;
  padding: 0 6px;
  border: 1px solid var(--c-border-2);
  border-radius: 5px;
  font-size: 11.5px;
  line-height: 16px;
  color: var(--c-text);
  max-width: 100%;
  white-space: nowrap;
  background: var(--c-elev-2);
}
.ai-pill-at {
  color: var(--c-accent);
}
.ai-pill-x {
  border: none;
  background: transparent;
  cursor: pointer;
  padding: 0;
  font-size: 9px;
  color: var(--c-faint);
  line-height: 1;
}
.ai-pill-x:hover {
  color: var(--c-text);
}
.ai-input {
  width: 100%;
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  color: var(--c-text);
  font-family: inherit;
  font-size: 13px;
  line-height: 1.5;
  max-height: 168px;
  padding: 2px 2px 0;
}
.ai-input::placeholder {
  color: var(--c-faint);
}
.ai-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
}
.ai-toolbar-spacer {
  flex: 1;
}
.ai-mode {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
  height: 24px;
  padding: 0 10px;
  border: 1px solid var(--c-border-2);
  border-radius: 9999px;
  background: var(--c-elev-2);
  color: var(--c-dim);
  font-size: 12px;
  cursor: pointer;
  transition:
    background 0.1s,
    color 0.1s,
    border-color 0.1s;
}
.ai-mode-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  opacity: 0.7;
}
.ai-mode.agent {
  border-color: rgba(74, 158, 255, 0.55);
  color: var(--c-accent);
  background: var(--c-accent-soft);
}
.ai-model {
  position: relative;
  flex: 1;
  min-width: 0;
}
.ai-model-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  height: 24px;
  padding: 0 6px;
  border: none;
  background: transparent;
  color: var(--c-dim);
  font-size: 12px;
  cursor: pointer;
  border-radius: 6px;
  transition:
    background 0.1s,
    color 0.1s;
}
.ai-model-btn:hover {
  background: var(--c-hover);
  color: var(--c-text);
}
.ai-model-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ai-model-caret {
  color: var(--c-faint);
  font-size: 11px;
  flex-shrink: 0;
}
.ai-no-provider {
  font-size: 12px;
  color: var(--c-faint);
}
.ai-icon-btn {
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--c-dim);
  cursor: pointer;
  font-size: 14px;
  transition:
    background 0.1s,
    color 0.1s;
}
.ai-icon-btn:hover:not(:disabled),
.ai-icon-btn.active {
  background: var(--c-hover);
  color: var(--c-text);
}
.ai-icon-btn:disabled {
  opacity: 0.35;
  cursor: default;
}
.ai-send {
  flex-shrink: 0;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 8px;
  background: var(--c-accent);
  color: #fff;
  cursor: pointer;
  font-size: 15px;
  line-height: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  transition:
    background 0.1s,
    opacity 0.1s;
}
.ai-send:hover:not(:disabled) {
  background: var(--c-accent-hover);
}
.ai-send:disabled {
  opacity: 0.35;
  cursor: default;
}
.ai-send.stop {
  background: var(--c-elev-2);
  border: 1px solid var(--c-border-2);
}
.ai-stop-glyph {
  width: 9px;
  height: 9px;
  border-radius: 2px;
  background: var(--c-text);
}

/* ── popovers (model picker / @ files) ── */
.ai-popover {
  position: absolute;
  z-index: 20;
  background: var(--c-elev);
  border: 1px solid var(--c-border-2);
  border-radius: 9px;
  box-shadow: var(--ordo-shadow-md, 0 4px 16px rgba(0, 0, 0, 0.14));
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.ai-model-pop {
  bottom: 30px;
  left: 0;
  width: 240px;
  max-height: 280px;
}
.ai-pin-pop {
  bottom: 46px;
  left: 8px;
  right: 8px;
  max-height: 240px;
}
.ai-pop-filter {
  border: none;
  border-bottom: 1px solid var(--c-border);
  background: transparent;
  color: var(--c-text);
  font-family: inherit;
  font-size: 12px;
  padding: 8px 10px;
  outline: none;
}
.ai-pop-filter::placeholder {
  color: var(--c-faint);
}
.ai-pop-list {
  overflow-y: auto;
  padding: 4px;
}
.ai-pop-item {
  display: block;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  color: var(--c-text);
  font-size: 12px;
  padding: 6px 8px;
  border-radius: 5px;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ai-pop-item:hover {
  background: var(--c-hover);
}
.ai-pop-item.active {
  color: var(--c-accent);
}
.ai-pop-empty {
  padding: 10px;
  text-align: center;
  color: var(--c-faint);
  font-size: 12px;
}
</style>
