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

// Model options (value = model slug). Users can also type any slug the provider
// supports — see the creatable select below.
const modelOptions = computed(() =>
  ai.providers.flatMap((p) => p.models.map((m) => ({ value: m.id, label: m.label })))
);
function providerForModel(modelId: string): string {
  for (const p of ai.providers) {
    if (p.models.some((m) => m.id === modelId)) return p.id;
  }
  return ai.provider || ai.providers[0]?.id || 'openai';
}
const selectedModel = computed({
  get: () => ai.modelId,
  set: (v: string) => {
    if (v) ai.selectModel(providerForModel(v), v);
  },
});

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

// Mode pill cycles agent -> ask -> agent.
function toggleMode() {
  ai.setMode(ai.mode === 'agent' ? 'ask' : 'agent');
}

// @ context picker
const showPin = ref(false);
const projectFiles = ref<string[]>([]);
async function openPin() {
  projectFiles.value = await ai.listProjectFiles();
  showPin.value = true;
}
function pick(path: unknown) {
  if (typeof path === 'string') ai.pinFile(path);
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
        <t-button size="small" variant="text" @click="ai.reset">{{ t('ai.clear') }}</t-button>
        <t-button size="small" variant="text" shape="square" @click="emit('close')">✕</t-button>
      </div>
    </header>

    <div v-if="ai.touchedFiles.length" class="ai-changed">
      <div class="ai-changed-label">{{ t('ai.changedFiles') }}</div>
      <div v-for="f in ai.touchedFiles" :key="f" class="ai-changed-row">
        <span class="ai-changed-file">{{ f }}</span>
        <span class="ai-changed-actions">
          <button v-if="ai.canRevert(f)" class="ai-changed-btn revert" @click="ai.revertFile(f)">
            {{ t('ai.revert') }}
          </button>
          <button class="ai-changed-btn keep" @click="ai.keepFile(f)">{{ t('ai.keep') }}</button>
        </span>
      </div>
    </div>

    <div class="ai-messages">
      <div v-if="!ai.messages.length" class="ai-empty">
        <p>{{ t('ai.emptyTitle') }}</p>
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
            <t-icon v-if="tool.status === 'running'" name="loading" class="ai-tool-spin" />
            <span v-else class="ai-tool-tick">{{ tool.status === 'error' ? '✕' : '✓' }}</span>
            <span class="ai-tool-verb">{{ verbOf(tool) }}</span>
            <span v-if="tool.target" class="ai-tool-target">{{ tool.target }}</span>
          </div>
        </template>
      </div>

      <div v-if="ai.running && streamingIdx === -1" class="ai-running">{{ t('ai.thinking') }}</div>
      <t-alert v-if="ai.error" theme="error" :message="ai.error" class="ai-error" />
    </div>

    <!-- Task checklist (update_plan tool) -->
    <div v-if="planActive" class="ai-plan">
      <div class="ai-plan-head">
        <span class="ai-plan-title">{{ t('ai.planTitle') }}</span>
        <span class="ai-plan-count">{{ planDone }}/{{ ai.plan.length }}</span>
      </div>
      <div v-for="(item, k) in ai.plan" :key="k" class="ai-plan-item" :class="item.status">
        <span class="ai-plan-dot" :class="item.status">
          <t-icon v-if="item.status === 'in_progress'" name="loading" class="ai-plan-spin" />
          <span v-else-if="item.status === 'completed'" class="ai-plan-check">✓</span>
        </span>
        <span class="ai-plan-text">{{ item.content }}</span>
      </div>
    </div>

    <!-- Human-in-the-loop question (ask_question tool) -->
    <div v-if="ai.pendingQuestion" class="ai-question">
      <p class="ai-question-text">{{ ai.pendingQuestion.question }}</p>
      <div class="ai-question-options">
        <t-button
          v-for="(opt, k) in ai.pendingQuestion.options"
          :key="k"
          size="small"
          variant="outline"
          @click="ai.answerQuestion(opt)"
        >
          {{ opt }}
        </t-button>
      </div>
    </div>

    <!-- High-risk confirmation card -->
    <div v-if="ai.pending" class="ai-confirm">
      <p class="ai-confirm-title">⚠ {{ pendingTitle }}</p>
      <pre class="ai-confirm-input">{{ JSON.stringify(ai.pending.call.input, null, 2) }}</pre>
      <div class="ai-confirm-actions">
        <t-button size="small" variant="text" @click="ai.rejectPending">{{
          t('ai.reject')
        }}</t-button>
        <t-button size="small" variant="outline" @click="ai.approvePendingAlways">{{
          t('ai.approveAlways')
        }}</t-button>
        <t-button size="small" theme="danger" @click="ai.approvePending">{{
          t('ai.approve')
        }}</t-button>
      </div>
    </div>

    <!-- Unified composer: context pills + textarea + toolbar, all in one box -->
    <footer class="ai-composer" :class="{ disabled: !ai.ready }">
      <div v-if="ai.contextFiles.length || showPin" class="ai-composer-pills">
        <span v-for="p in ai.contextFiles" :key="p" class="ai-pill">
          <span class="ai-pill-at">@</span>{{ p }}
          <button class="ai-pill-x" @click="ai.unpinFile(p)">✕</button>
        </span>
        <t-select
          v-if="showPin"
          size="small"
          filterable
          autofocus
          :placeholder="t('ai.addContext')"
          class="ai-pin-select"
          @change="pick"
        >
          <t-option v-for="f in projectFiles" :key="f" :value="f" :label="f" />
        </t-select>
      </div>

      <textarea
        v-model="input"
        class="ai-composer-input"
        :placeholder="t('ai.placeholder')"
        :disabled="ai.running || !ai.ready"
        rows="1"
        @keydown.enter.exact.prevent="submit"
      ></textarea>

      <div class="ai-composer-toolbar">
        <button class="ai-mode-pill" :class="ai.mode" :title="t('ai.modeHint')" @click="toggleMode">
          {{ ai.mode === 'agent' ? t('ai.modeAgent') : t('ai.modeAsk') }}
        </button>
        <t-select
          v-if="modelOptions.length"
          v-model="selectedModel"
          size="small"
          filterable
          :creatable="true"
          :placeholder="t('ai.selectModel')"
          class="ai-model-select"
        >
          <t-option v-for="o in modelOptions" :key="o.value" :value="o.value" :label="o.label" />
        </t-select>
        <span v-else class="ai-no-provider">{{ t('ai.noProvider') }}</span>

        <span class="ai-toolbar-spacer" />

        <button
          class="ai-icon-btn"
          :disabled="!ai.ready"
          :title="t('ai.addContext')"
          @click="openPin"
        >
          @
        </button>
        <button v-if="ai.running" class="ai-send-btn stop" @click="ai.stop">
          <span class="ai-stop-glyph" />
        </button>
        <button v-else class="ai-send-btn" :disabled="!ai.ready || !input.trim()" @click="submit">
          ↑
        </button>
      </div>
    </footer>
  </aside>
</template>

<style scoped>
.ai-sidebar {
  display: flex;
  flex-direction: column;
  width: 360px;
  height: 100%;
  border-left: 1px solid var(--ordo-border, #e7e7e7);
  background: var(--ordo-bg-elevated, #fff);
}
.ai-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px 8px 12px;
  border-bottom: 1px solid var(--ordo-border, #e7e7e7);
}
.ai-title {
  font-weight: 600;
  font-size: 13px;
}
.ai-header-actions {
  display: flex;
  gap: 2px;
}

/* ── changed-file review ── */
.ai-changed {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border, #e7e7e7);
}
.ai-changed-label {
  font-size: 11px;
  color: var(--ordo-text-secondary, #888);
  margin-bottom: 2px;
}
.ai-changed-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
}
.ai-changed-file {
  font-family: var(--td-font-family-mono, ui-monospace, monospace);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ordo-text-primary, #333);
}
.ai-changed-actions {
  display: flex;
  gap: 8px;
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
  color: var(--td-success-color, #2ba471);
}
.ai-changed-btn.revert {
  color: var(--td-error-color, #d54941);
}

/* ── message flow ── */
.ai-messages {
  flex: 1;
  overflow-y: auto;
  padding: 14px 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.ai-empty {
  margin-top: 24px;
  text-align: center;
  color: var(--ordo-text-secondary, #888);
  font-size: 13px;
}
.ai-empty-hint {
  font-size: 12px;
  margin-top: 6px;
}
.ai-msg {
  display: flex;
  flex-direction: column;
  gap: 5px;
  font-size: 13px;
}
/* User turn — a compact bubble aligned to the trailing edge. */
.ai-user {
  align-self: flex-end;
  max-width: 88%;
  min-width: 64px;
  padding: 6px 10px;
  border: 1px solid var(--ordo-border, #e3e3e3);
  border-radius: 12px 12px 4px 12px;
  background: var(--ordo-bg-sunken, #f6f7f9);
  color: var(--ordo-text-primary, #1a1a1a);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
/* Assistant turn — rendered markdown. */
.ai-assistant {
  color: var(--ordo-text-primary, #1a1a1a);
  line-height: 1.6;
  word-break: break-word;
}
.ai-md :deep(.ai-code) {
  background: var(--ordo-bg-sunken, #f4f4f4);
  border: 1px solid var(--ordo-border, #ececec);
  border-radius: 6px;
  padding: 8px 10px;
  margin: 6px 0;
  overflow-x: auto;
  font-family: var(--td-font-family-mono, ui-monospace, monospace);
  font-size: 12px;
  line-height: 1.45;
  white-space: pre;
}
.ai-md :deep(.ai-inline-code) {
  background: var(--ordo-bg-sunken, #f0f0f0);
  border-radius: 4px;
  padding: 1px 4px;
  font-family: var(--td-font-family-mono, ui-monospace, monospace);
  font-size: 0.92em;
}
.ai-md :deep(.ai-list) {
  margin: 4px 0;
  padding-left: 18px;
}
.ai-md :deep(h3),
.ai-md :deep(h4),
.ai-md :deep(h5) {
  font-size: 13px;
  font-weight: 600;
  margin: 8px 0 4px;
}
/* A tool call as a compact monochrome line: verb + dimmed target. */
.ai-tool-line {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  line-height: 1.5;
  overflow: hidden;
  white-space: nowrap;
  color: var(--ordo-text-secondary, #8a8a8a);
}
.ai-tool-verb {
  flex-shrink: 0;
  font-weight: 500;
}
.ai-tool-target {
  color: var(--ordo-text-placeholder, #b0b0b0);
  font-family: var(--td-font-family-mono, ui-monospace, SFMono-Regular, monospace);
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ai-tool-line.error,
.ai-tool-line.error .ai-tool-target {
  color: var(--td-error-color, #d54941);
}
.ai-tool-tick {
  flex-shrink: 0;
  width: 12px;
  text-align: center;
  font-size: 11px;
  opacity: 0.55;
}
.ai-tool-spin {
  flex-shrink: 0;
  animation: ai-spin 0.9s linear infinite;
}
@keyframes ai-spin {
  to {
    transform: rotate(360deg);
  }
}
.ai-cursor {
  display: inline-block;
  animation: ai-blink 1s step-end infinite;
  color: var(--ordo-brand, #0052d9);
  font-weight: 400;
}
@keyframes ai-blink {
  50% {
    opacity: 0;
  }
}
.ai-running,
.ai-error {
  font-size: 12px;
  color: var(--ordo-text-secondary, #888);
}

/* ── task checklist ── */
.ai-plan {
  margin: 0 12px 8px;
  padding: 8px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  border: 1px solid var(--ordo-border, #e3e3e3);
  border-radius: 6px;
  background: var(--ordo-bg-sunken, #f7f8fa);
}
.ai-plan-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.ai-plan-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-primary, #333);
}
.ai-plan-count {
  font-size: 11px;
  color: var(--ordo-text-secondary, #999);
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
  border: 1.5px solid var(--ordo-text-placeholder, #bdbdbd);
  display: flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
}
.ai-plan-dot.in_progress {
  border-color: var(--ordo-brand, #0052d9);
}
.ai-plan-dot.completed {
  border-color: var(--td-success-color, #2ba471);
  background: var(--td-success-color, #2ba471);
}
.ai-plan-check {
  color: #fff;
  font-size: 9px;
  line-height: 1;
}
.ai-plan-spin {
  color: var(--ordo-brand, #0052d9);
  font-size: 10px;
  animation: ai-spin 0.9s linear infinite;
}
.ai-plan-text {
  color: var(--ordo-text-primary, #333);
}
.ai-plan-item.completed .ai-plan-text {
  color: var(--ordo-text-secondary, #999);
  text-decoration: line-through;
}
.ai-plan-item.pending .ai-plan-text {
  color: var(--ordo-text-secondary, #8a8a8a);
}

/* ── inline cards (question / confirm) ── */
.ai-question {
  margin: 0 12px 8px;
  padding: 10px;
  border: 1px solid var(--ordo-border, #dcdcdc);
  border-radius: 8px;
  background: var(--ordo-bg-sunken, #fafafa);
}
.ai-question-text {
  font-size: 13px;
  line-height: 1.5;
  margin-bottom: 8px;
}
.ai-question-options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.ai-confirm {
  margin: 0 12px 8px;
  padding: 10px;
  border: 1px solid var(--ordo-warning, #e37318);
  border-radius: 8px;
  background: rgba(227, 115, 24, 0.06);
}
.ai-confirm-title {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 6px;
}
.ai-confirm-input {
  font-size: 11px;
  max-height: 120px;
  overflow: auto;
  background: var(--ordo-bg-sunken, #f3f3f3);
  padding: 6px;
  border-radius: 6px;
  margin-bottom: 8px;
}
.ai-confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* ── unified composer ── */
.ai-composer {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 8px 12px 12px;
  padding: 8px 10px 6px;
  border: 1px solid var(--ordo-border, #dcdcdc);
  border-radius: 10px;
  background: var(--ordo-bg-elevated, #fff);
  transition: border-color 0.12s;
}
.ai-composer:focus-within {
  border-color: var(--ordo-brand, #0052d9);
}
.ai-composer.disabled {
  opacity: 0.6;
}
.ai-composer-pills {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
}
.ai-pill {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 20px;
  padding: 0 6px;
  border: 1px solid var(--ordo-border, #e0e0e0);
  border-radius: 4px;
  font-size: 12px;
  line-height: 16px;
  color: var(--ordo-text-primary, #444);
  max-width: 100%;
  white-space: nowrap;
}
.ai-pill-at {
  color: var(--ordo-text-secondary, #999);
}
.ai-pill-x {
  border: none;
  background: transparent;
  cursor: pointer;
  padding: 0;
  font-size: 9px;
  color: var(--ordo-text-secondary, #aaa);
  line-height: 1;
}
.ai-pill-x:hover {
  color: var(--ordo-text-primary, #555);
}
.ai-pin-select {
  min-width: 160px;
}
.ai-composer-input {
  width: 100%;
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  color: var(--ordo-text-primary, #1a1a1a);
  font-family: inherit;
  font-size: 13px;
  line-height: 1.5;
  max-height: 160px;
  padding: 2px 0;
}
.ai-composer-input::placeholder {
  color: var(--ordo-text-placeholder, #b0b0b0);
}
.ai-composer-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
}
.ai-toolbar-spacer {
  flex: 1;
}
.ai-mode-pill {
  flex-shrink: 0;
  height: 22px;
  padding: 0 9px;
  border: 1px solid var(--ordo-border, #dcdcdc);
  border-radius: 9999px;
  background: var(--ordo-bg-sunken, #f4f5f7);
  color: var(--ordo-text-secondary, #666);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition:
    background 0.1s,
    color 0.1s,
    border-color 0.1s;
}
.ai-mode-pill.agent {
  border-color: var(--ordo-brand, #0052d9);
  color: var(--ordo-brand, #0052d9);
  background: color-mix(in srgb, var(--ordo-brand, #0052d9) 8%, transparent);
}
.ai-model-select {
  flex: 1;
  min-width: 0;
}
/* strip the model select down to a borderless pill inside the toolbar */
.ai-model-select :deep(.t-input) {
  border: none;
  background: transparent;
  padding-left: 2px;
}
.ai-model-select :deep(.t-input:hover),
.ai-model-select :deep(.t-input--focused) {
  border: none;
  box-shadow: none;
}
.ai-no-provider {
  flex: 1;
  font-size: 12px;
  color: var(--ordo-text-secondary, #888);
}
.ai-icon-btn {
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--ordo-text-secondary, #888);
  cursor: pointer;
  font-size: 14px;
  transition: background 0.1s;
}
.ai-icon-btn:hover:not(:disabled) {
  background: var(--ordo-bg-sunken, #f0f0f0);
}
.ai-icon-btn:disabled {
  opacity: 0.4;
  cursor: default;
}
.ai-send-btn {
  flex-shrink: 0;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 7px;
  background: var(--ordo-brand, #0052d9);
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
.ai-send-btn:disabled {
  opacity: 0.4;
  cursor: default;
}
.ai-send-btn.stop {
  background: var(--ordo-text-secondary, #666);
}
.ai-stop-glyph {
  width: 9px;
  height: 9px;
  border-radius: 2px;
  background: #fff;
}
</style>
