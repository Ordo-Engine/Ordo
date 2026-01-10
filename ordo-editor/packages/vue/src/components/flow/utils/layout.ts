/**
 * Auto layout utilities for group-based flow layout
 * 基于分组的流程布局工具
 * 
 * Layout Strategy (v2 - Intelligent Topology-based):
 * 1. Groups are arranged horizontally (left to right)
 * 2. Nodes within each group are arranged based on topological depth
 * 3. Same-depth nodes are arranged vertically
 * 4. Cross-group edge targets are ordered to minimize crossing
 */

import type { FlowNode, FlowEdge } from './converter';
import type { StepGroup } from '@ordo/editor-core';

/** Layout direction */
export type LayoutDirection = 'TB' | 'LR' | 'BT' | 'RL';

/** Layout options */
export interface LayoutOptions {
  /** Layout direction (TB = top-bottom, LR = left-right) */
  direction?: LayoutDirection;
  /** Horizontal spacing between groups */
  groupSpacingX?: number;
  /** Vertical spacing between nodes within a group */
  nodeSpacingY?: number;
  /** Horizontal spacing between depth layers within a group */
  layerSpacingX?: number;
  /** Padding inside group boxes */
  groupPadding?: number;
  /** Starting X position */
  startX?: number;
  /** Starting Y position */
  startY?: number;
}

const DEFAULT_OPTIONS: Required<LayoutOptions> = {
  direction: 'LR',
  groupSpacingX: 180,   // Gap between groups (more space for cross-group edges)
  nodeSpacingY: 60,     // Gap between nodes within a group (room for edge routing)
  layerSpacingX: 120,   // Gap between depth layers (clearer branch visualization)
  groupPadding: 70,     // Padding inside group box (comfortable margins)
  startX: 60,
  startY: 60,
};

/** Node dimensions */
interface NodeDimensions {
  width: number;
  height: number;
}

/** Node with layout info */
interface NodeLayoutInfo {
  node: FlowNode;
  width: number;
  height: number;
  depth: number;        // Topological depth within group
  order: number;        // Order within depth layer
  x: number;
  y: number;
}

/**
 * Calculate node height based on its content
 */
function calculateNodeHeight(node: FlowNode): number {
  const baseHeight = 70;    // Increased base height for header and padding
  const rowHeight = 26;     // Slightly taller rows for readability
  const minHeight = 100;    // Minimum node height
  
  if (!node.data?.step) {
    return 200; // Group node default
  }
  
  const step = node.data.step;
  let contentRows = 0;
  
  switch (step.type) {
    case 'decision':
      // Each branch takes one row, plus header
      contentRows = (step.branches?.length || 0) + 1;
      break;
    case 'action':
      // Assignments and optional logging
      contentRows = (step.assignments?.length || 0);
      if (step.logging) contentRows++;
      contentRows = Math.max(contentRows, 2);
      break;
    case 'terminal':
      // Output fields plus return indicator
      contentRows = (step.output?.length || 0) + 1;
      break;
    default:
      contentRows = 2;
  }
  
  return Math.max(minHeight, baseHeight + contentRows * rowHeight);
}

/**
 * Calculate node width based on its type
 */
function calculateNodeWidth(node: FlowNode): number {
  if (!node.data?.step) {
    return 300; // Group node
  }
  
  const step = node.data.step;
  
  switch (step.type) {
    case 'decision':
      return 220;  // Decision nodes need more space for branch labels
    case 'action':
      return 240;  // Action nodes need more space for assignments
    case 'terminal':
      return 220;  // Terminal nodes for output display
    default:
      return 220;
  }
}

/**
 * Get node dimensions
 */
function getNodeDimensions(node: FlowNode): NodeDimensions {
  return {
    width: calculateNodeWidth(node),
    height: calculateNodeHeight(node),
  };
}

/**
 * Build adjacency list from edges
 */
function buildAdjacencyList(
  edges: FlowEdge[],
  nodeIds: Set<string>
): { outgoing: Map<string, string[]>; incoming: Map<string, string[]> } {
  const outgoing = new Map<string, string[]>();
  const incoming = new Map<string, string[]>();
  
  // Initialize
  for (const nodeId of nodeIds) {
    outgoing.set(nodeId, []);
    incoming.set(nodeId, []);
  }
  
  // Build adjacency
  for (const edge of edges) {
    if (nodeIds.has(edge.source) && nodeIds.has(edge.target)) {
      outgoing.get(edge.source)!.push(edge.target);
      incoming.get(edge.target)!.push(edge.source);
    }
  }
  
  return { outgoing, incoming };
}

