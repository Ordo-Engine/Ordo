<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter, useRoute } from 'vue-router'
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next'
import { useCatalogStore } from '@/stores/catalog'
import { useProjectStore } from '@/stores/project'
import { useOrgStore } from '@/stores/org'
import { useAuthStore } from '@/stores/auth'
import { rulesetDraftApi } from '@/api/platform-client'
import { normalizeRuleset } from '@/utils/ruleset'
import {
  conditionToString,
  exprToString,
} from '@ordo-engine/editor-core'
import type {
  Condition,
  Expr,
  RuleSet,
  SchemaField,
} from '@ordo-engine/editor-core'
import type { ContractField, DecisionContract, FactDataType } from '@/api/types'

type ContractForm = Omit<DecisionContract, 'updated_at'>
type SectionKey = 'overview' | 'inputs' | 'outputs' | 'notes'
type ValidationLevel = 'error' | 'warning'

interface ValidationIssue {
  id: string
  level: ValidationLevel
  message: string
  action?: {
    kind: 'create-fact'
    name: string
    label: string
  }
}

const catalog = useCatalogStore()
const projectStore = useProjectStore()
const orgStore = useOrgStore()
const auth = useAuthStore()
const { t } = useI18n()
const router = useRouter()
const route = useRoute()

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)
const canEdit = computed(() => auth.user ? orgStore.canAdmin(auth.user.id) : false)

const selectedRulesetName = ref('')
const activeSection = ref<SectionKey>('overview')
const saving = ref(false)
const loadingRuleset = ref(false)
const rulesetLoadError = ref<string | null>(null)
const rulesetCache = ref<Record<string, RuleSet>>({})
const form = ref<ContractForm>(emptyContract(''))
const baselineSignature = ref('')

const dataTypeOptions = computed<{ label: string; value: FactDataType }[]>(() => [
  { label: t('facts.typeString'), value: 'string' },
  { label: t('facts.typeNumber'), value: 'number' },
  { label: t('facts.typeBoolean'), value: 'boolean' },
  { label: t('facts.typeDate'), value: 'date' },
  { label: t('facts.typeObject'), value: 'object' },
])

const sortedRulesets = computed(() => {
  const withContract = new Set(catalog.contracts.map((c) => c.ruleset_name))
  const all = projectStore.rulesets.map((r) => r.name)
  return [
    ...all.filter((n) => withContract.has(n)),
    ...all.filter((n) => !withContract.has(n)),
  ]
})

const contractMap = computed(() => {
  const map = new Map<string, DecisionContract>()
  for (const contract of catalog.contracts) {
    map.set(contract.ruleset_name, contract)
  }
  return map
})

const selectedRulesetInfo = computed(
  () => projectStore.rulesets.find((ruleset) => ruleset.name === selectedRulesetName.value) ?? null,
)

const selectedContract = computed(
  () => contractMap.value.get(selectedRulesetName.value) ?? null,
)

const selectedRuleset = computed(
  () => rulesetCache.value[selectedRulesetName.value] ?? null,
)

const factLookup = computed(() => {
  const map = new Map<string, { type: FactDataType; description?: string; required?: boolean }>()
  for (const fact of catalog.facts) {
    map.set(fact.name, {
      type: fact.data_type,
      description: fact.description,
      required: fact.null_policy === 'error',
    })
  }
  return map
})

const inputSuggestions = computed(() =>
  selectedRuleset.value ? inferInputFields(selectedRuleset.value) : [],
)

const outputSuggestions = computed(() =>
  selectedRuleset.value ? inferOutputFields(selectedRuleset.value) : [],
)

const rulesetStats = computed(() => {
  const ruleset = selectedRuleset.value
  if (!ruleset) {
    return {
      steps: 0,
      decisions: 0,
      actions: 0,
      terminals: 0,
    }
  }

  return {
    steps: ruleset.steps.length,
    decisions: ruleset.steps.filter((step) => step.type === 'decision').length,
    actions: ruleset.steps.filter((step) => step.type === 'action').length,
    terminals: ruleset.steps.filter((step) => step.type === 'terminal').length,
  }
})

const terminalCodes = computed(() => {
  const ruleset = selectedRuleset.value
  if (!ruleset) return []
  const codes = new Set<string>()
  for (const step of ruleset.steps) {
    if (step.type === 'terminal' && step.code) {
      codes.add(step.code)
    }
  }
  return Array.from(codes)
})

const baselineContract = computed(() =>
  selectedContract.value
    ? toForm(selectedContract.value)
    : buildDraftContract(selectedRulesetName.value, selectedRuleset.value),
)

const isDirty = computed(() => contractSignature(form.value) !== baselineSignature.value)

