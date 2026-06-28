<template>
  <div class="marketplace-detail">
    <!-- Loading -->
    <div v-if="loading" class="marketplace-detail__loading">
      <t-loading size="large" />
    </div>

    <!-- Error -->
    <div v-else-if="error" class="marketplace-detail__error">
      <t-icon name="close-circle" size="48" style="color: var(--td-error-color)" />
      <p>{{ error }}</p>
      <t-button @click="router.back()">{{ t('common.back') }}</t-button>
    </div>

    <!-- Content -->
    <template v-else-if="detail">
      <!-- Back nav -->
      <div class="marketplace-detail__nav">
        <t-button variant="text" @click="router.back()">
          <template #icon><t-icon name="chevron-left" /></template>
          {{ t('marketplace.backToMarketplace') }}
        </t-button>
      </div>

      <!-- Hero -->
      <div class="marketplace-detail__hero">
        <img
          v-if="detail.owner_avatar"
          :src="detail.owner_avatar"
          :alt="detail.owner_login"
          class="marketplace-detail__avatar"
        />
        <div class="marketplace-detail__info">
          <h1 class="marketplace-detail__name">{{ detail.name }}</h1>
          <div class="marketplace-detail__meta">
            <a
              :href="detail.github_url"
              target="_blank"
              rel="noopener"
              class="marketplace-detail__repo-link"
            >
              <t-icon name="logo-github" size="14" />
              {{ detail.full_name }}
            </a>
            <span v-if="detail.stars !== undefined" class="marketplace-detail__stars">
              <t-icon name="star" size="14" />
              {{ detail.stars }}
            </span>
          </div>
          <p class="marketplace-detail__desc">{{ detail.description }}</p>
          <div class="marketplace-detail__tags">
            <t-tag
              v-for="tag in detail.tags"
              :key="tag"
              size="small"
              theme="primary"
              variant="light"
            >
              {{ tag }}
            </t-tag>
            <t-tag
              v-for="topic in (detail.topics || []).slice(0, 6)"
              :key="topic"
              size="small"
              variant="outline"
            >
              {{ topic }}
            </t-tag>
          </div>
        </div>

        <!-- Install panel -->
        <div class="marketplace-detail__install-panel">
          <h3>{{ t('marketplace.installTitle') }}</h3>

          <t-form label-width="100px" :colon="true">
            <t-form-item :label="t('marketplace.installOrg')">
              <t-select v-model="selectedOrgId" :placeholder="t('marketplace.selectOrg')">
                <t-option
                  v-for="org in orgStore.orgs"
                  :key="org.id"
                  :value="org.id"
                  :label="org.name"
                />
              </t-select>
            </t-form-item>
            <t-form-item :label="t('marketplace.projectName')">
              <t-input v-model="projectName" :placeholder="detail.name" />
            </t-form-item>
            <t-form-item :label="t('marketplace.projectDesc')">
              <t-textarea
                v-model="projectDesc"
                :placeholder="t('marketplace.projectDescPlaceholder')"
                :rows="2"
              />
            </t-form-item>
          </t-form>

          <t-button
            block
            theme="primary"
            :loading="installing"
            :disabled="!selectedOrgId || !projectName.trim()"
            @click="handleInstall"
          >
            {{ t('marketplace.installBtn') }}
          </t-button>
        </div>
      </div>

      <!-- Detail tabs -->
      <t-tabs v-model="activeTab" class="marketplace-detail__tabs">
        <t-tab-panel value="overview" :label="t('marketplace.tabOverview')">
          <div class="marketplace-detail__section">
            <h3>{{ t('marketplace.factsTitle') }}</h3>
            <t-table
              v-if="detail.facts?.length"
              :data="detail.facts"
              :columns="factColumns"
              size="small"
              row-key="name"
              stripe
            />
            <p v-else class="empty-hint">{{ t('marketplace.noFacts') }}</p>

            <h3>{{ t('marketplace.conceptsTitle') }}</h3>
            <t-table
              v-if="detail.concepts?.length"
              :data="detail.concepts"
              :columns="conceptColumns"
              size="small"
              row-key="name"
              stripe
            />
            <p v-else class="empty-hint">{{ t('marketplace.noConcepts') }}</p>
          </div>
        </t-tab-panel>

        <t-tab-panel
          value="tests"
          :label="`${t('marketplace.tabTests')} (${detail.tests?.length || 0})`"
        >
          <div class="marketplace-detail__section">
            <t-table
              v-if="detail.tests?.length"
              :data="detail.tests"
              :columns="testColumns"
              size="small"
              row-key="id"
              stripe
            />
            <p v-else class="empty-hint">{{ t('marketplace.noTests') }}</p>
          </div>
        </t-tab-panel>

        <t-tab-panel v-if="detail.contract" value="contract" :label="t('marketplace.tabContract')">
          <div class="marketplace-detail__section marketplace-detail__contract">
            <div class="contract-meta">
              <span
                ><b>{{ t('marketplace.contractOwner') }}:</b> {{ detail.contract.owner }}</span
              >
              <span v-if="detail.contract.sla_p99_ms"
                ><b>SLA p99:</b> {{ detail.contract.sla_p99_ms }}ms</span
              >
            </div>
            <h4>{{ t('marketplace.contractInputs') }}</h4>
            <t-table
              :data="detail.contract.input_fields"
              :columns="contractFieldColumns"
              size="small"
              row-key="name"
              stripe
            />
            <h4>{{ t('marketplace.contractOutputs') }}</h4>
            <t-table
              :data="detail.contract.output_fields"
              :columns="contractFieldColumns"
              size="small"
              row-key="name"
              stripe
            />
          </div>
        </t-tab-panel>
      </t-tabs>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin } from 'tdesign-vue-next';
