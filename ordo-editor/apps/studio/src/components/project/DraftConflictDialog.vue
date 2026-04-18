<template>
  <t-dialog
    :visible="true"
    :header="$t('conflict.title')"
    width="960px"
    :footer="false"
    destroy-on-close
  >
    <div class="conflict-body">
      <p class="conflict-desc">{{ $t('conflict.description') }}</p>

      <div class="conflict-grid">
        <div class="conflict-pane">
          <div class="conflict-pane__label">{{ $t('conflict.local') }}</div>
          <pre class="conflict-pane__content">{{ localJson }}</pre>
        </div>
        <div class="conflict-pane">
          <div class="conflict-pane__label">{{ $t('conflict.server') }}</div>
          <pre class="conflict-pane__content">{{ serverJson }}</pre>
        </div>
      </div>

      <StudioDialogActions>
        <t-button variant="outline" @click="$emit('useServer')">
          {{ $t('conflict.useServer') }}
        </t-button>
        <t-button theme="primary" @click="$emit('useLocal')">
          {{ $t('conflict.useLocal') }}
        </t-button>
      </StudioDialogActions>
    </div>
  </t-dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { RuleSet } from '@ordo-engine/editor-core'
import { StudioDialogActions } from '@/components/ui'

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
.conflict-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.conflict-desc {
  margin: 0;
  font-size: 13px;
  color: var(--ordo-text-secondary);
  line-height: 1.6;
}

.conflict-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.conflict-pane {
  min-width: 0;
}

.conflict-pane__label {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  margin-bottom: 8px;
}

.conflict-pane__content {
  margin: 0;
  min-height: 360px;
  max-height: 50vh;
  overflow: auto;
  padding: 12px;
  border-radius: var(--ordo-radius-md);
  border: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  color: var(--ordo-text-primary);
  font-size: 12px;
  line-height: 1.5;
  font-family: 'JetBrains Mono', monospace;
  white-space: pre-wrap;
  word-break: break-word;
}

@media (max-width: 900px) {
  .conflict-grid {
    grid-template-columns: 1fr;
  }

  .conflict-pane__content {
    min-height: 220px;
  }
}
</style>
