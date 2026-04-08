import {
  CompletionError,
  CompletionProvider,
  CompletionRequest,
  Message,
  ToolDef,
} from "./llm.js";
import { extractUriArgument, McpClient, McpTool, parseJsonObject } from "./mcp.js";

const READ_RESOURCE_TOOL_NAME = "read_resource";
const LIST_SESSIONS_TOOL_NAME = "list_sessions";
const TOOL_EVENT_PREVIEW_CHARS = 240;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export interface CompactionConfig {
  enabled: boolean;
  estimatedContextWindow: number;
  contextBudgetRatio: number;
  minRecentMessages: number;
  charsPerToken: number;
  summaryMaxCompletionTokens: number;
}

export const defaultCompactionConfig: CompactionConfig = {
  enabled: true,
  estimatedContextWindow: 128_000,
  contextBudgetRatio: 0.55,
  minRecentMessages: 10,
  charsPerToken: 4,
  summaryMaxCompletionTokens: 1024,
};

export interface LoopConfig {
  maxToolRounds: number;
  maxRetries: number;
  retryBaseDelayMs: number;
  llmTimeoutMs: number;
  includeSyntheticReadResource: boolean;
  maxCompletionTokens: number;
  temperature: number;
  compaction: CompactionConfig;
}

export const defaultLoopConfig: LoopConfig = {
  maxToolRounds: 30,
  maxRetries: 3,
  retryBaseDelayMs: 5_000,
  llmTimeoutMs: 120_000,
  includeSyntheticReadResource: true,
  maxCompletionTokens: 4096,
  temperature: 0.7,
  compaction: defaultCompactionConfig,
};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Compaction state
// ---------------------------------------------------------------------------

export interface CompactionState {
  summary: string | undefined;
}

export function newCompactionState(): CompactionState {
  return { summary: undefined };
}

// ---------------------------------------------------------------------------
// ChatToolLoop
// ---------------------------------------------------------------------------

export class ChatToolLoop {
  private toolDefs: ToolDef[] | undefined;
  private bootstrapReference: string | undefined;
  private bootstrapSessionSnapshot: string | undefined;

  constructor(
    private readonly provider: CompletionProvider,
    private readonly mcp: McpClient,
    private readonly model: string | undefined,
    private readonly config: LoopConfig
  ) {}

  getModel(): string {
    return this.model ?? "";
  }

  getBootstrapReference(): string | undefined {
    return this.bootstrapReference;
  }

  getBootstrapSessionSnapshot(): string | undefined {
    return this.bootstrapSessionSnapshot;
  }

  async connect(): Promise<void> {
    const tools = await this.mcp.listTools();
    this.toolDefs = buildToolDefs(tools, this.config.includeSyntheticReadResource);

    try {
      this.bootstrapReference = await this.mcp.readResource("prayer://dsl/reference");
    } catch {
      // optional
    }

    const hasListSessions = tools.some((t) => t.name === LIST_SESSIONS_TOOL_NAME);
    if (hasListSessions) {
      try {
        const sessions = await this.mcp.callTool(LIST_SESSIONS_TOOL_NAME, {});
        if (!sessions.isError && sessions.text.trim()) {
          this.bootstrapSessionSnapshot = sessions.text;
        }
      } catch {
        // optional
      }
    }
  }

  async close(): Promise<void> {
    await this.mcp.close();
  }

