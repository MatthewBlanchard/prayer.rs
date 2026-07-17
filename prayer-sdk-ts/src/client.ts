import type { Action, ActionRunResponse, BotSummary, FleetEntry, QueueLane, QueueResponse, RouteQuery, RouteSelection as CachedRouteSelection, ScriptRunResponse, WorldState } from "./generated/types.js";
import type { StateSnapshot, WorldStateUpdate } from "./conveniences.js";
import { PrayerCompatibilityError } from "./errors.js";
import { Transport, type RequestOptions, type TransportOptions } from "./transport.js";
import { PrayerApi } from "./generated/api.js";

export interface SubmitOptions extends RequestOptions { idempotencyKey?: string }
export interface ExecuteOptions extends SubmitOptions { pollMs?: number }
export interface OverrideOptions extends RequestOptions { returnToOrigin?: boolean }
export interface WaitOptions<T> extends RequestOptions { pollMs?: number; onStatus?: (status: T) => void | Promise<void> }
export type ActionInput = Action | readonly Action[];
export interface PrayerAdvanced { readonly api: PrayerApi }
export interface RouteOptions { safe?: boolean }
export type RouteRequest = RouteQuery;
export type AuthoritativeRoute = CachedRouteSelection;

export class Prayer {
  private readonly transport: Transport;
  readonly advanced: PrayerAdvanced;
  private get api(): PrayerApi { return this.advanced.api; }
  private stateCache: StateSnapshot | undefined;
  private constructor(options: TransportOptions) { this.transport = new Transport(options); this.advanced = Object.freeze({ api: new PrayerApi(this.transport) }); }
  static async connect(options: TransportOptions): Promise<Prayer> {
    const prayer = new Prayer(options); const meta = await prayer.api.getMeta();
    if (meta.apiVersion.split(".")[0] !== "1") throw new PrayerCompatibilityError(`Unsupported Prayer API ${meta.apiVersion}`);
    return prayer;
  }
  async bots(options?: RequestOptions): Promise<BotSummary[]> { return this.api.listBots(options); }
  async route(from: string, to: string, options: RouteOptions = {}, requestOptions?: RequestOptions): Promise<AuthoritativeRoute | null> {
    return (await this.routes([{ from, to }], options, requestOptions))[0] ?? null;
  }
  async routes(routes: readonly RouteRequest[], options: RouteOptions = {}, requestOptions?: RequestOptions): Promise<Array<AuthoritativeRoute | null>> {
    return (await this.api.selectRoutes({ routes: [...routes], safe: options.safe ?? true }, requestOptions)).routes;
  }
  async state(options?: RequestOptions): Promise<StateSnapshot> {
    const versions = this.stateCache?.versions;
    const response = await this.api.getState(versions && {
      fleetVersion: versions.fleet, worldVersion: versions.world, mapVersion: versions.map,
      resourcesVersion: versions.resources,
      wildlifeVersion: versions.wildlife, marketsVersion: versions.markets,
      storageVersion: versions.storage, facilitiesVersion: versions.facilities,
      observationsVersion: versions.observations, communicationsVersion: versions.communications,
      factionsVersion: versions.factions, catalogVersion: versions.catalog,
    }, options);
    const fleet = response.fleet ?? this.stateCache?.fleet;
    let world: WorldState | undefined;
    try {
      world = response.world ? applyWorldUpdate(this.stateCache?.world, this.stateCache?.versions.markets, response.world) : this.stateCache?.world;
    } catch (error) {
      if (!(error instanceof PrayerCompatibilityError) || !response.world?.stationMarketDelta) throw error;
      const recovered = await this.api.getState({}, options);
      if (!recovered.fleet || !recovered.world || !recovered.catalog) throw error;
      const recoveredWorld = applyWorldUpdate(undefined, undefined, recovered.world);
      this.stateCache = deepFreeze({ versions: recovered.versions, fleet: recovered.fleet, world: recoveredWorld, catalog: recovered.catalog });
      return this.stateCache;
    }
    const catalog = response.catalog ?? this.stateCache?.catalog;
    if (!fleet || !world || !catalog) throw new PrayerCompatibilityError("Prayer API returned an incomplete initial state snapshot");
    this.stateCache = deepFreeze({ versions: response.versions, fleet, world, catalog });
    return this.stateCache;
  }
  async bot(selector: string, options?: RequestOptions): Promise<Bot> {
    const summary = await this.api.getBot(selector, options); return new Bot(this.api, summary, (stateOptions) => this.state(stateOptions));
  }
}

