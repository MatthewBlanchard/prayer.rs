import { actions, go, Prayer } from "@prayer/sdk";

type FuelStation = { id: string; systemId: string };
type MapSnapshot = { knownPois: readonly { id: string; systemId: string; type: string; hasBase: boolean }[] };
type Markets = Record<string, { sell_orders: Record<string, readonly { quantity: number }[]> }>;
type RouteLookup = (routes: readonly { from: string; to: string }[]) => Promise<readonly ({ cost: number } | null)[]>;

export const FUEL_WATCH_INTERVAL_MS = 1_000;
export const LOW_FUEL_PERCENT = 50;

/** Stations count only when current market knowledge includes sellable fuel. */
export function knownFuelStations(map: MapSnapshot, markets: Markets): FuelStation[] {
  return knownStations(map).filter((poi) => (markets[poi.id]?.sell_orders["fuel"] ?? []).some((order) => order.quantity > 0));
}

function knownStations(map: MapSnapshot): FuelStation[] {
  return map.knownPois
    .filter((poi) => poi.hasBase || /station/i.test(`${poi.type} ${poi.id}`))
    .map(({ id, systemId }) => ({ id, systemId }))
    .sort((left, right) => left.id.localeCompare(right.id));
}

async function nearestReachableStation(originSystem: string, stations: readonly FuelStation[], lookup: RouteLookup): Promise<string | null> {
  const local = stations.find((station) => station.systemId === originSystem);
  if (local) return local.id;
  if (!stations.length) return null;

  const routes = await lookup(stations.map((station) => ({ from: originSystem, to: station.id })));
  return (
    stations
      .map((station, index) => ({ station, route: routes[index] }))
      .filter(
        (candidate): candidate is { station: FuelStation; route: { cost: number } } =>
          candidate.route !== null && candidate.route !== undefined && Number.isFinite(candidate.route.cost),
      )
      .sort((left, right) => left.route.cost - right.route.cost || left.station.id.localeCompare(right.station.id))[0]?.station.id ?? null
  );
}

export async function nearestKnownFuelStation(origin: string, map: MapSnapshot, markets: Markets, lookup: RouteLookup): Promise<string | null> {
  const originSystem = map.knownPois.find((poi) => poi.id === origin)?.systemId ?? origin;
  const withFuel = await nearestReachableStation(originSystem, knownFuelStations(map, markets), lookup);
  return withFuel ?? nearestReachableStation(originSystem, knownStations(map), lookup);
}

export class FuelWatcher {
  private readonly dispatched = new Set<string>();
  private timer: NodeJS.Timeout | undefined;
  private ticking = false;

  constructor(private readonly prayer: Prayer) {}

  start(): void {
    if (this.timer) return;
    void this.tick();
    this.timer = setInterval(() => void this.tick(), FUEL_WATCH_INTERVAL_MS);
  }

  stop(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = undefined;
  }

  async tick(): Promise<void> {
    if (this.ticking) return;
    this.ticking = true;
    try {
      const snapshot = await this.prayer.state();
      const map = snapshot.world.map;
      if (!map) return;
      for (const account of Object.values(snapshot.fleet.bots)) {
        if (account.state.fuel_pct >= LOW_FUEL_PERCENT) {
          this.dispatched.delete(account.id);
          continue;
        }
        if (String(account.connection).toLowerCase() !== "connected" || this.dispatched.has(account.id)) continue;
        const origin = account.state.location.poi_id ?? account.state.location.system_id;
        if (!origin) continue;
        const station = await nearestKnownFuelStation(origin, map, snapshot.world.stationMarkets, (routes) => this.prayer.routes(routes, { safe: true }));
        if (!station) continue;

        this.dispatched.add(account.id);
        try {
          const bot = await this.prayer.bot(account.id);
          await bot.executeActionOverride([go({ poi: station }), actions.refuel()]);
          console.info(`Fuel watcher sent ${account.username ?? account.id} to refuel at ${station}.`);
        } catch (error) {
          this.dispatched.delete(account.id);
          console.warn(`Fuel watcher could not submit an override for ${account.username ?? account.id}:`, error);
        }
      }
    } catch (error) {
      console.warn("Fuel watcher check failed:", error);
    } finally {
      this.ticking = false;
    }
  }
}
