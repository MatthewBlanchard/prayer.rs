// Shared event types between server and client

export type ToolCallOutcome = "ok" | "error";

export type ToolLoopEvent =
  | { type: "turn_started" }
  | {
      type: "assistant_draft";
      content: string | null;
      finishReason: string;
      toolCallCount: number;
    }
  | { type: "tool_call_started"; toolCallId: string; name: string; argsPreview: string }
  | {
      type: "tool_call_completed";
      toolCallId: string;
      name: string;
      outcome: ToolCallOutcome;
      resultPreview: string;
    }
  | { type: "turn_completed"; finalContent: string | null }
  | { type: "error"; message: string };

// Chat transcript items for rendering
export type TranscriptItem =
  | { kind: "user"; content: string }
  | { kind: "assistant"; content: string }
  | { kind: "tool_card"; toolCallId: string; name: string; status: "ok" | "error" | "running"; argsPreview: string; resultPreview: string | null }
  | { kind: "error"; message: string };
