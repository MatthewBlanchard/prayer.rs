import { JOB_DEFINITIONS } from "./definitions.js";
import type { JobConfig } from "./types.js";
import type { PluginRegistry } from "../plugins/registry.js";

export function parseJobConfig(value: unknown, plugins?: PluginRegistry): JobConfig {
  if (!value || typeof value !== "object") throw new Error("configuration is required");
  const input = value as Record<string, unknown>;
  const definition = [...JOB_DEFINITIONS, ...(plugins?.definitions() ?? [])].find((candidate) => candidate.kind === input["kind"]);
  if (!definition) throw new Error("unknown job kind");
  if (!Array.isArray(input["botIds"]) || input["botIds"].length === 0 || input["botIds"].some((id) => typeof id !== "string" || !id.trim()))
    throw new Error("at least one bot ID is required");
  const botIds = [...new Set((input["botIds"] as string[]).map((id) => id.trim()))];
  const allowed = new Set(["kind", "botIds", ...definition.fields.map((field) => field.name), ...Object.keys(definition.defaults)]);
  const supplied = Object.fromEntries(Object.entries(input).filter(([key]) => allowed.has(key)));
  const config: Record<string, unknown> = { ...definition.defaults, ...supplied, kind: definition.kind, botIds };
  if (typeof config["itemIds"] === "string")
    config["itemIds"] = config["itemIds"]
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean);
  for (const field of definition.fields) {
    const fieldValue = config[field.name];
    if (field.required && (fieldValue === undefined || fieldValue === null || fieldValue === "")) throw new Error(`${field.label} is required`);
    if (field.type === "number" && fieldValue !== undefined && (typeof fieldValue !== "number" || !Number.isFinite(fieldValue)))
      throw new Error(`${field.label} must be a finite number`);
  }
  for (const key of ["quantity", "limit", "idleDelayMs"] as const)
    if (typeof config[key] === "number" && config[key] <= 0) throw new Error(`${key} must be greater than zero`);
  for (const key of ["minProfitPerJump", "minScore", "maxUnits"] as const)
    if (typeof config[key] === "number" && config[key] < 0) throw new Error(`${key} cannot be negative`);
  if (definition.kind === "mine" && !["storage", "space"].includes(String(config["disposition"]))) throw new Error("disposition must be storage or space");
  if (definition.kind === "mine" && !["personal", "faction"].includes(String(config["storageTarget"])))
    throw new Error("storageTarget must be personal or faction");
  return plugins?.validate(config as JobConfig) ?? (config as JobConfig);
}
