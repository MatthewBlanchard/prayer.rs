import { Prayer, PrayerApiError, PrayerConnectionError, type Bot, type StateSnapshot } from "@prayer/sdk";
import type { GalaxyMap, GalaxyResources, FacilityResponse, PoiFacilitiesSnapshot, StationMarkets, StorageByOwner } from "@prayer/sdk/types";
import type {
  AgentSightingWire,
  ChatMessageWire,
  FactionSnapshotWire,
  FleetEntry,
  GalaxyCatalog,
  GalaxyWildlife,
  PassengerBoardWire,
  SalvageStateWire,
} from "./worldSelectors.js";
import type { GalaxyExplorationData } from "../api.js";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { fetchConfig } from "../api/config.js";
import { selectBotView, type BotView } from "./selectors.js";

const POLL_MS = 2_000;
const INITIAL_STATE_TIMEOUT_MS = 120_000;

export type PrayerClientError = {
  kind: "api" | "connection" | "unexpected";
  code: string;
  message: string;
  retryable: boolean;
};

type PrayerContextValue = {
  connection: "connecting" | "connected" | "unavailable";
  fleet: FleetEntry[];
  galaxyMap: GalaxyMap | null;
  galaxyResources: GalaxyResources | null;
  galaxyExploration: GalaxyExplorationData | null;
  galaxyWildlife: GalaxyWildlife | null;
  catalog: GalaxyCatalog | null;
  stationMarkets: StationMarkets;
  storageByPlayer: StorageByOwner;
  factionStorageByFactionPoi: StorageByOwner;
  facilitiesByPoi: Record<string, PoiFacilitiesSnapshot>;
  ownedFacilitiesByPlayer: Record<string, FacilityResponse>;
  ownedFacilitiesByFaction: Record<string, FacilityResponse>;
  stationPassengers: Record<string, PassengerBoardWire>;
  salvageByPoi: Record<string, SalvageStateWire>;
  agentSightings: Record<string, AgentSightingWire>;
  chatMessagesBySession: Record<string, ChatMessageWire[]>;
  factionBySession: Record<string, FactionSnapshotWire>;
  bots: BotView[];
  error: PrayerClientError | null;
  refresh: () => Promise<void>;
  refreshKnowledge: () => Promise<void>;
  bot: (selector: string) => Promise<Bot>;
};

const PrayerContext = createContext<PrayerContextValue | null>(null);

function normalizeError(error: unknown): PrayerClientError {
  if (error instanceof PrayerApiError) return { kind: "api", code: error.code, message: error.message, retryable: error.retryable };
  if (error instanceof PrayerConnectionError) return { kind: "connection", code: "connection_error", message: error.message, retryable: true };
  return { kind: "unexpected", code: "unexpected", message: error instanceof Error ? error.message : String(error), retryable: false };
}

