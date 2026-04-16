/**
 * ordo-platform API client
 * Handles auth, organizations, projects, and members.
 */

import { i18n } from '@/i18n'
import type {
  AppendRulesetHistoryEntry,
  AuthResponse,
  CreateFromTemplatePayload,
  Member,
  OrgResponse,
  Organization,
  Project,
  ProjectTestRunResult,
  Role,
  RulesetHistoryResponse,
  TemplateDetail,
  TemplateMetadata,
  TestCase,
  TestCaseInput,
  TestRunResult,
  UserInfo,
} from './types'

const BASE = '/api/v1'

function currentLocale(): string {
  try {
    return (i18n.global.locale as any).value || 'en'
  } catch {
    return 'en'
  }
}

// ── HTTP helper ───────────────────────────────────────────────────────────────

async function request<T>(
  path: string,
  options: RequestInit & { token?: string } = {},
): Promise<T> {
  const { token, ...init } = options
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    'Accept-Language': currentLocale(),
    ...(init.headers as Record<string, string>),
  }
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }

  const resp = await fetch(`${BASE}${path}`, { ...init, headers })
  if (!resp.ok) {
    let errMsg = `HTTP ${resp.status}`
    try {
      const body = await resp.json()
      errMsg = body.error || errMsg
    } catch {
      // ignore parse errors
    }
    const err = new Error(errMsg) as Error & { status: number }
    err.status = resp.status
    throw err
  }
  if (resp.status === 204) return undefined as T
  return resp.json()
}

// ── Auth ──────────────────────────────────────────────────────────────────────

export const authApi = {
  register(email: string, password: string, display_name: string): Promise<AuthResponse> {
    return request('/auth/register', {
      method: 'POST',
      body: JSON.stringify({ email, password, display_name }),
    })
  },

  login(email: string, password: string): Promise<AuthResponse> {
    return request('/auth/login', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    })
  },

  me(token: string): Promise<UserInfo> {
    return request('/auth/me', { token })
  },

  refresh(token: string): Promise<AuthResponse> {
    return request('/auth/refresh', { method: 'POST', token })
  },

  updateProfile(token: string, patch: { display_name?: string }): Promise<UserInfo> {
    return request('/auth/me', { method: 'PUT', token, body: JSON.stringify(patch) })
  },

  changePassword(token: string, current_password: string, new_password: string): Promise<void> {
    return request('/auth/change-password', {
      method: 'POST',
      token,
      body: JSON.stringify({ current_password, new_password }),
    })
  },
}

// ── Organizations ─────────────────────────────────────────────────────────────

export const orgApi = {
  list(token: string): Promise<OrgResponse[]> {
    return request('/orgs', { token })
  },

  get(token: string, orgId: string): Promise<Organization> {
    return request(`/orgs/${orgId}`, { token })
  },

  create(token: string, name: string, description?: string): Promise<OrgResponse> {
    return request('/orgs', {
      method: 'POST',
      token,
      body: JSON.stringify({ name, description }),
    })
  },

  update(token: string, orgId: string, patch: { name?: string; description?: string }): Promise<OrgResponse> {
    return request(`/orgs/${orgId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(patch),
    })
  },

  delete(token: string, orgId: string): Promise<void> {
    return request(`/orgs/${orgId}`, { method: 'DELETE', token })
  },
}

// ── Members ───────────────────────────────────────────────────────────────────

export const memberApi = {
  list(token: string, orgId: string): Promise<Member[]> {
    return request(`/orgs/${orgId}/members`, { token })
  },

  invite(token: string, orgId: string, email: string, role: Role): Promise<Member> {
    return request(`/orgs/${orgId}/members`, {
      method: 'POST',
      token,
      body: JSON.stringify({ email, role }),
    })
  },

  updateRole(token: string, orgId: string, userId: string, role: Role): Promise<void> {
    return request(`/orgs/${orgId}/members/${userId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify({ role }),
    })
  },

  remove(token: string, orgId: string, userId: string): Promise<void> {
    return request(`/orgs/${orgId}/members/${userId}`, { method: 'DELETE', token })
  },
}

