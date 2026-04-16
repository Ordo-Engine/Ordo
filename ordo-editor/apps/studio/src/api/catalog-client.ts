/**
 * Catalog API client — Fact Catalog, Concept Registry, Decision Contracts
 * All endpoints are project-scoped: /api/v1/projects/:pid/...
 */

import type {
  ConceptDefinition,
  DecisionContract,
  FactDefinition,
} from './types'

const BASE = '/api/v1'

async function request<T>(
  path: string,
  options: RequestInit & { token: string },
): Promise<T> {
  const { token, ...init } = options
  const resp = await fetch(`${BASE}/${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
      ...(init.headers as Record<string, string>),
    },
  })
  if (!resp.ok) {
    let errMsg = `HTTP ${resp.status}`
    try {
      const body = await resp.json()
      errMsg = body.error || errMsg
    } catch {
      // ignore
    }
    const err = new Error(errMsg) as Error & { status: number }
    err.status = resp.status
    throw err
  }
  if (resp.status === 204) return undefined as T
  return resp.json()
}

export const catalogApi = {
  // ── Fact Catalog ─────────────────────────────────────────────────────────────

  listFacts(token: string, projectId: string): Promise<FactDefinition[]> {
    return request(`projects/${projectId}/facts`, { method: 'GET', token })
  },

  upsertFact(token: string, projectId: string, fact: Omit<FactDefinition, 'created_at' | 'updated_at'>): Promise<FactDefinition> {
    return request(`projects/${projectId}/facts`, {
      method: 'POST',
      token,
      body: JSON.stringify(fact),
    })
  },

  deleteFact(token: string, projectId: string, name: string): Promise<void> {
    return request(`projects/${projectId}/facts/${encodeURIComponent(name)}`, {
      method: 'DELETE',
      token,
    })
  },

  // ── Concept Registry ─────────────────────────────────────────────────────────

  listConcepts(token: string, projectId: string): Promise<ConceptDefinition[]> {
    return request(`projects/${projectId}/concepts`, { method: 'GET', token })
  },

  upsertConcept(token: string, projectId: string, concept: Omit<ConceptDefinition, 'created_at' | 'updated_at'>): Promise<ConceptDefinition> {
    return request(`projects/${projectId}/concepts`, {
      method: 'POST',
      token,
      body: JSON.stringify(concept),
    })
  },

  deleteConcept(token: string, projectId: string, name: string): Promise<void> {
    return request(`projects/${projectId}/concepts/${encodeURIComponent(name)}`, {
      method: 'DELETE',
      token,
    })
  },

  // ── Decision Contracts ───────────────────────────────────────────────────────

  listContracts(token: string, projectId: string): Promise<DecisionContract[]> {
    return request(`projects/${projectId}/contracts`, { method: 'GET', token })
  },

  upsertContract(
    token: string,
    projectId: string,
    rulesetName: string,
    contract: Omit<DecisionContract, 'ruleset_name' | 'updated_at'>,
  ): Promise<DecisionContract> {
    return request(`projects/${projectId}/contracts/${encodeURIComponent(rulesetName)}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(contract),
    })
  },

  deleteContract(token: string, projectId: string, rulesetName: string): Promise<void> {
    return request(`projects/${projectId}/contracts/${encodeURIComponent(rulesetName)}`, {
      method: 'DELETE',
      token,
    })
  },
}
