<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue';
import type { ConnectionStatus, ServerInfo, DebugSessionInfo } from './types';

const props = defineProps<{
  modelValue?: string;
  autoConnect?: boolean;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: string];
  connected: [info: ServerInfo];
  disconnected: [];
  error: [error: string];
}>();

// State
const endpoint = ref(props.modelValue || 'http://localhost:8080');
const status = ref<ConnectionStatus>('disconnected');
const serverInfo = ref<ServerInfo | null>(null);
const errorMessage = ref('');
const sessions = ref<DebugSessionInfo[]>([]);
const isPolling = ref(false);
let pollingInterval: ReturnType<typeof setInterval> | null = null;

// Computed
const isConnected = computed(() => status.value === 'connected');
const isDebugMode = computed(() => serverInfo.value?.debug_mode ?? false);
const statusColor = computed(() => {
  switch (status.value) {
    case 'connected':
      return isDebugMode.value ? '#10b981' : '#3b82f6';
    case 'connecting':
      return '#f59e0b';
    case 'error':
      return '#ef4444';
    default:
      return '#6b7280';
  }
});
const statusText = computed(() => {
  switch (status.value) {
    case 'connected':
      return isDebugMode.value ? 'Debug Mode' : 'Connected';
    case 'connecting':
      return 'Connecting...';
    case 'error':
      return 'Error';
    default:
      return 'Disconnected';
  }
});

