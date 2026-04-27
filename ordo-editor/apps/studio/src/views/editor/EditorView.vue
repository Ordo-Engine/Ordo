<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useAuthStore } from '@/stores/auth';
import { useOrgStore } from '@/stores/org';
import { useProjectStore } from '@/stores/project';
import { useCatalogStore } from '@/stores/catalog';
import { useEnvironmentStore } from '@/stores/environment';
import { useRbacStore } from '@/stores/rbac';
import ChangeHistoryPanel from '@/components/ChangeHistoryPanel.vue';
import TestCasePanel from './TestCasePanel.vue';
import { rulesetHistoryApi, subRuleApi } from '@/api/platform-client';
import DraftConflictDialog from '@/components/project/DraftConflictDialog.vue';
import { normalizeRuleset } from '@/utils/ruleset';
import { getCurrentVersionDisplay, stripVersionSuffix } from '@/utils/ruleset-version';
import type {
  AppendRulesetHistoryEntry,
  DraftConflictResponse,
  RulesetHistoryEntry,
  RulesetHistorySource,
  SubRuleAssetMeta,
} from '@/api/types';
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next';
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
  type SubRuleStep,
  type SubRuleAssetOption,
  type ExtractSubRulePayload,
  type ExtractSubRuleRequest,
  type DecisionTable,
} from '@ordo-engine/editor-vue';

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const orgStore = useOrgStore();
const projectStore = useProjectStore();
const catalogStore = useCatalogStore();
const environmentStore = useEnvironmentStore();
const rbacStore = useRbacStore();
const { t } = useI18n();

const LOCAL_HISTORY_LIMIT = 120;
const HISTORY_SYNC_DELAY_MS = 700;
const EDIT_HISTORY_COMMIT_DELAY_MS = 450;
const pendingSubRuleAssets = new Set<string>();

const orgId = computed(() => route.params.orgId as string);
const projectId = computed(() => route.params.projectId as string);
const rulesetNameParam = computed(() => route.params.rulesetName as string | undefined);

const projectBase = computed(() => `/orgs/${orgId.value}/projects/${projectId.value}`);

// ── Editor mode — stored per-tab so switching tabs restores correct mode ─────
const editorMode = ref<'form' | 'flow' | 'table'>('form');
const tabModes = new Map<string, 'form' | 'flow' | 'table'>();
const openMenu = ref<'file' | 'edit' | 'select' | 'view' | 'window' | null>(null);
const showHistoryPanel = ref(false);

function switchToTab(name: string) {
  if (projectStore.activeTabName) {
    flushPendingEditHistory(projectStore.activeTabName);
  }
  projectStore.activeTabName = name;
  editorMode.value = tabModes.get(name) ?? 'form';
}

// ── Local history (per-tab, PS-style) ────────────────────────────────────────
interface TabHistoryState {
  entries: RulesetHistoryEntry[];
  currentIndex: number;
  loaded: boolean;
  loading: boolean;
  syncing: boolean;
}

const historyStates = ref<Record<string, TabHistoryState>>({});
const historyPanelCollapsed = ref(false);
const pendingHistoryEntries = new Map<string, AppendRulesetHistoryEntry[]>();
const historyFlushTimers = new Map<string, ReturnType<typeof setTimeout>>();
const savedRulesetSnapshots = new Map<string, string>();
const pendingEditHistory = new Map<string, { ruleset: RuleSet; action: string }>();
const editHistoryTimers = new Map<string, ReturnType<typeof setTimeout>>();

function cloneRuleset(ruleset: RuleSet): RuleSet {
  return JSON.parse(JSON.stringify(ruleset));
}

function serializeRuleset(ruleset: RuleSet): string {
  return JSON.stringify(ruleset);
}

function isSameRuleset(a: RuleSet, b: RuleSet): boolean {
  return serializeRuleset(a) === serializeRuleset(b);
}

function getHistoryState(name: string): TabHistoryState {
  if (!historyStates.value[name]) {
    historyStates.value[name] = {
      entries: [],
      currentIndex: -1,
      loaded: false,
      loading: false,
      syncing: false,
    };
  }
  return historyStates.value[name];
}

const activeHistoryState = computed(() => {
  const tab = projectStore.activeTab;
  if (!tab) return null;
  return getHistoryState(tab.name);
});

const activeHistoryEntries = computed(() => activeHistoryState.value?.entries ?? []);
const activeHistoryIndex = computed(() => activeHistoryState.value?.currentIndex ?? -1);
const canUndoHistory = computed(() => (activeHistoryState.value?.currentIndex ?? 0) > 0);
const canRedoHistory = computed(() => {
  const state = activeHistoryState.value;
  if (!state) return false;
  return state.currentIndex < state.entries.length - 1;
});

function createHistoryEntry(
  rulesetName: string,
  ruleset: RuleSet,
  action: string,
  source: RulesetHistorySource
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
  };
}

function syncDecisionTableFromRuleset(name: string, ruleset: RuleSet) {
  const metaTableStr = ruleset.config.metadata?._table;
  if (metaTableStr) {
    try {
      decisionTables.value[name] = JSON.parse(metaTableStr);
      return;
    } catch {
      // fall through to decompile
    }
  }

  const table = decompileStepsToTable(ruleset.steps, ruleset.startStepId);
  if (table) {
    decisionTables.value[name] = table;
  } else {
    delete decisionTables.value[name];
  }
}

function updateRulesetState(name: string, ruleset: RuleSet) {
  const snapshot = cloneRuleset(ruleset);
  const savedSnapshot = savedRulesetSnapshots.get(name);
  const dirty = savedSnapshot ? savedSnapshot !== serializeRuleset(snapshot) : true;
  projectStore.setTabRuleset(name, snapshot, dirty);
  syncDecisionTableFromRuleset(name, snapshot);
}

function buildHistoryAction(previous: RuleSet, next: RuleSet) {
  if (next.steps.length > previous.steps.length) {
    return t('historyPanel.actionAddStep');
  }
  if (next.steps.length < previous.steps.length) {
    return t('historyPanel.actionRemoveStep');
  }
  if (previous.startStepId !== next.startStepId) {
    return t('historyPanel.actionSetStart');
  }
  if (
    previous.config.name !== next.config.name ||
    previous.config.version !== next.config.version ||
    previous.config.description !== next.config.description
  ) {
    return t('historyPanel.actionUpdateSettings');
  }
  if (editorMode.value === 'table') {
    return t('historyPanel.actionEditTable');
  }
  if (editorMode.value === 'flow') {
    return t('historyPanel.actionEditFlow');
  }
  return t('historyPanel.actionEditRuleset');
}

function queueHistoryPersistence(name: string, entry: RulesetHistoryEntry) {
  if (!canEdit.value) return;

  const queue = pendingHistoryEntries.get(name) ?? [];
  queue.push({
    id: entry.id,
    action: entry.action,
    source: entry.source,
    created_at: entry.created_at,
    snapshot: cloneRuleset(entry.snapshot),
  });
  pendingHistoryEntries.set(name, queue);

  const timer = historyFlushTimers.get(name);
  if (timer) clearTimeout(timer);

  const state = getHistoryState(name);
  state.syncing = true;

  historyFlushTimers.set(
    name,
    setTimeout(() => {
      void flushHistoryQueue(name);
    }, HISTORY_SYNC_DELAY_MS)
  );
}

function flushPendingEditHistory(name: string) {
  const timer = editHistoryTimers.get(name);
  if (timer) {
    clearTimeout(timer);
    editHistoryTimers.delete(name);
  }

  const pending = pendingEditHistory.get(name);
  if (!pending) return;

  pendingEditHistory.delete(name);
  pushHistoryEntry(name, pending.ruleset, pending.action, 'edit');
}

