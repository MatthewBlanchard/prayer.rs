import type { AuthoritativeRoute } from "@prayer/sdk";
import { fetchRoutes } from "./api.js";

export type StationPoi = { id: string; systemId: string; name?: string | null; type?: string | null; hasBase?: boolean };
type RouteLookup = (routes: readonly { from: string; to: string }[], safe?: boolean) => Promise<Array<AuthoritativeRoute | null>>;

export function isStationPoi(poi: StationPoi): boolean {
  return poi.hasBase === true || /station/i.test(`${poi.type} ${poi.id} ${poi.name}`);
}

/** Resolve the nearest known station to one explicit system or POI as a concrete POI ID. */
export async function findNearestStationPoi(
  map: { knownPois: readonly StationPoi[] } | null,
  origin: string,
  lookup: RouteLookup = fetchRoutes,
): Promise<string | null> {
  if (!map || !origin) return null;
  const originSystem = map.knownPois.find((poi) => poi.id === origin)?.systemId ?? origin;
  const stations = map.knownPois.filter(isStationPoi).sort((left, right) => left.id.localeCompare(right.id));
  const local = stations.find((station) => station.systemId === originSystem);
  if (local) return local.id;
  if (!stations.length) return null;

  const routes = await lookup(
    stations.map((station) => ({ from: originSystem, to: station.id })),
    true,
  );
  const reachable: Array<{ id: string; cost: number }> = [];
  routes.forEach((route, index) => {
    const station = stations[index];
    if (!route || !station || !Number.isFinite(route.cost)) return;
    reachable.push({ id: station.id, cost: route.cost });
  });
  reachable.sort((left, right) => left.cost - right.cost || left.id.localeCompare(right.id));
  return reachable[0]?.id ?? null;
}