/**
 * Calculate topological depth using BFS from entry nodes
 */
function calculateDepths(
  nodeIds: string[],
  outgoing: Map<string, string[]>,
  incoming: Map<string, string[]>,
  startNodeId?: string
): Map<string, number> {
  const depths = new Map<string, number>();
  
  // Find entry nodes (nodes with no incoming edges within group)
  let entryNodes = nodeIds.filter(id => incoming.get(id)!.length === 0);
  
  // If we have a known start node and it's in this group, prioritize it
  if (startNodeId && nodeIds.includes(startNodeId)) {
    entryNodes = [startNodeId, ...entryNodes.filter(id => id !== startNodeId)];
  }
  
  // If no entry nodes (cycle), use all nodes
  if (entryNodes.length === 0) {
    entryNodes = [...nodeIds];
  }
  
  // BFS to calculate depths
  const visited = new Set<string>();
  const queue: Array<{ id: string; depth: number }> = [];
  
  for (const entryId of entryNodes) {
    queue.push({ id: entryId, depth: 0 });
  }
  
  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;
    
    if (visited.has(id)) {
      // Update depth if we found a longer path
      depths.set(id, Math.max(depths.get(id) || 0, depth));
      continue;
    }
    
    visited.add(id);
    depths.set(id, depth);
    
    // Add children
    for (const targetId of outgoing.get(id) || []) {
      if (nodeIds.includes(targetId)) {
        queue.push({ id: targetId, depth: depth + 1 });
      }
    }
  }
  
  // Handle unvisited nodes (disconnected)
  for (const id of nodeIds) {
    if (!depths.has(id)) {
      depths.set(id, 0);
    }
  }
  
  return depths;
}

/**
 * Get branch order from edge sourceHandle
 * Decision nodes have branches ordered top-to-bottom, we want to preserve that order
 */
function getBranchOrder(edge: FlowEdge, nodeMap: Map<string, FlowNode>): number {
  const sourceNode = nodeMap.get(edge.source);
  if (!sourceNode?.data?.step) return 500;
  
  const step = sourceNode.data.step;
  if (step.type !== 'decision') {
    // Action nodes - "next" output, give middle priority
    return 500;
  }
  
  // Decision node - find branch index
  const branches = step.branches || [];
  const handleId = edge.sourceHandle;
  
  if (handleId === 'default') {
    // Default branch goes last
    return branches.length * 100 + 100;
  }
  
  // Find the branch index
  const branchIndex = branches.findIndex(b => b.id === handleId);
  if (branchIndex !== -1) {
    return branchIndex * 100;
  }
  
  return 500; // Unknown handle
}

/**
 * Order nodes at same depth to minimize edge crossings
 * Key insight: nodes that are targets of branches from the same decision node
 * should be ordered according to their branch order (top-to-bottom)
 */
function optimizeNodeOrder(
  nodesAtDepth: Map<number, string[]>,
  edges: FlowEdge[],
  nodeIdSet: Set<string>,
  nodeMap: Map<string, FlowNode>
): Map<number, string[]> {
  const result = new Map<number, string[]>();
  const depths = Array.from(nodesAtDepth.keys()).sort((a, b) => a - b);
  
  if (depths.length === 0) return result;
  
  // Keep first depth as-is
  result.set(depths[0], nodesAtDepth.get(depths[0])!);
  
  // For each subsequent depth, order nodes based on parent positions AND branch order
  for (let i = 1; i < depths.length; i++) {
    const currentDepth = depths[i];
    const prevDepth = depths[i - 1];
    const prevNodes = result.get(prevDepth)!;
    const currentNodes = [...nodesAtDepth.get(currentDepth)!];
    
    // Build a map of node -> (parent index, branch order, edge count)
    const nodeOrderInfo = new Map<string, { 
      parentIndex: number; 
      branchOrder: number;
      parentId: string | null;
    }>();
    
    for (const nodeId of currentNodes) {
      let bestParentIndex = Infinity;
      let bestBranchOrder = Infinity;
      let bestParentId: string | null = null;
      
      // Find all incoming edges from previous depth
      for (const edge of edges) {
        if (edge.target === nodeId && nodeIdSet.has(edge.source)) {
          const parentIndex = prevNodes.indexOf(edge.source);
          if (parentIndex !== -1) {
            const branchOrder = getBranchOrder(edge, nodeMap);
            
            // Use the parent that appears earliest, and within same parent, use branch order
            if (parentIndex < bestParentIndex || 
                (parentIndex === bestParentIndex && branchOrder < bestBranchOrder)) {
              bestParentIndex = parentIndex;
              bestBranchOrder = branchOrder;
              bestParentId = edge.source;
            }
          }
        }
      }
      
      nodeOrderInfo.set(nodeId, {
        parentIndex: bestParentIndex === Infinity ? currentNodes.indexOf(nodeId) : bestParentIndex,
        branchOrder: bestBranchOrder === Infinity ? 500 : bestBranchOrder,
        parentId: bestParentId,
      });
    }
    
    // Sort: first by parent index, then by branch order within same parent
    currentNodes.sort((a, b) => {
      const infoA = nodeOrderInfo.get(a)!;
      const infoB = nodeOrderInfo.get(b)!;
      
      // If same parent, sort by branch order
      if (infoA.parentId === infoB.parentId && infoA.parentId !== null) {
        return infoA.branchOrder - infoB.branchOrder;
      }
      
      // Otherwise sort by parent index
      if (infoA.parentIndex !== infoB.parentIndex) {
        return infoA.parentIndex - infoB.parentIndex;
      }
      
      // Same parent index but different parents, use branch order as tiebreaker
      return infoA.branchOrder - infoB.branchOrder;
    });
    
    result.set(currentDepth, currentNodes);
  }
  
  return result;
}

