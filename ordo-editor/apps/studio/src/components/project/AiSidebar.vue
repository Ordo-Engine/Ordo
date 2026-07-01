<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useAiStore } from '@/stores/ai-agent';
import { renderMarkdown } from '@/utils/ai-markdown';

const props = defineProps<{ orgId: string; projectId: string }>();
const emit = defineEmits<{ (e: 'close'): void }>();

const { t } = useI18n();
const ai = useAiStore();
const input = ref('');

onMounted(() => ai.init(props.orgId, props.projectId));

// Model options (value = model slug). Users can also type any slug the provider
// supports (e.g. an OpenRouter model) — see the creatable select below.
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

    <div class="ai-model-bar">
      <div class="ai-mode-toggle">
        <button :class="{ active: ai.mode === 'agent' }" @click="ai.setMode('agent')">
          {{ t('ai.modeAgent') }}
        </button>
        <button :class="{ active: ai.mode === 'ask' }" @click="ai.setMode('ask')">
          {{ t('ai.modeAsk') }}
        </button>
      </div>
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
    </div>

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
          <div v-for="tool in msg.tools" :key="tool.id" class="ai-tool-line" :class="tool.status">
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

    <!-- @ context pins -->
    <div v-if="ai.contextFiles.length || showPin" class="ai-context">
      <t-tag
        v-for="p in ai.contextFiles"
        :key="p"
        size="small"
        variant="light"
        closable
        @close="ai.unpinFile(p)"
      >
        @ {{ p }}
      </t-tag>
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

    <footer class="ai-input-bar">
      <t-button
        size="small"
        variant="text"
        shape="square"
        :disabled="!ai.ready"
        :title="t('ai.addContext')"
        @click="openPin"
      >
        @
      </t-button>
      <t-textarea
        v-model="input"
        :placeholder="t('ai.placeholder')"
        :autosize="{ minRows: 1, maxRows: 5 }"
        :disabled="ai.running || !ai.ready"
        @keydown.enter.exact.prevent="submit"
      />
      <t-button v-if="ai.running" size="small" theme="default" @click="ai.stop">
        ■ {{ t('ai.stop') }}
      </t-button>
      <t-button
        v-else
        theme="primary"
        size="small"
        :disabled="!ai.ready || !input.trim()"
        @click="submit"
      >
        {{ t('ai.send') }}
      </t-button>
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
  padding: 10px 12px;
  border-bottom: 1px solid var(--ordo-border, #e7e7e7);
}
.ai-title {
  font-weight: 600;
  font-size: 14px;
}
.ai-header-actions {
  display: flex;
  gap: 2px;
}
.ai-model-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border, #e7e7e7);
}
.ai-model-select {
  flex: 1;
  min-width: 0;
}
.ai-mode-toggle {
  display: inline-flex;
  flex-shrink: 0;
  border: 1px solid var(--ordo-border, #dcdcdc);
  border-radius: 6px;
  overflow: hidden;
}
.ai-mode-toggle button {
  border: none;
  background: transparent;
  font-size: 12px;
  padding: 3px 10px;
  cursor: pointer;
  color: var(--ordo-text-secondary, #888);
  transition:
    background 0.1s,
    color 0.1s;
}
.ai-mode-toggle button.active {
  background: var(--ordo-brand, #0052d9);
  color: #fff;
}
.ai-no-provider {
  font-size: 12px;
  color: var(--ordo-text-secondary, #888);
}
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
.ai-messages {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
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
  gap: 3px;
  font-size: 13px;
}
/* User turn — dimmed, marked with a subtle left rule (no chat bubble). */
.ai-user {
  color: var(--ordo-text-primary, #1a1a1a);
  font-weight: 500;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  padding-left: 9px;
  border-left: 2px solid var(--ordo-border, #dcdcdc);
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
  gap: 5px;
  font-size: 12px;
  line-height: 1.5;
  overflow: hidden;
  white-space: nowrap;
  color: var(--ordo-text-secondary, #8a8a8a);
}
.ai-tool-verb {
  flex-shrink: 0;
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
.ai-context {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  padding: 6px 12px 0;
}
.ai-pin-select {
  min-width: 160px;
}
.ai-input-bar {
  display: flex;
  gap: 6px;
  align-items: flex-end;
  padding: 10px 12px;
  border-top: 1px solid var(--ordo-border, #e7e7e7);
}
.ai-input-bar :deep(.t-textarea) {
  flex: 1;
}
</style>
