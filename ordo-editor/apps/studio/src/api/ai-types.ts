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
}

export interface AiChatResponse {
  content: string;
  tool_calls: AiToolCall[];
  stop_reason: 'tool_use' | 'end_turn';
}

export interface AiModelOption {
  id: string;
  label: string;
}

export interface AiProviderOption {
  id: string;
  label: string;
  models: AiModelOption[];
}
