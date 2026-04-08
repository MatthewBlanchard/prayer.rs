const GEMINI_BASE_URL =
  "https://generativelanguage.googleapis.com/v1beta/models";

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

export interface CompletionRequest {
  model?: string;
  messages: Message[];
  tools?: ToolDef[];
  toolChoice?: string;
  maxCompletionTokens: number;
  temperature: number;
}

export interface CompletionResponse {
  message: Message;
  finishReason: string;
}

export type Message = Record<string, unknown>;
export type ToolDef = Record<string, unknown>;

export class CompletionError extends Error {
  constructor(
    message: string,
    public readonly status?: number,
    public readonly body?: string
  ) {
    super(message);
    this.name = "CompletionError";
  }
}

export interface CompletionProvider {
  complete(request: CompletionRequest): Promise<CompletionResponse>;
}

// ---------------------------------------------------------------------------
// OpenAI-compatible provider
// ---------------------------------------------------------------------------

export class OpenAiProvider implements CompletionProvider {
  private readonly completionsUrl: string;

  constructor(
    llmBaseUrl: string,
    private readonly apiKey: string | undefined
  ) {
    this.completionsUrl = `${llmBaseUrl.replace(/\/$/, "")}/chat/completions`;
  }

  async complete(request: CompletionRequest): Promise<CompletionResponse> {
    const body: Record<string, unknown> = {
      messages: request.messages,
      max_completion_tokens: request.maxCompletionTokens,
      temperature: request.temperature,
    };

    if (request.model) body.model = request.model;
    if (request.tools?.length) body.tools = request.tools;
    if (request.toolChoice) body.tool_choice = request.toolChoice;

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };
    if (this.apiKey) headers["Authorization"] = `Bearer ${this.apiKey}`;

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 120_000);

    let res: Response;
    try {
      res = await fetch(this.completionsUrl, {
        method: "POST",
        headers,
        body: JSON.stringify(body),
        signal: controller.signal,
      });
    } finally {
      clearTimeout(timer);
    }

    if (!res.ok) {
      const text = await res.text().catch(() => "");
      throw new CompletionError(
        `Provider returned ${res.status}`,
        res.status,
        text
      );
    }

    const payload = (await res.json()) as Record<string, unknown>;
    const choices = payload["choices"] as Array<Record<string, unknown>>;
    const choice = choices?.[0];
    if (!choice) {
      throw new CompletionError("missing choices[0]");
    }

    const message = choice["message"] as Message;
    const finishReason =
      (choice["finish_reason"] as string | undefined) ?? "stop";

    return { message, finishReason };
  }
}

// ---------------------------------------------------------------------------
// Google Gemini provider
// ---------------------------------------------------------------------------

export class GeminiProvider implements CompletionProvider {
  constructor(private readonly apiKey: string) {}

  private endpoint(model: string): string {
    return `${GEMINI_BASE_URL}/${model}:generateContent?key=${this.apiKey}`;
  }

  // Convert OpenAI messages → Gemini contents + system_instruction
  private convertMessages(messages: Message[]): {
    systemInstruction: Record<string, unknown> | undefined;
    contents: Record<string, unknown>[];
  } {
    let systemText = "";
    const contents: Record<string, unknown>[] = [];

    for (const msg of messages) {
      const role = (msg["role"] as string | undefined) ?? "user";

      switch (role) {
        case "system": {
          const text = (msg["content"] as string | undefined) ?? "";
          if (systemText) systemText += "\n";
          systemText += text;
          break;
        }

        case "assistant": {
          const parts: Record<string, unknown>[] = [];
          const text = msg["content"] as string | undefined;
          if (text?.trim()) parts.push({ text });

          const toolCalls = msg["tool_calls"] as
            | Array<Record<string, unknown>>
            | undefined;
          if (toolCalls) {
            for (const call of toolCalls) {
              const fn = call["function"] as Record<string, unknown> | undefined;
              const name = (fn?.["name"] as string | undefined) ?? "";
              const argsStr = (fn?.["arguments"] as string | undefined) ?? "{}";
              let argsObj: unknown;
              try {
                argsObj = JSON.parse(argsStr);
              } catch {
                argsObj = {};
              }
              parts.push({ functionCall: { name, args: argsObj } });
            }
          }

          if (!parts.length) parts.push({ text: "" });
          contents.push({ role: "model", parts });
          break;
        }

        case "tool": {
          const callId =
            (msg["tool_call_id"] as string | undefined) ?? "";
          // Best-effort: derive function name from call id
          const fnName =
            callId.split("_").slice(-2, -1)[0] ?? callId;
          const resultText = (msg["content"] as string | undefined) ?? "";

          // Group consecutive tool responses under a single user turn
          const last = contents[contents.length - 1];
          if (last && (last["role"] as string) === "user") {
            (last["parts"] as Array<unknown>).push({
              functionResponse: {
                name: fnName,
                response: { content: resultText },
              },
            });
          } else {
            contents.push({
              role: "user",
              parts: [
                {
                  functionResponse: {
                    name: fnName,
                    response: { content: resultText },
                  },
                },
              ],
            });
          }
          break;
        }

        default: {
          // user
          const text = (msg["content"] as string | undefined) ?? "";
          contents.push({ role: "user", parts: [{ text }] });
          break;
        }
      }
    }

    const systemInstruction = systemText
      ? { parts: [{ text: systemText }] }
      : undefined;

    return { systemInstruction, contents };
  }

