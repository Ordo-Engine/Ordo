import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { memberRoleApi, roleApi } from '@/api/platform-client';
import type {
  AssignRoleRequest,
  CreateRoleRequest,
  OrgRole,
  UpdateRoleRequest,
  UserRoleAssignment,
} from '@/api/types';
import { useAuthStore } from './auth';

export const useRbacStore = defineStore('rbac', () => {
  const auth = useAuthStore();

  const roles = ref<OrgRole[]>([]);
  const myAssignments = ref<UserRoleAssignment[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const myPermissions = computed<Set<string>>(() => {
    const perms = new Set<string>();
    for (const assignment of myAssignments.value) {
      const role = roles.value.find((r) => r.id === assignment.role_id);
      if (role) {
        for (const p of role.permissions) perms.add(p);
      }
    }
    return perms;
  });

  function can(permission: string): boolean {
    return myPermissions.value.has(permission);
  }

  async function fetchRoles(orgId: string) {
    loading.value = true;
    error.value = null;
    try {
      roles.value = await roleApi.list(auth.token!, orgId);
    } catch (e: any) {
      error.value = e.message;
    } finally {
      loading.value = false;
    }
  }

  async function fetchMyRoles(orgId: string) {
    if (!auth.user) return;
    try {
      myAssignments.value = await memberRoleApi.list(auth.token!, orgId, auth.user.id);
    } catch {
      myAssignments.value = [];
    }
  }

  async function createRole(orgId: string, req: CreateRoleRequest): Promise<OrgRole> {
    const role = await roleApi.create(auth.token!, orgId, req);
    roles.value.push(role);
    return role;
  }

  async function updateRole(
    orgId: string,
    roleId: string,
    req: UpdateRoleRequest
  ): Promise<OrgRole> {
    const updated = await roleApi.update(auth.token!, orgId, roleId, req);
    const idx = roles.value.findIndex((r) => r.id === roleId);
    if (idx !== -1) roles.value[idx] = updated;
    return updated;
  }

  async function deleteRole(orgId: string, roleId: string) {
    await roleApi.delete(auth.token!, orgId, roleId);
    roles.value = roles.value.filter((r) => r.id !== roleId);
  }

  async function assignRole(orgId: string, userId: string, req: AssignRoleRequest) {
    await memberRoleApi.assign(auth.token!, orgId, userId, req);
  }

  async function revokeRole(orgId: string, userId: string, roleId: string) {
    await memberRoleApi.revoke(auth.token!, orgId, userId, roleId);
  }

  function reset() {
    roles.value = [];
    myAssignments.value = [];
    error.value = null;
  }

  return {
    roles,
    myAssignments,
    myPermissions,
    loading,
    error,
    can,
    fetchRoles,
    fetchMyRoles,
    createRole,
    updateRole,
    deleteRole,
    assignRole,
    revokeRole,
    reset,
  };
});
