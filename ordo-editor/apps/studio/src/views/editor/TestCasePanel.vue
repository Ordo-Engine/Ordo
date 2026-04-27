<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next';
import { useTestStore } from '@/stores/test';
import { useCatalogStore } from '@/stores/catalog';
import { useAuthStore } from '@/stores/auth';
import { testApi } from '@/api/platform-client';
import TraceStepTree from './TraceStepTree.vue';
import type {
  TestCase,
  TestCaseInput,
  TestExecutionTraceStep,
  TestFailureDetail,
  TestRunResult,
} from '@/api/types';
import type { RuleSet } from '@ordo-engine/editor-core';

const props = defineProps<{
  projectId: string;
  rulesetName: string;
  visible: boolean;
  height: number;
  ruleset?: RuleSet | null;
  subRuleMode?: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:visible', v: boolean): void;
  (e: 'update:height', v: number): void;
  (
    e: 'show-in-flow',
    trace: {
      path: string[];
      steps: Array<{ id: string; name: string; duration_us: number; result?: string | null }>;
      resultCode: string;
      resultMessage: string;
      output?: Record<string, any>;
    }
  ): void;
  (
    e: 'open-sub-rule-trace',
    payload: {
      refName: string;
      trace: {
        path: string[];
        steps: Array<{ id: string; name: string; duration_us: number; result?: string | null }>;
        resultCode: string;
        resultMessage: string;
        output?: Record<string, any>;
      };
    }
  ): void;
}>();

const { t } = useI18n();
const route = useRoute();
const testStore = useTestStore();
const catalog = useCatalogStore();
const auth = useAuthStore();
const orgId = computed(() => route.params.orgId as string);

// ── State ────────────────────────────────────────────────────────────────────

const expandedId = ref<string | null>(null);
const editingId = ref<string | null>(null);
const showEditor = ref(false);
const form = ref<TestCaseInput>(emptyForm());
const inputJson = ref('{}');
const expectOutputJson = ref('');
const saving = ref(false);
const jsonError = ref('');
const probeInputJson = ref('{}');
const probeExpectCode = ref('');
const probeExpectMessage = ref('');
const probeExpectOutputJson = ref('');
const probeJsonError = ref<'input' | 'output' | ''>('');
const probeRunning = ref(false);
const probeResult = ref<TestRunResult | null>(null);

function emptyForm(): TestCaseInput {
  return {
    name: '',
    description: undefined,
    input: {},
    expect: { code: undefined, message: undefined, output: undefined },
    tags: [],
  };
}

const tests = computed(() => testStore.testsByRuleset.get(props.rulesetName) ?? []);
const results = computed(() => testStore.runResults.get(props.rulesetName) ?? []);
const running = computed(() => testStore.running);
const runningOne = computed(() => testStore.runningOne);
const loading = computed(() => testStore.loadingRuleset.get(props.rulesetName) ?? false);
const isSubRuleMode = computed(() => props.subRuleMode === true);

const passCount = computed(() => results.value.filter((r) => r.passed).length);
const failCount = computed(() => results.value.filter((r) => !r.passed).length);
const hasResults = computed(() => results.value.length > 0);
const probePassCount = computed(() => (probeResult.value?.passed ? 1 : 0));
const probeFailCount = computed(() => (probeResult.value && !probeResult.value.passed ? 1 : 0));
const hasProbeResult = computed(() => !!probeResult.value);

function resultFor(id: string): TestRunResult | undefined {
  return results.value.find((r) => r.test_id === id);
}

function toggleExpand(id: string) {
  expandedId.value = expandedId.value === id ? null : id;
}

// ── Load tests ────────────────────────────────────────────────────────────────

watch(
  () => [props.projectId, props.rulesetName],
  ([pid, name]) => {
    if (pid && name && !isSubRuleMode.value) testStore.fetchTests(pid as string, name as string);
  },
  { immediate: true }
);

watch(
  () => [props.rulesetName, props.subRuleMode],
  () => {
    probeResult.value = null;
    probeJsonError.value = '';
    if (isSubRuleMode.value) seedSubRuleProbeInput();
  },
  { immediate: true }
);

// ── Resize ────────────────────────────────────────────────────────────────────

let dragStartY = 0;
let dragStartH = 0;

function onResizeMousedown(e: MouseEvent) {
  dragStartY = e.clientY;
  dragStartH = props.height;
  window.addEventListener('mousemove', onResizeMousemove);
  window.addEventListener('mouseup', onResizeMouseup);
}
function onResizeMousemove(e: MouseEvent) {
  emit('update:height', Math.max(200, Math.min(640, dragStartH + dragStartY - e.clientY)));
}
function onResizeMouseup() {
  window.removeEventListener('mousemove', onResizeMousemove);
  window.removeEventListener('mouseup', onResizeMouseup);
}

