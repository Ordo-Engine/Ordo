/**
 * ordo-platform API client
 * Handles auth, organizations, projects, and members.
 */

import type { RuleSet } from '@ordo-engine/editor-core';
import { i18n } from '@/i18n';
import type {
  AppendRulesetHistoryEntry,
  SystemConfig,
  AssignRoleRequest,
  AuthResponse,
  BindServerRequest,
  CreateEnvironmentRequest,
  CreateFromTemplatePayload,
  CreateRoleRequest,
  DraftConflictResponse,
  GitHubConnectUrlResponse,
  GitHubStatus,
  InstallMarketplacePayload,
  MarketplaceDetail,
  MarketplaceSearchResponse,
  Member,
  NotificationCount,
  OrgResponse,
  OrgRole,
  Organization,
  PlatformNotification,
  Project,
  ProjectEnvironment,
  ProjectRuleset,
  ProjectRulesetMeta,
  ProjectTestRunResult,
  PublishRequest,
  AdHocTestRunRequest,
  RedeployRequest,
  ReleaseExecution,
  ReleaseExecutionEvent,
  ReleasePolicy,
  ReleaseRequest,
  ReleaseRequestHistoryEntry,
  ReleaseTargetPreview,
  ReviewReleaseRequest,
  Role,
  RollbackPolicy,
  RolloutStrategy,
  RulesetDeployment,
  RulesetHistoryResponse,
  SaveDraftRequest,
  ServerInfo,
  ConnectTokenInfo,
  CreateConnectTokenResponse,
  SetCanaryRequest,
  SaveSubRuleAssetRequest,
  SubRuleAsset,
  SubRuleAssetMeta,
  SubRuleScope,
  TemplateDetail,
  TemplateMetadata,
  TestCase,
  TestCaseInput,
  TestRunRequest,
  TestRunResult,
  ProjectTestRunRequest,
  UpdateEnvironmentRequest,
  UpdateRoleRequest,
  UserInfo,
  UserRoleAssignment,
  AnalyticsResponse,
} from './types';
import type { AiChatRequest, AiStreamEvent, AiProviderOption } from './ai-types';

const BASE = '/api/v1';

/**
 * Base URL for streamed (SSE) requests. Same-origin rewrites (e.g. a CDN proxy)
 * buffer streamed responses and deliver the whole reply at once, which defeats
 * token-by-token streaming. When deployed under an `app.<domain>` host we hit
 * the `api.<domain>` origin directly instead (CORS is configured server-side).
 * Local dev keeps the same-origin path — the dev server streams fine.
 */
function streamBase(): string {
  if (typeof window === 'undefined') return BASE;
  const h = window.location.hostname;
  if (h === 'localhost' || h === '127.0.0.1') return BASE;
  if (h.startsWith('app.')) return `${window.location.protocol}//api.${h.slice(4)}/api/v1`;
  return BASE;
}

export const systemApi = {
  getConfig(): Promise<SystemConfig> {
    return request('/system/config');
  },
};

type PlatformApiError = Error & { status: number; code?: string };

function currentLocale(): string {
  try {
    return (i18n.global.locale as any).value || 'en';
  } catch {
    return 'en';
  }
}

// ── HTTP helper ───────────────────────────────────────────────────────────────

/**
 * Called when an authenticated request returns 401 (the session expired or the
 * token was revoked). The app registers a handler (clear auth + redirect to
 * login) so an expired session doesn't leave the UI stuck in a broken shell.
 */
let onUnauthorized: (() => void) | null = null;
export function setUnauthorizedHandler(fn: () => void): void {
  onUnauthorized = fn;
}

/**
 * Fire the session-expired handler when an *authenticated* call is rejected as
 * unauthorized (the token expired or was revoked). A 401 without a token is a
 * bad-credentials login attempt, not an expired session, so it's ignored here.
 * Every fetch path — the shared `request()` and the hand-rolled streaming/draft
 * fetches — must funnel 401s through this, or an expired session leaves the UI
 * stuck instead of bouncing to login.
 */
function noteUnauthorized(status: number, token?: string): void {
  if (status === 401 && token) onUnauthorized?.();
}

