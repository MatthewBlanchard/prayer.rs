import { useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import type { GalaxyMap } from "@prayer/sdk/types";
import type { SessionLocation, SessionState } from "../SessionsPanel.js";
import type { BotView } from "./selectors.js";
import { projectRunningScript, projectSessionLocation } from "./sessionProjection.js";

function emptyLocation(): SessionLocation {
  return {
    system: null,
    poi: null,
    inTransit: false,
    transitDestSystem: null,
    transitDestPoi: null,
    activeRouteDestSystem: null,
    activeRouteDestPoi: null,
    activeRouteHops: [],
  };
}

function hydrateSession(bot: BotView, existing?: SessionState): SessionState {
  const sessionHandle = bot.name?.trim() || bot.botId;
  const base: SessionState = existing ?? {
    sessionHandle,
    botId: bot.botId,
    username: null,
    connected: false,
    credits: null,
    fuel: null,
    maxFuel: null,
    hull: null,
    maxHull: null,
    cargoUsed: null,
    cargoCapacity: null,
    passengerBerths: null,
    cargo: {},
    passengersAboard: [],
    inBattle: false,
    combatStance: null,
    combatTarget: null,
    battleStartedAt: null,
    runningScript: null,
    location: emptyLocation(),
    observedPlayers: {},
  };
  return {
    ...base,
    sessionHandle,
    botId: bot.botId,
    username: bot.name,
    connected: bot.connection === "connected",
    credits: bot.credits,
    fuel: bot.fuel,
    maxFuel: bot.maxFuel,
    hull: bot.hull,
    maxHull: bot.maxHull,
    cargoUsed: bot.cargoUsed,
    cargoCapacity: bot.cargoCapacity,
    cargo: bot.cargo,
    passengerBerths: bot.passengerBerths,
    inBattle: bot.inBattle,
    combatStance: bot.combatStance,
    combatTarget: bot.combatTarget,
    runningScript: projectRunningScript(bot),
    location: projectSessionLocation(bot),
    observedPlayers: bot.observedPlayers,
  };
}

export function projectFleetSessions(bots: BotView[], previous: SessionState[]): SessionState[] {
  return bots.map((bot) => {
    const sessionHandle = bot.name?.trim() || bot.botId;
    const existing = previous.find((session) => session.botId === bot.botId || session.sessionHandle === sessionHandle);
    return hydrateSession(bot, existing);
  });
}

function galaxySystemEmpires(map: GalaxyMap | null): Record<string, string> {
  if (!map) return {};
  const entries: Array<[string, string]> = [];
  for (const system of map.systems) {
    const empire = system.empire?.trim() ?? "";
    if (empire) entries.push([system.id, empire], [system.id.toLowerCase(), empire]);
  }
  return Object.fromEntries(entries);
}

export function useFleetSessions(
  bots: BotView[],
  galaxyMap: GalaxyMap | null,
): {
  sessions: SessionState[];
  setSessions: Dispatch<SetStateAction<SessionState[]>>;
  systemEmpires: Record<string, string>;
} {
  const [sessions, setSessions] = useState<SessionState[]>([]);
  const systemEmpires = useMemo(() => galaxySystemEmpires(galaxyMap), [galaxyMap]);

  useEffect(() => {
    setSessions((previous) => projectFleetSessions(bots, previous));
  }, [bots]);

  return { sessions, setSessions, systemEmpires };
}
