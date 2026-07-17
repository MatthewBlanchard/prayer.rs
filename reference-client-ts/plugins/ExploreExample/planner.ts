import type { Prayer } from "@prayer/sdk";

type Snapshot = Awaited<ReturnType<Prayer["state"]>>;
type GalaxyMap = NonNullable<Snapshot["world"]["map"]>;
type GalaxyMapSystem = GalaxyMap["systems"][number];
type GalaxyKnownPoi = GalaxyMap["knownPois"][number];
type RouteSelection = NonNullable<Awaited<ReturnType<Prayer["routes"]>>[number]>;

export type Candidate = { system: GalaxyMapSystem; route: RouteSelection; targetKind: "poi" | "system"; targetId: string; priority: number; timestamp: number };
export type BotRoutes = { botId: string; candidates: Candidate[] };
const missing = (value: number | null | undefined) => value === null || value === undefined;

export function strongholdDistances(systems: readonly GalaxyMapSystem[]): Map<string, number> {
  const known = new Map(systems.map((system) => [system.id, system]));
  const distances = new Map<string, number>();
  const queue = systems.filter((system) => system.isStronghold).map((system) => system.id);
  for (const id of queue) distances.set(id, 0);
  for (let index = 0; index < queue.length; index += 1) {
    const id = queue[index]!;
    const distance = distances.get(id)!;
    for (const next of known.get(id)?.connections ?? [])
      if (known.has(next) && !distances.has(next)) {
        distances.set(next, distance + 1);
        queue.push(next);
      }
  }
  return distances;
}

export function effectiveVisit(system: GalaxyMapSystem, pois: readonly GalaxyKnownPoi[]): number {
  const values = [system.lastEnteredUnix, ...pois.filter((poi) => poi.systemId === system.id).map((poi) => poi.lastVisitedUnix)];
  return values.some((value) => value === null) ? Number.NEGATIVE_INFINITY : Math.min(...(values as number[]));
}

export function isUnexplored(system: GalaxyMapSystem, pois: readonly GalaxyKnownPoi[]): boolean {
  const known = pois.filter((poi) => poi.systemId === system.id);
  return system.firstEnteredUnix === null || !system.poisComplete || known.some((poi) => poi.firstVisitedUnix === null);
}

export function rankCandidates(
  systems: readonly GalaxyMapSystem[],
  pois: readonly GalaxyKnownPoi[],
  routes: ReadonlyMap<string, RouteSelection | null>,
  exclusionHops: number,
  manuallyBlacklisted: ReadonlySet<string> = new Set(),
  manuallyUnblacklisted: ReadonlySet<string> = new Set(),
  hasSurveyScanner = false,
): Candidate[] {
  const distances = strongholdDistances(systems);
  const candidates = systems.flatMap((system): Candidate[] => {
    const automaticallyBlacklisted = (distances.get(system.id) ?? Infinity) <= exclusionHops;
    if (manuallyBlacklisted.has(system.id) || (automaticallyBlacklisted && !manuallyUnblacklisted.has(system.id))) return [];
    const route = routes.get(system.id);
    if (!route) return [];
    const knownPois = pois.filter((poi) => poi.systemId === system.id && system.pois.some((listed) => listed.id === poi.id));
    const result: Candidate[] = knownPois.map((poi) => ({
      system,
      route,
      targetKind: "poi",
      targetId: poi.id,
      priority: missing(poi.firstVisitedUnix) ? 0 : 3,
      timestamp: poi.lastVisitedUnix ?? poi.lastObservedUnix ?? poi.firstVisitedUnix ?? Number.NEGATIVE_INFINITY,
    }));
    if (missing(system.firstEnteredUnix))
      result.push({ system, route, targetKind: "system", targetId: system.id, priority: 0, timestamp: Number.NEGATIVE_INFINITY });
    else if (hasSurveyScanner && missing(system.lastSurveyedUnix))
      result.push({ system, route, targetKind: "system", targetId: system.id, priority: 2, timestamp: system.lastEnteredUnix ?? Number.NEGATIVE_INFINITY });
    return result;
  });
  const priority = Math.min(...candidates.map((candidate) => candidate.priority));
  return candidates
    .filter((candidate) => candidate.priority === priority)
    .sort((a, b) =>
      priority === 3
        ? a.timestamp - b.timestamp || a.route.totalJumps - b.route.totalJumps || a.targetId.localeCompare(b.targetId)
        : a.route.totalJumps - b.route.totalJumps || a.timestamp - b.timestamp || a.targetId.localeCompare(b.targetId),
    );
}

export function allocateDistinct(input: readonly BotRoutes[]): Map<string, Candidate> {
  const result = new Map<string, Candidate>();
  const used = new Set<string>();
  const bots = [...input].sort((a, b) => a.botId.localeCompare(b.botId));
  for (const bot of bots) {
    const candidate = bot.candidates.find((item) => !used.has(item.system.id));
    if (candidate) {
      result.set(bot.botId, candidate);
      used.add(candidate.system.id);
    }
  }
  return result;
}

export function orderedPois(system: GalaxyMapSystem, pois: readonly GalaxyKnownPoi[], seen: ReadonlySet<string> = new Set()): GalaxyKnownPoi[] {
  const order = new Map(system.pois.map((poi, index) => [poi.id, index]));
  return pois
    .filter((poi) => poi.systemId === system.id && order.has(poi.id) && !seen.has(poi.id))
    .sort(
      (a, b) =>
        Number(a.firstVisitedUnix !== null) - Number(b.firstVisitedUnix !== null) ||
        (a.lastVisitedUnix ?? Number.NEGATIVE_INFINITY) - (b.lastVisitedUnix ?? Number.NEGATIVE_INFINITY) ||
        order.get(a.id)! - order.get(b.id)! ||
        a.id.localeCompare(b.id),
    );
}