const validationIssues = computed<ValidationIssue[]>(() => {
  const issues: ValidationIssue[] = []

  if (!form.value.version_pattern.trim()) {
    issues.push({
      id: 'missing-version',
      level: 'error',
      message: t('contracts.validationMissingVersion'),
    })
  }

  if (!form.value.owner.trim()) {
    issues.push({
      id: 'missing-owner',
      level: 'warning',
      message: t('contracts.validationMissingOwner'),
    })
  }

  if (!form.value.sla_p99_ms && form.value.sla_p99_ms !== 0) {
    issues.push({
      id: 'missing-sla',
      level: 'warning',
      message: t('contracts.validationMissingSla'),
    })
  }

  if (form.value.input_fields.length === 0) {
    issues.push({
      id: 'missing-inputs',
      level: 'error',
      message: t('contracts.validationNoInputFields'),
    })
  }

  if (form.value.output_fields.length === 0) {
    issues.push({
      id: 'missing-outputs',
      level: 'error',
      message: t('contracts.validationNoOutputFields'),
    })
  }

  for (const duplicate of findDuplicateNames(form.value.input_fields)) {
    issues.push({
      id: `dup-input-${duplicate}`,
      level: 'error',
      message: t('contracts.validationDuplicateField', { name: duplicate }),
    })
  }

  for (const duplicate of findDuplicateNames(form.value.output_fields)) {
    issues.push({
      id: `dup-output-${duplicate}`,
      level: 'error',
      message: t('contracts.validationDuplicateField', { name: duplicate }),
    })
  }

  const knownInputNames = new Set([
    ...inputSuggestions.value.map((field) => field.name),
    ...catalog.facts.map((fact) => fact.name),
  ])
  const knownOutputNames = new Set(outputSuggestions.value.map((field) => field.name))

  for (const field of form.value.input_fields) {
    const name = field.name.trim()
    if (name && !knownInputNames.has(name)) {
      issues.push({
        id: `unknown-input-${name}`,
        level: 'warning',
        message: t('contracts.validationUnknownInput', { name }),
        action: canEdit.value
          ? {
              kind: 'create-fact',
              name,
              label: t('contracts.actionCreateFact'),
            }
          : undefined,
      })
    }
  }

  for (const field of form.value.output_fields) {
    const name = field.name.trim()
    if (name && knownOutputNames.size > 0 && !knownOutputNames.has(name)) {
      issues.push({
        id: `unknown-output-${name}`,
        level: 'warning',
        message: t('contracts.validationUnknownOutput', { name }),
      })
    }
  }

  return issues
})

const requestExample = computed(() =>
  JSON.stringify(buildExampleObject(form.value.input_fields), null, 2),
)

const responseExample = computed(() =>
  JSON.stringify(
    {
      code: terminalCodes.value[0] ?? 'OK',
      output: buildExampleObject(form.value.output_fields),
    },
    null,
    2,
  ),
)

const sidebarRulesets = computed(() =>
  sortedRulesets.value.map((name) => {
    const contract = contractMap.value.get(name)
    return {
      name,
      hasContract: Boolean(contract),
      inputCount: contract?.input_fields.length ?? 0,
      outputCount: contract?.output_fields.length ?? 0,
    }
  }),
)

watch(
  [sortedRulesets, () => route.query.ruleset],
  () => {
    if (sortedRulesets.value.length === 0) {
      selectedRulesetName.value = ''
      return
    }

    const queryRuleset = typeof route.query.ruleset === 'string' ? route.query.ruleset : ''
    if (queryRuleset && sortedRulesets.value.includes(queryRuleset)) {
      if (selectedRulesetName.value !== queryRuleset) {
        selectedRulesetName.value = queryRuleset
      }
      return
    }

    if (!selectedRulesetName.value || !sortedRulesets.value.includes(selectedRulesetName.value)) {
      selectedRulesetName.value = sortedRulesets.value[0]
    }
  },
  { immediate: true },
)

watch(
  selectedRulesetName,
  async (name) => {
    activeSection.value = 'overview'
    rulesetLoadError.value = null

    if (!name) return

    if (route.query.ruleset !== name) {
      void router.replace({
        query: {
          ...route.query,
          ruleset: name,
        },
      })
    }

    await ensureRulesetLoaded(name)
    hydrateForm()
  },
  { immediate: true },
)

watch(selectedContract, () => {
  if (!isDirty.value) {
    hydrateForm()
  }
})

function emptyContract(rulesetName: string): ContractForm {
  return {
    ruleset_name: rulesetName,
    version_pattern: '1.x',
    owner: '',
    sla_p99_ms: undefined,
    input_fields: [],
    output_fields: [],
    notes: '',
  }
}

function cloneField(field: ContractField): ContractField {
  return {
    name: field.name,
    data_type: field.data_type,
    required: field.required,
    description: field.description ?? '',
  }
}

function toForm(contract: DecisionContract | ContractForm): ContractForm {
  return {
    ruleset_name: contract.ruleset_name,
    version_pattern: contract.version_pattern,
    owner: contract.owner,
    sla_p99_ms: contract.sla_p99_ms,
    input_fields: contract.input_fields.map(cloneField),
    output_fields: contract.output_fields.map(cloneField),
    notes: contract.notes ?? '',
  }
}

function contractSignature(contract: ContractForm): string {
  return JSON.stringify({
    ruleset_name: contract.ruleset_name,
    version_pattern: contract.version_pattern.trim(),
    owner: contract.owner.trim(),
    sla_p99_ms: contract.sla_p99_ms ?? null,
    input_fields: contract.input_fields.map(normalizeFieldForSignature),
    output_fields: contract.output_fields.map(normalizeFieldForSignature),
    notes: (contract.notes ?? '').trim(),
  })
}