  // Convert OpenAI tools → Gemini functionDeclarations
  private convertTools(tools: ToolDef[]): Record<string, unknown>[] {
    const decls = tools
      .map((t) => {
        const f = t["function"] as Record<string, unknown> | undefined;
        if (!f) return null;
        const decl: Record<string, unknown> = {
          name: f["name"],
          description: (f["description"] as string | undefined) ?? "",
        };
        const params = f["parameters"];
        if (params) decl["parameters"] = this.sanitizeSchema(params);
        return decl;
      })
      .filter((d): d is Record<string, unknown> => d !== null);

    return [{ functionDeclarations: decls }];
  }

  // Recursively strip $schema and flatten union types for Gemini
  private sanitizeSchema(schema: unknown): unknown {
    if (Array.isArray(schema)) {
      return schema.map((v) => this.sanitizeSchema(v));
    }
    if (schema !== null && typeof schema === "object") {
      const obj = schema as Record<string, unknown>;
      const out: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(obj)) {
        if (k === "$schema") continue;
        if (k === "type" && Array.isArray(v)) {
          // flatten ["string", "null"] → "string"
          const picked =
            (v as string[]).find((s) => s !== "null") ?? "string";
          out[k] = picked;
          continue;
        }
        out[k] = this.sanitizeSchema(v);
      }
      return out;
    }
    return schema;
  }

  // Convert Gemini candidate → OpenAI-format message + finishReason
  private convertResponse(candidate: Record<string, unknown>): {
    message: Message;
    finishReason: string;
  } {
    const content = candidate["content"] as
      | Record<string, unknown>
      | undefined;
    const parts = (content?.["parts"] as Array<Record<string, unknown>>) ?? [];

    let text = "";
    const toolCalls: Record<string, unknown>[] = [];

    for (let idx = 0; idx < parts.length; idx++) {
      const part = parts[idx];
      if (typeof part["text"] === "string") {
        const raw = part["text"];
        // Thinking-capable models (e.g. gemini-2.0-flash-thinking) return
        // thought parts with `"thought": true`.
        if (part["thought"] === true) {
          text += `<thinking>${raw}</thinking>`;
        } else {
          // Gemma instruction-tuned models emit thinking via inline channel
          // tokens: <|channel>thought\n...<|channel>response\n...
          // Normalise these to <thinking> tags.
          text += normalizeChannelThoughts(raw);
        }
      }
      const fc = part["functionCall"] as
        | Record<string, unknown>
        | undefined;
      if (fc) {
        const name = (fc["name"] as string | undefined) ?? "";
        const argsObj = fc["args"] ?? {};
        const arguments_ = JSON.stringify(argsObj);
        toolCalls.push({
          id: `call_${idx}`,
          type: "function",
          function: { name, arguments: arguments_ },
        });
      }
    }

    const rawFinish =
      (candidate["finishReason"] as string | undefined) ?? "STOP";
    let finishReason: string;
    if (rawFinish === "MAX_TOKENS") finishReason = "length";
    else if (rawFinish === "SAFETY") finishReason = "content_filter";
    else if (toolCalls.length > 0) finishReason = "tool_calls";
    else finishReason = "stop";

    const message: Message = {
      role: "assistant",
      content: text || null,
    };
    if (toolCalls.length > 0) message["tool_calls"] = toolCalls;

    return { message, finishReason };
  }

  async complete(request: CompletionRequest): Promise<CompletionResponse> {
    const model = request.model ?? "gemma-4-31b-it";
    const { systemInstruction, contents } = this.convertMessages(
      request.messages
    );

    const body: Record<string, unknown> = {
      contents,
      generationConfig: {
        maxOutputTokens: request.maxCompletionTokens,
        temperature: request.temperature,
      },
    };

    if (systemInstruction) body["system_instruction"] = systemInstruction;

    if (request.tools?.length) {
      body["tools"] = this.convertTools(request.tools);
      body["tool_config"] = {
        function_calling_config: { mode: "AUTO" },
      };
    }

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 120_000);

    let res: Response;
    try {
      res = await fetch(this.endpoint(model), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal: controller.signal,
      });
    } finally {
      clearTimeout(timer);
    }

    if (!res.ok) {
      const text = await res.text().catch(() => "");
      throw new CompletionError(
        `Gemini returned ${res.status}`,
        res.status,
        text
      );
    }

    const payload = (await res.json()) as Record<string, unknown>;
    const candidates = payload["candidates"] as
      | Array<Record<string, unknown>>
      | undefined;
    const candidate = candidates?.[0];
    if (!candidate) {
      throw new CompletionError("missing candidates[0]");
    }

    const { message, finishReason } = this.convertResponse(candidate);
    return { message, finishReason };
  }
}

// ---------------------------------------------------------------------------
// Union type for runtime dispatch
// ---------------------------------------------------------------------------

export type AnyProvider = OpenAiProvider | GeminiProvider;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Gemma instruction-tuned models emit thinking via special channel tokens:
 *
 *   <|channel>thought\n<thinking content>\n<|channel>response\n<actual response>
 *
 * This function converts that format into <thinking>...</thinking> tags so the
 * rest of the pipeline can handle it uniformly.
 */
function normalizeChannelThoughts(text: string): string {
  // Match <|channel>thought ... up to the next <|channel> marker or end of string
  return text.replace(
    /<\|channel>thought\n?([\s\S]*?)(?=<\|channel>|$)/g,
    (_, thinking: string) => `<thinking>${thinking.trim()}</thinking>`
  ).replace(/<\|channel>response\n?/g, "");
}