// ── Projects ──────────────────────────────────────────────────────────────────

export const projectApi = {
  list(token: string, orgId: string): Promise<Project[]> {
    return request(`/orgs/${orgId}/projects`, { token })
  },

  get(token: string, orgId: string, projectId: string): Promise<Project> {
    return request(`/orgs/${orgId}/projects/${projectId}`, { token })
  },

  create(token: string, orgId: string, name: string, description?: string): Promise<Project> {
    return request(`/orgs/${orgId}/projects`, {
      method: 'POST',
      token,
      body: JSON.stringify({ name, description }),
    })
  },

  update(
    token: string,
    orgId: string,
    projectId: string,
    patch: { name?: string; description?: string },
  ): Promise<Project> {
    return request(`/orgs/${orgId}/projects/${projectId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(patch),
    })
  },

  delete(token: string, orgId: string, projectId: string): Promise<void> {
    return request(`/orgs/${orgId}/projects/${projectId}`, { method: 'DELETE', token })
  },
}

// ── Templates ────────────────────────────────────────────────────────────────

export const templateApi = {
  list(token: string): Promise<TemplateMetadata[]> {
    return request('/templates', { token })
  },

  get(token: string, id: string): Promise<TemplateDetail> {
    return request(`/templates/${id}`, { token })
  },

  createProject(
    token: string,
    orgId: string,
    payload: CreateFromTemplatePayload,
  ): Promise<Project> {
    return request(`/orgs/${orgId}/projects/from-template`, {
      method: 'POST',
      token,
      body: JSON.stringify(payload),
    })
  },
}

// ── Test Cases ───────────────────────────────────────────────────────────────

export const testApi = {
  list(token: string, projectId: string, rulesetName: string): Promise<TestCase[]> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests`, {
      token,
    })
  },

  create(token: string, projectId: string, rulesetName: string, tc: TestCaseInput): Promise<TestCase> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests`, {
      method: 'POST',
      token,
      body: JSON.stringify(tc),
    })
  },

  update(
    token: string,
    projectId: string,
    rulesetName: string,
    id: string,
    tc: TestCaseInput,
  ): Promise<TestCase> {
    return request(
      `/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/${id}`,
      { method: 'PUT', token, body: JSON.stringify(tc) },
    )
  },

  delete(token: string, projectId: string, rulesetName: string, id: string): Promise<void> {
    return request(
      `/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/${id}`,
      { method: 'DELETE', token },
    )
  },

  runAll(token: string, projectId: string, rulesetName: string): Promise<TestRunResult[]> {
    return request(
      `/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/run`,
      { method: 'POST', token },
    )
  },

  runOne(token: string, projectId: string, rulesetName: string, testId: string): Promise<TestRunResult> {
    return request(
      `/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/${testId}/run`,
      { method: 'POST', token },
    )
  },

  runProject(token: string, projectId: string): Promise<ProjectTestRunResult> {
    return request(`/projects/${projectId}/tests/run`, { token })
  },

  /** Returns a download URL (use window.open or anchor href). */
  exportUrl(projectId: string, rulesetName: string, format: 'yaml' | 'json' = 'yaml'): string {
    return `${BASE}/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/export?format=${format}`
  },
}

// ── Ruleset History ──────────────────────────────────────────────────────────

export const rulesetHistoryApi = {
  list(token: string, projectId: string, rulesetName: string): Promise<RulesetHistoryResponse> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/history`, {
      method: 'GET',
      token,
    })
  },

  append(
    token: string,
    projectId: string,
    rulesetName: string,
    entries: AppendRulesetHistoryEntry[],
  ): Promise<RulesetHistoryResponse> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/history`, {
      method: 'POST',
      token,
      body: JSON.stringify({ entries }),
    })
  },
}