/**
 * Order target nodes in a group based on source edge order
 * This helps minimize edge crossings for cross-group edges
 */
function orderTargetNodesBySourceEdges(
  targetGroupNodes: FlowNode[],
  edges: FlowEdge[],
  sourceGroupNodeIds: Set<string>
): FlowNode[] {
  // Build a map of target node -> list of (source node, branch index)
  const targetToSources = new Map<string, Array<{ sourceId: string; branchOrder: number }>>();
  
  for (const node of targetGroupNodes) {
    targetToSources.set(node.id, []);
  }
  
  // Find all edges from source group to target group
  for (const edge of edges) {
    if (sourceGroupNodeIds.has(edge.source) && targetToSources.has(edge.target)) {
      // Determine branch order from source handle
      let branchOrder = 0;
      if (edge.sourceHandle) {
        // For decision nodes, branches have IDs like "branch_xxx"
        // Default is always last
        if (edge.sourceHandle === 'default') {
          branchOrder = 1000; // Put default branches in the middle-ish
        } else if (edge.sourceHandle === 'output') {
          branchOrder = 500; // Action node "next" goes in middle
        } else {
          // Extract order from the edge data if available, otherwise use hash
          branchOrder = edge.sourceHandle.charCodeAt(0) * 100;
        }
      }
      
      targetToSources.get(edge.target)!.push({
        sourceId: edge.source,
        branchOrder,
      });
    }
  }
  
  // Sort target nodes by their source connections
  return [...targetGroupNodes].sort((a, b) => {
    const aSource = targetToSources.get(a.id)?.[0];
    const bSource = targetToSources.get(b.id)?.[0];
    
    if (!aSource && !bSource) return 0;
    if (!aSource) return 1;
    if (!bSource) return -1;
    
    // If from same source, order by branch order
    if (aSource.sourceId === bSource.sourceId) {
      return aSource.branchOrder - bSource.branchOrder;
    }
    
    // Otherwise keep original order
    return 0;
  });
}

/**
 * Group-based layout algorithm (v2 - Intelligent)
 * 基于分组的智能布局算法
 * 
 * This algorithm:
 * 1. Organizes nodes by their groups
 * 2. Within each group, calculates topological depth
 * 3. Arranges nodes by depth (horizontal) and optimizes vertical order
 * 4. Group boxes are sized to wrap their child nodes
 */