function normalizeFieldForSignature(field: ContractField) {
  return {
    name: field.name.trim(),
    data_type: field.data_type,
    required: field.required,
    description: (field.description ?? '').trim(),
  }
}

function hydrateForm() {
  const next = baselineContract.value
  form.value = toForm(next)
  baselineSignature.value = contractSignature(form.value)
}

async function ensureRulesetLoaded(name: string) {
  if (!auth.token || !projectId.value || !orgId.value || rulesetCache.value[name]) {
    return
  }

  loadingRuleset.value = true
  try {
    const draft = await rulesetDraftApi.get(auth.token, orgId.value, projectId.value, name)
    rulesetCache.value = {
      ...rulesetCache.value,
      [name]: normalizeRuleset(draft.draft, name),
    }
  } catch (error: any) {
    rulesetLoadError.value = error.message || t('contracts.loadRulesetFailed')
  } finally {
    loadingRuleset.value = false
  }
}

function buildDraftContract(rulesetName: string, ruleset: RuleSet | null): ContractForm {
  const draft = emptyContract(rulesetName)

  if (!ruleset) return draft

  draft.version_pattern = toVersionPattern(ruleset.config.version)
  draft.input_fields = inferInputFields(ruleset)
  draft.output_fields = inferOutputFields(ruleset)

  return draft
}

function toVersionPattern(version?: string): string {
  if (!version) return '1.x'
  const major = version.split('.')[0]?.trim()
  return major ? `${major}.x` : '1.x'
}

function mapSchemaFieldType(type?: SchemaField['type']): FactDataType {
  switch (type) {
    case 'number':
      return 'number'
    case 'boolean':
      return 'boolean'
    case 'object':
    case 'array':
    case 'any':
      return 'object'
    case 'string':
    default:
      return 'string'
  }
}

function inferInputFields(ruleset: RuleSet): ContractField[] {
  const fields = new Map<string, ContractField>()

  for (const field of ruleset.config.inputSchema ?? []) {
    fields.set(field.name, {
      name: field.name,
      data_type: mapSchemaFieldType(field.type),
      required: field.required ?? false,
      description: field.description ?? '',
    })
  }

  for (const path of collectReferencedInputs(ruleset)) {
    const fact = factLookup.value.get(path)
    if (!fact || fields.has(path)) continue

    fields.set(path, {
      name: path,
      data_type: fact.type,
      required: fact.required ?? false,
      description: fact.description ?? '',
    })
  }

  return Array.from(fields.values())
}

function inferOutputFields(ruleset: RuleSet): ContractField[] {
  const fields = new Map<string, ContractField>()

  for (const field of ruleset.config.outputSchema ?? []) {
    fields.set(field.name, {
      name: field.name,
      data_type: mapSchemaFieldType(field.type),
      required: field.required ?? false,
      description: field.description ?? '',
    })
  }

  for (const step of ruleset.steps) {
    if (step.type !== 'terminal') continue

    for (const output of step.output ?? []) {
      if (!output.name || fields.has(output.name)) continue

      fields.set(output.name, {
        name: output.name,
        data_type: inferExprDataType(output.value, ruleset),
        required: true,
        description: '',
      })
    }
  }

  return Array.from(fields.values())
}

function collectReferencedInputs(ruleset: RuleSet): string[] {
  const paths = new Set<string>()

  for (const step of ruleset.steps) {
    if (step.type === 'decision') {
      for (const branch of step.branches) {
        collectConditionPaths(branch.condition, paths)
      }
      continue
    }

    if (step.type === 'action') {
      for (const assignment of step.assignments ?? []) {
        collectExprPaths(assignment.value, paths)
      }
      for (const call of step.externalCalls ?? []) {
        for (const param of Object.values(call.params ?? {})) {
          collectExprPaths(param, paths)
        }
        if (call.fallbackValue) {
          collectExprPaths(call.fallbackValue, paths)
        }
      }
      if (step.logging?.message) {
        collectExprPaths(step.logging.message, paths)
      }
      continue
    }

    if (step.type === 'terminal') {
      if (step.message) {
        collectExprPaths(step.message, paths)
      }
      for (const output of step.output ?? []) {
        collectExprPaths(output.value, paths)
      }
    }
  }

  return Array.from(paths)
}

function collectConditionPaths(condition: Condition, paths: Set<string>) {
  switch (condition.type) {
    case 'simple':
      collectExprPaths(condition.left, paths)
      collectExprPaths(condition.right, paths)
      break
    case 'logical':
      for (const child of condition.conditions) {
        collectConditionPaths(child, paths)
      }
      break
    case 'not':
      collectConditionPaths(condition.condition, paths)
      break
    case 'expression':
      for (const match of condition.expression.matchAll(/\$\.([A-Za-z0-9_.]+)/g)) {
        paths.add(match[1])
      }
      break
    case 'constant':
      break
  }
}

