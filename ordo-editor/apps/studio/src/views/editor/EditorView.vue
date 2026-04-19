<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { useCatalogStore } from '@/stores/catalog'
import { useEnvironmentStore } from '@/stores/environment'
import { useRbacStore } from '@/stores/rbac'
import ChangeHistoryPanel from '@/components/ChangeHistoryPanel.vue'
import TestCasePanel from './TestCasePanel.vue'
import { rulesetHistoryApi } from '@/api/platform-client'
import DraftConflictDialog from '@/components/project/DraftConflictDialog.vue'
import { normalizeRuleset } from '@/utils/ruleset'
import type {
  AppendRulesetHistoryEntry,
  DraftConflictResponse,
  RulesetHistoryEntry,
  RulesetHistorySource,
} from '@/api/types'
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next'
import {
  OrdoFormEditor,
  OrdoFlowEditor,
  OrdoDecisionTable,
  OrdoExecutionPanel,
  createEmptyFlowDocument,
  createEmptyTableDocument,
  documentToRuleSet,
  decompileStepsToTable,
  compileTableToSteps,
  generateId,
  Step,
  type RuleSet,
  type DecisionTable,
} from '@ordo-engine/editor-vue'

const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const projectStore = useProjectStore()
const catalogStore = useCatalogStore()
const environmentStore = useEnvironmentStore()
const rbacStore = useRbacStore()
const { t } = useI18n()

const LOCAL_HISTORY_LIMIT = 120
const HISTORY_SYNC_DELAY_MS = 700
const EDIT_HISTORY_COMMIT_DELAY_MS = 450

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)
const rulesetNameParam = computed(() => route.params.rulesetName as string | undefined)

const projectBase = computed(() => `/orgs/${orgId.value}/projects/${projectId.value}`)

// ── Editor mode — stored per-tab so switching tabs restores correct mode ─────
const editorMode = ref<'form' | 'flow' | 'table'>('form')
const tabModes = new Map<string, 'form' | 'flow' | 'table'>()
const openMenu = ref<'file' | 'edit' | 'select' | 'view' | 'window' | null>(null)
const showHistoryPanel = ref(false)

function switchToTab(name: string) {
  if (projectStore.activeTabName) {
    flushPendingEditHistory(projectStore.activeTabName)
  }
  projectStore.activeTabName = name
  editorMode.value = tabModes.get(name) ?? 'form'
}

// ── Local history (per-tab, PS-style) ────────────────────────────────────────
interface TabHistoryState {
  entries: RulesetHistoryEntry[]
  currentIndex: number
  loaded: boolean
  loading: boolean
  syncing: boolean
}

const historyStates = ref<Record<string, TabHistoryState>>({})
const historyPanelCollapsed = ref(false)
const pendingHistoryEntries = new Map<string, AppendRulesetHistoryEntry[]>()
const historyFlushTimers = new Map<string, ReturnType<typeof setTimeout>>()
const savedRulesetSnapshots = new Map<string, string>()
const pendingEditHistory = new Map<string, { ruleset: RuleSet; action: string }>()
const editHistoryTimers = new Map<string, ReturnType<typeof setTimeout>>()

function cloneRuleset(ruleset: RuleSet): RuleSet {
  return JSON.parse(JSON.stringify(ruleset))
}

function serializeRuleset(ruleset: RuleSet): string {
  return JSON.stringify(ruleset)
}

function isSameRuleset(a: RuleSet, b: RuleSet): boolean {
  return serializeRuleset(a) === serializeRuleset(b)
}

function getHistoryState(name: string): TabHistoryState {
  if (!historyStates.value[name]) {
    historyStates.value[name] = {
      entries: [],
      currentIndex: -1,
      loaded: false,
      loading: false,
      syncing: false,
    }
  }
  return historyStates.value[name]
}

const activeHistoryState = computed(() => {
  const tab = projectStore.activeTab
  if (!tab) return null
  return getHistoryState(tab.name)
})

const activeHistoryEntries = computed(() => activeHistoryState.value?.entries ?? [])
const activeHistoryIndex = computed(() => activeHistoryState.value?.currentIndex ?? -1)
const canUndoHistory = computed(() => (activeHistoryState.value?.currentIndex ?? 0) > 0)
const canRedoHistory = computed(() => {
  const state = activeHistoryState.value
  if (!state) return false
  return state.currentIndex < state.entries.length - 1
})

function createHistoryEntry(
  rulesetName: string,
  ruleset: RuleSet,
  action: string,
  source: RulesetHistorySource,
): RulesetHistoryEntry {
  return {
    id: `history_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`,
    ruleset_name: rulesetName,
    action,
    source,
    created_at: new Date().toISOString(),
    author_id: auth.user?.id ?? 'local',
    author_email: auth.user?.email ?? '',
    author_display_name: auth.user?.display_name ?? '',
    snapshot: cloneRuleset(ruleset),
  }
}

function syncDecisionTableFromRuleset(name: string, ruleset: RuleSet) {
  const metaTableStr = ruleset.config.metadata?._table
  if (metaTableStr) {
    try {
      decisionTables.value[name] = JSON.parse(metaTableStr)
      return
    } catch {
      // fall through to decompile
    }
  }

  const table = decompileStepsToTable(ruleset.steps, ruleset.startStepId)
  if (table) {
    decisionTables.value[name] = table
  } else {
    delete decisionTables.value[name]
  }
}

function updateRulesetState(name: string, ruleset: RuleSet) {
  const snapshot = cloneRuleset(ruleset)
  const savedSnapshot = savedRulesetSnapshots.get(name)
  const dirty = savedSnapshot ? savedSnapshot !== serializeRuleset(snapshot) : true
  projectStore.setTabRuleset(name, snapshot, dirty)
  syncDecisionTableFromRuleset(name, snapshot)
}

