import type {
  FleetSnapshot,
  GalaxyCatalog,
  StationMarketData,
  StateVersions,
  StationMarketDelta,
  WorldState,
} from "./generated/types.js";

type StationMarkets = Record<string, StationMarketData>;

/** A fully resolved state response after conditional updates have been merged. */
export interface StateSnapshot {
  versions: StateVersions;
  fleet: FleetSnapshot;
  world: WorldState;
  catalog: GalaxyCatalog;
}

/** The conditional world payload accepted by the SDK cache merger. */
export type WorldStateUpdate = Partial<WorldState> & {
  stationMarkets?: StationMarkets | null;
  stationMarketDelta?: StationMarketDelta | null;
};

/** Higher-level navigation target accepted by the action helper. */
export type GoTarget =
  | { kind: "identifier" | "system" | "poi"; value: string }
  | { kind: "coordinate"; value: { x: number; y: number } };
