<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter } from 'vue-router';
import { DialogPlugin, MessagePlugin } from 'tdesign-vue-next';
import { serverApi } from '@/api/platform-client';
import type { ServerInfo } from '@/api/types';
import { useAuthStore } from '@/stores/auth';

interface HealthPayload {
  online: boolean;
  response?: string;
  error?: string;
  url: string;
}

interface MetricSample {
  name: string;
  value: string;
  labels: string[];
}

type ServerFilter = 'active' | 'offline' | 'all';

const { t } = useI18n();
const router = useRouter();
const auth = useAuthStore();

const loadingServers = ref(false);
const removingServerId = ref<string | null>(null);
const servers = ref<ServerInfo[]>([]);
const serverFilter = ref<ServerFilter>('active');
const keyword = ref('');

const healthDialogVisible = ref(false);
const healthDialogServer = ref<ServerInfo | null>(null);
const healthLoading = ref(false);
const healthRawOpen = ref(false);
const healthContent = ref<HealthPayload | null>(null);

const metricsDialogVisible = ref(false);
const metricsDialogServer = ref<ServerInfo | null>(null);
const metricsLoading = ref(false);
const metricsRawOpen = ref(false);
const metricsRaw = ref('');
const metricsParsed = ref<MetricSample[]>([]);

const filterOptions = computed(() => [
  { label: t('settings.serverRegistry.filterActive'), value: 'active' },
  { label: t('settings.serverRegistry.filterOffline'), value: 'offline' },
  { label: t('settings.serverRegistry.filterAll'), value: 'all' },
]);

const columns = computed(() => [
  { colKey: 'name', title: t('settings.serverRegistry.serverName'), width: 240 },
  { colKey: 'url', title: t('settings.serverRegistry.endpoint'), minWidth: 240 },
  {
    colKey: 'status',
    title: t('settings.serverRegistry.status'),
    width: 120,
    align: 'center' as const,
  },
  { colKey: 'version', title: t('settings.serverRegistry.version'), width: 120 },
  { colKey: 'last_seen', title: t('settings.serverRegistry.lastSeen'), width: 190 },
  { colKey: 'registered_at', title: t('settings.serverRegistry.registeredAt'), width: 190 },
  { colKey: 'labels', title: t('settings.serverRegistry.labels'), minWidth: 180 },
  {
    colKey: 'actions',
    title: t('settings.serverRegistry.actions'),
    width: 220,
    align: 'right' as const,
  },
]);

const filteredServers = computed(() => {
  const query = keyword.value.trim().toLowerCase();
  const statusWeight: Record<ServerInfo['status'], number> = {
    online: 0,
    degraded: 1,
    offline: 2,
  };

  return servers.value
    .filter((server) => {
      if (serverFilter.value === 'active' && server.status === 'offline') return false;
      if (serverFilter.value === 'offline' && server.status !== 'offline') return false;
      if (!query) return true;
      return [
        server.name,
        server.url,
        server.version ?? '',
        ...Object.entries(server.labels).map(([key, value]) => `${key}=${value}`),
      ]
        .join(' ')
        .toLowerCase()
        .includes(query);
    })
    .slice()
    .sort((left, right) => {
      const statusDelta = statusWeight[left.status] - statusWeight[right.status];
      if (statusDelta !== 0) return statusDelta;
      const leftSeen = left.last_seen ? new Date(left.last_seen).getTime() : 0;
      const rightSeen = right.last_seen ? new Date(right.last_seen).getTime() : 0;
      if (leftSeen !== rightSeen) return rightSeen - leftSeen;
      return new Date(right.registered_at).getTime() - new Date(left.registered_at).getTime();
    });
});

function formatTimestamp(value: string | null | undefined) {
  if (!value) return t('settings.serverRegistry.neverSeen');
  return new Date(value).toLocaleString();
}

function statusLabel(status: ServerInfo['status']) {
  return t(`settings.serverRegistry.statusMap.${status}`);
}

function serverTheme(status: ServerInfo['status']) {
  switch (status) {
    case 'online':
      return 'success';
    case 'degraded':
      return 'warning';
    default:
      return 'danger';
  }
}

