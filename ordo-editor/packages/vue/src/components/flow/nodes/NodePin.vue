<script setup lang="ts">
/**
 * NodePin - Universal pin component for flow nodes
 * 通用端口组件，支持执行流和数据流
 */
import { computed } from 'vue';
import { Handle, Position } from '@vue-flow/core';
import type { Pin, PinType, PinDirection } from '../types';
import { PIN_COLORS, PIN_SIZES } from '../types';

const props = withDefaults(
  defineProps<{
    /** Pin configuration */
    pin: Pin;
    /** Position on node */
    position: Position;
    /** Vertical offset (percentage or pixels) */
    offset?: string | number;
  }>(),
  {
    offset: '50%',
  }
);

/** Handle type for Vue Flow */
const handleType = computed(() => (props.pin.direction === 'input' ? 'target' : 'source'));

/** Get pin color based on type and state */
const pinColor = computed(() => {
  const { pin } = props;

  if (pin.type === 'data') {
    return PIN_COLORS.dataPin;
  }

  // Execution flow
  if (pin.direction === 'input') {
    return PIN_COLORS.execInput;
  }

  // Output execution
  if (pin.isDefault) {
    return PIN_COLORS.execDefault;
  }

  return PIN_COLORS.execBranch;
});

/** Get pin size */
const pinSize = computed(() => PIN_SIZES[props.pin.type]);

/** CSS classes for the pin */
const pinClasses = computed(() => [
  'node-pin',
  `pin-${props.pin.type}`,
  `pin-${props.pin.direction}`,
  {
    'pin-default': props.pin.isDefault,
    'pin-branch':
      !props.pin.isDefault && props.pin.direction === 'output' && props.pin.type === 'exec',
  },
]);

/** Style for positioning */
const positionStyle = computed(() => {
  const offset = typeof props.offset === 'number' ? `${props.offset}px` : props.offset;

  if (props.position === Position.Left || props.position === Position.Right) {
    return { top: offset };
  }
  return { left: offset };
});

/** Tooltip content */
const tooltip = computed(() => {
  const { pin } = props;
  if (pin.condition) {
    return pin.condition;
  }
  if (pin.label) {
    return pin.label;
  }
  if (pin.dataType) {
    return `${pin.dataType}${pin.value ? ': ' + pin.value : ''}`;
  }
  return '';
});
</script>

<template>
  <Handle
    :id="pin.id"
    :type="handleType"
    :position="position"
    :class="pinClasses"
    :style="positionStyle"
    :title="tooltip"
  >
    <!-- Exec pin: triangle shape -->
    <svg
      v-if="pin.type === 'exec'"
      class="pin-shape pin-exec-shape"
      :width="pinSize.width"
      :height="pinSize.height"
      viewBox="0 0 10 10"
    >
      <polygon
        :points="pin.direction === 'input' ? '0,0 10,5 0,10' : '0,0 10,5 0,10'"
        :fill="pinColor"
        class="pin-fill"
      />
    </svg>

    <!-- Data pin: circle shape -->
    <svg
      v-else
      class="pin-shape pin-data-shape"
      :width="pinSize.width"
      :height="pinSize.height"
      viewBox="0 0 8 8"
    >
      <circle cx="4" cy="4" r="3.5" :fill="pinColor" class="pin-fill" />
    </svg>
  </Handle>
</template>

<style scoped>
.node-pin {
  /* Reset default handle styles */
  width: auto !important;
  height: auto !important;
  min-width: 0 !important;
  min-height: 0 !important;
  background: transparent !important;
  border: none !important;
  border-radius: 0 !important;

  /* Positioning */
  display: flex;
  align-items: center;
  justify-content: center;

  /* Remove default transform on hover */
  transform: none !important;
}

.pin-shape {
  display: block;
  transition: filter 0.15s ease;
}

.pin-fill {
  transition: fill 0.15s ease;
}

/* Hover effects - no size change, only glow */
.node-pin:hover .pin-exec-shape .pin-fill {
  filter: drop-shadow(0 0 4px currentColor);
}

.node-pin:hover .pin-data-shape .pin-fill {
  filter: drop-shadow(0 0 4px v-bind('PIN_COLORS.dataHover'));
}

/* Input pins on left */
.pin-input {
  left: -5px;
}

/* Output pins on right */
.pin-output {
  right: -5px;
}

/* Exec input - rotated to point right */
.pin-exec.pin-input .pin-exec-shape {
  transform: rotate(0deg);
}

/* Exec output - already points right */
.pin-exec.pin-output .pin-exec-shape {
  transform: rotate(0deg);
}
</style>