function buildHistoryAction(previous: RuleSet, next: RuleSet) {
  if (next.steps.length > previous.steps.length) {
    return t('historyPanel.actionAddStep')
  }
  if (next.steps.length < previous.steps.length) {
    return t('historyPanel.actionRemoveStep')
  }
  if (previous.startStepId !== next.startStepId) {
    return t('historyPanel.actionSetStart')
  }
  if (
    previous.config.name !== next.config.name ||
    previous.config.version !== next.config.version ||
    previous.config.description !== next.config.description
  ) {
    return t('historyPanel.actionUpdateSettings')
  }
  if (editorMode.value === 'table') {
    return t('historyPanel.actionEditTable')
  }
  if (editorMode.value === 'flow') {
    return t('historyPanel.actionEditFlow')
  }
  return t('historyPanel.actionEditRuleset')
}

function queueHistoryPersistence(name: string, entry: RulesetHistoryEntry) {
  if (!canEdit.value) return

  const queue = pendingHistoryEntries.get(name) ?? []
  queue.push({
    id: entry.id,
    action: entry.action,
    source: entry.source,
    created_at: entry.created_at,
    snapshot: cloneRuleset(entry.snapshot),
  })
  pendingHistoryEntries.set(name, queue)

  const timer = historyFlushTimers.get(name)
  if (timer) clearTimeout(timer)

  const state = getHistoryState(name)
  state.syncing = true

  historyFlushTimers.set(
    name,
    setTimeout(() => {
      void flushHistoryQueue(name)
    }, HISTORY_SYNC_DELAY_MS),
  )
}

function flushPendingEditHistory(name: string) {
  const timer = editHistoryTimers.get(name)
  if (timer) {
    clearTimeout(timer)
    editHistoryTimers.delete(name)
  }

  const pending = pendingEditHistory.get(name)
  if (!pending) return

  pendingEditHistory.delete(name)
  pushHistoryEntry(name, pending.ruleset, pending.action, 'edit')
}

function scheduleEditHistoryEntry(name: string, ruleset: RuleSet, action: string) {
  pendingEditHistory.set(name, {
    ruleset: cloneRuleset(ruleset),
    action,
  })

  const timer = editHistoryTimers.get(name)
  if (timer) clearTimeout(timer)

  editHistoryTimers.set(
    name,
    setTimeout(() => {
      flushPendingEditHistory(name)
    }, EDIT_HISTORY_COMMIT_DELAY_MS),
  )
}

async function flushHistoryQueue(name: string) {
  const timer = historyFlushTimers.get(name)
  if (timer) {
    clearTimeout(timer)
    historyFlushTimers.delete(name)
  }

  const queue = pendingHistoryEntries.get(name)
  const state = getHistoryState(name)
  if (!queue?.length || !auth.token || !projectStore.currentProject) {
    state.syncing = false
    return
  }

  pendingHistoryEntries.delete(name)

  try {
    await rulesetHistoryApi.append(auth.token, projectStore.currentProject.id, name, queue)
    state.syncing = false
  } catch (error) {
    const retryQueue = [...queue, ...(pendingHistoryEntries.get(name) ?? [])]
    pendingHistoryEntries.set(name, retryQueue)
    state.syncing = true
  }
}

function pushHistoryEntry(
  name: string,
  ruleset: RuleSet,
  action: string,
  source: RulesetHistorySource,
  persist = true,
) {
  const state = getHistoryState(name)
  const currentEntry = state.entries[state.currentIndex]

  if (source === 'edit' && currentEntry?.snapshot && isSameRuleset(currentEntry.snapshot, ruleset)) {
    return
  }

  if (state.currentIndex < state.entries.length - 1) {
    state.entries = state.entries.slice(0, state.currentIndex + 1)
  }

  state.entries.push(createHistoryEntry(name, ruleset, action, source))
  if (state.entries.length > LOCAL_HISTORY_LIMIT) {
    state.entries = state.entries.slice(state.entries.length - LOCAL_HISTORY_LIMIT)
  }
  state.currentIndex = state.entries.length - 1

  if (persist) {
    queueHistoryPersistence(name, state.entries[state.currentIndex])
  }
}

function applyHistoryIndex(name: string, index: number) {
  const state = getHistoryState(name)
  const entry = state.entries[index]
  if (!entry) return

  state.currentIndex = index
  updateRulesetState(name, entry.snapshot)
}

function undoHistory() {
  const tab = projectStore.activeTab
  if (!tab || !canUndoHistory.value) return
  flushPendingEditHistory(tab.name)
  applyHistoryIndex(tab.name, activeHistoryIndex.value - 1)
}

function redoHistory() {
  const tab = projectStore.activeTab
  if (!tab || !canRedoHistory.value) return
  flushPendingEditHistory(tab.name)
  applyHistoryIndex(tab.name, activeHistoryIndex.value + 1)
}

function restoreHistory(index: number) {
  const tab = projectStore.activeTab
  if (!tab) return
  flushPendingEditHistory(tab.name)

  const state = getHistoryState(tab.name)
  const entry = state.entries[index]
  if (!entry) return

  if (isSameRuleset(tab.ruleset, entry.snapshot)) {
    state.currentIndex = index
    return
  }

  updateRulesetState(tab.name, entry.snapshot)
  pushHistoryEntry(
    tab.name,
    entry.snapshot,
    t('historyPanel.actionRestoreSnapshot', { action: entry.action }),
    'restore',
  )
}

