import { actions, type Prayer } from "@prayer/sdk";
import type { JobPlugin, JobRunnerContext, ServerPlugin } from "../../src/plugin-api/server.js";
import type { JobConfig, JobDefinition } from "../../src/plugin-api/shared.js";
import { allocateDistinct, rankCandidates, type Candidate } from "./planner.js";
import { fleetLocation, hasEquippedSurveyScanner, isAvailableForExplore } from "./state.js";

type ExploreConfig = JobConfig & {
  strongholdExclusionHops: number;
  manuallyBlacklistedSystemIds: string[];
  manuallyUnblacklistedSystemIds: string[];
};
const DISPATCH_INTERVAL_MS = 2_000;
type Snapshot = Awaited<ReturnType<Prayer["state"]>>;
type FleetEntry = Snapshot["fleet"]["bots"][string];

export const definition: JobDefinition = {
  kind: "explore",
  title: "Explore",
  description: "Continuously survey reachable systems and visit their known POIs.",
  mode: "continuous",
  fields: [{ name: "strongholdExclusionHops", label: "Stronghold exclusion hops", type: "number", required: true }],
  defaults: { strongholdExclusionHops: 3, manuallyBlacklistedSystemIds: [], manuallyUnblacklistedSystemIds: [] },
  capabilities: ["stop_after_current", "halt_now"],
};

export function validate(config: JobConfig): ExploreConfig {
  const strongholdExclusionHops = config.strongholdExclusionHops ?? definition.defaults.strongholdExclusionHops;
  if (!Number.isInteger(strongholdExclusionHops) || Number(strongholdExclusionHops) < 0)
    throw new Error("strongholdExclusionHops must be a non-negative integer");
  const manuallyBlacklistedSystemIds = Array.isArray(config.manuallyBlacklistedSystemIds) ? config.manuallyBlacklistedSystemIds.map(String) : [];
  const manuallyUnblacklistedSystemIds = Array.isArray(config.manuallyUnblacklistedSystemIds) ? config.manuallyUnblacklistedSystemIds.map(String) : [];
  const { selection: _legacySelection, ...current } = config;
  return {
    ...current,
    strongholdExclusionHops: Number(strongholdExclusionHops),
    manuallyBlacklistedSystemIds,
    manuallyUnblacklistedSystemIds,
  } as ExploreConfig;
}

const abort = (context: JobRunnerContext) => {
  if (context.signal.aborted) throw context.signal.reason;
};
const fleetEntry = (snapshot: Awaited<ReturnType<JobRunnerContext["prayer"]["state"]>>, botId: string) =>
  snapshot.fleet.bots[botId] ?? Object.values(snapshot.fleet.bots).find((entry) => entry.id === botId);
const galaxyMap = (snapshot: Snapshot) => {
  if (!snapshot.world.map) throw new Error("State snapshot omitted the galaxy map");
  return snapshot.world.map;
};
async function executeAssignment(context: JobRunnerContext, botId: string, candidate: Candidate, sequence: number): Promise<void> {
  const target = candidate.targetId;
  const targetSystem = candidate.system.id;
  const key = (action: string) => `${context.run.id}:${botId}:${target}:${sequence}:${action}`;
  await context.setBot(botId, { status: "running", currentWork: `exploring ${target}` });
  try {
    abort(context);
    let snapshot = await context.prayer.state({ signal: context.signal });
    let entry = fleetEntry(snapshot, botId);
    if (!entry) throw new Error(`Bot ${botId} is absent from fleet state`);
    if (fleetLocation(entry).systemId !== targetSystem) {
      await context.update({ phase: `travelling to ${targetSystem}` });
      await context.execute(botId, actions.go({ system: targetSystem }), { idempotencyKey: key("system") });
    }
    abort(context);
    snapshot = await context.prayer.state({ signal: context.signal });
    entry = fleetEntry(snapshot, botId);
    if (!entry) throw new Error(`Bot ${botId} disappeared after arrival`);
    const currentSystem = galaxyMap(snapshot).systems.find((system) => system.id === targetSystem);
    if (hasEquippedSurveyScanner(entry) && currentSystem?.lastSurveyedUnix == null) {
      await context.update({ phase: `surveying ${targetSystem}` });
      abort(context);
      await context.execute(botId, actions.survey(), { idempotencyKey: key("survey") });
      abort(context);
      snapshot = await context.prayer.state({ signal: context.signal });
    }
    if (candidate.targetKind === "poi") {
      await context.update({ phase: `visiting ${target}` });
      await context.execute(botId, actions.go({ poi: target }), { idempotencyKey: key(`poi:${target}`) });
    }
    context.run.summary["completed"] = Number(context.run.summary["completed"] ?? 0) + 1;
    await context.setBot(botId, { status: "idle", currentWork: undefined, lastError: undefined });
  } catch (error) {
    if (context.signal.aborted) throw error;
    const message = error instanceof Error ? error.message : String(error);
    context.run.summary["failed"] = Number(context.run.summary["failed"] ?? 0) + 1;
    await context.setBot(botId, { status: "error", currentWork: undefined, lastError: message });
    await context.update({}, `${botId}: ${message}`, "error");
  }
}

