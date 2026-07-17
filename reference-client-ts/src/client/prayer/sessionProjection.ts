import type { BotView } from "./selectors.js";
import type { RunningScript, SessionLocation } from "../SessionsPanel.js";

export function projectSessionLocation(bot: BotView): SessionLocation {
  return {
    system: bot.systemId,
    poi: bot.poiId,
    inTransit: bot.inTransit,
    transitDestSystem: bot.transitDestSystem,
    transitDestPoi: bot.transitDestPoi,
    activeRouteDestSystem: bot.activeRoute?.targetSystem ?? null,
    activeRouteDestPoi: bot.activeRoute?.targetPoi ?? null,
    activeRouteHops: bot.activeRoute?.hops ?? [],
  };
}

export function projectRunningScript(bot: BotView): RunningScript | null {
  const execution = bot.scriptExecution;
  if (!execution?.script) return null;
  return {
    script: execution.script,
    currentLine: execution.currentLine ?? execution.lastLine ?? null,
    isRunning: execution.state === "running",
    frameKind: execution.frameKind ?? "main",
    frameName: execution.frameName ?? null,
  };
}