function applyWorldUpdate(previous: WorldState | undefined, previousMarketVersion: number | undefined, update: WorldStateUpdate): WorldState {
  let stationMarkets = update.stationMarkets ?? undefined;
  if (!stationMarkets) {
    if (update.stationMarketDelta) {
      if (!previous) throw new PrayerCompatibilityError("Prayer API returned a market delta without a cached base snapshot");
      if (update.stationMarketDelta.baseVersion !== previousMarketVersion) {
        throw new PrayerCompatibilityError(`Prayer API market delta base ${update.stationMarketDelta.baseVersion} does not match cached markets ${previousMarketVersion}`);
      }
      stationMarkets = { ...previous.stationMarkets, ...update.stationMarketDelta.upsert };
      for (const stationId of update.stationMarketDelta.remove) delete stationMarkets[stationId];
    } else {
      stationMarkets = previous?.stationMarkets ?? undefined;
    }
  }
  const { stationMarketDelta: _delta, ...world } = update;
  const merged = { ...previous, ...world, stationMarkets };
  if (!merged.map || !merged.resources || !merged.wildlife || !merged.stationMarkets || !merged.storageByPlayer || !merged.factionStorageByFactionPoi || !merged.facilitiesByPoi || !merged.ownedFacilitiesByPlayer || !merged.ownedFacilitiesByFaction || !merged.stationPassengers || !merged.salvageByPoi || !merged.agentSightings || !merged.chatMessagesBySession || !merged.factionBySession || !merged.updatedAtUtc) {
    throw new PrayerCompatibilityError("Prayer API returned an incomplete initial world snapshot");
  }
  return merged as WorldState;
}

function deepFreeze<T>(value: T): T {
  if (value === null || typeof value !== "object" || Object.isFrozen(value)) return value;
  for (const child of Object.values(value)) deepFreeze(child);
  return Object.freeze(value);
}

export class Bot {
  constructor(private readonly api: PrayerApi, public readonly summary: BotSummary, private readonly readState: (options?: RequestOptions) => Promise<StateSnapshot>) {}
  get id(): string { return this.summary.botId; }
  async state(options?: RequestOptions): Promise<FleetEntry> {
    const fleet = (await this.readState(options)).fleet;
    const entry = fleet.bots[this.id] ?? Object.values(fleet.bots).find((candidate) => candidate.id === this.id);
    if (!entry) throw new PrayerCompatibilityError(`Bot ${this.id} is missing from the aggregate state snapshot`);
    return entry;
  }
  async queue(options?: RequestOptions): Promise<QueueResponse> { return this.api.getBotQueue(this.id, options); }
  async normalQueue(options?: RequestOptions): Promise<QueueLane> { return this.api.getBotNormalQueue(this.id, options); }
  async overrideQueue(options?: RequestOptions): Promise<QueueLane> { return this.api.getBotOverrideQueue(this.id, options); }
  async halt(reason?: string, options?: RequestOptions): Promise<void> { await this.api.haltBot(this.id, reason ? { reason } : undefined, options); }
  async startActions(actions: ActionInput, options: SubmitOptions = {}): Promise<ActionRun> {
    const idempotencyKey = effectiveIdempotencyKey(options.idempotencyKey);
    const response = await this.api.startActionRun(this.id, { actions: Array.isArray(actions) ? [...actions] : [actions as Action] }, idempotencyKey, options);
    return new ActionRun(this.api, response, idempotencyKey);
  }
  /** @deprecated Prefer the explicit `startActions` name. */
  async start(actions: ActionInput, options: SubmitOptions = {}): Promise<ActionRun> { return this.startActions(actions, options); }
  async actionRun(runId: string, options?: RequestOptions): Promise<ActionRun> {
    return new ActionRun(this.api, await this.api.getActionRun(this.id, runId, options));
  }
  async execute(actions: ActionInput, options: ExecuteOptions = {}): Promise<ActionRunResponse> {
    return (await this.startActions(actions, options)).wait(options);
  }
  async executeActionOverride(actions: ActionInput, options: OverrideOptions = {}): Promise<void> {
    await this.api.executeActionOverride(this.id, {
      actions: Array.isArray(actions) ? [...actions] : [actions as Action],
      returnToOrigin: options.returnToOrigin ?? false,
    }, options);
  }
  async executeScriptOverride(script: string, options: OverrideOptions = {}): Promise<void> {
    await this.api.executeScriptOverride(this.id, {
      script,
      returnToOrigin: options.returnToOrigin ?? false,
    }, options);
  }
  async startScript(script: string, options: SubmitOptions = {}): Promise<ScriptRun> {
    const idempotencyKey = effectiveIdempotencyKey(options.idempotencyKey);
    const response = await this.api.startScriptRun(this.id, { script }, idempotencyKey, options);
    return new ScriptRun(this.api, response, idempotencyKey);
  }
  async scriptRun(runId: string, options?: RequestOptions): Promise<ScriptRun> {
    return new ScriptRun(this.api, await this.api.getScriptRun(this.id, runId, options));
  }
}