function parseMetrics(raw: string): MetricSample[] {
  return raw
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'))
    .map((line) => {
      const match = line.match(/^([^\s{]+)(?:\{([^}]*)\})?\s+(.+)$/);
      if (!match) return { name: line, value: '', labels: [] };
      const [, name, labelsPart, value] = match;
      const labels = labelsPart
        ? labelsPart
            .split(/,(?=[a-zA-Z_][a-zA-Z0-9_]*=)/)
            .map((label) => label.trim())
            .filter(Boolean)
        : [];
      return { name, value: value.trim(), labels };
    });
}

async function loadServers() {
  if (!auth.token) return;
  loadingServers.value = true;
  try {
    servers.value = await serverApi.list(auth.token);
  } catch {
    MessagePlugin.error(t('settings.serverRegistry.loadFailed'));
  } finally {
    loadingServers.value = false;
  }
}

async function openHealth(server: ServerInfo) {
  if (!auth.token) return;
  healthDialogServer.value = server;
  healthDialogVisible.value = true;
  healthRawOpen.value = false;
  healthLoading.value = true;
  try {
    healthContent.value = await serverApi.getHealth(auth.token, server.id);
  } catch (error: any) {
    healthContent.value = {
      online: false,
      error: error.message || t('settings.serverRegistry.healthFailed'),
      url: server.url,
    };
  } finally {
    healthLoading.value = false;
  }
}

async function openMetrics(server: ServerInfo) {
  if (!auth.token) return;
  metricsDialogServer.value = server;
  metricsDialogVisible.value = true;
  metricsRawOpen.value = false;
  metricsLoading.value = true;
  try {
    const raw = await serverApi.getMetrics(auth.token, server.id);
    metricsRaw.value = raw;
    metricsParsed.value = parseMetrics(raw);
  } catch (error: any) {
    metricsRaw.value = error.message || t('settings.serverRegistry.metricsFailed');
    metricsParsed.value = [];
  } finally {
    metricsLoading.value = false;
  }
}

