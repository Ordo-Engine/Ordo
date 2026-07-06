<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useProjectStore } from '@/stores/project';
import { useAnalyticsStore } from '@/stores/analytics';

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const projectStore = useProjectStore();
const analytics = useAnalyticsStore();

const orgId = computed(() => route.params.orgId as string);
const projectId = computed(() => route.params.projectId as string);
const project = computed(() => projectStore.currentProject);

const rangeOptions = [
  { label: t('analytics.range5m'), value: '5m' },
  { label: t('analytics.range1h'), value: '1h' },
  { label: t('analytics.range24h'), value: '24h' },
  { label: t('analytics.range7d'), value: '7d' },
];

const data = computed(() => analytics.data);
const hasData = computed(() => (data.value?.totals.calls ?? 0) > 0);

function fmtMs(v: number): string {
  if (v >= 1000) return `${(v / 1000).toFixed(2)} s`;
  if (v >= 1) return `${v.toFixed(2)} ms`;
  return `${(v * 1000).toFixed(0)} µs`;
}
function fmtPct(v: number): string {
  return `${(v * 100).toFixed(1)}%`;
}

// ── charts (raw echarts, lazy-imported so it code-splits out of the bundle) ──
const lineEl = ref<HTMLElement | null>(null);
const pieEl = ref<HTMLElement | null>(null);
type Chart = { setOption: (o: unknown) => void; resize: () => void; dispose: () => void };
let echartsMod: typeof import('echarts') | null = null;
let lineChart: Chart | null = null;
let pieChart: Chart | null = null;

// Init charts against the DOM refs — which only exist once `hasData` renders the
// chart section, so this must run AFTER data arrives + a DOM flush (see watch).
async function ensureCharts() {
  if (!lineEl.value || !pieEl.value) return;
  if (!echartsMod) echartsMod = await import('echarts');
  if (!lineChart) lineChart = echartsMod.init(lineEl.value) as unknown as Chart;
  if (!pieChart) pieChart = echartsMod.init(pieEl.value) as unknown as Chart;
}

function renderCharts() {
  const d = data.value;
  if (!d) return;
  if (lineChart) {
    // Densify: emit every bucket across the full [from, to] window (zero-filled),
    // so the x-axis spans the whole selected period instead of only the buckets
    // that happened to have traffic.
    const bucketMs = Math.max(1, d.bucket_seconds) * 1000;
    const from = new Date(d.from).getTime();
    const to = new Date(d.to).getTime();
    const start = Math.floor(from / bucketMs) * bucketMs;
    const byTs = new Map(d.series.map((p) => [new Date(p.ts).getTime(), p]));
    const pad = (n: number) => String(n).padStart(2, '0');
    const fmt = (ms: number) => {
      const dt = new Date(ms);
      return d.bucket_seconds >= 86400
        ? `${pad(dt.getMonth() + 1)}-${pad(dt.getDate())}`
        : `${pad(dt.getHours())}:${pad(dt.getMinutes())}`;
    };
    const labels: string[] = [];
    const calls: number[] = [];
    const errors: number[] = [];
    for (let ms = start; ms <= to; ms += bucketMs) {
      const p = byTs.get(ms);
      labels.push(fmt(ms));
      calls.push(p ? p.calls : 0);
      errors.push(p ? p.errors : 0);
    }
    lineChart.setOption({
      tooltip: { trigger: 'axis' },
      legend: { data: [t('analytics.calls'), t('analytics.errors')] },
      grid: { left: 48, right: 24, top: 32, bottom: 28 },
      xAxis: { type: 'category', data: labels, boundaryGap: false },
      yAxis: { type: 'value', minInterval: 1 },
      series: [
        {
          name: t('analytics.calls'),
          type: 'line',
          areaStyle: { opacity: 0.08 },
          showSymbol: false,
          data: calls,
        },
        {
          name: t('analytics.errors'),
          type: 'line',
          showSymbol: false,
          itemStyle: { color: '#e34d59' },
          data: errors,
        },
      ],
    });
  }
  if (pieChart) {
    const entries = Object.entries(d.totals.by_code).sort((a, b) => b[1] - a[1]);
    pieChart.setOption({
      tooltip: { trigger: 'item', formatter: '{b}: {c} ({d}%)' },
      legend: { bottom: 0, type: 'scroll' },
      series: [
        {
          type: 'pie',
          radius: ['45%', '70%'],
          center: ['50%', '45%'],
          data: entries.map(([name, value]) => ({ name, value })),
          label: { formatter: '{b}\n{d}%' },
        },
      ],
    });
  }
}

function onResize() {
  lineChart?.resize();
  pieChart?.resize();
}

async function reload() {
  await analytics.fetch(orgId.value, projectId.value);
}

function onRangeChange(v: string) {
  analytics.range = v;
  reload();
}

// When data changes, wait for the DOM (the chart section is v-if'd on hasData),
// lazily init the charts, then draw.
watch(data, async () => {
  await nextTick();
  await ensureCharts();
  renderCharts();
});

onMounted(async () => {
  window.addEventListener('resize', onResize);
  await reload();
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', onResize);
  lineChart?.dispose();
  pieChart?.dispose();
});
</script>

