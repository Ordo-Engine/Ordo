import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { orgApi, memberApi } from '@/api/platform-client'
import { useAuthStore } from './auth'
import type { Member, OrgResponse, Organization, Role } from '@/api/types'

const CURRENT_ORG_KEY = 'ordo_studio_current_org'

export const useOrgStore = defineStore('org', () => {
  const auth = useAuthStore()

  const orgs = ref<OrgResponse[]>([])
  const currentOrg = ref<Organization | null>(null)
  const members = ref<Member[]>([])
  const loading = ref(false)

  /** Sub-orgs keyed by parent org id. Populated on demand. */
  const subOrgs = ref<Record<string, OrgResponse[]>>({})

  const currentOrgId = computed(() => currentOrg.value?.id ?? null)

  // Restore last selected org from localStorage
  const _savedOrgId = localStorage.getItem(CURRENT_ORG_KEY)

  async function fetchOrgs() {
    if (!auth.token) return
    loading.value = true
    try {
      orgs.value = await orgApi.list(auth.token)
      // Auto-select saved org or first
      if (!currentOrg.value && orgs.value.length > 0) {
        const target = orgs.value.find((o) => o.id === _savedOrgId) ?? orgs.value[0]
        await selectOrg(target.id)
      }
    } finally {
      loading.value = false
    }
  }

  async function selectOrg(orgId: string) {
    if (!auth.token) return
    currentOrg.value = await orgApi.get(auth.token, orgId)
    members.value = currentOrg.value.members
    localStorage.setItem(CURRENT_ORG_KEY, orgId)
  }

  async function createOrg(name: string, description?: string, parent_org_id?: string) {
    if (!auth.token) throw new Error('Not authenticated')
    const org = await orgApi.create(auth.token, name, description, parent_org_id)
    orgs.value.push(org)
    await selectOrg(org.id)
    return org
  }

  async function fetchSubOrgs(parentOrgId: string) {
    if (!auth.token) return
    const children = await orgApi.listSubOrgs(auth.token, parentOrgId)
    subOrgs.value[parentOrgId] = children
    // Merge into flat orgs list so hierarchy computed props work
    for (const child of children) {
      if (!orgs.value.find((o) => o.id === child.id)) {
        orgs.value.push(child)
      }
    }
  }

  async function createSubOrg(parentOrgId: string, name: string, description?: string) {
    if (!auth.token) throw new Error('Not authenticated')
    const org = await orgApi.create(auth.token, name, description, parentOrgId)
    orgs.value.push(org)
    subOrgs.value[parentOrgId] = [...(subOrgs.value[parentOrgId] ?? []), org]
    // Increment child_count on parent in list
    const parent = orgs.value.find((o) => o.id === parentOrgId)
    if (parent) parent.child_count = (parent.child_count ?? 0) + 1
    return org
  }

  async function deleteSubOrg(subOrgId: string, parentOrgId: string) {
    if (!auth.token) throw new Error('Not authenticated')
    await orgApi.delete(auth.token, subOrgId)
    orgs.value = orgs.value.filter((o) => o.id !== subOrgId)
    subOrgs.value[parentOrgId] = (subOrgs.value[parentOrgId] ?? []).filter(
      (o) => o.id !== subOrgId,
    )
    const parent = orgs.value.find((o) => o.id === parentOrgId)
    if (parent && parent.child_count > 0) parent.child_count -= 1
    if (currentOrg.value?.id === subOrgId) {
      currentOrg.value = null
      members.value = []
      localStorage.removeItem(CURRENT_ORG_KEY)
    }
  }

  async function updateOrg(orgId: string, patch: { name?: string; description?: string }) {
    if (!auth.token) throw new Error('Not authenticated')
    const updated = await orgApi.update(auth.token, orgId, patch)
    const idx = orgs.value.findIndex((o) => o.id === orgId)
    if (idx !== -1) orgs.value[idx] = updated
    if (currentOrg.value?.id === orgId) {
      currentOrg.value = { ...currentOrg.value, ...updated }
    }
    return updated
  }

  async function deleteOrg(orgId: string) {
    if (!auth.token) throw new Error('Not authenticated')
    await orgApi.delete(auth.token, orgId)
    orgs.value = orgs.value.filter((o) => o.id !== orgId)
    if (currentOrg.value?.id === orgId) {
      currentOrg.value = null
      members.value = []
      localStorage.removeItem(CURRENT_ORG_KEY)
      if (orgs.value.length > 0) await selectOrg(orgs.value[0].id)
    }
  }

  async function inviteMember(orgId: string, email: string, role: Role) {
    if (!auth.token) throw new Error('Not authenticated')
    const member = await memberApi.invite(auth.token, orgId, email, role)
    members.value.push(member)
    return member
  }

  async function updateMemberRole(orgId: string, userId: string, role: Role) {
    if (!auth.token) throw new Error('Not authenticated')
    await memberApi.updateRole(auth.token, orgId, userId, role)
    const m = members.value.find((m) => m.user_id === userId)
    if (m) m.role = role
  }

  async function removeMember(orgId: string, userId: string) {
    if (!auth.token) throw new Error('Not authenticated')
    await memberApi.remove(auth.token, orgId, userId)
    members.value = members.value.filter((m) => m.user_id !== userId)
  }

  /** Get the current user's role in the current org */
  function myRole(userId: string): Role | null {
    return members.value.find((m) => m.user_id === userId)?.role ?? null
  }

  function canEdit(userId: string): boolean {
    const r = myRole(userId)
    return r === 'owner' || r === 'admin' || r === 'editor'
  }

  function canAdmin(userId: string): boolean {
    const r = myRole(userId)
    return r === 'owner' || r === 'admin'
  }

  return {
    orgs,
    currentOrg,
    currentOrgId,
    members,
    loading,
    subOrgs,
    fetchOrgs,
    selectOrg,
    createOrg,
    fetchSubOrgs,
    createSubOrg,
    deleteSubOrg,
    updateOrg,
    deleteOrg,
    inviteMember,
    updateMemberRole,
    removeMember,
    myRole,
    canEdit,
    canAdmin,
  }
})
