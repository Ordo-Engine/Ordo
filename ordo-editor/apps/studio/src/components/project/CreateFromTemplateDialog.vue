<script setup lang="ts">
import { ref, watch, computed, onBeforeUnmount } from 'vue'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { useTemplateStore } from '@/stores/template'
import { useGithubStore } from '@/stores/github'
import { useAuthStore } from '@/stores/auth'
import { marketplaceApi } from '@/api/platform-client'
import type { MarketplaceDetail, MarketplaceItem, Project, TemplateDetail, TemplateMetadata } from '@/api/types'

const props = defineProps<{
  visible: boolean
  orgId: string
}>()

const emit = defineEmits<{
  (e: 'update:visible', v: boolean): void
  (e: 'created', p: Project): void
}>()

type TemplateSource = 'builtin' | 'github'

const { t } = useI18n()
const store = useTemplateStore()
const githubStore = useGithubStore()
const authStore = useAuthStore()

const source = ref<TemplateSource>('builtin')
const selectedId = ref<string | null>(null)
const selectedRepo = ref<string | null>(null)
const projectName = ref('')
const projectDesc = ref('')
const githubQuery = ref('')

const marketplaceItems = ref<MarketplaceItem[]>([])
const marketplaceDetail = ref<MarketplaceDetail | null>(null)
const marketplaceLoading = ref(false)
const marketplaceDetailLoading = ref(false)
const marketplaceInstalling = ref(false)
let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null

const selectedTemplate = computed<TemplateMetadata | null>(() =>
  store.templates.find((tpl) => tpl.id === selectedId.value) ?? null,
)

const selectedMarketplaceItem = computed<MarketplaceItem | null>(() =>
  marketplaceItems.value.find((item) => item.full_name === selectedRepo.value) ?? null,
)

const activeDetail = computed<TemplateDetail | MarketplaceDetail | null>(() =>
  source.value === 'github' ? marketplaceDetail.value : store.currentDetail,
)

const activeCreating = computed(() =>
  source.value === 'github' ? marketplaceInstalling.value : store.creating,
)

const activeFacts = computed(() => activeDetail.value?.facts ?? [])
const activeConcepts = computed(() => activeDetail.value?.concepts ?? [])
const activeSamples = computed(() => activeDetail.value?.samples ?? [])
const activeFeatures = computed(() => activeDetail.value?.features ?? [])

async function onOpen() {
  source.value = 'builtin'
  selectedId.value = null
  selectedRepo.value = null
  projectName.value = ''
  projectDesc.value = ''
  githubQuery.value = ''
  marketplaceItems.value = []
  marketplaceDetail.value = null
  store.clearDetail()

  await Promise.all([store.fetchTemplates(), githubStore.fetchStatus()])

  if (store.templates.length > 0) {
    await selectBuiltinTemplate(store.templates[0].id)
  }
}

watch(
  () => props.visible,
  async (open) => {
    if (!open) return
    await onOpen()
  },
)

watch(source, async (mode) => {
  projectName.value = ''
  projectDesc.value = ''

  if (mode === 'builtin') {
    if (!selectedId.value && store.templates.length > 0) {
      await selectBuiltinTemplate(store.templates[0].id)
      return
    }
    const tpl = selectedTemplate.value
    if (tpl) {
      projectName.value = `${tpl.name} - ${t('template.copySuffix')}`
    }
    return
  }

  await githubStore.fetchStatus()
  if (!marketplaceItems.value.length) {
    await fetchMarketplaceItems()
  }
  if (!selectedRepo.value && marketplaceItems.value.length > 0) {
    await selectMarketplaceItem(marketplaceItems.value[0])
    return
  }
  const detail = marketplaceDetail.value
  if (detail) {
    projectName.value = detail.name || detail.full_name || ''
  }
})

watch(githubQuery, () => {
  if (source.value !== 'github') return
  if (searchDebounceTimer) clearTimeout(searchDebounceTimer)
  searchDebounceTimer = setTimeout(() => {
    void fetchMarketplaceItems()
  }, 350)
})

onBeforeUnmount(() => {
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer)
  }
})

async function selectBuiltinTemplate(id: string) {
  selectedId.value = id
  await store.fetchDetail(id)
  const tpl = store.templates.find((x) => x.id === id)
  if (tpl) {
    projectName.value = `${tpl.name} - ${t('template.copySuffix')}`
  }
}