function collectExprPaths(expr: Expr, paths: Set<string>) {
  switch (expr.type) {
    case 'variable': {
      const normalized = normalizeVariablePath(expr.path)
      if (normalized) {
        paths.add(normalized)
      }
      break
    }
    case 'binary':
      collectExprPaths(expr.left, paths)
      collectExprPaths(expr.right, paths)
      break
    case 'unary':
      collectExprPaths(expr.operand, paths)
      break
    case 'function':
      for (const arg of expr.args) {
        collectExprPaths(arg, paths)
      }
      break
    case 'conditional':
      collectExprPaths(expr.condition, paths)
      collectExprPaths(expr.thenExpr, paths)
      collectExprPaths(expr.elseExpr, paths)
      break
    case 'array':
      for (const element of expr.elements) {
        collectExprPaths(element, paths)
      }
      break
    case 'object':
      for (const value of Object.values(expr.properties)) {
        collectExprPaths(value, paths)
      }
      break
    case 'member':
      collectExprPaths(expr.object, paths)
      if (typeof expr.property !== 'string') {
        collectExprPaths(expr.property, paths)
      }
      break
    case 'literal':
      break
  }
}

function normalizeVariablePath(path: string): string | null {
  if (!path.startsWith('$.')) return null
  return path.slice(2)
}

function inferExprDataType(expr: Expr, ruleset: RuleSet): FactDataType {
  switch (expr.type) {
    case 'literal':
      if (expr.valueType === 'number') return 'number'
      if (expr.valueType === 'boolean') return 'boolean'
      return 'string'
    case 'variable': {
      const normalized = normalizeVariablePath(expr.path)
      if (!normalized) return 'string'

      const fact = factLookup.value.get(normalized)
      if (fact) return fact.type

      for (const field of ruleset.config.inputSchema ?? []) {
        if (field.name === normalized) return mapSchemaFieldType(field.type)
      }
      for (const field of ruleset.config.outputSchema ?? []) {
        if (field.name === normalized) return mapSchemaFieldType(field.type)
      }
      return 'string'
    }
    case 'binary':
      if (['add', 'sub', 'mul', 'div', 'mod'].includes(expr.op)) return 'number'
      return 'boolean'
    case 'unary':
      return expr.op === 'neg' ? 'number' : 'boolean'
    case 'conditional':
      return inferExprDataType(expr.thenExpr, ruleset)
    case 'array':
    case 'object':
    case 'member':
      return 'object'
    case 'function':
      return 'string'
  }
}

function findDuplicateNames(fields: ContractField[]): string[] {
  const seen = new Set<string>()
  const duplicates = new Set<string>()

  for (const field of fields) {
    const name = field.name.trim()
    if (!name) continue
    if (seen.has(name)) {
      duplicates.add(name)
    } else {
      seen.add(name)
    }
  }

  return Array.from(duplicates)
}

function buildExampleObject(fields: ContractField[]) {
  const payload: Record<string, unknown> = {}
  for (const field of fields) {
    const name = field.name.trim()
    if (!name) continue
    payload[name] = sampleValueForType(field.data_type)
  }
  return payload
}

function sampleValueForType(type: FactDataType): unknown {
  switch (type) {
    case 'number':
      return 100
    case 'boolean':
      return true
    case 'date':
      return '2026-04-12T00:00:00Z'
    case 'object':
      return {}
    case 'string':
    default:
      return 'example'
  }
}

function addField(list: ContractField[]) {
  list.push({
    name: '',
    data_type: 'string',
    required: true,
    description: '',
  })
}

function removeField(list: ContractField[], index: number) {
  list.splice(index, 1)
}

function applyInputSuggestions() {
  form.value.input_fields = inputSuggestions.value.map(cloneField)
}

function applyOutputSuggestions() {
  form.value.output_fields = outputSuggestions.value.map(cloneField)
}

function refreshFromRuleset() {
  hydrateForm()
}

async function handleSave() {
  if (!selectedRulesetName.value) return
  saving.value = true
  try {
    const saved = await catalog.upsertContract(selectedRulesetName.value, {
      version_pattern: form.value.version_pattern.trim(),
      owner: form.value.owner.trim(),
      sla_p99_ms: form.value.sla_p99_ms,
      input_fields: form.value.input_fields.map((field) => ({
        name: field.name.trim(),
        data_type: field.data_type,
        required: field.required,
        description: field.description?.trim() || undefined,
      })),
      output_fields: form.value.output_fields.map((field) => ({
        name: field.name.trim(),
        data_type: field.data_type,
        required: field.required,
        description: field.description?.trim() || undefined,
      })),
      notes: form.value.notes?.trim() || undefined,
    })
    if (!saved) {
      throw new Error(t('contracts.saveFailed'))
    }
    form.value = toForm(saved)
    baselineSignature.value = contractSignature(form.value)
    MessagePlugin.success(t('contracts.saveSuccess'))
  } catch (error: any) {
    MessagePlugin.error(error.message || t('contracts.saveFailed'))
  } finally {
    saving.value = false
  }
}

function handleDelete() {
  if (!selectedRulesetName.value) return

  const name = selectedRulesetName.value
  const dialog = DialogPlugin.confirm({
    header: t('contracts.deleteDialog'),
    body: t('contracts.deleteConfirm', { name }),
    confirmBtn: { content: t('common.delete'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await catalog.deleteContract(name)
        dialog.hide()
        hydrateForm()
        MessagePlugin.success(t('contracts.deleteSuccess'))
      } catch (error: any) {
        MessagePlugin.error(error.message || t('contracts.saveFailed'))
      }
    },
  })
}

