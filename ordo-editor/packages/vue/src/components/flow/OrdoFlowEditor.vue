<script setup lang="ts">
/**
 * OrdoFlowEditor - Flow-based ruleset editor
 * 流程图模式规则集编辑器
 */
import { ref, computed, inject, watch, onMounted, markRaw, provide, nextTick } from 'vue';
import { VueFlow, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { MiniMap } from '@vue-flow/minimap';
import type { RuleSet, Step, SubRuleGraph, SubRuleStep } from '@ordo-engine/editor-core';
import { Step as StepFactory, generateId } from '@ordo-engine/editor-core';

import {
  DecisionNode,
  ActionNode,
  TerminalNode,
  SubRuleNode,
  GroupNode,
  type StepTraceInfo,
} from './nodes';
import { OrdoEdge } from './edges';
import OrdoIcon from '../icons/OrdoIcon.vue';
import OrdoFlowToolbar from './OrdoFlowToolbar.vue';
import OrdoFlowPropertyPanel from './OrdoFlowPropertyPanel.vue';
import {
  rulesetToFlow,
  flowToRuleset,
  createNodeFromStep,
  createEdge,
  type EdgeRenderStyle,
  type FlowEdge,
  type FlowNode,
  createGroupNode,
} from './utils/converter';
import {
  applyDagreLayout,
  applyGroupBasedLayout,
  needsAutoLayout,
  getSuggestedPosition,
  type LayoutDirection,
} from './utils/layout';
import { useI18n, LOCALE_KEY, type Lang } from '../../locale';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';
import type { SubRuleAssetOption } from '../step/subRuleAssets';
type NodeCreationType = 'decision' | 'action' | 'terminal' | 'sub_rule';

export interface ExecutionTraceStep {
  id: string;
  name: string;
  duration_us: number;
  result?: string | null;
  next_step?: string | null;
  is_terminal?: boolean;
  input_snapshot?: Record<string, any> | null;
  variables_snapshot?: Record<string, any> | null;
  sub_rule_ref?: string | null;
  sub_rule_input?: Record<string, any> | null;
  sub_rule_outputs?: Array<{
    parent_var: string;
    child_var: string;
    value?: any;
    missing?: boolean;
  }>;
  sub_rule_frames?: ExecutionTraceStep[];
}

/** Execution trace data for overlay */
export interface ExecutionTraceData {
  /** Execution path as array of step IDs */
  path: string[];
  /** Step trace details */
  steps: ExecutionTraceStep[];
  /** Final result code */
  resultCode: string;
  /** Final result message */
  resultMessage: string;
  /** Output data */
  output?: Record<string, any>;
}

export interface ExtractSubRulePayload {
  suggestedName: string;
  displayName: string;
  subRuleStepId: string;
  selectedStepCount: number;
  draft: RuleSet;
  parentRuleset: RuleSet;
}

export interface ExtractSubRuleRequest {
  id: number;
  stepIds: string[];
}

export interface Props {
  /** RuleSet data */
  modelValue: RuleSet;
  /** Field suggestions for expressions */
  suggestions?: FieldSuggestion[];
  /** Managed project/org sub-rule assets */
  managedSubRules?: SubRuleAssetOption[];
  /** Whether the editor is disabled */
  disabled?: boolean;
  /** Locale */
  locale?: Lang;
  /** Execution trace to display as overlay */
  executionTrace?: ExecutionTraceData | null;
  /** Executable ruleset used for trace-only expansion of hydrated sub-rules */
  executionRuleset?: RuleSet | null;
  /** Lock the canvas into execution path mode without selection/edit interactions */
  traceMode?: boolean;
  /** Programmatic extraction request from a host shell */
  extractSubRuleRequest?: ExtractSubRuleRequest | null;
}

const props = withDefaults(defineProps<Props>(), {
  suggestions: () => [],
  managedSubRules: () => [],
  disabled: false,
  executionTrace: null,
  executionRuleset: null,
  traceMode: false,
  extractSubRuleRequest: null,
});

const emit = defineEmits<{
  'update:modelValue': [value: RuleSet];
  change: [value: RuleSet];
  'open-sub-rule': [name: string];
  'extract-sub-rule': [payload: ExtractSubRulePayload];
  'extract-sub-rule-invalid': [reason: string];
}>();

const FLOW_EDGE_STYLE_KEY = '_flowEdgeStyle';
const FLOW_LAYOUT_DIRECTION_KEY = '_flowLayoutDirection';
const FLOW_NODE_DRAG_TYPE = 'application/x-ordo-flow-node';
const DEFAULT_NODE_DROP_OFFSET = { x: 90, y: 40 };
const TRACE_EXPANSION_PREFIX = '__ordo_trace_subrule__';
const TRACE_EXPANSION_GAP_X = 360;
const TRACE_EXPANSION_GAP_Y = 56;

function getStoredEdgeStyle(ruleset: RuleSet): EdgeRenderStyle {
  const stored = ruleset.config.metadata?.[FLOW_EDGE_STYLE_KEY];
  return stored === 'step' ? 'step' : 'bezier';
}

function getStoredLayoutDirection(ruleset: RuleSet): LayoutDirection {
  const stored = ruleset.config.metadata?.[FLOW_LAYOUT_DIRECTION_KEY];
  return stored === 'TB' || stored === 'RL' || stored === 'BT' ? stored : 'LR';
}

// Inherit locale from parent provider unless the caller explicitly overrides it.
const inheritedLocale = inject(LOCALE_KEY, ref<Lang>('en'));
const currentLocale = computed<Lang>(() => props.locale ?? inheritedLocale.value);
provide(LOCALE_KEY, currentLocale);

const { t } = useI18n();

// Vue Flow instance
const {
  onNodesChange,
  onEdgesChange,
  onConnect,
  onNodeDragStart,
  onNodeDrag,
  onNodeDragStop,
  onEdgeUpdateStart,
  onEdgeUpdateEnd,
  fitView,
  getIntersectingNodes,
  updateEdge,
  removeEdges,
  addEdges,
  screenToFlowCoordinate,
} = useVueFlow();

// Edge update state
const edgeUpdating = ref(false);

// Track group drag state for moving child nodes together
const groupDragState = ref<{
  groupId: string;
  startPosition: { x: number; y: number };
  childNodeIds: string[];
} | null>(null);

// Custom node types (using any to avoid Vue Flow type conflicts)
const nodeTypes: Record<string, any> = {
  decision: markRaw(DecisionNode),
  action: markRaw(ActionNode),
  terminal: markRaw(TerminalNode),
  sub_rule: markRaw(SubRuleNode),
  group: markRaw(GroupNode),
};

// Custom edge types
const edgeTypes: Record<string, any> = {
  ordo: markRaw(OrdoEdge),
};

// State (using any[] to avoid Vue Flow type conflicts)
const nodes = ref<any[]>([]);
const edges = ref<any[]>([]);
const groupNodes = ref<any[]>([]);
const selectedNodeId = ref<string | null>(null);
const selectedNodeIds = ref<string[]>([]); // For multi-select
const selectedEdgeId = ref<string | null>(null); // For edge context menu
const edgeStyle = ref<EdgeRenderStyle>(getStoredEdgeStyle(props.modelValue));
const layoutDirection = ref<LayoutDirection>(getStoredLayoutDirection(props.modelValue));

// Context menu state
const showContextMenu = ref(false);
const contextMenuPosition = ref({ x: 0, y: 0 });
const flowCanvasContainer = ref<HTMLElement | null>(null);
const draggedNodeType = ref<NodeCreationType | null>(null);
const nodeDragPreview = ref<{
  type: NodeCreationType;
  x: number;
  y: number;
} | null>(null);

// Flag to prevent re-initialization during internal updates
const isInternalUpdate = ref(false);

// Highlighted path state (for path tracing)
const highlightedNodeIds = ref<Set<string>>(new Set());
const highlightedEdgeIds = ref<Set<string>>(new Set());

// Execution trace overlay state
const showExecutionOverlay = ref(false);
const executionAnnotations = ref<Map<string, StepTraceInfo>>(new Map());
let pendingTraceApplyToken = 0;
let lastHandledExtractSubRuleRequestId: number | null = null;

const isCanvasReadOnly = computed(() => props.disabled || props.traceMode);

// Selected node data
const selectedNode = computed(() => {
  if (!selectedNodeId.value) return null;
  return nodes.value.find((n) => n.id === selectedNodeId.value) || null;
});

// Check if selected node is a step (not a group)
const selectedStepNode = computed(() => {
  const node = selectedNode.value;
  if (!node || node.type === 'group') return null;
  return node;
});

// Check if selected node is a group
const selectedGroupNode = computed(() => {
  const node = selectedNode.value;
  if (!node || node.type !== 'group') return null;
  return node;
});

interface ExtractSubRuleEligibility {
  valid: boolean;
  reason?: string;
  entryId?: string;
  exitTargetId?: string;
  selectedNodes: FlowNode[];
  internalEdges: FlowEdge[];
  externalIncomingEdges: FlowEdge[];
  externalOutgoingEdges: FlowEdge[];
}

const extractSubRuleEligibility = computed(() => getExtractSubRuleEligibility());

const nodeDragPreviewLabel = computed(() => {
  if (!nodeDragPreview.value) return '';

  switch (nodeDragPreview.value.type) {
    case 'decision':
      return t('step.decision');
    case 'action':
      return t('step.action');
    case 'terminal':
      return t('step.terminal');
    case 'sub_rule':
      return t('step.subRule');
  }
});

const nodeDragPreviewTypeLabel = computed(() => {
  if (!nodeDragPreview.value) return '';

  switch (nodeDragPreview.value.type) {
    case 'decision':
      return t('step.typeDecision');
    case 'action':
      return t('step.typeAction');
    case 'terminal':
      return t('step.typeTerminal');
    case 'sub_rule':
      return t('step.typeSubRule');
  }
});

// Initialize from ruleset
function initFromRuleset(forceLayout = false) {
  edgeStyle.value = getStoredEdgeStyle(props.modelValue);
  layoutDirection.value = getStoredLayoutDirection(props.modelValue);

  const flowData = rulesetToFlow(props.modelValue, edgeStyle.value);

  // Add zIndex to group nodes to keep them at bottom
  // NOTE: Do NOT set draggable here - let it inherit from VueFlow's nodes-draggable prop
  // This ensures groups are locked when the lock button is clicked
  const groupsWithZIndex = flowData.groups.map((g) => ({
    ...g,
    zIndex: -1000, // Keep groups at the bottom
    selectable: true,
    // draggable is NOT set here - inherited from VueFlow's nodes-draggable
    connectable: false, // Groups don't have handles
  }));

  groupNodes.value = groupsWithZIndex;
  nodes.value = [...groupsWithZIndex, ...flowData.nodes];
  edges.value = flowData.edges;

  // Auto layout if needed or forced
  // Always layout if nodes have no positions or are overlapping
  if (forceLayout || needsAutoLayout(flowData.nodes)) {
    // Use setTimeout to ensure Vue has rendered the nodes first
    setTimeout(() => {
      autoLayout();
    }, 10);
  }

  if (props.executionTrace) {
    void scheduleTraceApply(props.executionTrace);
  }
}

function buildFlowConfig() {
  return {
    ...props.modelValue.config,
    metadata: {
      ...(props.modelValue.config.metadata ?? {}),
      [FLOW_EDGE_STYLE_KEY]: edgeStyle.value,
      [FLOW_LAYOUT_DIRECTION_KEY]: layoutDirection.value,
    },
  };
}

function isTraceOnlyElement(element: any): boolean {
  return element?.data?.traceOnly === true;
}

// Sync back to ruleset
function syncToRuleset() {
  if (isCanvasReadOnly.value) return;

  // Set flag to prevent watch from re-initializing
  isInternalUpdate.value = true;

  // Filter out group nodes for step processing
  const stepNodes = nodes.value.filter((n) => n.type !== 'group' && !isTraceOnlyElement(n));
  const currentGroupNodes = nodes.value.filter((n) => n.type === 'group' && !isTraceOnlyElement(n));
  const persistedEdges = edges.value.filter((e) => !isTraceOnlyElement(e));

  const newRuleset = flowToRuleset(
    stepNodes,
    persistedEdges,
    buildFlowConfig(),
    props.modelValue.startStepId,
    currentGroupNodes,
    props.modelValue.subRules
  );
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);

  // Reset flag after Vue's next tick
  setTimeout(() => {
    isInternalUpdate.value = false;
  }, 0);
}