export function applyGroupBasedLayout(
  nodes: FlowNode[],
  edges: FlowEdge[],
  groups: StepGroup[],
  options: LayoutOptions = {}
): { nodes: FlowNode[]; groupUpdates: Partial<StepGroup>[] } {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  
  // Separate step nodes and existing group nodes
  const stepNodes = nodes.filter(n => n.type !== 'group');
  const existingGroupNodes = nodes.filter(n => n.type === 'group');
  
  if (stepNodes.length === 0) {
    return { nodes, groupUpdates: [] };
  }
  
  // Create a map of stepId -> node
  const nodeMap = new Map<string, FlowNode>();
  for (const node of stepNodes) {
    nodeMap.set(node.id, node);
  }
  
  // Create a map of stepId -> groupId
  const stepToGroup = new Map<string, string>();
  for (const group of groups) {
    for (const stepId of group.stepIds) {
      stepToGroup.set(stepId, group.id);
    }
  }
  
  // Organize nodes by group
  const groupedNodes = new Map<string, FlowNode[]>();
  const ungroupedNodes: FlowNode[] = [];
  
  // Initialize group buckets
  for (const group of groups) {
    groupedNodes.set(group.id, []);
  }
  
  // Assign nodes to groups
  for (const node of stepNodes) {
    const groupId = stepToGroup.get(node.id);
    if (groupId && groupedNodes.has(groupId)) {
      groupedNodes.get(groupId)!.push(node);
    } else {
      ungroupedNodes.push(node);
    }
  }
  
  // Find the start node for reference
  const startNode = stepNodes.find(n => n.data?.isStart);
  const startNodeId = startNode?.id;
  
  // Layout results
  const updatedNodes: FlowNode[] = [];
  const groupUpdates: Partial<StepGroup>[] = [];
  
  let currentX = opts.startX;
  let previousGroupNodeIds: Set<string> = new Set();
  
  // Layout each group
  for (let groupIndex = 0; groupIndex < groups.length; groupIndex++) {
    const group = groups[groupIndex];
    let nodesInGroup = groupedNodes.get(group.id) || [];
    
    if (nodesInGroup.length === 0) {
      // Empty group - create minimal placeholder
      groupUpdates.push({
        id: group.id,
        position: { x: currentX, y: opts.startY },
        size: { width: 200, height: 100 },
      });
      currentX += 200 + opts.groupSpacingX;
      previousGroupNodeIds = new Set();
      continue;
    }
    
    // Order nodes based on incoming edges from previous group
    if (groupIndex > 0 && previousGroupNodeIds.size > 0) {
      nodesInGroup = orderTargetNodesBySourceEdges(
        nodesInGroup, 
        edges, 
        previousGroupNodeIds
      );
    }
    
    // Build adjacency list for nodes in this group
    const nodeIds = nodesInGroup.map(n => n.id);
    const nodeIdSet = new Set(nodeIds);
    const { outgoing, incoming } = buildAdjacencyList(edges, nodeIdSet);
    
    // Calculate topological depths
    const depths = calculateDepths(nodeIds, outgoing, incoming, startNodeId);
    
    // Group nodes by depth
    const nodesAtDepth = new Map<number, string[]>();
    for (const [nodeId, depth] of depths) {
      if (!nodesAtDepth.has(depth)) {
        nodesAtDepth.set(depth, []);
      }
      nodesAtDepth.get(depth)!.push(nodeId);
    }
    
    // Calculate dimensions for each node (needed before ordering)
    const nodeLayouts = new Map<string, NodeLayoutInfo>();
    for (const node of nodesInGroup) {
      const dim = getNodeDimensions(node);
      nodeLayouts.set(node.id, {
        node,
        width: dim.width,
        height: dim.height,
        depth: depths.get(node.id) || 0,
        order: 0,
        x: 0,
        y: 0,
      });
    }
    
    // Optimize node order at each depth (now with branch-aware sorting)
    const optimizedOrder = optimizeNodeOrder(nodesAtDepth, edges, nodeIdSet, nodeMap);
    
    // Calculate layout positions
    const depthLevels = Array.from(optimizedOrder.keys()).sort((a, b) => a - b);
    
    // Calculate width needed for each depth layer
    const depthWidths = new Map<number, number>();
    for (const depth of depthLevels) {
      const nodesAtThisDepth = optimizedOrder.get(depth)!;
      let maxWidth = 0;
      for (const nodeId of nodesAtThisDepth) {
        const layout = nodeLayouts.get(nodeId)!;
        maxWidth = Math.max(maxWidth, layout.width);
      }
      depthWidths.set(depth, maxWidth);
    }
    
    // Calculate total height needed for each depth layer
    const depthHeights = new Map<number, number>();
    for (const depth of depthLevels) {
      const nodesAtThisDepth = optimizedOrder.get(depth)!;
      let totalHeight = 0;
      for (const nodeId of nodesAtThisDepth) {
        const layout = nodeLayouts.get(nodeId)!;
        totalHeight += layout.height;
      }
      totalHeight += (nodesAtThisDepth.length - 1) * opts.nodeSpacingY;
      depthHeights.set(depth, totalHeight);
    }
    
    // Find max height among all depth layers
    const maxDepthHeight = Math.max(...depthHeights.values());
    
    // Calculate starting X for each depth layer
    let layerX = currentX + opts.groupPadding;
    const depthStartX = new Map<number, number>();
    for (const depth of depthLevels) {
      depthStartX.set(depth, layerX);
      layerX += depthWidths.get(depth)! + opts.layerSpacingX;
    }
    
    // Position nodes
    const groupContentStartY = opts.startY + opts.groupPadding + 30; // +30 for group header
    
    for (const depth of depthLevels) {
      const nodesAtThisDepth = optimizedOrder.get(depth)!;
      const depthHeight = depthHeights.get(depth)!;
      const layerWidth = depthWidths.get(depth)!;
    
      
      // Center this depth layer vertically
      let nodeY = groupContentStartY + (maxDepthHeight - depthHeight) / 2;
      
      for (let i = 0; i < nodesAtThisDepth.length; i++) {
        const nodeId = nodesAtThisDepth[i];
        const layout = nodeLayouts.get(nodeId)!;
        
        // Center node horizontally within its depth layer
        const nodeX = depthStartX.get(depth)! + (layerWidth - layout.width) / 2;
      
        
        layout.x = nodeX;
        layout.y = nodeY;
        layout.order = i;
        
        nodeY += layout.height + opts.nodeSpacingY;
      }
    }
    
    // Add nodes to result
    for (const layout of nodeLayouts.values()) {
      updatedNodes.push({
        ...layout.node,
        position: { x: layout.x, y: layout.y },
        parentNode: undefined,
        extent: undefined,
      });
    }
    
    // Calculate group dimensions
    const groupContentWidth = depthLevels.length > 0 
      ? (layerX - opts.layerSpacingX - currentX - opts.groupPadding)
      : 200;
    const groupWidth = groupContentWidth + opts.groupPadding * 2;
    const groupHeight = maxDepthHeight + opts.groupPadding * 2 + 30; // +30 for header
    
    // Update group position and size
    groupUpdates.push({
      id: group.id,
      position: { x: currentX, y: opts.startY },
      size: { width: groupWidth, height: groupHeight },
    });
    
    // Track current group nodes for next iteration
    previousGroupNodeIds = nodeIdSet;
    
    // Move to next column
    currentX += groupWidth + opts.groupSpacingX;
  }
  
  // Layout ungrouped nodes to the right of all groups
  if (ungroupedNodes.length > 0) {
    let nodeY = opts.startY;
    
    for (const node of ungroupedNodes) {
      const { height } = getNodeDimensions(node);
      
      updatedNodes.push({
        ...node,
        position: { x: currentX, y: nodeY },
        parentNode: undefined,
        extent: undefined,
      });
      
      nodeY += height + opts.nodeSpacingY;
    }
  }
  
  // Re-add existing group nodes with updated positions
  for (const groupNode of existingGroupNodes) {
    // Match by groupId in data or by extracting from node id (group-xxx format)
    const nodeData = groupNode.data as any;
    const groupId = nodeData?.groupId || nodeData?.group?.id;
    const groupUpdate = groupUpdates.find(u => u.id === groupId);
    
    if (groupUpdate) {
      updatedNodes.push({
        ...groupNode,
        position: groupUpdate.position!,
        style: {
          ...groupNode.style,
          width: `${groupUpdate.size!.width}px`,
          height: `${groupUpdate.size!.height}px`,
        },
      });
    } else {
      updatedNodes.push(groupNode);
    }
  }
  
  return { nodes: updatedNodes, groupUpdates };
}