  async runTurn(
    messages: Message[],
    compaction: CompactionState,
    onEvent: (event: ToolLoopEvent) => void
  ): Promise<string | null> {
    if (!this.toolDefs) throw new Error("Not connected; call connect() first");

    const toolDefs = this.toolDefs;
    let finalContent: string | null = null;
    onEvent({ type: "turn_started" });

    for (let round = 0; round < this.config.maxToolRounds; round++) {
      await this.compactIfNeeded(messages, compaction);

      const completion = await this.completeWithRetry(messages, toolDefs);
      const assistantMessage = completion.message;
      const finishReason = completion.finishReason;

      const contentStr = assistantMessage["content"] as string | null | undefined;
      if (contentStr?.trim()) {
        finalContent = contentStr;
      }

      messages.push(assistantMessage);

      const toolCalls =
        (assistantMessage["tool_calls"] as Array<Record<string, unknown>> | undefined) ?? [];

      onEvent({
        type: "assistant_draft",
        content: (contentStr as string | null) ?? null,
        finishReason,
        toolCallCount: toolCalls.length,
      });

      if (toolCalls.length === 0 || finishReason === "stop") {
        onEvent({ type: "turn_completed", finalContent });
        return finalContent;
      }

      for (let idx = 0; idx < toolCalls.length; idx++) {
        const toolCall = toolCalls[idx];
        const toolCallId =
          (toolCall["id"] as string | undefined) ??
          `tool_call_${round}_${idx}`;

        const fn = toolCall["function"] as Record<string, unknown> | undefined;
        const toolName = (fn?.["name"] as string | undefined) ?? "";
        const argsJson = (fn?.["arguments"] as string | undefined) ?? "{}";

        onEvent({
          type: "tool_call_started",
          toolCallId,
          name: toolName,
          argsPreview: previewText(argsJson, TOOL_EVENT_PREVIEW_CHARS),
        });

        let toolResult: string;
        let outcome: ToolCallOutcome;
        try {
          const result = await this.executeTool(toolName, argsJson);
          toolResult = result.text;
          outcome = result.isError ? "error" : "ok";
        } catch (err) {
          toolResult = err instanceof Error ? err.message : String(err);
          outcome = "error";
        }

        onEvent({
          type: "tool_call_completed",
          toolCallId,
          name: toolName,
          outcome,
          resultPreview: toolResult,
        });

        messages.push({
          role: "tool",
          tool_call_id: toolCallId,
          content: toolResult,
          ...(outcome === "error" && { isError: true }),
        });
      }
    }

    onEvent({ type: "turn_completed", finalContent });
    return finalContent;
  }

  private async executeTool(toolName: string, argsJson: string): Promise<{ text: string; isError: boolean }> {
    if (toolName === READ_RESOURCE_TOOL_NAME && this.config.includeSyntheticReadResource) {
      const uri = extractUriArgument(argsJson) ?? "";
      if (!uri) return { text: "Error: uri argument is required", isError: true };
      const text = await this.mcp.readResource(uri);
      return { text, isError: false };
    }

    const args = parseJsonObject(argsJson) ?? {};
    return this.mcp.callTool(toolName, args);
  }

  private async completeWithRetry(
    messages: Message[],
    tools: ToolDef[]
  ): Promise<{ message: Message; finishReason: string }> {
    let lastError: Error | undefined;

    for (let attempt = 0; attempt < this.config.maxRetries; attempt++) {
      const request: CompletionRequest = {
        model: this.model,
        messages: [...messages],
        tools,
        toolChoice: "auto",
        maxCompletionTokens: this.config.maxCompletionTokens,
        temperature: this.config.temperature,
      };

      try {
        return await this.provider.complete(request);
      } catch (err) {
        lastError = err instanceof Error ? err : new Error(String(err));
        if (attempt + 1 < this.config.maxRetries) {
          const delayMs = this.config.retryBaseDelayMs * Math.pow(2, attempt);
          await sleep(delayMs);
        }
      }
    }

    throw lastError ?? new CompletionError("completion failed with no attempts");
  }