async function removeServer(server: ServerInfo) {
  if (!auth.token) return;

  const dialog = DialogPlugin.confirm({
    header: t('settings.serverRegistry.removeDialog'),
    body: t('settings.serverRegistry.removeConfirm', { name: server.name }),
    confirmBtn: { content: t('common.confirm'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      removingServerId.value = server.id;
      try {
        await serverApi.delete(auth.token!, server.id);
        servers.value = servers.value.filter((item) => item.id !== server.id);
        dialog.hide();
        MessagePlugin.success(t('settings.serverRegistry.removeSuccess'));
      } catch {
        MessagePlugin.error(t('settings.serverRegistry.removeFailed'));
      } finally {
        removingServerId.value = null;
      }
    },
  });
}

onMounted(loadServers);
</script>

<template>
  <div class="registry-page">
    <t-breadcrumb class="breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{
        t('breadcrumb.home')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('settings.serverRegistry.title') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t('settings.serverRegistry.title') }}</h1>
        <p class="page-desc">{{ t('settings.serverRegistry.desc') }}</p>
        <p class="page-hint">{{ t('settings.serverRegistry.cleanupHint') }}</p>
      </div>
      <t-button variant="outline" :loading="loadingServers" @click="loadServers">
        {{ t('settings.serverRegistry.refresh') }}
      </t-button>
    </div>

    <section class="registry-panel">
      <div class="toolbar">
        <t-radio-group v-model="serverFilter" variant="default-filled">
          <t-radio-button v-for="option in filterOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </t-radio-button>
        </t-radio-group>
        <t-input
          v-model="keyword"
          class="toolbar-search"
          clearable
          :placeholder="t('settings.serverRegistry.searchPlaceholder')"
        />
      </div>

      <t-table
        row-key="id"
        size="small"
        hover
        :loading="loadingServers"
        :data="filteredServers"
        :columns="columns"
        :empty="t('settings.serverRegistry.empty')"
      >
        <template #name="{ row }">
          <div class="server-name-cell">
            <strong>{{ row.name }}</strong>
            <span class="server-id">{{ row.id }}</span>
          </div>
        </template>

        <template #url="{ row }">
          <span class="mono">{{ row.url }}</span>
        </template>

        <template #status="{ row }">
          <t-tag :theme="serverTheme(row.status)" variant="light" size="small">
            {{ statusLabel(row.status) }}
          </t-tag>
        </template>

        <template #version="{ row }">
          {{ row.version || '-' }}
        </template>

        <template #last_seen="{ row }">
          {{ formatTimestamp(row.last_seen) }}
        </template>

        <template #registered_at="{ row }">
          {{ formatTimestamp(row.registered_at) }}
        </template>

        <template #labels="{ row }">
          <div v-if="Object.keys(row.labels).length" class="label-list">
            <span v-for="(value, key) in row.labels" :key="key" class="label-chip"
              >{{ key }}={{ value }}</span
            >
          </div>
          <span v-else class="text-muted">{{ t('settings.serverRegistry.noLabels') }}</span>
        </template>

        <template #actions="{ row }">
          <div class="action-group">
            <t-button size="small" variant="text" @click="openHealth(row)">
              {{ t('settings.serverRegistry.healthAction') }}
            </t-button>
            <t-button size="small" variant="text" @click="openMetrics(row)">
              {{ t('settings.serverRegistry.metricsAction') }}
            </t-button>
            <t-button
              v-if="row.status === 'offline'"
              size="small"
              theme="danger"
              variant="text"
              :loading="removingServerId === row.id"
              @click="removeServer(row)"
            >
              {{ t('settings.serverRegistry.removeAction') }}
            </t-button>
          </div>
        </template>
      </t-table>
    </section>

    <t-dialog
      v-model:visible="healthDialogVisible"
      :header="
        healthDialogServer
          ? `${t('settings.serverRegistry.healthDialog')} · ${healthDialogServer.name}`
          : t('settings.serverRegistry.healthDialog')
      "
      width="720px"
      destroy-on-close
    >
      <div v-if="healthLoading" class="detail-loading">
        <t-loading size="small" />
        <span>{{ t('settings.serverRegistry.loadingHealth') }}</span>
      </div>

      <template v-else-if="healthDialogServer && healthContent">
        <div class="dialog-summary">
          <t-tag :theme="healthContent.online ? 'success' : 'danger'" variant="light">
            {{
              healthContent.online
                ? t('settings.serverRegistry.healthOnline')
                : t('settings.serverRegistry.healthOffline')
            }}
          </t-tag>
        </div>
        <div class="detail-grid">
          <div class="detail-row">
            <span>{{ t('settings.serverRegistry.endpoint') }}</span>
            <strong>{{ healthContent.url || healthDialogServer.url }}</strong>
          </div>
          <div class="detail-row">
            <span>{{ t('settings.serverRegistry.lastSeen') }}</span>
            <strong>{{ formatTimestamp(healthDialogServer.last_seen) }}</strong>
          </div>
          <div v-if="healthContent.response" class="detail-row">
            <span>{{ t('settings.serverRegistry.response') }}</span>
            <strong>{{ healthContent.response }}</strong>
          </div>
          <div v-if="healthContent.error" class="detail-row">
            <span>{{ t('settings.serverRegistry.error') }}</span>
            <strong>{{ healthContent.error }}</strong>
          </div>
        </div>
        <div class="raw-toggle">
          <t-button size="small" variant="text" @click="healthRawOpen = !healthRawOpen">
            {{
              healthRawOpen
                ? t('settings.serverRegistry.hideRaw')
                : t('settings.serverRegistry.showRaw')
            }}
          </t-button>
        </div>
        <pre v-if="healthRawOpen" class="detail-pre">{{
          JSON.stringify(healthContent, null, 2)
        }}</pre>
      </template>
    </t-dialog>

    <t-dialog
      v-model:visible="metricsDialogVisible"
      :header="
        metricsDialogServer
          ? `${t('settings.serverRegistry.metricsDialog')} · ${metricsDialogServer.name}`
          : t('settings.serverRegistry.metricsDialog')
      "
      width="920px"
      destroy-on-close
    >
      <div v-if="metricsLoading" class="detail-loading">
        <t-loading size="small" />
        <span>{{ t('settings.serverRegistry.loadingMetrics') }}</span>
      </div>

      <template v-else>
        <div v-if="metricsParsed.length" class="metrics-table">
          <div class="metrics-table__head">
            <span>{{ t('settings.serverRegistry.metricName') }}</span>
            <span>{{ t('settings.serverRegistry.metricValue') }}</span>
            <span>{{ t('settings.serverRegistry.labels') }}</span>
          </div>
          <div
            v-for="sample in metricsParsed"
            :key="`${sample.name}-${sample.value}-${sample.labels.join('|')}`"
            class="metrics-table__row"
          >
            <span class="metrics-table__name">{{ sample.name }}</span>
            <strong class="metrics-table__value">{{ sample.value }}</strong>
            <span class="metrics-table__labels">
              {{
                sample.labels.length
                  ? sample.labels.join(', ')
                  : t('settings.serverRegistry.noLabels')
              }}
            </span>
          </div>
        </div>
        <div v-else class="detail-empty">{{ t('settings.serverRegistry.noMetrics') }}</div>

        <div class="raw-toggle">
          <t-button size="small" variant="text" @click="metricsRawOpen = !metricsRawOpen">
            {{
              metricsRawOpen
                ? t('settings.serverRegistry.hideRaw')
                : t('settings.serverRegistry.showRaw')
            }}
          </t-button>
        </div>
        <pre v-if="metricsRawOpen" class="detail-pre">{{ metricsRaw }}</pre>
      </template>
    </t-dialog>
  </div>