// Watch for external changes (skip if internal update)
watch(
  () => props.modelValue,
  () => {
    if (isInternalUpdate.value) return;
    initFromRuleset();
  },
  { deep: true }
);

// Watch for execution trace changes
watch(
  () => props.executionTrace,
  (trace) => {
    if (trace) {
      void scheduleTraceApply(trace);
    } else {
      clearExecutionTrace();
    }
  },
  { immediate: true }
);

watch(
  () => props.extractSubRuleRequest?.id,
  () => {
    if (props.extractSubRuleRequest) {
      void applyExtractSubRuleRequest(props.extractSubRuleRequest);
    }
  }
);

async function scheduleTraceApply(trace: ExecutionTraceData) {
  const token = ++pendingTraceApplyToken;
  await nextTick();
  setTimeout(() => {
    if (token !== pendingTraceApplyToken) return;
    applyExecutionTrace(trace);
  }, 120);
}

// ============================================
// Execution trace overlay functionality
// ============================================

function stripTransientHighlightClasses(className: unknown): string {
  return String(className ?? '')
    .split(/\s+/)
    .filter(
      (name) =>
        name &&
        name !== 'execution-highlighted' &&
        name !== 'execution-dimmed' &&
        name !== 'path-highlighted' &&
        name !== 'path-dimmed'
    )
    .join(' ');
}

function withTransientClass(className: unknown, nextClass: string | null): string {
  const base = stripTransientHighlightClasses(className);
  return [base, nextClass].filter(Boolean).join(' ');
}

function removeTraceExpansion() {
  nodes.value = nodes.value.filter((node) => !isTraceOnlyElement(node));
  edges.value = edges.value.filter((edge) => !isTraceOnlyElement(edge));
}

function safeTraceId(value: string): string {
  return (
    value
      .trim()
      .replace(/[^a-zA-Z0-9_]+/g, '_')
      .replace(/^_+|_+$/g, '') || 'value'
  );
}

function runtimeRefSuffixForStep(stepId: string): string {
  return `__${safeTraceId(stepId)}_terminal_return`;
}

function stripRuntimeSubRuleRef(refName: string | null | undefined, stepId: string): string | null {
  if (!refName) return null;
  const suffix = runtimeRefSuffixForStep(stepId);
  return refName.endsWith(suffix) ? refName.slice(0, -suffix.length) : refName;
}

function traceChildNodeId(parentDisplayId: string, childStepId: string): string {
  return `${TRACE_EXPANSION_PREFIX}${parentDisplayId}__${childStepId}`;
}

function traceSubRuleGraphs(): Record<string, SubRuleGraph> {
  return {
    ...(props.executionRuleset?.subRules ?? {}),
    ...(props.modelValue.subRules ?? {}),
  };
}

function resolveTraceSubRuleGraph(
  parentNode: any,
  traceStep: ExecutionTraceStep
): { name: string; graph: SubRuleGraph } | null {
  const graphs = traceSubRuleGraphs();
  const step = parentNode?.data?.step as SubRuleStep | undefined;
  const candidates = [
    stripRuntimeSubRuleRef(step?.refName, traceStep.id),
    stripRuntimeSubRuleRef(traceStep.sub_rule_ref, traceStep.id),
    step?.refName,
    traceStep.sub_rule_ref ?? undefined,
  ].filter((name): name is string => !!name);

  for (const name of candidates) {
    const graph = graphs[name];
    if (graph) return { name, graph };
  }

  return null;
}

function buildSubRuleTraceExpansion(
  trace: ExecutionTraceData,
  annotations: Map<string, StepTraceInfo>,
  nodeIds: Set<string>,
  edgeIds: Set<string>
): { nodes: any[]; edges: any[] } {
  const expansionNodes: any[] = [];
  const expansionEdges: any[] = [];
  const nodeLookup = new Map<string, any>(nodes.value.map((node) => [node.id, node]));

  function addStepAnnotations(
    steps: ExecutionTraceStep[],
    resolveDisplayId: (stepId: string) => string,
    orderPrefix: string,
    resultCode?: string
  ) {
    steps.forEach((step, index) => {
      const displayId = resolveDisplayId(step.id);
      const isEntry = index === 0;
      const isTerminal = step.is_terminal || index === steps.length - 1;

      annotations.set(displayId, {
        stepId: displayId,
        stepName: step.name,
        durationUs: step.duration_us,
        order: orderPrefix ? `${orderPrefix}.${index + 1}` : index + 1,
        isEntry,
        isTerminal,
        resultCode: isTerminal ? step.result ?? resultCode : undefined,
      });
      nodeIds.add(displayId);
    });

    const pathIds = steps.map((step) => resolveDisplayId(step.id));
    for (let i = 0; i < pathIds.length - 1; i++) {
      const edge = [...edges.value, ...expansionEdges].find(
        (item) => item.source === pathIds[i] && item.target === pathIds[i + 1]
      );
      if (edge) edgeIds.add(edge.id);
    }
  }

  function expandNestedSteps(
    steps: ExecutionTraceStep[],
    resolveDisplayId: (stepId: string) => string,
    orderPrefix: string
  ) {
    steps.forEach((step, index) => {
      if (!step.sub_rule_frames?.length) return;

      const parentDisplayId = resolveDisplayId(step.id);
      const parentNode = nodeLookup.get(parentDisplayId);
      const resolved = resolveTraceSubRuleGraph(parentNode, step);
      if (!parentNode || !resolved) return;

      const { graph, name } = resolved;
      const graphRuleset: RuleSet = {
        config: {
          ...props.modelValue.config,
          name,
        },
        startStepId: graph.entryStep,
        steps: graph.steps,
        subRules: traceSubRuleGraphs(),
        groups: [],
      };
      const flowData = rulesetToFlow(graphRuleset, edgeStyle.value);
      const childNodes = needsAutoLayout(flowData.nodes)
        ? applyDagreLayout(flowData.nodes, flowData.edges, { direction: layoutDirection.value })
        : flowData.nodes;
      if (childNodes.length === 0) return;

      const minX = Math.min(...childNodes.map((node) => node.position?.x ?? 0));
      const minY = Math.min(...childNodes.map((node) => node.position?.y ?? 0));
      const anchor = parentNode.position ?? { x: 0, y: 0 };
      const childId = (stepId: string) => traceChildNodeId(parentDisplayId, stepId);

      for (const node of childNodes) {
        const displayId = childId(node.id);
        const displayNode = {
          ...node,
          id: displayId,
          position: {
            x: anchor.x + TRACE_EXPANSION_GAP_X + ((node.position?.x ?? 0) - minX),
            y: anchor.y + TRACE_EXPANSION_GAP_Y + ((node.position?.y ?? 0) - minY) + index * 36,
          },
          selectable: false,
          draggable: false,
          connectable: false,
          class: withTransientClass(node.class, 'trace-expanded-subrule'),
          data: {
            ...node.data,
            traceOnly: true,
            traceParentStepId: parentDisplayId,
            traceSubRuleRef: name,
          },
        };
        nodeLookup.set(displayId, displayNode);
        expansionNodes.push(displayNode);
      }

      for (const edge of flowData.edges) {
        expansionEdges.push({
          ...edge,
          id: `${TRACE_EXPANSION_PREFIX}${parentDisplayId}__edge__${edge.id}`,
          source: childId(edge.source),
          target: childId(edge.target),
          selectable: false,
          updatable: false,
          class: withTransientClass(edge.class, 'trace-expanded-subrule-edge'),
          data: {
            ...edge.data,
            traceOnly: true,
            traceParentStepId: parentDisplayId,
            traceSubRuleRef: name,
          },
        });
      }

      const entryNodeId = childId(graph.entryStep);
      const bridgeEdge = createEdge(parentDisplayId, entryNodeId, {
        sourceHandle: 'output',
        targetHandle: 'input',
        renderStyle: edgeStyle.value,
      });
      expansionEdges.push({
        ...bridgeEdge,
        id: `${TRACE_EXPANSION_PREFIX}${parentDisplayId}__bridge`,
        selectable: false,
        updatable: false,
        class: withTransientClass(bridgeEdge.class, 'trace-subrule-bridge'),
        data: {
          ...bridgeEdge.data,
          traceOnly: true,
          traceParentStepId: parentDisplayId,
          traceSubRuleRef: name,
        },
      });
      edgeIds.add(`${TRACE_EXPANSION_PREFIX}${parentDisplayId}__bridge`);

      const childOrderPrefix = orderPrefix ? `${orderPrefix}.${index + 1}` : `${index + 1}`;
      addStepAnnotations(step.sub_rule_frames, childId, childOrderPrefix, trace.resultCode);
      expandNestedSteps(step.sub_rule_frames, childId, childOrderPrefix);
    });
  }

  expandNestedSteps(trace.steps, (stepId) => stepId, '');
  return { nodes: expansionNodes, edges: expansionEdges };
}

/**
 * Apply execution trace overlay to the flow
 */
