<template>
  <div class="marketplace-view">
    <div class="marketplace-header">
      <div class="marketplace-header__title">
        <h1>{{ t('marketplace.title') }}</h1>
        <p class="marketplace-header__subtitle">{{ t('marketplace.subtitle') }}</p>
      </div>

      <!-- GitHub connection banner -->
      <div v-if="!githubStore.status.connected" class="github-connect-banner">
        <t-icon name="logo-github" size="20" />
        <span>{{ t('marketplace.connectPrompt') }}</span>
        <t-button size="small" :loading="connecting" @click="handleConnect">
          {{ t('marketplace.connectBtn') }}
        </t-button>
      </div>
      <div v-else class="github-connected-badge">
        <t-icon name="logo-github" size="16" />
        <span>{{ githubStore.status.login }}</span>
      </div>
    </div>

    <!-- Search bar -->
    <div class="marketplace-search">
      <t-input
        v-model="query"
        :placeholder="t('marketplace.searchPlaceholder')"
        clearable
        @change="onSearch"
        @clear="onSearch"
      >
        <template #prefix-icon><t-icon name="search" /></template>
      </t-input>
      <t-select v-model="sort" style="width: 160px" @change="onSearch">
        <t-option value="stars" :label="t('marketplace.sortStars')" />
        <t-option value="updated" :label="t('marketplace.sortUpdated')" />
      </t-select>
    </div>

    <!-- Loading skeleton -->
    <div v-if="loading" class="marketplace-grid">
      <t-skeleton
        v-for="i in 8"
        :key="i"
        class="marketplace-card-skeleton"
        :row-col="skeletonRow"
      />
    </div>

    <!-- Empty state -->
    <div v-else-if="!items.length" class="marketplace-placeholder">
      <t-icon name="map" size="64" style="color: var(--td-text-color-placeholder)" />
      <p>{{ t('marketplace.noResults') }}</p>
    </div>

    <!-- Template grid -->
    <div v-else class="marketplace-grid">
      <div
        v-for="item in items"
        :key="item.full_name"
        class="marketplace-card"
        @click="openDetail(item)"
      >
        <div class="marketplace-card__header">
          <img :src="item.owner_avatar" :alt="item.owner_login" class="marketplace-card__avatar" />
          <div class="marketplace-card__repo">
            <span class="marketplace-card__repo-name">{{ item.full_name }}</span>
          </div>
          <span class="marketplace-card__stars">
            <t-icon name="star" size="14" />
            {{ item.stars }}
          </span>
        </div>
        <div class="marketplace-card__body">
          <p>{{ item.description || t('marketplace.noDescription') }}</p>
        </div>
        <div class="marketplace-card__footer">
          <t-tag
            v-for="topic in item.topics.slice(0, 4)"
            :key="topic"
            size="small"
            variant="outline"
          >
            {{ topic }}
          </t-tag>
          <span class="marketplace-card__updated">{{ formatDate(item.updated_at) }}</span>
        </div>
      </div>
    </div>

    <!-- Pagination -->
    <div v-if="totalCount > perPage" class="marketplace-pagination">
      <t-pagination
        :current="page"
        :total="totalCount"
        :page-size="perPage"
        :page-size-options="[]"
        @current-change="onPageChange"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin } from 'tdesign-vue-next';
import { useGithubStore } from '@/stores/github';
import { marketplaceApi } from '@/api/platform-client';
import { useAuthStore } from '@/stores/auth';
import type { MarketplaceItem } from '@/api/types';

const { t } = useI18n();
const router = useRouter();
const authStore = useAuthStore();
const githubStore = useGithubStore();

const query = ref('');
const sort = ref<'stars' | 'updated'>('stars');
const page = ref(1);
const perPage = 24;
const items = ref<MarketplaceItem[]>([]);
const totalCount = ref(0);
const loading = ref(false);
const connecting = ref(false);

const skeletonRow = [{ width: '100%', height: '140px' }];

async function fetchItems() {
  if (!authStore.token) return;
  loading.value = true;
  try {
    const resp = await marketplaceApi.search(authStore.token, {
      q: query.value || undefined,
      sort: sort.value,
      page: page.value,
      per_page: perPage,
    });
    items.value = resp.items;
    totalCount.value = resp.total_count;
  } catch (e: any) {
    MessagePlugin.error(e.message || t('marketplace.loadError'));
  } finally {
    loading.value = false;
  }
}

function onSearch() {
  page.value = 1;
  fetchItems();
}

function onPageChange(p: number) {
  page.value = p;
  fetchItems();
}

function openDetail(item: MarketplaceItem) {
  const [owner, repo] = item.full_name.split('/');
  router.push({ name: 'marketplace-detail', params: { owner, repo } });
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString();
  } catch {
    return iso;
  }
}

async function handleConnect() {
  connecting.value = true;
  try {
    await githubStore.connect();
    fetchItems();
  } catch (e: any) {
    MessagePlugin.error(e.message || t('marketplace.connectError'));
  } finally {
    connecting.value = false;
  }
}

onMounted(async () => {
  await githubStore.fetchStatus();
  fetchItems();
});
</script>

<style scoped>
.marketplace-view {
  max-width: 1200px;
  margin: 0 auto;
  padding: 24px;
}

.marketplace-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 24px;
  flex-wrap: wrap;
}

.marketplace-header__title h1 {
  margin: 0 0 4px;
  font-size: 22px;
  font-weight: 600;
}

.marketplace-header__subtitle {
  margin: 0;
  color: var(--td-text-color-secondary);
  font-size: 14px;
}

.github-connect-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 16px;
  background: var(--td-bg-color-container);
  border: 1px solid var(--td-component-border);
  border-radius: var(--td-radius-medium);
  flex-shrink: 0;
}

.github-connected-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--td-success-color-light);
  color: var(--td-success-color);
  border-radius: var(--td-radius-medium);
  font-size: 13px;
  font-weight: 500;
  flex-shrink: 0;
}

.marketplace-search {
  display: flex;
  gap: 12px;
  margin-bottom: 24px;
}

.marketplace-search .t-input {
  flex: 1;
}

.marketplace-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 80px 0;
  color: var(--td-text-color-secondary);
}

.marketplace-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
}

.marketplace-card-skeleton {
  height: 140px;
  border-radius: var(--td-radius-medium);
}

.marketplace-card {
  padding: 16px;
  background: var(--td-bg-color-container);
  border: 1px solid var(--td-component-border);
  border-radius: var(--td-radius-medium);
  cursor: pointer;
  transition:
    box-shadow 0.2s,
    border-color 0.2s;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.marketplace-card:hover {
  border-color: var(--td-brand-color);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
}

.marketplace-card__header {
  display: flex;
  align-items: center;
  gap: 10px;
}

.marketplace-card__avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  flex-shrink: 0;
}

.marketplace-card__repo {
  flex: 1;
  min-width: 0;
}

.marketplace-card__repo-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--td-text-color-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: block;
}

.marketplace-card__stars {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 12px;
  color: var(--td-text-color-secondary);
  flex-shrink: 0;
}

.marketplace-card__body p {
  margin: 0;
  font-size: 13px;
  color: var(--td-text-color-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.marketplace-card__footer {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px;
}

.marketplace-card__updated {
  margin-left: auto;
  font-size: 11px;
  color: var(--td-text-color-placeholder);
}

.marketplace-pagination {
  margin-top: 24px;
  display: flex;
  justify-content: center;
}
</style>