function resetTabHistory(name: string) {
  const editTimer = editHistoryTimers.get(name)
  if (editTimer) {
    clearTimeout(editTimer)
    editHistoryTimers.delete(name)
  }
  pendingEditHistory.delete(name)

  const timer = historyFlushTimers.get(name)
  if (timer) {
    clearTimeout(timer)
    historyFlushTimers.delete(name)
  }
  pendingHistoryEntries.delete(name)
  savedRulesetSnapshots.delete(name)
  delete historyStates.value[name]
}

async function disposeTabHistory(name: string) {
  await flushHistoryQueue(name)
  resetTabHistory(name)
}

async function ensureHistoryLoaded(name: string, ruleset: RuleSet) {
  const state = getHistoryState(name)
  if (state.loaded || state.loading) return

  state.loading = true
  const currentSnapshot = cloneRuleset(ruleset)
  const loadedEntries: RulesetHistoryEntry[] = []

  try {
    if (auth.token && projectStore.currentProject) {
      const response = await rulesetHistoryApi.list(auth.token, projectStore.currentProject.id, name)
      loadedEntries.push(
        ...response.entries.map((entry) => ({
          ...entry,
          snapshot: cloneRuleset(entry.snapshot),
        })),
      )
    }
  } catch (error) {
    console.error('[history] failed to load ruleset history:', error)
  }

  if (loadedEntries.length === 0 || !isSameRuleset(loadedEntries[loadedEntries.length - 1].snapshot, currentSnapshot)) {
    loadedEntries.push(
      createHistoryEntry(name, currentSnapshot, t('historyPanel.actionOpenCurrent'), 'sync'),
    )
  }

  state.entries = loadedEntries
  state.currentIndex = loadedEntries.length - 1
  state.loaded = true
  state.loading = false
  state.syncing = false

  savedRulesetSnapshots.set(name, serializeRuleset(currentSnapshot))
}

// ── Execution panel ──────────────────────────────────────────────────────────
const showExecution = ref(false)
const executionHeight = ref(280)

// ── Test case panel ───────────────────────────────────────────────────────────
const showTests = ref(false)
const testsHeight = ref(280)

function toggleTests() {
  showTests.value = !showTests.value
  if (showTests.value) showExecution.value = false
}

function toggleExecution() {
  showExecution.value = !showExecution.value
  if (showExecution.value) showTests.value = false
}

// ── Execution trace overlay (for "show in flow") ─────────────────────────────
const executionTrace = ref<{
  path: string[]
  steps: Array<{ id: string; name: string; duration_us: number; result?: string | null }>
  resultCode: string
  resultMessage: string
  output?: Record<string, any>
} | null>(null)

function handleShowInFlow(trace: typeof executionTrace.value) {
  executionTrace.value = trace
  setEditorMode('flow')
}

function handleClearFlowTrace() {
  executionTrace.value = null
}

function handleShowAsFlow() {
  setEditorMode('flow')
}

// ── Create dialog ─────────────────────────────────────────────────────────────
const showCreate = ref(false)
const creating = ref(false)
const newName = ref('')
const newType = ref<'flow' | 'table'>('flow')
const saving = ref(false)
const conflictState = ref<{
  rulesetName: string
  localDraft: RuleSet
  serverDraft: RuleSet
  serverSeq: number
} | null>(null)

// ── Permissions ───────────────────────────────────────────────────────────────
const canEdit = computed(() => {
  if (!auth.user) return false
  return rbacStore.can('ruleset:edit') || orgStore.canEdit(auth.user.id)
})

const canAdmin = computed(() => {
  if (!auth.user) return false
  return rbacStore.can('project:manage') || orgStore.canAdmin(auth.user.id)
})

const canPublish = computed(() => {
  if (!auth.user) return false
  return rbacStore.can('ruleset:publish') || orgStore.canAdmin(auth.user.id)
})

// ── Table support ──────────────────────────────────────────────────────────────
const decisionTables = ref<Record<string, DecisionTable>>({})

const activeDecisionTable = computed(() => {
  const tab = projectStore.activeTab
  if (!tab) return null
  return decisionTables.value[tab.name] ?? null
})

function handleTableChange(table: DecisionTable) {
  const tab = projectStore.activeTab
  if (!tab) return
  decisionTables.value[tab.name] = table

  const result = compileTableToSteps(table)

  const nextRuleset: RuleSet = {
    ...tab.ruleset,
    steps: result.steps,
    startStepId: result.startStepId,
    config: {
      ...tab.ruleset.config,
      metadata: {
        ...(tab.ruleset.config.metadata ?? {}),
        _table: JSON.stringify(table),
      },
    },
  }

  updateRulesetState(tab.name, nextRuleset)
  scheduleEditHistoryEntry(tab.name, nextRuleset, t('historyPanel.actionEditTable'))
}

// ── Lifecycle ──────────────────────────────────────────────────────────────────
onMounted(async () => {
  if (!projectStore.currentProject || projectStore.currentProject.id !== projectId.value) {
    const project = projectStore.projects.find((p) => p.id === projectId.value)
    if (project) {
      await projectStore.selectProject(project)
    }
  }
  await projectStore.fetchRulesets()
  await rbacStore.fetchRoles(orgId.value)
  await rbacStore.fetchMyRoles(orgId.value)
  await environmentStore.fetchEnvironments(orgId.value, projectId.value)

  // Open ruleset from URL param
  if (rulesetNameParam.value) {
    await openRuleset(rulesetNameParam.value)
  } else if (projectStore.rulesets.length > 0 && projectStore.openTabs.length === 0) {
    await openRuleset(projectStore.rulesets[0].name)
  }
})

watch(
  () => rulesetNameParam.value,
  async (name) => {
    if (name) await openRuleset(name)
  },
)

