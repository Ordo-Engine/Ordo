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

// ── Error ─────────────────────────────────────────────────────────────────────

export interface ApiError {
  error: string
  status: number
}