async function fetchMarketplaceItems() {
  if (!authStore.token) return
  marketplaceLoading.value = true
  try {
    const resp = await marketplaceApi.search(authStore.token, {
      q: githubQuery.value.trim() || undefined,
      sort: 'stars',
      page: 1,
      per_page: 24,
    })
    marketplaceItems.value = resp.items
    if (selectedRepo.value && !marketplaceItems.value.some((item) => item.full_name === selectedRepo.value)) {
      selectedRepo.value = null
      marketplaceDetail.value = null
    }
    if (!selectedRepo.value && marketplaceItems.value.length > 0) {
      await selectMarketplaceItem(marketplaceItems.value[0])
    }
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('marketplace.loadError'))
  } finally {
    marketplaceLoading.value = false
  }
}

async function selectMarketplaceItem(item: MarketplaceItem) {
  if (!authStore.token) return
  const [owner, repo] = item.full_name.split('/')
  if (!owner || !repo) return
  selectedRepo.value = item.full_name
  marketplaceDetailLoading.value = true
  try {
    marketplaceDetail.value = await marketplaceApi.getItem(authStore.token, owner, repo)
    projectName.value = marketplaceDetail.value.name || marketplaceDetail.value.full_name || repo
  } catch (e: any) {
    marketplaceDetail.value = null
    MessagePlugin.error(e?.message ?? t('marketplace.loadError'))
  } finally {
    marketplaceDetailLoading.value = false
  }
}

async function connectGithub() {
  try {
    await githubStore.connect()
    await fetchMarketplaceItems()
    if (marketplaceItems.value.length > 0) {
      await selectMarketplaceItem(marketplaceItems.value[0])
    }
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('marketplace.connectError'))
  }
}

function close() {
  emit('update:visible', false)
}