async function request<T>(
  path: string,
  options: RequestInit & { token?: string } = {}
): Promise<T> {
  const { token, ...init } = options;
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    'Accept-Language': currentLocale(),
    ...(init.headers as Record<string, string>),
  };
  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const resp = await fetch(`${BASE}${path}`, { ...init, headers });
  if (!resp.ok) {
    noteUnauthorized(resp.status, token);
    let errMsg = `HTTP ${resp.status}`;
    let errCode: string | undefined;
    try {
      const body = await resp.json();
      errMsg = body.error || errMsg;
      errCode = body.code;
    } catch {
      // ignore parse errors
    }
    const err = new Error(errMsg) as PlatformApiError;
    err.status = resp.status;
    err.code = errCode;
    throw err;
  }
  if (resp.status === 204) return undefined as T;
  return resp.json();
}

async function requestText(
  path: string,
  options: RequestInit & { token?: string } = {}
): Promise<string> {
  const { token, ...init } = options;
  const headers: Record<string, string> = {
    'Accept-Language': currentLocale(),
    ...(init.headers as Record<string, string>),
  };
  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const resp = await fetch(`${BASE}${path}`, { ...init, headers });
  if (!resp.ok) {
    noteUnauthorized(resp.status, token);
    let errMsg = `HTTP ${resp.status}`;
    let errCode: string | undefined;
    try {
      const body = await resp.json();
      errMsg = body.error || errMsg;
      errCode = body.code;
    } catch {
      // ignore parse errors
    }
    const err = new Error(errMsg) as PlatformApiError;
    err.status = resp.status;
    err.code = errCode;
    throw err;
  }
  return resp.text();
}

// ── Auth ──────────────────────────────────────────────────────────────────────

export const aiApi = {
  /** Configured providers + their model lists (only providers with a key set). */
  listModels(token: string): Promise<AiProviderOption[]> {
    return request('/ai/models', { token });
  },

  /**
   * Stream one assistant turn. `onEvent` fires for each normalized SSE event
   * (text delta / tool_start / tool / done / error). The client executes the tool
   * calls it receives, then streams the next turn.
   */
  async chatStream(
    token: string,
    req: AiChatRequest,
    onEvent: (ev: AiStreamEvent) => void,
    signal?: AbortSignal
  ): Promise<void> {
    const resp = await fetch(`${streamBase()}/ai/chat`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Accept-Language': currentLocale(),
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify(req),
      signal,
    });
    if (!resp.ok || !resp.body) {
      noteUnauthorized(resp.status, token);
      let m = `HTTP ${resp.status}`;
      try {
        m = (await resp.json()).error || m;
      } catch {
        // ignore
      }
      throw new Error(m);
    }
    const reader = resp.body.getReader();
    const decoder = new TextDecoder();
    let buf = '';
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      let idx: number;
      while ((idx = buf.indexOf('\n\n')) !== -1) {
        const frame = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        const line = frame.split('\n').find((l) => l.startsWith('data:'));
        if (!line) continue;
        const json = line.slice(5).trim();
        if (!json) continue;
        try {
          onEvent(JSON.parse(json) as AiStreamEvent);
        } catch {
          // ignore malformed frame
        }
      }
    }
  },
};

export const authApi = {
  register(email: string, password: string, display_name: string): Promise<AuthResponse> {
    return request('/auth/register', {
      method: 'POST',
      body: JSON.stringify({ email, password, display_name }),
    });
  },

  login(email: string, password: string): Promise<AuthResponse> {
    return request('/auth/login', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    });
  },

  me(token: string): Promise<UserInfo> {
    return request('/auth/me', { token });
  },

  refresh(token: string): Promise<AuthResponse> {
    return request('/auth/refresh', { method: 'POST', token });
  },

  updateProfile(token: string, patch: { display_name?: string }): Promise<UserInfo> {
    return request('/auth/me', { method: 'PUT', token, body: JSON.stringify(patch) });
  },

  changePassword(token: string, current_password: string, new_password: string): Promise<void> {
    return request('/auth/change-password', {
      method: 'POST',
      token,
      body: JSON.stringify({ current_password, new_password }),
    });
  },
};

// ── Organizations ─────────────────────────────────────────────────────────────