function onKeydown(e: KeyboardEvent) {
  const key = e.key.toLowerCase()
  const isPrimary = e.ctrlKey || e.metaKey

  if (!isPrimary) return

  if (key === 's') {
    e.preventDefault()
    if (projectStore.activeTab) handleSave(projectStore.activeTab.name)
    return
  }

  if (key === 'z') {
    e.preventDefault()
    if (e.shiftKey) {
      redoHistory()
    } else {
      undoHistory()
    }
    return
  }

  if (key === 'y') {
    e.preventDefault()
    redoHistory()
  }
}

function closeMenus() {
  openMenu.value = null
}

function toggleMenu(menu: 'file' | 'edit' | 'select' | 'view' | 'window') {
  openMenu.value = openMenu.value === menu ? null : menu
}

function hoverMenu(menu: 'file' | 'edit' | 'select' | 'view' | 'window') {
  if (openMenu.value) {
    openMenu.value = menu
  }
}

function onDocumentPointerDown(event: MouseEvent) {
  const target = event.target as HTMLElement | null
  if (!target?.closest('.editor-menubar')) {
    closeMenus()
  }
}

function runMenuAction(action: () => void) {
  closeMenus()
  action()
}

onMounted(() => document.addEventListener('keydown', onKeydown))
onUnmounted(() => document.removeEventListener('keydown', onKeydown))
onMounted(() => document.addEventListener('mousedown', onDocumentPointerDown))
onUnmounted(() => document.removeEventListener('mousedown', onDocumentPointerDown))

// ── Actions ───────────────────────────────────────────────────────────────────
async function openRuleset(name: string) {
  try {
    await projectStore.openRuleset(name)
    const tab = projectStore.openTabs.find((t) => t.name === name)
    if (tab) {
      syncDecisionTableFromRuleset(name, tab.ruleset)
      editorMode.value = canBeTable(tab.ruleset) ? 'table' : 'form'
      await ensureHistoryLoaded(name, tab.ruleset)
    }
    tabModes.set(name, editorMode.value)
    router.replace(`${projectBase.value}/editor/${encodeURIComponent(name)}`)
  } catch (e: any) {
    MessagePlugin.error(e.message || t('editor.loadFailed'))
  }
}

function canBeTable(rs: RuleSet): boolean {
  try {
    return !!decompileStepsToTable(rs.steps, rs.startStepId)
  } catch {
    return false
  }
}

function handleRulesetChange(ruleset: RuleSet) {
  const tab = projectStore.activeTab
  if (!tab) return

  const action = buildHistoryAction(tab.ruleset, ruleset)
  updateRulesetState(tab.name, ruleset)
  scheduleEditHistoryEntry(tab.name, ruleset, action)
}

async function handleSave(name: string) {
  if (!canEdit.value) {
    MessagePlugin.warning(t('editor.noPermission'))
    return
  }
  saving.value = true
  try {
    flushPendingEditHistory(name)
    const result = await projectStore.saveRuleset(name)
    if (result?.conflict) {
      const tab = projectStore.openTabs.find((item) => item.name === name)
      if (!tab) {
        MessagePlugin.error(t('editor.saveFailed'))
        return
      }
      conflictState.value = {
        rulesetName: name,
        localDraft: cloneRuleset(tab.ruleset),
        serverDraft: cloneRuleset(normalizeRuleset(result.server_draft, name)),
        serverSeq: result.server_seq,
      }
      return
    }
    const tab = projectStore.openTabs.find((item) => item.name === name)
    if (tab) {
      savedRulesetSnapshots.set(name, serializeRuleset(tab.ruleset))
      projectStore.setTabRuleset(name, cloneRuleset(tab.ruleset), false)
      pushHistoryEntry(name, tab.ruleset, t('historyPanel.actionSaveCheckpoint'), 'save')
      await flushHistoryQueue(name)
    }
    MessagePlugin.success(t('editor.saveSuccess'))
  } catch (e: any) {
    MessagePlugin.error(e.message || t('editor.saveFailed'))
  } finally {
    saving.value = false
  }
}

async function resolveConflictUseServer() {
  const conflict = conflictState.value
  if (!conflict) return
  const tab = projectStore.openTabs.find((item) => item.name === conflict.rulesetName)
  if (!tab) {
    conflictState.value = null
    return
  }

  tab.draft_seq = conflict.serverSeq
  savedRulesetSnapshots.set(conflict.rulesetName, serializeRuleset(conflict.serverDraft))
  projectStore.setTabRuleset(conflict.rulesetName, cloneRuleset(conflict.serverDraft), false)
  syncDecisionTableFromRuleset(conflict.rulesetName, conflict.serverDraft)
  conflictState.value = null
  MessagePlugin.success(t('conflict.useServerSuccess'))
}

async function resolveConflictUseLocal() {
  const conflict = conflictState.value
  if (!conflict) return
  const tab = projectStore.openTabs.find((item) => item.name === conflict.rulesetName)
  if (!tab) {
    conflictState.value = null
    return
  }

  tab.draft_seq = conflict.serverSeq
  projectStore.setTabRuleset(conflict.rulesetName, cloneRuleset(conflict.localDraft), true)
  conflictState.value = null
  await handleSave(conflict.rulesetName)
}

function openReleaseCenter() {
  if (!projectStore.activeTab) return
  router.push({
    name: 'project-release-requests',
    query: { ruleset: projectStore.activeTab.name },
  })
}

