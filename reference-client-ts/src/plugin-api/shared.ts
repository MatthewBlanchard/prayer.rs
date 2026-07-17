export const PLUGIN_API_VERSION = 1;

export type PluginCapability = "jobs" | "sidebar_panels";

export type PluginManifest = {
  id: string;
  version: string;
  hostApiVersion: number;
  shared?: string;
  client?: string;
  server?: string;
  capabilities: PluginCapability[];
};

export type JobFieldDefinition = {
  name: string;
  label: string;
  type: "text" | "number" | "boolean" | "textarea";
  required?: boolean;
  description?: string;
};

export type JobDefinition = {
  kind: string;
  title: string;
  description: string;
  mode: "one_shot" | "continuous";
  fields: JobFieldDefinition[];
  defaults: Record<string, unknown>;
  capabilities: string[];
};

export type JobConfig = { kind: string; botIds: string[]; [key: string]: unknown };

export type PluginDescriptor = { id: string; version: string; jobs: JobDefinition[] };
