import type {
  JobBotRunState,
  JobConfig,
  JobDefinition,
  JobFieldDefinition,
  JobRun,
  JobRunEvent,
  JobRunStatus,
  Squad,
} from "../../shared/types.js";
import { decodeArray, decodeResponse, isRecord, isStringArray } from "./decoding.js";

function decodeJobConfig(value: unknown): JobConfig | null {
  return isRecord(value) && typeof value.kind === "string" && isStringArray(value.botIds) ? { ...value, kind: value.kind, botIds: value.botIds } : null;
}

export function decodeJobRun(value: unknown): JobRun | null {
  if (
    !isRecord(value) ||
    typeof value.id !== "string" ||
    typeof value.squadId !== "string" ||
    typeof value.squadName !== "string" ||
    typeof value.kind !== "string" ||
    !isJobStatus(value.status) ||
    typeof value.phase !== "string" ||
    typeof value.createdAt !== "string" ||
    typeof value.updatedAt !== "string" ||
    typeof value.revision !== "number" ||
    !isRecord(value.summary) ||
    !isRecord(value.botStates) ||
    !Array.isArray(value.events)
  )
    return null;
  const config = decodeJobConfig(value.config);
  const summary = decodeSummary(value.summary);
  const botStates = decodeBotStates(value.botStates);
  const events = decodeArray(value.events, decodeJobRunEvent);
  if (
    !config ||
    !summary ||
    !botStates ||
    !events ||
    !optionalString(value.pluginId) ||
    !optionalString(value.startedAt) ||
    !optionalString(value.finishedAt) ||
    !optionalString(value.stopRequestedAt)
  )
    return null;
  if (value.artifacts !== undefined && !isRecord(value.artifacts)) return null;
  const lastError = value.lastError === undefined ? undefined : decodeLastError(value.lastError);
  if (value.lastError !== undefined && !lastError) return null;
  return {
    id: value.id,
    squadId: value.squadId,
    squadName: value.squadName,
    kind: value.kind,
    config,
    status: value.status,
    phase: value.phase,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
    summary,
    botStates,
    events,
    revision: value.revision,
    ...(typeof value.pluginId === "string" ? { pluginId: value.pluginId } : {}),
    ...(typeof value.startedAt === "string" ? { startedAt: value.startedAt } : {}),
    ...(typeof value.finishedAt === "string" ? { finishedAt: value.finishedAt } : {}),
    ...(typeof value.stopRequestedAt === "string" ? { stopRequestedAt: value.stopRequestedAt } : {}),
    ...(isRecord(value.artifacts) ? { artifacts: value.artifacts } : {}),
    ...(lastError ? { lastError } : {}),
  };
}

function isJobStatus(value: unknown): value is JobRunStatus {
  switch (value) {
    case "queued":
    case "starting":
    case "running":
    case "stopping":
    case "succeeded":
    case "failed":
    case "cancelled":
    case "interrupted":
      return true;
    default:
      return false;
  }
}
function optionalString(value: unknown): value is string | undefined {
  return value === undefined || typeof value === "string";
}
function decodeSummary(value: unknown): JobRun["summary"] | null {
  if (!isRecord(value)) return null;
  const summary: JobRun["summary"] = {};
  for (const [key, item] of Object.entries(value)) {
    if (item !== null && typeof item !== "string" && typeof item !== "number" && typeof item !== "boolean") return null;
    summary[key] = item;
  }
  return summary;
}
function decodeJobRunEvent(value: unknown): JobRunEvent | null {
  if (!isRecord(value) || typeof value.at !== "string" || typeof value.message !== "string") return null;
  const level = decodeJobRunEventLevel(value.level);
  return level ? { at: value.at, level, message: value.message } : null;
}
function decodeJobRunEventLevel(value: unknown): JobRunEvent["level"] | null {
  switch (value) {
    case "info":
    case "warning":
    case "error":
      return value;
    default:
      return null;
  }
}
function decodeLastError(value: unknown): NonNullable<JobRun["lastError"]> | null {
  if (!isRecord(value) || typeof value.message !== "string" || !optionalString(value.code)) return null;
  return { message: value.message, ...(typeof value.code === "string" ? { code: value.code } : {}) };
}
function decodeBotState(value: unknown): JobBotRunState | null {
  if (
    !isRecord(value) ||
    typeof value.botId !== "string" ||
    typeof value.status !== "string" ||
    typeof value.updatedAt !== "string" ||
    !optionalString(value.name) ||
    !optionalString(value.currentWork) ||
    !optionalString(value.prayerRunId) ||
    !optionalString(value.movementId) ||
    !optionalString(value.lastError) ||
    (value.consecutiveFailures !== undefined && typeof value.consecutiveFailures !== "number")
  )
    return null;
  return {
    botId: value.botId,
    status: value.status,
    updatedAt: value.updatedAt,
    ...(typeof value.name === "string" ? { name: value.name } : {}),
    ...(typeof value.currentWork === "string" ? { currentWork: value.currentWork } : {}),
    ...(typeof value.prayerRunId === "string" ? { prayerRunId: value.prayerRunId } : {}),
    ...(typeof value.movementId === "string" ? { movementId: value.movementId } : {}),
    ...(typeof value.lastError === "string" ? { lastError: value.lastError } : {}),
    ...(typeof value.consecutiveFailures === "number" ? { consecutiveFailures: value.consecutiveFailures } : {}),
  };
}
function decodeBotStates(value: unknown): Record<string, JobBotRunState> | null {
  if (!isRecord(value)) return null;
  const result: Record<string, JobBotRunState> = {};
  for (const [key, entry] of Object.entries(value)) {
    const decoded = decodeBotState(entry);
    if (!decoded) return null;
    result[key] = decoded;
  }
  return result;
}