export function PrayerProvider({ children }: { children: ReactNode }) {
  const clientRef = useRef<Prayer | null>(null);
  const pollingRef = useRef(false);
  const [connection, setConnection] = useState<PrayerContextValue["connection"]>("connecting");
  const [snapshot, setSnapshot] = useState<StateSnapshot | null>(null);
  const [error, setError] = useState<PrayerClientError | null>(null);

  const refresh = useCallback(async () => {
    const client = clientRef.current;
    if (!client || pollingRef.current) return;
    pollingRef.current = true;
    try {
      const snapshot = await client.state({ timeoutMs: INITIAL_STATE_TIMEOUT_MS });
      const map = snapshot.world.map;
      if (!map) throw new Error("Initial state snapshot omitted the galaxy map");
      setSnapshot(snapshot);
      setConnection("connected");
      setError(null);
    } catch (cause) {
      setConnection("unavailable");
      setError(normalizeError(cause));
    } finally {
      pollingRef.current = false;
    }
  }, []);

  const refreshKnowledge = refresh;

  const fleet = Object.values(snapshot?.fleet.bots ?? {});
  const galaxyMap = snapshot?.world.map ?? null;
  const galaxyResources = snapshot?.world.resources ?? null;
  const galaxyWildlife = snapshot?.world.wildlife ?? null;
  const catalog = snapshot?.catalog ?? null;
  const stationMarkets = snapshot?.world.stationMarkets ?? {};
  const storageByPlayer = snapshot?.world.storageByPlayer ?? {};
  const factionStorageByFactionPoi = snapshot?.world.factionStorageByFactionPoi ?? {};
  const facilitiesByPoi = snapshot?.world.facilitiesByPoi ?? {};
  const ownedFacilitiesByPlayer = snapshot?.world.ownedFacilitiesByPlayer ?? {};
  const ownedFacilitiesByFaction = snapshot?.world.ownedFacilitiesByFaction ?? {};
  const stationPassengers = snapshot?.world.stationPassengers ?? {};
  const salvageByPoi = snapshot?.world.salvageByPoi ?? {};
  const agentSightings = snapshot?.world.agentSightings ?? {};
  const chatMessagesBySession = snapshot?.world.chatMessagesBySession ?? {};
  const factionBySession = snapshot?.world.factionBySession ?? {};
  const galaxyExploration: GalaxyExplorationData | null = galaxyMap
    ? {
        exploredSystems: galaxyMap.systems.filter((system) => system.firstEnteredUnix != null).map((system) => system.id),
        surveyedSystems: galaxyMap.systems.filter((system) => system.lastSurveyedUnix != null).map((system) => system.id),
        visitedPois: galaxyMap.knownPois.filter((poi) => poi.firstVisitedUnix != null).map((poi) => poi.id),
      }
    : null;

  useEffect(() => {
    let stopped = false;
    let timer: number | undefined;
    void (async () => {
      const config = await fetchConfig();
      const baseUrl = config?.prayerApiUrl?.trim() || window.location.origin;
      const connectAndRefresh = async () => {
        try {
          if (!clientRef.current) {
            clientRef.current = await Prayer.connect({ baseUrl });
          }
          if (!stopped) await refresh();
        } catch (cause) {
          clientRef.current = null;
          if (!stopped) {
            const normalized = normalizeError(cause);
            setConnection("unavailable");
            setError({ ...normalized, message: `${normalized.message} (${baseUrl})` });
          }
        }
      };
      await connectAndRefresh();
      if (!stopped) timer = window.setInterval(() => void connectAndRefresh(), POLL_MS);
    })();
    return () => {
      stopped = true;
      if (timer !== undefined) window.clearInterval(timer);
    };
  }, [refresh]);

  const value = useMemo<PrayerContextValue>(
    () => ({
      connection,
      fleet,
      galaxyMap,
      galaxyResources,
      galaxyExploration,
      galaxyWildlife,
      catalog,
      stationMarkets,
      storageByPlayer,
      factionStorageByFactionPoi,
      facilitiesByPoi,
      ownedFacilitiesByPlayer,
      ownedFacilitiesByFaction,
      stationPassengers,
      salvageByPoi,
      agentSightings,
      chatMessagesBySession,
      factionBySession,
      bots: fleet.map(selectBotView),
      error,
      refresh,
      refreshKnowledge,
      async bot(selector) {
        const client = clientRef.current;
        if (!client) throw new PrayerConnectionError("Prayer API is not connected");
        return client.bot(selector);
      },
    }),
    [
      agentSightings,
      catalog,
      chatMessagesBySession,
      connection,
      error,
      facilitiesByPoi,
      factionBySession,
      factionStorageByFactionPoi,
      fleet,
      galaxyExploration,
      galaxyMap,
      galaxyResources,
      galaxyWildlife,
      ownedFacilitiesByFaction,
      ownedFacilitiesByPlayer,
      refresh,
      refreshKnowledge,
      salvageByPoi,
      stationMarkets,
      stationPassengers,
      storageByPlayer,
    ],
  );

  return <PrayerContext.Provider value={value}>{children}</PrayerContext.Provider>;
}

export function usePrayer(): PrayerContextValue {
  const value = useContext(PrayerContext);
  if (!value) throw new Error("usePrayer must be used within PrayerProvider");
  return value;
}
