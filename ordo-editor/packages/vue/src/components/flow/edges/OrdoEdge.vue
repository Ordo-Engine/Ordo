<script setup lang="ts">
/**
 * OrdoEdge - Custom edge component with tooltip support
 * 自定义边组件，支持条件提示
 */
import { computed, ref } from 'vue';
import { getBezierPath, Position } from '@vue-flow/core';
import type { EdgeProps } from '@vue-flow/core';
import type { FlowEdgeData } from '../utils/converter';
import { EDGE_COLORS } from '../types';
import { useI18n } from '../../../locale';

const props = defineProps<EdgeProps<FlowEdgeData>>();

const { t } = useI18n();

const showTooltip = ref(false);
const tooltipPosition = ref({ x: 0, y: 0 });

/** Edge display label (reactive to i18n changes) */
const displayLabel = computed(() => {
  // If explicitly provided via props.label, use it
  if (props.label) return String(props.label);
  
  // Otherwise determine from data
  if (props.data?.isDefault) return t('step.default');
  if (props.data?.edgeType === 'exec-branch') return t('step.branch');
  
  return '';
});

/** Edge color based on type */
const edgeColor = computed(() => {
  const edgeType = props.data?.edgeType || 'exec';
  switch (edgeType) {
    case 'exec-branch':
      return EDGE_COLORS.execBranch;
    case 'data':
      return EDGE_COLORS.data;
    case 'exec':
    default:
      return EDGE_COLORS.exec;
  }
});

/** Edge stroke width */
const strokeWidth = computed(() => {
  const edgeType = props.data?.edgeType || 'exec';
  return edgeType === 'data' ? 1.5 : 2;
});

/** Has condition to show */
const hasCondition = computed(() => !!props.data?.condition);

/** Get path for the edge - using Bezier curves for smooth flow visualization */
const edgePath = computed(() => {
  // Calculate curvature based on vertical distance (more curve for longer vertical spans)
  const verticalDistance = Math.abs(props.targetY - props.sourceY);
  const horizontalDistance = Math.abs(props.targetX - props.sourceX);
  
  // Adjust curvature: more curve for vertical edges, less for horizontal
  // This helps separate edges that have similar start/end points
  const baseCurvature = 0.25;
  const curvature = horizontalDistance > 0 
    ? baseCurvature + Math.min(0.15, verticalDistance / horizontalDistance * 0.1)
    : baseCurvature;
  
  const [path] = getBezierPath({
    sourceX: props.sourceX,
    sourceY: props.sourceY,
    sourcePosition: props.sourcePosition,
    targetX: props.targetX,
    targetY: props.targetY,
    targetPosition: props.targetPosition,
    curvature,
  });
  return path;
});

/** Label position (midpoint) */
const labelPosition = computed(() => ({
  x: (props.sourceX + props.targetX) / 2,
  y: (props.sourceY + props.targetY) / 2,
}));

/** Arrow points based on target position */
const arrowPoints = computed(() => {
  const size = 8;
  const tx = props.targetX;
  const ty = props.targetY;
  const pos = props.targetPosition;
  
  // Arrow points toward the node based on target position
  switch (pos) {
    case Position.Left:
      // Arrow pointing right (entering from left)
      return `${tx + size},${ty - size/2} ${tx},${ty} ${tx + size},${ty + size/2}`;
    case Position.Right:
      // Arrow pointing left (entering from right)
      return `${tx - size},${ty - size/2} ${tx},${ty} ${tx - size},${ty + size/2}`;
    case Position.Top:
      // Arrow pointing down (entering from top)
      return `${tx - size/2},${ty + size} ${tx},${ty} ${tx + size/2},${ty + size}`;
    case Position.Bottom:
      // Arrow pointing up (entering from bottom)
      return `${tx - size/2},${ty - size} ${tx},${ty} ${tx + size/2},${ty - size}`;
    default:
      // Default: arrow pointing left (most common for LR layout)
      return `${tx - size},${ty - size/2} ${tx},${ty} ${tx - size},${ty + size/2}`;
  }
});

