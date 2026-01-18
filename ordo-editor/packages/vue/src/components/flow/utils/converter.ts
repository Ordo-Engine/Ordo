/**
 * RuleSet <-> Vue Flow data converter
 * 规则集与 Vue Flow 数据格式双向转换
 */

import type { Node, Edge } from '@vue-flow/core';
import type { RuleSet, Step, DecisionStep, RuleSetConfig, StepGroup } from '@ordo-engine/editor-core';
import { conditionToString, StepGroup as StepGroupFactory, generateId } from '@ordo-engine/editor-core';
import { EDGE_COLORS } from '../types';

/** Node types for Vue Flow */
export type FlowNodeType = 'decision' | 'action' | 'terminal' | 'group';

/** Edge types */
export type FlowEdgeType = 'exec' | 'exec-branch' | 'data';

/** Custom node data for step nodes */
export interface FlowNodeData {
  step: Step;
  isStart: boolean;
  label: string;
}

/** Custom node data for group nodes */
export interface FlowGroupNodeData {
  group: StepGroup;
  groupId: string;
  name: string;
  description?: string;
  color?: string;
  stepIds: string[];
}

/** Flow node with typed data */
export type FlowNode = Node<FlowNodeData, any, FlowNodeType>;

/** Flow group node */
export type FlowGroupNode = Node<FlowGroupNodeData, any, 'group'>;

/** Edge data with type info */
export interface FlowEdgeData {
  branchId?: string;
  isDefault?: boolean;
  edgeType: FlowEdgeType;
  condition?: string; // Full condition expression for tooltip
}

/** Flow edge with typed data */
export type FlowEdge = Edge<FlowEdgeData>;

/** Conversion result */
export interface FlowData {
  nodes: FlowNode[];
  edges: FlowEdge[];
  groups: FlowGroupNode[];
}

/**
 * Get edge style based on type
 * 根据类型获取边的样式
 */
export function getEdgeStyle(edgeType: FlowEdgeType): { stroke: string; strokeWidth: number } {
  switch (edgeType) {
    case 'exec-branch':
      return { stroke: EDGE_COLORS.execBranch, strokeWidth: 2 };
    case 'data':
      return { stroke: EDGE_COLORS.data, strokeWidth: 1.5 };
    case 'exec':
    default:
      return { stroke: EDGE_COLORS.exec, strokeWidth: 2 };
  }
}

/**
 * Convert RuleSet to Vue Flow nodes and edges
 * 将 RuleSet 转换为 Vue Flow 节点和边
 */
export function rulesetToFlow(ruleset: RuleSet): FlowData {
  const nodes: FlowNode[] = [];
  const edges: FlowEdge[] = [];
  const groups: FlowGroupNode[] = [];

  // Convert steps to nodes
  for (const step of ruleset.steps) {
    const node: FlowNode = {
      id: step.id,
      type: step.type,
      position: step.position || { x: 0, y: 0 },
      data: {
        step,
        isStart: step.id === ruleset.startStepId,
        label: step.name,
      },
    };
    nodes.push(node);

    // Create edges based on step type
    switch (step.type) {
      case 'decision': {
        const decisionStep = step as DecisionStep;

        // Edges for each branch
        for (const branch of decisionStep.branches) {
          if (branch.nextStepId) {
            // Get condition string for tooltip
            const conditionStr = branch.condition ? conditionToString(branch.condition) : undefined;

            const edgeStyle = getEdgeStyle('exec-branch');
            edges.push({
              id: `${step.id}-${branch.id}`,
              source: step.id,
              target: branch.nextStepId,
              sourceHandle: branch.id,
              targetHandle: 'input',
              // label is now handled dynamically in OrdoEdge component
              style: edgeStyle,
              data: {
                branchId: branch.id,
                isDefault: false,
                edgeType: 'exec-branch',
                condition: conditionStr,
              },
            });
          }
        }

        // Default edge
        if (decisionStep.defaultNextStepId) {
          const edgeStyle = getEdgeStyle('exec');
          edges.push({
            id: `${step.id}-default`,
            source: step.id,
            target: decisionStep.defaultNextStepId,
            sourceHandle: 'default',
            targetHandle: 'input',
            // label is now handled dynamically in OrdoEdge component
            style: edgeStyle,
            data: {
              isDefault: true,
              edgeType: 'exec',
            },
          });
        }
        break;
      }

      case 'action': {
        if (step.nextStepId) {
          const edgeStyle = getEdgeStyle('exec');
          edges.push({
            id: `${step.id}-next`,
            source: step.id,
            target: step.nextStepId,
            sourceHandle: 'output',
            targetHandle: 'input',
            style: edgeStyle,
            data: {
              edgeType: 'exec',
            },
          });
        }
        break;
      }

      case 'terminal':
        // Terminal nodes have no outgoing edges
        break;
    }
  }

  // Convert groups to group nodes (as independent background nodes)
  if (ruleset.groups) {
    for (const group of ruleset.groups) {
      // Parse size from stored values (may be string or number)
      const width =
        typeof group.size.width === 'string' ? parseInt(group.size.width, 10) : group.size.width;
      const height =
        typeof group.size.height === 'string' ? parseInt(group.size.height, 10) : group.size.height;

      const groupNode: FlowGroupNode = {
        id: `group-${group.id}`, // Prefix to avoid ID collision with steps
        type: 'group',
        position: group.position,
        data: {
          group,
          groupId: group.id,
          name: group.name,
          description: group.description,
          color: group.color,
          stepIds: group.stepIds,
        },
        style: {
          width: `${width}px`,
          height: `${height}px`,
        },
        zIndex: -1000, // Keep at background
        selectable: true,
        // NOTE: Do NOT set draggable here - let Vue Flow's global nodes-draggable control it
        connectable: false,
      };
      groups.push(groupNode);
    }
  }

  return { nodes, edges, groups };
}

