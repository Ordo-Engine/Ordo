/**
 * Debug component types
 */

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

export interface ServerInfo {
  status: string;
  version: string;
  uptime_seconds: number;
  debug_mode: boolean;
  storage: {
    mode: string;
    rules_count: number;
  };
}

export interface TraceLevel {
  none: 'none';
  minimal: 'minimal';
  standard: 'standard';
  full: 'full';
}

export interface RegisterValue {
  index: number;
  value: unknown;
  type_name: string;
}

export interface VMSnapshot {
  ip: number;
  instruction: string;
  registers: RegisterValue[];
  duration_ns: number;
}

export interface VMTrace {
  instructions: string[];
  constants: string[];
  fields: string[];
  functions: string[];
  snapshots: VMSnapshot[];
  total_instructions: number;
  total_duration_ns: number;
}

export interface ASTNode {
  node_type: string;
  label: string;
  children: ASTNode[];
  value?: string;
}

export interface BytecodeInfo {
  instruction_count: number;
  constant_count: number;
  field_count: number;
  function_count: number;
  instructions: string[];
}

export interface EvalStep {
  step: number;
  description: string;
  result: unknown;
}

export interface DebugEvalResponse {
  result: unknown;
  ast: ASTNode;
  bytecode?: BytecodeInfo;
  eval_steps: EvalStep[];
  parse_duration_ns: number;
  compile_duration_ns?: number;
  eval_duration_ns: number;
}

export interface DebugSessionInfo {
  id: string;
  ruleset_name: string;
  state: SessionState;
  created_at: string;
  breakpoint_count: number;
}

export type SessionState = 'created' | 'running' | 'paused' | 'completed' | 'terminated';

export interface DebugEvent {
  type: string;
  [key: string]: unknown;
}
