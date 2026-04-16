<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { RulesetHistoryEntry } from '@/api/types'

const props = defineProps<{
  entries: RulesetHistoryEntry[]
  currentIndex: number
  collapsed: boolean
  loading?: boolean
  syncing?: boolean
  canUndo: boolean
  canRedo: boolean
}>()

const emit = defineEmits<{
  toggle: []
  undo: []
  redo: []
  restore: [index: number]
}>()

const { t, locale } = useI18n()

const currentEntry = computed(() => props.entries[props.currentIndex] ?? null)

function formatTime(value: string) {
  return new Date(value).toLocaleTimeString(
    locale.value === 'zh-TW' ? 'zh-TW' : locale.value === 'zh-CN' ? 'zh-CN' : 'en-US',
    {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    },
  )
}

function sourceLabel(source: RulesetHistoryEntry['source']) {
  switch (source) {
    case 'save':
      return t('historyPanel.sourceSave')
    case 'restore':
      return t('historyPanel.sourceRestore')
    case 'create':
      return t('historyPanel.sourceCreate')
    case 'sync':
      return t('historyPanel.sourceSync')
    case 'edit':
    default:
      return t('historyPanel.sourceEdit')
  }
}

function sourceBadgeLabel(source: RulesetHistoryEntry['source']) {
  switch (source) {
    case 'save':
      return t('historyPanel.badgeCheckpoint')
    case 'restore':
      return t('historyPanel.badgeRestore')
    case 'create':
      return t('historyPanel.badgeCreate')
    default:
      return ''
  }
}
</script>

<template>
  <div class="history-panel" :class="{ 'is-collapsed': collapsed }">
    <button class="history-panel__toggle" @click="emit('toggle')">
      <div class="history-panel__toggle-main">
        <t-icon name="history" size="15px" />
        <span class="history-panel__title">{{ t('historyPanel.title') }}</span>
      </div>
      <div class="history-panel__toggle-meta">
        <span class="history-panel__count">{{ entries.length }}</span>
        <t-icon :name="collapsed ? 'chevron-up' : 'chevron-down'" size="14px" />
      </div>
    </button>

    <div v-if="!collapsed" class="history-panel__body">
      <div class="history-panel__toolbar">
        <button class="history-panel__action" :disabled="!canUndo" @click="emit('undo')">
          <t-icon name="rollback" size="14px" />
          <span>{{ t('historyPanel.undo') }}</span>
        </button>
        <button class="history-panel__action" :disabled="!canRedo" @click="emit('redo')">
          <t-icon name="rollfront" size="14px" />
          <span>{{ t('historyPanel.redo') }}</span>
        </button>
        <span v-if="syncing" class="history-panel__status">{{ t('historyPanel.syncing') }}</span>
      </div>

      <div v-if="currentEntry" class="history-panel__current">
        <div class="history-panel__current-label">{{ t('historyPanel.current') }}</div>
        <div class="history-panel__current-action">{{ currentEntry.action }}</div>
      </div>

      <div v-if="loading" class="history-panel__empty">
        <t-loading size="small" />
      </div>
      <div v-else-if="entries.length === 0" class="history-panel__empty">
        {{ t('historyPanel.empty') }}
      </div>
      <div v-else class="history-panel__list">
        <button
          v-for="(entry, index) in entries"
          :key="entry.id"
          class="history-entry"
          :class="{ 'is-current': index === currentIndex }"
          @click="emit('restore', index)"
        >
          <div class="history-entry__marker"></div>
          <div class="history-entry__body">
            <div class="history-entry__headline">
              <div class="history-entry__action">{{ entry.action }}</div>
              <span
                v-if="sourceBadgeLabel(entry.source)"
                class="history-entry__badge"
                :class="`is-${entry.source}`"
              >
                {{ sourceBadgeLabel(entry.source) }}
              </span>
            </div>
            <div class="history-entry__meta">
              <span>{{ sourceLabel(entry.source) }}</span>
              <span>{{ formatTime(entry.created_at) }}</span>
            </div>
          </div>
          <t-icon
            v-if="index === currentIndex"
            name="check-circle-filled"
            size="14px"
            class="history-entry__current-icon"
          />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.history-panel {
  width: 320px;
  background: color-mix(in srgb, var(--ordo-bg-panel) 94%, transparent);
  border: 1px solid var(--ordo-border-color);
  border-radius: 14px;
  box-shadow: 0 18px 40px rgba(0, 0, 0, 0.24);
  backdrop-filter: blur(14px);
  overflow: hidden;
}

.history-panel.is-collapsed {
  width: 260px;
}

.history-panel__toggle {
  width: 100%;
  padding: 10px 12px;
  border: none;
  background: transparent;
  color: var(--ordo-text-primary);
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
}

.history-panel__toggle-main,
.history-panel__toggle-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.history-panel__title {
  font-size: 12px;
  font-weight: 700;
}

.history-panel__count {
  min-width: 20px;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--ordo-active-bg);
  color: var(--ordo-text-secondary);
  font-size: 11px;
}

.history-panel__body {
  border-top: 1px solid var(--ordo-border-light);
  padding: 12px;
}

.history-panel__toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.history-panel__action {
  border: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-item);
  color: var(--ordo-text-primary);
  border-radius: 8px;
  height: 30px;
  padding: 0 10px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.history-panel__action:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.history-panel__status {
  margin-left: auto;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.history-panel__current {
  margin-bottom: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--ordo-active-bg);
}

.history-panel__current-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--ordo-text-tertiary);
  margin-bottom: 4px;
}

.history-panel__current-action {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.history-panel__list {
  max-height: 360px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.history-panel__empty {
  min-height: 120px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
}

.history-entry {
  width: 100%;
  border: 1px solid var(--ordo-border-light);
  border-radius: 10px;
  background: var(--ordo-bg-item);
  color: inherit;
  padding: 10px;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  cursor: pointer;
  text-align: left;
}

.history-entry:hover {
  border-color: var(--ordo-border-color);
  background: var(--ordo-hover-bg);
}

.history-entry.is-current {
  border-color: var(--ordo-accent);
  background: color-mix(in srgb, var(--ordo-accent-bg) 75%, transparent);
}

.history-entry__marker {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ordo-border-color);
  margin-top: 5px;
  flex-shrink: 0;
}

.history-entry.is-current .history-entry__marker {
  background: var(--ordo-accent);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--ordo-accent) 22%, transparent);
}

.history-entry__body {
  min-width: 0;
  flex: 1;
}

.history-entry__headline {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.history-entry__action {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  flex: 1;
}

.history-entry__badge {
  flex-shrink: 0;
  padding: 1px 6px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.history-entry__badge.is-save {
  background: color-mix(in srgb, var(--ordo-accent) 18%, transparent);
  color: var(--ordo-accent);
}

.history-entry__badge.is-restore {
  background: rgba(232, 148, 20, 0.16);
  color: #d68a13;
}

.history-entry__badge.is-create {
  background: rgba(41, 141, 72, 0.16);
  color: #2f8f4c;
}

.history-entry__meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.history-entry__current-icon {
  color: var(--ordo-accent);
  flex-shrink: 0;
  margin-top: 2px;
}
</style>