// ── Actions ───────────────────────────────────────────────────────────────────

async function runAll() {
  try {
    await testStore.runTests(orgId.value, props.projectId, props.rulesetName);
    MessagePlugin.success(t('test.runSuccess'));
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('test.saveFailed'));
  }
}

async function runOne(tc: TestCase) {
  try {
    await testStore.runOneTest(orgId.value, props.projectId, props.rulesetName, tc.id);
    // Auto-expand to show result
    expandedId.value = tc.id;
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('test.saveFailed'));
  }
}

function defaultValueForSchemaField(field: any): unknown {
  if (field?.defaultValue !== undefined) return field.defaultValue;
  switch (field?.type) {
    case 'number':
      return 0;
    case 'boolean':
      return false;
    case 'array':
      return [];
    case 'object':
      return seedObjectFromSchema(field.fields ?? []);
    default:
      return '';
  }
}

function seedObjectFromSchema(fields: any[]): Record<string, unknown> {
  const skeleton: Record<string, unknown> = {};
  for (const field of fields) {
    if (!field?.name) continue;
    skeleton[field.name] = defaultValueForSchemaField(field);
  }
  return skeleton;
}

function seedSubRuleProbeInput() {
  const fields = props.ruleset?.config.inputSchema ?? [];
  probeInputJson.value = JSON.stringify(seedObjectFromSchema(fields), null, 2);
}

async function runSubRuleProbe() {
  if (!auth.token || !props.ruleset) return;

  probeJsonError.value = '';
  let input: Record<string, unknown>;
  try {
    input = JSON.parse(probeInputJson.value || '{}');
  } catch {
    probeJsonError.value = 'input';
    return;
  }

  const expect: TestCaseInput['expect'] = {};
  if (probeExpectCode.value.trim()) expect.code = probeExpectCode.value.trim();
  if (probeExpectMessage.value.trim()) expect.message = probeExpectMessage.value.trim();
  if (probeExpectOutputJson.value.trim()) {
    try {
      expect.output = JSON.parse(probeExpectOutputJson.value);
    } catch {
      probeJsonError.value = 'output';
      return;
    }
  }

  probeRunning.value = true;
  try {
    probeResult.value = await testApi.runAdHoc(auth.token, props.projectId, {
      ruleset: props.ruleset,
      name: `${props.rulesetName} probe`,
      input,
      expect,
      include_trace: true,
    });
    MessagePlugin.success(
      probeResult.value.passed ? t('test.runSuccess') : t('test.subRuleProbe.runFinished')
    );
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('test.saveFailed'));
  } finally {
    probeRunning.value = false;
  }
}

function doExport(format: 'yaml' | 'json') {
  if (!auth.token) return;
  const url = `/api/v1/projects/${props.projectId}/rulesets/${encodeURIComponent(
    props.rulesetName
  )}/tests/export?format=${format}`;
  fetch(url, { headers: { Authorization: `Bearer ${auth.token}` } })
    .then((r) => r.blob())
    .then((blob) => {
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = `${props.rulesetName}_tests.${format}`;
      a.click();
      URL.revokeObjectURL(a.href);
    });
}

// ── CRUD ─────────────────────────────────────────────────────────────────────

function openCreate() {
  editingId.value = null;
  form.value = emptyForm();
  inputJson.value = '{}';
  expectOutputJson.value = '';
  jsonError.value = '';
  showEditor.value = true;
  expandedId.value = null;
}

function openEdit(tc: TestCase) {
  editingId.value = tc.id;
  form.value = {
    name: tc.name,
    description: tc.description,
    input: tc.input,
    expect: { ...tc.expect },
    tags: [...tc.tags],
  };
  inputJson.value = JSON.stringify(tc.input, null, 2);
  expectOutputJson.value = tc.expect.output ? JSON.stringify(tc.expect.output, null, 2) : '';
  jsonError.value = '';
  showEditor.value = true;
}

function cancelEdit() {
  showEditor.value = false;
}

async function saveTest() {
  jsonError.value = '';
  const name = form.value.name.trim();
  if (!name) {
    MessagePlugin.warning(t('test.nameRequired'));
    return;
  }

  try {
    form.value.input = JSON.parse(inputJson.value || '{}');
  } catch {
    jsonError.value = 'input';
    return;
  }

  if (expectOutputJson.value.trim()) {
    try {
      form.value.expect.output = JSON.parse(expectOutputJson.value);
    } catch {
      jsonError.value = 'output';
      return;
    }
  } else {
    form.value.expect.output = undefined;
  }

  saving.value = true;
  try {
    if (editingId.value) {
      await testStore.updateTest(props.projectId, props.rulesetName, editingId.value, form.value);
      MessagePlugin.success(t('test.updateSuccess'));
    } else {
      await testStore.createTest(props.projectId, props.rulesetName, form.value);
      MessagePlugin.success(t('test.createSuccess'));
    }
    showEditor.value = false;
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('test.saveFailed'));
  } finally {
    saving.value = false;
  }
}

