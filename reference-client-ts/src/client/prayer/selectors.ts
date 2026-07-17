import type { FleetEntry } from "./worldSelectors.js";

export type BotView = {
  botId: string;
  name: string;
  connection: "connected" | "disconnected";
  stateVersion: number;
  observedAt: string | null;
  systemId: string | null;
  poiId: string | null;
  credits: number | null;
  fuel: number;
  maxFuel: number;
  hull: number | null;
  maxHull: number | null;
  cargoUsed: number;
  cargoCapacity: number;
  cargo: Record<string, number>;
  passengerBerths: number;
  inBattle: boolean;
  combatStance: string | null;
  combatTarget: string | null;
  scriptExecution: FleetEntry["script_execution"];
  activeRoute: FleetEntry["active_route"];
  inTransit: boolean;
  transitDestSystem: string | null;
  transitDestPoi: string | null;
  observedPlayers: Record<string, ObservedPlayerView>;
};

export type ObservedPlayerView = {
  playerId: string;
  username: string;
  offline: boolean;
};

function connection(value: FleetEntry["connection"]): BotView["connection"] {
  return value === "Connected" ? "connected" : "disconnected";
}

export function selectBotView(snapshot: FleetEntry): BotView {
  const state = snapshot.state;
  return {
    botId: snapshot.id,
    name: snapshot.username?.trim() || snapshot.id,
    connection: connection(snapshot.connection),
    stateVersion: snapshot.version,
    observedAt: snapshot.observed_at ?? null,
    systemId: state.location.system_id ?? null,
    poiId: state.location.poi_id ?? null,
    credits: state.player.credits ?? null,
    fuel: state.fuel,
    maxFuel: state.max_fuel,
    hull: state.ship.hull ?? null,
    maxHull: state.ship.max_hull ?? null,
    cargoUsed: state.cargo_used,
    cargoCapacity: state.cargo_capacity,
    cargo: state.cargo,
    passengerBerths: state.passengers.economy_berths.max + state.passengers.business_berths.max + state.passengers.first_berths.max,
    inBattle: state.in_battle,
    combatStance: state.combat_stance ?? null,
    combatTarget: state.combat_target ?? null,
    scriptExecution: snapshot.script_execution ?? null,
    activeRoute: snapshot.active_route ?? null,
    inTransit: snapshot.in_transit ?? false,
    transitDestSystem: snapshot.transit_dest_system ?? null,
    transitDestPoi: snapshot.transit_dest_poi ?? null,
    observedPlayers: Object.fromEntries(
      Object.entries(state.observation_nearby ?? {}).map(([key, value]) => {
        return [
          key,
          {
            playerId: value.player_id ?? key,
            username: value.username ?? key,
            offline: value.offline ?? false,
          },
        ];
      }),
    ),
  };
}