function handleCloseTab(name: string) {
  const tab = projectStore.openTabs.find((t) => t.name === name)
  if (tab?.dirty) {
    const dlg = DialogPlugin.confirm({
      header: t('editor.closeConfirm'),
      body: t('editor.closeConfirmBody', { name }),
      confirmBtn: { content: t('editor.closeConfirmBtn'), theme: 'danger' },
      cancelBtn: t('common.cancel'),
      onConfirm: async () => {
        projectStore.closeTab(name)
        await disposeTabHistory(name)
        dlg.hide()
        if (!projectStore.activeTabName) {
          router.replace(`${projectBase.value}/editor`)
        }
      },
    })
  } else {
    projectStore.closeTab(name)
    void disposeTabHistory(name)
    if (!projectStore.activeTabName) {
      router.replace(`${projectBase.value}/editor`)
    }
  }
}

async function handleCreateRuleset() {
  if (!newName.value.trim()) {
    MessagePlugin.warning(t('editor.nameRequired'))
    return
  }
  creating.value = true
  try {
    let rs: RuleSet
    const name = newName.value.trim()

    if (newType.value === 'table') {
      const doc = createEmptyTableDocument(name)
      rs = documentToRuleSet(doc)
    } else {
      // Flow: Decision → Terminal
      const decisionId = generateId()
      const terminalId = generateId()
      rs = {
        config: { name, version: '1.0.0' },
        startStepId: decisionId,
        steps: [
          Step.decision({
            id: decisionId,
            name: 'start',
            branches: [],
            defaultNextStepId: terminalId,
          }),
          Step.terminal({
            id: terminalId,
            name: 'result',
            code: 'DEFAULT',
          }),
        ],
      }
    }

    await projectStore.createRuleset(rs)
    showCreate.value = false
    newName.value = ''
    MessagePlugin.success(t('editor.createSuccess'))
    await openRuleset(name)
    pushHistoryEntry(name, rs, t('historyPanel.actionCreateRuleset'), 'create')
    showHistoryPanel.value = true
  } catch (e: any) {
    MessagePlugin.error(e.message || t('editor.createFailed'))
  } finally {
    creating.value = false
  }
}

function handleDeleteRuleset(name: string) {
  const dlg = DialogPlugin.confirm({
    header: t('editor.deleteDialog'),
    body: t('editor.deleteConfirm', { name }),
    confirmBtn: { content: t('editor.deleteConfirmBtn'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await projectStore.deleteRuleset(name)
        await disposeTabHistory(name)
        dlg.hide()
        MessagePlugin.success(t('editor.deleteSuccess'))
        if (!projectStore.activeTabName && projectStore.rulesets.length > 0) {
          await openRuleset(projectStore.rulesets[0].name)
        }
      } catch (e: any) {
        MessagePlugin.error(e.message)
      }
    },
  })
}

function setEditorMode(mode: 'form' | 'flow' | 'table') {
  const tab = projectStore.activeTab
  if (!tab) return
  if (mode === 'table' && !canBeTable(tab.ruleset)) {
    MessagePlugin.warning(t('editor.tableUnsupported'))
    return
  }
  editorMode.value = mode
  tabModes.set(tab.name, mode)
}

onUnmounted(() => {
  for (const name of Array.from(editHistoryTimers.keys())) {
    flushPendingEditHistory(name)
  }
  for (const name of Array.from(historyFlushTimers.keys())) {
    void flushHistoryQueue(name)
  }
})
</script>