function openRulesetEditor() {
  if (!selectedRulesetName.value) return
  void router.push(`/orgs/${orgId.value}/projects/${projectId.value}/editor/${selectedRulesetName.value}`)
}

function handleValidationAction(issue: ValidationIssue) {
  if (!issue.action) return

  if (issue.action.kind === 'create-fact') {
    void router.push({
      path: `/orgs/${orgId.value}/projects/${projectId.value}/facts`,
      query: { createFact: issue.action.name },
    })
  }
}

function selectRuleset(name: string) {
  selectedRulesetName.value = name
}

function sectionLabel(section: SectionKey): string {
  switch (section) {
    case 'overview':
      return t('contracts.overviewTab')
    case 'inputs':
      return t('contracts.inputFields')
    case 'outputs':
      return t('contracts.outputFields')
    case 'notes':
      return t('contracts.notesLabel')
  }
}

function summaryRowForRuleset() {
  const ruleset = selectedRuleset.value
  if (!ruleset) return []

  return [
    ...ruleset.steps
      .filter((step) => step.type === 'decision')
      .slice(0, 3)
      .map((step) => ({
        id: step.id,
        title: step.name,
        detail:
          step.type === 'decision'
            ? step.branches.map((branch) => branch.label || conditionToString(branch.condition)).join(' · ')
            : '',
      })),
    ...ruleset.steps
      .filter((step) => step.type === 'terminal')
      .slice(0, 2)
      .map((step) => ({
        id: step.id,
        title: step.name,
        detail: step.type === 'terminal' ? (step.output ?? []).map((field) => `${field.name} = ${exprToString(field.value)}`).join(' · ') : '',
      })),
  ]
}
</script>