function scheduleEditHistoryEntry(name: string, ruleset: RuleSet, action: string) {
  pendingEditHistory.set(name, {
    ruleset: cloneRuleset(ruleset),
    action,
  });

  const timer = editHistoryTimers.get(name);
  if (timer) clearTimeout(timer);

  editHistoryTimers.set(
    name,
    setTimeout(() => {
      flushPendingEditHistory(name);
    }, EDIT_HISTORY_COMMIT_DELAY_MS)
  );
}

async function flushHistoryQueue(name: string) {
  const timer = historyFlushTimers.get(name);
  if (timer) {
    clearTimeout(timer);
    historyFlushTimers.delete(name);
  }

  const queue = pendingHistoryEntries.get(name);
  const state = getHistoryState(name);
  if (!queue?.length || !auth.token || !projectStore.currentProject) {
    state.syncing = false;
    return;
  }

  pendingHistoryEntries.delete(name);

  try {
    await rulesetHistoryApi.append(auth.token, projectStore.currentProject.id, name, queue);
    state.syncing = false;
  } catch (error) {
    const retryQueue = [...queue, ...(pendingHistoryEntries.get(name) ?? [])];
    pendingHistoryEntries.set(name, retryQueue);
    state.syncing = true;
  }
}

function pushHistoryEntry(
  name: string,
  ruleset: RuleSet,
  action: string,
  source: RulesetHistorySource,
  persist = true
) {
  const state = getHistoryState(name);
  const currentEntry = state.entries[state.currentIndex];

  if (
    source === 'edit' &&
    currentEntry?.snapshot &&
    isSameRuleset(currentEntry.snapshot, ruleset)
  ) {
    return;
  }

  if (state.currentIndex < state.entries.length - 1) {
    state.entries = state.entries.slice(0, state.currentIndex + 1);
  }

  state.entries.push(createHistoryEntry(name, ruleset, action, source));
  if (state.entries.length > LOCAL_HISTORY_LIMIT) {
    state.entries = state.entries.slice(state.entries.length - LOCAL_HISTORY_LIMIT);
  }
  state.currentIndex = state.entries.length - 1;

  if (persist) {
    queueHistoryPersistence(name, state.entries[state.currentIndex]);
  }
}

function applyHistoryIndex(name: string, index: number) {
  const state = getHistoryState(name);
  const entry = state.entries[index];
  if (!entry) return;

  state.currentIndex = index;
  updateRulesetState(name, entry.snapshot);
}

function undoHistory() {
  const tab = projectStore.activeTab;
  if (!tab || !canUndoHistory.value) return;
  flushPendingEditHistory(tab.name);
  applyHistoryIndex(tab.name, activeHistoryIndex.value - 1);
}

function redoHistory() {
  const tab = projectStore.activeTab;
  if (!tab || !canRedoHistory.value) return;
  flushPendingEditHistory(tab.name);
  applyHistoryIndex(tab.name, activeHistoryIndex.value + 1);
}

function restoreHistory(index: number) {
  const tab = projectStore.activeTab;
  if (!tab) return;
  flushPendingEditHistory(tab.name);

  const state = getHistoryState(tab.name);
  const entry = state.entries[index];
  if (!entry) return;

  if (isSameRuleset(tab.ruleset, entry.snapshot)) {
    state.currentIndex = index;
    return;
  }

  updateRulesetState(tab.name, entry.snapshot);
  pushHistoryEntry(
    tab.name,
    entry.snapshot,
    t('historyPanel.actionRestoreSnapshot', { action: entry.action }),
    'restore'
  );
}

function resetTabHistory(name: string) {
  const editTimer = editHistoryTimers.get(name);
  if (editTimer) {
    clearTimeout(editTimer);
    editHistoryTimers.delete(name);
  }
  pendingEditHistory.delete(name);

  const timer = historyFlushTimers.get(name);
  if (timer) {
    clearTimeout(timer);
    historyFlushTimers.delete(name);
  }
  pendingHistoryEntries.delete(name);
  savedRulesetSnapshots.delete(name);
  delete historyStates.value[name];
}

async function disposeTabHistory(name: string) {
  await flushHistoryQueue(name);
  resetTabHistory(name);
}

async function ensureHistoryLoaded(name: string, ruleset: RuleSet) {
  const state = getHistoryState(name);
  if (state.loaded || state.loading) return;

  state.loading = true;
  const currentSnapshot = cloneRuleset(ruleset);
  const loadedEntries: RulesetHistoryEntry[] = [];

  try {
    if (auth.token && projectStore.currentProject) {
      const response = await rulesetHistoryApi.list(
        auth.token,
        projectStore.currentProject.id,
        name
      );
      loadedEntries.push(
        ...response.entries.map((entry) => ({
          ...entry,
          snapshot: cloneRuleset(entry.snapshot),
        }))
      );
    }
  } catch (error) {
    console.error('[history] failed to load ruleset history:', error);
  }

  if (
    loadedEntries.length === 0 ||
    !isSameRuleset(loadedEntries[loadedEntries.length - 1].snapshot, currentSnapshot)
  ) {
    loadedEntries.push(
      createHistoryEntry(name, currentSnapshot, t('historyPanel.actionOpenCurrent'), 'sync')
    );
  }

  state.entries = loadedEntries;
  state.currentIndex = loadedEntries.length - 1;
  state.loaded = true;
  state.loading = false;
  state.syncing = false;

  savedRulesetSnapshots.set(name, serializeRuleset(currentSnapshot));
}

// ── Execution panel ──────────────────────────────────────────────────────────
const showExecution = ref(false);
const executionHeight = ref(280);

// ── Test case panel ───────────────────────────────────────────────────────────
const showTests = ref(false);
const testsHeight = ref(280);

function toggleTests() {
  showTests.value = !showTests.value;
  if (showTests.value) showExecution.value = false;
}

function toggleExecution() {
  showExecution.value = !showExecution.value;
  if (showExecution.value) showTests.value = false;
}

// ── Execution trace overlay (for "show in flow") ─────────────────────────────
const executionTrace = ref<{
  path: string[];
  steps: Array<{ id: string; name: string; duration_us: number; result?: string | null }>;
  resultCode: string;
  resultMessage: string;
  output?: Record<string, any>;
} | null>(null);
const flowTraceMode = ref(false);

function handleShowInFlow(trace: typeof executionTrace.value) {
  executionTrace.value = trace
    ? { ...trace, steps: [...trace.steps], path: [...trace.path] }
    : null;
  flowTraceMode.value = true;
  setEditorMode('flow');
}

async function handleOpenSubRuleTrace(payload: {
  refName: string;
  trace: NonNullable<typeof executionTrace.value>;
}) {
  await handleOpenSubRule(payload.refName);
  executionTrace.value = {
    ...payload.trace,
    steps: [...payload.trace.steps],
    path: [...payload.trace.path],
  };
  flowTraceMode.value = true;
  setEditorMode('flow');
}

function handleClearFlowTrace() {
  executionTrace.value = null;
  flowTraceMode.value = false;
}

function handleShowAsFlow() {
  setEditorMode('flow');
}

// ── Create dialog ─────────────────────────────────────────────────────────────
const showCreate = ref(false);
const creating = ref(false);
const newName = ref('');
const newType = ref<'flow' | 'table'>('flow');
const saving = ref(false);
const subRuleAssets = ref<SubRuleAssetMeta[]>([]);
const subRuleAssetsLoaded = ref(false);
const subRuleParentTabs = new Map<string, string>();
const extractSubRuleRequest = ref<ExtractSubRuleRequest | null>(null);
const extractingSubRule = ref(false);
const extractSubRuleState = ref<{
  parentTabName: string;
  payload: ExtractSubRulePayload;
  name: string;
  displayName: string;
  description: string;
} | null>(null);
let extractSubRuleRequestSeq = 0;
const conflictState = ref<{
  rulesetName: string;
  localDraft: RuleSet;
  serverDraft: RuleSet;
  serverSeq: number;
} | null>(null);