async function deleteTest(tc: TestCase) {
  const dialog = DialogPlugin.confirm({
    header: t('test.deleteDialog'),
    body: t('test.deleteConfirm', { name: tc.name }),
    confirmBtn: t('test.deleteCase'),
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await testStore.deleteTest(props.projectId, props.rulesetName, tc.id);
        if (expandedId.value === tc.id) expandedId.value = null;
        MessagePlugin.success(t('test.deleteSuccess'));
      } catch (e: any) {
        MessagePlugin.error(e?.message ?? t('common.saveFailed'));
      } finally {
        dialog.hide();
      }
    },
    onClose: () => dialog.hide(),
  });
}

function generateFromContract() {
  const contract = catalog.contracts.find((c) => c.ruleset_name === props.rulesetName);
  if (!contract) return;
  const skeleton: Record<string, unknown> = {};
  for (const field of contract.input_fields) {
    switch (field.data_type) {
      case 'number':
        skeleton[field.name] = 0;
        break;
      case 'boolean':
        skeleton[field.name] = false;
        break;
      case 'date':
        skeleton[field.name] = new Date().toISOString().slice(0, 10);
        break;
      case 'object':
        skeleton[field.name] = {};
        break;
      default:
        skeleton[field.name] = '';
    }
  }
  inputJson.value = JSON.stringify(skeleton, null, 2);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function fmtJson(v: unknown): string {
  return JSON.stringify(v, null, 2);
}

function durationMs(us: number): string {
  return (us / 1000).toFixed(1) + 'ms';
}

function buildFlowTrace(
  result: TestRunResult,
  steps: TestExecutionTraceStep[] = result.trace?.steps ?? []
) {
  return {
    path: steps.map((step) => step.id),
    steps: steps.map((step) => ({
      id: step.id,
      name: step.name,
      duration_us: step.duration_us,
      result: step.is_terminal ? result.actual_code ?? result.trace?.result_code ?? null : null,
    })),
    resultCode: result.actual_code ?? result.trace?.result_code ?? '',
    resultMessage: result.actual_message ?? '',
    output: result.actual_output as Record<string, any> | undefined,
  };
}

function showResultInFlow(result: TestRunResult) {
  if (!result.trace) return;
  emit('show-in-flow', buildFlowTrace(result));
}

function openSubRuleTrace(result: TestRunResult, step: TestExecutionTraceStep) {
  if (!step.sub_rule_ref || !step.sub_rule_frames?.length) return;
  emit('open-sub-rule-trace', {
    refName: step.sub_rule_ref,
    trace: buildFlowTrace(result, step.sub_rule_frames),
  });
}

function failureDetailFor(result: TestRunResult, index: number): TestFailureDetail | null {
  return result.failure_details?.[index] ?? null;
}

function failureKind(message: string, detail?: TestFailureDetail | null) {
  if (detail?.kind) {
    return detail.kind === 'sub_rule' ? 'subRule' : detail.kind;
  }
  const lower = message.toLowerCase();
  if (lower.includes('sub-rule') && lower.includes('not found')) return 'reference';
  if (lower.includes('contract') || lower.includes('schema')) return 'contract';
  if (lower.includes('field not found') || lower.includes('evaluation error')) return 'binding';
  if (lower.includes('output') || lower.includes('write')) return 'output';
  if (lower.includes('invalid test input')) return 'execution';
  if (lower.includes('execution failed')) return 'subRule';
  return 'assertion';
}

function failureKindLabel(message: string, detail?: TestFailureDetail | null) {
  return t(`test.trace.failureKinds.${failureKind(message, detail)}`);
}
</script>

<template>
  <div class="test-panel" :style="{ height: height + 'px' }">
    <!-- Drag handle -->
    <div class="test-panel__handle" @mousedown="onResizeMousedown" />

    <!-- Header bar -->
    <div class="test-panel__header">
      <div class="test-panel__title">
        <t-icon name="task-checked" size="13px" />
        {{ t('test.panel.title') }}
        <span v-if="rulesetName" class="ruleset-badge">{{ rulesetName }}</span>
      </div>

      <div class="summary-badges" v-if="!isSubRuleMode && hasResults">
        <span class="badge badge--pass">✓ {{ passCount }}</span>
        <span class="badge badge--fail" v-if="failCount > 0">✗ {{ failCount }}</span>
      </div>
      <div class="summary-badges" v-else-if="isSubRuleMode && hasProbeResult">
        <span class="badge badge--pass" v-if="probePassCount">✓ {{ probePassCount }}</span>
        <span class="badge badge--fail" v-if="probeFailCount">✗ {{ probeFailCount }}</span>
      </div>

      <div class="test-panel__actions">
        <template v-if="isSubRuleMode">
          <t-button
            size="small"
            variant="outline"
            :disabled="!ruleset"
            @click="seedSubRuleProbeInput"
          >
            <t-icon name="refresh" size="12px" />{{ t('test.subRuleProbe.seed') }}
          </t-button>
          <t-button
            size="small"
            variant="outline"
            :loading="probeRunning"
            :disabled="!ruleset"
            @click="runSubRuleProbe"
          >
            <t-icon name="play-circle" size="12px" />{{ t('test.subRuleProbe.run') }}
          </t-button>
        </template>
        <template v-else>
          <t-button size="small" variant="outline" @click="openCreate" :disabled="!rulesetName">
            <t-icon name="add" size="12px" />{{ t('test.newCase') }}
          </t-button>
          <t-button
            size="small"
            variant="outline"
            :loading="running"
            :disabled="!rulesetName || tests.length === 0"
            @click="runAll"
          >
            <t-icon name="play-circle" size="12px" />{{ t('test.runAll') }}
          </t-button>
          <t-dropdown trigger="click">
            <t-button size="small" variant="outline" :disabled="!rulesetName || tests.length === 0">
              <t-icon name="download" size="12px" />{{ t('test.exportYaml') }}
            </t-button>
            <template #dropdown>
              <t-dropdown-menu>
                <t-dropdown-item @click="doExport('yaml')">{{
                  t('test.export.yaml')
                }}</t-dropdown-item>
                <t-dropdown-item @click="doExport('json')">{{
                  t('test.export.json')
                }}</t-dropdown-item>
              </t-dropdown-menu>
            </template>
          </t-dropdown>
        </template>
      </div>

      <button class="close-btn" @click="emit('update:visible', false)">
        <t-icon name="close" size="13px" />
      </button>
    </div>

    <!-- No ruleset -->
    <div v-if="!rulesetName" class="panel-empty">
      <t-icon name="task-checked" size="32px" style="opacity: 0.25" />
      <p>{{ t('test.panel.noRuleset') }}</p>
    </div>

    <!-- Content -->
    <div v-else class="test-panel__body">
      <div v-if="isSubRuleMode" class="sub-rule-probe">
        <div class="sub-rule-probe__editor">
          <div class="sub-rule-probe__intro">
            <div class="sub-rule-probe__eyebrow">{{ t('test.subRuleProbe.title') }}</div>
            <p>{{ t('test.subRuleProbe.desc') }}</p>
          </div>

          <div class="form-row">
            <div class="form-label-row">
              <label class="form-label">{{ t('test.fieldInput') }}</label>
              <t-button
                size="small"
                variant="text"
                style="font-size: 11px; padding: 0"
                @click="seedSubRuleProbeInput"
              >
                {{ t('test.subRuleProbe.seed') }}
              </t-button>
            </div>
            <div class="json-wrap" :class="{ 'has-error': probeJsonError === 'input' }">
              <textarea v-model="probeInputJson" class="json-editor" rows="8" spellcheck="false" />
            </div>
          </div>

          <div class="sub-rule-probe__expect-grid">
            <div class="form-row">
              <label class="form-label">{{ t('test.fieldExpectCode') }}</label>
              <t-input v-model="probeExpectCode" size="small" placeholder="optional" />
            </div>
            <div class="form-row">
              <label class="form-label">{{ t('test.fieldExpectMessage') }}</label>
              <t-input v-model="probeExpectMessage" size="small" placeholder="optional" />
            </div>
          </div>

          <div class="form-row">
            <label class="form-label">{{ t('test.fieldExpectOutput') }}</label>
            <div class="json-wrap" :class="{ 'has-error': probeJsonError === 'output' }">
              <textarea
                v-model="probeExpectOutputJson"
                class="json-editor"
                rows="4"
                spellcheck="false"
                placeholder="optional"
              />
            </div>
          </div>

          <div class="form-actions">
            <t-button size="small" variant="outline" @click="seedSubRuleProbeInput">
              {{ t('test.subRuleProbe.seed') }}
            </t-button>
            <t-button
              size="small"
              theme="primary"
              :loading="probeRunning"
              :disabled="!ruleset"
              @click="runSubRuleProbe"
            >
              {{ t('test.subRuleProbe.run') }}
            </t-button>
          </div>
        </div>

        <div class="sub-rule-probe__result">
          <div v-if="!probeResult" class="panel-empty-small panel-empty-small--probe">
            <span>{{ t('test.subRuleProbe.emptyResult') }}</span>
          </div>

          <template v-else>
            <div
              class="detail-section detail-section--result"
              :class="probeResult.passed ? 'result--pass' : 'result--fail'"
            >
              <div class="detail-label">
                {{ probeResult.passed ? t('test.result.pass') : t('test.result.fail') }}
              </div>

              <div v-if="probeResult.failures.length" class="failures">
                <div v-for="(f, i) in probeResult.failures" :key="i" class="failure-line">
                  <t-icon name="close-circle" size="11px" class="failure-icon" />
                  <span class="failure-kind">{{
                    failureKindLabel(f, failureDetailFor(probeResult, i))
                  }}</span>
                  <span>{{ f }}</span>
                </div>
              </div>

              <div v-if="probeResult.actual_code" class="actual-row">
                <span class="expect-pill expect-pill--actual">
                  code: <strong>{{ probeResult.actual_code }}</strong>
                </span>
                <span v-if="probeResult.actual_message" class="expect-pill expect-pill--actual">
                  message: <strong>{{ probeResult.actual_message }}</strong>
                </span>
              </div>
              <pre v-if="probeResult.actual_output" class="detail-code detail-code--actual">{{
                fmtJson(probeResult.actual_output)
              }}</pre>
            </div>

            <div v-if="probeResult.trace" class="detail-section trace-section">
              <div class="trace-section__header">
                <div>
                  <div class="detail-label">{{ t('test.project.traceDetails') }}</div>
                  <div class="trace-section__meta">
                    <span>{{ t('test.project.path') }}: {{ probeResult.trace.path_string }}</span>
                    <span
                      >{{ t('test.project.totalDuration') }}:
                      {{ durationMs(probeResult.trace.total_duration_us) }}</span
                    >
                  </div>
                </div>
                <button class="trace-flow-btn" @click="showResultInFlow(probeResult)">
                  <t-icon name="flowchart" size="12px" />
                  {{ t('test.trace.showInFlow') }}
                </button>
              </div>

              <TraceStepTree
                :steps="probeResult.trace.steps"
                @open-sub-rule="(step) => openSubRuleTrace(probeResult!, step)"
              />
            </div>
          </template>
        </div>
      </div>

      <template v-else>
        <!-- Test list -->
        <div class="test-list" :class="{ 'test-list--narrow': showEditor }">
          <div v-if="loading" class="panel-loading"><t-loading size="small" /></div>

          <div v-else-if="tests.length === 0" class="panel-empty-small">
            <span>{{ t('test.noTests') }}</span>
            <t-button size="small" variant="text" @click="openCreate">{{
              t('test.addFirst')
            }}</t-button>
          </div>

          <template v-else>
            <div
              v-for="tc in tests"
              :key="tc.id"
              class="test-item"
              :class="{
                'test-item--pass': resultFor(tc.id)?.passed === true,
                'test-item--fail': resultFor(tc.id)?.passed === false,
                'test-item--expanded': expandedId === tc.id,
              }"
            >
              <!-- Row summary (always visible) -->
              <div class="test-item__row" @click="toggleExpand(tc.id)">
                <span class="status-icon">
                  <t-loading v-if="runningOne.has(tc.id)" size="small" />
                  <span v-else-if="!resultFor(tc.id)" class="dot dot--pending" />
                  <span v-else-if="resultFor(tc.id)?.passed" class="dot dot--pass">✓</span>
                  <span v-else class="dot dot--fail">✗</span>
                </span>

                <div class="test-item__info">
                  <span class="test-item__name">{{ tc.name }}</span>
                  <span v-if="tc.tags.length" class="test-item__tags">
                    <span v-for="tag in tc.tags" :key="tag" class="tag">{{ tag }}</span>
                  </span>
                </div>

                <span v-if="resultFor(tc.id)" class="test-item__duration">
                  {{ durationMs(resultFor(tc.id)!.duration_us) }}
                </span>

                <div class="test-item__btns" @click.stop>
                  <t-button
                    size="small"
                    variant="text"
                    :loading="runningOne.has(tc.id)"
                    @click="runOne(tc)"
                  >
                    <t-icon name="play-circle" size="12px" />
                  </t-button>
                  <t-button size="small" variant="text" @click="openEdit(tc)">
                    <t-icon name="edit" size="12px" />
                  </t-button>
                  <t-button size="small" variant="text" theme="danger" @click="deleteTest(tc)">
                    <t-icon name="delete" size="12px" />
                  </t-button>
                </div>

                <t-icon
                  :name="expandedId === tc.id ? 'chevron-up' : 'chevron-down'"
                  size="12px"
                  class="expand-chevron"
                />
              </div>

              <!-- Expanded detail -->
              <div v-if="expandedId === tc.id" class="test-item__detail">
                <!-- Input -->
                <div class="detail-section">
                  <div class="detail-label">{{ t('test.fieldInput') }}</div>
                  <pre class="detail-code">{{ fmtJson(tc.input) }}</pre>
                </div>

                <!-- Expect -->
                <div class="detail-section">
                  <div class="detail-label">{{ t('test.fieldExpectCode') }}</div>
                  <div class="expect-row">
                    <span class="expect-pill"
                      >code: <strong>{{ tc.expect.code ?? '—' }}</strong></span
                    >
                    <span v-if="tc.expect.message" class="expect-pill">
                      message: <strong>{{ tc.expect.message }}</strong>
                    </span>
                  </div>
                  <div v-if="tc.expect.output" class="detail-label" style="margin-top: 6px">
                    {{ t('test.fieldExpectOutput') }}
                  </div>
                  <pre v-if="tc.expect.output" class="detail-code">{{
                    fmtJson(tc.expect.output)
                  }}</pre>
                </div>

                <!-- Result (if run) -->
                <template v-if="resultFor(tc.id)">
                  <div
                    class="detail-section detail-section--result"
                    :class="resultFor(tc.id)!.passed ? 'result--pass' : 'result--fail'"
                  >
                    <div class="detail-label">
                      {{ resultFor(tc.id)!.passed ? t('test.result.pass') : t('test.result.fail') }}
                    </div>

                    <!-- Failures list -->
                    <div v-if="resultFor(tc.id)!.failures.length" class="failures">
                      <div
                        v-for="(f, i) in resultFor(tc.id)!.failures"
                        :key="i"
                        class="failure-line"
                      >
                        <t-icon name="close-circle" size="11px" class="failure-icon" />
                        <span class="failure-kind">{{
                          failureKindLabel(f, failureDetailFor(resultFor(tc.id)!, i))
                        }}</span>
                        <span>{{ f }}</span>
                      </div>
                    </div>

                    <!-- Actual output -->
                    <div v-if="resultFor(tc.id)!.actual_code" class="actual-row">
                      <span class="expect-pill expect-pill--actual">
                        code: <strong>{{ resultFor(tc.id)!.actual_code }}</strong>
                      </span>
                      <span
                        v-if="resultFor(tc.id)!.actual_message"
                        class="expect-pill expect-pill--actual"
                      >
                        message: <strong>{{ resultFor(tc.id)!.actual_message }}</strong>
                      </span>
                    </div>
                    <pre
                      v-if="resultFor(tc.id)!.actual_output"
                      class="detail-code detail-code--actual"
                      >{{ fmtJson(resultFor(tc.id)!.actual_output) }}</pre
                    >
                  </div>

                  <div v-if="resultFor(tc.id)!.trace" class="detail-section trace-section">
                    <div class="trace-section__header">
                      <div>
                        <div class="detail-label">{{ t('test.project.traceDetails') }}</div>
                        <div class="trace-section__meta">
                          <span
                            >{{ t('test.project.path') }}:
                            {{ resultFor(tc.id)!.trace!.path_string }}</span
                          >
                          <span
                            >{{ t('test.project.totalDuration') }}:
                            {{ durationMs(resultFor(tc.id)!.trace!.total_duration_us) }}</span
                          >
                        </div>
                      </div>
                      <button class="trace-flow-btn" @click="showResultInFlow(resultFor(tc.id)!)">
                        <t-icon name="flowchart" size="12px" />
                        {{ t('test.trace.showInFlow') }}
                      </button>
                    </div>

                    <TraceStepTree
                      :steps="resultFor(tc.id)!.trace!.steps"
                      @open-sub-rule="(step) => openSubRuleTrace(resultFor(tc.id)!, step)"
                    />
                  </div>
                </template>
              </div>
            </div>
          </template>
        </div>

        <!-- Inline editor panel -->
        <div v-if="showEditor" class="test-editor">
          <div class="test-editor__header">
            <span>{{ editingId ? t('test.editCase') : t('test.newCase') }}</span>
            <button class="close-btn" @click="cancelEdit">
              <t-icon name="close" size="12px" />
            </button>
          </div>

          <div class="test-editor__form">
            <div class="form-row">
              <label class="form-label">{{ t('test.fieldName') }}</label>
              <t-input v-model="form.name" size="small" :placeholder="t('test.fieldName')" />
            </div>

            <div class="form-row">
              <div class="form-label-row">
                <label class="form-label">{{ t('test.fieldInput') }}</label>
                <t-button
                  v-if="catalog.contracts.some((c) => c.ruleset_name === rulesetName)"
                  size="small"
                  variant="text"
                  style="font-size: 11px; padding: 0"
                  @click="generateFromContract"
                >
                  {{ t('test.generateFromContract') }}
                </t-button>
              </div>
              <div class="json-wrap" :class="{ 'has-error': jsonError === 'input' }">
                <textarea v-model="inputJson" class="json-editor" rows="6" spellcheck="false" />
              </div>
            </div>

            <div class="form-row">
              <label class="form-label">{{ t('test.fieldExpectCode') }}</label>
              <t-input v-model="form.expect.code" size="small" placeholder="e.g. GRANTED" />
            </div>

            <div class="form-row">
              <label class="form-label">{{ t('test.fieldExpectMessage') }}</label>
              <t-input v-model="form.expect.message" size="small" placeholder="optional" />
            </div>

            <div class="form-row">
              <label class="form-label">{{ t('test.fieldExpectOutput') }}</label>
              <div class="json-wrap" :class="{ 'has-error': jsonError === 'output' }">
                <textarea
                  v-model="expectOutputJson"
                  class="json-editor"
                  rows="3"
                  spellcheck="false"
                  placeholder='{"coupon_type":"vip"}'
                />
              </div>
            </div>

            <div class="form-row">
              <label class="form-label">{{ t('test.fieldTags') }}</label>
              <t-input v-model="form.tags" size="small" placeholder="tag1, tag2" />
            </div>

            <div class="form-actions">
              <t-button size="small" variant="outline" @click="cancelEdit">{{
                t('common.cancel')
              }}</t-button>
              <t-button size="small" theme="primary" :loading="saving" @click="saveTest">{{
                t('common.save')
              }}</t-button>
            </div>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.test-panel {
  display: flex;
  flex-direction: column;
  background: var(--ordo-bg-panel);
  border-top: 1px solid var(--ordo-border-color);
  overflow: hidden;
}