/**
 * Legacy dagre-based layout (for when no groups are defined)
 * 传统的 dagre 布局（当没有定义分组时使用）
 */
export function applyDagreLayout(
  nodes: FlowNode[],
  edges: FlowEdge[],
  options: LayoutOptions = {}
): FlowNode[] {
  // Dynamically import dagre only when needed
  const dagre = require('dagre');
  
  const opts = { ...DEFAULT_OPTIONS, ...options };
  
  const stepNodes = nodes.filter(n => n.type !== 'group');
  const groupNodes = nodes.filter(n => n.type === 'group');
  
  if (stepNodes.length === 0) {
    return nodes;
  }
  
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({
    rankdir: opts.direction,
    nodesep: opts.nodeSpacingY,
    ranksep: opts.groupSpacingX,
    marginx: 80,
    marginy: 80,
    align: 'UL',
  });

  for (const node of stepNodes) {
    const { width, height } = getNodeDimensions(node);
    g.setNode(node.id, { width, height });
  }

  for (const edge of edges) {
    if (stepNodes.some(n => n.id === edge.source) && 
        stepNodes.some(n => n.id === edge.target)) {
      g.setEdge(edge.source, edge.target);
    }
  }

  dagre.layout(g);

  const updatedStepNodes = stepNodes.map((node) => {
    const nodeWithPosition = g.node(node.id);
    if (nodeWithPosition) {
      const { width, height } = getNodeDimensions(node);
      return {
        ...node,
        position: {
          x: nodeWithPosition.x - width / 2,
          y: nodeWithPosition.y - height / 2,
        },
      };
    }
    return node;
  });

  return [...updatedStepNodes, ...groupNodes];
}

