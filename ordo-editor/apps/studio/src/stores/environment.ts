import { defineStore } from 'pinia';
import { ref } from 'vue';
import { environmentApi } from '@/api/platform-client';
import type {
  CreateEnvironmentRequest,
  ProjectEnvironment,
  SetCanaryRequest,
  UpdateEnvironmentRequest,
} from '@/api/types';
import { useAuthStore } from './auth';

export const useEnvironmentStore = defineStore('environment', () => {
  const auth = useAuthStore();

  const environments = ref<ProjectEnvironment[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function fetchEnvironments(orgId: string, projectId: string) {
    loading.value = true;
    error.value = null;
    try {
      environments.value = await environmentApi.list(auth.token!, orgId, projectId);
    } catch (e: any) {
      error.value = e.message;
    } finally {
      loading.value = false;
    }
  }

  async function createEnvironment(
    orgId: string,
    projectId: string,
    req: CreateEnvironmentRequest
  ): Promise<ProjectEnvironment> {
    const env = await environmentApi.create(auth.token!, orgId, projectId, req);
    environments.value.push(env);
    return env;
  }

  async function updateEnvironment(
    orgId: string,
    projectId: string,
    envId: string,
    req: UpdateEnvironmentRequest
  ): Promise<ProjectEnvironment> {
    const updated = await environmentApi.update(auth.token!, orgId, projectId, envId, req);
    const idx = environments.value.findIndex((e) => e.id === envId);
    if (idx !== -1) environments.value[idx] = updated;
    return updated;
  }

  async function deleteEnvironment(orgId: string, projectId: string, envId: string) {
    await environmentApi.delete(auth.token!, orgId, projectId, envId);
    environments.value = environments.value.filter((e) => e.id !== envId);
  }

  async function setCanary(
    orgId: string,
    projectId: string,
    envId: string,
    req: SetCanaryRequest
  ): Promise<ProjectEnvironment> {
    const updated = await environmentApi.setCanary(auth.token!, orgId, projectId, envId, req);
    const idx = environments.value.findIndex((e) => e.id === envId);
    if (idx !== -1) environments.value[idx] = updated;
    return updated;
  }

  function reset() {
    environments.value = [];
    error.value = null;
  }

  return {
    environments,
    loading,
    error,
    fetchEnvironments,
    createEnvironment,
    updateEnvironment,
    deleteEnvironment,
    setCanary,
    reset,
  };
});