abstract class Run<T extends ActionRunResponse | ScriptRunResponse> {
  constructor(protected readonly api: PrayerApi, protected current: T, private readonly kind: "action-runs" | "script-runs", public readonly idempotencyKey?: string) {}
  get id(): string { return this.current.runId; } get prayerlang(): string { return this.current.prayerlang; }
  get snapshot(): T { return this.current; }
  get isTerminal(): boolean { return this.current.status !== "running"; }
  get succeeded(): boolean { return this.current.status === "succeeded"; }
  get cancellationKind(): "cancelled" | "halted" | undefined {
    return this.current.status === "cancelled" || this.current.status === "halted" ? this.current.status : undefined;
  }
  abstract get errorMessage(): string | undefined;
  abstract status(options?: RequestOptions): Promise<T>;
  async wait(options: WaitOptions<T> = {}): Promise<T> { while (this.current.status === "running") { await delay(options.pollMs ?? 250, options.signal); await this.status(options); await options.onStatus?.(this.current); } return this.current; }
  abstract cancel(reason?: string, options?: RequestOptions): Promise<T>;
}
export class ActionRun extends Run<ActionRunResponse> {
  constructor(api: PrayerApi, snapshot: ActionRunResponse, idempotencyKey?: string) { super(api, snapshot, "action-runs", idempotencyKey); }
  get errorMessage(): string | undefined {
    if (this.current.status === "running" || this.current.status === "succeeded") return undefined;
    return "message" in this.current.outcome ? this.current.outcome.message : "reason" in this.current.outcome ? this.current.outcome.reason : undefined;
  }
  async status(options?: RequestOptions): Promise<ActionRunResponse> { return this.current = await this.api.getActionRun(this.current.botId, this.id, options); }
  async cancel(reason?: string, options?: RequestOptions): Promise<ActionRunResponse> { return this.current = await this.api.cancelActionRun(this.current.botId, this.id, reason ? { reason } : undefined, options); }
}
export class ScriptRun extends Run<ScriptRunResponse> {
  constructor(api: PrayerApi, snapshot: ScriptRunResponse, idempotencyKey?: string) { super(api, snapshot, "script-runs", idempotencyKey); }
  get errorMessage(): string | undefined {
    if (this.current.status === "running" || this.current.status === "succeeded") return undefined;
    return this.current.outcome.status === "error" ? this.current.outcome.message : this.current.outcome.message ?? undefined;
  }
  async status(options?: RequestOptions): Promise<ScriptRunResponse> { return this.current = await this.api.getScriptRun(this.current.botId, this.id, options); }
  async cancel(reason?: string, options?: RequestOptions): Promise<ScriptRunResponse> { return this.current = await this.api.cancelScriptRun(this.current.botId, this.id, reason ? { reason } : undefined, options); }
}
function effectiveIdempotencyKey(value?: string): string {
  if (value === undefined) return crypto.randomUUID();
  const key = value.trim();
  if (!key) throw new TypeError("idempotencyKey must not be blank");
  if (key.length > 255) throw new TypeError("idempotencyKey must be at most 255 characters");
  return key;
}
function delay(ms: number, signal?: AbortSignal): Promise<void> {
  if (signal?.aborted) return Promise.reject(signal.reason);
  return new Promise((resolve, reject) => {
    const onAbort = () => {
      clearTimeout(timer);
      reject(signal?.reason);
    };
    const timer = setTimeout(() => {
      signal?.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    signal?.addEventListener("abort", onAbort, { once: true });
  });
}