/**
 * Check if nodes need auto layout
 */
export function needsAutoLayout(nodes: FlowNode[]): boolean {
  if (nodes.length === 0) return false;
  
  const stepNodes = nodes.filter(n => n.type !== 'group');
  if (stepNodes.length === 0) return false;
  
  const allAtOrigin = stepNodes.every(
    (n) => n.position.x === 0 && n.position.y === 0
  );
  
  if (!allAtOrigin) {
    return checkForOverlap(stepNodes);
  }
  
  return allAtOrigin;
}

/**
 * Check for node overlap
 */
function checkForOverlap(nodes: FlowNode[]): boolean {
  for (let i = 0; i < nodes.length; i++) {
    for (let j = i + 1; j < nodes.length; j++) {
      const a = nodes[i];
      const b = nodes[j];
      
      const aDim = getNodeDimensions(a);
      const bDim = getNodeDimensions(b);
      
      const overlapX = Math.abs(a.position.x - b.position.x) < Math.min(aDim.width, bDim.width) * 0.5;
      const overlapY = Math.abs(a.position.y - b.position.y) < Math.min(aDim.height, bDim.height) * 0.5;
      
      if (overlapX && overlapY) {
        return true;
      }
    }
  }
  return false;
}

/**
 * Get suggested position for a new node
 */
export function getSuggestedPosition(
  existingNodes: FlowNode[],
  selectedNodeId?: string
): { x: number; y: number } {
  const stepNodes = existingNodes.filter(n => n.type !== 'group');
  
  if (stepNodes.length === 0) {
    return { x: 100, y: 100 };
  }

  if (selectedNodeId) {
    const selectedNode = stepNodes.find((n) => n.id === selectedNodeId);
    if (selectedNode) {
      const { width } = getNodeDimensions(selectedNode);
      return {
        x: selectedNode.position.x + width + DEFAULT_OPTIONS.groupSpacingX,
        y: selectedNode.position.y,
      };
    }
  }

  let maxX = 0;
  let nodeAtMaxX: FlowNode | null = null;
  
  for (const node of stepNodes) {
    const nodeRight = node.position.x + getNodeDimensions(node).width;
    if (nodeRight > maxX) {
      maxX = nodeRight;
      nodeAtMaxX = node;
    }
  }

  if (nodeAtMaxX) {
    return {
      x: maxX + DEFAULT_OPTIONS.groupSpacingX,
      y: nodeAtMaxX.position.y,
    };
  }

  return { x: 100, y: 100 };
}

/**
 * Snap position to grid
 */
export function snapToGrid(
  position: { x: number; y: number },
  gridSize = 20
): { x: number; y: number } {
  return {
    x: Math.round(position.x / gridSize) * gridSize,
    y: Math.round(position.y / gridSize) * gridSize,
  };
}

/**
 * Calculate viewport to fit all nodes
 */
export function calculateFitViewport(
  nodes: FlowNode[],
  padding = 50
): { x: number; y: number; zoom: number } {
  const stepNodes = nodes.filter(n => n.type !== 'group');
  
  if (stepNodes.length === 0) {
    return { x: 0, y: 0, zoom: 1 };
  }

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const node of stepNodes) {
    const { width, height } = getNodeDimensions(node);
    
    minX = Math.min(minX, node.position.x);
    minY = Math.min(minY, node.position.y);
    maxX = Math.max(maxX, node.position.x + width);
    maxY = Math.max(maxY, node.position.y + height);
  }

  const width = maxX - minX + padding * 2;
  const height = maxY - minY + padding * 2;

  const zoomX = 800 / width;
  const zoomY = 600 / height;
  const zoom = Math.min(zoomX, zoomY, 1);

  return {
    x: -(minX - padding) * zoom,
    y: -(minY - padding) * zoom,
    zoom,
  };
}
