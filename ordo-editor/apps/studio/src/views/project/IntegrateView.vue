<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin } from 'tdesign-vue-next';
import { useProjectStore } from '@/stores/project';
import { useServerStore } from '@/stores/server';

const { t, locale } = useI18n();
const route = useRoute();
const router = useRouter();
const projectStore = useProjectStore();
const serverStore = useServerStore();

const orgId = computed(() => route.params.orgId as string);
const projectId = computed(() => route.params.projectId as string);
const project = computed(() => projectStore.currentProject);

// Project id IS the ordo-server tenant id (see Project type).
const tenantId = computed(() => project.value?.id ?? projectId.value);

// ── Rulesets ──────────────────────────────────────────────────────────────
const metas = computed(() => projectStore.draftMetas);
const selectedName = ref<string>('');

const rulesetOptions = computed(() => metas.value.map((m) => ({ label: m.name, value: m.name })));

const selectedMeta = computed(() => metas.value.find((m) => m.name === selectedName.value) ?? null);
const isPublished = computed(() => !!selectedMeta.value?.published_version);
const rulesetName = computed(() => selectedName.value || 'my-ruleset');

function pickDefaultRuleset() {
  if (!metas.value.length) {
    selectedName.value = '';
    return;
  }
  if (selectedName.value && metas.value.some((m) => m.name === selectedName.value)) return;
  const published = metas.value.find((m) => m.published_version);
  selectedName.value = (published ?? metas.value[0]).name;
}

// ── Engine host ───────────────────────────────────────────────────────────
const boundServer = computed(() =>
  project.value?.server_id ? serverStore.getById(project.value.server_id) ?? null : null
);
const engineBase = ref('');

const engineForSnippet = computed(() => {
  const raw = engineBase.value.trim() || 'https://<your-ordo-engine>';
  return raw.replace(/\/+$/, '');
});

// Platform API origin (for the proxy path) — mirrors the app→api host rewrite.
const platformOrigin = computed(() => {
  if (typeof window === 'undefined') return 'https://<platform>';
  const { protocol, host, origin } = window.location;
  if (host.startsWith('app.')) return `${protocol}//api.${host.slice(4)}`;
  return origin;
});

// ── Sample input ──────────────────────────────────────────────────────────
const sampleInput = ref('{\n  "amount": 5000\n}');
const compactInput = computed<string | null>(() => {
  try {
    return JSON.stringify(JSON.parse(sampleInput.value));
  } catch {
    return null;
  }
});
const inputValid = computed(() => compactInput.value !== null);
const effectiveJson = computed(() => compactInput.value ?? (sampleInput.value.trim() || '{}'));

// ── Transport + language ──────────────────────────────────────────────────
const transport = ref<'direct' | 'proxy'>('direct');
const lang = ref<'curl' | 'node' | 'python' | 'go'>('curl');

const langs = computed(() =>
  transport.value === 'direct'
    ? (['curl', 'node', 'python', 'go'] as const)
    : (['curl', 'node'] as const)
);
const langLabels: Record<string, string> = {
  curl: 'cURL',
  node: 'Node.js',
  python: 'Python',
  go: 'Go',
};

watch(transport, () => {
  if (!langs.value.includes(lang.value as never)) lang.value = 'curl';
});

// ── Snippet generation ──────────────────────────────────────────────────────
function directSnippet(l: string): string {
  const E = engineForSnippet.value;
  const T = tenantId.value;
  const N = rulesetName.value;
  const J = effectiveJson.value;
  switch (l) {
    case 'curl':
      return `curl -X POST ${E}/api/v1/execute/${N} \\
  -H 'content-type: application/json' \\
  -H 'x-tenant-id: ${T}' \\
  -d '{"input": ${J}}'`;
    case 'node':
      return `const res = await fetch("${E}/api/v1/execute/${N}", {
  method: "POST",
  headers: {
    "content-type": "application/json",
    "x-tenant-id": "${T}",
  },
  body: JSON.stringify({ input: ${J} }),
});
const decision = await res.json();
console.log(decision.code, decision.output);`;
    case 'python':
      return `import json
from ordo import OrdoClient

client = OrdoClient(http_address="${E}", tenant_id="${T}")
result = client.execute("${N}", json.loads('${J}'))
print(result.code, result.output)`;
    case 'go':
      return `import (
    "context"
    "encoding/json"
    "fmt"

    "github.com/pama-lee/ordo-go/ordo"
)

client, _ := ordo.NewClient(
    ordo.WithHTTPAddress("${E}"),
    ordo.WithTenantID("${T}"),
)

var input map[string]any
_ = json.Unmarshal([]byte(\`${J}\`), &input)

result, _ := client.Execute(context.Background(), "${N}", input)
fmt.Printf("%s %s\\n", result.Code, result.Output)`;
    default:
      return '';
  }
}