function applyExecutionTrace(trace: ExecutionTraceData) {
  removeTraceExpansion();
  showExecutionOverlay.value = true;

  // Build annotations map
  const annotations = new Map<string, StepTraceInfo>();
  const pathSteps = trace.path || [];

  trace.steps.forEach((step, index) => {
    const isEntry = index === 0;
    const isTerminal = index === trace.steps.length - 1;

    annotations.set(step.id, {
      stepId: step.id,
      stepName: step.name,
      durationUs: step.duration_us,
      order: index + 1,
      isEntry,
      isTerminal,
      resultCode: isTerminal ? trace.resultCode : undefined,
    });
  });

  // Highlight the execution path
  const nodeIds = new Set<string>(trace.steps.map((s) => s.id));
  const edgeIds = new Set<string>();

  // Find edges that connect consecutive steps in the path
  for (let i = 0; i < pathSteps.length - 1; i++) {
    const sourceId = pathSteps[i];
    const targetId = pathSteps[i + 1];

    const edge = edges.value.find((e) => e.source === sourceId && e.target === targetId);
    if (edge) {
      edgeIds.add(edge.id);
    }
  }

  const expansion = buildSubRuleTraceExpansion(trace, annotations, nodeIds, edgeIds);
  if (expansion.nodes.length > 0 || expansion.edges.length > 0) {
    nodes.value = [...nodes.value, ...expansion.nodes];
    edges.value = [...edges.value, ...expansion.edges];
  }

  executionAnnotations.value = annotations;
  highlightedNodeIds.value = nodeIds;
  highlightedEdgeIds.value = edgeIds;
  applyExecutionHighlightStyles();

  // Fit view to show the execution path
  setTimeout(() => {
    fitView({ padding: 0.2 });
  }, 100);
}

/**
 * Clear execution trace overlay
 */
function clearExecutionTrace() {
  pendingTraceApplyToken++;
  showExecutionOverlay.value = false;
  executionAnnotations.value = new Map();
  highlightedNodeIds.value = new Set();
  highlightedEdgeIds.value = new Set();
  removeTraceExpansion();
  applyHighlightStyles();
}

/**
 * Apply execution-specific highlight styles (green for executed path)
 */
function applyExecutionHighlightStyles() {
  const hasHighlight = highlightedNodeIds.value.size > 0;

  // Update nodes with execution highlight classes
  nodes.value = nodes.value.map((node) => {
    if (node.type === 'group') return node;

    const isHighlighted = highlightedNodeIds.value.has(node.id);
    const annotation = executionAnnotations.value.get(node.id);

    return {
      ...node,
      class: withTransientClass(
        node.class,
        hasHighlight ? (isHighlighted ? 'execution-highlighted' : 'execution-dimmed') : null
      ),
      data: {
        ...node.data,
        executionAnnotation: annotation || null,
      },
    };
  });

  // Update edges with execution highlight classes
  edges.value = edges.value.map((edge) => {
    const isHighlighted = highlightedEdgeIds.value.has(edge.id);
    return {
      ...edge,
      class: withTransientClass(
        edge.class,
        hasHighlight ? (isHighlighted ? 'execution-highlighted' : 'execution-dimmed') : null
      ),
      animated: isHighlighted, // Animate executed edges
    };
  });
}

// ============================================
// Path highlighting functionality
// ============================================

/**
 * Find all nodes and edges in the path connected to a given node
 * This traces upstream (incoming) and downstream (outgoing) paths SEPARATELY
 * to avoid traversing the entire connected graph
 */
function findConnectedPath(nodeId: string): { nodeIds: Set<string>; edgeIds: Set<string> } {
  const nodeIds = new Set<string>();
  const edgeIds = new Set<string>();

  // Skip group nodes
  const node = nodes.value.find((n) => n.id === nodeId);
  if (!node || node.type === 'group') {
    return { nodeIds, edgeIds };
  }

  // Build adjacency maps
  const outgoingEdges = new Map<string, Array<{ edgeId: string; targetId: string }>>();
  const incomingEdges = new Map<string, Array<{ edgeId: string; sourceId: string }>>();

  for (const edge of edges.value) {
    if (!outgoingEdges.has(edge.source)) {
      outgoingEdges.set(edge.source, []);
    }
    outgoingEdges.get(edge.source)!.push({ edgeId: edge.id, targetId: edge.target });

    if (!incomingEdges.has(edge.target)) {
      incomingEdges.set(edge.target, []);
    }
    incomingEdges.get(edge.target)!.push({ edgeId: edge.id, sourceId: edge.source });
  }

  // Add the selected node itself
  nodeIds.add(nodeId);

  // Trace DOWNSTREAM only (follow edges in their direction: source -> target)
  const downstreamVisited = new Set<string>([nodeId]);
  const downstreamQueue: string[] = [nodeId];

  while (downstreamQueue.length > 0) {
    const currentId = downstreamQueue.shift()!;

    const outgoing = outgoingEdges.get(currentId) || [];
    for (const { edgeId, targetId } of outgoing) {
      // Skip group nodes
      const targetNode = nodes.value.find((n) => n.id === targetId);
      if (targetNode?.type === 'group') continue;

      edgeIds.add(edgeId);
      nodeIds.add(targetId);

      if (!downstreamVisited.has(targetId)) {
        downstreamVisited.add(targetId);
        downstreamQueue.push(targetId);
      }
    }
  }

  // Trace UPSTREAM only (follow edges backwards: target -> source)
  const upstreamVisited = new Set<string>([nodeId]);
  const upstreamQueue: string[] = [nodeId];

  while (upstreamQueue.length > 0) {
    const currentId = upstreamQueue.shift()!;

    const incoming = incomingEdges.get(currentId) || [];
    for (const { edgeId, sourceId } of incoming) {
      // Skip group nodes
      const sourceNode = nodes.value.find((n) => n.id === sourceId);
      if (sourceNode?.type === 'group') continue;

      edgeIds.add(edgeId);
      nodeIds.add(sourceId);

      if (!upstreamVisited.has(sourceId)) {
        upstreamVisited.add(sourceId);
        upstreamQueue.push(sourceId);
      }
    }
  }

  return { nodeIds, edgeIds };
}

/**
 * Update highlighted path based on selected node
 */
function updateHighlightedPath(nodeId: string | null) {
  if (!nodeId) {
    // Clear highlights
    highlightedNodeIds.value = new Set();
    highlightedEdgeIds.value = new Set();
    applyHighlightStyles();
    return;
  }

  const { nodeIds, edgeIds } = findConnectedPath(nodeId);
  highlightedNodeIds.value = nodeIds;
  highlightedEdgeIds.value = edgeIds;
  applyHighlightStyles();
}

/**
 * Apply highlight styles to nodes and edges
 */
function applyHighlightStyles() {
  const hasHighlight = highlightedNodeIds.value.size > 0;

  // Update nodes with highlight/dim classes
  nodes.value = nodes.value.map((node) => {
    if (node.type === 'group') return node;

    const isHighlighted = highlightedNodeIds.value.has(node.id);
    return {
      ...node,
      class: withTransientClass(
        node.class,
        hasHighlight ? (isHighlighted ? 'path-highlighted' : 'path-dimmed') : null
      ),
    };
  });

  // Update edges with highlight/dim classes
  edges.value = edges.value.map((edge) => {
    const isHighlighted = highlightedEdgeIds.value.has(edge.id);
    return {
      ...edge,
      class: withTransientClass(
        edge.class,
        hasHighlight ? (isHighlighted ? 'path-highlighted' : 'path-dimmed') : null
      ),
    };
  });
}

// ============================================
// Event handlers
// ============================================

// Handle node selection
function onNodeClick(event: any) {
  if (isCanvasReadOnly.value) {
    hideContextMenu();
    return;
  }
  const nodeId = event.node?.id;
  if (!nodeId) return;

  // Check if Ctrl/Cmd is pressed for multi-select
  if (event.event?.ctrlKey || event.event?.metaKey) {
    if (selectedNodeIds.value.includes(nodeId)) {
      selectedNodeIds.value = selectedNodeIds.value.filter((id) => id !== nodeId);
    } else {
      selectedNodeIds.value = [...selectedNodeIds.value, nodeId];
    }
    // Clear highlight on multi-select
    updateHighlightedPath(null);
  } else {
    selectedNodeIds.value = [nodeId];
    // Highlight connected path for single selection (skip groups)
    const node = nodes.value.find((n) => n.id === nodeId);
    if (node?.type !== 'group') {
      updateHighlightedPath(nodeId);
    } else {
      updateHighlightedPath(null);
    }
  }

  selectedNodeId.value = nodeId;
  hideContextMenu();
}

function onPaneClick() {
  if (isCanvasReadOnly.value) {
    hideContextMenu();
    return;
  }
  selectedNodeId.value = null;
  selectedNodeIds.value = [];
  updateHighlightedPath(null); // Clear highlight
  hideContextMenu();
}

function onNodeDblClick(event: any) {
  const step = event.node?.data?.step;
  if (step?.type === 'sub_rule' && step.refName) {
    emit('open-sub-rule', step.refName);
  }
}

// Handle right-click on pane
function onPaneContextMenu(event: MouseEvent) {
  if (isCanvasReadOnly.value) return;
  event.preventDefault();
  selectedEdgeId.value = null; // Clear edge selection
  // Only show context menu if there are selected nodes
  if (selectedNodeIds.value.length > 0) {
    showContextMenuAt(event);
  }
}

// Handle right-click on node
function onNodeContextMenu(event: any) {
  if (isCanvasReadOnly.value) return;
  const nodeEvent = event.event as MouseEvent;
  nodeEvent.preventDefault();
  nodeEvent.stopPropagation();

  selectedEdgeId.value = null; // Clear edge selection
  const nodeId = event.node?.id;
  if (!nodeId) return;

  // Add node to selection if not already selected
  if (!selectedNodeIds.value.includes(nodeId)) {
    selectedNodeIds.value = [nodeId];
    selectedNodeId.value = nodeId;
  }

  showContextMenuAt(nodeEvent);
}

// Handle right-click on edge
function onEdgeContextMenu(event: any) {
  if (isCanvasReadOnly.value) return;
  const edgeEvent = event.event as MouseEvent;
  edgeEvent.preventDefault();
  edgeEvent.stopPropagation();

  const edgeId = event.edge?.id;
  if (!edgeId) return;

  selectedEdgeId.value = edgeId;
  selectedNodeId.value = null;
  selectedNodeIds.value = [];

  showContextMenuAt(edgeEvent);
}

// Handle selection change from Vue Flow
function onSelectionChange(params: any) {
  if (isCanvasReadOnly.value) return;
  const nodeIds = params.nodes?.map((n: any) => n.id) || [];
  selectedNodeIds.value = nodeIds;
  if (nodeIds.length === 1) {
    selectedNodeId.value = nodeIds[0];
  } else if (nodeIds.length === 0) {
    selectedNodeId.value = null;
  }
}