// ── Permissions ───────────────────────────────────────────────────────────────
const canEdit = computed(() => {
  if (!auth.user) return false;
  return rbacStore.can('ruleset:edit') || orgStore.canEdit(auth.user.id);
});

const canAdmin = computed(() => {
  if (!auth.user) return false;
  return rbacStore.can('project:manage') || orgStore.canAdmin(auth.user.id);
});

const canPublish = computed(() => {
  if (!auth.user) return false;
  return rbacStore.can('ruleset:publish') || orgStore.canAdmin(auth.user.id);
});

const activeRulesetMeta = computed(() => {
  const tab = projectStore.activeTab;
  if (!tab) return null;
  return projectStore.draftMetas.find((item) => item.name === tab.name) ?? null;
});

const activeDraftVersion = computed(() =>
  stripVersionSuffix(projectStore.activeTab?.ruleset.config.version)
);

const activePublishedVersion = computed(() =>
  stripVersionSuffix(activeRulesetMeta.value?.published_version)
);

const activeVersionDisplay = computed(() =>
  getCurrentVersionDisplay(
    activeHistoryEntries.value,
    projectStore.activeTab?.ruleset.config.version
  )
);

const requiresVersionBump = computed(
  () => !!activePublishedVersion.value && activePublishedVersion.value === activeDraftVersion.value
);

const subRuleAssetOptions = computed<SubRuleAssetOption[]>(() =>
  subRuleAssets.value.map((asset) => ({
    name: asset.name,
    scope: asset.scope,
    displayName: asset.display_name,
    description: asset.description,
  }))
);

const activeSubRuleName = computed(() => {
  const tab = projectStore.activeTab;
  if (tab?.kind !== 'sub_rule') return null;
  return tab.name.startsWith('§') ? tab.name.slice(1) : tab.name;
});

const activeSubRuleParentName = computed(() => {
  const tab = projectStore.activeTab;
  if (!tab || tab.kind !== 'sub_rule') return null;
  return subRuleParentTabs.get(tab.name) ?? null;
});

type RulesetStep = RuleSet['steps'][number];

interface RulesetGraphEdge {
  source: string;
  target: string;
}

interface SubRuleSuggestion {
  id: string;
  kind: 'group' | 'decision' | 'chain';
  title: string;
  description: string;
  stepIds: string[];
  entryStepId: string;
  entryName: string;
  stepCount: number;
  score: number;
}

interface SubRuleCandidateValidation {
  entryId: string;
  exitTargetId?: string;
}

const activeSubRuleSuggestions = computed<SubRuleSuggestion[]>(() => {
  const tab = projectStore.activeTab;
  if (!tab || tab.kind === 'sub_rule' || tab.ruleset.steps.length < 3) return [];
  return analyzeSubRuleSuggestions(tab.ruleset).slice(0, 3);
});

function getStepOutgoingIds(step: RulesetStep): string[] {
  switch (step.type) {
    case 'decision':
      return [...step.branches.map((branch) => branch.nextStepId), step.defaultNextStepId].filter(
        Boolean
      );
    case 'action':
    case 'sub_rule':
      return step.nextStepId ? [step.nextStepId] : [];
    case 'terminal':
    default:
      return [];
  }
}

function buildRulesetGraph(ruleset: RuleSet) {
  const stepMap = new Map(ruleset.steps.map((step) => [step.id, step]));
  const edges: RulesetGraphEdge[] = [];
  const incoming = new Map<string, RulesetGraphEdge[]>();
  const outgoing = new Map<string, RulesetGraphEdge[]>();

  for (const step of ruleset.steps) {
    incoming.set(step.id, []);
    outgoing.set(step.id, []);
  }

  for (const step of ruleset.steps) {
    for (const target of getStepOutgoingIds(step)) {
      if (!stepMap.has(target)) continue;
      const edge = { source: step.id, target };
      edges.push(edge);
      outgoing.get(step.id)?.push(edge);
      incoming.get(target)?.push(edge);
    }
  }

  return { stepMap, edges, incoming, outgoing };
}

function validateSubRuleCandidate(
  ruleset: RuleSet,
  stepIds: string[]
): SubRuleCandidateValidation | null {
  const selectedStepIds = new Set(stepIds);
  if (selectedStepIds.size < 2) return null;

  const graph = buildRulesetGraph(ruleset);
  const selectedSteps = ruleset.steps.filter((step) => selectedStepIds.has(step.id));
  if (selectedSteps.length !== selectedStepIds.size) return null;

  const internalEdges = graph.edges.filter(
    (edge) => selectedStepIds.has(edge.source) && selectedStepIds.has(edge.target)
  );
  const externalIncomingEdges = graph.edges.filter(
    (edge) => !selectedStepIds.has(edge.source) && selectedStepIds.has(edge.target)
  );
  const externalOutgoingEdges = graph.edges.filter(
    (edge) => selectedStepIds.has(edge.source) && !selectedStepIds.has(edge.target)
  );

  const incomingTargets = new Set(externalIncomingEdges.map((edge) => edge.target));
  if (incomingTargets.size > 1) return null;

  const internalIncomingTargets = new Set(internalEdges.map((edge) => edge.target));
  let entryId: string | undefined;
  if (incomingTargets.size === 1) {
    entryId = [...incomingTargets][0];
  } else if (selectedStepIds.has(ruleset.startStepId)) {
    entryId = ruleset.startStepId;
  } else {
    const rootSteps = selectedSteps.filter((step) => !internalIncomingTargets.has(step.id));
    if (rootSteps.length !== 1) return null;
    entryId = rootSteps[0].id;
  }

  const reachable = new Set<string>();
  const stack = [entryId];
  while (stack.length > 0) {
    const current = stack.pop()!;
    if (reachable.has(current)) continue;
    reachable.add(current);
    for (const edge of internalEdges) {
      if (edge.source === current && !reachable.has(edge.target)) {
        stack.push(edge.target);
      }
    }
  }
  if (reachable.size !== selectedSteps.length) return null;

  const exitTargets = new Set(externalOutgoingEdges.map((edge) => edge.target));
  if (exitTargets.size > 1) return null;

  const hasTerminal = selectedSteps.some((step) => step.type === 'terminal');
  if (hasTerminal && externalOutgoingEdges.length > 0) return null;
  if (!hasTerminal && externalOutgoingEdges.length === 0) return null;

  return {
    entryId,
    exitTargetId: exitTargets.size === 1 ? [...exitTargets][0] : undefined,
  };
}

function collectDownstreamRegion(ruleset: RuleSet, startStepId: string, limit = 10): string[] {
  const graph = buildRulesetGraph(ruleset);
  const selected = new Set<string>([startStepId]);
  const queue = [...(graph.outgoing.get(startStepId) ?? []).map((edge) => edge.target)];

  while (queue.length > 0 && selected.size < limit) {
    const current = queue.shift()!;
    if (selected.has(current) || !graph.stepMap.has(current)) continue;

    const hasOutsideIncoming = (graph.incoming.get(current) ?? []).some(
      (edge) => !selected.has(edge.source)
    );
    if (hasOutsideIncoming) continue;

    selected.add(current);
    const step = graph.stepMap.get(current);
    if (!step || step.type === 'terminal') continue;

    for (const edge of graph.outgoing.get(current) ?? []) {
      if (!selected.has(edge.target)) queue.push(edge.target);
    }
  }

  return ruleset.steps.map((step) => step.id).filter((id) => selected.has(id));
}

