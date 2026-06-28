import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { catalogApi } from '@/api/catalog-client';
import { useAuthStore } from './auth';
import type {
  ConceptDefinition,
  DecisionContract,
  FactDataType,
  FactDefinition,
} from '@/api/types';
import type { SchemaField } from '@ordo-engine/editor-core';

// Maps FactDataType → editor SchemaFieldType
function mapDataType(dt: FactDataType): SchemaField['type'] {
  switch (dt) {
    case 'number':
      return 'number';
    case 'boolean':
      return 'boolean';
    case 'object':
      return 'object';
    case 'date':
      return 'string'; // dates serialise as strings
    default:
      return 'string';
  }
}

export const useCatalogStore = defineStore('catalog', () => {
  const auth = useAuthStore();

  const facts = ref<FactDefinition[]>([]);
  const concepts = ref<ConceptDefinition[]>([]);
  const contracts = ref<DecisionContract[]>([]);
  const loading = ref(false);
  const currentProjectId = ref<string | null>(null);

  /**
   * Fetch all catalog assets for a project (parallel).
   * Safe to call multiple times — idempotent if projectId unchanged.
   */
  async function fetchAll(projectId: string) {
    if (!auth.token) return;
    loading.value = true;
    currentProjectId.value = projectId;
    try {
      const [f, c, ct] = await Promise.all([
        catalogApi.listFacts(auth.token, projectId),
        catalogApi.listConcepts(auth.token, projectId),
        catalogApi.listContracts(auth.token, projectId),
      ]);
      facts.value = f;
      concepts.value = c;
      contracts.value = ct;
    } finally {
      loading.value = false;
    }
  }

  // ── Facts ──────────────────────────────────────────────────────────────────

  async function upsertFact(fact: Omit<FactDefinition, 'created_at' | 'updated_at'>) {
    if (!auth.token || !currentProjectId.value) return;
    const saved = await catalogApi.upsertFact(auth.token, currentProjectId.value, fact);
    const idx = facts.value.findIndex((f) => f.name === saved.name);
    if (idx >= 0) facts.value[idx] = saved;
    else facts.value.push(saved);
    return saved;
  }

  async function deleteFact(name: string) {
    if (!auth.token || !currentProjectId.value) return;
    await catalogApi.deleteFact(auth.token, currentProjectId.value, name);
    facts.value = facts.value.filter((f) => f.name !== name);
  }

  // ── Concepts ───────────────────────────────────────────────────────────────

  async function upsertConcept(concept: Omit<ConceptDefinition, 'created_at' | 'updated_at'>) {
    if (!auth.token || !currentProjectId.value) return;
    const saved = await catalogApi.upsertConcept(auth.token, currentProjectId.value, concept);
    const idx = concepts.value.findIndex((c) => c.name === saved.name);
    if (idx >= 0) concepts.value[idx] = saved;
    else concepts.value.push(saved);
    return saved;
  }

  async function deleteConcept(name: string) {
    if (!auth.token || !currentProjectId.value) return;
    await catalogApi.deleteConcept(auth.token, currentProjectId.value, name);
    concepts.value = concepts.value.filter((c) => c.name !== name);
  }

  // ── Contracts ──────────────────────────────────────────────────────────────

  async function upsertContract(
    rulesetName: string,
    contract: Omit<DecisionContract, 'ruleset_name' | 'updated_at'>
  ) {
    if (!auth.token || !currentProjectId.value) return;
    const saved = await catalogApi.upsertContract(
      auth.token,
      currentProjectId.value,
      rulesetName,
      contract
    );
    const idx = contracts.value.findIndex((c) => c.ruleset_name === saved.ruleset_name);
    if (idx >= 0) contracts.value[idx] = saved;
    else contracts.value.push(saved);
    return saved;
  }

  async function deleteContract(rulesetName: string) {
    if (!auth.token || !currentProjectId.value) return;
    await catalogApi.deleteContract(auth.token, currentProjectId.value, rulesetName);
    contracts.value = contracts.value.filter((c) => c.ruleset_name !== rulesetName);
  }

  // ── Derived ────────────────────────────────────────────────────────────────

  /** All facts + concepts as SchemaField[] for editor field suggestions */
  const schemaFields = computed<SchemaField[]>(() => [
    ...facts.value.map((f) => ({
      name: f.name,
      type: mapDataType(f.data_type),
      description: f.description,
      required: f.null_policy === 'error',
    })),
    ...concepts.value.map((c) => ({
      name: c.name,
      type: mapDataType(c.data_type),
      description: c.description,
    })),
  ]);

  /** All known field names (facts + concepts) for dependency pickers */
  const allFieldNames = computed(() => [
    ...facts.value.map((f) => f.name),
    ...concepts.value.map((c) => c.name),
  ]);

  function reset() {
    facts.value = [];
    concepts.value = [];
    contracts.value = [];
    currentProjectId.value = null;
  }

  return {
    facts,
    concepts,
    contracts,
    loading,
    currentProjectId,
    schemaFields,
    allFieldNames,
    fetchAll,
    upsertFact,
    deleteFact,
    upsertConcept,
    deleteConcept,
    upsertContract,
    deleteContract,
    reset,
  };
});