// Handle node changes (position, etc.)
onNodesChange((changes) => {
  // Update positions
  for (const change of changes) {
    if (change.type === 'position' && change.position) {
      const node = nodes.value.find((n) => n.id === change.id);
      if (node) {
        node.position = change.position;
      }
    }
  }
  syncToRuleset();
});

// Handle edge changes
onEdgesChange((changes) => {
  for (const change of changes) {
    if (change.type === 'remove') {
      edges.value = edges.value.filter((e) => e.id !== change.id);
    }
  }
  syncToRuleset();
});

// Handle new connections
onConnect((params) => {
  const newEdge = createEdge(params.source, params.target, {
    sourceHandle: params.sourceHandle || undefined,
    targetHandle: params.targetHandle || undefined,
    renderStyle: edgeStyle.value,
  });

  // For action/sub_rule nodes, an output handle can only have one outgoing exec edge.
  // Replace any existing outgoing exec edge from the same source+handle to avoid
  // findLinearExecutionEdge picking up the stale edge and silently discarding the new one.
  const isLinearExecEdge =
    !newEdge.data?.branchId && !newEdge.data?.isDefault && newEdge.data?.edgeType === 'exec';
  const filtered = isLinearExecEdge
    ? edges.value.filter(
        (e) =>
          !(
            e.source === newEdge.source &&
            e.data?.edgeType === 'exec' &&
            !e.data?.branchId &&
            !e.data?.isDefault
          )
      )
    : edges.value;

  edges.value = [...filtered, newEdge];
  syncToRuleset();
});

// Handle edge update (reconnect)
onEdgeUpdateStart(() => {
  edgeUpdating.value = true;
});

onEdgeUpdateEnd(() => {
  edgeUpdating.value = false;
});

// Handle edge reconnection
function onEdgeUpdateHandler(oldEdge: any, newConnection: any) {
  // Use Vue Flow's updateEdge helper
  const success = updateEdge(oldEdge, newConnection);
  if (success) {
    syncToRuleset();
  }
}

// ============================================
// Group drag handling - move child nodes together
// ============================================

// When starting to drag a group, record its position and child nodes
onNodeDragStart(({ node }) => {
  if (node.type !== 'group') {
    groupDragState.value = null;
    return;
  }

  // Get the group data to find child step IDs
  const groupData = node.data;
  const stepIds = groupData?.stepIds || groupData?.group?.stepIds || [];

  groupDragState.value = {
    groupId: node.id,
    startPosition: { x: node.position.x, y: node.position.y },
    childNodeIds: stepIds,
  };
});

// During group drag, update all child node positions
onNodeDrag(({ node }) => {
  if (node.type !== 'group' || !groupDragState.value) return;
  if (groupDragState.value.groupId !== node.id) return;

  // Calculate the delta (how much the group has moved)
  const deltaX = node.position.x - groupDragState.value.startPosition.x;
  const deltaY = node.position.y - groupDragState.value.startPosition.y;

  if (deltaX === 0 && deltaY === 0) return;

  // Update all child nodes by the same delta
  nodes.value = nodes.value.map((n) => {
    if (groupDragState.value!.childNodeIds.includes(n.id)) {
      return {
        ...n,
        position: {
          x: n.position.x + deltaX,
          y: n.position.y + deltaY,
        },
      };
    }
    return n;
  });

  // Update the start position for the next drag event
  groupDragState.value.startPosition = { x: node.position.x, y: node.position.y };
});

// Handle node drag stop - check if node was dropped into a group
onNodeDragStop(({ node }) => {
  // Clear group drag state
  if (node.type === 'group') {
    groupDragState.value = null;
    syncToRuleset(); // Sync positions after group drag
    return;
  }

  // Find intersecting group nodes
  const intersectingGroups = getIntersectingNodes(node).filter((n: any) => n.type === 'group');

  if (intersectingGroups.length > 0) {
    // Get the first (topmost) group
    const targetGroup = intersectingGroups[0];

    // If node is already in this group, do nothing
    if (node.parentNode === targetGroup.id) return;

    // Calculate relative position within the group
    const relativeX = node.position.x - targetGroup.position.x;
    const relativeY = node.position.y - targetGroup.position.y - 32; // Account for header

    // Update node to be child of group
    nodes.value = nodes.value.map((n) => {
      if (n.id === node.id) {
        return {
          ...n,
          parentNode: targetGroup.id,
          extent: 'parent',
          position: { x: Math.max(10, relativeX), y: Math.max(10, relativeY) },
        };
      }
      return n;
    });

    syncToRuleset();
  } else if (node.parentNode) {
    // Node was dragged out of a group - remove parent
    const parentGroup = nodes.value.find((n) => n.id === node.parentNode);
    if (parentGroup) {
      // Calculate absolute position
      const absoluteX = node.position.x + parentGroup.position.x;
      const absoluteY = node.position.y + parentGroup.position.y + 32;

      nodes.value = nodes.value.map((n) => {
        if (n.id === node.id) {
          return {
            ...n,
            parentNode: undefined,
            extent: undefined,
            position: { x: absoluteX, y: absoluteY },
          };
        }
        return n;
      });

      syncToRuleset();
    }
  }
});

function createStep(type: NodeCreationType, id: string): Step {
  switch (type) {
    case 'decision':
      return StepFactory.decision({
        id,
        name: t('step.decision'),
        branches: [],
        defaultNextStepId: '',
      });
    case 'action':
      return StepFactory.action({
        id,
        name: t('step.action'),
        nextStepId: '',
      });
    case 'terminal':
      return StepFactory.terminal({
        id,
        name: t('step.terminal'),
        code: 'RESULT',
      });
    case 'sub_rule':
      const firstAsset =
        props.managedSubRules.find((asset) => asset.scope === 'project') ??
        props.managedSubRules[0];
      const firstSubRuleName =
        firstAsset?.name ?? Object.keys(props.modelValue.subRules ?? {})[0] ?? '';
      return StepFactory.subRule({
        id,
        name: t('step.subRule'),
        refName: firstSubRuleName,
        assetRef: {
          scope: firstAsset?.scope ?? 'project',
          name: firstSubRuleName,
        },
        nextStepId: '',
      });
  }
}

function addNodeAtPosition(type: NodeCreationType, position: { x: number; y: number }) {
  const id = generateId('step');
  const step = createStep(type, id);
  const newNode = createNodeFromStep(step, position, nodes.value.length === 0);

  nodes.value.push(newNode);
  selectedNodeId.value = id;
  selectedNodeIds.value = [id];
  selectedEdgeId.value = null;
  syncToRuleset();
}

function getVisibleCanvasCenterPosition() {
  if (!flowCanvasContainer.value) {
    return getSuggestedPosition(nodes.value, selectedNodeId.value || undefined);
  }

  const rect = flowCanvasContainer.value.getBoundingClientRect();
  const centerPosition = screenToFlowCoordinate({
    x: rect.left + rect.width / 2,
    y: rect.top + rect.height / 2,
  });

  return {
    x: centerPosition.x - DEFAULT_NODE_DROP_OFFSET.x,
    y: centerPosition.y - DEFAULT_NODE_DROP_OFFSET.y,
  };
}

// Add new node
function addNode(type: NodeCreationType) {
  const position = getVisibleCanvasCenterPosition();
  addNodeAtPosition(type, position);
}

function isNodeCreationType(value: string): value is NodeCreationType {
  return value === 'decision' || value === 'action' || value === 'terminal' || value === 'sub_rule';
}

function clearNodeDragPreview() {
  nodeDragPreview.value = null;
}

function endNodeDrag() {
  draggedNodeType.value = null;
  clearNodeDragPreview();
}

function updateNodeDragPreview(event: DragEvent, type: NodeCreationType) {
  const container = event.currentTarget as HTMLElement | null;
  if (!container) return;

  const rect = container.getBoundingClientRect();

  nodeDragPreview.value = {
    type,
    x: event.clientX - rect.left - DEFAULT_NODE_DROP_OFFSET.x,
    y: event.clientY - rect.top - DEFAULT_NODE_DROP_OFFSET.y,
  };
}

function onCanvasDragOver(event: DragEvent) {
  if (!event.dataTransfer || isCanvasReadOnly.value) return;

  const isToolbarNodeDrag = Array.from(event.dataTransfer.types).includes(FLOW_NODE_DRAG_TYPE);
  if (!isToolbarNodeDrag || !draggedNodeType.value) return;

  event.preventDefault();
  event.dataTransfer.dropEffect = 'copy';
  updateNodeDragPreview(event, draggedNodeType.value);
}

function onCanvasDragLeave(event: DragEvent) {
  if (!nodeDragPreview.value) return;

  const container = event.currentTarget as HTMLElement | null;
  if (!container) {
    clearNodeDragPreview();
    return;
  }

  const rect = container.getBoundingClientRect();
  const hasLeftContainer =
    event.clientX <= rect.left ||
    event.clientX >= rect.right ||
    event.clientY <= rect.top ||
    event.clientY >= rect.bottom;

  if (hasLeftContainer) {
    clearNodeDragPreview();
  }
}

function onCanvasDrop(event: DragEvent) {
  if (isCanvasReadOnly.value || !event.dataTransfer) return;

  const droppedType = event.dataTransfer.getData(FLOW_NODE_DRAG_TYPE);
  const type = droppedType || draggedNodeType.value;
  if (!type || !isNodeCreationType(type)) return;

  event.preventDefault();
  clearNodeDragPreview();

  const flowPosition = screenToFlowCoordinate({
    x: event.clientX,
    y: event.clientY,
  });

  addNodeAtPosition(type, {
    x: flowPosition.x - DEFAULT_NODE_DROP_OFFSET.x,
    y: flowPosition.y - DEFAULT_NODE_DROP_OFFSET.y,
  });
}

// Delete selected node
function deleteSelectedNode() {
  if (!selectedNodeId.value) return;

  const nodeToDelete = nodes.value.find((n) => n.id === selectedNodeId.value);

  // If deleting a group, unparent all child nodes first
  if (nodeToDelete?.type === 'group') {
    nodes.value = nodes.value.map((n) => {
      if (n.parentNode === selectedNodeId.value) {
        return { ...n, parentNode: undefined, extent: undefined };
      }
      return n;
    });
  }

  // Remove node
  nodes.value = nodes.value.filter((n) => n.id !== selectedNodeId.value);

  // Remove connected edges
  edges.value = edges.value.filter(
    (e) => e.source !== selectedNodeId.value && e.target !== selectedNodeId.value
  );

  selectedNodeId.value = null;
  syncToRuleset();
}