export const orgApi = {
  list(token: string): Promise<OrgResponse[]> {
    return request('/orgs', { token });
  },

  get(token: string, orgId: string): Promise<Organization> {
    return request(`/orgs/${orgId}`, { token });
  },

  create(
    token: string,
    name: string,
    description?: string,
    parent_org_id?: string
  ): Promise<OrgResponse> {
    return request('/orgs', {
      method: 'POST',
      token,
      body: JSON.stringify({ name, description, parent_org_id }),
    });
  },

  listSubOrgs(token: string, orgId: string): Promise<OrgResponse[]> {
    return request(`/orgs/${orgId}/sub-orgs`, { token });
  },

  update(
    token: string,
    orgId: string,
    patch: { name?: string; description?: string }
  ): Promise<OrgResponse> {
    return request(`/orgs/${orgId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(patch),
    });
  },

  delete(token: string, orgId: string): Promise<void> {
    return request(`/orgs/${orgId}`, { method: 'DELETE', token });
  },
};

// ── Members ───────────────────────────────────────────────────────────────────

export const memberApi = {
  list(token: string, orgId: string): Promise<Member[]> {
    return request(`/orgs/${orgId}/members`, { token });
  },

  invite(token: string, orgId: string, email: string, role: Role): Promise<Member> {
    return request(`/orgs/${orgId}/members`, {
      method: 'POST',
      token,
      body: JSON.stringify({ email, role }),
    });
  },

  updateRole(token: string, orgId: string, userId: string, role: Role): Promise<void> {
    return request(`/orgs/${orgId}/members/${userId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify({ role }),
    });
  },

  remove(token: string, orgId: string, userId: string): Promise<void> {
    return request(`/orgs/${orgId}/members/${userId}`, { method: 'DELETE', token });
  },
};

// ── Sub-org member management (parent org context) ────────────────────────────

export const subOrgMemberApi = {
  list(token: string, parentOrgId: string, subOrgId: string): Promise<Member[]> {
    return request(`/orgs/${parentOrgId}/sub-orgs/${subOrgId}/members`, { token });
  },

  add(
    token: string,
    parentOrgId: string,
    subOrgId: string,
    body: { user_id: string; role: Role }
  ): Promise<Member> {
    return request(`/orgs/${parentOrgId}/sub-orgs/${subOrgId}/members`, {
      method: 'POST',
      token,
      body: JSON.stringify(body),
    });
  },

  remove(token: string, parentOrgId: string, subOrgId: string, userId: string): Promise<void> {
    return request(`/orgs/${parentOrgId}/sub-orgs/${subOrgId}/members/${userId}`, {
      method: 'DELETE',
      token,
    });
  },
};

// ── Projects ──────────────────────────────────────────────────────────────────

