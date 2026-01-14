<script setup lang="ts">
import { computed } from 'vue';
import type { RegisterValue } from './types';

const props = defineProps<{
  registers: RegisterValue[];
  highlightIndex?: number;
}>();

const sortedRegisters = computed(() => {
  return [...props.registers].sort((a, b) => a.index - b.index);
});

function formatValue(value: unknown): string {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (typeof value === 'string') return `"${value}"`;
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (Array.isArray(value)) return `[${value.length}]`;
  if (typeof value === 'object') return '{...}';
  return String(value);
}

function getTypeColor(typeName: string): string {
  switch (typeName) {
    case 'int':
    case 'float':
      return '#fab387'; // Orange
    case 'string':
      return '#a6e3a1'; // Green
    case 'bool':
      return '#89b4fa'; // Blue
    case 'array':
      return '#cba6f7'; // Purple
    case 'object':
      return '#f9e2af'; // Yellow
    case 'null':
      return '#6c7086'; // Gray
    default:
      return '#cdd6f4'; // Default
  }
}
</script>

<template>
  <div class="register-panel">
    <div class="panel-header">
      <span class="title">Registers</span>
      <span class="count">{{ registers.length }}</span>
    </div>
    <div class="register-list">
      <div
        v-for="reg in sortedRegisters"
        :key="reg.index"
        class="register-item"
        :class="{ highlighted: reg.index === highlightIndex }"
      >
        <span class="reg-index">R{{ reg.index }}</span>
        <span class="reg-value" :style="{ color: getTypeColor(reg.type_name) }">
          {{ formatValue(reg.value) }}
        </span>
        <span class="reg-type">{{ reg.type_name }}</span>
      </div>
      <div v-if="registers.length === 0" class="empty-state">No registers</div>
    </div>
  </div>
</template>

<style scoped>
.register-panel {
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 11px;
  background: #1e1e2e;
  border-radius: 6px;
  overflow: hidden;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #313244;
  border-bottom: 1px solid #45475a;
}

.title {
  font-weight: 600;
  color: #cdd6f4;
}

.count {
  font-size: 10px;
  padding: 2px 6px;
  background: #45475a;
  border-radius: 4px;
  color: #a6adc8;
}

.register-list {
  max-height: 200px;
  overflow-y: auto;
}

.register-item {
  display: grid;
  grid-template-columns: 40px 1fr 60px;
  gap: 8px;
  padding: 6px 12px;
  border-bottom: 1px solid #313244;
  transition: background 0.15s;
}

.register-item:hover {
  background: #313244;
}

.register-item.highlighted {
  background: rgba(137, 180, 250, 0.15);
  border-left: 2px solid #89b4fa;
}

.reg-index {
  color: #89b4fa;
  font-weight: 500;
}

.reg-value {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reg-type {
  color: #6c7086;
  font-size: 10px;
  text-align: right;
}

.empty-state {
  padding: 16px;
  text-align: center;
  color: #6c7086;
}

.register-list::-webkit-scrollbar {
  width: 6px;
}

.register-list::-webkit-scrollbar-track {
  background: #1e1e2e;
}

.register-list::-webkit-scrollbar-thumb {
  background: #45475a;
  border-radius: 3px;
}

.register-list::-webkit-scrollbar-thumb:hover {
  background: #585b70;
}
</style>
