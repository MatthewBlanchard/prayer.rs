import { ToolLoopEvent } from "../shared/types.js";

export type StateSyncEvent = {
  type: "state_sync";
  messages: Record<string, unknown>[];
  model: string;
  busy: boolean;
};

export type ServerEvent = ToolLoopEvent | StateSyncEvent;

export function connectEvents(
  onEvent: (event: ServerEvent) => void,
  onError: (err: Event) => void
): () => void {
  const es = new EventSource("/events");

  const handleMessage = (e: MessageEvent, eventType: string) => {
    try {
      const data = JSON.parse(e.data as string) as ServerEvent;
      // attach the event type in case the parsed object doesn't have it
      if (!("type" in data)) {
        (data as Record<string, unknown>)["type"] = eventType;
      }
      onEvent(data);
    } catch {
      // ignore malformed events
    }
  };

  const eventTypes = [
    "turn_started",
    "assistant_draft",
    "tool_call_started",
    "tool_call_completed",
    "turn_completed",
    "error",
    "state_sync",
  ];

  for (const et of eventTypes) {
    es.addEventListener(et, (e) => handleMessage(e as MessageEvent, et));
  }

  es.onerror = onError;

  return () => es.close();
}

export async function sendMessage(content: string): Promise<void> {
  const res = await fetch("/api/chat", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ content }),
  });
  if (!res.ok) {
    const body = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(body.error ?? `HTTP ${res.status}`);
  }
}

export async function resetConversation(): Promise<void> {
  await fetch("/api/reset", { method: "POST" });
}
