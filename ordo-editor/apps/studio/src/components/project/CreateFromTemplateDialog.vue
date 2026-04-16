<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { useTemplateStore } from '@/stores/template'
import type { Project, TemplateMetadata } from '@/api/types'

const props = defineProps<{
  visible: boolean
  orgId: string
}>()

const emit = defineEmits<{
  (e: 'update:visible', v: boolean): void
  (e: 'created', p: Project): void
}>()

const { t } = useI18n()
const store = useTemplateStore()

const selectedId = ref<string | null>(null)
const projectName = ref('')
const projectDesc = ref('')

const selectedTemplate = computed<TemplateMetadata | null>(() =>
  store.templates.find((tpl) => tpl.id === selectedId.value) ?? null,
)

watch(
  () => props.visible,
  async (open) => {
    if (!open) return
    selectedId.value = null
    projectName.value = ''
    projectDesc.value = ''
    store.clearDetail()
    await store.fetchTemplates()
    if (store.templates.length > 0) {
      await selectTemplate(store.templates[0].id)
    }
  },
)

async function selectTemplate(id: string) {
  selectedId.value = id
  await store.fetchDetail(id)
  const tpl = store.templates.find((x) => x.id === id)
  if (tpl) {
    projectName.value = `${tpl.name} - ${t('template.copySuffix')}`
  }
}

function close() {
  emit('update:visible', false)
}

async function handleCreate() {
  if (!selectedId.value) {
    MessagePlugin.warning(t('template.selectRequired'))
    return
  }
  if (!projectName.value.trim()) {
    MessagePlugin.warning(t('template.projectNameRequired'))
    return
  }
  try {
    const project = await store.createFromTemplate(props.orgId, {
      template_id: selectedId.value,
      project_name: projectName.value.trim(),
      project_description: projectDesc.value.trim() || undefined,
    })
    MessagePlugin.success(t('template.createSuccess'))
    emit('created', project)
    close()
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('template.createFailed'))
  }
}

function difficultyTheme(d: string): 'primary' | 'warning' | 'danger' {
  if (d === 'advanced') return 'danger'
  if (d === 'intermediate') return 'warning'
  return 'primary'
}
</script>

