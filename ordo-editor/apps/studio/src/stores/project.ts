import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { projectApi, rulesetDraftApi, subRuleApi } from '@/api/platform-client';
import { normalizeRuleset } from '@/utils/ruleset';
import { useAuthStore } from './auth';
import { useOrgStore } from './org';
import type {
  DraftConflictResponse,
  Project,
  ProjectRuleset,
  ProjectRulesetMeta,
  RuleSetInfo,
  SubRuleScope,
} from '@/api/types';
import type { RuleSet } from '@ordo-engine/editor-core';

const CURRENT_PROJECT_KEY = 'ordo_studio_current_project';

export interface OpenTab {
  name: string;
  ruleset: RuleSet;
  dirty: boolean;
  /** Platform draft sequence number for optimistic locking */
  draft_seq: number;
  /** 'sub_rule' when this tab holds a managed SubRule asset draft */
  kind?: 'sub_rule';
  /** SubRule asset scope (only set when kind === 'sub_rule') */
  subRuleScope?: SubRuleScope;
}

export const useProjectStore = defineStore('project', () => {
  const auth = useAuthStore();
  const orgStore = useOrgStore();

  const projects = ref<Project[]>([]);
  const currentProject = ref<Project | null>(null);
  const rulesets = ref<RuleSetInfo[]>([]);
  const draftMetas = ref<ProjectRulesetMeta[]>([]);
  const openTabs = ref<OpenTab[]>([]);
  const activeTabName = ref<string | null>(null);
  const loading = ref(false);

  const currentProjectId = computed(() => currentProject.value?.id ?? null);
  const activeTab = computed(
    () => openTabs.value.find((t) => t.name === activeTabName.value) ?? null
  );

  function rebuildRulesets() {
    rulesets.value = draftMetas.value
      .map<RuleSetInfo>((draft) => ({
        name: draft.name,
        version: draft.draft_version ?? draft.published_version ?? '1.0.0',
        published_version: draft.published_version,
        description: '',
      }))
      .sort((left, right) => left.name.localeCompare(right.name));
  }

  function upsertDraftMeta(meta: ProjectRulesetMeta) {
    const index = draftMetas.value.findIndex((item) => item.name === meta.name);
    if (index === -1) {
      draftMetas.value.push(meta);
    } else {
      draftMetas.value[index] = meta;
    }
    rebuildRulesets();
  }

  function pickDraftMeta(ruleset: ProjectRuleset): ProjectRulesetMeta {
    return {
      id: ruleset.id,
      project_id: ruleset.project_id,
      name: ruleset.name,
      draft_seq: ruleset.draft_seq,
      draft_updated_at: ruleset.draft_updated_at,
      draft_updated_by: ruleset.draft_updated_by,
      draft_version: ruleset.draft_version,
      published_version: ruleset.published_version,
      published_at: ruleset.published_at,
      created_at: ruleset.created_at,
    };
  }

  async function fetchProjects(orgId: string) {
    if (!auth.token) return;
    loading.value = true;
    try {
      projects.value = await projectApi.list(auth.token, orgId);
      const savedId = localStorage.getItem(CURRENT_PROJECT_KEY);
      if (!currentProject.value && projects.value.length > 0) {
        const target = projects.value.find((p) => p.id === savedId) ?? projects.value[0];
        await selectProject(target);
      }
    } finally {
      loading.value = false;
    }
  }

  async function selectProject(project: Project) {
    currentProject.value = project;
    localStorage.setItem(CURRENT_PROJECT_KEY, project.id);
    await fetchRulesets();
  }

  async function fetchRulesets() {
    if (!auth.token || !currentProject.value) return;
    const org = orgStore.currentOrg;
    if (org) {
      try {
        draftMetas.value = await rulesetDraftApi.list(auth.token, org.id, currentProject.value.id);
      } catch {
        draftMetas.value = [];
      }
    } else {
      draftMetas.value = [];
    }

    rebuildRulesets();
  }

  async function openRuleset(name: string) {
    if (!auth.token || !currentProject.value) return;
    // Already open?
    const existing = openTabs.value.find((t) => t.name === name);
    if (existing) {
      activeTabName.value = name;
      return;
    }

    const org = orgStore.currentOrg;
    if (!org) throw new Error('No active org');
    const draft = await rulesetDraftApi.get(auth.token, org.id, currentProject.value.id, name);
    const ruleset = normalizeRuleset(draft.draft, name);
    const draft_seq = draft.draft_seq;
    upsertDraftMeta(pickDraftMeta(draft));

    openTabs.value.push({ name, ruleset, dirty: false, draft_seq });
    activeTabName.value = name;
  }

  async function openSubRule(name: string, scope: SubRuleScope = 'project') {
    if (!auth.token || !currentProject.value) return;
    const tabName = `§${name}`;
    const existing = openTabs.value.find((t) => t.name === tabName);
    if (existing) {
      activeTabName.value = tabName;
      return;
    }
    const org = orgStore.currentOrg;
    if (!org) throw new Error('No active org');
    const asset =
      scope === 'org'
        ? await subRuleApi.getOrg(auth.token, org.id, name)
        : await subRuleApi.getProject(auth.token, org.id, currentProject.value.id, name);
    const ruleset = normalizeRuleset(asset.draft, name);
    openTabs.value.push({
      name: tabName,
      ruleset,
      dirty: false,
      draft_seq: asset.draft_seq,
      kind: 'sub_rule',
      subRuleScope: scope,
    });
    activeTabName.value = tabName;
  }

  function updateActiveRuleset(ruleset: RuleSet) {
    const tab = openTabs.value.find((t) => t.name === activeTabName.value);
    if (tab) {
      tab.ruleset = ruleset;
      tab.dirty = true;
    }
  }

  function setTabRuleset(name: string, ruleset: RuleSet, dirty = true) {
    const tab = openTabs.value.find((t) => t.name === name);
    if (!tab) return;
    tab.ruleset = ruleset;
    tab.dirty = dirty;
  }

  /**
   * Save as draft via platform API (optimistic locking).
   * Returns null on success or a DraftConflictResponse if there's a seq conflict.
   */
  async function saveRuleset(name: string): Promise<DraftConflictResponse | null> {
    if (!auth.token || !currentProject.value) throw new Error('No active project');
    const tab = openTabs.value.find((t) => t.name === name);
    if (!tab) throw new Error('Ruleset not open');

    const org = orgStore.currentOrg;
    if (!org) throw new Error('No active org');

    if (tab.kind === 'sub_rule') {
      const assetName = name.startsWith('§') ? name.slice(1) : name;
      const asset =
        tab.subRuleScope === 'org'
          ? await subRuleApi.saveOrg(auth.token, org.id, assetName, {
              name: assetName,
              draft: tab.ruleset as any,
              input_schema: [],
              output_schema: [],
              expected_seq: tab.draft_seq,
            })
          : await subRuleApi.saveProject(auth.token, org.id, currentProject.value.id, assetName, {
              name: assetName,
              draft: tab.ruleset as any,
              input_schema: [],
              output_schema: [],
              expected_seq: tab.draft_seq,
            });
      tab.dirty = false;
      tab.draft_seq = asset.draft_seq;
      return null;
    }

    const result = await rulesetDraftApi.save(auth.token, org.id, currentProject.value.id, name, {
      ruleset: tab.ruleset as any,
      expected_seq: tab.draft_seq,
    });

    if ('conflict' in result) {
      return result;
    }

    tab.dirty = false;
    tab.draft_seq = result.draft_seq;
    upsertDraftMeta(pickDraftMeta(result));
    return null;
  }

  async function createRuleset(ruleset: RuleSet) {
    if (!auth.token || !currentProject.value) throw new Error('No active project');
    const org = orgStore.currentOrg;
    const name = ruleset.config.name?.trim();
    if (!org) throw new Error('No active org');
    if (!name) throw new Error('Ruleset name is required');

    const result = await rulesetDraftApi.save(auth.token, org.id, currentProject.value.id, name, {
      ruleset: ruleset as any,
      expected_seq: 0,
    });
    if ('conflict' in result) {
      throw new Error('Ruleset already exists');
    }
    await fetchRulesets();
  }

  async function deleteRuleset(name: string) {
    if (!auth.token || !currentProject.value) throw new Error('No active project');
    const org = orgStore.currentOrg;
    if (!org) throw new Error('No active org');
    await rulesetDraftApi.delete(auth.token, org.id, currentProject.value.id, name);
    closeTab(name);
    await fetchRulesets();
  }

  function closeTab(name: string) {
    const idx = openTabs.value.findIndex((t) => t.name === name);
    if (idx === -1) return;
    openTabs.value.splice(idx, 1);
    if (activeTabName.value === name) {
      activeTabName.value = openTabs.value[Math.max(0, idx - 1)]?.name ?? null;
    }
  }

  async function createProject(orgId: string, name: string, description?: string) {
    if (!auth.token) throw new Error('Not authenticated');
    const p = await projectApi.create(auth.token, orgId, name, description);
    projects.value.push(p);
    return p;
  }

  async function deleteProject(orgId: string, projectId: string) {
    if (!auth.token) throw new Error('Not authenticated');
    await projectApi.delete(auth.token, orgId, projectId);
    projects.value = projects.value.filter((p) => p.id !== projectId);
    if (currentProject.value?.id === projectId) {
      currentProject.value = null;
      rulesets.value = [];
      draftMetas.value = [];
      openTabs.value = [];
      activeTabName.value = null;
    }
  }

  return {
    projects,
    currentProject,
    currentProjectId,
    rulesets,
    draftMetas,
    openTabs,
    activeTabName,
    activeTab,
    loading,
    fetchProjects,
    selectProject,
    fetchRulesets,
    openRuleset,
    openSubRule,
    updateActiveRuleset,
    setTabRuleset,
    saveRuleset,
    createRuleset,
    deleteRuleset,
    closeTab,
    createProject,
    deleteProject,
  };
});
