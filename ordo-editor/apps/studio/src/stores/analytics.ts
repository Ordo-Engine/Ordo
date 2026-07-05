import { defineStore } from 'pinia';
import { ref } from 'vue';
import { analyticsApi } from '@/api/platform-client';
import { useAuthStore } from './auth';
import type { AnalyticsResponse } from '@/api/types';

export const useAnalyticsStore = defineStore('analytics', () => {
  const auth = useAuthStore();

  const data = ref<AnalyticsResponse | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const range = ref<string>('24h');

  async function fetch(orgId: string, projectId: string) {
    if (!auth.token) return;
    loading.value = true;
    error.value = null;
    try {
      data.value = await analyticsApi.get(auth.token, orgId, projectId, { range: range.value });
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to load analytics';
      data.value = null;
    } finally {
      loading.value = false;
    }
  }

  return { data, loading, error, range, fetch };
});
