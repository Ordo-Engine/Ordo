<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useAiStore } from '@/stores/ai-agent';

const props = defineProps<{ orgId: string; projectId: string }>();
const emit = defineEmits<{ (e: 'close'): void }>();

const { t } = useI18n();
const ai = useAiStore();
const input = ref('');

onMounted(() => ai.init(props.orgId, props.projectId));

// Flat provider·model options for the selector.
const modelOptions = computed(() =>
  ai.providers.flatMap((p) =>
    p.models.map((m) => ({ value: `${p.id}::${m.id}`, label: `${p.label} · ${m.label}` }))
  )
);
const selectedModel = computed({
  get: () => (ai.provider && ai.modelId ? `${ai.provider}::${ai.modelId}` : ''),
  set: (v: string) => {
    const [p, m] = v.split('::');
    ai.selectModel(p, m);
  },
});

const pendingTitle = computed(() => {
  const c = ai.pending?.call;
  if (!c) return '';
  if (c.name === 'publish') return t('ai.confirmPublish');
  if (c.name === 'delete_file') return t('ai.confirmDelete');
  return c.name;
});

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
        <t-button
          v-if="ai.canUndo"
          size="small"
          variant="text"
          theme="warning"
          @click="ai.undoLastChange"
        >
          {{ t('ai.undo') }}
        </t-button>
        <t-button size="small" variant="text" @click="ai.reset">{{ t('ai.clear') }}</t-button>
        <t-button size="small" variant="text" shape="square" @click="emit('close')">✕</t-button>
      </div>
    </header>

    <div class="ai-model-bar">
      <t-select
        v-if="modelOptions.length"
        v-model="selectedModel"
        size="small"
        :placeholder="t('ai.selectModel')"
      >
        <t-option v-for="o in modelOptions" :key="o.value" :value="o.value" :label="o.label" />
      </t-select>
      <span v-else class="ai-no-provider">{{ t('ai.noProvider') }}</span>
    </div>

    <div v-if="ai.touchedFiles.length" class="ai-changed">
      <span class="ai-changed-label">{{ t('ai.changedFiles') }}</span>
      <t-tag v-for="f in ai.touchedFiles" :key="f" size="small" variant="outline">{{ f }}</t-tag>
    </div>

    <div class="ai-messages">
      <div v-if="!ai.messages.length" class="ai-empty">
        <p>{{ t('ai.emptyTitle') }}</p>
        <p class="ai-empty-hint">{{ t('ai.emptyHint') }}</p>
      </div>

      <div v-for="(msg, i) in ai.messages" :key="i" class="ai-msg" :class="msg.role">
        <div v-if="msg.text" class="ai-bubble">{{ msg.text }}</div>
        <div v-if="msg.tools.length" class="ai-tools">
          <t-tag
            v-for="(tool, j) in msg.tools"
            :key="j"
            size="small"
            variant="light"
            :theme="tool.ok ? 'success' : 'danger'"
          >
            {{ tool.name }}
          </t-tag>
        </div>
      </div>

      <div v-if="ai.running" class="ai-running">{{ t('ai.thinking') }}</div>
      <t-alert v-if="ai.error" theme="error" :message="ai.error" class="ai-error" />
    </div>

    <!-- High-risk confirmation card -->
    <div v-if="ai.pending" class="ai-confirm">
      <p class="ai-confirm-title">⚠ {{ pendingTitle }}</p>
      <pre class="ai-confirm-input">{{ JSON.stringify(ai.pending.call.input, null, 2) }}</pre>
      <div class="ai-confirm-actions">
        <t-button size="small" variant="outline" @click="ai.rejectPending">{{
          t('ai.reject')
        }}</t-button>
        <t-button size="small" theme="danger" @click="ai.approvePending">{{
          t('ai.approve')
        }}</t-button>
      </div>
    </div>

    <footer class="ai-input-bar">
      <t-textarea
        v-model="input"
        :placeholder="t('ai.placeholder')"
        :autosize="{ minRows: 1, maxRows: 5 }"
        :disabled="ai.running || !ai.ready"
        @keydown.enter.exact.prevent="submit"
      />
      <t-button
        theme="primary"
        size="small"
        :loading="ai.running"
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
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border, #e7e7e7);
}
.ai-no-provider {
  font-size: 12px;
  color: var(--ordo-text-secondary, #888);
}
.ai-changed {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  border-bottom: 1px solid var(--ordo-border, #e7e7e7);
}
.ai-changed-label {
  font-size: 11px;
  color: var(--ordo-text-secondary, #888);
  margin-right: 4px;
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
  gap: 4px;
}
.ai-msg.user {
  align-items: flex-end;
}
.ai-bubble {
  max-width: 92%;
  padding: 8px 10px;
  border-radius: 10px;
  font-size: 13px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
.ai-msg.user .ai-bubble {
  background: var(--ordo-brand, #0052d9);
  color: #fff;
}
.ai-msg.assistant .ai-bubble {
  background: var(--ordo-bg-sunken, #f3f3f3);
  color: var(--ordo-text-primary, #1a1a1a);
}
.ai-tools {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
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
.ai-input-bar {
  display: flex;
  gap: 8px;
  align-items: flex-end;
  padding: 10px 12px;
  border-top: 1px solid var(--ordo-border, #e7e7e7);
}
.ai-input-bar :deep(.t-textarea) {
  flex: 1;
}
</style>