function decodeJobDefinition(value: unknown): JobDefinition | null {
  if (
    !isRecord(value) ||
    typeof value.kind !== "string" ||
    typeof value.title !== "string" ||
    typeof value.description !== "string" ||
    (value.mode !== "one_shot" && value.mode !== "continuous") ||
    !Array.isArray(value.fields) ||
    !isRecord(value.defaults) ||
    !isStringArray(value.capabilities)
  )
    return null;
  const fields = decodeArray(value.fields, decodeJobField);
  return fields
    ? {
        kind: value.kind,
        title: value.title,
        description: value.description,
        mode: value.mode,
        fields,
        defaults: value.defaults,
        capabilities: value.capabilities,
      }
    : null;
}

function decodeJobField(value: unknown): JobFieldDefinition | null {
  if (
    !isRecord(value) ||
    typeof value.name !== "string" ||
    typeof value.label !== "string" ||
    (value.required !== undefined && typeof value.required !== "boolean") ||
    (value.description !== undefined && typeof value.description !== "string")
  )
    return null;
  const type = decodeJobFieldType(value.type);
  if (!type) return null;
  return {
    name: value.name,
    label: value.label,
    type,
    ...(typeof value.required === "boolean" ? { required: value.required } : {}),
    ...(typeof value.description === "string" ? { description: value.description } : {}),
  };
}

function decodeJobFieldType(value: unknown): JobFieldDefinition["type"] | null {
  switch (value) {
    case "text":
    case "number":
    case "boolean":
    case "textarea":
      return value;
    default:
      return null;
  }
}

function decodeSquad(value: unknown): Squad | null {
  if (
    !isRecord(value) ||
    typeof value.id !== "string" ||
    typeof value.name !== "string" ||
    typeof value.color !== "string" ||
    typeof value.priority !== "number" ||
    !isStringArray(value.botIds) ||
    typeof value.createdAt !== "string" ||
    typeof value.updatedAt !== "string"
  )
    return null;
  return {
    id: value.id,
    name: value.name,
    color: value.color,
    priority: value.priority,
    botIds: value.botIds,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
  };
}

export async function fetchJobDefinitions(): Promise<JobDefinition[]> {
  return decodeResponse(await fetch("/api/job-definitions"), (value) => decodeArray(value, decodeJobDefinition));
}
export async function fetchJobRuns(): Promise<JobRun[]> {
  return decodeResponse(await fetch("/api/job-runs?limit=50"), (value) => decodeArray(value, decodeJobRun));
}
export async function startJobRun(squadId: string, config: JobConfig): Promise<JobRun> {
  return decodeResponse(
    await fetch("/api/job-runs", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ squadId, config }) }),
    decodeJobRun,
  );
}
export const fetchSquads = async (): Promise<Squad[]> => decodeResponse(await fetch("/api/squads"), (value) => decodeArray(value, decodeSquad));
export const createSquad = async (): Promise<Squad> =>
  decodeResponse(await fetch("/api/squads", { method: "POST", headers: { "Content-Type": "application/json" }, body: "{}" }), decodeSquad);
export const updateSquad = async (id: string, patch: Partial<Pick<Squad, "name" | "color" | "priority" | "botIds">>): Promise<Squad> =>
  decodeResponse(
    await fetch(`/api/squads/${encodeURIComponent(id)}`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify(patch) }),
    decodeSquad,
  );
export const deleteSquad = async (id: string): Promise<void> => {
  const response = await fetch(`/api/squads/${encodeURIComponent(id)}`, { method: "DELETE" });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
};
export async function stopJobRun(id: string, mode: "after_current" | "halt_now"): Promise<JobRun> {
  return decodeResponse(
    await fetch(`/api/job-runs/${encodeURIComponent(id)}/stop`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ mode }),
    }),
    decodeJobRun,
  );
}
export type CharacterSkillInfo = {
  id: string;
  name: string;
  category: string | null;
  level: number | null;
  maxLevel: number | null;
  xp: number | null;
  nextLevelXp: number | null;
};
export type CharacterSkillsState = { sessionId: string | null; stateVersion: number | null; username: string | null; skills: CharacterSkillInfo[] };