function collectLinearRegion(ruleset: RuleSet, startStepId: string, limit = 8): string[] {
  const graph = buildRulesetGraph(ruleset);
  const selected = new Set<string>([startStepId]);
  let current = startStepId;

  while (selected.size < limit) {
    const outgoingEdges = graph.outgoing.get(current) ?? [];
    if (outgoingEdges.length !== 1) break;

    const next = outgoingEdges[0].target;
    if (selected.has(next) || !graph.stepMap.has(next)) break;

    const incomingEdges = graph.incoming.get(next) ?? [];
    if (incomingEdges.length !== 1) break;

    selected.add(next);
    const nextStep = graph.stepMap.get(next);
    if (!nextStep || nextStep.type === 'terminal') break;
    current = next;
  }

  return ruleset.steps.map((step) => step.id).filter((id) => selected.has(id));
}

function analyzeSubRuleSuggestions(ruleset: RuleSet): SubRuleSuggestion[] {
  const graph = buildRulesetGraph(ruleset);
  const suggestions: SubRuleSuggestion[] = [];
  const seen = new Set<string>();

  function pushSuggestion(
    kind: SubRuleSuggestion['kind'],
    title: string,
    description: string,
    stepIds: string[],
    score: number
  ) {
    const candidateIds = ruleset.steps
      .map((step) => step.id)
      .filter((id) => stepIds.includes(id) && graph.stepMap.has(id));
    const validation = validateSubRuleCandidate(ruleset, candidateIds);
    if (!validation) return;

    const key = candidateIds.slice().sort().join('|');
    if (seen.has(key)) return;
    seen.add(key);

    const entryStep = graph.stepMap.get(validation.entryId);
    suggestions.push({
      id: `${kind}:${key}`,
      kind,
      title,
      description,
      stepIds: candidateIds,
      entryStepId: validation.entryId,
      entryName: entryStep?.name ?? validation.entryId,
      stepCount: candidateIds.length,
      score: score + candidateIds.length,
    });
  }

  for (const group of ruleset.groups ?? []) {
    if (group.stepIds.length < 2) continue;
    pushSuggestion(
      'group',
      t('subRules.suggestionGroupTitle', { name: group.name }),
      group.description || t('subRules.suggestionGroupDesc'),
      group.stepIds,
      90
    );
  }

  for (const step of ruleset.steps) {
    if (step.type === 'decision' && step.branches.length >= 2) {
      const region = collectDownstreamRegion(ruleset, step.id);
      if (region.length >= 3) {
        pushSuggestion(
          'decision',
          t('subRules.suggestionDecisionTitle', { name: step.name }),
          t('subRules.suggestionDecisionDesc', { count: region.length }),
          region,
          75
        );
      }
    }
  }

  for (const step of ruleset.steps) {
    if (step.type === 'terminal') continue;

    const incomingEdges = graph.incoming.get(step.id) ?? [];
    if (incomingEdges.length === 1) {
      const previousOutgoing = graph.outgoing.get(incomingEdges[0].source) ?? [];
      if (previousOutgoing.length === 1) continue;
    }

    const region = collectLinearRegion(ruleset, step.id);
    if (region.length >= 3) {
      pushSuggestion(
        'chain',
        t('subRules.suggestionChainTitle', { name: step.name }),
        t('subRules.suggestionChainDesc', { count: region.length }),
        region,
        55
      );
    }
  }

  return suggestions.sort((a, b) => b.score - a.score);
}

async function requestSuggestedSubRuleExtraction(suggestion: SubRuleSuggestion) {
  if (!canEdit.value || activeSubRuleName.value) return;

  setEditorMode('flow');
  await nextTick();
  extractSubRuleRequest.value = {
    id: ++extractSubRuleRequestSeq,
    stepIds: suggestion.stepIds,
  };
}

function handleExtractSubRuleInvalid(reason: string) {
  MessagePlugin.warning(reason);
}

// ── Table support ──────────────────────────────────────────────────────────────
const decisionTables = ref<Record<string, DecisionTable>>({});

const activeDecisionTable = computed(() => {
  const tab = projectStore.activeTab;
  if (!tab) return null;
  return decisionTables.value[tab.name] ?? null;
});

function handleTableChange(table: DecisionTable) {
  const tab = projectStore.activeTab;
  if (!tab) return;
  decisionTables.value[tab.name] = table;

  const result = compileTableToSteps(table);

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
  };

  updateRulesetState(tab.name, nextRuleset);
  scheduleEditHistoryEntry(tab.name, nextRuleset, t('historyPanel.actionEditTable'));
}

// ── Lifecycle ──────────────────────────────────────────────────────────────────
onMounted(async () => {
  if (!projectStore.currentProject || projectStore.currentProject.id !== projectId.value) {
    const project = projectStore.projects.find((p) => p.id === projectId.value);
    if (project) {
      await projectStore.selectProject(project);
    }
  }
  await projectStore.fetchRulesets();
  await rbacStore.fetchRoles(orgId.value);
  await rbacStore.fetchMyRoles(orgId.value);
  await environmentStore.fetchEnvironments(orgId.value, projectId.value);
  await refreshSubRuleAssets();

  // Open ruleset or sub-rule from URL param
  if (rulesetNameParam.value) {
    await openTabFromParam(rulesetNameParam.value);
  } else if (projectStore.rulesets.length > 0 && projectStore.openTabs.length === 0) {
    await openRuleset(projectStore.rulesets[0].name);
  }
});

async function openTabFromParam(name: string) {
  if (name.startsWith('§')) {
    const refName = name.slice(1);
    try {
      await projectStore.openSubRule(refName, 'project');
      tabModes.set(name, 'flow');
    } catch {
      await openRuleset(projectStore.rulesets[0]?.name ?? '');
    }
  } else {
    await openRuleset(name);
  }
}

watch(
  () => rulesetNameParam.value,
  async (name) => {
    if (name) await openTabFromParam(name);
  }
);

function onKeydown(e: KeyboardEvent) {
  const key = e.key.toLowerCase();
  const isPrimary = e.ctrlKey || e.metaKey;

  if (!isPrimary) return;

  if (key === 's') {
    e.preventDefault();
    if (projectStore.activeTab) handleSave(projectStore.activeTab.name);
    return;
  }

  if (key === 'z') {
    e.preventDefault();
    if (e.shiftKey) {
      redoHistory();
    } else {
      undoHistory();
    }
    return;
  }

  if (key === 'y') {
    e.preventDefault();
    redoHistory();
  }
}

function closeMenus() {
  openMenu.value = null;
}

function toggleMenu(menu: 'file' | 'edit' | 'select' | 'view' | 'window') {
  openMenu.value = openMenu.value === menu ? null : menu;
}

function hoverMenu(menu: 'file' | 'edit' | 'select' | 'view' | 'window') {
  if (openMenu.value) {
    openMenu.value = menu;
  }
}

function onDocumentPointerDown(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (!target?.closest('.editor-menubar')) {
    closeMenus();
  }
}

function runMenuAction(action: () => void) {
  closeMenus();
  action();
}

onMounted(() => document.addEventListener('keydown', onKeydown));
onUnmounted(() => document.removeEventListener('keydown', onKeydown));
onMounted(() => document.addEventListener('mousedown', onDocumentPointerDown));
onUnmounted(() => document.removeEventListener('mousedown', onDocumentPointerDown));

// ── Actions ───────────────────────────────────────────────────────────────────
async function openRuleset(name: string) {
  try {
    await projectStore.openRuleset(name);
    const tab = projectStore.openTabs.find((t) => t.name === name);
    if (tab) {
      syncDecisionTableFromRuleset(name, tab.ruleset);
      editorMode.value = canBeTable(tab.ruleset) ? 'table' : 'form';
      await ensureHistoryLoaded(name, tab.ruleset);
    }
    tabModes.set(name, editorMode.value);
    router.replace(`${projectBase.value}/editor/${encodeURIComponent(name)}`);
  } catch (e: any) {
    MessagePlugin.error(e.message || t('editor.loadFailed'));
  }
}