<template>
  <div class="contract-view">
    <t-breadcrumb class="asset-breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{ t('breadcrumb.home') }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{ t('breadcrumb.projects') }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('projectNav.contracts') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <div class="asset-header">
      <div class="asset-header__info">
        <h2 class="asset-header__title">{{ t('contracts.title') }}</h2>
        <p class="asset-header__desc">{{ t('contracts.desc') }}</p>
      </div>
    </div>

    <div v-if="catalog.loading" class="asset-loading">
      <t-loading />
    </div>

    <div v-else-if="projectStore.rulesets.length === 0" class="asset-empty">
      <t-icon name="file-safety" size="40px" style="opacity:0.3" />
      <p>{{ t('contracts.noRulesets') }}</p>
    </div>

    <div v-else class="contract-workbench">
      <aside class="ruleset-nav">
        <div class="ruleset-nav__header">
          <span class="ruleset-nav__title">{{ t('contracts.rulesetsTitle') }}</span>
          <span class="ruleset-nav__count">{{ sidebarRulesets.length }}</span>
        </div>

        <button
          v-for="item in sidebarRulesets"
          :key="item.name"
          class="ruleset-nav__item"
          :class="{ 'is-active': selectedRulesetName === item.name }"
          @click="selectRuleset(item.name)"
        >
          <div class="ruleset-nav__row">
            <span class="ruleset-nav__name">{{ item.name }}</span>
            <t-tag size="small" variant="light" :theme="item.hasContract ? 'success' : 'warning'">
              {{ item.hasContract ? t('contracts.workspaceSaved') : t('contracts.workspaceDraft') }}
            </t-tag>
          </div>
          <div class="ruleset-nav__meta">
            <span>{{ item.inputCount }} in</span>
            <span>{{ item.outputCount }} out</span>
          </div>
        </button>
      </aside>

      <div class="contract-main">
        <div class="contract-main__header">
          <div class="contract-main__title-wrap">
            <div class="contract-main__title-row">
              <h3 class="contract-main__title">{{ selectedRulesetName }}</h3>
              <t-tag size="small" variant="light" :theme="selectedContract ? 'success' : 'warning'">
                {{ selectedContract ? t('contracts.workspaceSaved') : t('contracts.workspaceDraft') }}
              </t-tag>
              <t-tag v-if="selectedRulesetInfo?.version" size="small" variant="light">
                v{{ selectedRulesetInfo.version }}
              </t-tag>
            </div>
            <p class="contract-main__subtitle">
              {{ selectedRuleset?.config.description || selectedRulesetInfo?.description || t('project.noDesc') }}
            </p>
          </div>

          <div class="contract-main__actions">
            <t-button size="small" variant="outline" @click="openRulesetEditor">
              {{ t('contracts.openEditor') }}
            </t-button>
            <t-button size="small" variant="outline" @click="refreshFromRuleset">
              {{ t('contracts.refreshDraft') }}
            </t-button>
            <t-button v-if="canEdit" size="small" theme="primary" :loading="saving" :disabled="!isDirty" @click="handleSave">
              {{ t('contracts.saveBtn') }}
            </t-button>
            <t-button
              v-if="canEdit && selectedContract"
              size="small"
              theme="danger"
              variant="outline"
              @click="handleDelete"
            >
              {{ t('contracts.deleteBtn') }}
            </t-button>
          </div>
        </div>

        <div class="contract-stats">
          <div class="contract-stat">
            <span class="contract-stat__label">{{ t('contracts.stepCount') }}</span>
            <strong class="contract-stat__value">{{ rulesetStats.steps }}</strong>
          </div>
          <div class="contract-stat">
            <span class="contract-stat__label">{{ t('contracts.decisionCount') }}</span>
            <strong class="contract-stat__value">{{ rulesetStats.decisions }}</strong>
          </div>
          <div class="contract-stat">
            <span class="contract-stat__label">{{ t('contracts.actionCount') }}</span>
            <strong class="contract-stat__value">{{ rulesetStats.actions }}</strong>
          </div>
          <div class="contract-stat">
            <span class="contract-stat__label">{{ t('contracts.terminalCount') }}</span>
            <strong class="contract-stat__value">{{ rulesetStats.terminals }}</strong>
          </div>
        </div>

        <div v-if="loadingRuleset" class="contract-inline-loading">
          <t-loading size="small" />
        </div>
        <div v-else-if="rulesetLoadError" class="contract-inline-error">
          {{ rulesetLoadError }}
        </div>

        <div class="contract-body">
          <section class="contract-editor">
            <div class="contract-sections">
              <button
                v-for="section in ['overview', 'inputs', 'outputs', 'notes'] as SectionKey[]"
                :key="section"
                class="contract-section-tab"
                :class="{ 'is-active': activeSection === section }"
                @click="activeSection = section"
              >
                {{ sectionLabel(section) }}
              </button>
            </div>

            <div class="contract-panel">
              <template v-if="activeSection === 'overview'">
                <div class="contract-form-grid">
                  <label class="contract-form-item">
                    <span class="contract-form-item__label">{{ t('contracts.versionLabel') }}</span>
                    <t-input
                      v-model="form.version_pattern"
                      :disabled="!canEdit"
                      :placeholder="t('contracts.versionPlaceholder')"
                    />
                  </label>

                  <label class="contract-form-item">
                    <span class="contract-form-item__label">{{ t('contracts.ownerLabel') }}</span>
                    <t-input
                      v-model="form.owner"
                      :disabled="!canEdit"
                      :placeholder="t('contracts.ownerPlaceholder')"
                    />
                  </label>

                  <label class="contract-form-item">
                    <span class="contract-form-item__label">{{ t('contracts.slaLabel') }}</span>
                    <t-input-number
                      v-model="form.sla_p99_ms"
                      :disabled="!canEdit"
                      :min="0"
                    />
                  </label>
                </div>

                <div class="contract-summary">
                  <div class="contract-summary__title">{{ t('contracts.rulesetSummary') }}</div>
                  <div class="contract-summary__chips">
                    <t-tag v-for="code in terminalCodes" :key="code" size="small" variant="light">
                      {{ code }}
                    </t-tag>
                    <span v-if="terminalCodes.length === 0" class="contract-summary__empty">
                      {{ t('contracts.noDetected') }}
                    </span>
                  </div>

                  <div v-if="summaryRowForRuleset().length" class="contract-summary__list">
                    <div
                      v-for="row in summaryRowForRuleset()"
                      :key="row.id"
                      class="contract-summary__row"
                    >
                      <strong>{{ row.title }}</strong>
                      <span>{{ row.detail || '—' }}</span>
                    </div>
                  </div>
                </div>
              </template>

              <template v-else-if="activeSection === 'inputs'">
                <div class="field-toolbar">
                  <div>
                    <div class="field-toolbar__title">{{ t('contracts.inputCount', { count: form.input_fields.length }) }}</div>
                    <div class="field-toolbar__hint">{{ t('contracts.detectedInputs') }}: {{ inputSuggestions.length }}</div>
                  </div>
                  <div class="field-toolbar__actions">
                    <t-button size="small" variant="outline" :disabled="!canEdit" @click="applyInputSuggestions">
                      {{ t('contracts.useSuggestedInputs') }}
                    </t-button>
                    <t-button size="small" theme="primary" variant="outline" :disabled="!canEdit" @click="addField(form.input_fields)">
                      <t-icon name="add" />
                      {{ t('contracts.addInput') }}
                    </t-button>
                  </div>
                </div>

                <div v-if="form.input_fields.length === 0" class="field-empty">
                  {{ t('contracts.noFields') }}
                </div>

                <div v-else class="field-editor">
                  <div v-for="(field, index) in form.input_fields" :key="index" class="field-editor__row">
                    <t-input v-model="field.name" :disabled="!canEdit" :placeholder="t('contracts.fieldName')" />
                    <t-select v-model="field.data_type" :disabled="!canEdit" :options="dataTypeOptions" />
                    <t-checkbox v-model="field.required" :disabled="!canEdit">
                      {{ t('contracts.required') }}
                    </t-checkbox>
                    <t-input v-model="field.description" :disabled="!canEdit" :placeholder="t('contracts.fieldDesc')" />
                    <t-button
                      size="small"
                      variant="text"
                      theme="danger"
                      :disabled="!canEdit"
                      @click="removeField(form.input_fields, index)"
                    >
                      <t-icon name="close" />
                    </t-button>
                  </div>
                </div>
              </template>

              <template v-else-if="activeSection === 'outputs'">
                <div class="field-toolbar">
                  <div>
                    <div class="field-toolbar__title">{{ t('contracts.outputCount', { count: form.output_fields.length }) }}</div>
                    <div class="field-toolbar__hint">{{ t('contracts.detectedOutputs') }}: {{ outputSuggestions.length }}</div>
                  </div>
                  <div class="field-toolbar__actions">
                    <t-button size="small" variant="outline" :disabled="!canEdit" @click="applyOutputSuggestions">
                      {{ t('contracts.useSuggestedOutputs') }}
                    </t-button>
                    <t-button size="small" theme="primary" variant="outline" :disabled="!canEdit" @click="addField(form.output_fields)">
                      <t-icon name="add" />
                      {{ t('contracts.addOutput') }}
                    </t-button>
                  </div>
                </div>

                <div v-if="form.output_fields.length === 0" class="field-empty">
                  {{ t('contracts.noFields') }}
                </div>

                <div v-else class="field-editor">
                  <div v-for="(field, index) in form.output_fields" :key="index" class="field-editor__row">
                    <t-input v-model="field.name" :disabled="!canEdit" :placeholder="t('contracts.fieldName')" />
                    <t-select v-model="field.data_type" :disabled="!canEdit" :options="dataTypeOptions" />
                    <t-checkbox v-model="field.required" :disabled="!canEdit">
                      {{ t('contracts.required') }}
                    </t-checkbox>
                    <t-input v-model="field.description" :disabled="!canEdit" :placeholder="t('contracts.fieldDesc')" />
                    <t-button
                      size="small"
                      variant="text"
                      theme="danger"
                      :disabled="!canEdit"
                      @click="removeField(form.output_fields, index)"
                    >
                      <t-icon name="close" />
                    </t-button>
                  </div>
                </div>
              </template>

              <template v-else>
                <label class="contract-form-item contract-form-item--block">
                  <span class="contract-form-item__label">{{ t('contracts.notesLabel') }}</span>
                  <t-textarea
                    v-model="form.notes"
                    :disabled="!canEdit"
                    :autosize="{ minRows: 10, maxRows: 16 }"
                    :placeholder="t('contracts.notesPlaceholder')"
                  />
                </label>
              </template>
            </div>
          </section>

          <aside class="contract-assist">
            <section class="assist-card">
              <div class="assist-card__header">
                <h4>{{ t('contracts.healthTitle') }}</h4>
                <t-tag size="small" :theme="validationIssues.some((item) => item.level === 'error') ? 'danger' : 'success'" variant="light">
                  {{ validationIssues.length === 0 ? t('contracts.healthReady') : t('contracts.healthNeedsWork') }}
                </t-tag>
              </div>
              <div v-if="validationIssues.length === 0" class="assist-empty">
                {{ t('contracts.healthReady') }}
              </div>
              <div v-else class="assist-list">
                <div
                  v-for="issue in validationIssues"
                  :key="issue.id"
                  class="assist-list__item"
                  :class="`is-${issue.level}`"
                >
                  <span class="assist-list__dot" />
                  <div class="assist-list__content">
                    <span>{{ issue.message }}</span>
                    <t-link
                      v-if="issue.action"
                      theme="primary"
                      hover="color"
                      @click="handleValidationAction(issue)"
                    >
                      {{ issue.action.label }}
                    </t-link>
                  </div>
                </div>
              </div>
            </section>

            <section class="assist-card">
              <div class="assist-card__header">
                <h4>{{ t('contracts.examplesTitle') }}</h4>
              </div>
              <div class="assist-code-block">
                <div class="assist-code-block__label">{{ t('contracts.requestExample') }}</div>
                <pre>{{ requestExample }}</pre>
              </div>
              <div class="assist-code-block">
                <div class="assist-code-block__label">{{ t('contracts.responseExample') }}</div>
                <pre>{{ responseExample }}</pre>
              </div>
            </section>

            <section class="assist-card">
              <div class="assist-card__header">
                <h4>{{ t('contracts.detectedInputs') }}</h4>
              </div>
              <div v-if="inputSuggestions.length === 0" class="assist-empty">
                {{ t('contracts.noDetected') }}
              </div>
              <div v-else class="assist-tags">
                <t-tag
                  v-for="field in inputSuggestions"
                  :key="field.name"
                  size="small"
                  variant="light"
                >
                  {{ field.name }}
                </t-tag>
              </div>
            </section>

            <section class="assist-card">
              <div class="assist-card__header">
                <h4>{{ t('contracts.detectedOutputs') }}</h4>
              </div>
              <div v-if="outputSuggestions.length === 0" class="assist-empty">
                {{ t('contracts.noDetected') }}
              </div>
              <div v-else class="assist-tags">
                <t-tag
                  v-for="field in outputSuggestions"
                  :key="field.name"
                  size="small"
                  variant="light"
                >
                  {{ field.name }}
                </t-tag>
              </div>
            </section>
          </aside>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.contract-view {
  padding: 20px 24px;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.asset-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 16px;
  gap: 16px;
  flex-shrink: 0;
}