<template>
  <div class="analytics-page">
    <t-breadcrumb class="breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{
        t('breadcrumb.home')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{
        t('breadcrumb.projects')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ project?.name }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('projectNav.analytics') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <div class="header">
      <div>
        <h2 class="title">{{ t('analytics.title') }}</h2>
        <p class="subtitle">{{ t('analytics.subtitle') }}</p>
      </div>
      <div class="controls">
        <t-radio-group variant="default-filled" :value="analytics.range" @change="onRangeChange">
          <t-radio-button v-for="o in rangeOptions" :key="o.value" :value="o.value">
            {{ o.label }}
          </t-radio-button>
        </t-radio-group>
        <t-button theme="default" variant="outline" :loading="analytics.loading" @click="reload">
          {{ t('analytics.refresh') }}
        </t-button>
      </div>
    </div>

    <!-- Totals -->
    <div class="stat-row">
      <t-card class="stat" :bordered="true">
        <div class="stat-label">{{ t('analytics.totalCalls') }}</div>
        <div class="stat-value">{{ Math.round(data?.totals.calls ?? 0).toLocaleString() }}</div>
      </t-card>
      <t-card class="stat" :bordered="true">
        <div class="stat-label">{{ t('analytics.errorRate') }}</div>
        <div class="stat-value" :class="{ danger: (data?.totals.error_rate ?? 0) > 0 }">
          {{ fmtPct(data?.totals.error_rate ?? 0) }}
        </div>
      </t-card>
      <t-card class="stat" :bordered="true">
        <div class="stat-label">{{ t('analytics.avgLatency') }}</div>
        <div class="stat-value">{{ fmtMs(data?.totals.avg_latency_ms ?? 0) }}</div>
      </t-card>
      <t-card class="stat" :bordered="true">
        <div class="stat-label">{{ t('analytics.rulesetsActive') }}</div>
        <div class="stat-value">{{ data?.rulesets.length ?? 0 }}</div>
      </t-card>
    </div>

    <!-- Error state — a failed load must not masquerade as "no traffic yet" -->
    <t-card v-if="!analytics.loading && analytics.error" class="empty" :bordered="true">
      <p class="empty-title">{{ t('analytics.errorTitle') }}</p>
      <p class="empty-hint">{{ analytics.error }}</p>
      <t-button class="empty-action" theme="default" variant="outline" @click="reload">
        {{ t('analytics.refresh') }}
      </t-button>
    </t-card>

    <!-- Empty state -->
    <t-card v-else-if="!analytics.loading && !hasData" class="empty" :bordered="true">
      <p class="empty-title">{{ t('analytics.emptyTitle') }}</p>
      <p class="empty-hint">{{ t('analytics.emptyHint') }}</p>
    </t-card>

    <template v-else>
      <t-card class="chart-card" :title="t('analytics.trafficOverTime')" :bordered="true">
        <div ref="lineEl" class="chart line"></div>
      </t-card>

      <div class="two-col">
        <t-card class="chart-card" :title="t('analytics.decisionDistribution')" :bordered="true">
          <div ref="pieEl" class="chart pie"></div>
        </t-card>

        <t-card class="chart-card" :title="t('analytics.byRuleset')" :bordered="true">
          <t-table
            row-key="ruleset"
            :data="data?.rulesets ?? []"
            :columns="[
              { colKey: 'ruleset', title: t('analytics.ruleset') },
              { colKey: 'calls', title: t('analytics.calls'), width: 90 },
              { colKey: 'error_rate', title: t('analytics.errorRate'), width: 100 },
              { colKey: 'avg_latency_ms', title: t('analytics.avgLatency'), width: 110 },
            ]"
          >
            <template #calls="{ row }">{{ Math.round(row.calls).toLocaleString() }}</template>
            <template #error_rate="{ row }">{{ fmtPct(row.error_rate) }}</template>
            <template #avg_latency_ms="{ row }">{{ fmtMs(row.avg_latency_ms) }}</template>
          </t-table>
        </t-card>
      </div>
    </template>
  </div>
</template>

<style scoped>
.analytics-page {
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
  padding: 16px 24px 32px;
}
.breadcrumb {
  margin-bottom: 16px;
}
.header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 16px;
  gap: 16px;
}
.title {
  margin: 0;
  font-size: 20px;
}
.subtitle {
  margin: 4px 0 0;
  color: var(--td-text-color-secondary);
  font-size: 13px;
}
.controls {
  display: flex;
  gap: 12px;
  align-items: center;
}
.stat-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-bottom: 16px;
}
.stat-label {
  color: var(--td-text-color-secondary);
  font-size: 13px;
}
.stat-value {
  font-size: 26px;
  font-weight: 600;
  margin-top: 4px;
}
.stat-value.danger {
  color: var(--td-error-color);
}
.chart-card {
  margin-bottom: 16px;
}
.chart {
  width: 100%;
}
.chart.line {
  height: 300px;
}
.chart.pie {
  height: 300px;
}
.two-col {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}
.empty {
  text-align: center;
  padding: 48px 16px;
}
.empty-title {
  font-size: 16px;
  font-weight: 600;
  margin: 0 0 8px;
}
.empty-hint {
  color: var(--td-text-color-secondary);
  margin: 0;
}
.empty-action {
  margin-top: 16px;
}
@media (max-width: 900px) {
  .stat-row {
    grid-template-columns: repeat(2, 1fr);
  }
  .two-col {
    grid-template-columns: 1fr;
  }
}
</style>
