<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue';
import OrdoRegisterPanel from './OrdoRegisterPanel.vue';
import type { VMTrace, VMSnapshot, RegisterValue, DebugEvent, SessionState } from './types';

const props = defineProps<{
  endpoint: string;
  sessionId?: string;
  trace?: VMTrace;
  autoPlay?: boolean;
}>();

const emit = defineEmits<{
  step: [snapshot: VMSnapshot];
  complete: [];
  error: [message: string];
}>();

// State
const currentIp = ref(0);
const isPlaying = ref(false);
const playSpeed = ref(500); // ms per instruction
const sessionState = ref<SessionState>('created');
const currentRegisters = ref<RegisterValue[]>([]);
const eventSource = ref<EventSource | null>(null);

// Computed
const currentSnapshot = computed(() => {
  if (!props.trace?.snapshots) return null;
  return props.trace.snapshots.find((s) => s.ip === currentIp.value) ?? null;
});

const instructions = computed(() => props.trace?.instructions ?? []);
const constants = computed(() => props.trace?.constants ?? []);
const fields = computed(() => props.trace?.fields ?? []);

const progress = computed(() => {
  if (!props.trace?.snapshots?.length) return 0;
  const idx = props.trace.snapshots.findIndex((s) => s.ip === currentIp.value);
  return idx >= 0 ? ((idx + 1) / props.trace.snapshots.length) * 100 : 0;
});

// Playback controls
let playTimer: ReturnType<typeof setTimeout> | null = null;

function play() {
  if (!props.trace?.snapshots?.length) return;
  isPlaying.value = true;
  stepForward();
}

function pause() {
  isPlaying.value = false;
  if (playTimer) {
    clearTimeout(playTimer);
    playTimer = null;
  }
}

function stop() {
  pause();
  currentIp.value = 0;
  updateRegisters();
}

function stepForward() {
  if (!props.trace?.snapshots?.length) return;

  const currentIdx = props.trace.snapshots.findIndex((s) => s.ip === currentIp.value);
  const nextIdx = currentIdx + 1;

  if (nextIdx < props.trace.snapshots.length) {
    currentIp.value = props.trace.snapshots[nextIdx].ip;
    updateRegisters();
    emit('step', props.trace.snapshots[nextIdx]);

    if (isPlaying.value) {
      playTimer = setTimeout(stepForward, playSpeed.value);
    }
  } else {
    isPlaying.value = false;
    emit('complete');
  }
}

function stepBack() {
  if (!props.trace?.snapshots?.length) return;

  const currentIdx = props.trace.snapshots.findIndex((s) => s.ip === currentIp.value);
  if (currentIdx > 0) {
    currentIp.value = props.trace.snapshots[currentIdx - 1].ip;
    updateRegisters();
    emit('step', props.trace.snapshots[currentIdx - 1]);
  }
}

function jumpTo(ip: number) {
  currentIp.value = ip;
  updateRegisters();
  const snapshot = props.trace?.snapshots?.find((s) => s.ip === ip);
  if (snapshot) {
    emit('step', snapshot);
  }
}

function updateRegisters() {
  const snapshot = currentSnapshot.value;
  currentRegisters.value = snapshot?.registers ?? [];
}

// SSE connection for live debugging
function connectSSE() {
  if (!props.sessionId || !props.endpoint) return;

  const url = `${props.endpoint}/api/v1/debug/stream/${props.sessionId}`;
  eventSource.value = new EventSource(url);

  eventSource.value.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data) as DebugEvent;
      handleDebugEvent(data);
    } catch {
      // Ignore parse errors
    }
  };

  eventSource.value.onerror = () => {
    emit('error', 'SSE connection error');
    disconnectSSE();
  };
}

function disconnectSSE() {
  if (eventSource.value) {
    eventSource.value.close();
    eventSource.value = null;
  }
}

function handleDebugEvent(event: DebugEvent) {
  switch (event.type) {
    case 'state_change':
      sessionState.value = event.state as SessionState;
      break;
    case 'vm_state':
      currentIp.value = event.ip as number;
      currentRegisters.value = event.registers as RegisterValue[];
      break;
    case 'execution_complete':
      sessionState.value = 'completed';
      emit('complete');
      break;
    case 'error':
      emit('error', event.message as string);
      break;
  }
}