</template>

<style scoped>
.registry-page {
  height: 100%;
  overflow-y: auto;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.breadcrumb {
  margin-bottom: -4px;
}

.page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.page-title {
  margin: 0;
  font-size: 22px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.page-desc,
.page-hint {
  margin: 6px 0 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--ordo-text-secondary);
}

.registry-panel {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: 10px;
  padding: 16px;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
}

.toolbar-search {
  width: 320px;
}

.server-name-cell {
  display: grid;
  gap: 4px;
}

.server-id,
.mono,
.text-muted {
  color: var(--ordo-text-secondary);
  font-size: 12px;
}

.server-id,
.mono {
  font-family: 'JetBrains Mono', monospace;
}

.label-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.label-chip {
  padding: 3px 8px;
  border-radius: 999px;
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-secondary);
  font-size: 11px;
}

.action-group {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
}

.dialog-summary {
  margin-bottom: 16px;
}

.detail-loading,
.detail-empty {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.detail-grid {
  display: grid;
  gap: 12px;
}

.detail-row {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--ordo-border-light);
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.detail-row strong {
  color: var(--ordo-text-primary);
  text-align: right;
  word-break: break-word;
}

.raw-toggle {
  margin-top: 14px;
}

.detail-pre {
  margin: 12px 0 0;
  padding: 14px;
  border-radius: 10px;
  background: var(--ordo-bg-app);
  border: 1px solid var(--ordo-border-color);
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 320px;
  overflow: auto;
}

.metrics-table {
  display: grid;
  gap: 8px;
}

.metrics-table__head,
.metrics-table__row {
  display: grid;
  grid-template-columns: minmax(220px, 1.4fr) minmax(120px, 0.8fr) minmax(260px, 1fr);
  gap: 12px;
  align-items: start;
}

.metrics-table__head {
  padding: 0 12px 6px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.metrics-table__row {
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--ordo-bg-app);
  border: 1px solid var(--ordo-border-color);
  font-size: 12px;
}

.metrics-table__name {
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-primary);
}

.metrics-table__value {
  color: #335eea;
}

.metrics-table__labels {
  color: var(--ordo-text-secondary);
}

@media (max-width: 960px) {
  .toolbar {
    flex-direction: column;
    align-items: stretch;
  }

  .toolbar-search {
    width: 100%;
  }

  .action-group {
    justify-content: flex-start;
    flex-wrap: wrap;
  }
}
</style>
