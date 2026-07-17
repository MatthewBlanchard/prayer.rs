import type { Prayer } from "@prayer/sdk";
type FleetEntry = Awaited<ReturnType<Prayer["state"]>>["fleet"]["bots"][string];

export function fleetLocation(entry: FleetEntry): { systemId?: string; poiId?: string } {
  return {
    systemId: entry.state.location.system_id ?? undefined,
    poiId: entry.state.location.poi_id ?? undefined,
  };
}

export function hasEquippedSurveyScanner(entry: FleetEntry): boolean {
  return entry.state.modules.some((module) => module.type_id?.includes("survey_scanner"));
}

export function isAvailableForExplore(entry: FleetEntry | undefined): entry is FleetEntry {
  return Boolean(
    entry &&
      entry.connection === "Connected" &&
      !entry.in_transit &&
      entry.script_execution?.state !== "running" &&
      !entry.active_route,
  );
}