function canBeTable(rs: RuleSet): boolean {
  try {
    return !!decompileStepsToTable(rs.steps, rs.startStepId);
  } catch {
    return false;
  }
}

function handleRulesetChange(ruleset: RuleSet) {
  applyRulesetChange(ruleset);
}

function stripDecisionTableMetadata(ruleset: RuleSet): RuleSet {
  if (!ruleset.config.metadata?._table) return ruleset;

  const metadata = { ...ruleset.config.metadata };
  delete metadata._table;
  return {
    ...ruleset,
    config: {
      ...ruleset.config,
      metadata,
    },
  };
}

function applyRulesetChange(ruleset: RuleSet, actionOverride?: string) {
  const tab = projectStore.activeTab;
  if (!tab) return;

  const incomingRuleset =
    editorMode.value === 'flow' ? stripDecisionTableMetadata(ruleset) : ruleset;
  const { ruleset: normalizedRuleset, refsToCreate } = normalizeSubRuleReferences(
    tab.name,
    incomingRuleset
  );
  const action = actionOverride ?? buildHistoryAction(tab.ruleset, normalizedRuleset);
  updateRulesetState(tab.name, normalizedRuleset);
  scheduleEditHistoryEntry(tab.name, normalizedRuleset, action);

  for (const refName of refsToCreate) {
    void ensureProjectSubRuleAsset(refName);
  }
}

function makeUniqueProjectSubRuleName(baseName: string) {
  const base = sanitizeAssetName(baseName);
  const names = new Set(
    subRuleAssets.value.filter((asset) => asset.scope === 'project').map((asset) => asset.name)
  );
  if (!names.has(base)) return base;

  let suffix = 2;
  while (names.has(`${base}_${suffix}`)) {
    suffix += 1;
  }
  return `${base}_${suffix}`;
}

function hasAnySubRuleAsset(name: string) {
  return subRuleAssets.value.some((asset) => asset.name === name);
}

function handleExtractSubRule(payload: ExtractSubRulePayload) {
  const tab = projectStore.activeTab;
  if (!tab || tab.kind === 'sub_rule') return;

  const name = makeUniqueProjectSubRuleName(payload.suggestedName);
  extractSubRuleState.value = {
    parentTabName: tab.name,
    payload,
    name,
    displayName: payload.displayName,
    description: t('subRules.extractedDescription', { count: payload.selectedStepCount }),
  };
}

function retargetExtractedSubRuleStep(
  ruleset: RuleSet,
  subRuleStepId: string,
  name: string,
  displayName: string
): RuleSet {
  return {
    ...ruleset,
    steps: ruleset.steps.map((step) => {
      if (step.id !== subRuleStepId || step.type !== 'sub_rule') return step;
      const subRuleStep = step as SubRuleStep;
      return {
        ...subRuleStep,
        name: displayName || name,
        refName: name,
        assetRef: {
          scope: 'project' as const,
          name,
        },
      };
    }),
  };
}

async function confirmExtractSubRule() {
  const state = extractSubRuleState.value;
  if (!state || !auth.token) return;

  const name = sanitizeAssetName(state.name);
  if (!name) {
    MessagePlugin.warning(t('subRules.nameRequired'));
    return;
  }
  if (!subRuleAssetsLoaded.value) {
    await refreshSubRuleAssets();
  }
  if (hasAnySubRuleAsset(name)) {
    MessagePlugin.warning(t('subRules.nameExists', { name }));
    return;
  }

  extractingSubRule.value = true;
  try {
    const displayName = state.displayName.trim() || name;
    const description = state.description.trim();
    const draft: RuleSet = {
      ...cloneRuleset(state.payload.draft),
      config: {
        ...state.payload.draft.config,
        name,
        description,
      },
    };

    await subRuleApi.saveProject(auth.token, orgId.value, projectId.value, name, {
      name,
      display_name: displayName,
      description,
      draft,
      input_schema: [],
      output_schema: [],
      expected_seq: 0,
    });
    await refreshSubRuleAssets();

    if (projectStore.activeTabName !== state.parentTabName) {
      switchToTab(state.parentTabName);
    }

    const parentRuleset = retargetExtractedSubRuleStep(
      cloneRuleset(state.payload.parentRuleset),
      state.payload.subRuleStepId,
      name,
      displayName
    );
    applyRulesetChange(parentRuleset, t('historyPanel.actionExtractSubRule', { name }));

    await projectStore.openSubRule(name, 'project');
    const tabName = `§${name}`;
    subRuleParentTabs.set(tabName, state.parentTabName);
    tabModes.set(tabName, 'flow');
    router.replace(`${projectBase.value}/editor/${encodeURIComponent(tabName)}`);

    MessagePlugin.success(t('subRules.extractSuccess', { name }));
    extractSubRuleState.value = null;
  } catch (e: any) {
    MessagePlugin.error(e.message || t('subRules.saveFailed'));
  } finally {
    extractingSubRule.value = false;
  }
}

function normalizeSubRuleReferences(parentRulesetName: string, ruleset: RuleSet) {
  let changed = false;
  const refsToCreate: string[] = [];

  const steps = ruleset.steps.map((step) => {
    if (step.type !== 'sub_rule') return step;

    const subRuleStep = step as SubRuleStep;
    const generatedName =
      subRuleStep.refName.trim() || `${sanitizeAssetName(parentRulesetName)}_${subRuleStep.id}`;
    const assetRef: NonNullable<SubRuleStep['assetRef']> = {
      ...(subRuleStep.assetRef ?? { scope: 'project' as const }),
      scope: subRuleStep.assetRef?.scope ?? ('project' as const),
      name: subRuleStep.assetRef?.name?.trim() || generatedName,
    };

    const needsPatch =
      !subRuleStep.refName.trim() ||
      !subRuleStep.assetRef ||
      !subRuleStep.assetRef.name?.trim() ||
      subRuleStep.assetRef.name !== assetRef.name;

    if (needsPatch) {
      changed = true;
      refsToCreate.push(generatedName);
      return {
        ...subRuleStep,
        refName: generatedName,
        assetRef,
      };
    }

    return subRuleStep;
  });

  return {
    ruleset: changed ? { ...ruleset, steps } : ruleset,
    refsToCreate,
  };
}

function sanitizeAssetName(name: string) {
  return (
    name
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9_]+/g, '_')
      .replace(/^_+|_+$/g, '') || 'sub_rule'
  );
}

function createDefaultSubRuleDraft(name: string): RuleSet {
  const terminal = Step.terminal({
    id: 'return_result',
    name: t('subRules.defaultTerminalName'),
    code: 'OK',
    message: {
      type: 'literal',
      value: '',
      valueType: 'string',
    },
    output: [],
    position: { x: 160, y: 120 },
  });

  return {
    config: {
      name,
      version: '0.1.0',
      description: t('subRules.defaultDescription'),
      enableTrace: true,
    },
    startStepId: terminal.id,
    steps: [terminal],
    groups: [],
    metadata: {
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    },
  };
}

async function refreshSubRuleAssets() {
  if (!auth.token || !orgId.value || !projectId.value) return;
  try {
    subRuleAssets.value = await subRuleApi.listProject(
      auth.token,
      orgId.value,
      projectId.value,
      true
    );
    subRuleAssetsLoaded.value = true;
  } catch (e: any) {
    subRuleAssetsLoaded.value = false;
    MessagePlugin.warning(e.message || t('subRules.loadFailed'));
  }
}

function hasProjectSubRuleAsset(name: string) {
  return subRuleAssets.value.some((asset) => asset.scope === 'project' && asset.name === name);
}