/**
 * Convert Vue Flow nodes and edges back to RuleSet
 * 将 Vue Flow 节点和边转换回 RuleSet
 */
export function flowToRuleset(
  nodes: FlowNode[],
  edges: FlowEdge[],
  config: RuleSetConfig,
  startStepId: string,
  groupNodes?: FlowGroupNode[]
): RuleSet {
  const steps: Step[] = [];
  const groups: StepGroup[] = [];

  // Process group nodes first
  if (groupNodes) {
    for (const groupNode of groupNodes) {
      if (!groupNode.data?.group) continue;

      // Parse width/height from style (may be "300px" format)
      let width = groupNode.data.group.size.width;
      let height = groupNode.data.group.size.height;

      if (groupNode.style) {
        const styleWidth = (groupNode.style as any)?.width;
        const styleHeight = (groupNode.style as any)?.height;

        if (typeof styleWidth === 'string') {
          width = parseInt(styleWidth, 10) || width;
        } else if (typeof styleWidth === 'number') {
          width = styleWidth;
        }

        if (typeof styleHeight === 'string') {
          height = parseInt(styleHeight, 10) || height;
        } else if (typeof styleHeight === 'number') {
          height = styleHeight;
        }
      }

      const group: StepGroup = {
        ...groupNode.data.group,
        position: { x: groupNode.position.x, y: groupNode.position.y },
        size: { width, height },
        // Keep original stepIds (we don't use parentNode anymore)
      };
      groups.push(group);
    }
  }

  for (const node of nodes) {
    // Get the step data from node
    if (!node.data) continue;

    const step = { ...node.data.step };

    // Update position
    step.position = { x: node.position.x, y: node.position.y };

    // Update connections based on edges
    switch (step.type) {
      case 'decision': {
        const decisionStep = step as DecisionStep;
        const outgoingEdges = edges.filter((e) => e.source === node.id);

        // Update branch targets
        for (const branch of decisionStep.branches) {
          const branchEdge = outgoingEdges.find((e) => e.data?.branchId === branch.id);
          if (branchEdge) {
            branch.nextStepId = branchEdge.target;
          }
        }

        // Update default target
        const defaultEdge = outgoingEdges.find((e) => e.data?.isDefault);
        if (defaultEdge) {
          decisionStep.defaultNextStepId = defaultEdge.target;
        }
        break;
      }

      case 'action': {
        const outgoingEdge = edges.find((e) => e.source === node.id);
        if (outgoingEdge) {
          step.nextStepId = outgoingEdge.target;
        }
        break;
      }

      case 'terminal':
        // No connections to update
        break;
    }

    steps.push(step);
  }

  return {
    config,
    startStepId,
    steps,
    groups: groups.length > 0 ? groups : undefined,
    metadata: {
      updatedAt: new Date().toISOString(),
    },
  };
}

/**
 * Find the start node from nodes
 * 从节点中找到起始节点
 */
export function findStartNode(nodes: FlowNode[]): FlowNode | undefined {
  return nodes.find((n) => n.data?.isStart);
}

/**
 * Update start step in nodes
 * 更新节点中的起始步骤
 */
export function updateStartStep(nodes: FlowNode[], newStartId: string): FlowNode[] {
  return nodes.map((node) => {
    if (!node.data) return node;
    return {
      ...node,
      data: {
        ...node.data,
        isStart: node.id === newStartId,
      },
    };
  });
}

/**
 * Create a new node from step
 * 从步骤创建新节点
 */
export function createNodeFromStep(
  step: Step,
  position: { x: number; y: number },
  isStart = false
): FlowNode {
  return {
    id: step.id,
    type: step.type,
    position,
    data: {
      step,
      isStart,
      label: step.name,
    },
  };
}

/**
 * Create a new group node
 * 创建新的分组节点
 */
export function createGroupNode(
  name: string,
  position: { x: number; y: number },
  size: { width: number; height: number } = { width: 300, height: 200 },
  color?: string
): FlowGroupNode {
  const group = StepGroupFactory.create({
    id: generateId('group'),
    name,
    color,
    position,
    size,
  });

  return {
    id: group.id,
    type: 'group',
    position: group.position,
    data: {
      group,
      groupId: group.id,
      name: group.name,
      description: group.description,
      color: group.color,
      stepIds: group.stepIds,
    },
    style: {
      width: size.width,
      height: size.height,
    },
  };
}

/**
 * Create edge between two nodes
 * 在两个节点之间创建边
 */
export function createEdge(
  sourceId: string,
  targetId: string,
  options?: {
    label?: string;
    branchId?: string;
    isDefault?: boolean;
    sourceHandle?: string;
    targetHandle?: string;
    condition?: string;
  }
): FlowEdge {
  const edgeId = options?.branchId
    ? `${sourceId}-${options.branchId}`
    : options?.isDefault
      ? `${sourceId}-default`
      : `${sourceId}-next`;

  // Determine edge type
  const edgeType: FlowEdgeType = options?.branchId ? 'exec-branch' : 'exec';

  const edgeStyle = getEdgeStyle(edgeType);

  return {
    id: edgeId,
    source: sourceId,
    target: targetId,
    sourceHandle:
      options?.sourceHandle || options?.branchId || (options?.isDefault ? 'default' : 'output'),
    targetHandle: options?.targetHandle || 'input',
    label: options?.label,
    style: edgeStyle,
    data: {
      branchId: options?.branchId,
      isDefault: options?.isDefault,
      edgeType,
      condition: options?.condition,
    },
  };
}
