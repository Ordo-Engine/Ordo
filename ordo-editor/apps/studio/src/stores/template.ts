import { defineStore } from 'pinia';
import { ref } from 'vue';
import { templateApi } from '@/api/platform-client';
import { useAuthStore } from './auth';
import type {
  CreateFromTemplatePayload,
  Project,
  TemplateDetail,
  TemplateMetadata,
} from '@/api/types';

export const useTemplateStore = defineStore('template', () => {
  const auth = useAuthStore();

  const templates = ref<TemplateMetadata[]>([]);
  const currentDetail = ref<TemplateDetail | null>(null);
  const loading = ref(false);
  const detailLoading = ref(false);
  const creating = ref(false);
  const error = ref<string | null>(null);

  async function fetchTemplates() {
    if (!auth.token) return;
    loading.value = true;
    error.value = null;
    try {
      templates.value = await templateApi.list(auth.token);
    } catch (e: any) {
      error.value = e?.message ?? String(e);
      templates.value = [];
    } finally {
      loading.value = false;
    }
  }

  async function fetchDetail(id: string) {
    if (!auth.token) return;
    detailLoading.value = true;
    error.value = null;
    try {
      currentDetail.value = await templateApi.get(auth.token, id);
    } catch (e: any) {
      error.value = e?.message ?? String(e);
      currentDetail.value = null;
    } finally {
      detailLoading.value = false;
    }
  }

  async function createFromTemplate(
    orgId: string,
    payload: CreateFromTemplatePayload
  ): Promise<Project> {
    if (!auth.token) throw new Error('Not authenticated');
    creating.value = true;
    error.value = null;
    try {
      return await templateApi.createProject(auth.token, orgId, payload);
    } finally {
      creating.value = false;
    }
  }

  function clearDetail() {
    currentDetail.value = null;
  }

  return {
    templates,
    currentDetail,
    loading,
    detailLoading,
    creating,
    error,
    fetchTemplates,
    fetchDetail,
    createFromTemplate,
    clearDetail,
  };
});