async function ensureProjectSubRuleAsset(name: string) {
  if (!auth.token || !orgId.value || !projectId.value) return;
  const key = `${projectId.value}:${name}`;
  if (pendingSubRuleAssets.has(key)) return;

  if (!subRuleAssetsLoaded.value) {
    await refreshSubRuleAssets();
  }
  if (hasProjectSubRuleAsset(name)) return;

  pendingSubRuleAssets.add(key);
  try {
    await subRuleApi.saveProject(auth.token, orgId.value, projectId.value, name, {
      name,
      display_name: name,
      description: t('subRules.defaultDescription'),
      draft: createDefaultSubRuleDraft(name),
      input_schema: [],
      output_schema: [],
      expected_seq: 0,
    });
    await refreshSubRuleAssets();
  } catch (e: any) {
    MessagePlugin.warning(e.message || t('subRules.saveFailed'));
  } finally {
    pendingSubRuleAssets.delete(key);
  }
}

async function handleOpenSubRule(refName: string) {
  if (!refName) return;
  const parentTabName = projectStore.activeTab?.name ?? null;
  const scope =
    (projectStore.activeTab?.ruleset.steps as any[])?.find(
      (s: any) => s.type === 'sub_rule' && s.refName === refName
    )?.assetRef?.scope ?? 'project';
  if (scope === 'project') {
    await ensureProjectSubRuleAsset(refName);
  }
  try {
    await projectStore.openSubRule(refName, scope);
    const tabName = `§${refName}`;
    if (parentTabName && parentTabName !== tabName) {
      subRuleParentTabs.set(tabName, parentTabName);
    }
    tabModes.set(tabName, 'flow');
    router.replace(`${projectBase.value}/editor/${encodeURIComponent(tabName)}`);
  } catch (e: any) {
    MessagePlugin.error(e.message || t('subRules.loadFailed'));
  }
}

function returnToSubRuleParent() {
  const parentName = activeSubRuleParentName.value;
  if (!parentName) return;
  switchToTab(parentName);
  router.replace(`${projectBase.value}/editor/${encodeURIComponent(parentName)}`);
}

function handleVersionChange(event: Event) {
  const tab = projectStore.activeTab;
  if (!tab) return;

  const target = event.target as HTMLInputElement;
  const nextVersion = stripVersionSuffix(target.value);
  const nextRuleset: RuleSet = {
    ...tab.ruleset,
    config: {
      ...tab.ruleset.config,
      version: nextVersion,
    },
  };

  const action = buildHistoryAction(tab.ruleset, nextRuleset);
  updateRulesetState(tab.name, nextRuleset);
  scheduleEditHistoryEntry(tab.name, nextRuleset, action);
}

async function handleSave(name: string) {
  if (!canEdit.value) {
    MessagePlugin.warning(t('editor.noPermission'));
    return;
  }
  const tab = projectStore.openTabs.find((item) => item.name === name);
  if (!tab) return;

  const nextVersion = stripVersionSuffix(tab.ruleset.config.version);
  const meta = projectStore.draftMetas.find((item) => item.name === name) ?? null;
  const publishedVersion = stripVersionSuffix(meta?.published_version);
  if (!nextVersion) {
    MessagePlugin.warning(t('editor.versionRequired'));
    return;
  }
  if (publishedVersion && publishedVersion === nextVersion) {
    MessagePlugin.warning(t('editor.versionBumpRequired', { version: publishedVersion }));
    return;
  }
  saving.value = true;
  try {
    flushPendingEditHistory(name);
    const result = await projectStore.saveRuleset(name);
    if (result?.conflict) {
      const tab = projectStore.openTabs.find((item) => item.name === name);
      if (!tab) {
        MessagePlugin.error(t('editor.saveFailed'));
        return;
      }
      conflictState.value = {
        rulesetName: name,
        localDraft: cloneRuleset(tab.ruleset),
        serverDraft: cloneRuleset(normalizeRuleset(result.server_draft, name)),
        serverSeq: result.server_seq,
      };
      return;
    }
    const tab = projectStore.openTabs.find((item) => item.name === name);
    if (tab) {
      savedRulesetSnapshots.set(name, serializeRuleset(tab.ruleset));
      projectStore.setTabRuleset(name, cloneRuleset(tab.ruleset), false);
      if (tab.kind === 'sub_rule') {
        await refreshSubRuleAssets();
      }
      pushHistoryEntry(name, tab.ruleset, t('historyPanel.actionSaveCheckpoint'), 'save');
      await flushHistoryQueue(name);
    }
    MessagePlugin.success(t('editor.saveSuccess'));
  } catch (e: any) {
    MessagePlugin.error(e.message || t('editor.saveFailed'));
  } finally {
    saving.value = false;
  }
}

async function resolveConflictUseServer() {
  const conflict = conflictState.value;
  if (!conflict) return;
  const tab = projectStore.openTabs.find((item) => item.name === conflict.rulesetName);
  if (!tab) {
    conflictState.value = null;
    return;
  }

  tab.draft_seq = conflict.serverSeq;
  savedRulesetSnapshots.set(conflict.rulesetName, serializeRuleset(conflict.serverDraft));
  projectStore.setTabRuleset(conflict.rulesetName, cloneRuleset(conflict.serverDraft), false);
  syncDecisionTableFromRuleset(conflict.rulesetName, conflict.serverDraft);
  conflictState.value = null;
  MessagePlugin.success(t('conflict.useServerSuccess'));
}

async function resolveConflictUseLocal() {
  const conflict = conflictState.value;
  if (!conflict) return;
  const tab = projectStore.openTabs.find((item) => item.name === conflict.rulesetName);
  if (!tab) {
    conflictState.value = null;
    return;
  }

  tab.draft_seq = conflict.serverSeq;
  projectStore.setTabRuleset(conflict.rulesetName, cloneRuleset(conflict.localDraft), true);
  conflictState.value = null;
  await handleSave(conflict.rulesetName);
}

function openReleaseCenter() {
  if (!projectStore.activeTab) return;
  if (projectStore.activeTab.kind === 'sub_rule') return;
  router.push({
    name: 'project-release-request-create',
    params: {
      orgId: route.params.orgId,
      projectId: route.params.projectId,
    },
    query: { ruleset: projectStore.activeTab.name },
  });
}

function handleCloseTab(name: string) {
  const tab = projectStore.openTabs.find((t) => t.name === name);
  if (tab?.dirty) {
    const dlg = DialogPlugin.confirm({
      header: t('editor.closeConfirm'),
      body: t('editor.closeConfirmBody', { name }),
      confirmBtn: { content: t('editor.closeConfirmBtn'), theme: 'danger' },
      cancelBtn: t('common.cancel'),
      onConfirm: async () => {
        projectStore.closeTab(name);
        await disposeTabHistory(name);
        dlg.hide();
        if (!projectStore.activeTabName) {
          router.replace(`${projectBase.value}/editor`);
        }
      },
    });
  } else {
    projectStore.closeTab(name);
    void disposeTabHistory(name);
    if (!projectStore.activeTabName) {
      router.replace(`${projectBase.value}/editor`);
    }
  }
}

async function handleCreateRuleset() {
  if (!newName.value.trim()) {
    MessagePlugin.warning(t('editor.nameRequired'));
    return;
  }
  creating.value = true;
  try {
    let rs: RuleSet;
    const name = newName.value.trim();

    if (newType.value === 'table') {
      const doc = createEmptyTableDocument(name);
      rs = documentToRuleSet(doc);
    } else {
      // Flow: Decision → Terminal
      const decisionId = generateId();
      const terminalId = generateId();
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
      };
    }

    await projectStore.createRuleset(rs);
    showCreate.value = false;
    newName.value = '';
    MessagePlugin.success(t('editor.createSuccess'));
    await openRuleset(name);
    pushHistoryEntry(name, rs, t('historyPanel.actionCreateRuleset'), 'create');
    showHistoryPanel.value = true;
  } catch (e: any) {
    MessagePlugin.error(e.message || t('editor.createFailed'));
  } finally {
    creating.value = false;
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
        await projectStore.deleteRuleset(name);
        await disposeTabHistory(name);
        dlg.hide();
        MessagePlugin.success(t('editor.deleteSuccess'));
        if (!projectStore.activeTabName && projectStore.rulesets.length > 0) {
          await openRuleset(projectStore.rulesets[0].name);
        }
      } catch (e: any) {
        MessagePlugin.error(e.message);
      }
    },
  });
}

