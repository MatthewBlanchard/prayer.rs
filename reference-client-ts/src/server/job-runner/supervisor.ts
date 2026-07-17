import type { ActionRun, Prayer, ScriptRun } from "@prayer/sdk";
import { JOB_DEFINITIONS } from "./definitions.js";
import { scriptFor } from "./scripts.js";
import { JobRunStore } from "./store.js";
import { ACTIVE_JOB_STATUSES, type JobRun } from "./types.js";
import { parseJobConfig } from "./validation.js";
import type { PluginRegistry } from "../plugins/registry.js";

type StopMode = "after_current" | "halt_now";
type ActiveProcess = { controller: AbortController; handles: Map<string, ScriptRun | ActionRun>; stopSettled?: Promise<void> };

export class JobSupervisor {
  private readonly active = new Map<string, ActiveProcess>();
  private readonly locks = new Map<string, string>();
  constructor(
    private readonly prayer: Prayer,
    private readonly store: JobRunStore,
    private readonly publish: (run: JobRun) => void,
    private readonly plugins?: PluginRegistry,
  ) {}
  listDefinitions() { return [...JOB_DEFINITIONS, ...(this.plugins?.definitions() ?? [])]; }
  listRuns(filter: { status?: string; kind?: string; limit?: number } = {}) {
    return this.store.list().filter((run) => !filter.status || run.status === filter.status).filter((run) => !filter.kind || run.kind === filter.kind).slice(0, filter.limit ?? 50);
  }
  getRun(id: string) { return this.store.get(id); }
  async start(input: unknown, squad: { id: string; name: string; botIds: string[] }): Promise<JobRun> {
    const fleet = await this.prayer.bots();
    const identifiers = new Map(fleet.flatMap((bot) => [[bot.botId, bot.botId], ...(bot.name ? [[bot.name, bot.botId] as const] : [])]));
    const config = parseJobConfig({ ...(input as Record<string, unknown>), botIds: squad.botIds.map((id) => identifiers.get(id) ?? id) }, this.plugins);
    for (const botId of config.botIds) if (this.locks.has(botId)) throw new Error(`bot ${botId} is locked by run ${this.locks.get(botId)}`);
    const now = new Date().toISOString();
    const known = new Map(fleet.map((bot) => [bot.botId, bot]));
    const run: JobRun = { id: `job_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`, pluginId: this.plugins?.owner(config.kind), squadId: squad.id, squadName: squad.name, kind: config.kind, config, status: "queued", phase: "queued", createdAt: now, updatedAt: now, summary: { bots: config.botIds.length, completed: 0, failed: 0 }, botStates: Object.fromEntries(config.botIds.map((botId) => [botId, { botId, name: known.get(botId)?.name ?? undefined, status: "queued", updatedAt: now }])), events: [{ at: now, level: "info", message: "Run queued" }], revision: 1 };
    await this.store.put(run); this.publish(run); this.launch(run); return run;
  }
  async stop(id: string, mode: StopMode = "after_current"): Promise<JobRun> {
    const run = this.requireRun(id); const process = this.active.get(id);
    if (!ACTIVE_JOB_STATUSES.has(run.status)) throw new Error("run is already terminal");
    if (!process) throw new Error("run process is not active");
    await this.update(run, { status: "stopping", phase: mode === "halt_now" ? "halting" : "finishing current operations", stopRequestedAt: new Date().toISOString() }, `Stop requested (${mode})`);
    if (mode === "halt_now") { process.controller.abort(new Error("stop requested")); process.stopSettled = Promise.allSettled([...process.handles.values()].map((handle) => handle.cancel("job halted by user"))).then(() => undefined); await process.stopSettled; }
    return run;
  }
  async delete(id: string) { const run = this.requireRun(id); if (ACTIVE_JOB_STATUSES.has(run.status)) throw new Error("active runs cannot be deleted"); return this.store.delete(id); }
  async recover(): Promise<void> {
    for (const run of this.store.list().filter((candidate) => ACTIVE_JOB_STATUSES.has(candidate.status))) {
      if (run.pluginId && this.plugins?.owner(run.kind) !== run.pluginId) { await this.update(run, { status: "interrupted", phase: "plugin unavailable", finishedAt: new Date().toISOString(), lastError: { message: `Plugin '${run.pluginId}' is unavailable` } }, "Run could not recover because its plugin is unavailable", "error"); continue; }
      const conflict = run.config.botIds.find((botId) => this.locks.has(botId));
      if (conflict) { await this.update(run, { status: "interrupted", phase: "recovery lock conflict", finishedAt: new Date().toISOString(), lastError: { message: `Bot ${conflict} is locked` } }, "Run could not recover because its bot lock was claimed", "error"); continue; }
      await this.update(run, { status: "queued", phase: "recovering" }, "Recovering run after server restart"); this.launch(run, true);
    }
  }
  private launch(run: JobRun, recovering = false) {
    for (const botId of run.config.botIds) this.locks.set(botId, run.id);
    const process: ActiveProcess = { controller: new AbortController(), handles: new Map() }; this.active.set(run.id, process);
    void this.execute(run, process, recovering).finally(() => { this.active.delete(run.id); for (const botId of run.config.botIds) if (this.locks.get(botId) === run.id) this.locks.delete(botId); });
  }
  private async execute(run: JobRun, process: ActiveProcess, recovering: boolean) {
    try {
      await this.update(run, { status: "starting", phase: recovering ? "recovering" : "starting", startedAt: run.startedAt ?? new Date().toISOString() }, "Starting run");
      await this.waitForConnections(run, process.controller.signal);
      await this.update(run, { status: "running", phase: "executing" }, "Run started");
      const plugin = this.plugins?.job(run.kind);
      if (plugin) await plugin.execute({ prayer: this.prayer, run, config: run.config, signal: process.controller.signal, recovering, update: (patch, message, level) => this.update(run, patch, message, level), setBot: (botId, patch) => this.setBot(run, botId, patch), delay: (ms) => delay(ms, process.controller.signal), execute: (botId, actions, options) => this.executePluginActions(process, botId, actions, options) });
      else if (JOB_DEFINITIONS.some((definition) => definition.kind === run.kind)) await this.runScripts(run, process, recovering);
      else throw new Error(`plugin for job kind '${run.kind}' is unavailable`);
      const cancelled = Boolean(run.stopRequestedAt); await this.update(run, { status: cancelled ? "cancelled" : "succeeded", phase: cancelled ? "cancelled" : "complete", finishedAt: new Date().toISOString() }, cancelled ? "Run stopped" : "Run completed");
    } catch (error) { await process.stopSettled; const message = error instanceof Error ? error.message : String(error); await this.update(run, { status: run.stopRequestedAt ? "cancelled" : "failed", phase: run.stopRequestedAt ? "cancelled" : "failed", finishedAt: new Date().toISOString(), ...(run.stopRequestedAt ? {} : { lastError: { message } }) }, run.stopRequestedAt ? "Run stopped" : message, run.stopRequestedAt ? "info" : "error"); }
  }
  private async runScripts(run: JobRun, process: ActiveProcess, recovering: boolean) {
    const ids = recovering ? run.config.botIds.filter((id) => !["succeeded", "failed"].includes(run.botStates[id]?.status ?? "")) : run.config.botIds;
    const quantity = typeof run.config.quantity === "number" ? run.config.quantity : 0;
    const results = await Promise.allSettled(ids.map(async (botId) => { const index = run.config.botIds.indexOf(botId); const config = run.kind === "mine" ? { ...run.config, quantity: Math.floor(quantity / run.config.botIds.length) + (index < quantity % run.config.botIds.length ? 1 : 0) } : run.config; await this.runScript(run, process, botId, scriptFor(config)); }));
    const failed = results.find((result) => result.status === "rejected"); if (failed?.status === "rejected") throw failed.reason;
  }
  private async runScript(run: JobRun, process: ActiveProcess, botId: string, script: string) {
    const bot = await this.prayer.bot(botId); const handle = await bot.startScript(script, { idempotencyKey: `${run.id}:${botId}:script` }); process.handles.set(botId, handle); await this.setBot(run, botId, { status: "running", prayerRunId: handle.id, prayerRunKind: "script" });
    try { const outcome = await handle.wait({ signal: process.controller.signal, pollMs: 500 }); if (outcome.status !== "succeeded") throw new Error(`Prayer run ${handle.id} ended ${outcome.status}`); run.summary["completed"] = Number(run.summary["completed"] ?? 0) + 1; await this.setBot(run, botId, { status: "succeeded", prayerRunId: undefined, prayerRunKind: undefined }); }
    finally { process.handles.delete(botId); }
  }
  private async executePluginActions(process: ActiveProcess, botId: string, actions: Parameters<Awaited<ReturnType<Prayer["bot"]>>["startActions"]>[0], options?: { idempotencyKey?: string }) {
    if (process.controller.signal.aborted) throw process.controller.signal.reason;
    const bot = await this.prayer.bot(botId, { signal: process.controller.signal });
    const handle = await bot.startActions(actions, { ...options, signal: process.controller.signal });
    process.handles.set(`${botId}:actions:${handle.id}`, handle);
    try {
      const outcome = await handle.wait({ signal: process.controller.signal, pollMs: 250 });
      if (outcome.status !== "succeeded") throw new Error(`Prayer action run ${handle.id} ended ${outcome.status}: ${JSON.stringify("outcome" in outcome ? outcome.outcome : outcome)}`);
      return outcome;
    } finally {
      process.handles.delete(`${botId}:actions:${handle.id}`);
    }
  }
  private async waitForConnections(run: JobRun, signal: AbortSignal) { while (!signal.aborted) { const fleet = await this.prayer.bots(); const known = new Map(fleet.map((bot) => [bot.botId, bot])); const waiting = run.config.botIds.filter((id) => String(known.get(id)?.connection ?? "").toLowerCase() !== "connected"); if (!waiting.length) return; await this.update(run, { phase: `waiting for ${waiting.length} squad member(s)` }); await delay(1000, signal); } throw signal.reason; }
  private async setBot(run: JobRun, botId: string, patch: Partial<JobRun["botStates"][string]>) { run.botStates[botId] = { ...run.botStates[botId]!, ...patch, botId, updatedAt: new Date().toISOString() }; await this.update(run, {}); }
  private async update(run: JobRun, patch: Partial<JobRun>, message?: string, level: "info" | "warning" | "error" = "info") { Object.assign(run, patch); run.updatedAt = new Date().toISOString(); run.revision += 1; if (message) run.events.push({ at: run.updatedAt, level, message }); await this.store.put(run); this.publish(run); }
  private requireRun(id: string) { const run = this.store.get(id); if (!run) throw new Error("job run not found"); return run; }
}
function delay(ms: number, signal: AbortSignal) { return new Promise<void>((resolve, reject) => { if (signal.aborted) return reject(signal.reason); const timer = setTimeout(resolve, ms); signal.addEventListener("abort", () => { clearTimeout(timer); reject(signal.reason); }, { once: true }); }); }
