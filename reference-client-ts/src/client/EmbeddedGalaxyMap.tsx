import { lazy, Suspense } from "react";
import type { SessionState } from "./SessionsPanel.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectGalaxyExploration, selectGalaxyMap } from "./prayer/worldSelectors.js";
import { useSquads } from "./prayer/useSquads.js";

const noSessions: SessionState[] = [];
const GalaxyMapViewport = lazy(async () => {
  const module = await import("./GalaxyMap.js");
  return { default: module.GalaxyMapViewport };
});

export type EmbeddedGalaxyMapProps = {
  sessions?: SessionState[];
  selectedSystemId?: string;
  highlightedSystemIds?: string[];
  dimmedSystemIds?: string[];
  selectablePoiIds?: string[];
  onSystemClick?: (systemId: string) => void;
  onSelectSystem?: (systemId: string) => void;
};

export default function EmbeddedGalaxyMap({
  sessions = noSessions,
  selectedSystemId,
  highlightedSystemIds,
  dimmedSystemIds,
  selectablePoiIds,
  onSystemClick,
  onSelectSystem,
}: EmbeddedGalaxyMapProps) {
  const prayer = usePrayer();
  const squads = useSquads();
  const map = selectGalaxyMap(prayer.galaxyMap);
  const graphLoading = !prayer.error && (prayer.connection === "connecting" || !map || map.systems.length === 0);
  return (
    <Suspense fallback={<div className="galaxy-map-empty">Loading galaxy graph…</div>}>
      <GalaxyMapViewport
        sessions={sessions}
        map={map}
        exploration={selectGalaxyExploration(prayer.galaxyExploration)}
        loading={graphLoading}
        error={prayer.error?.message ?? null}
        variant="embedded"
        squads={squads}
        selectedSystemId={selectedSystemId}
        highlightedSystemIds={highlightedSystemIds}
        dimmedSystemIds={dimmedSystemIds}
        selectablePoiIds={selectablePoiIds}
        onSystemClick={onSystemClick}
        onSelectSystem={onSelectSystem}
      />
    </Suspense>
  );
}
