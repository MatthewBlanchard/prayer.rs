import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { PLUGIN_API_VERSION, type JobConfig, type PluginDescriptor, type PluginManifest } from "../../plugin-api/shared.js";
import type { JobPlugin, ServerPlugin } from "../../plugin-api/server.js";

const ID = /^[a-z][a-z0-9_-]*$/;
const ENTRY = /^\.\/[a-zA-Z0-9_./-]+\.(?:ts|tsx|js|mjs)$/;

export class PluginRegistry {
  private readonly jobs = new Map<string, JobPlugin>();
  private readonly owners = new Map<string, string>();
  private readonly servers: ServerPlugin[] = [];
  constructor(readonly plugins: PluginDescriptor[]) {}
  definitions() { return this.plugins.flatMap((plugin) => plugin.jobs); }
  job(kind: string) { return this.jobs.get(kind); }
  owner(kind: string) { return this.owners.get(kind); }
  register(pluginId: string, job: JobPlugin): void {
    if (this.jobs.has(job.definition.kind)) throw new Error(`duplicate job kind '${job.definition.kind}' (plugin '${pluginId}')`);
    this.jobs.set(job.definition.kind, job);
    this.owners.set(job.definition.kind, pluginId);
  }
  validate(config: JobConfig): JobConfig { return this.job(config.kind)?.validate?.(config) ?? config; }
  addServer(server: ServerPlugin) { this.servers.push(server); }
  async registerRoutes(app: import("express").Express) { for (const server of this.servers) await server.routes?.(app); }
}

export async function discoverPlugins(root: string): Promise<PluginRegistry> {
  let entries: import("node:fs").Dirent[];
  try { entries = await fs.readdir(root, { withFileTypes: true }); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return new PluginRegistry([]); throw error; }
  const manifests: Array<{ directory: string; manifest: PluginManifest }> = [];
  const ids = new Set<string>();
  for (const entry of entries.filter((item) => item.isDirectory()).sort((a, b) => a.name.localeCompare(b.name))) {
    const directory = path.join(root, entry.name);
    const manifest = validateManifest(JSON.parse(await fs.readFile(path.join(directory, "plugin.json"), "utf8")), entry.name);
    if (ids.has(manifest.id)) throw new Error(`duplicate plugin ID '${manifest.id}'`);
    ids.add(manifest.id);
    manifests.push({ directory, manifest });
  }
  for (const { directory, manifest } of manifests)
    for (const key of ["shared", "client", "server"] as const) if (manifest[key]) await fs.access(path.resolve(directory, manifest[key]));
  const registry = new PluginRegistry(manifests.map(({ manifest }) => ({ id: manifest.id, version: manifest.version, jobs: [] })));
  for (const [index, item] of manifests.entries()) {
    if (!item.manifest.server) continue;
    const module = await import(pathToFileURL(path.resolve(item.directory, item.manifest.server)).href) as { default?: ServerPlugin; plugin?: ServerPlugin };
    const server = module.default ?? module.plugin;
    if (!server) throw new Error(`plugin '${item.manifest.id}' server entry must export default or plugin`);
    registry.addServer(server);
    for (const job of server.jobs ?? []) {
      registry.register(item.manifest.id, job);
      registry.plugins[index]!.jobs.push(job.definition);
    }
  }
  return registry;
}

export function validateManifest(value: unknown, folder = "plugin"): PluginManifest {
  if (!value || typeof value !== "object") throw new Error(`${folder}/plugin.json must contain an object`);
  const input = value as Record<string, unknown>;
  if (typeof input["id"] !== "string" || !ID.test(input["id"])) throw new Error(`${folder}: invalid plugin ID`);
  if (typeof input["version"] !== "string" || !/^\d+\.\d+\.\d+(?:[-+].+)?$/.test(input["version"])) throw new Error(`${folder}: invalid plugin version`);
  if (input["hostApiVersion"] !== PLUGIN_API_VERSION) throw new Error(`${folder}: incompatible host API version ${String(input["hostApiVersion"])} (expected ${PLUGIN_API_VERSION})`);
  const capabilities = input["capabilities"];
  if (!Array.isArray(capabilities) || capabilities.some((item) => item !== "jobs" && item !== "sidebar_panels")) throw new Error(`${folder}: invalid capabilities`);
  for (const key of ["shared", "client", "server"] as const) if (input[key] !== undefined && (typeof input[key] !== "string" || !ENTRY.test(input[key]))) throw new Error(`${folder}: invalid ${key} entry`);
  if (capabilities.includes("jobs") && typeof input["server"] !== "string") throw new Error(`${folder}: jobs capability requires a server entry`);
  if (capabilities.includes("sidebar_panels") && typeof input["client"] !== "string") throw new Error(`${folder}: sidebar_panels capability requires a client entry`);
  return input as PluginManifest;
}