<template>
  <div class="editor-shell">
    <!-- ── Left sidebar: ruleset list ── -->
    <aside class="ruleset-sidebar">
      <div class="ruleset-sidebar__header">
        <span class="ruleset-sidebar__title">
          {{ projectStore.currentProject?.name ?? t('editor.newRuleset') }}
        </span>
        <button
          v-if="canEdit"
          class="sidebar-btn"
          :title="t('editor.newRuleset')"
          @click="showCreate = true"
        >
          <t-icon name="add" size="16px" />
        </button>
      </div>

      <div class="ruleset-sidebar__list">
        <div v-if="projectStore.loading" class="sidebar-empty">
          <t-loading size="small" />
        </div>
        <div v-else-if="projectStore.rulesets.length === 0" class="sidebar-empty">
          {{ t('editor.noRulesets') }}
        </div>
        <div
          v-for="rs in projectStore.rulesets"
          :key="rs.name"
          class="ruleset-item"
          :class="{ 'is-active': rs.name === projectStore.activeTabName }"
          @click="openRuleset(rs.name)"
          @contextmenu.prevent="() => {}"
        >
          <t-icon
            name="file-code"
            size="14px"
            class="ruleset-item__icon"
          />
          <span class="ruleset-item__name">{{ rs.name }}</span>
          <span
            v-if="projectStore.openTabs.find((t) => t.name === rs.name)?.dirty"
            class="ruleset-item__dot"
            :title="t('editor.unsaved')"
          />
          <button
            v-if="canAdmin"
            class="ruleset-item__del"
            :title="t('editor.deleteTitle')"
            @click.stop="handleDeleteRuleset(rs.name)"
          >
            <t-icon name="close" size="12px" />
          </button>
        </div>
      </div>
    </aside>

    <!-- ── Main area ── -->
    <div class="editor-main">
      <div class="editor-menubar" v-if="projectStore.openTabs.length > 0">
        <div
          class="editor-menu"
          @mouseenter="hoverMenu('file')"
        >
          <button class="editor-menu__trigger" :class="{ 'is-open': openMenu === 'file' }" @click="toggleMenu('file')">
            {{ t('menuBar.file') }}
          </button>
          <div v-if="openMenu === 'file'" class="editor-menu__dropdown">
            <button class="editor-menu__item" :disabled="!canEdit" @click="runMenuAction(() => (showCreate = true))">
              <span>{{ t('menuBar.newRuleset') }}</span>
            </button>
            <button
              class="editor-menu__item"
              :disabled="!canEdit || !projectStore.activeTab"
              @click="runMenuAction(() => projectStore.activeTab && handleSave(projectStore.activeTab.name))"
            >
              <span>{{ t('menuBar.save') }}</span>
              <span class="editor-menu__shortcut">Ctrl+S</span>
            </button>
            <button
              class="editor-menu__item"
              :disabled="!canPublish || !projectStore.activeTab"
              @click="runMenuAction(openReleaseCenter)"
            >
              <span>{{ t('releaseCenter.createRequest') }}</span>
            </button>
          </div>
        </div>

        <div class="editor-menu" @mouseenter="hoverMenu('edit')">
          <button class="editor-menu__trigger" :class="{ 'is-open': openMenu === 'edit' }" @click="toggleMenu('edit')">
            {{ t('menuBar.edit') }}
          </button>
          <div v-if="openMenu === 'edit'" class="editor-menu__dropdown">
            <button class="editor-menu__item" :disabled="!canUndoHistory" @click="runMenuAction(undoHistory)">
              <span>{{ t('menuBar.undo') }}</span>
              <span class="editor-menu__shortcut">Ctrl+Z</span>
            </button>
            <button class="editor-menu__item" :disabled="!canRedoHistory" @click="runMenuAction(redoHistory)">
              <span>{{ t('menuBar.redo') }}</span>
              <span class="editor-menu__shortcut">Ctrl+Shift+Z</span>
            </button>
          </div>
        </div>

        <div class="editor-menu" @mouseenter="hoverMenu('select')">
          <button class="editor-menu__trigger" :class="{ 'is-open': openMenu === 'select' }" @click="toggleMenu('select')">
            {{ t('menuBar.select') }}
          </button>
          <div v-if="openMenu === 'select'" class="editor-menu__dropdown">
            <button class="editor-menu__item is-disabled" disabled>
              <span>{{ t('menuBar.selectionSoon') }}</span>
            </button>
          </div>
        </div>

        <div class="editor-menu" @mouseenter="hoverMenu('view')">
          <button class="editor-menu__trigger" :class="{ 'is-open': openMenu === 'view' }" @click="toggleMenu('view')">
            {{ t('menuBar.view') }}
          </button>
          <div v-if="openMenu === 'view'" class="editor-menu__dropdown">
            <button class="editor-menu__item" :disabled="editorMode === 'form'" @click="runMenuAction(() => setEditorMode('form'))">
              <span>{{ t('editor.formMode') }}</span>
            </button>
            <button class="editor-menu__item" :disabled="editorMode === 'flow'" @click="runMenuAction(() => setEditorMode('flow'))">
              <span>{{ t('editor.flowMode') }}</span>
            </button>
            <button class="editor-menu__item" :disabled="editorMode === 'table'" @click="runMenuAction(() => setEditorMode('table'))">
              <span>{{ t('editor.tableMode') }}</span>
            </button>
          </div>
        </div>

        <div class="editor-menu" @mouseenter="hoverMenu('window')">
          <button class="editor-menu__trigger" :class="{ 'is-open': openMenu === 'window' }" @click="toggleMenu('window')">
            {{ t('menuBar.window') }}
          </button>
          <div v-if="openMenu === 'window'" class="editor-menu__dropdown">
            <button
              class="editor-menu__item"
              @click="runMenuAction(() => {
                showHistoryPanel = !showHistoryPanel
                if (showHistoryPanel) historyPanelCollapsed = false
              })"
            >
              <span>{{ t('menuBar.history') }}</span>
              <t-icon v-if="showHistoryPanel" name="check" size="13px" />
            </button>
            <button class="editor-menu__item" @click="runMenuAction(() => toggleExecution())">
              <span>{{ t('menuBar.executionPanel') }}</span>
              <t-icon v-if="showExecution" name="check" size="13px" />
            </button>
          </div>
        </div>
      </div>

      <!-- Tabs bar -->
      <div class="editor-tabs" v-if="projectStore.openTabs.length > 0">
        <div
          v-for="tab in projectStore.openTabs"
          :key="tab.name"
          class="editor-tab"
          :class="{ 'is-active': tab.name === projectStore.activeTabName }"
          @click="switchToTab(tab.name)"
        >
          <t-icon name="file-code" size="13px" class="tab-icon" />
          <span class="tab-name">{{ tab.name }}</span>
          <span v-if="tab.dirty" class="tab-dot" :title="t('editor.unsaved')" />
          <button
            class="tab-close"
            @click.stop="handleCloseTab(tab.name)"
            :title="t('editor.close')"
          >
            <t-icon name="close" size="12px" />
          </button>
        </div>

        <!-- Mode switcher + actions on the right -->
        <div class="editor-tabs__spacer" />
        <div class="editor-tabs__actions" v-if="projectStore.activeTab">
          <div class="mode-switch">
            <button
              class="mode-btn"
              :class="{ 'is-active': editorMode === 'form' }"
              @click="setEditorMode('form')"
              :title="t('editor.formMode')"
            >
              <t-icon name="view-list" size="14px" />
            </button>
            <button
              class="mode-btn"
              :class="{ 'is-active': editorMode === 'flow' }"
              @click="setEditorMode('flow')"
              :title="t('editor.flowMode')"
            >
              <t-icon name="view-module" size="14px" />
            </button>
            <button
              class="mode-btn"
              :class="{ 'is-active': editorMode === 'table' }"
              @click="setEditorMode('table')"
              :title="t('editor.tableMode')"
            >
              <t-icon name="table" size="14px" />
            </button>
          </div>
          <div class="tab-divider" />
          <button
            class="toolbar-btn"
            :class="{ 'is-active': showExecution }"
            :title="t('editor.execPanel')"
            @click="toggleExecution"
          >
            <t-icon name="play-circle" size="15px" />
          </button>
          <button
            class="toolbar-btn"
            :class="{ 'is-active': showTests }"
            :title="t('test.panel.title')"
            @click="toggleTests"
          >
            <t-icon name="task-checked" size="15px" />
          </button>
          <button
            v-if="canEdit"
            class="toolbar-btn toolbar-btn--save"
            :class="{ 'is-loading': saving }"
            :title="t('editor.saveTitle')"
            @click="projectStore.activeTab && handleSave(projectStore.activeTab.name)"
          >
            <t-icon name="save" size="15px" />
          </button>
          <button
            v-if="canPublish"
            class="toolbar-btn toolbar-btn--publish"
            :title="t('releaseCenter.createRequest')"
            @click="openReleaseCenter"
          >
            <t-icon name="upload" size="15px" />
          </button>
        </div>
      </div>

      <!-- Empty state (no tab open) -->
      <div v-if="projectStore.openTabs.length === 0" class="editor-empty">
        <div class="editor-empty__inner">
          <t-icon name="file-code" size="48px" class="editor-empty__icon" />
          <p class="editor-empty__title">{{ t('editor.emptyHint') }}</p>
          <t-button v-if="canEdit" variant="outline" @click="showCreate = true">
            <t-icon name="add" />
            {{ t('editor.newRulesetBtn') }}
          </t-button>
        </div>
      </div>

      <!-- Editor area + execution panel -->
      <div v-else class="editor-body">
        <div
          class="editor-canvas"
          :style="showExecution ? { flex: 'none', height: `calc(100% - ${executionHeight}px - 2px)` } : showTests ? { flex: 'none', height: `calc(100% - ${testsHeight}px - 2px)` } : {}"
        >
          <template v-if="projectStore.activeTab">
            <!-- Form mode -->
            <OrdoFormEditor
              v-if="editorMode === 'form'"
              :model-value="projectStore.activeTab.ruleset"
              :disabled="!canEdit"
              :input-schema="catalogStore.schemaFields.length ? catalogStore.schemaFields : undefined"
              @update:model-value="handleRulesetChange"
            />
            <!-- Flow mode -->
            <OrdoFlowEditor
              v-else-if="editorMode === 'flow'"
              :model-value="projectStore.activeTab.ruleset"
              :disabled="!canEdit"
              :execution-trace="executionTrace"
              @update:model-value="handleRulesetChange"
            />
            <!-- Decision table mode -->
            <OrdoDecisionTable
              v-else-if="editorMode === 'table' && activeDecisionTable"
              :model-value="activeDecisionTable"
              :disabled="!canEdit"
              @update:model-value="handleTableChange"
              @show-as-flow="handleShowAsFlow"
            />
          </template>
        </div>

        <!-- Execution panel -->
        <div
          v-if="showExecution && projectStore.activeTab"
          class="execution-panel-wrap"
          :style="{ height: executionHeight + 'px' }"
        >
          <OrdoExecutionPanel
            :ruleset="projectStore.activeTab.ruleset"
            :visible="showExecution"
            :height="executionHeight"
            @update:visible="showExecution = $event"
            @update:height="executionHeight = $event"
            @show-in-flow="handleShowInFlow"
            @clear-flow-trace="handleClearFlowTrace"
          />
        </div>

        <!-- Test case panel -->
        <TestCasePanel
          v-if="showTests"
          :project-id="projectId"
          :ruleset-name="projectStore.activeTab?.name ?? ''"
          :visible="showTests"
          :height="testsHeight"
          @update:visible="showTests = $event"
          @update:height="testsHeight = $event"
        />
      </div>

      <div
        v-if="projectStore.activeTab && showHistoryPanel"
        class="history-panel-wrap"
        :style="{ bottom: (showExecution ? executionHeight : showTests ? testsHeight : 0) + 20 + 'px' }"
      >
        <ChangeHistoryPanel
          :entries="activeHistoryEntries"
          :current-index="activeHistoryIndex"
          :collapsed="historyPanelCollapsed"
          :loading="activeHistoryState?.loading"
          :syncing="activeHistoryState?.syncing"
          :can-undo="canUndoHistory"
          :can-redo="canRedoHistory"
          @toggle="historyPanelCollapsed = !historyPanelCollapsed"
          @undo="undoHistory"
          @redo="redoHistory"
          @restore="restoreHistory"
        />
      </div>
    </div>

    <!-- Create ruleset dialog -->
    <t-dialog
      v-model:visible="showCreate"
      :header="t('editor.createDialog')"
      :confirm-btn="{ content: t('common.create'), loading: creating }"
      @confirm="handleCreateRuleset"
      @close="showCreate = false"
      width="440px"
    >
      <t-form label-align="top">
        <t-form-item :label="t('editor.nameLabel')" required>
          <t-input
            v-model="newName"
            :placeholder="t('editor.namePlaceholder')"
            autofocus
            @keyup.enter="handleCreateRuleset"
          />
        </t-form-item>
        <t-form-item :label="t('editor.typeLabel')">
          <t-radio-group v-model="newType" variant="default-filled">
            <t-radio-button value="flow">
              <t-icon name="flowchart" size="14px" />
              {{ t('editor.flowType') }}
            </t-radio-button>
            <t-radio-button value="table">
              <t-icon name="table" size="14px" />
              {{ t('editor.tableType') }}
            </t-radio-button>
          </t-radio-group>
        </t-form-item>
      </t-form>
    </t-dialog>

    <DraftConflictDialog
      v-if="conflictState"
      :local-draft="conflictState.localDraft"
      :server-draft="conflictState.serverDraft"
      @use-local="resolveConflictUseLocal"
      @use-server="resolveConflictUseServer"
    />
  </div>