export const projectApi = {
  list(token: string, orgId: string): Promise<Project[]> {
    return request(`/orgs/${orgId}/projects`, { token });
  },

  get(token: string, orgId: string, projectId: string): Promise<Project> {
    return request(`/orgs/${orgId}/projects/${projectId}`, { token });
  },

  create(token: string, orgId: string, name: string, description?: string): Promise<Project> {
    return request(`/orgs/${orgId}/projects`, {
      method: 'POST',
      token,
      body: JSON.stringify({ name, description }),
    });
  },

  update(
    token: string,
    orgId: string,
    projectId: string,
    patch: { name?: string; description?: string }
  ): Promise<Project> {
    return request(`/orgs/${orgId}/projects/${projectId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(patch),
    });
  },

  delete(token: string, orgId: string, projectId: string): Promise<void> {
    return request(`/orgs/${orgId}/projects/${projectId}`, { method: 'DELETE', token });
  },

  bindServer(
    token: string,
    orgId: string,
    projectId: string,
    payload: BindServerRequest
  ): Promise<void> {
    return request(`/orgs/${orgId}/projects/${projectId}/server`, {
      method: 'PUT',
      token,
      body: JSON.stringify(payload),
    });
  },
};

// ── Templates ────────────────────────────────────────────────────────────────

export const templateApi = {
  list(token: string): Promise<TemplateMetadata[]> {
    return request('/templates', { token });
  },

  get(token: string, id: string): Promise<TemplateDetail> {
    return request(`/templates/${id}`, { token });
  },

  createProject(
    token: string,
    orgId: string,
    payload: CreateFromTemplatePayload
  ): Promise<Project> {
    return request(`/orgs/${orgId}/projects/from-template`, {
      method: 'POST',
      token,
      body: JSON.stringify(payload),
    });
  },
};

// ── Test Cases ───────────────────────────────────────────────────────────────

export const testApi = {
  list(token: string, projectId: string, rulesetName: string): Promise<TestCase[]> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests`, {
      token,
    });
  },

  create(
    token: string,
    projectId: string,
    rulesetName: string,
    tc: TestCaseInput
  ): Promise<TestCase> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests`, {
      method: 'POST',
      token,
      body: JSON.stringify(tc),
    });
  },

  update(
    token: string,
    projectId: string,
    rulesetName: string,
    id: string,
    tc: TestCaseInput
  ): Promise<TestCase> {
    return request(
      `/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/${id}`,
      { method: 'PUT', token, body: JSON.stringify(tc) }
    );
  },

  delete(token: string, projectId: string, rulesetName: string, id: string): Promise<void> {
    return request(
      `/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/${id}`,
      { method: 'DELETE', token }
    );
  },

  runAll(
    token: string,
    projectId: string,
    rulesetName: string,
    req: TestRunRequest = {}
  ): Promise<TestRunResult[]> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/run`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  runOne(
    token: string,
    projectId: string,
    rulesetName: string,
    testId: string,
    req: TestRunRequest = {}
  ): Promise<TestRunResult> {
    return request(
      `/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/tests/${testId}/run`,
      { method: 'POST', token, body: JSON.stringify(req) }
    );
  },

  runProject(
    token: string,
    projectId: string,
    req: ProjectTestRunRequest = {}
  ): Promise<ProjectTestRunResult> {
    return request(`/projects/${projectId}/tests/run`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  runAdHoc(token: string, projectId: string, req: AdHocTestRunRequest): Promise<TestRunResult> {
    return request(`/projects/${projectId}/tests/run-ad-hoc`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  /** Returns a download URL (use window.open or anchor href). */
  exportUrl(projectId: string, rulesetName: string, format: 'yaml' | 'json' = 'yaml'): string {
    return `${BASE}/projects/${projectId}/rulesets/${encodeURIComponent(
      rulesetName
    )}/tests/export?format=${format}`;
  },
};

// ── Ruleset History ──────────────────────────────────────────────────────────

export const rulesetHistoryApi = {
  list(token: string, projectId: string, rulesetName: string): Promise<RulesetHistoryResponse> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/history`, {
      method: 'GET',
      token,
    });
  },

  append(
    token: string,
    projectId: string,
    rulesetName: string,
    entries: AppendRulesetHistoryEntry[]
  ): Promise<RulesetHistoryResponse> {
    return request(`/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/history`, {
      method: 'POST',
      token,
      body: JSON.stringify({ entries }),
    });
  },
};

// ── GitHub OAuth ──────────────────────────────────────────────────────────────

export const githubApi = {
  getConnectUrl(token: string): Promise<GitHubConnectUrlResponse> {
    return request('/github/connect', { token });
  },

  getStatus(token: string): Promise<GitHubStatus> {
    return request('/github/status', { token });
  },

  disconnect(token: string): Promise<void> {
    return request('/github/disconnect', { method: 'DELETE', token });
  },
};

// ── GitHub Marketplace ────────────────────────────────────────────────────────

export const marketplaceApi = {
  search(
    token: string,
    params: { q?: string; sort?: 'stars' | 'updated'; page?: number; per_page?: number }
  ): Promise<MarketplaceSearchResponse> {
    const qs = new URLSearchParams();
    if (params.q) qs.set('q', params.q);
    if (params.sort) qs.set('sort', params.sort);
    if (params.page) qs.set('page', String(params.page));
    if (params.per_page) qs.set('per_page', String(params.per_page));
    return request(`/marketplace/search?${qs}`, { token });
  },

  getItem(token: string, owner: string, repo: string): Promise<MarketplaceDetail> {
    return request(`/marketplace/repos/${owner}/${repo}`, { token });
  },

  install(
    token: string,
    owner: string,
    repo: string,
    payload: InstallMarketplacePayload
  ): Promise<Project> {
    return request(`/marketplace/install/${owner}/${repo}`, {
      method: 'POST',
      token,
      body: JSON.stringify(payload),
    });
  },
};

// ── Servers ───────────────────────────────────────────────────────────────────

export const serverApi = {
  list(token: string): Promise<ServerInfo[]> {
    return request('/servers', { token });
  },

  get(token: string, id: string): Promise<ServerInfo> {
    return request(`/servers/${id}`, { token });
  },

  getHealth(
    token: string,
    id: string
  ): Promise<{ online: boolean; response?: string; error?: string; url: string }> {
    return request(`/servers/${id}/health`, { token });
  },

  getMetrics(token: string, id: string): Promise<string> {
    return requestText(`/servers/${id}/metrics`, { token });
  },

  delete(token: string, id: string): Promise<void> {
    return request(`/servers/${id}`, { method: 'DELETE', token });
  },
};

// ── Execution analytics ───────────────────────────────────────────────────────

export const analyticsApi = {
  get(
    token: string,
    orgId: string,
    projectId: string,
    params?: { range?: string; bucket?: number; ruleset?: string }
  ): Promise<AnalyticsResponse> {
    const qs = new URLSearchParams();
    if (params?.range) qs.set('range', params.range);
    if (params?.bucket != null) qs.set('bucket', String(params.bucket));
    if (params?.ruleset) qs.set('ruleset', params.ruleset);
    const q = qs.toString();
    return request(`/orgs/${orgId}/projects/${projectId}/analytics${q ? `?${q}` : ''}`, {
      token,
    });
  },
};

// ── Org connect tokens ────────────────────────────────────────────────────────

export const connectTokenApi = {
  list(token: string, orgId: string): Promise<ConnectTokenInfo[]> {
    return request(`/orgs/${orgId}/connect-tokens`, { token });
  },

  create(token: string, orgId: string, label?: string): Promise<CreateConnectTokenResponse> {
    return request(`/orgs/${orgId}/connect-tokens`, {
      method: 'POST',
      token,
      body: JSON.stringify({ label: label || null }),
    });
  },

  delete(token: string, orgId: string, id: string): Promise<void> {
    return request(`/orgs/${orgId}/connect-tokens/${id}`, { method: 'DELETE', token });
  },
};

// ── Environments ──────────────────────────────────────────────────────────────

export const environmentApi = {
  list(token: string, orgId: string, projectId: string): Promise<ProjectEnvironment[]> {
    return request(`/orgs/${orgId}/projects/${projectId}/environments`, { token });
  },

  create(
    token: string,
    orgId: string,
    projectId: string,
    req: CreateEnvironmentRequest
  ): Promise<ProjectEnvironment> {
    return request(`/orgs/${orgId}/projects/${projectId}/environments`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  update(
    token: string,
    orgId: string,
    projectId: string,
    envId: string,
    req: UpdateEnvironmentRequest
  ): Promise<ProjectEnvironment> {
    return request(`/orgs/${orgId}/projects/${projectId}/environments/${envId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(req),
    });
  },

  delete(token: string, orgId: string, projectId: string, envId: string): Promise<void> {
    return request(`/orgs/${orgId}/projects/${projectId}/environments/${envId}`, {
      method: 'DELETE',
      token,
    });
  },

  setCanary(
    token: string,
    orgId: string,
    projectId: string,
    envId: string,
    req: SetCanaryRequest
  ): Promise<ProjectEnvironment> {
    return request(`/orgs/${orgId}/projects/${projectId}/environments/${envId}/canary`, {
      method: 'PUT',
      token,
      body: JSON.stringify(req),
    });
  },
};

// ── Draft Rulesets ────────────────────────────────────────────────────────────

export const rulesetDraftApi = {
  list(token: string, orgId: string, projectId: string): Promise<ProjectRulesetMeta[]> {
    return request(`/orgs/${orgId}/projects/${projectId}/rulesets`, { token });
  },

  get(token: string, orgId: string, projectId: string, name: string): Promise<ProjectRuleset> {
    return request(`/orgs/${orgId}/projects/${projectId}/rulesets/${encodeURIComponent(name)}`, {
      token,
    });
  },

  convert(
    token: string,
    orgId: string,
    projectId: string,
    name: string,
    ruleset: RuleSet
  ): Promise<Record<string, unknown>> {
    return request(
      `/orgs/${orgId}/projects/${projectId}/rulesets/${encodeURIComponent(name)}/convert`,
      {
        method: 'POST',
        token,
        body: JSON.stringify({ ruleset }),
      }
    );
  },

  /** Returns the saved draft on success, or a DraftConflictResponse (status 409) on conflict. */
  async save(
    token: string,
    orgId: string,
    projectId: string,
    name: string,
    req: SaveDraftRequest
  ): Promise<ProjectRuleset | DraftConflictResponse> {
    const resp = await fetch(
      `${BASE}/orgs/${orgId}/projects/${projectId}/rulesets/${encodeURIComponent(name)}`,
      {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Accept-Language': currentLocale(),
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify(req),
      }
    );
    if (resp.status === 409) {
      return resp.json() as Promise<DraftConflictResponse>;
    }
    if (!resp.ok) {
      noteUnauthorized(resp.status, token);
      let errMsg = `HTTP ${resp.status}`;
      let errCode: string | undefined;
      try {
        const body = await resp.json();
        errMsg = body.error || errMsg;
        errCode = body.code;
      } catch {}
      const err = new Error(errMsg) as PlatformApiError;
      err.status = resp.status;
      err.code = errCode;
      throw err;
    }
    return resp.json();
  },

  delete(token: string, orgId: string, projectId: string, name: string): Promise<void> {
    return request(`/orgs/${orgId}/projects/${projectId}/rulesets/${encodeURIComponent(name)}`, {
      method: 'DELETE',
      token,
    });
  },

  publish(
    token: string,
    orgId: string,
    projectId: string,
    name: string,
    req: PublishRequest
  ): Promise<RulesetDeployment> {
    return request(
      `/orgs/${orgId}/projects/${projectId}/rulesets/${encodeURIComponent(name)}/publish`,
      { method: 'POST', token, body: JSON.stringify(req) }
    );
  },

  listDeployments(
    token: string,
    orgId: string,
    projectId: string,
    name: string,
    limit?: number
  ): Promise<RulesetDeployment[]> {
    const qs = limit ? `?limit=${limit}` : '';
    return request(
      `/orgs/${orgId}/projects/${projectId}/rulesets/${encodeURIComponent(name)}/deployments${qs}`,
      { token }
    );
  },

  listProjectDeployments(
    token: string,
    orgId: string,
    projectId: string,
    limit?: number
  ): Promise<RulesetDeployment[]> {
    const qs = limit ? `?limit=${limit}` : '';
    return request(`/orgs/${orgId}/projects/${projectId}/deployments${qs}`, { token });
  },

  redeploy(
    token: string,
    orgId: string,
    projectId: string,
    name: string,
    deploymentId: string,
    req: RedeployRequest
  ): Promise<RulesetDeployment> {
    return request(
      `/orgs/${orgId}/projects/${projectId}/rulesets/${encodeURIComponent(
        name
      )}/deployments/${deploymentId}/redeploy`,
      { method: 'POST', token, body: JSON.stringify(req) }
    );
  },
};

// ── Managed SubRule Assets ───────────────────────────────────────────────────

export const subRuleApi = {
  listProject(
    token: string,
    orgId: string,
    projectId: string,
    includeOrg = true
  ): Promise<SubRuleAssetMeta[]> {
    const qs = includeOrg ? '' : '?include_org=false';
    return request(`/orgs/${orgId}/projects/${projectId}/sub-rules${qs}`, { token });
  },

  listOrg(token: string, orgId: string): Promise<SubRuleAssetMeta[]> {
    return request(`/orgs/${orgId}/sub-rules`, { token });
  },

  getProject(token: string, orgId: string, projectId: string, name: string): Promise<SubRuleAsset> {
    return request(`/orgs/${orgId}/projects/${projectId}/sub-rules/${encodeURIComponent(name)}`, {
      token,
    });
  },

  getOrg(token: string, orgId: string, name: string): Promise<SubRuleAsset> {
    return request(`/orgs/${orgId}/sub-rules/${encodeURIComponent(name)}`, { token });
  },

  saveProject(
    token: string,
    orgId: string,
    projectId: string,
    name: string,
    req: SaveSubRuleAssetRequest
  ): Promise<SubRuleAsset> {
    return request(`/orgs/${orgId}/projects/${projectId}/sub-rules/${encodeURIComponent(name)}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(req),
    });
  },

  saveOrg(
    token: string,
    orgId: string,
    name: string,
    req: SaveSubRuleAssetRequest
  ): Promise<SubRuleAsset> {
    return request(`/orgs/${orgId}/sub-rules/${encodeURIComponent(name)}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(req),
    });
  },

  deleteProject(token: string, orgId: string, projectId: string, name: string): Promise<void> {
    return request(`/orgs/${orgId}/projects/${projectId}/sub-rules/${encodeURIComponent(name)}`, {
      method: 'DELETE',
      token,
    });
  },

  deleteOrg(token: string, orgId: string, name: string): Promise<void> {
    return request(`/orgs/${orgId}/sub-rules/${encodeURIComponent(name)}`, {
      method: 'DELETE',
      token,
    });
  },
};

// ── RBAC ──────────────────────────────────────────────────────────────────────

export const roleApi = {
  list(token: string, orgId: string): Promise<OrgRole[]> {
    return request(`/orgs/${orgId}/roles`, { token });
  },

  create(token: string, orgId: string, req: CreateRoleRequest): Promise<OrgRole> {
    return request(`/orgs/${orgId}/roles`, { method: 'POST', token, body: JSON.stringify(req) });
  },

  update(token: string, orgId: string, roleId: string, req: UpdateRoleRequest): Promise<OrgRole> {
    return request(`/orgs/${orgId}/roles/${roleId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(req),
    });
  },

  delete(token: string, orgId: string, roleId: string): Promise<void> {
    return request(`/orgs/${orgId}/roles/${roleId}`, { method: 'DELETE', token });
  },
};

export const memberRoleApi = {
  list(token: string, orgId: string, userId: string): Promise<UserRoleAssignment[]> {
    return request(`/orgs/${orgId}/members/${userId}/roles`, { token });
  },

  assign(token: string, orgId: string, userId: string, req: AssignRoleRequest): Promise<void> {
    return request(`/orgs/${orgId}/members/${userId}/roles`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  revoke(token: string, orgId: string, userId: string, roleId: string): Promise<void> {
    return request(`/orgs/${orgId}/members/${userId}/roles/${roleId}`, { method: 'DELETE', token });
  },
};

// ── Release Center ───────────────────────────────────────────────────────────

export const releaseApi = {
  listPolicies(token: string, orgId: string, projectId: string): Promise<ReleasePolicy[]> {
    return request(`/orgs/${orgId}/projects/${projectId}/release-policies`, { token });
  },

  createPolicy(
    token: string,
    orgId: string,
    projectId: string,
    req: {
      name: string;
      scope: 'org' | 'project';
      target_type: 'project' | 'environment';
      target_id: string;
      description?: string;
      min_approvals: number;
      allow_self_approval: boolean;
      approver_ids: string[];
      rollout_strategy: RolloutStrategy;
      rollback_policy: RollbackPolicy;
    }
  ): Promise<ReleasePolicy> {
    return request(`/orgs/${orgId}/projects/${projectId}/release-policies`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  updatePolicy(
    token: string,
    orgId: string,
    projectId: string,
    policyId: string,
    req: {
      name?: string;
      description?: string;
      min_approvals?: number;
      allow_self_approval?: boolean;
      approver_ids?: string[];
      rollout_strategy?: RolloutStrategy;
      rollback_policy?: RollbackPolicy;
    }
  ): Promise<ReleasePolicy> {
    return request(`/orgs/${orgId}/projects/${projectId}/release-policies/${policyId}`, {
      method: 'PUT',
      token,
      body: JSON.stringify(req),
    });
  },

  deletePolicy(token: string, orgId: string, projectId: string, policyId: string): Promise<void> {
    return request(`/orgs/${orgId}/projects/${projectId}/release-policies/${policyId}`, {
      method: 'DELETE',
      token,
    });
  },

  listRequests(token: string, orgId: string, projectId: string): Promise<ReleaseRequest[]> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases`, { token });
  },

  previewTarget(
    token: string,
    orgId: string,
    projectId: string,
    environmentId: string
  ): Promise<ReleaseTargetPreview> {
    const qs = new URLSearchParams({ environment_id: environmentId });
    return request(`/orgs/${orgId}/projects/${projectId}/releases/preview?${qs.toString()}`, {
      token,
    });
  },

  createRequest(
    token: string,
    orgId: string,
    projectId: string,
    req: {
      ruleset_name: string;
      version: string;
      environment_id: string;
      policy_id?: string;
      title: string;
      change_summary: string;
      release_note?: string;
      rollback_version?: string;
      affected_instance_count?: number;
    }
  ): Promise<ReleaseRequest> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  getRequest(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string
  ): Promise<ReleaseRequest> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}`, { token });
  },

  getRequestHistory(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string
  ): Promise<ReleaseRequestHistoryEntry[]> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/history`, {
      token,
    });
  },

  executeRequest(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string
  ): Promise<ReleaseExecution> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/execute`, {
      method: 'POST',
      token,
    });
  },

  pauseExecution(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string
  ): Promise<ReleaseExecution> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/pause`, {
      method: 'POST',
      token,
    });
  },

  resumeExecution(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string
  ): Promise<ReleaseExecution> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/resume`, {
      method: 'POST',
      token,
    });
  },

  rollbackExecution(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string
  ): Promise<ReleaseExecution> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/rollback`, {
      method: 'POST',
      token,
    });
  },

  getRequestExecution(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string
  ): Promise<ReleaseExecution | null> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/execution`, {
      token,
    });
  },

  getCurrentExecution(
    token: string,
    orgId: string,
    projectId: string
  ): Promise<ReleaseExecution | null> {
    return request(`/orgs/${orgId}/projects/${projectId}/release-executions/current`, { token });
  },

  approveRequest(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string,
    req: ReviewReleaseRequest
  ): Promise<ReleaseRequest> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/approve`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  rejectRequest(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string,
    req: ReviewReleaseRequest
  ): Promise<ReleaseRequest> {
    return request(`/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/reject`, {
      method: 'POST',
      token,
      body: JSON.stringify(req),
    });
  },

  getExecutionEvents(
    token: string,
    orgId: string,
    projectId: string,
    releaseId: string,
    executionId: string
  ): Promise<ReleaseExecutionEvent[]> {
    return request(
      `/orgs/${orgId}/projects/${projectId}/releases/${releaseId}/executions/${executionId}/events`,
      { token }
    );
  },

  listPendingForMe(token: string, orgId: string): Promise<ReleaseRequest[]> {
    return request(`/orgs/${orgId}/releases/pending-for-me`, { token });
  },
};