function setEditorMode(mode: 'form' | 'flow' | 'table') {
  const tab = projectStore.activeTab;
  if (!tab) return;
  if (mode === 'table' && !canBeTable(tab.ruleset)) {
    MessagePlugin.warning(t('editor.tableUnsupported'));
    return;
  }
  editorMode.value = mode;
  tabModes.set(tab.name, mode);
}

onUnmounted(() => {
  for (const name of Array.from(editHistoryTimers.keys())) {
    flushPendingEditHistory(name);
  }
  for (const name of Array.from(historyFlushTimers.keys())) {
    void flushHistoryQueue(name);
  }
});
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
          <t-icon name="file-code" size="14px" class="ruleset-item__icon" />
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
        <div class="editor-menu" @mouseenter="hoverMenu('file')">
          <button
            class="editor-menu__trigger"
            :class="{ 'is-open': openMenu === 'file' }"
            @click="toggleMenu('file')"
          >
            {{ t('menuBar.file') }}
          </button>
          <div v-if="openMenu === 'file'" class="editor-menu__dropdown">
            <button
              class="editor-menu__item"
              :disabled="!canEdit"
              @click="runMenuAction(() => (showCreate = true))"
            >
              <span>{{ t('menuBar.newRuleset') }}</span>
            </button>
            <button
              class="editor-menu__item"
              :disabled="!canEdit || !projectStore.activeTab"
              @click="
                runMenuAction(
                  () => projectStore.activeTab && handleSave(projectStore.activeTab.name)
                )
              "
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
          <button
            class="editor-menu__trigger"
            :class="{ 'is-open': openMenu === 'edit' }"
            @click="toggleMenu('edit')"
          >
            {{ t('menuBar.edit') }}
          </button>
          <div v-if="openMenu === 'edit'" class="editor-menu__dropdown">
            <button
              class="editor-menu__item"
              :disabled="!canUndoHistory"
              @click="runMenuAction(undoHistory)"
            >
              <span>{{ t('menuBar.undo') }}</span>
              <span class="editor-menu__shortcut">Ctrl+Z</span>
            </button>
            <button
              class="editor-menu__item"
              :disabled="!canRedoHistory"
              @click="runMenuAction(redoHistory)"
            >
              <span>{{ t('menuBar.redo') }}</span>
              <span class="editor-menu__shortcut">Ctrl+Shift+Z</span>
            </button>
          </div>
        </div>

        <div class="editor-menu" @mouseenter="hoverMenu('select')">
          <button
            class="editor-menu__trigger"
            :class="{ 'is-open': openMenu === 'select' }"
            @click="toggleMenu('select')"
          >
            {{ t('menuBar.select') }}
          </button>
          <div v-if="openMenu === 'select'" class="editor-menu__dropdown">
            <button class="editor-menu__item is-disabled" disabled>
              <span>{{ t('menuBar.selectionSoon') }}</span>
            </button>
          </div>
        </div>

        <div class="editor-menu" @mouseenter="hoverMenu('view')">
          <button
            class="editor-menu__trigger"
            :class="{ 'is-open': openMenu === 'view' }"
            @click="toggleMenu('view')"
          >
            {{ t('menuBar.view') }}
          </button>
          <div v-if="openMenu === 'view'" class="editor-menu__dropdown">
            <button
              class="editor-menu__item"
              :disabled="editorMode === 'form'"
              @click="runMenuAction(() => setEditorMode('form'))"
            >
              <span>{{ t('editor.formMode') }}</span>
            </button>
            <button
              class="editor-menu__item"
              :disabled="editorMode === 'flow'"
              @click="runMenuAction(() => setEditorMode('flow'))"
            >
              <span>{{ t('editor.flowMode') }}</span>
            </button>
            <button
              class="editor-menu__item"
              :disabled="editorMode === 'table'"
              @click="runMenuAction(() => setEditorMode('table'))"
            >
              <span>{{ t('editor.tableMode') }}</span>
            </button>
          </div>
        </div>

        <div class="editor-menu" @mouseenter="hoverMenu('window')">
          <button
            class="editor-menu__trigger"
            :class="{ 'is-open': openMenu === 'window' }"
            @click="toggleMenu('window')"
          >
            {{ t('menuBar.window') }}
          </button>
          <div v-if="openMenu === 'window'" class="editor-menu__dropdown">
            <button
              class="editor-menu__item"
              @click="
                runMenuAction(() => {
                  showHistoryPanel = !showHistoryPanel;
                  if (showHistoryPanel) historyPanelCollapsed = false;
                })
              "
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
          <t-icon
            :name="tab.kind === 'sub_rule' ? 'git-branch' : 'file-code'"
            size="13px"
            class="tab-icon"
          />
          <span class="tab-name">{{
            tab.name.startsWith('§') ? tab.name.slice(1) : tab.name
          }}</span>
          <t-tag
            v-if="tab.kind === 'sub_rule'"
            size="small"
            variant="light"
            theme="warning"
            class="tab-kind-badge"
            >sub</t-tag
          >
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
          <div class="toolbar-version">
            <label>{{ t('common.version') }}</label>
            <input
              :value="activeDraftVersion"
              :disabled="!canEdit"
              placeholder="1.0.0"
              class="ordo-input-base toolbar-version__input"
              @input="handleVersionChange"
            />
            <t-tag size="small" theme="primary" variant="light">
              v{{ activeVersionDisplay }}
            </t-tag>
            <t-tag v-if="activePublishedVersion" size="small" variant="light">
              {{ t('editor.publishedVersionTag', { version: activePublishedVersion }) }}
            </t-tag>
          </div>
          <div v-if="requiresVersionBump" class="toolbar-version__warning">
            {{ t('editor.versionBumpRequired', { version: activePublishedVersion }) }}
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
            :disabled="projectStore.activeTab.kind === 'sub_rule'"
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
          :style="
            showExecution
              ? { flex: 'none', height: `calc(100% - ${executionHeight}px - 2px)` }
              : showTests
                ? { flex: 'none', height: `calc(100% - ${testsHeight}px - 2px)` }
                : {}
          "
        >
          <template v-if="projectStore.activeTab">
            <div class="editor-view-shell">
              <div v-if="canEdit && activeSubRuleSuggestions.length > 0" class="sub-rule-advisor">
                <div class="sub-rule-advisor__intro">
                  <div class="sub-rule-advisor__icon">
                    <t-icon name="git-branch" size="16px" />
                  </div>
                  <div>
                    <div class="sub-rule-advisor__title">{{ t('subRules.suggestionsTitle') }}</div>
                    <div class="sub-rule-advisor__desc">{{ t('subRules.suggestionsDesc') }}</div>
                  </div>
                </div>
                <div class="sub-rule-advisor__cards">
                  <button
                    v-for="suggestion in activeSubRuleSuggestions"
                    :key="suggestion.id"
                    type="button"
                    class="sub-rule-advisor__card"
                    @click="requestSuggestedSubRuleExtraction(suggestion)"
                  >
                    <span class="sub-rule-advisor__card-title">{{ suggestion.title }}</span>
                    <span class="sub-rule-advisor__card-desc">{{ suggestion.description }}</span>
                    <span class="sub-rule-advisor__meta">
                      <span>{{
                        t('subRules.suggestionSteps', { count: suggestion.stepCount })
                      }}</span>
                      <span>{{ suggestion.entryName }}</span>
                    </span>
                  </button>
                </div>
              </div>
              <div v-if="activeSubRuleName" class="sub-rule-focus-strip">
                <div class="sub-rule-focus-strip__main">
                  <t-icon name="git-branch" size="15px" />
                  <span class="sub-rule-focus-strip__title">
                    {{ t('subRules.focusTitle', { name: activeSubRuleName }) }}
                  </span>
                  <span class="sub-rule-focus-strip__desc">{{ t('subRules.focusDesc') }}</span>
                </div>
                <button
                  v-if="activeSubRuleParentName"
                  class="sub-rule-focus-strip__back"
                  @click="returnToSubRuleParent"
                >
                  <t-icon name="rollback" size="14px" />
                  {{ t('subRules.returnParent') }}
                </button>
              </div>
              <!-- Form mode -->
              <OrdoFormEditor
                v-if="editorMode === 'form'"
                :model-value="projectStore.activeTab.ruleset"
                :disabled="!canEdit"
                :managed-sub-rules="subRuleAssetOptions"
                :input-schema="
                  catalogStore.schemaFields.length ? catalogStore.schemaFields : undefined
                "
                @update:model-value="handleRulesetChange"
                @open-sub-rule="handleOpenSubRule"
              />
              <!-- Flow mode -->
              <OrdoFlowEditor
                v-else-if="editorMode === 'flow'"
                :model-value="projectStore.activeTab.ruleset"
                :disabled="!canEdit"
                :managed-sub-rules="subRuleAssetOptions"
                :execution-trace="executionTrace"
                :trace-mode="flowTraceMode"
                :extract-sub-rule-request="extractSubRuleRequest"
                @update:model-value="handleRulesetChange"
                @open-sub-rule="handleOpenSubRule"
                @extract-sub-rule="handleExtractSubRule"
                @extract-sub-rule-invalid="handleExtractSubRuleInvalid"
              />
              <!-- Decision table mode -->
              <OrdoDecisionTable
                v-else-if="editorMode === 'table' && activeDecisionTable"
                :model-value="activeDecisionTable"
                :disabled="!canEdit"
                @update:model-value="handleTableChange"
                @show-as-flow="handleShowAsFlow"
              />
            </div>
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
          @show-in-flow="handleShowInFlow"
          @open-sub-rule-trace="handleOpenSubRuleTrace"
        />
      </div>

      <div
        v-if="projectStore.activeTab && showHistoryPanel"
        class="history-panel-wrap"
        :style="{
          bottom: (showExecution ? executionHeight : showTests ? testsHeight : 0) + 20 + 'px',
        }"
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

    <t-dialog
      :visible="!!extractSubRuleState"
      :header="t('subRules.extractTitle')"
      :confirm-btn="{ content: t('subRules.extractConfirm'), loading: extractingSubRule }"
      width="480px"
      @confirm="confirmExtractSubRule"
      @close="extractSubRuleState = null"
    >
      <t-form v-if="extractSubRuleState" label-align="top">
        <p class="extract-sub-rule-dialog__desc">
          {{
            t('subRules.extractDesc', {
              count: extractSubRuleState.payload.selectedStepCount,
            })
          }}
        </p>
        <t-form-item :label="t('subRules.name')" required>
          <t-input
            v-model="extractSubRuleState.name"
            :placeholder="t('subRules.namePlaceholder')"
          />
        </t-form-item>
        <t-form-item :label="t('subRules.displayName')">
          <t-input
            v-model="extractSubRuleState.displayName"
            :placeholder="t('subRules.displayNamePlaceholder')"
          />
        </t-form-item>
        <t-form-item :label="t('subRules.description')">
          <t-textarea v-model="extractSubRuleState.description" :autosize="{ minRows: 3 }" />
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