</template>

<style scoped>
/* ── Layout ── */
.editor-shell {
  display: flex;
  height: 100%;
  overflow: hidden;
  background: var(--ordo-bg-app);
}

/* ── Left sidebar ── */
.ruleset-sidebar {
  width: 200px;
  flex-shrink: 0;
  background: var(--ordo-bg-panel);
  border-right: 1px solid var(--ordo-border-color);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.ruleset-sidebar__header {
  height: 36px;
  padding: 0 8px 0 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
}

.ruleset-sidebar__title {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--ordo-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.sidebar-btn {
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--ordo-text-secondary);
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.sidebar-btn:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.ruleset-sidebar__list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.sidebar-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px 12px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.ruleset-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  cursor: pointer;
  border-radius: 0;
  font-size: 13px;
  color: var(--ordo-text-secondary);
  position: relative;
  user-select: none;
}

.ruleset-item:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.ruleset-item.is-active {
  background: var(--ordo-active-bg);
  color: var(--ordo-text-primary);
}

.ruleset-item__icon {
  flex-shrink: 0;
  opacity: 0.6;
}

.ruleset-item__name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  font-family: 'JetBrains Mono', monospace;
}

.ruleset-item__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ordo-accent);
  flex-shrink: 0;
}

.ruleset-item__del {
  display: none;
  width: 18px;
  height: 18px;
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  border-radius: 3px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.ruleset-item:hover .ruleset-item__del {
  display: flex;
}

.ruleset-item__del:hover {
  background: rgba(255, 80, 80, 0.15);
  color: #e34d59;
}

/* ── Main ── */
.editor-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
  position: relative;
}