.test-panel__handle {
  height: 4px;
  cursor: ns-resize;
  flex-shrink: 0;
}
.test-panel__handle:hover {
  background: var(--ordo-accent);
  opacity: 0.35;
}

/* ── Header ── */
.test-panel__header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
  height: 34px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--ordo-border-color);
  font-size: 12px;
}

.test-panel__title {
  display: flex;
  align-items: center;
  gap: 5px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
}

.ruleset-badge {
  background: var(--ordo-hover-bg);
  border-radius: 3px;
  padding: 1px 6px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.summary-badges {
  display: flex;
  gap: 4px;
}

.badge {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 3px;
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-tertiary);
}
.badge--pass {
  background: rgba(34, 197, 94, 0.12);
  color: #16a34a;
}
.badge--fail {
  background: rgba(239, 68, 68, 0.12);
  color: #dc2626;
}

.test-panel__actions {
  display: flex;
  gap: 4px;
  margin-left: auto;
}

.close-btn {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  padding: 2px;
  display: flex;
  align-items: center;
}
.close-btn:hover {
  color: var(--ordo-text-primary);
}

/* ── Body layout ── */
.test-panel__body {
  flex: 1;
  overflow: hidden;
  display: flex;
}

.test-list {
  flex: 1;
  overflow-y: auto;
  min-width: 0;
}