.toolbar-version {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.toolbar-version label {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--ordo-text-tertiary);
  white-space: nowrap;
}

.toolbar-version__input {
  width: 96px;
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-app);
  color: var(--ordo-text-primary);
  outline: none;
}

.toolbar-version__input:focus {
  border-color: var(--ordo-accent);
}

.toolbar-version__warning {
  max-width: 240px;
  font-size: 12px;
  color: var(--ordo-warning);
  line-height: 1.2;
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
  transition:
    background 0.1s,
    color 0.1s;
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
  transition:
    background 0.1s,
    color 0.1s;
}

.toolbar-btn:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.toolbar-btn:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.toolbar-btn:disabled:hover {
  background: transparent;
  color: var(--ordo-text-secondary);
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

.editor-view-shell {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.editor-view-shell > :not(.sub-rule-focus-strip):not(.sub-rule-advisor) {
  flex: 1;
  min-height: 0;
}

.sub-rule-advisor {
  flex: none;
  display: flex;
  align-items: stretch;
  gap: 12px;
  padding: 10px 14px;
  border-bottom: 1px solid rgba(93, 126, 99, 0.22);
  background: radial-gradient(circle at 18px 0, rgba(74, 138, 89, 0.18), transparent 32%),
    linear-gradient(90deg, rgba(37, 62, 45, 0.44), rgba(31, 37, 43, 0.1)), var(--ordo-bg-panel);
}

.sub-rule-advisor__intro {
  width: 280px;
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.sub-rule-advisor__icon {
  width: 32px;
  height: 32px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #9bd39f;
  background: rgba(72, 126, 84, 0.2);
  border: 1px solid rgba(126, 191, 132, 0.24);
}

.sub-rule-advisor__title {
  color: var(--ordo-text-primary);
  font-size: 13px;
  font-weight: 800;
  letter-spacing: 0.01em;
}

.sub-rule-advisor__desc {
  margin-top: 3px;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
  line-height: 1.35;
}

.sub-rule-advisor__cards {
  min-width: 0;
  flex: 1;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
}

.sub-rule-advisor__card {
  min-width: 0;
  border: 1px solid rgba(126, 191, 132, 0.18);
  border-radius: 12px;
  background: rgba(18, 27, 23, 0.44);
  color: var(--ordo-text-primary);
  cursor: pointer;
  text-align: left;
  padding: 9px 11px;
  display: flex;
  flex-direction: column;
  gap: 5px;
  transition:
    border-color 0.12s ease,
    background 0.12s ease,
    transform 0.12s ease;
}

.sub-rule-advisor__card:hover {
  border-color: rgba(149, 216, 154, 0.48);
  background: rgba(35, 58, 42, 0.62);
  transform: translateY(-1px);
}

.sub-rule-advisor__card-title,
.sub-rule-advisor__card-desc {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sub-rule-advisor__card-title {
  font-size: 12px;
  font-weight: 800;
}

.sub-rule-advisor__card-desc {
  color: var(--ordo-text-tertiary);
  font-size: 11px;
}

.sub-rule-advisor__meta {
  display: flex;
  align-items: center;
  gap: 6px;
  color: #9bd39f;
  font-size: 11px;
}

.sub-rule-advisor__meta span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sub-rule-focus-strip {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--ordo-border-color);
  background: linear-gradient(90deg, rgba(91, 112, 138, 0.16), rgba(91, 112, 138, 0.04)),
    var(--ordo-bg-panel);
}

.sub-rule-focus-strip__main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.sub-rule-focus-strip__title {
  color: var(--ordo-text-primary);
  font-size: 13px;
  font-weight: 700;
}

.sub-rule-focus-strip__desc {
  color: var(--ordo-text-tertiary);
  font-size: 12px;
}

.sub-rule-focus-strip__back {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 999px;
  background: var(--ordo-bg-item);
  color: var(--ordo-text-secondary);
  padding: 5px 10px;
  font-size: 12px;
  cursor: pointer;
}

.sub-rule-focus-strip__back:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.extract-sub-rule-dialog__desc {
  margin: 0 0 16px;
  color: var(--ordo-text-secondary);
  font-size: 13px;
  line-height: 1.6;
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
