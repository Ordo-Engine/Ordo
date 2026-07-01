/**
 * Types for the AI rule assistant (Studio sidebar ↔ /api/v1/ai/* proxy).
 */

export interface AiToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface AiToolResult {
  tool_call_id: string;
  content: string;
  is_error?: boolean;
}

export interface AiChatMessage {
  role: 'user' | 'assistant' | 'tool';
  content?: string;
  tool_calls?: AiToolCall[];
  tool_results?: AiToolResult[];
}

export interface AiChatRequest {
  provider: string;
  model: string;
  messages: AiChatMessage[];
  /** Live editor context folded into the system prompt server-side. */
  context?: Record<string, unknown>;
  /** "agent" (default, full tools) | "ask" (read-only). */
  mode?: string;
}

/** One normalized SSE event from /ai/chat (streaming). */
export type AiStreamEvent =
  | { type: 'text'; text: string }
  | { type: 'tool_start'; id: string; name: string }
  | { type: 'tool'; id: string; name: string; input: Record<string, unknown> }
  | { type: 'done'; stop_reason: 'tool_use' | 'end_turn' }
  | { type: 'error'; message: string };

export interface AiModelOption {
  id: string;
  label: string;
}

export interface AiProviderOption {
  id: string;
  label: string;
  models: AiModelOption[];
}
