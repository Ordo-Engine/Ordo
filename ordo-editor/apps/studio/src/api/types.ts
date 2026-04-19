/**
 * Shared TypeScript types mirroring ordo-platform and ordo-server responses
 */

import type { RuleSet } from '@ordo-engine/editor-core'

// ── Auth ──────────────────────────────────────────────────────────────────────

export type Role = 'owner' | 'admin' | 'editor' | 'viewer'

export interface UserInfo {
  id: string
  email: string
  display_name: string
  created_at: string
  last_login: string | null
}

export interface AuthResponse {
  token: string
  user: UserInfo
}

// ── Organization ──────────────────────────────────────────────────────────────

export interface OrgResponse {
  id: string
  name: string
  description: string | null
  created_at: string
  created_by: string
  member_count: number
}

export interface Organization {
  id: string
  name: string
  description: string | null
  created_at: string
  created_by: string
  members: Member[]
}

export interface Member {
  user_id: string
  email: string
  display_name: string
  role: Role
  invited_at: string
}

// ── Project ───────────────────────────────────────────────────────────────────

export interface Project {
  /** Same as ordo-server tenant_id */
  id: string
  name: string
  description: string | null
  org_id: string
  created_at: string
  created_by: string
  server_id: string | null
}

// ── Server Registry ───────────────────────────────────────────────────────────

export type ServerStatus = 'online' | 'offline' | 'degraded'

export interface ServerInfo {
  id: string
  name: string
  url: string
  org_id: string | null
  labels: Record<string, string>
  version: string | null
  status: ServerStatus
  last_seen: string | null
  registered_at: string
}

export interface BindServerRequest {
  server_id: string | null
}

// ── Ruleset Change History (ordo-platform) ──────────────────────────────────

export type RulesetHistorySource = 'sync' | 'edit' | 'save' | 'restore' | 'create'

export interface RulesetHistoryEntry {
  id: string
  ruleset_name: string
  action: string
  source: RulesetHistorySource
  created_at: string
  author_id: string
  author_email: string
  author_display_name: string
  snapshot: RuleSet
}

export interface RulesetHistoryResponse {
  ruleset_name: string
  entries: RulesetHistoryEntry[]
}

export interface AppendRulesetHistoryEntry {
  id: string
  action: string
  source: RulesetHistorySource
  created_at?: string
  snapshot: RuleSet
}

// ── Engine (ordo-server) ──────────────────────────────────────────────────────

export interface RuleSetInfo {
  name: string
  version: string
  description: string
}

export interface VersionInfo {
  seq: number
  version: string
  created_at: string
}

export interface VersionListResponse {
  name: string
  current_version: string
  versions: VersionInfo[]
}

export interface ExecuteRequest {
  input: Record<string, unknown>
  options?: {
    include_trace?: boolean
    timeout_ms?: number
  }
}

export interface ExecuteResponse {
  result: {
    code: string
    message: string
    output: Record<string, unknown>
  }
  duration_us: number
  trace?: unknown
}

// ── Fact Catalog (ordo-book Ch7) ──────────────────────────────────────────────

export type FactDataType = 'string' | 'number' | 'boolean' | 'date' | 'object'
export type NullPolicy = 'error' | 'default' | 'skip'

export interface FactDefinition {
  name: string
  data_type: FactDataType
  source: string
  latency_ms?: number
  null_policy: NullPolicy
  description?: string
  owner?: string
  created_at: string
  updated_at: string
}

// ── Concept Registry (ordo-book Ch7) ─────────────────────────────────────────

export interface ConceptDefinition {
  name: string
  data_type: FactDataType
  expression: string
  dependencies: string[]
  description?: string
  created_at: string
  updated_at: string
}

// ── Decision Contract (ordo-book Ch13) ───────────────────────────────────────

export interface ContractField {
  name: string
  data_type: FactDataType
  required: boolean
  description?: string
}

export interface DecisionContract {
  ruleset_name: string
  version_pattern: string
  owner: string
  sla_p99_ms?: number
  input_fields: ContractField[]
  output_fields: ContractField[]
  notes?: string
  updated_at: string
}

// ── Rule Templates (M1.1) ────────────────────────────────────────────────────

export type TemplateDifficulty = 'beginner' | 'intermediate' | 'advanced'

export interface TemplateMetadata {
  id: string
  name: string
  description: string
  tags: string[]
  icon?: string
  difficulty: TemplateDifficulty
  features: string[]
}

export interface TemplateSample {
  label: string
  input: Record<string, unknown>
  expected_result?: string
}

export interface TemplateDetail extends TemplateMetadata {
  facts: FactDefinition[]
  concepts: ConceptDefinition[]
  ruleset: RuleSet
  samples: TemplateSample[]
  contract?: DecisionContract
  tests: TestCase[]
}

export interface CreateFromTemplatePayload {
  template_id: string
  project_name: string
  project_description?: string
}

// ── Test Cases (M1.2) ────────────────────────────────────────────────────────

export interface TestExpectation {
  code?: string
  message?: string
  output?: Record<string, unknown>
}