// Add new group (empty or from selected nodes)
function addGroup() {
  // Get selected step nodes (not groups)
  const selectedSteps = nodes.value.filter(
    (n) => selectedNodeIds.value.includes(n.id) && n.type !== 'group'
  );

  let position: { x: number; y: number };
  let size: { width: number; height: number };

  if (selectedSteps.length > 0) {
    // Calculate bounding box of selected nodes
    const padding = 40;
    const headerHeight = 32;

    let minX = Infinity,
      minY = Infinity,
      maxX = -Infinity,
      maxY = -Infinity;
    for (const node of selectedSteps) {
      const nodeWidth = 180; // Approximate node width
      const nodeHeight = 100; // Approximate node height
      minX = Math.min(minX, node.position.x);
      minY = Math.min(minY, node.position.y);
      maxX = Math.max(maxX, node.position.x + nodeWidth);
      maxY = Math.max(maxY, node.position.y + nodeHeight);
    }

    position = { x: minX - padding, y: minY - padding - headerHeight };
    size = {
      width: maxX - minX + padding * 2,
      height: maxY - minY + padding * 2 + headerHeight,
    };
  } else {
    // Create empty group at suggested position
    position = getSuggestedPosition(nodes.value, selectedNodeId.value || undefined);
    size = { width: 300, height: 200 };
  }

  const newGroup = createGroupNode(t('flow.newGroup'), position, size);

  // Add zIndex to keep at bottom
  const groupWithZIndex = {
    ...newGroup,
    zIndex: -1000,
    connectable: false,
  };

  // Insert group at the beginning (so it renders behind other nodes)
  nodes.value = [groupWithZIndex, ...nodes.value];
  selectedNodeId.value = newGroup.id;
  selectedNodeIds.value = [newGroup.id];

  syncToRuleset();
}

// Create group from selected nodes (via context menu)
function createGroupFromSelection() {
  if (selectedNodeIds.value.length === 0) return;
  addGroup();
  hideContextMenu();
}

function isStepFlowNode(node: any): node is FlowNode {
  return node?.type !== 'group' && !!node?.data?.step;
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value));
}

function getExecutableEdges(): FlowEdge[] {
  return edges.value.filter(
    (edge) => edge.data?.edgeType === 'exec' || edge.data?.edgeType === 'exec-branch'
  );
}

function getExtractSubRuleEligibility(): ExtractSubRuleEligibility {
  const selectedIds = new Set(selectedNodeIds.value);
  const selectedNodes = nodes.value.filter(
    (node): node is FlowNode => selectedIds.has(node.id) && isStepFlowNode(node)
  );
  const empty: ExtractSubRuleEligibility = {
    valid: false,
    selectedNodes,
    internalEdges: [],
    externalIncomingEdges: [],
    externalOutgoingEdges: [],
  };

  if (selectedIds.size === 0) {
    return { ...empty, reason: t('flow.extractSubRuleSelectNodes') };
  }

  if (selectedNodes.length !== selectedIds.size) {
    return { ...empty, reason: t('flow.extractSubRuleNoGroups') };
  }

  const selectedStepIds = new Set(selectedNodes.map((node) => node.id));
  const executableEdges = getExecutableEdges();
  const internalEdges = executableEdges.filter(
    (edge) => selectedStepIds.has(edge.source) && selectedStepIds.has(edge.target)
  );
  const externalIncomingEdges = executableEdges.filter(
    (edge) => !selectedStepIds.has(edge.source) && selectedStepIds.has(edge.target)
  );
  const externalOutgoingEdges = executableEdges.filter(
    (edge) => selectedStepIds.has(edge.source) && !selectedStepIds.has(edge.target)
  );

  const incomingTargets = new Set(externalIncomingEdges.map((edge) => edge.target));
  if (incomingTargets.size > 1) {
    return {
      ...empty,
      internalEdges,
      externalIncomingEdges,
      externalOutgoingEdges,
      reason: t('flow.extractSubRuleSingleEntry'),
    };
  }

  const internalIncomingTargets = new Set(internalEdges.map((edge) => edge.target));
  let entryId: string | undefined;
  if (incomingTargets.size === 1) {
    entryId = [...incomingTargets][0];
  } else if (selectedStepIds.has(props.modelValue.startStepId)) {
    entryId = props.modelValue.startStepId;
  } else {
    const rootNodes = selectedNodes.filter((node) => !internalIncomingTargets.has(node.id));
    if (rootNodes.length !== 1) {
      return {
        ...empty,
        internalEdges,
        externalIncomingEdges,
        externalOutgoingEdges,
        reason: t('flow.extractSubRuleSingleEntry'),
      };
    }
    entryId = rootNodes[0].id;
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

  if (reachable.size !== selectedNodes.length) {
    return {
      ...empty,
      internalEdges,
      externalIncomingEdges,
      externalOutgoingEdges,
      reason: t('flow.extractSubRuleConnected'),
    };
  }

  const exitTargets = new Set(externalOutgoingEdges.map((edge) => edge.target));
  if (exitTargets.size > 1) {
    return {
      ...empty,
      internalEdges,
      externalIncomingEdges,
      externalOutgoingEdges,
      reason: t('flow.extractSubRuleSingleExit'),
    };
  }

  const hasTerminal = selectedNodes.some((node) => node.data?.step?.type === 'terminal');
  if (hasTerminal && externalOutgoingEdges.length > 0) {
    return {
      ...empty,
      internalEdges,
      externalIncomingEdges,
      externalOutgoingEdges,
      reason: t('flow.extractSubRuleNoMixedExit'),
    };
  }

  if (!hasTerminal && externalOutgoingEdges.length === 0) {
    return {
      ...empty,
      internalEdges,
      externalIncomingEdges,
      externalOutgoingEdges,
      reason: t('flow.extractSubRuleNeedsExit'),
    };
  }

  return {
    valid: true,
    entryId,
    exitTargetId: exitTargets.size === 1 ? [...exitTargets][0] : undefined,
    selectedNodes,
    internalEdges,
    externalIncomingEdges,
    externalOutgoingEdges,
  };
}

function sanitizeSubRuleName(name: string) {
  return (
    name
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9_]+/g, '_')
      .replace(/^_+|_+$/g, '') || 'sub_rule'
  );
}

function buildExtractSubRuleName(entryNode: FlowNode) {
  const rulesetName = sanitizeSubRuleName(props.modelValue.config.name || 'ruleset');
  const entryName = sanitizeSubRuleName(entryNode.data?.step?.name || entryNode.id);
  return `${rulesetName}_${entryName}`;
}

async function applyExtractSubRuleRequest(request: ExtractSubRuleRequest) {
  if (lastHandledExtractSubRuleRequestId === request.id) return;
  lastHandledExtractSubRuleRequestId = request.id;

  if (isCanvasReadOnly.value) {
    emit('extract-sub-rule-invalid', t('flow.extractSubRuleReadOnly'));
    return;
  }

  await nextTick();
  if (nodes.value.length === 0) {
    await nextTick();
  }

  const requestedIds = request.stepIds.filter((id) =>
    nodes.value.some((node) => node.id === id && isStepFlowNode(node))
  );
  if (requestedIds.length === 0) {
    emit('extract-sub-rule-invalid', t('flow.extractSubRuleSelectNodes'));
    return;
  }

  const requestedIdSet = new Set(requestedIds);
  selectedNodeIds.value = requestedIds;
  selectedNodeId.value = requestedIds[0] ?? null;
  selectedEdgeId.value = null;
  highlightedNodeIds.value = requestedIdSet;
  highlightedEdgeIds.value = new Set(
    getExecutableEdges()
      .filter((edge) => requestedIdSet.has(edge.source) && requestedIdSet.has(edge.target))
      .map((edge) => edge.id)
  );
  applyHighlightStyles();

  await nextTick();
  extractSubRuleFromSelection({ reportInvalid: true });
}

function cloneEdgeForChild(edge: FlowEdge, targetOverride?: string): FlowEdge {
  return createEdge(edge.source, targetOverride ?? edge.target, {
    branchId: edge.data?.branchId,
    isDefault: edge.data?.isDefault,
    sourceHandle: edge.sourceHandle || undefined,
    targetHandle: 'input',
    condition: edge.data?.condition,
    renderStyle: edgeStyle.value,
  });
}

function buildChildFlowNodes(
  selectedNodes: FlowNode[],
  entryId: string,
  returnStep?: Step
): FlowNode[] {
  const minX = Math.min(...selectedNodes.map((node) => node.position.x));
  const minY = Math.min(...selectedNodes.map((node) => node.position.y));
  const childNodes = selectedNodes.map((node) => {
    const step = cloneJson(node.data!.step);
    const position = {
      x: node.position.x - minX + 120,
      y: node.position.y - minY + 120,
    };
    step.position = position;
    return createNodeFromStep(step, position, node.id === entryId);
  });

  if (returnStep) {
    const maxX = Math.max(...selectedNodes.map((node) => node.position.x));
    const averageY =
      selectedNodes.reduce((sum, node) => sum + node.position.y, 0) / selectedNodes.length;
    const position = {
      x: maxX - minX + 360,
      y: averageY - minY + 120,
    };
    returnStep.position = position;
    childNodes.push(createNodeFromStep(returnStep, position, false));
  }

  return childNodes;
}

function updateGroupNodesAfterExtraction(selectedStepIds: Set<string>, subRuleStepId: string) {
  const groupNodes = nodes.value.filter((node) => node.type === 'group');
  const containingGroups = groupNodes.filter(
    (node) => node.data?.stepIds?.some((stepId: string) => selectedStepIds.has(stepId))
  );

  return groupNodes.map((node) => {
    if (!node.data?.group) return node;

    const originalStepIds = node.data.stepIds ?? node.data.group.stepIds ?? [];
    const selectedInGroup = originalStepIds.filter((stepId: string) => selectedStepIds.has(stepId));
    if (selectedInGroup.length === 0) return node;

    const nextStepIds = originalStepIds.filter((stepId: string) => !selectedStepIds.has(stepId));
    if (containingGroups.length === 1) {
      const insertAt = originalStepIds.findIndex((stepId: string) => selectedStepIds.has(stepId));
      nextStepIds.splice(Math.max(insertAt, 0), 0, subRuleStepId);
    }

    return {
      ...node,
      data: {
        ...node.data,
        stepIds: nextStepIds,
        group: {
          ...node.data.group,
          stepIds: nextStepIds,
        },
      },
    };
  });
}