async function handleCreate() {
  if (!projectName.value.trim()) {
    MessagePlugin.warning(t('template.projectNameRequired'))
    return
  }

  if (source.value === 'github') {
    if (!selectedRepo.value || !authStore.token) {
      MessagePlugin.warning(t('template.selectRequired'))
      return
    }
    const [owner, repo] = selectedRepo.value.split('/')
    if (!owner || !repo) {
      MessagePlugin.warning(t('template.selectRequired'))
      return
    }
    marketplaceInstalling.value = true
    try {
      const project = await marketplaceApi.install(authStore.token, owner, repo, {
        org_id: props.orgId,
        project_name: projectName.value.trim(),
        project_description: projectDesc.value.trim() || undefined,
      })
      MessagePlugin.success(t('template.createSuccess'))
      emit('created', project)
      close()
    } catch (e: any) {
      MessagePlugin.error(e?.message ?? t('template.createFailed'))
    } finally {
      marketplaceInstalling.value = false
    }
    return
  }

  if (!selectedId.value) {
    MessagePlugin.warning(t('template.selectRequired'))
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

function stepCount(detail: TemplateDetail | MarketplaceDetail | null): number {
  if (!detail?.ruleset?.steps) return 0
  return Object.keys(detail.ruleset.steps).length
}
</script>

<template>
  <t-dialog
    :visible="visible"
    :header="t('template.dialogTitle')"
    :width="1040"
    :footer="false"
    destroy-on-close
    @update:visible="emit('update:visible', $event)"
    @close="close"
  >
    <div class="tpl-source-switch">
      <t-radio-group v-model="source" variant="default-filled">
        <t-radio-button value="builtin">{{ t('template.sourceBuiltin') }}</t-radio-button>
        <t-radio-button value="github">{{ t('template.sourceGithub') }}</t-radio-button>
      </t-radio-group>
    </div>

    <div class="tpl-dialog">
      <div class="tpl-list">
        <template v-if="source === 'github'">
          <div class="tpl-search">
            <t-input
              v-model="githubQuery"
              :placeholder="t('marketplace.searchPlaceholder')"
              clearable
            >
              <template #prefix-icon><t-icon name="search" /></template>
            </t-input>
          </div>

          <div v-if="!githubStore.status.connected" class="tpl-github-connect">
            <div class="tpl-github-connect__title">{{ t('marketplace.connectOptional') }}</div>
            <t-button size="small" theme="primary" variant="outline" @click="connectGithub">
              <template #icon><t-icon name="logo-github" /></template>
              {{ t('marketplace.connectBtn') }}
            </t-button>
          </div>

          <div v-if="marketplaceLoading" class="tpl-loading">
            <t-skeleton theme="paragraph" animation="gradient" :row-col="[1, 1, 1]" />
          </div>
          <div v-else-if="marketplaceItems.length === 0" class="tpl-empty">
            <t-empty :description="t('marketplace.noResults')" />
          </div>
          <div
            v-else
            v-for="item in marketplaceItems"
            :key="item.full_name"
            class="tpl-card"
            :class="{ 'is-active': item.full_name === selectedRepo }"
            @click="selectMarketplaceItem(item)"
          >
            <div class="tpl-card__header">
              <div class="tpl-card__icon">
                <t-icon :name="item.icon || 'logo-github'" size="18px" />
              </div>
              <div class="tpl-card__name">{{ item.name }}</div>
            </div>
            <div class="tpl-card__repo">{{ item.full_name }}</div>
            <div class="tpl-card__desc">
              {{ item.description || t('marketplace.noDescription') }}
            </div>
            <div class="tpl-card__footer">
              <t-tag
                v-if="item.difficulty"
                size="small"
                :theme="difficultyTheme(item.difficulty)"
                variant="light"
              >
                {{ t(`template.difficulty.${item.difficulty}`) }}
              </t-tag>
              <t-tag v-for="tag in (item.tags ?? []).slice(0, 3)" :key="tag" size="small" variant="outline">
                {{ tag }}
              </t-tag>
            </div>
          </div>
        </template>

        <template v-else>
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
            @click="selectBuiltinTemplate(tpl.id)"
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
        </template>
      </div>

      <div class="tpl-detail">
        <div v-if="(source === 'github' && marketplaceDetailLoading) || (source === 'builtin' && store.detailLoading)" class="tpl-loading tpl-loading--detail">
          <t-skeleton theme="article" animation="gradient" :row-col="[1, 1, 1, 1, 1, 1]" />
        </div>
        <template v-else-if="activeDetail">
          <div class="tpl-detail__title">{{ activeDetail.name }}</div>
          <div v-if="source === 'github' && selectedMarketplaceItem" class="tpl-detail__meta">
            <span>{{ selectedMarketplaceItem.full_name }}</span>
          </div>
          <div class="tpl-detail__desc">{{ activeDetail.description }}</div>

          <div class="tpl-detail__stats">
            <div class="tpl-stat">
              <div class="tpl-stat__num">{{ stepCount(activeDetail) }}</div>
              <div class="tpl-stat__label">{{ t('template.includes.steps') }}</div>
            </div>
            <div class="tpl-stat">
              <div class="tpl-stat__num">{{ activeFacts.length }}</div>
              <div class="tpl-stat__label">{{ t('template.includes.facts') }}</div>
            </div>
            <div class="tpl-stat">
              <div class="tpl-stat__num">{{ activeConcepts.length }}</div>
              <div class="tpl-stat__label">{{ t('template.includes.concepts') }}</div>
            </div>
            <div class="tpl-stat">
              <div class="tpl-stat__num">{{ activeSamples.length }}</div>
              <div class="tpl-stat__label">{{ t('template.includes.samples') }}</div>
            </div>
          </div>

          <div v-if="activeFeatures.length > 0" class="tpl-features">
            <div class="tpl-section-label">{{ t('template.featuresLabel') }}</div>
            <div class="tpl-features__list">
              <t-tag v-for="f in activeFeatures" :key="f" variant="light-outline">
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
            :loading="activeCreating"
            :disabled="source === 'github' ? !selectedRepo : !selectedId"
            @click="handleCreate"
          >
            {{ activeCreating ? t('template.creating') : t('template.createButton') }}
          </t-button>
        </div>
      </div>
    </div>
  </t-dialog>
</template>

<style scoped>
.tpl-source-switch {
  margin-bottom: 16px;
}

.tpl-dialog {
  display: grid;
  grid-template-columns: 320px 1fr;
  gap: 20px;
  min-height: 520px;
}

.tpl-list {
  border-right: 1px solid var(--ordo-border-color);
  padding-right: 16px;
  max-height: 620px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.tpl-search {
  position: sticky;
  top: 0;
  z-index: 1;
  background: var(--td-bg-color-container);
  padding-bottom: 8px;
}

.tpl-github-connect {
  border: 1px dashed var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  padding: 18px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  align-items: flex-start;
}

.tpl-github-connect__title {
  font-size: 13px;
  line-height: 1.6;
  color: var(--ordo-text-secondary);
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
  flex-shrink: 0;
}

.tpl-card__name {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.tpl-card__repo {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
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

.tpl-detail__meta {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
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
  min-height: 160px;
}

@media (max-width: 900px) {
  .tpl-dialog {
    grid-template-columns: 1fr;
  }

  .tpl-list {
    border-right: 0;
    border-bottom: 1px solid var(--ordo-border-color);
    padding-right: 0;
    padding-bottom: 16px;
    max-height: 280px;
  }

  .tpl-detail__stats {
    grid-template-columns: repeat(2, 1fr);
  }
}
</style>
