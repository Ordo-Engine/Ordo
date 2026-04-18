<template>
  <div class="dialog-overlay">
    <div class="dialog">
      <h3 class="dialog-title">{{ $t('conflict.title') }}</h3>
      <p class="dialog-desc">{{ $t('conflict.description') }}</p>

      <div class="diff-pane-row">
        <div class="diff-pane">
          <div class="pane-label">{{ $t('conflict.local') }}</div>
          <pre class="diff-content">{{ localJson }}</pre>
        </div>
        <div class="diff-pane">
          <div class="pane-label">{{ $t('conflict.server') }}</div>
          <pre class="diff-content">{{ serverJson }}</pre>
        </div>
      </div>

      <div class="dialog-actions">
        <button class="btn-secondary" @click="$emit('useServer')">{{ $t('conflict.useServer') }}</button>
        <button class="btn-primary" @click="$emit('useLocal')">{{ $t('conflict.useLocal') }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { RuleSet } from '@ordo-engine/editor-core'

const props = defineProps<{
  localDraft: RuleSet
  serverDraft: RuleSet
}>()

defineEmits<{
  (e: 'useLocal'): void
  (e: 'useServer'): void
}>()

const localJson = computed(() => JSON.stringify(props.localDraft, null, 2))
const serverJson = computed(() => JSON.stringify(props.serverDraft, null, 2))
</script>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.dialog {
  background: var(--surface-color, #1e1e2e);
  border: 1px solid var(--border-color, #313244);
  border-radius: 8px;
  padding: 24px;
  width: 900px;
  max-width: 95vw;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.dialog-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.dialog-desc {
  margin: 0;
  font-size: 13px;
  color: var(--text-secondary, #a6adc8);
}
.diff-pane-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  overflow: hidden;
  flex: 1;
  min-height: 0;
}
.diff-pane {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: hidden;
}
.pane-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary, #a6adc8);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
.diff-content {
  flex: 1;
  overflow: auto;
  background: var(--code-bg, #181825);
  border: 1px solid var(--border-color, #313244);
  border-radius: 4px;
  padding: 10px;
  font-size: 11px;
  font-family: 'JetBrains Mono', monospace;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
}
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.btn-primary {
  padding: 8px 16px;
  border-radius: 4px;
  border: none;
  cursor: pointer;
  background: var(--accent-color, #cba6f7);
  color: #1e1e2e;
  font-weight: 600;
  font-size: 13px;
}
.btn-secondary {
  padding: 8px 16px;
  border-radius: 4px;
  border: 1px solid var(--border-color, #45475a);
  cursor: pointer;
  background: transparent;
  color: inherit;
  font-size: 13px;
}
</style>