function extractSubRuleFromSelection(options?: { reportInvalid?: boolean }) {
  const eligibility = extractSubRuleEligibility.value;
  if (!eligibility.valid || !eligibility.entryId) {
    if (options?.reportInvalid && eligibility.reason) {
      emit('extract-sub-rule-invalid', eligibility.reason);
    }
    hideContextMenu();
    return;
  }

  const selectedStepIds = new Set(eligibility.selectedNodes.map((node) => node.id));
  const entryNode = eligibility.selectedNodes.find((node) => node.id === eligibility.entryId);
  if (!entryNode) {
    hideContextMenu();
    return;
  }

  const suggestedName = buildExtractSubRuleName(entryNode);
  const displayName = entryNode.data?.step?.name || t('step.subRule');
  const subRuleStepId = generateId('step');
  const returnStep =
    eligibility.externalOutgoingEdges.length > 0
      ? StepFactory.terminal({
          id: generateId('return'),
          name: t('flow.subRuleReturn'),
          code: 'OK',
        })
      : undefined;
  const returnStepId = returnStep?.id;

  const childNodes = buildChildFlowNodes(
    eligibility.selectedNodes,
    eligibility.entryId,
    returnStep
  );
  const childEdges = [
    ...eligibility.internalEdges.map((edge) => cloneEdgeForChild(edge)),
    ...eligibility.externalOutgoingEdges.map((edge) => cloneEdgeForChild(edge, returnStepId)),
  ];
  const childRuleset = flowToRuleset(
    childNodes,
    childEdges,
    {
      ...props.modelValue.config,
      name: suggestedName,
      version: '0.1.0',
      description: t('flow.extractedSubRuleDescription'),
      metadata: {
        ...(props.modelValue.config.metadata ?? {}),
        extractedFrom: props.modelValue.config.name,
        extractedAt: new Date().toISOString(),
      },
    },
    eligibility.entryId,
    undefined,
    props.modelValue.subRules
  );

  const minX = Math.min(...eligibility.selectedNodes.map((node) => node.position.x));
  const minY = Math.min(...eligibility.selectedNodes.map((node) => node.position.y));
  const subRuleStep = StepFactory.subRule({
    id: subRuleStepId,
    name: displayName,
    refName: suggestedName,
    assetRef: {
      scope: 'project',
      name: suggestedName,
    },
    nextStepId: eligibility.exitTargetId ?? '',
    position: { x: minX, y: minY },
  });
  const subRuleNode = createNodeFromStep(
    subRuleStep,
    { x: minX, y: minY },
    selectedStepIds.has(props.modelValue.startStepId)
  );

  const parentStepNodes = [
    ...nodes.value.filter((node) => isStepFlowNode(node) && !selectedStepIds.has(node.id)),
    subRuleNode,
  ];
  const parentGroupNodes = updateGroupNodesAfterExtraction(selectedStepIds, subRuleStepId);
  const parentEdges = [
    ...edges.value.filter(
      (edge) => !selectedStepIds.has(edge.source) && !selectedStepIds.has(edge.target)
    ),
    ...eligibility.externalIncomingEdges.map((edge) =>
      createEdge(edge.source, subRuleStepId, {
        branchId: edge.data?.branchId,
        isDefault: edge.data?.isDefault,
        sourceHandle: edge.sourceHandle || undefined,
        targetHandle: 'input',
        condition: edge.data?.condition,
        renderStyle: edgeStyle.value,
      })
    ),
  ];

  if (eligibility.exitTargetId) {
    parentEdges.push(
      createEdge(subRuleStepId, eligibility.exitTargetId, {
        sourceHandle: 'output',
        targetHandle: 'input',
        renderStyle: edgeStyle.value,
      })
    );
  }

  const parentStartStepId = selectedStepIds.has(props.modelValue.startStepId)
    ? subRuleStepId
    : props.modelValue.startStepId;
  const parentRuleset = flowToRuleset(
    parentStepNodes,
    parentEdges,
    buildFlowConfig(),
    parentStartStepId,
    parentGroupNodes,
    props.modelValue.subRules
  );

  emit('extract-sub-rule', {
    suggestedName,
    displayName,
    subRuleStepId,
    selectedStepCount: eligibility.selectedNodes.length,
    draft: childRuleset,
    parentRuleset,
  });
  hideContextMenu();
}

// Set as start from context menu
function setAsStartFromMenu() {
  if (selectedNodeId.value) {
    setAsStart(selectedNodeId.value);
  }
  hideContextMenu();
}

// Duplicate selected node
function duplicateSelectedNode() {
  if (!selectedStepNode.value?.data?.step) {
    hideContextMenu();
    return;
  }

  const originalStep = selectedStepNode.value.data.step;
  const newId = generateId('step');

  // Clone the step with a new ID
  let newStep: Step;
  switch (originalStep.type) {
    case 'decision':
      newStep = StepFactory.decision({
        ...originalStep,
        id: newId,
        name: `${originalStep.name} (copy)`,
        branches: originalStep.branches.map((b: any) => ({
          ...b,
          id: generateId('branch'),
          nextStepId: '', // Clear connections
        })),
        defaultNextStepId: '',
      });
      break;
    case 'action':
      newStep = StepFactory.action({
        ...originalStep,
        id: newId,
        name: `${originalStep.name} (copy)`,
        nextStepId: '',
      });
      break;
    case 'terminal':
      newStep = StepFactory.terminal({
        ...originalStep,
        id: newId,
        name: `${originalStep.name} (copy)`,
      });
      break;
    case 'sub_rule':
      newStep = StepFactory.subRule({
        ...originalStep,
        id: newId,
        name: `${originalStep.name} (copy)`,
        nextStepId: '',
      });
      break;
    default:
      hideContextMenu();
      return;
  }

  // Position the new node slightly offset from the original
  const position = {
    x: selectedStepNode.value.position.x + 40,
    y: selectedStepNode.value.position.y + 40,
  };

  const newNode = createNodeFromStep(newStep, position, false);
  nodes.value.push(newNode);

  // Select the new node
  selectedNodeId.value = newId;
  selectedNodeIds.value = [newId];

  syncToRuleset();
  hideContextMenu();
}

// Delete selected edge
function deleteSelectedEdge() {
  if (!selectedEdgeId.value) return;

  const edgeIdToDelete = selectedEdgeId.value;
  selectedEdgeId.value = null;

  // Use Vue Flow's removeEdges method
  removeEdges([edgeIdToDelete]);

  syncToRuleset();
  hideContextMenu();
}

// Reverse selected edge direction
function reverseSelectedEdge() {
  if (!selectedEdgeId.value) return;

  const edgeId = selectedEdgeId.value;
  const edge = edges.value.find((e) => e.id === edgeId);
  if (!edge) return;

  // Create new edge with swapped source and target
  const newEdge = createEdge(edge.target, edge.source, {
    sourceHandle: edge.targetHandle || undefined,
    targetHandle: edge.sourceHandle || undefined,
    renderStyle: edgeStyle.value,
  });

  // Remove old edge and add new edge using Vue Flow methods
  removeEdges([edgeId]);
  addEdges([newEdge]);

  selectedEdgeId.value = newEdge.id;
  syncToRuleset();
  hideContextMenu();
}

// Delete from context menu
function deleteFromMenu() {
  if (selectedEdgeId.value) {
    deleteSelectedEdge();
    return;
  }

  // Delete all selected nodes
  const idsToDelete = [...selectedNodeIds.value];

  for (const id of idsToDelete) {
    const nodeToDelete = nodes.value.find((n) => n.id === id);

    // If deleting a group, unparent all child nodes first
    if (nodeToDelete?.type === 'group') {
      nodes.value = nodes.value.map((n) => {
        if (n.parentNode === id) {
          return { ...n, parentNode: undefined, extent: undefined };
        }
        return n;
      });
    }

    // Remove node
    nodes.value = nodes.value.filter((n) => n.id !== id);

    // Remove connected edges
    edges.value = edges.value.filter((e) => e.source !== id && e.target !== id);
  }

  selectedNodeId.value = null;
  selectedNodeIds.value = [];
  syncToRuleset();
  hideContextMenu();
}

// Show context menu
function showContextMenuAt(event: MouseEvent) {
  event.preventDefault();
  contextMenuPosition.value = { x: event.clientX, y: event.clientY };
  showContextMenu.value = true;
}

// Hide context menu
function hideContextMenu() {
  showContextMenu.value = false;
}

// Set node as start
function setAsStart(nodeId: string) {
  nodes.value = nodes.value.map((n) => ({
    ...n,
    data: {
      ...n.data,
      isStart: n.id === nodeId,
    },
  }));

  const newRuleset = flowToRuleset(
    nodes.value,
    edges.value,
    buildFlowConfig(),
    nodeId,
    undefined,
    props.modelValue.subRules
  );
  emit('update:modelValue', newRuleset);
  emit('change', newRuleset);
}

// Auto layout
function autoLayout() {
  const groups = props.modelValue.groups || [];

  if (groups.length > 0) {
    // Use group-based layout when groups are defined
    const { nodes: layoutedNodes, groupUpdates } = applyGroupBasedLayout(
      nodes.value,
      edges.value,
      groups,
      { direction: layoutDirection.value }
    );

    // Update nodes with new positions (applyGroupBasedLayout already handles group nodes)
    nodes.value = layoutedNodes;

    // Sync group updates to ruleset
    if (groupUpdates.length > 0) {
      isInternalUpdate.value = true;

      const updatedGroups = groups.map((g) => {
        const update = groupUpdates.find((u) => u.id === g.id);
        if (update) {
          return {
            ...g,
            position: update.position || g.position,
            size: update.size || g.size,
          };
        }
        return g;
      });

      const newRuleset = {
        ...props.modelValue,
        config: buildFlowConfig(),
        groups: updatedGroups,
      };
      emit('update:modelValue', newRuleset);

      // Reset flag after Vue's next tick
      setTimeout(() => {
        isInternalUpdate.value = false;
      }, 0);
    }
  } else {
    // Use dagre layout when no groups are defined
    nodes.value = applyDagreLayout(nodes.value, edges.value, {
      direction: layoutDirection.value,
    });
  }

  // Fit view after layout
  setTimeout(() => {
    fitView({ padding: 0.2 });
  }, 50);
}

// Update node from property panel
function updateNode(updatedStep: Step) {
  nodes.value = nodes.value.map((n) => {
    if (n.id === updatedStep.id) {
      return {
        ...n,
        data: {
          ...n.data,
          step: updatedStep,
          label: updatedStep.name,
        },
      };
    }
    return n;
  });
  syncToRuleset();
}

// Update group name
function updateGroupName(newName: string) {
  if (!selectedGroupNode.value) return;

  nodes.value = nodes.value.map((n) => {
    if (n.id === selectedGroupNode.value?.id && n.data?.group) {
      return {
        ...n,
        data: {
          ...n.data,
          group: {
            ...n.data.group,
            name: newName,
          },
        },
      };
    }
    return n;
  });
  syncToRuleset();
}

function setEdgeStyle(style: EdgeRenderStyle) {
  if (edgeStyle.value === style) return;

  edgeStyle.value = style;
  edges.value = edges.value.map((edge) => ({
    ...edge,
    data: {
      ...edge.data,
      renderStyle: style,
    },
  }));
  syncToRuleset();
}