.sub-rule-probe {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  display: grid;
  grid-template-columns: minmax(280px, 0.42fr) minmax(360px, 0.58fr);
  gap: 10px;
  padding: 10px;
}

.sub-rule-probe__editor,
.sub-rule-probe__result {
  min-width: 0;
  overflow-y: auto;
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 10px;
  background: linear-gradient(180deg, rgba(15, 23, 32, 0.46), rgba(15, 23, 32, 0.2)),
    var(--ordo-bg-main, var(--ordo-hover-bg));
  padding: 12px;
}

.sub-rule-probe__intro {
  margin-bottom: 12px;
}

.sub-rule-probe__intro p {
  margin: 4px 0 0;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
  line-height: 1.5;
}

.sub-rule-probe__eyebrow {
  color: var(--ordo-text-primary);
  font-size: 13px;
  font-weight: 800;
}

.sub-rule-probe__expect-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.panel-empty-small--probe {
  border: 1px dashed rgba(148, 163, 184, 0.22);
  border-radius: 8px;
  justify-content: center;
  min-height: 180px;
  color: var(--ordo-text-tertiary);
}

/* ── Empty / loading ── */
.panel-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: 8px;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
}

.panel-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}

.panel-empty-small {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

/* ── Test item ── */
.test-item {
  border-bottom: 1px solid var(--ordo-border-color);
  border-left: 2px solid transparent;
  transition: border-color 0.1s;
}
.test-item--pass {
  border-left-color: #22c55e;
}
.test-item--fail {
  border-left-color: #ef4444;
}

.test-item__row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 12px;
  cursor: pointer;
  font-size: 12px;
  transition: background 0.1s;
  user-select: none;
}
.test-item__row:hover {
  background: var(--ordo-hover-bg);
}
.test-item--expanded .test-item__row {
  background: var(--ordo-hover-bg);
}