export const engineApi = {
  executeWithTrace(
    token: string,
    orgId: string,
    projectId: string,
    rulesetName: string,
    input: Record<string, unknown>,
    ruleset: RuleSet
  ): Promise<{
    code: string;
    message: string;
    output: Record<string, unknown>;
    duration_us: number;
    trace?: {
      path: string;
      steps: Array<{ id: string; name: string; duration_us: number }>;
    };
  }> {
    return request(
      `/orgs/${orgId}/projects/${projectId}/rulesets/${encodeURIComponent(rulesetName)}/trace`,
      {
        method: 'POST',
        token,
        body: JSON.stringify({ ruleset, input }),
      }
    );
  },
};

export const notificationApi = {
  list(
    token: string,
    orgId: string,
    opts: { unread_only?: boolean; limit?: number; offset?: number } = {}
  ): Promise<PlatformNotification[]> {
    const params = new URLSearchParams();
    if (opts.unread_only) params.set('unread_only', 'true');
    if (opts.limit) params.set('limit', String(opts.limit));
    if (opts.offset) params.set('offset', String(opts.offset));
    const qs = params.toString();
    return request(`/orgs/${orgId}/notifications${qs ? `?${qs}` : ''}`, { token });
  },

  count(token: string, orgId: string): Promise<NotificationCount> {
    return request(`/orgs/${orgId}/notifications/count`, { token });
  },

  markRead(token: string, orgId: string, notifId: string): Promise<void> {
    return request(`/orgs/${orgId}/notifications/${notifId}/read`, { method: 'POST', token });
  },

  markAllRead(token: string, orgId: string): Promise<void> {
    return request(`/orgs/${orgId}/notifications/read-all`, { method: 'POST', token });
  },
};