// Send control commands
async function sendCommand(command: string, params: Record<string, unknown> = {}) {
  if (!props.sessionId || !props.endpoint) return;

  try {
    await fetch(`${props.endpoint}/api/v1/debug/control/${props.sessionId}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ command, ...params }),
    });
  } catch (e) {
    emit('error', e instanceof Error ? e.message : 'Command failed');
  }
}

// Watchers
watch(
  () => props.trace,
  () => {
    currentIp.value = 0;
    updateRegisters();
  }
);

watch(
  () => props.sessionId,
  (newId) => {
    disconnectSSE();
    if (newId) {
      connectSSE();
    }
  }
);

// Auto-play
if (props.autoPlay && props.trace?.snapshots?.length) {
  play();
}

// Cleanup
onUnmounted(() => {
  pause();
  disconnectSSE();
});

// Expose
defineExpose({
  play,
  pause,
  stop,
  stepForward,
  stepBack,
  jumpTo,
  sendCommand,
});
</script>

<template>
  <div class="vm-debugger">
    <!-- Toolbar -->
    <div class="debugger-toolbar">
      <div class="controls">
        <button class="ctrl-btn" title="Step Back" @click="stepBack">⏮</button>
        <button v-if="!isPlaying" class="ctrl-btn play" title="Play" @click="play">▶</button>
        <button v-else class="ctrl-btn" title="Pause" @click="pause">⏸</button>
        <button class="ctrl-btn" title="Stop" @click="stop">⏹</button>
        <button class="ctrl-btn" title="Step Forward" @click="stepForward">⏭</button>
      </div>

      <div class="speed-control">
        <label>Speed:</label>
        <input v-model.number="playSpeed" type="range" min="100" max="2000" step="100" />
        <span>{{ playSpeed }}ms</span>
      </div>

      <div class="progress-bar">
        <div class="progress-fill" :style="{ width: `${progress}%` }" />
      </div>
    </div>

    <!-- Main Content -->
    <div class="debugger-content">
      <!-- Instructions Panel -->
      <div class="instructions-panel">
        <div class="panel-header">Instructions</div>
        <div class="instruction-list">
          <div
            v-for="(inst, idx) in instructions"
            :key="idx"
            class="instruction-item"
            :class="{
              current: idx === currentIp,
              executed: trace?.snapshots?.some((s) => s.ip === idx),
            }"
            @click="jumpTo(idx)"
          >
            <span class="inst-index">{{ idx.toString().padStart(3, '0') }}</span>
            <span class="inst-text">{{ inst }}</span>
          </div>
        </div>
      </div>

      <!-- Right Panel -->
      <div class="right-panel">
        <!-- Registers -->
        <OrdoRegisterPanel
          :registers="currentRegisters"
          :highlight-index="currentSnapshot?.registers?.[0]?.index"
        />

        <!-- Pools -->
        <div class="pools">
          <div class="pool">
            <div class="pool-header">Constants</div>
            <div class="pool-list">
              <div v-for="(c, i) in constants" :key="i" class="pool-item">
                <span class="pool-idx">{{ i }}</span>
                <span class="pool-val">{{ c }}</span>
              </div>
            </div>
          </div>

          <div class="pool">
            <div class="pool-header">Fields</div>
            <div class="pool-list">
              <div v-for="(f, i) in fields" :key="i" class="pool-item">
                <span class="pool-idx">{{ i }}</span>
                <span class="pool-val">{{ f }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Status Bar -->
    <div class="status-bar">
      <span>IP: {{ currentIp }}</span>
      <span v-if="currentSnapshot">Duration: {{ currentSnapshot.duration_ns }}ns</span>
      <span v-if="trace">Total: {{ trace.total_instructions }} instructions</span>
    </div>
  </div>
</template>

<style scoped>
.vm-debugger {
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 12px;
  background: #1e1e2e;
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  height: 100%;
  color: #cdd6f4;
}

.debugger-toolbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 8px 12px;
  background: #313244;
  border-bottom: 1px solid #45475a;
}

.controls {
  display: flex;
  gap: 4px;
}

.ctrl-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #45475a;
  border: none;
  border-radius: 6px;
  color: #cdd6f4;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s;
}

.ctrl-btn:hover {
  background: #585b70;
}

.ctrl-btn.play {
  background: #a6e3a1;
  color: #1e1e2e;
}

.ctrl-btn.play:hover {
  background: #94e2d5;
}

.speed-control {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #a6adc8;
}

.speed-control input {
  width: 100px;
}

.progress-bar {
  flex: 1;
  height: 4px;
  background: #45475a;
  border-radius: 2px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: #89b4fa;
  transition: width 0.2s;
}

.debugger-content {
  display: grid;
  grid-template-columns: 1fr 300px;
  flex: 1;
  overflow: hidden;
}

.instructions-panel {
  display: flex;
  flex-direction: column;
  border-right: 1px solid #45475a;
  overflow: hidden;
}

.panel-header {
  padding: 8px 12px;
  background: #313244;
  font-weight: 600;
  border-bottom: 1px solid #45475a;
}

.instruction-list {
  flex: 1;
  overflow-y: auto;
}

.instruction-item {
  display: flex;
  gap: 12px;
  padding: 6px 12px;
  cursor: pointer;
  transition: background 0.15s;
  border-left: 3px solid transparent;
}

.instruction-item:hover {
  background: #313244;
}

.instruction-item.current {
  background: rgba(137, 180, 250, 0.2);
  border-left-color: #89b4fa;
}

.instruction-item.executed {
  color: #a6adc8;
}

.inst-index {
  color: #6c7086;
  font-size: 10px;
  min-width: 24px;
}

.inst-text {
  flex: 1;
}

.right-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
  overflow-y: auto;
}

.pools {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.pool {
  background: #313244;
  border-radius: 6px;
  overflow: hidden;
}

.pool-header {
  padding: 6px 8px;
  background: #45475a;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  color: #a6adc8;
}

.pool-list {
  max-height: 100px;
  overflow-y: auto;
}

.pool-item {
  display: flex;
  gap: 8px;
  padding: 4px 8px;
  font-size: 10px;
}

.pool-idx {
  color: #6c7086;
  min-width: 16px;
}

.pool-val {
  color: #cdd6f4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-bar {
  display: flex;
  gap: 16px;
  padding: 6px 12px;
  background: #313244;
  border-top: 1px solid #45475a;
  font-size: 11px;
  color: #a6adc8;
}

/* Scrollbar */
.instruction-list::-webkit-scrollbar,
.pool-list::-webkit-scrollbar,
.right-panel::-webkit-scrollbar {
  width: 6px;
}

.instruction-list::-webkit-scrollbar-track,
.pool-list::-webkit-scrollbar-track,
.right-panel::-webkit-scrollbar-track {
  background: #1e1e2e;
}

.instruction-list::-webkit-scrollbar-thumb,
.pool-list::-webkit-scrollbar-thumb,
.right-panel::-webkit-scrollbar-thumb {
  background: #45475a;
  border-radius: 3px;
}
</style>