.status-icon {
  width: 16px;
  flex-shrink: 0;
}

.dot {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  font-size: 9px;
  font-weight: bold;
}
.dot--pending {
  background: var(--ordo-hover-bg);
  border: 1px solid var(--ordo-border-color);
}
.dot--pass {
  background: rgba(34, 197, 94, 0.15);
  color: #16a34a;
}
.dot--fail {
  background: rgba(239, 68, 68, 0.15);
  color: #dc2626;
}

.test-item__info {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}
.test-item__name {
  font-weight: 500;
  color: var(--ordo-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.test-item__tags {
  display: flex;
  gap: 3px;
  flex-shrink: 0;
}
.tag {
  background: var(--ordo-hover-bg);
  border-radius: 3px;
  padding: 0px 5px;
  font-size: 10px;
  color: var(--ordo-text-tertiary);
}
.test-item__duration {
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  flex-shrink: 0;
}
.test-item__btns {
  display: flex;
  gap: 1px;
  flex-shrink: 0;
}
.expand-chevron {
  color: var(--ordo-text-tertiary);
  flex-shrink: 0;
}

/* ── Expanded detail ── */
.test-item__detail {
  padding: 0 14px 12px 32px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.detail-section {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.detail-section--result {
  border-radius: 5px;
  padding: 8px 10px;
  background: var(--ordo-bg-main, var(--ordo-hover-bg));
}
.result--pass {
  border: 1px solid rgba(34, 197, 94, 0.3);
}
.result--fail {
  border: 1px solid rgba(239, 68, 68, 0.25);
}

.detail-label {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--ordo-text-tertiary);
}

.detail-code {
  margin: 0;
  padding: 6px 8px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-deep, rgba(0, 0, 0, 0.06));
  border-radius: 4px;
  white-space: pre-wrap;
  word-break: break-all;
  line-height: 1.5;
}
.detail-code--actual {
  background: rgba(239, 68, 68, 0.05);
  border: 1px solid rgba(239, 68, 68, 0.15);
}

.expect-row {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
.expect-pill {
  background: rgba(99, 102, 241, 0.1);
  border-radius: 4px;
  padding: 2px 7px;
  font-size: 11px;
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-secondary);
}
.expect-pill--actual {
  background: rgba(239, 68, 68, 0.1);
  color: #dc2626;
}

.failures {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin: 4px 0;
}
.failure-line {
  display: flex;
  align-items: flex-start;
  gap: 5px;
  font-size: 11px;
  color: #ef4444;
  line-height: 1.4;
}
.failure-icon {
  flex-shrink: 0;
  margin-top: 1px;
  color: #ef4444;
}

.failure-kind {
  flex-shrink: 0;
  border-radius: 999px;
  padding: 1px 6px;
  background: rgba(239, 68, 68, 0.1);
  color: #f87171;
  font-size: 10px;
  font-weight: 700;
}

.actual-row {
  margin-top: 6px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.trace-section {
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 8px;
  padding: 10px;
  background: linear-gradient(180deg, rgba(15, 23, 32, 0.46), rgba(15, 23, 32, 0.2)),
    var(--ordo-bg-main, var(--ordo-hover-bg));
}

.trace-section__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.trace-section__meta {
  margin-top: 4px;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  color: var(--ordo-text-tertiary);
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
}

.trace-flow-btn {
  border: 1px solid var(--ordo-border-color);
  border-radius: 999px;
  background: var(--ordo-bg-item, transparent);
  color: var(--ordo-text-secondary);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 8px;
  font-size: 11px;
  cursor: pointer;
  flex-shrink: 0;
}

.trace-flow-btn:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

/* ── Editor panel ── */
.test-editor {
  width: 300px;
  flex-shrink: 0;
  border-left: 1px solid var(--ordo-border-color);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.test-editor__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  font-size: 12px;
  font-weight: 500;
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
}

.test-editor__form {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.form-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.form-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.form-label {
  font-size: 11px;
  color: var(--ordo-text-secondary);
  font-weight: 500;
}

.json-wrap {
  border: 1px solid var(--ordo-border-color);
  border-radius: 4px;
  overflow: hidden;
}
.json-wrap.has-error {
  border-color: #ef4444;
}

.json-editor {
  width: 100%;
  background: var(--ordo-bg-deep, var(--ordo-bg-panel));
  color: var(--ordo-text-primary);
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
  border: none;
  padding: 6px 8px;
  resize: vertical;
  outline: none;
  box-sizing: border-box;
}

.form-actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
  margin-top: 4px;
}

@media (max-width: 920px) {
  .sub-rule-probe {
    grid-template-columns: 1fr;
    overflow-y: auto;
  }

  .sub-rule-probe__editor,
  .sub-rule-probe__result {
    overflow: visible;
  }
}
</style>
