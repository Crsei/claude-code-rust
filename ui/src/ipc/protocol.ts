// Frontend → Backend
export type FrontendMessage =
  | { type: 'submit_prompt'; text: string; id: string }
  | { type: 'abort_query' }
  | { type: 'permission_response'; tool_use_id: string; decision: string }
  | { type: 'slash_command'; raw: string }
  | { type: 'resize'; cols: number; rows: number }
  | { type: 'quit' }

// Backend → Frontend
export type BackendMessage =
  | { type: 'ready'; session_id: string; model: string; cwd: string }
  | { type: 'stream_start'; message_id: string }
  | { type: 'stream_delta'; message_id: string; text: string }
  | { type: 'stream_end'; message_id: string }
  | { type: 'assistant_message'; id: string; content: any; cost_usd: number }
  | { type: 'tool_use'; id: string; name: string; input: any }
  | { type: 'tool_result'; tool_use_id: string; output: string; is_error: boolean }
  | { type: 'permission_request'; tool_use_id: string; tool: string; command: string; options: string[] }
  | { type: 'system_info'; text: string; level: string }
  | { type: 'usage_update'; input_tokens: number; output_tokens: number; cost_usd: number }
  | { type: 'suggestions'; items: string[] }
  | { type: 'error'; message: string; recoverable: boolean }