import { useAuthStore } from '@/stores/auth';
import { useOrgStore } from '@/stores/org';
import { marketplaceApi } from '@/api/platform-client';
import type { MarketplaceDetail } from '@/api/types';

const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const authStore = useAuthStore();
const orgStore = useOrgStore();

const owner = route.params.owner as string;
const repo = route.params.repo as string;

const detail = ref<MarketplaceDetail | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const activeTab = ref('overview');

const selectedOrgId = ref('');
const projectName = ref('');
const projectDesc = ref('');
const installing = ref(false);

const factColumns = [
  { colKey: 'name', title: t('marketplace.colName'), width: 180 },
  { colKey: 'data_type', title: t('marketplace.colType'), width: 100 },
  { colKey: 'source', title: t('marketplace.colSource') },
  { colKey: 'description', title: t('marketplace.colDescription') },
];

const conceptColumns = [
  { colKey: 'name', title: t('marketplace.colName'), width: 180 },
  { colKey: 'data_type', title: t('marketplace.colType'), width: 100 },
  { colKey: 'expression', title: t('marketplace.colExpression') },
  { colKey: 'description', title: t('marketplace.colDescription') },
];

const testColumns = [
  { colKey: 'name', title: t('marketplace.colName') },
  { colKey: 'description', title: t('marketplace.colDescription') },
  {
    colKey: 'tags',
    title: t('marketplace.colTags'),
    cell: (_: any, { row }: any) => (row.tags || []).join(', '),
  },
];

const contractFieldColumns = [
  { colKey: 'name', title: t('marketplace.colName'), width: 180 },
  { colKey: 'data_type', title: t('marketplace.colType'), width: 100 },
  { colKey: 'required', title: t('marketplace.colRequired'), width: 80 },
  { colKey: 'description', title: t('marketplace.colDescription') },
];

async function loadDetail() {
  loading.value = true;
  error.value = null;
  try {
    detail.value = await marketplaceApi.getItem(authStore.token!, owner, repo);
    projectName.value = detail.value.name || repo;
  } catch (e: any) {
    error.value = e.message || t('marketplace.loadError');
  } finally {
    loading.value = false;
  }
}

async function handleInstall() {
  if (!selectedOrgId.value || !projectName.value.trim()) return;
  installing.value = true;
  try {
    const project = await marketplaceApi.install(authStore.token!, owner, repo, {
      org_id: selectedOrgId.value,
      project_name: projectName.value.trim(),
      project_description: projectDesc.value.trim() || undefined,
    });
    MessagePlugin.success(t('marketplace.installSuccess', { name: project.name }));
    router.push({
      name: 'project-editor',
      params: { orgId: selectedOrgId.value, projectId: project.id },
    });
  } catch (e: any) {
    MessagePlugin.error(e.message || t('marketplace.installError'));
  } finally {
    installing.value = false;
  }
}

onMounted(async () => {
  await orgStore.fetchOrgs();
  if (orgStore.orgs.length) selectedOrgId.value = orgStore.orgs[0].id;
  await loadDetail();
});
</script>

<style scoped>
.marketplace-detail {
  max-width: 1100px;
  margin: 0 auto;
  padding: 24px;
}

.marketplace-detail__loading,
.marketplace-detail__error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 80px 0;
}

.marketplace-detail__nav {
  margin-bottom: 16px;
}

.marketplace-detail__hero {
  display: flex;
  gap: 24px;
  align-items: flex-start;
  margin-bottom: 32px;
  flex-wrap: wrap;
}

.marketplace-detail__avatar {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  flex-shrink: 0;
}

.marketplace-detail__info {
  flex: 1;
  min-width: 240px;
}

.marketplace-detail__name {
  margin: 0 0 6px;
  font-size: 24px;
  font-weight: 700;
}

.marketplace-detail__meta {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 8px;
  font-size: 13px;
  color: var(--td-text-color-secondary);
}

.marketplace-detail__repo-link {
  display: flex;
  align-items: center;
  gap: 4px;
  color: var(--td-brand-color);
  text-decoration: none;
}

.marketplace-detail__repo-link:hover {
  text-decoration: underline;
}

.marketplace-detail__stars {
  display: flex;
  align-items: center;
  gap: 3px;
}

.marketplace-detail__desc {
  margin: 0 0 10px;
  color: var(--td-text-color-secondary);
  font-size: 14px;
}

.marketplace-detail__tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.marketplace-detail__install-panel {
  width: 320px;
  flex-shrink: 0;
  padding: 20px;
  background: var(--td-bg-color-container);
  border: 1px solid var(--td-component-border);
  border-radius: var(--td-radius-large);
}

.marketplace-detail__install-panel h3 {
  margin: 0 0 16px;
  font-size: 15px;
  font-weight: 600;
}

.marketplace-detail__tabs {
  margin-top: 8px;
}

.marketplace-detail__section {
  padding: 20px 0;
}

.marketplace-detail__section h3 {
  font-size: 15px;
  font-weight: 600;
  margin: 16px 0 8px;
}

.marketplace-detail__section h3:first-child {
  margin-top: 0;
}

.marketplace-detail__contract h4 {
  font-size: 14px;
  font-weight: 600;
  margin: 14px 0 6px;
}

.contract-meta {
  display: flex;
  gap: 24px;
  margin-bottom: 12px;
  font-size: 13px;
  color: var(--td-text-color-secondary);
}

.empty-hint {
  color: var(--td-text-color-placeholder);
  font-size: 13px;
  padding: 16px 0;
}
</style>