<template>
  <t-dialog
    :visible="visible"
    :header="t('template.dialogTitle')"
    :width="960"
    :footer="false"
    destroy-on-close
    @update:visible="emit('update:visible', $event)"
    @close="close"
  >
    <div class="tpl-dialog">
      <!-- Left: template list -->
      <div class="tpl-list">
        <div v-if="store.loading" class="tpl-loading">
          <t-skeleton theme="paragraph" animation="gradient" :row-col="[1, 1, 1]" />
        </div>
        <div v-else-if="store.templates.length === 0" class="tpl-empty">
          <t-empty :description="t('template.emptyList')" />
        </div>
        <div
          v-else
          v-for="tpl in store.templates"
          :key="tpl.id"
          class="tpl-card"
          :class="{ 'is-active': tpl.id === selectedId }"
          @click="selectTemplate(tpl.id)"
        >
          <div class="tpl-card__header">
            <div class="tpl-card__icon">
              <t-icon :name="tpl.icon || 'layers'" size="18px" />
            </div>
            <div class="tpl-card__name">{{ tpl.name }}</div>
          </div>
          <div class="tpl-card__desc">{{ tpl.description }}</div>
          <div class="tpl-card__footer">
            <t-tag size="small" :theme="difficultyTheme(tpl.difficulty)" variant="light">
              {{ t(`template.difficulty.${tpl.difficulty}`) }}
            </t-tag>
            <t-tag v-for="tag in tpl.tags" :key="tag" size="small" variant="outline">{{ tag }}</t-tag>
          </div>
        </div>
      </div>

      <!-- Right: detail + form -->
      <div class="tpl-detail">
        <div v-if="store.detailLoading" class="tpl-loading">
          <t-skeleton theme="paragraph" animation="gradient" :row-col="[1, 1, 1, 1]" />
        </div>
        <template v-else-if="store.currentDetail">
          <div class="tpl-detail__title">{{ store.currentDetail.name }}</div>
          <div class="tpl-detail__desc">{{ store.currentDetail.description }}</div>

          <div class="tpl-detail__stats">
            <div class="tpl-stat">
              <div class="tpl-stat__num">{{ Object.keys(store.currentDetail.ruleset.steps).length }}</div>
              <div class="tpl-stat__label">{{ t('template.includes.steps') }}</div>
            </div>
            <div class="tpl-stat">
              <div class="tpl-stat__num">{{ store.currentDetail.facts.length }}</div>
              <div class="tpl-stat__label">{{ t('template.includes.facts') }}</div>
            </div>
            <div class="tpl-stat">
              <div class="tpl-stat__num">{{ store.currentDetail.concepts.length }}</div>
              <div class="tpl-stat__label">{{ t('template.includes.concepts') }}</div>
            </div>
            <div class="tpl-stat">
              <div class="tpl-stat__num">{{ store.currentDetail.samples.length }}</div>
              <div class="tpl-stat__label">{{ t('template.includes.samples') }}</div>
            </div>
          </div>

          <div v-if="store.currentDetail.features.length > 0" class="tpl-features">
            <div class="tpl-section-label">{{ t('template.featuresLabel') }}</div>
            <div class="tpl-features__list">
              <t-tag v-for="f in store.currentDetail.features" :key="f" variant="light-outline">
                {{ f }}
              </t-tag>
            </div>
          </div>

          <t-form label-align="top" class="tpl-form">
            <t-form-item :label="t('template.projectNameLabel')" required>
              <t-input v-model="projectName" :placeholder="t('template.projectNamePlaceholder')" />
            </t-form-item>
            <t-form-item :label="t('template.projectDescLabel')">
              <t-input v-model="projectDesc" :placeholder="t('template.projectDescPlaceholder')" />
            </t-form-item>
          </t-form>
        </template>
        <div v-else class="tpl-empty">
          <t-empty :description="t('template.selectHint')" />
        </div>

        <div class="tpl-actions">
          <t-button variant="outline" @click="close">{{ t('common.cancel') }}</t-button>
          <t-button
            theme="primary"
            :loading="store.creating"
            :disabled="!selectedId"
            @click="handleCreate"
          >
            {{ store.creating ? t('template.creating') : t('template.createButton') }}
          </t-button>
        </div>
      </div>
    </div>
  </t-dialog>
</template>

<style scoped>
.tpl-dialog {
  display: grid;
  grid-template-columns: 300px 1fr;
  gap: 20px;
  min-height: 480px;
}

.tpl-list {
  border-right: 1px solid var(--ordo-border-color);
  padding-right: 16px;
  max-height: 560px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.tpl-card {
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  padding: 12px 14px;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.tpl-card:hover {
  border-color: var(--ordo-accent);
}

.tpl-card.is-active {
  border-color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
}

.tpl-card__header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tpl-card__icon {
  width: 28px;
  height: 28px;
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-accent-bg);
  color: var(--ordo-accent);
  display: flex;
  align-items: center;
  justify-content: center;
}

.tpl-card__name {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.tpl-card__desc {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  line-height: 1.5;
}

.tpl-card__footer {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 2px;
}

.tpl-detail {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.tpl-detail__title {
  font-size: 18px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.tpl-detail__desc {
  font-size: 13px;
  color: var(--ordo-text-secondary);
  line-height: 1.6;
}

.tpl-detail__stats {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}

.tpl-stat {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  padding: 12px;
  text-align: center;
}

.tpl-stat__num {
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-accent);
}

.tpl-stat__label {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  margin-top: 2px;
}

.tpl-section-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  margin-bottom: 6px;
}

.tpl-features__list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.tpl-form {
  margin-top: 4px;
}

.tpl-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: auto;
  padding-top: 16px;
  border-top: 1px solid var(--ordo-border-color);
}

.tpl-loading,
.tpl-empty {
  padding: 24px;
  display: flex;
  justify-content: center;
  align-items: center;
}
</style>