function proxySnippet(l: string): string {
  const P = platformOrigin.value;
  const PID = tenantId.value;
  const N = rulesetName.value;
  const J = effectiveJson.value;
  switch (l) {
    case 'curl':
      return `curl -X POST ${P}/api/v1/engine/${PID}/execute/${N} \\
  -H 'content-type: application/json' \\
  -H 'authorization: Bearer <token>' \\
  -d '{"input": ${J}}'`;
    case 'node':
      return `const res = await fetch("${P}/api/v1/engine/${PID}/execute/${N}", {
  method: "POST",
  headers: {
    "content-type": "application/json",
    "authorization": \`Bearer \${token}\`,
  },
  body: JSON.stringify({ input: ${J} }),
});
const decision = await res.json();
console.log(decision.code, decision.output);`;
    default:
      return '';
  }
}

const snippet = computed(() =>
  transport.value === 'direct' ? directSnippet(lang.value) : proxySnippet(lang.value)
);

// ── Docs link ───────────────────────────────────────────────────────────────
const docsUrl = computed(() => {
  const l = String(locale.value).startsWith('zh') ? 'zh' : 'en';
  return `https://docs.ordoengine.com/${l}/platform/integrate`;
});

// ── Actions ─────────────────────────────────────────────────────────────────
async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    MessagePlugin.success(t('integrate.copied'));
  } catch {
    MessagePlugin.error(t('integrate.copyFailed'));
  }
}

async function refresh() {
  await projectStore.fetchRulesets();
  pickDefaultRuleset();
}

onMounted(async () => {
  await Promise.all([projectStore.fetchRulesets(), serverStore.fetchServers()]);
  pickDefaultRuleset();
  if (boundServer.value) engineBase.value = boundServer.value.url;
});

watch(metas, pickDefaultRuleset);
watch(boundServer, (s) => {
  if (s && !engineBase.value) engineBase.value = s.url;
});
</script>