export async function execute(context: JobRunnerContext): Promise<void> {
  const config = validate(context.config);
  let sequence = Number(context.run.artifacts?.["assignmentSequence"] ?? 0);
  const inFlight = new Map<string, { candidate: Candidate; promise: Promise<void> }>();
  while (true) {
    abort(context);
    if (context.run.stopRequestedAt) {
      await Promise.allSettled([...inFlight.values()].map(({ promise }) => promise));
      return;
    }
    await context.update({ phase: "planning" });
    const snapshot = await context.prayer.state({ signal: context.signal });
    abort(context);
    const map = galaxyMap(snapshot);
    const systems = map.systems;
    const ready: Array<{ botId: string; entry: FleetEntry; systemId: string }> = [];
    for (const botId of [...config.botIds].sort()) {
      if (inFlight.has(botId)) continue;
      const entry = fleetEntry(snapshot, botId);
      const location = entry && fleetLocation(entry);
      if (!isAvailableForExplore(entry) || !location?.systemId) {
        await context.setBot(botId, {
          status: "idle",
          currentWork: entry?.in_transit ? "in transit" : entry?.connection !== "Connected" ? "disconnected" : "busy",
        });
        continue;
      }
      await context.setBot(botId, { status: "idle", currentWork: undefined, lastError: undefined });
      ready.push({ botId, entry: entry!, systemId: location.systemId });
    }
    const routeInputs = ready.flatMap((bot) => systems.filter((system) => system.id !== bot.systemId).map((system) => ({ from: bot.systemId, to: system.id })));
    abort(context);
    const routes = routeInputs.length ? await context.prayer.routes(routeInputs, { safe: true }, { signal: context.signal }) : [];
    const reservedSystems = new Set([...inFlight.values()].map(({ candidate }) => candidate.system.id));
    const plans = ready.map((bot) => {
      const map = new Map<string, NonNullable<(typeof routes)[number]> | null>();
      routeInputs.forEach((query, index) => {
        if (query.from === bot.systemId) map.set(query.to, routes[index] ?? null);
      });
      map.set(bot.systemId, {
        cost: 0,
        from: bot.systemId,
        fromSystem: bot.systemId,
        hops: [],
        safe: true,
        to: bot.systemId,
        toSystem: bot.systemId,
        totalJumps: 0,
      });
      return {
        botId: bot.botId,
        candidates: rankCandidates(
          systems,
          galaxyMap(snapshot).knownPois,
          map,
          config.strongholdExclusionHops,
          new Set(config.manuallyBlacklistedSystemIds),
          new Set(config.manuallyUnblacklistedSystemIds),
          hasEquippedSurveyScanner(bot.entry),
        ).filter((candidate) => !reservedSystems.has(candidate.system.id)),
      };
    });
    const assignments = allocateDistinct(plans);
    if (!assignments.size) {
      await context.update({ phase: inFlight.size ? "dispatching" : "idle: no reachable targets" });
      await context.delay(DISPATCH_INTERVAL_MS);
      continue;
    }
    sequence += 1;
    await context.update({
      artifacts: {
        ...context.run.artifacts,
        assignmentSequence: sequence,
        assignments: Object.fromEntries([...assignments].map(([id, value]) => [id, value.system.id])),
      },
    });
    for (const [botId, candidate] of assignments) {
      const promise = executeAssignment(context, botId, candidate, sequence)
        .catch((error) => {
          if (!context.signal.aborted) throw error;
        })
        .finally(() => {
          inFlight.delete(botId);
        });
      inFlight.set(botId, { candidate, promise });
    }
    await context.delay(DISPATCH_INTERVAL_MS);
  }
}

const job: JobPlugin = { definition, validate, execute };
const plugin: ServerPlugin = { jobs: [job] };
export default plugin;