export interface TestCase {
  id: string
  name: string
  description?: string
  input: Record<string, unknown>
  expect: TestExpectation
  tags: string[]
  created_at: string
  updated_at: string
  created_by: string
}

export interface TestCaseInput {
  name: string
  description?: string
  input: Record<string, unknown>
  expect: TestExpectation
  tags?: string[]
}

export interface TestRunResult {
  test_id: string
  test_name: string
  passed: boolean
  failures: string[]
  duration_us: number
  actual_code?: string
  actual_output?: Record<string, unknown>
}

export interface RulesetTestSummary {
  ruleset_name: string
  total: number
  passed: number
  failed: number
  results: TestRunResult[]
}

export interface ProjectTestRunResult {
  total: number
  passed: number
  failed: number
  rulesets: RulesetTestSummary[]
}

// ── GitHub Integration ────────────────────────────────────────────────────────

export interface GitHubConnectUrlResponse {
  url: string
}

export interface GitHubStatus {
  connected: boolean
  login?: string
  name?: string
  avatar_url?: string
  connected_at?: string
}

// ── GitHub Marketplace ────────────────────────────────────────────────────────

export interface MarketplaceItem {
  id: string
  name: string
  full_name: string
  description: string | null
  html_url: string
  stars: number
  topics: string[]
  updated_at: string
  owner_login: string
  owner_avatar: string
  icon?: string
  difficulty?: TemplateDifficulty
  tags: string[]
  features: string[]
}

export interface MarketplaceSearchResponse {
  items: MarketplaceItem[]
  total_count: number
  page: number
  per_page: number
}

/** Full detail: MarketplaceItem fields + TemplateDetail fields + GitHub enrichments */
export interface MarketplaceDetail extends TemplateDetail {
  github_url?: string
  stars?: number
  owner_login?: string
  owner_avatar?: string
  full_name?: string
  updated_at?: string
  topics?: string[]
}

export interface InstallMarketplacePayload {
  org_id: string
  project_name: string
  project_description?: string
}

// ── Environments ─────────────────────────────────────────────────────────────

export interface ProjectEnvironment {
  id: string
  project_id: string
  name: string
  server_id: string | null
  nats_subject_prefix: string | null
  is_default: boolean
  canary_target_env_id: string | null
  canary_percentage: number
  created_at: string
}

export interface CreateEnvironmentRequest {
  name: string
  server_id?: string | null
  nats_subject_prefix?: string | null
}

export interface UpdateEnvironmentRequest {
  name?: string
  server_id?: string | null
  nats_subject_prefix?: string | null
}

export interface SetCanaryRequest {
  canary_target_env_id: string | null
  canary_percentage: number
}

// ── Draft Rulesets ────────────────────────────────────────────────────────────

export interface ProjectRulesetMeta {
  id: string
  project_id: string
  name: string
  draft_seq: number
  draft_updated_at: string
  draft_updated_by: string | null
  published_version: string | null
  published_at: string | null
  created_at: string
}

export interface ProjectRuleset extends ProjectRulesetMeta {
  draft: RuleSet
}

export interface SaveDraftRequest {
  ruleset: RuleSet
  expected_seq: number
}

export interface DraftConflictResponse {
  conflict: true
  server_draft: RuleSet
  server_seq: number
}

// ── Deployments ───────────────────────────────────────────────────────────────

export type DeploymentStatus = 'queued' | 'success' | 'failed'

export interface RulesetDeployment {
  id: string
  project_id: string
  environment_id: string
  environment_name: string | null
  ruleset_name: string
  version: string
  release_note: string | null
  snapshot: RuleSet
  deployed_at: string
  deployed_by: string | null
  status: DeploymentStatus
}

export interface PublishRequest {
  environment_id: string
  release_note?: string
}

export interface RedeployRequest {
  environment_id: string
  release_note?: string
}

// ── Release Center ────────────────────────────────────────────────────────────

export type ReleaseRequestStatus =
  | 'draft'
  | 'pending_approval'
  | 'approved'
  | 'rejected'
  | 'cancelled'
  | 'executing'
  | 'completed'
  | 'failed'
  | 'rolled_back'

export type ReleaseApprovalDecision = 'pending' | 'approved' | 'rejected'

export type RolloutStrategyKind =
  | 'all_at_once'
  | 'fixed_batch'
  | 'percentage_batch'
  | 'time_interval_batch'

export interface RolloutStrategy {
  kind: RolloutStrategyKind
  batch_size?: number
  batch_percentage?: number
  batch_interval_seconds?: number
  auto_rollback_on_failure?: boolean
  pause_on_error_rate?: number
  pause_on_metric_breach?: boolean
}

export interface RollbackPolicy {
  auto_rollback: boolean
  max_failed_instances: number
  metric_guard?: string
}

export interface ReleaseApprovalRecord {
  id: string
  stage: number
  reviewer_id: string
  reviewer_name: string
  reviewer_email?: string | null
  decision: ReleaseApprovalDecision
  comment?: string
  decided_at?: string
}