function setLayoutDirectionAndPersist(direction: LayoutDirection) {
  if (layoutDirection.value === direction) return;
  layoutDirection.value = direction;
  syncToRuleset();
}

// Computed edge type for Vue Flow
const defaultEdgeOptions = computed(() => ({
  type: 'ordo', // Use custom edge type
  animated: false,
}));

// Track if this is first initialization
const isFirstInit = ref(true);

// Initialize
onMounted(() => {
  // Force layout on first initialization
  initFromRuleset(isFirstInit.value);
  isFirstInit.value = false;
  if (props.extractSubRuleRequest) {
    void applyExtractSubRuleRequest(props.extractSubRuleRequest);
  }
});
</script>

<template>
  <div class="ordo-flow-editor" :class="{ disabled }">
    <!-- Toolbar -->
    <OrdoFlowToolbar
      v-if="!disabled"
      :edge-style="edgeStyle"
      :layout-direction="layoutDirection"
      :has-selection="!!selectedNodeId"
      @add-node="addNode"
      @start-node-drag="draggedNodeType = $event"
      @end-node-drag="endNodeDrag"
      @add-group="addGroup"
      @delete-node="deleteSelectedNode"
      @auto-layout="autoLayout"
      @set-edge-style="setEdgeStyle"
      @set-layout-direction="setLayoutDirectionAndPersist"
    />

    <!-- Main Canvas -->
    <div
      ref="flowCanvasContainer"
      class="flow-canvas-container"
      @contextmenu="onPaneContextMenu"
      @dragover="onCanvasDragOver"
      @dragleave="onCanvasDragLeave"
      @drop="onCanvasDrop"
    >
      <VueFlow
        v-model:nodes="nodes"
        v-model:edges="edges"
        :node-types="nodeTypes"
        :edge-types="edgeTypes"
        :default-edge-options="defaultEdgeOptions"
        :snap-to-grid="true"
        :snap-grid="[20, 20]"
        :fit-view-on-init="true"
        :nodes-draggable="!isCanvasReadOnly"
        :nodes-connectable="!isCanvasReadOnly"
        :elements-selectable="!isCanvasReadOnly"
        :edges-updatable="!isCanvasReadOnly"
        :selection-key-code="'Shift'"
        :multi-selection-key-code="['Meta', 'Control']"
        class="flow-canvas"
        @node-click="onNodeClick"
        @node-double-click="onNodeDblClick"
        @pane-click="onPaneClick"
        @selection-change="onSelectionChange"
        @node-context-menu="onNodeContextMenu"
        @edge-context-menu="onEdgeContextMenu"
        @edge-update="({ edge, connection }) => onEdgeUpdateHandler(edge, connection)"
      >
        <Background pattern-color="var(--ordo-border-light)" :gap="20" />
        <Controls />
        <MiniMap />
      </VueFlow>

      <div
        v-if="nodeDragPreview"
        class="node-drag-preview"
        :class="`type-${nodeDragPreview.type}`"
        :style="{
          transform: `translate(${nodeDragPreview.x}px, ${nodeDragPreview.y}px)`,
        }"
      >
        <div class="node-drag-preview-header">
          <OrdoIcon :name="nodeDragPreview.type" :size="14" class="node-drag-preview-icon" />
          <span class="node-drag-preview-title">{{ nodeDragPreviewLabel }}</span>
          <span class="node-drag-preview-badge">{{ nodeDragPreviewTypeLabel }}</span>
        </div>
      </div>

      <!-- Context Menu -->
      <div
        v-if="showContextMenu && !isCanvasReadOnly"
        class="context-menu"
        :style="{ left: contextMenuPosition.x + 'px', top: contextMenuPosition.y + 'px' }"
        @click.stop
        @mousedown.stop
      >
        <!-- Extract sub-rule -->
        <div
          v-if="selectedNodeIds.length > 0"
          class="context-menu-item"
          :class="{ 'is-disabled': !extractSubRuleEligibility.valid }"
          :title="extractSubRuleEligibility.reason"
          @click="extractSubRuleFromSelection()"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path d="M6 4h12v6H6z" />
            <path d="M6 14h12v6H6z" />
            <path d="M12 10v4" />
          </svg>
          <span>{{ t('flow.extractSubRule') }}</span>
          <span class="shortcut" v-if="extractSubRuleEligibility.valid">{{
            extractSubRuleEligibility.selectedNodes.length
          }}</span>
        </div>

        <div class="context-menu-divider" v-if="selectedNodeIds.length > 0"></div>

        <!-- Group creation (only when multiple nodes selected) -->
        <div
          v-if="selectedNodeIds.length > 1"
          class="context-menu-item"
          @click="createGroupFromSelection"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="4 2" />
          </svg>
          <span>{{ t('flow.createGroup') }}</span>
          <span class="shortcut">{{ selectedNodeIds.length }}</span>
        </div>

        <!-- Set as start (only for single step node) -->
        <div
          v-if="selectedNodeIds.length === 1 && selectedStepNode"
          class="context-menu-item"
          @click="setAsStartFromMenu"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <polygon points="5 3 19 12 5 21 5 3" />
          </svg>
          <span>{{ t('flow.setAsStart') }}</span>
        </div>

        <!-- Duplicate node -->
        <div
          v-if="selectedNodeIds.length === 1 && selectedStepNode"
          class="context-menu-item"
          @click="duplicateSelectedNode"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <rect x="9" y="9" width="13" height="13" rx="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </svg>
          <span>{{ t('flow.duplicate') }}</span>
        </div>

        <div class="context-menu-divider" v-if="selectedNodeIds.length > 0"></div>

        <!-- Delete nodes -->
        <div
          v-if="selectedNodeIds.length > 0"
          class="context-menu-item danger"
          @click="deleteFromMenu"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <polyline points="3 6 5 6 21 6" />
            <path
              d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
            />
          </svg>
          <span>{{ t('common.delete') }}</span>
          <span class="shortcut" v-if="selectedNodeIds.length > 1">{{
            selectedNodeIds.length
          }}</span>
        </div>

        <!-- Edge context menu items -->
        <template v-if="selectedEdgeId">
          <div class="context-menu-item" @click="reverseSelectedEdge">
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="17 1 21 5 17 9" />
              <path d="M3 11V9a4 4 0 0 1 4-4h14" />
              <polyline points="7 23 3 19 7 15" />
              <path d="M21 13v2a4 4 0 0 1-4 4H3" />
            </svg>
            <span>{{ t('flow.reverseEdge') }}</span>
          </div>

          <div class="context-menu-divider"></div>

          <div class="context-menu-item danger" @click="deleteSelectedEdge">
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="3 6 5 6 21 6" />
              <path
                d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
              />
            </svg>
            <span>{{ t('flow.deleteEdge') }}</span>
          </div>
        </template>
      </div>
    </div>

    <!-- Property Panel for Step Nodes -->
    <OrdoFlowPropertyPanel
      v-if="selectedStepNode && !isCanvasReadOnly"
      :node="selectedStepNode"
      :available-steps="modelValue.steps"
      :available-sub-rules="modelValue.subRules ?? {}"
      :managed-sub-rules="managedSubRules"
      :suggestions="suggestions"
      :disabled="disabled"
      @update="updateNode"
      @set-start="setAsStart"
      @delete="deleteSelectedNode"
      @close="selectedNodeId = null"
      @open-sub-rule="(name: string) => emit('open-sub-rule', name)"
    />

    <!-- Property Panel for Group Nodes -->
    <div v-if="selectedGroupNode && !isCanvasReadOnly" class="group-property-panel">
      <div class="panel-header">
        <div class="header-title">
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="4 2" />
          </svg>
          <span class="type-label">{{ t('flow.group') }}</span>
        </div>
        <button class="close-btn" @click="selectedNodeId = null" :title="t('common.close')">
          <svg
            width="14"
            height="14"
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
        <div class="form-row">
          <label>{{ t('common.name') }}</label>
          <input
            type="text"
            :value="selectedGroupNode.data?.group?.name"
            @input="updateGroupName(($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="form-row">
          <label>{{ t('flow.stepsInGroup') }}</label>
          <div class="step-count">
            {{ selectedGroupNode.data?.group?.stepIds?.length || 0 }} {{ t('flow.steps') }}
          </div>
        </div>
        <div class="panel-actions">
          <button class="action-btn danger" @click="deleteSelectedNode">
            <OrdoIcon name="delete" :size="14" />
            {{ t('flow.deleteGroup') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ordo-flow-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  background: var(--ordo-bg-app);
  position: relative;
}

.flow-canvas-container {
  flex: 1;
  position: relative;
  overflow: hidden;
}

.flow-canvas {
  width: 100%;
  height: 100%;
}

.node-drag-preview {
  position: absolute;
  top: 0;
  left: 0;
  min-width: 180px;
  max-width: 220px;
  border: 2px dashed var(--ordo-border-color);
  border-top-width: 3px;
  border-radius: 6px;
  background: color-mix(in srgb, var(--ordo-bg-item) 88%, transparent);
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.22);
  pointer-events: none;
  z-index: 20;
  opacity: 0.95;
}

