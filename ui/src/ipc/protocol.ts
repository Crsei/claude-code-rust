export type FrontendContentBlock =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: any }
  | { type: 'tool_result'; tool_use_id: string; content: ToolResultContent; is_error?: boolean }
  | { type: 'thinking'; thinking: string; signature?: string | null }
  | { type: 'redacted_thinking'; data: string }
  | { type: 'image'; source: any }

export type ToolResultContent = string | FrontendContentBlock[]

// Frontend -> Backend
export type ConversationMessage = {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: number
  content_blocks?: FrontendContentBlock[]
  cost_usd?: number
  thinking?: string
  level?: string
}

export type FrontendMessage =
  | { type: 'submit_prompt'; text: string; id: string }
  | { type: 'abort_query' }
  | { type: 'permission_response'; tool_use_id: string; decision: string }
  | { type: 'slash_command'; raw: string }
  | { type: 'resize'; cols: number; rows: number }
  | { type: 'quit' }

// Backend -> Frontend
export type BackendMessage =
  | { type: 'ready'; session_id: string; model: string; cwd: string }
  | { type: 'stream_start'; message_id: string }
  | { type: 'stream_delta'; message_id: string; text: string }
  | { type: 'thinking_delta'; message_id: string; thinking: string }
  | { type: 'stream_end'; message_id: string }
  | { type: 'assistant_message'; id: string; content: FrontendContentBlock[] | null; cost_usd: number }
  | { type: 'tool_use'; id: string; name: string; input: any }
  | { type: 'tool_result'; tool_use_id: string; output: string; is_error: boolean }
  | { type: 'permission_request'; tool_use_id: string; tool: string; command: string; options: string[] }
  | { type: 'system_info'; text: string; level: string }
  | { type: 'conversation_replaced'; messages: ConversationMessage[] }
  | { type: 'usage_update'; input_tokens: number; output_tokens: number; cost_usd: number }
  | { type: 'suggestions'; items: string[] }
  | { type: 'background_agent_complete'; agent_id: string; description: string; result_preview: string; had_error: boolean; duration_ms: number }
  | { type: 'brief_message'; message: string; status: string; attachments: string[] }
  | { type: 'autonomous_start'; source: string; time: string }
  | { type: 'notification_sent'; title: string; level: string }
  | { type: 'error'; message: string; recoverable: boolean }
