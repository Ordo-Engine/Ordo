/**
 * ordo-server Engine API client
 *
 * All requests go through ordo-platform's authenticated proxy:
 *   /api/v1/engine/{projectId}/{...path}
 *
 * The proxy validates the JWT and injects X-Tenant-ID automatically.
 */

import type { ExecuteRequest, ExecuteResponse, RuleSetInfo, VersionListResponse } from './types'

const BASE = '/api/v1/engine'

async function request<T>(
  projectId: string,
  path: string,
  options: RequestInit & { token: string },
): Promise<T> {
  const { token, ...init } = options
  const resp = await fetch(`${BASE}/${projectId}/${path}`, {
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

export const engineApi = {
  // ── Rulesets ────────────────────────────────────────────────────────────────

  listRulesets(token: string, projectId: string): Promise<RuleSetInfo[]> {
    return request(projectId, 'rulesets', { method: 'GET', token })
  },

  getRuleset(token: string, projectId: string, name: string): Promise<unknown> {
    return request(projectId, `rulesets/${name}`, { method: 'GET', token })
  },

  saveRuleset(token: string, projectId: string, name: string, body: unknown): Promise<RuleSetInfo> {
    // ordo-server uses POST /rulesets for both create and update (upsert)
    return request(projectId, 'rulesets', {
      method: 'POST',
      token,
      body: JSON.stringify(body),
    })
  },

  createRuleset(token: string, projectId: string, body: unknown): Promise<RuleSetInfo> {
    return request(projectId, 'rulesets', {
      method: 'POST',
      token,
      body: JSON.stringify(body),
    })
  },

  deleteRuleset(token: string, projectId: string, name: string): Promise<void> {
    return request(projectId, `rulesets/${name}`, { method: 'DELETE', token })
  },

  // ── Versions ─────────────────────────────────────────────────────────────────

  listVersions(token: string, projectId: string, name: string): Promise<VersionListResponse> {
    return request(projectId, `rulesets/${name}/versions`, { method: 'GET', token })
  },

  rollback(token: string, projectId: string, name: string, seq: number): Promise<void> {
    return request(projectId, `rulesets/${name}/rollback`, {
      method: 'POST',
      token,
      body: JSON.stringify({ seq }),
    })
  },

  // ── Execution ────────────────────────────────────────────────────────────────

  execute(
    token: string,
    projectId: string,
    name: string,
    req: ExecuteRequest,
  ): Promise<ExecuteResponse> {
    return request(projectId, `execute/${name}`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    })
  },

  eval(token: string, projectId: string, expression: string, context?: Record<string, unknown>) {
    return request(projectId, 'eval', {
      method: 'POST',
      token,
      body: JSON.stringify({ expression, context }),
    })
  },
}