.node-drag-preview.type-decision {
  border-color: var(--ordo-node-decision, #b76e00);
}

.node-drag-preview.type-action {
  border-color: var(--ordo-node-action, #0066b8);
}

.node-drag-preview.type-terminal {
  border-color: var(--ordo-node-terminal, #388a34);
}

.node-drag-preview.type-sub_rule {
  border-color: var(--ordo-node-sub-rule, #5b708a);
}

.node-drag-preview-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
}

.node-drag-preview-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  flex: 1;
}

.node-drag-preview-badge {
  font-size: 9px;
  color: var(--ordo-text-tertiary);
  background: var(--ordo-bg-panel);
  padding: 2px 5px;
  border-radius: 999px;
}

.node-drag-preview-icon {
  flex-shrink: 0;
}

.node-drag-preview.type-decision .node-drag-preview-icon {
  color: var(--ordo-node-decision, #b76e00);
}

.node-drag-preview.type-action .node-drag-preview-icon {
  color: var(--ordo-node-action, #0066b8);
}

.node-drag-preview.type-terminal .node-drag-preview-icon {
  color: var(--ordo-node-terminal, #388a34);
}

.node-drag-preview.type-sub_rule .node-drag-preview-icon {
  color: var(--ordo-node-sub-rule, #5b708a);
}

/* Vue Flow overrides */
:deep(.vue-flow__minimap) {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: 4px;
}

:deep(.vue-flow__controls) {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: 4px;
  box-shadow: none;
}

:deep(.vue-flow__controls-button) {
  background: var(--ordo-bg-item);
  border-color: var(--ordo-border-color);
  color: var(--ordo-text-secondary);
}

:deep(.vue-flow__controls-button:hover) {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

:deep(.vue-flow__edge-path) {
  stroke: var(--ordo-border-color);
  stroke-width: 2;
}

:deep(.vue-flow__edge.selected .vue-flow__edge-path) {
  stroke: var(--ordo-accent);
}

:deep(.vue-flow__edge-text) {
  font-size: 10px;
  fill: var(--ordo-text-tertiary);
}

:deep(.vue-flow__background) {
  background: var(--ordo-bg-editor);
}

/* Group property panel */
.group-property-panel {
  position: absolute;
  top: 0;
  right: 0;
  width: 280px;
  height: 100%;
  background: var(--ordo-bg-panel);
  border-left: 1px solid var(--ordo-border-color);
  display: flex;
  flex-direction: column;
  z-index: 100;
  box-shadow: -4px 0 12px rgba(0, 0, 0, 0.08);
}

.group-property-panel .panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-item);
}

.group-property-panel .header-title {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--ordo-text-tertiary);
}

.group-property-panel .type-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.group-property-panel .close-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  padding: 4px;
  border-radius: var(--ordo-radius-sm);
}

.group-property-panel .close-btn:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.group-property-panel .panel-content {
  flex: 1;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.group-property-panel .form-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.group-property-panel .form-row label {
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-tertiary);
  text-transform: uppercase;
}

.group-property-panel .form-row input {
  padding: 8px 10px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-item);
  color: var(--ordo-text-primary);
  font-size: 13px;
}

.group-property-panel .form-row input:focus {
  outline: none;
  border-color: var(--ordo-accent);
}

.group-property-panel .step-count {
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.group-property-panel .panel-actions {
  margin-top: auto;
  padding-top: 12px;
  border-top: 1px solid var(--ordo-border-light);
}

.group-property-panel .action-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 8px 12px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-item);
  color: var(--ordo-text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s;
  width: 100%;
  justify-content: center;
}

.group-property-panel .action-btn.danger:hover {
  background: var(--ordo-error-bg, rgba(229, 20, 0, 0.1));
  color: var(--ordo-error);
}

/* Context Menu */
.context-menu {
  position: fixed;
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md, 6px);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  z-index: 1000;
  min-width: 200px;
  padding: 4px;
}

.context-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: var(--ordo-radius-sm, 4px);
  cursor: pointer;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  transition: all 0.15s;
}

.context-menu-item:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.context-menu-item.is-disabled {
  cursor: not-allowed;
  opacity: 0.48;
}

.context-menu-item.is-disabled:hover {
  background: transparent;
  color: var(--ordo-text-secondary);
}

.context-menu-item svg {
  color: var(--ordo-text-tertiary);
}

.context-menu-item .shortcut {
  margin-left: auto;
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  background: var(--ordo-bg-item);
  padding: 2px 6px;
  border-radius: 3px;
}

.context-menu-item.danger {
  color: var(--ordo-error);
}

.context-menu-item.danger:hover {
  background: var(--ordo-error-bg, rgba(229, 20, 0, 0.1));
}

.context-menu-item.danger svg {
  color: var(--ordo-error);
}

.context-menu-divider {
  height: 1px;
  background: var(--ordo-border-light);
  margin: 4px 8px;
}

/* Edge update styles */
:deep(.vue-flow__edge.updating) {
  stroke: var(--ordo-accent);
}

:deep(.vue-flow__edgeupdater) {
  cursor: move;
}

:deep(.vue-flow__edgeupdater-source),
:deep(.vue-flow__edgeupdater-target) {
  fill: var(--ordo-accent);
  stroke: var(--ordo-bg-panel);
  stroke-width: 2;
  r: 6; /* Smaller radius for edge updater handles */
}

/* Node handle (connection point) styles - use custom pin styles */
:deep(.vue-flow__handle) {
  /* Let the custom pin component handle styling */
  width: auto !important;
  height: auto !important;
  min-width: 0 !important;
  min-height: 0 !important;
  background: transparent !important;
  border: none !important;
  border-radius: 0 !important;
}

/* vue-flow__handle:hover - no transform needed, handled by pin component */

:deep(.vue-flow__handle.connectable) {
  cursor: crosshair;
}

/* If using default handles without custom pins */
:deep(.vue-flow__handle:not(.pin)) {
  width: 10px !important;
  height: 10px !important;
  background: var(--ordo-accent) !important;
  border: 2px solid var(--ordo-bg-panel) !important;
  border-radius: 50% !important;
}

:deep(.vue-flow__handle:not(.pin):hover) {
  transform: scale(1.15);
}

/* Edge updater handles - small circles at edge endpoints */
:deep(.vue-flow__edgeupdater) {
  width: 12px;
  height: 12px;
}

:deep(.vue-flow__edgeupdater circle) {
  r: 5;
}

/* ============================================ */
/* Path Highlighting Styles */
/* ============================================ */

/* Highlighted nodes - stand out */
:deep(.vue-flow__node.path-highlighted) {
  filter: drop-shadow(0 0 8px rgba(74, 158, 255, 0.6));
  z-index: 10 !important;
}

/* Dimmed nodes - fade to background */
:deep(.vue-flow__node.path-dimmed) {
  opacity: 0.3;
  filter: grayscale(0.5);
  transition:
    opacity 0.2s ease,
    filter 0.2s ease;
}

/* Highlighted edges - bright and visible */
:deep(.vue-flow__edge.path-highlighted path) {
  stroke-width: 3 !important;
  filter: drop-shadow(0 0 4px rgba(74, 158, 255, 0.8));
}

:deep(.vue-flow__edge.path-highlighted polygon) {
  filter: drop-shadow(0 0 4px rgba(74, 158, 255, 0.8));
}

/* Dimmed edges - fade to background */
:deep(.vue-flow__edge.path-dimmed path) {
  opacity: 0.15;
  transition: opacity 0.2s ease;
}

:deep(.vue-flow__edge.path-dimmed polygon) {
  opacity: 0.15;
  transition: opacity 0.2s ease;
}

:deep(.vue-flow__edge.path-dimmed .edge-label-bg),
:deep(.vue-flow__edge.path-dimmed .edge-label-text) {
  opacity: 0.15;
}

/* ============================================ */
/* Execution Trace Overlay Styles */
/* ============================================ */

/* Executed nodes - green glow with pulse animation */
:deep(.vue-flow__node.execution-highlighted) {
  filter: drop-shadow(0 0 12px rgba(78, 201, 105, 0.7));
  z-index: 10 !important;
  animation: execution-pulse 2s ease-in-out infinite;
}

@keyframes execution-pulse {
  0%,
  100% {
    filter: drop-shadow(0 0 8px rgba(78, 201, 105, 0.5));
  }
  50% {
    filter: drop-shadow(0 0 16px rgba(78, 201, 105, 0.9));
  }
}

:deep(.vue-flow__node.execution-highlighted .flow-node) {
  border-color: var(--ordo-success, #4ec969) !important;
  transition: border-color 0.3s ease;
}

/* Non-executed nodes - dimmed */
:deep(.vue-flow__node.execution-dimmed) {
  opacity: 0.25;
  filter: grayscale(0.7);
  transition:
    opacity 0.3s ease,
    filter 0.3s ease;
}

/* Executed edges - green and animated with flowing dash */
:deep(.vue-flow__edge.execution-highlighted path) {
  stroke: var(--ordo-success, #4ec969) !important;
  stroke-width: 3 !important;
  filter: drop-shadow(0 0 6px rgba(78, 201, 105, 0.8));
  stroke-dasharray: 8 4;
  animation: execution-flow 0.8s linear infinite;
}

@keyframes execution-flow {
  0% {
    stroke-dashoffset: 24;
  }
  100% {
    stroke-dashoffset: 0;
  }
}

:deep(.vue-flow__edge.execution-highlighted polygon) {
  fill: var(--ordo-success, #4ec969) !important;
  filter: drop-shadow(0 0 6px rgba(78, 201, 105, 0.8));
}

/* Non-executed edges - dimmed */
:deep(.vue-flow__edge.execution-dimmed path) {
  opacity: 0.1;
  transition: opacity 0.3s ease;
}

:deep(.vue-flow__edge.execution-dimmed polygon) {
  opacity: 0.1;
  transition: opacity 0.3s ease;
}

:deep(.vue-flow__edge.execution-dimmed .edge-label-bg),
:deep(.vue-flow__edge.execution-dimmed .edge-label-text) {
  opacity: 0.1;
}

/* Trace-only sub-rule expansion */
:deep(.vue-flow__node.trace-expanded-subrule) {
  z-index: 8 !important;
}

:deep(.vue-flow__node.trace-expanded-subrule .flow-node) {
  border-style: dashed;
  background: linear-gradient(180deg, rgba(91, 112, 138, 0.08), rgba(91, 112, 138, 0.02)),
    var(--ordo-bg-item, #1e1e1e);
}

:deep(.vue-flow__node.trace-expanded-subrule::after) {
  content: 'sub trace';
  position: absolute;
  top: -18px;
  left: 8px;
  padding: 2px 7px;
  border: 1px solid rgba(91, 112, 138, 0.28);
  border-radius: 999px;
  background: var(--ordo-bg-panel, #252526);
  color: var(--ordo-text-tertiary, #8a8f98);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.02em;
  pointer-events: none;
}

:deep(.vue-flow__edge.trace-subrule-bridge path),
:deep(.vue-flow__edge.trace-expanded-subrule-edge path) {
  stroke-dasharray: 7 5;
}

:deep(.vue-flow__edge.trace-subrule-bridge path) {
  stroke: var(--ordo-node-sub-rule, #5b708a) !important;
  stroke-width: 2.5 !important;
}

/* Entry node special styling */
:deep(.vue-flow__node.execution-entry) {
  animation: execution-entry-pulse 1.5s ease-in-out infinite;
}

@keyframes execution-entry-pulse {
  0%,
  100% {
    filter: drop-shadow(0 0 12px rgba(137, 180, 250, 0.6));
  }
  50% {
    filter: drop-shadow(0 0 20px rgba(137, 180, 250, 1));
  }
}

/* Terminal node special styling */
:deep(.vue-flow__node.execution-terminal) {
  animation: execution-terminal-pulse 1.5s ease-in-out infinite;
}

@keyframes execution-terminal-pulse {
  0%,
  100% {
    filter: drop-shadow(0 0 12px rgba(166, 227, 161, 0.6));
  }
  50% {
    filter: drop-shadow(0 0 20px rgba(166, 227, 161, 1));
  }
}
</style>

<style>
/* Import Vue Flow styles */
@import '@vue-flow/core/dist/style.css';
@import '@vue-flow/core/dist/theme-default.css';
@import '@vue-flow/controls/dist/style.css';
@import '@vue-flow/minimap/dist/style.css';
</style>
