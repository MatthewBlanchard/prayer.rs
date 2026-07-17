import type { ActiveRoute, ScriptExecution } from "@prayer/sdk/types";

export type RuntimeHostStatus = {
  isHalted: boolean;
  isFinished: boolean;
  hasActiveCommand: boolean;
  currentScriptLine: number | null;
  currentScript: string | null;
  resultMessage: string | null;
  activeFrame: null;
};

export type RuntimeStatus = {
  sessionId: string;
  username: string | null;
  snapshot: RuntimeHostStatus;
  latestSystem: string | null;
  latestPoi: string | null;
  docked: boolean | null;
  fuel: number;
  maxFuel: number;
  fuelPercent: number;
  fuelPerJump: number | null;
  hull: number | null;
  maxHull: number | null;
  cargoUsed: number;
  cargoCapacity: number;
  cargo: Record<string, number>;
  passengers: unknown;
  credits: number | null;
  lastUpdatedUtc: string;
  inTransit: boolean;
  transitDestSystem: string | null;
  transitDestPoi: string | null;
  homeBase: string | null;
  homePoi: string | null;
  scriptExecution: ScriptExecution | null;
  scriptRunning: boolean;
  inBattle: boolean;
  combatStance: string | null;
  combatTarget: string | null;
  activeRoute: ActiveRoute | null;
  stateVersion: number;
};