.editor-menubar {
  height: 30px;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 0 8px;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
  position: relative;
  z-index: 20;
}

.editor-menu {
  position: relative;
}

.editor-menu__trigger {
  height: 24px;
  border: none;
  background: transparent;
  color: var(--ordo-text-secondary);
  padding: 0 10px;
  border-radius: 6px;
  font-size: 12px;
  cursor: pointer;
}

.editor-menu__trigger:hover,
.editor-menu__trigger.is-open {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.editor-menu__dropdown {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  min-width: 220px;
  padding: 6px;
  border-radius: 10px;
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  box-shadow: 0 16px 36px rgba(0, 0, 0, 0.24);
}

.editor-menu__item {
  width: 100%;
  height: 32px;
  border: none;
  background: transparent;
  color: var(--ordo-text-primary);
  border-radius: 8px;
  padding: 0 10px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-size: 12px;
  cursor: pointer;
  text-align: left;
}

.editor-menu__item:hover:not(:disabled) {
  background: var(--ordo-hover-bg);
}

.editor-menu__item:disabled,
.editor-menu__item.is-disabled {
  color: var(--ordo-text-tertiary);
  cursor: not-allowed;
}

.editor-menu__shortcut {
  color: var(--ordo-text-tertiary);
  font-size: 11px;
}

/* ── Tabs ── */
.editor-tabs {
  height: 36px;
  display: flex;
  align-items: stretch;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
  overflow-x: auto;
  overflow-y: hidden;
}

.editor-tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 12px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  cursor: pointer;
  border-right: 1px solid var(--ordo-border-color);
  white-space: nowrap;
  min-width: 0;
  max-width: 180px;
  position: relative;
}

.editor-tab:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.editor-tab.is-active {
  background: var(--ordo-bg-app);
  color: var(--ordo-text-primary);
}

.editor-tab.is-active::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 1px;
  background: var(--ordo-accent);
}

.tab-icon {
  flex-shrink: 0;
  opacity: 0.6;
}

.tab-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
}

.tab-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ordo-accent);
  flex-shrink: 0;
}

.tab-close {
  width: 18px;
  height: 18px;
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  border-radius: 3px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  opacity: 0;
}

.editor-tab:hover .tab-close,
.editor-tab.is-active .tab-close {
  opacity: 1;
}

.tab-close:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.editor-tabs__spacer {
  flex: 1;
}

.editor-tabs__actions {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 0 8px;
  flex-shrink: 0;
}

.mode-switch {
  display: flex;
  align-items: center;
  gap: 1px;
  background: var(--ordo-bg-app);
  border: 1px solid var(--ordo-border-color);
  border-radius: 4px;
  padding: 2px;
}

.mode-btn {
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--ordo-text-secondary);
  border-radius: 3px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.1s, color 0.1s;
}

.mode-btn:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.mode-btn.is-active {
  background: var(--ordo-accent);
  color: #fff;
}

.tab-divider {
  width: 1px;
  height: 18px;
  background: var(--ordo-border-color);
  margin: 0 4px;
}

.toolbar-btn {
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--ordo-text-secondary);
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.1s, color 0.1s;
}

.toolbar-btn:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.toolbar-btn.is-active {
  color: var(--ordo-accent);
}

.toolbar-btn--save:hover {
  color: var(--ordo-accent);
}

/* ── Body ── */
.editor-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.editor-canvas {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.execution-panel-wrap {
  flex-shrink: 0;
  border-top: 1px solid var(--ordo-border-color);
  overflow: hidden;
}

.history-panel-wrap {
  position: absolute;
  right: 20px;
  z-index: 15;
  transition: bottom 0.18s ease;
}

/* ── Empty ── */
.editor-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.editor-empty__inner {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.editor-empty__icon {
  color: var(--ordo-text-tertiary);
}

.editor-empty__title {
  font-size: 14px;
  color: var(--ordo-text-secondary);
  margin: 0;
}
</style>
