// Shared event types between server and client

export type ToolCallOutcome = "ok" | "error";

export type ToolLoopEvent =
  | { type: "turn_started"; source?: string }
  | {
      type: "assistant_draft";
      content: string | null;
      finishReason: string;
      toolCallCount: number;
      source?: string;
    }
  | { type: "tool_call_started"; toolCallId: string; name: string; argsPreview: string; source?: string }
  | {
      type: "tool_call_completed";
      toolCallId: string;
      name: string;
      outcome: ToolCallOutcome;
      resultPreview: string;
      source?: string;
    }
  | { type: "turn_completed"; finalContent: string | null; source?: string }
  | { type: "error"; message: string; source?: string };

// Agent panel types
export type AgentInfo = {
  sessionHandle: string;
  paused: boolean;
};

export type AgentFeedItem =
  | { kind: "tool_call"; toolCallId: string; name: string; status: "running" | "ok" | "error"; resultPreview: string | null }
  | { kind: "error"; message: string };

export type AgentMindMessage = {
  role: string;
  content: string | null;
  toolCallId?: string;
  isError?: boolean;
  toolCalls?: Array<{ id: string; name: string; arguments: string }>;
};

export type AgentMindSnapshot = {
  objective: string;
  compactionSummary: string | null;
  messages: AgentMindMessage[];
};

// Chat transcript items for rendering
export type TranscriptItem =
  | { kind: "user"; content: string }
  | { kind: "assistant"; content: string }
  | { kind: "tool_card"; toolCallId: string; name: string; status: "ok" | "error" | "running"; argsPreview: string; resultPreview: string | null }
  | { kind: "error"; message: string };