.asset-header__title {
  font-size: 16px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin: 0 0 4px;
}

.asset-header__desc {
  font-size: 13px;
  color: var(--ordo-text-secondary);
  margin: 0;
  max-width: 720px;
}

.asset-loading,
.asset-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 12px;
  min-height: 200px;
  color: var(--ordo-text-tertiary);
  font-size: 13px;
}

.contract-workbench {
  display: flex;
  gap: 16px;
  min-height: 0;
  flex: 1;
}

.ruleset-nav {
  width: 240px;
  flex-shrink: 0;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-panel);
  overflow-y: auto;
}

.ruleset-nav__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border-light);
}

.ruleset-nav__title {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--ordo-text-tertiary);
}

.ruleset-nav__count {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  font-family: var(--ordo-font-mono);
}

.ruleset-nav__item {
  width: 100%;
  border: none;
  border-bottom: 1px solid var(--ordo-border-light);
  background: transparent;
  padding: 10px 12px;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s ease;
}

.ruleset-nav__item:hover,
.ruleset-nav__item.is-active {
  background: var(--ordo-active-bg);
}

.ruleset-nav__row,
.ruleset-nav__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.ruleset-nav__name {
  font-size: 12px;
  font-weight: 600;
  font-family: var(--ordo-font-mono);
  color: var(--ordo-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ruleset-nav__meta {
  margin-top: 6px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.contract-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.contract-main__header,
.contract-stats,
.contract-inline-loading,
.contract-inline-error,
.contract-panel,
.assist-card {
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-panel);
}

.contract-main__header {
  padding: 16px;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.contract-main__title-row,
.contract-main__actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.contract-main__title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  font-family: var(--ordo-font-mono);
  color: var(--ordo-text-primary);
}

.contract-main__subtitle {
  margin: 6px 0 0;
  color: var(--ordo-text-secondary);
  font-size: 12px;
}

.contract-stats {
  padding: 0;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0;
}

.contract-stat {
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  border-right: 1px solid var(--ordo-border-light);
}

.contract-stat:last-child {
  border-right: none;
}

.contract-stat__label {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.contract-stat__value {
  font-size: 18px;
  font-family: var(--ordo-font-mono);
  color: var(--ordo-text-primary);
}

.contract-inline-loading,
.contract-inline-error {
  padding: 12px 16px;
}

.contract-inline-error {
  color: var(--ordo-error);
  font-size: 12px;
}

.contract-body {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) 360px;
  gap: 16px;
  flex: 1;
}

.contract-editor,
.contract-assist {
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.contract-sections {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.contract-section-tab {
  border: 1px solid var(--ordo-border-color);
  background: transparent;
  border-radius: var(--ordo-radius-sm);
  padding: 6px 10px;
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  cursor: pointer;
}

.contract-section-tab.is-active {
  background: var(--ordo-active-bg);
  color: var(--ordo-accent);
  border-color: var(--ordo-border-color);
}

.contract-panel {
  padding: 16px;
  overflow-y: auto;
  min-height: 0;
}

.contract-form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.contract-form-item {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.contract-form-item--block {
  width: 100%;
}

.contract-form-item__label {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
}

.contract-summary {
  margin-top: 16px;
  padding-top: 16px;
  border-top: 1px solid var(--ordo-border-light);
}

.contract-summary__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin-bottom: 12px;
}

.contract-summary__chips {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.contract-summary__empty {
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.contract-summary__list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.contract-summary__row {
  padding: 12px;
  border: 1px solid var(--ordo-border-light);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-panel);
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
}

.contract-summary__row strong {
  color: var(--ordo-text-primary);
  font-family: var(--ordo-font-mono);
  font-weight: 600;
}

.contract-summary__row span {
  color: var(--ordo-text-secondary);
}

.field-toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}

.field-toolbar__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.field-toolbar__hint {
  font-size: 11px;
  color: var(--ordo-text-secondary);
  margin-top: 4px;
}

.field-toolbar__actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.field-empty {
  min-height: 160px;
  border: 1px dashed var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--ordo-text-secondary);
  font-size: 12px;
}

.field-editor {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.field-editor__row {
  display: grid;
  grid-template-columns: minmax(140px, 1.3fr) 120px 110px minmax(180px, 1.6fr) 40px;
  gap: 10px;
  align-items: center;
}

.assist-card {
  padding: 16px;
}

.assist-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.assist-card__header h4 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.assist-empty {
  color: var(--ordo-text-secondary);
  font-size: 12px;
}

.assist-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.assist-list__item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.assist-list__content {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 3px;
}

.assist-list__item.is-error {
  color: var(--ordo-error);
}

.assist-list__item.is-warning {
  color: var(--ordo-warning);
}

.assist-list__dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: currentColor;
  margin-top: 5px;
  flex-shrink: 0;
}

.assist-code-block + .assist-code-block {
  margin-top: 12px;
}

.assist-code-block__label {
  font-size: 11px;
  color: var(--ordo-text-secondary);
  margin-bottom: 6px;
}

.assist-code-block pre {
  margin: 0;
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-app);
  border: 1px solid var(--ordo-border-light);
  color: var(--ordo-text-primary);
  padding: 12px;
  font-size: 12px;
  line-height: 1.55;
  overflow: auto;
}

.assist-tags {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

@media (max-width: 1280px) {
  .contract-body {
    grid-template-columns: 1fr;
  }

  .contract-assist {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 980px) {
  .contract-workbench {
    flex-direction: column;
  }

  .ruleset-nav {
    width: 100%;
    max-height: 240px;
  }

  .contract-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .contract-form-grid,
  .field-editor__row,
  .contract-assist {
    grid-template-columns: 1fr;
  }

  .contract-main__header,
  .field-toolbar {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>