  private async compactIfNeeded(
    messages: Message[],
    compaction: CompactionState
  ): Promise<void> {
    if (!this.config.compaction.enabled) return;
    if (messages.length < 3) return;

    const charsPerToken = Math.max(1, this.config.compaction.charsPerToken);
    const budgetTokens = Math.floor(
      this.config.compaction.estimatedContextWindow *
        this.config.compaction.contextBudgetRatio
    );

    const currentTokens = estimateTokens(messages, charsPerToken);
    if (currentTokens < budgetTokens) return;

    const recentBudget = Math.floor(budgetTokens * 0.6);
    let recentTokens = 0;
    let splitIdx = messages.length;

    for (let i = messages.length - 1; i >= 1; i--) {
      const msgTokens = estimateTokensForValue(messages[i], charsPerToken);
      if (
        recentTokens + msgTokens > recentBudget &&
        splitIdx <
          messages.length - this.config.compaction.minRecentMessages
      ) {
        break;
      }
      recentTokens += msgTokens;
      splitIdx = i;
    }

    // Align to a user message boundary
    while (splitIdx > 1) {
      const role = messages[splitIdx]?.["role"] as string | undefined;
      if (role === "user") break;
      splitIdx++;
      if (splitIdx >= messages.length) break;
    }

    if (splitIdx <= 1 || splitIdx >= messages.length) return;

    const oldMessages = messages.slice(1, splitIdx);
    const recentMessages = messages.slice(splitIdx);

    const summary = await this.summarizeMessages(oldMessages, compaction.summary);
    compaction.summary = summary;

    const system = messages[0] ?? { role: "system", content: "" };
    messages.length = 0;
    messages.push(system);
    messages.push({
      role: "user",
      content: `## Conversation History Summary\n\n${summary}\n\n---\nContinue from here. Recent messages follow.`,
    });
    messages.push(...recentMessages);
  }

  private async summarizeMessages(
    oldMessages: Message[],
    previousSummary: string | undefined
  ): Promise<string> {
    const transcript = formatTranscript(oldMessages);

    let prompt =
      "Summarize this conversation transcript concisely.\n" +
      "Focus on: what was discussed, what actions were taken, and what was decided.\n" +
      "Bullet points are fine. Preserve all decision-relevant details.\n";
    if (previousSummary) {
      prompt += `\nPrevious summary (earlier context):\n${previousSummary}\n`;
    }
    prompt += `\nTranscript:\n${transcript}`;

    const request: CompletionRequest = {
      model: this.model,
      messages: [
        {
          role: "system",
          content: "You are a concise summarizer. Output only the summary, no preamble.",
        },
        { role: "user", content: prompt },
      ],
      maxCompletionTokens: this.config.compaction.summaryMaxCompletionTokens,
      temperature: 0.3,
    };

    const response = await this.provider.complete(request);
    const text = ((response.message["content"] as string | undefined) ?? "").trim();
    return text || "(Earlier context was summarized as empty.)";
  }
}

// ---------------------------------------------------------------------------
// Tool definition building
// ---------------------------------------------------------------------------

export function buildToolDefs(
  tools: McpTool[],
  includeReadResource: boolean
): ToolDef[] {
  const defs: ToolDef[] = tools.map((t) => ({
    type: "function",
    function: {
      name: t.name,
      description: t.description,
      parameters: t.inputSchema,
    },
  }));

  if (includeReadResource) {
    defs.push({
      type: "function",
      function: {
        name: READ_RESOURCE_TOOL_NAME,
        description: "Read an MCP resource by URI to inspect current state.",
        parameters: {
          type: "object",
          properties: {
            uri: {
              type: "string",
              description: "Full resource URI, e.g. prayer://dsl/reference",
            },
          },
          required: ["uri"],
        },
      },
    });
  }

  return defs;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function estimateTokens(messages: Message[], charsPerToken: number): number {
  return messages.reduce(
    (sum, m) => sum + estimateTokensForValue(m, charsPerToken),
    0
  );
}

function estimateTokensForValue(value: unknown, charsPerToken: number): number {
  const chars = JSON.stringify(value).length;
  return Math.ceil(chars / Math.max(1, charsPerToken));
}

function previewText(value: string, maxChars: number): string {
  if (value.length <= maxChars) return value;
  return value.slice(0, maxChars) + "...";
}

function formatTranscript(messages: Message[]): string {
  const lines: string[] = [];
  for (const msg of messages) {
    const role = ((msg["role"] as string | undefined) ?? "?").toUpperCase();
    const content = (msg["content"] as string | undefined) ?? "";
    if (role === "TOOL") {
      const callId = (msg["tool_call_id"] as string | undefined) ?? "";
      lines.push(`[TOOL RESULT id=${callId}]: ${content}`);
    } else {
      lines.push(`${role}: ${content}`);
    }
  }
  return lines.join("\n");
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