// Methods
async function connect() {
  if (status.value === 'connecting') return;

  status.value = 'connecting';
  errorMessage.value = '';

  try {
    const response = await fetch(`${endpoint.value}/health`, {
      method: 'GET',
      headers: { Accept: 'application/json' },
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const info = (await response.json()) as ServerInfo;
    serverInfo.value = info;
    status.value = 'connected';
    emit('update:modelValue', endpoint.value);
    emit('connected', info);

    // Start polling for sessions if in debug mode
    if (info.debug_mode) {
      startPolling();
    }
  } catch (e) {
    status.value = 'error';
    errorMessage.value = e instanceof Error ? e.message : 'Connection failed';
    emit('error', errorMessage.value);
  }
}

function disconnect() {
  stopPolling();
  status.value = 'disconnected';
  serverInfo.value = null;
  sessions.value = [];
  emit('disconnected');
}

async function fetchSessions() {
  if (!isConnected.value || !isDebugMode.value) return;

  try {
    const response = await fetch(`${endpoint.value}/api/v1/debug/sessions`);
    if (response.ok) {
      sessions.value = await response.json();
    }
  } catch {
    // Ignore polling errors
  }
}

function startPolling() {
  if (pollingInterval) return;
  isPolling.value = true;
  fetchSessions();
  pollingInterval = setInterval(fetchSessions, 5000);
}

function stopPolling() {
  if (pollingInterval) {
    clearInterval(pollingInterval);
    pollingInterval = null;
  }
  isPolling.value = false;
}

function formatUptime(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${h}h ${m}m`;
}

// Watchers
watch(
  () => props.modelValue,
  (val) => {
    if (val && val !== endpoint.value) {
      endpoint.value = val;
    }
  }
);

// Auto-connect
if (props.autoConnect) {
  connect();
}

// Cleanup
onUnmounted(() => {
  stopPolling();
});

// Expose for parent components
defineExpose({
  connect,
  disconnect,
  endpoint,
  status,
  serverInfo,
  isConnected,
  isDebugMode,
  sessions,
});
</script>

<template>
  <div class="ordo-server-connect">
    <!-- Connection Panel -->
    <div class="connect-panel">
      <div class="endpoint-input">
        <input
          v-model="endpoint"
          type="text"
          placeholder="http://localhost:8080"
          :disabled="isConnected"
          @keyup.enter="connect"
        />
        <button
          v-if="!isConnected"
          class="connect-btn"
          :disabled="status === 'connecting'"
          @click="connect"
        >
          {{ status === 'connecting' ? '...' : 'Connect' }}
        </button>
        <button v-else class="disconnect-btn" @click="disconnect">Disconnect</button>
      </div>

      <!-- Status Indicator -->
      <div class="status-row">
        <span class="status-dot" :style="{ backgroundColor: statusColor }" />
        <span class="status-text">{{ statusText }}</span>
        <span v-if="errorMessage" class="error-text">{{ errorMessage }}</span>
      </div>
    </div>

    <!-- Server Info -->
    <div v-if="serverInfo" class="server-info">
      <div class="info-row">
        <span class="label">Version:</span>
        <span class="value">{{ serverInfo.version }}</span>
      </div>
      <div class="info-row">
        <span class="label">Uptime:</span>
        <span class="value">{{ formatUptime(serverInfo.uptime_seconds) }}</span>
      </div>
      <div class="info-row">
        <span class="label">Storage:</span>
        <span class="value"
          >{{ serverInfo.storage.mode }} ({{ serverInfo.storage.rules_count }} rules)</span
        >
      </div>
      <div v-if="isDebugMode" class="debug-badge">
        <span class="badge-icon">🔧</span>
        <span>Debug Mode Active</span>
      </div>
    </div>

    <!-- Debug Sessions (only in debug mode) -->
    <div v-if="isDebugMode && sessions.length > 0" class="sessions-panel">
      <div class="sessions-header">
        <span>Debug Sessions ({{ sessions.length }})</span>
      </div>
      <div class="sessions-list">
        <div v-for="session in sessions" :key="session.id" class="session-item">
          <span class="session-name">{{ session.ruleset_name }}</span>
          <span class="session-state" :class="session.state">{{ session.state }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ordo-server-connect {
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 12px;
  background: #1e1e2e;
  border-radius: 8px;
  padding: 12px;
  color: #cdd6f4;
}

.connect-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.endpoint-input {
  display: flex;
  gap: 8px;
}

.endpoint-input input {
  flex: 1;
  padding: 8px 12px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 6px;
  color: #cdd6f4;
  font-family: inherit;
  font-size: 12px;
}

.endpoint-input input:focus {
  outline: none;
  border-color: #89b4fa;
}

.endpoint-input input:disabled {
  opacity: 0.6;
}

.connect-btn,
.disconnect-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 6px;
  font-family: inherit;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.connect-btn {
  background: #89b4fa;
  color: #1e1e2e;
}

.connect-btn:hover:not(:disabled) {
  background: #b4befe;
}

.connect-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.disconnect-btn {
  background: #f38ba8;
  color: #1e1e2e;
}

.disconnect-btn:hover {
  background: #eba0ac;
}

.status-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-text {
  font-weight: 500;
}

.error-text {
  color: #f38ba8;
  font-size: 11px;
}

.server-info {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid #45475a;
}

.info-row {
  display: flex;
  justify-content: space-between;
  padding: 4px 0;
}

.info-row .label {
  color: #a6adc8;
}

.info-row .value {
  color: #cdd6f4;
}

.debug-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 8px;
  padding: 6px 10px;
  background: rgba(166, 227, 161, 0.15);
  border: 1px solid #a6e3a1;
  border-radius: 6px;
  color: #a6e3a1;
  font-weight: 500;
}

.badge-icon {
  font-size: 14px;
}

.sessions-panel {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid #45475a;
}

.sessions-header {
  font-weight: 500;
  margin-bottom: 8px;
  color: #a6adc8;
}

.sessions-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.session-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 8px;
  background: #313244;
  border-radius: 4px;
}

.session-name {
  font-weight: 500;
}

.session-state {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 4px;
  text-transform: uppercase;
}

.session-state.created {
  background: #45475a;
  color: #a6adc8;
}

.session-state.running {
  background: rgba(137, 180, 250, 0.2);
  color: #89b4fa;
}

.session-state.paused {
  background: rgba(249, 226, 175, 0.2);
  color: #f9e2af;
}

.session-state.completed {
  background: rgba(166, 227, 161, 0.2);
  color: #a6e3a1;
}

.session-state.terminated {
  background: rgba(243, 139, 168, 0.2);
  color: #f38ba8;
}
</style>