/** Handle mouse enter on edge */
function handleMouseEnter(event: MouseEvent) {
  if (hasCondition.value) {
    showTooltip.value = true;
    tooltipPosition.value = { x: event.clientX, y: event.clientY };
  }
}

/** Handle mouse leave */
function handleMouseLeave() {
  showTooltip.value = false;
}

/** Handle mouse move for tooltip position */
function handleMouseMove(event: MouseEvent) {
  if (showTooltip.value) {
    tooltipPosition.value = { x: event.clientX + 10, y: event.clientY + 10 };
  }
}
</script>

<template>
  <g class="ordo-edge">
    <!-- Invisible wider path for easier hover -->
    <path
      :d="edgePath"
      fill="none"
      stroke="transparent"
      :stroke-width="20"
      class="edge-hover-zone"
      @mouseenter="handleMouseEnter"
      @mouseleave="handleMouseLeave"
      @mousemove="handleMouseMove"
    />
    
    <!-- Visible edge path -->
    <path
      :d="edgePath"
      fill="none"
      :stroke="edgeColor"
      :stroke-width="strokeWidth"
      :stroke-dasharray="data?.edgeType === 'data' ? '4 2' : undefined"
      class="edge-path"
      :class="{ 'edge-selected': selected }"
    />
    
    <!-- Arrow marker (direction based on target position) -->
    <polygon
      :points="arrowPoints"
      :fill="edgeColor"
      class="edge-arrow"
    />
    
    <!-- Edge label -->
    <g v-if="displayLabel" :transform="`translate(${labelPosition.x}, ${labelPosition.y})`">
      <rect
        :x="-displayLabel.length * 3.5 - 6"
        y="-10"
        :width="displayLabel.length * 7 + 12"
        height="20"
        rx="3"
        class="edge-label-bg"
      />
      <text
        class="edge-label-text"
        text-anchor="middle"
        dominant-baseline="middle"
      >
        {{ displayLabel }}
      </text>
    </g>
    
    <!-- Tooltip (rendered via portal in real app, simplified here) -->
    <Teleport to="body" v-if="showTooltip && hasCondition">
      <div 
        class="edge-tooltip"
        :style="{ 
          left: tooltipPosition.x + 'px', 
          top: tooltipPosition.y + 'px' 
        }"
      >
        <div class="tooltip-header">Condition</div>
        <div class="tooltip-content">{{ data?.condition }}</div>
      </div>
    </Teleport>
  </g>
</template>

<style scoped>
.ordo-edge {
  cursor: pointer;
}

.edge-hover-zone {
  cursor: pointer;
}

.edge-path {
  transition: stroke 0.15s ease, stroke-width 0.15s ease;
}

.edge-path.edge-selected {
  stroke: v-bind('EDGE_COLORS.selected');
  stroke-width: 3;
}

.edge-arrow {
  transition: fill 0.15s ease;
}

.edge-label-bg {
  fill: var(--ordo-bg-panel, #252525);
  stroke: var(--ordo-border-color, #3c3c3c);
  stroke-width: 1;
}

.edge-label-text {
  font-size: 10px;
  fill: var(--ordo-text-secondary, #b0b0b0);
  font-family: var(--ordo-font-sans);
}
</style>

<style>
/* Global styles for tooltip (since it's teleported) */
.edge-tooltip {
  position: fixed;
  z-index: 10000;
  background: var(--ordo-bg-panel, #252525);
  border: 1px solid var(--ordo-border-color, #3c3c3c);
  border-radius: 4px;
  padding: 8px 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
  max-width: 300px;
  pointer-events: none;
}

.edge-tooltip .tooltip-header {
  font-size: 10px;
  font-weight: 600;
  color: var(--ordo-text-tertiary, #888);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 4px;
}

.edge-tooltip .tooltip-content {
  font-size: 12px;
  color: var(--ordo-text-primary, #e0e0e0);
  font-family: var(--ordo-font-mono, monospace);
  white-space: pre-wrap;
  word-break: break-all;
}
</style>