<template>
  <div class="integrate-page">
    <t-breadcrumb class="breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{
        t('breadcrumb.home')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{
        t('breadcrumb.projects')
      }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ project?.name }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('integrate.title') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <h1 class="page-title">{{ t('integrate.title') }}</h1>
    <p class="page-subtitle">{{ t('integrate.subtitle') }}</p>

    <div class="integrate-layout">
      <!-- This project -->
      <t-card :bordered="false" class="card">
        <div class="field">
          <label class="field-label">{{ t('integrate.tenantLabel') }}</label>
          <div class="copy-row">
            <code class="inline-code">{{ tenantId }}</code>
            <t-button variant="text" size="small" theme="primary" @click="copyText(tenantId)">
              {{ t('integrate.copy') }}
            </t-button>
          </div>
          <p class="field-hint">{{ t('integrate.tenantHint') }}</p>
        </div>

        <div class="field">
          <div class="field-label-row">
            <label class="field-label">{{ t('integrate.rulesetLabel') }}</label>
            <t-button variant="text" size="small" @click="refresh">{{
              t('integrate.refresh')
            }}</t-button>
          </div>
          <div v-if="metas.length" class="ruleset-row">
            <t-select v-model="selectedName" :options="rulesetOptions" style="max-width: 320px" />
            <t-tag v-if="isPublished" theme="success" variant="light">{{
              t('integrate.publishedTag')
            }}</t-tag>
            <t-tag v-else theme="warning" variant="light">{{ t('integrate.draftTag') }}</t-tag>
          </div>
          <p v-else class="field-hint">{{ t('integrate.noRulesets') }}</p>
          <t-alert
            v-if="metas.length && !isPublished"
            theme="warning"
            :message="t('integrate.notPublishedWarn')"
            class="publish-warn"
          />
        </div>

        <div class="field">
          <label class="field-label">{{ t('integrate.engineLabel') }}</label>
          <t-input
            v-model="engineBase"
            placeholder="https://<your-ordo-engine>"
            style="max-width: 420px"
          />
          <p class="field-hint">
            {{ boundServer ? t('integrate.engineBoundHint') : t('integrate.engineDefaultHint') }}
          </p>
        </div>
      </t-card>

      <!-- Sample input -->
      <t-card :bordered="false" class="card">
        <div class="field-label-row">
          <label class="field-label">{{ t('integrate.sampleLabel') }}</label>
          <t-tag v-if="!inputValid" theme="danger" variant="light">{{
            t('integrate.invalidJson')
          }}</t-tag>
        </div>
        <t-textarea
          v-model="sampleInput"
          :autosize="{ minRows: 3, maxRows: 10 }"
          class="input-area"
        />
        <p class="field-hint">{{ t('integrate.sampleHint') }}</p>
      </t-card>

      <!-- Call it -->
      <t-card :bordered="false" class="card">
        <h2 class="card-title">{{ t('integrate.codeTitle') }}</h2>

        <t-radio-group v-model="transport" variant="default-filled" size="small" class="transport">
          <t-radio-button value="direct">{{ t('integrate.directTab') }}</t-radio-button>
          <t-radio-button value="proxy">{{ t('integrate.proxyTab') }}</t-radio-button>
        </t-radio-group>
        <p class="field-hint transport-desc">
          {{ transport === 'direct' ? t('integrate.directDesc') : t('integrate.proxyDesc') }}
        </p>

        <t-tabs v-model="lang" class="lang-tabs">
          <t-tab-panel v-for="l in langs" :key="l" :value="l" :label="langLabels[l]" />
        </t-tabs>

        <div class="code-block">
          <t-button
            class="code-copy"
            variant="text"
            size="small"
            theme="primary"
            @click="copyText(snippet)"
          >
            {{ t('integrate.copy') }}
          </t-button>
          <pre class="code-pre">{{ snippet }}</pre>
        </div>

        <p class="field-hint">{{ t('integrate.responseNote') }}</p>

        <a class="docs-link" :href="docsUrl" target="_blank" rel="noopener">
          <t-icon name="book" size="14px" />
          {{ t('integrate.docsLink') }}
        </a>
      </t-card>
    </div>
  </div>
</template>

<style scoped>
.integrate-page {
  padding: 32px;
  overflow-y: auto;
  height: 100%;
}

.breadcrumb {
  margin-bottom: 16px;
}

.page-title {
  margin: 0 0 6px;
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.page-subtitle {
  margin: 0 0 24px;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.integrate-layout {
  display: flex;
  flex-direction: column;
  gap: 20px;
  max-width: 760px;
}

.card {
  background: var(--ordo-bg-panel);
  border-radius: 8px;
}

.card-title {
  margin: 0 0 16px;
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.field {
  margin-bottom: 20px;
}

.field:last-child {
  margin-bottom: 0;
}

.field-label {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
  margin-bottom: 8px;
}

.field-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.field-hint {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.copy-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.inline-code {
  padding: 4px 10px;
  border-radius: 6px;
  background: var(--ordo-bg-secondary);
  font-family: var(--ordo-font-mono, 'JetBrains Mono', monospace);
  font-size: 13px;
  color: var(--ordo-text-primary);
  word-break: break-all;
}

.ruleset-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.publish-warn {
  margin-top: 12px;
}

.input-area :deep(textarea) {
  font-family: var(--ordo-font-mono, 'JetBrains Mono', monospace);
  font-size: 13px;
}

.transport {
  margin-bottom: 4px;
}

.transport-desc {
  margin-top: 8px;
  margin-bottom: 4px;
}

.lang-tabs {
  margin-top: 8px;
}

.code-block {
  position: relative;
  margin-top: 12px;
}

.code-copy {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 1;
}

.code-pre {
  margin: 0;
  padding: 16px;
  padding-right: 72px;
  border-radius: 8px;
  background: var(--ordo-bg-secondary);
  font-family: var(--ordo-font-mono, 'JetBrains Mono', monospace);
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--ordo-text-primary);
  white-space: pre-wrap;
  word-break: break-word;
  overflow-x: auto;
}

.docs-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 16px;
  font-size: 13px;
  color: var(--ordo-accent);
  text-decoration: none;
}

.docs-link:hover {
  text-decoration: underline;
}
</style>
