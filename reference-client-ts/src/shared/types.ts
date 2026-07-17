export type JobKind = string;
export type JobRunStatus = "queued" | "starting" | "running" | "stopping" | "succeeded" | "failed" | "cancelled" | "interrupted";
export type JobConfig = { kind: string; botIds: string[]; [key: string]: unknown };
export type JobFieldDefinition = { name: string; label: string; type: "text" | "number" | "boolean" | "textarea"; required?: boolean; description?: string };
export type JobDefinition = {
  kind: JobKind;
  title: string;
  description: string;
  mode: "one_shot" | "continuous";
  fields: JobFieldDefinition[];
  defaults: Record<string, unknown>;
  capabilities: string[];
};
export type JobRunEvent = { at: string; level: "info" | "warning" | "error"; message: string };
export type JobBotRunState = {
  botId: string;
  name?: string;
  status: string;
  currentWork?: string;
  prayerRunId?: string;
  movementId?: string;
  lastError?: string;
  consecutiveFailures?: number;
  updatedAt: string;
};
export type JobRun = {
  id: string;
  pluginId?: string;
  squadId: string;
  squadName: string;
  kind: JobKind;
  config: JobConfig;
  status: JobRunStatus;
  phase: string;
  createdAt: string;
  startedAt?: string;
  updatedAt: string;
  finishedAt?: string;
  stopRequestedAt?: string;
  summary: Record<string, number | string | boolean | null>;
  botStates: Record<string, JobBotRunState>;
  events: JobRunEvent[];
  artifacts?: Record<string, unknown>;
  lastError?: { message: string; code?: string };
  revision: number;
};

export type Squad = { id: string; name: string; color: string; priority: number; botIds: string[]; createdAt: string; updatedAt: string };

export type SessionInfo = {
  sessionHandle: string;
  latestSystem?: string | null;
  latestPoi?: string | null;
};