export interface ReleaseVersionDiff {
  from_version?: string | null
  to_version: string
  rollback_version?: string | null
  changed: boolean
}

export interface ReleaseStepDiffItem {
  id: string
  name: string
  step_type?: string | null
}

export interface ReleaseContentDiffSummary {
  baseline_version?: string | null
  step_count_before: number
  step_count_after: number
  group_count_before: number
  group_count_after: number
  added_steps: ReleaseStepDiffItem[]
  removed_steps: ReleaseStepDiffItem[]
  modified_steps: ReleaseStepDiffItem[]
  added_groups: string[]
  removed_groups: string[]
  modified_groups: string[]
  input_schema_changed: boolean
  output_schema_changed: boolean
  tags_changed: boolean
  description_changed: boolean
}

export interface ReleaseRequestSnapshot {
  requester_id: string
  requester_name?: string | null
  requester_email?: string | null
  policy_name?: string | null
  policy_scope?: 'org' | 'project' | null
  target_type?: 'environment' | 'project' | null
  target_id?: string | null
  environment_name?: string | null
  approver_ids: string[]
  approver_names: string[]
  approver_emails: string[]
  min_approvals?: number | null
  allow_self_approval?: boolean | null
  rollout_strategy: RolloutStrategy
  rollback_policy: RollbackPolicy
  affected_instance_count: number
}

export interface ReleaseRequest {
  id: string
  org_id?: string
  project_id?: string
  title: string
  ruleset_name: string
  version: string
  environment_id: string
  environment_name: string | null
  change_summary: string
  release_note?: string | null
  status: ReleaseRequestStatus
  created_by: string
  created_by_name?: string | null
  created_by_email?: string | null
  created_at: string
  updated_at?: string
  approvals: ReleaseApprovalRecord[]
  affected_instance_count: number
  policy_id?: string | null
  rollout_strategy: RolloutStrategy
  rollback_version?: string | null
  version_diff: ReleaseVersionDiff
  content_diff: ReleaseContentDiffSummary
  request_snapshot: ReleaseRequestSnapshot
}

export interface ReleasePolicy {
  id: string
  org_id?: string
  project_id?: string | null
  name: string
  scope: 'org' | 'project'
  target_type: 'environment' | 'project'
  target_id: string
  description?: string
  min_approvals: number
  allow_self_approval: boolean
  approver_ids: string[]
  rollout_strategy: RolloutStrategy
  rollback_policy: RollbackPolicy
  created_at?: string
  updated_at: string
}

export interface ReviewReleaseRequest {
  comment?: string
}

export type ReleaseExecutionStatus =
  | 'preparing'
  | 'waiting_start'
  | 'rolling_out'
  | 'paused'
  | 'rollback_in_progress'
  | 'completed'
  | 'failed'

export type ReleaseInstanceStatus =
  | 'pending'
  | 'dispatching'
  | 'updating'
  | 'verifying'
  | 'success'
  | 'failed'
  | 'rolled_back'

export interface ReleaseExecutionInstance {
  id: string
  instance_name: string
  zone?: string
  current_version: string
  target_version: string
  status: ReleaseInstanceStatus
  updated_at?: string
  message?: string
  metric_summary?: string
}

export interface ReleaseExecution {
  id: string
  request_id: string
  status: ReleaseExecutionStatus
  started_at: string
  current_batch: number
  total_batches: number
  strategy: RolloutStrategy
  summary: {
    total_instances: number
    succeeded_instances: number
    failed_instances: number
    pending_instances: number
  }
  instances: ReleaseExecutionInstance[]
}

// ── RBAC ──────────────────────────────────────────────────────────────────────

export interface OrgRole {
  id: string
  org_id: string
  name: string
  description: string | null
  permissions: string[]
  is_system: boolean
  created_at: string
}

export interface CreateRoleRequest {
  name: string
  description?: string
  permissions: string[]
}

export interface UpdateRoleRequest {
  name?: string
  description?: string
  permissions?: string[]
}

export interface UserRoleAssignment {
  user_id: string
  org_id: string
  role_id: string
  role_name: string
  assigned_at: string
  assigned_by: string | null
}

export interface AssignRoleRequest {
  role_id: string
}

// ── All permission constants ───────────────────────────────────────────────────

export const PERMISSIONS = [
  'org:view', 'org:manage',
  'member:view', 'member:invite', 'member:remove',
  'role:view', 'role:manage',
  'project:view', 'project:create', 'project:manage', 'project:delete',
  'ruleset:view', 'ruleset:edit', 'ruleset:publish',
  'environment:view', 'environment:manage',
  'server:view', 'server:manage',
  'test:run',
  'deployment:view', 'deployment:redeploy',
  'canary:manage',
  'release:policy.manage',
  'release:request.create',
  'release:request.view',
  'release:request.approve',
  'release:request.reject',
  'release:execute',
  'release:pause',
  'release:resume',
  'release:rollback',
  'release:instance.view',
] as const

export type Permission = typeof PERMISSIONS[number]

// ── Error ─────────────────────────────────────────────────────────────────────

export interface ApiError {
  error: string
  status: number
}
