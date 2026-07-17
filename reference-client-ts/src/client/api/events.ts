import type { JobRun } from "../../shared/types.js";
import { decodeArray, isRecord } from "./decoding.js";
import { decodeJobRun } from "./jobs.js";

export type StateSyncEvent = {
  type: "state_sync";
  jobRuns?: JobRun[];
};

export type JobRunUpdatedEvent = { type: "job_run_updated"; run: JobRun };

export type ServerEvent = StateSyncEvent | JobRunUpdatedEvent;

export function decodeServerEvent(value: unknown, browserEventType?: string): ServerEvent | null {
  if (!isRecord(value)) return null;
  const type = typeof value.type === "string" ? value.type : browserEventType;
  if (type === "state_sync") {
    if (value.jobRuns === undefined) return { type };
    const jobRuns = decodeArray(value.jobRuns, decodeJobRun);
    return jobRuns ? { type, jobRuns } : null;
  }
  if (type === "job_run_updated") {
    const run = decodeJobRun(value.run);
    return run ? { type, run } : null;
  }
  return null;
}

export function connectEvents(onEvent: (event: ServerEvent) => void, onError: (err: Event) => void): () => void {
  const es = new EventSource("/events");

  const handleMessage = (e: MessageEvent, eventType: string) => {
    try {
      const parsed: unknown = JSON.parse(String(e.data));
      const event = decodeServerEvent(parsed, eventType);
      if (event) onEvent(event);
    } catch {
      // Ignore malformed events.
    }
  };

  const eventTypes = ["state_sync", "job_run_updated"];

  for (const eventType of eventTypes) {
    es.addEventListener(eventType, (event) => handleMessage(event as MessageEvent, eventType));
  }
  es.onerror = onError;
  return () => es.close();
}
