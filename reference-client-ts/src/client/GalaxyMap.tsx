import { memo, useEffect, useMemo, useRef, useState } from "react";
import { GalaxyMapSystem, type GalaxyExplorationData, type GalaxyMapData } from "./api.js";
import { fetchRoutes } from "./api/routing.js";
import { SessionState } from "./SessionsPanel.js";
import type { Squad } from "../shared/types.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { useSquads } from "./prayer/useSquads.js";
import { selectGalaxyExploration, selectGalaxyMap } from "./prayer/worldSelectors.js";
import { activeRoutePath } from "./galaxyRoute.js";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

type GalaxyMapProps = {
  sessions: SessionState[];
};

export type GalaxyMapViewportProps = GalaxyMapProps & {
  map: GalaxyMapData | null;
  exploration: GalaxyExplorationData | null;
  loading?: boolean;
  error?: string | null;
  variant?: "panel" | "embedded" | "popout";
  squads?: Squad[];
  selectedSystemId?: string;
  highlightedSystemIds?: string[];
  dimmedSystemIds?: string[];
  selectablePoiIds?: string[];
  onSystemClick?: (systemId: string) => void;
  onSelectSystem?: (systemId: string) => void;
};

type PositionedSystem = GalaxyMapSystem & {
  sx: number;
  sy: number;
};

const WIDTH = 1000;
const HEIGHT = 720;
const PAD = 64;
const HOLOGRAM_REVEAL_DURATION_MS = 1800;
const HOLOGRAM_REVEAL_DELAY_MS = 385;
const HOLOGRAM_WAVE_FEATHER = 56;
const HOLOGRAM_WAVE_LIFT = 18;
const GRID_FOG_DENSITY = 0.00216;
const GALAXY_BACKGROUND_COLOR = 0x020b0e;
const DEFAULT_ORTHO_ZOOM = 2;
const DEFAULT_PERSPECTIVE_POSITION = new THREE.Vector3(0, 280, 380);
const MIN_PERSPECTIVE_DISTANCE = DEFAULT_PERSPECTIVE_POSITION.length();
const MAP_ZOOM_RANGE = 0.25;

type SystemHoverInfo = {
  x: number;
  y: number;
  name: string;
  color: string;
  bots: Array<{ name: string; color: string }>;
};

function systemElevation(id: string): number {
  let hash = 0;
  for (let i = 0; i < id.length; i++) hash = ((hash << 5) - hash + id.charCodeAt(i)) | 0;
  const magnitude = 18 + Math.abs(hash % 58);
  return hash % 2 === 0 ? magnitude : -magnitude;
}

type SystemVisitLevel = "unknown" | "visited" | "complete";

const VISIT_LEVEL_COLORS: Record<SystemVisitLevel, string> = {
  unknown: "#20292b",
  visited: "#a6a6a6",
  complete: "#f0f0f0",
};

const VISIT_LEVEL_MIX: Record<SystemVisitLevel, number> = {
  unknown: 0.38,
  visited: 0.68,
  complete: 1,
};

const EMPIRE_COLORS = {
  frontier: "#4fb56a",
  solarian: "#e8cf62",
  voidborn: "#9b6ae8",
  crimson: "#e0555f",
  trade: "#5f9fed",
};
const STRONGHOLD_COLOR = "#ff8a2b";

function hasCoordinates(system: GalaxyMapSystem): system is GalaxyMapSystem & { x: number; y: number } {
  return typeof system.x === "number" && Number.isFinite(system.x) && typeof system.y === "number" && Number.isFinite(system.y);
}

function fitSystems(systems: GalaxyMapSystem[]): PositionedSystem[] {
  const drawableWidth = WIDTH - PAD * 2;
  const drawableHeight = HEIGHT - PAD * 2;
  const withCoords = systems.filter(hasCoordinates);

  if (withCoords.length === 0) {
    return systems.map((system, i) => {
      const angle = (Math.PI * 2 * i) / Math.max(systems.length, 1);
      const radius = Math.min(drawableWidth, drawableHeight) * 0.36;
      return {
        ...system,
        sx: WIDTH / 2 + Math.cos(angle) * radius,
        sy: HEIGHT / 2 + Math.sin(angle) * radius,
      };
    });
  }

  const xs = withCoords.map((system) => system.x);
  const ys = withCoords.map((system) => system.y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  const spanX = Math.max(maxX - minX, 1);
  const spanY = Math.max(maxY - minY, 1);
  const scale = Math.min(drawableWidth / spanX, drawableHeight / spanY);
  const usedWidth = spanX * scale;
  const usedHeight = spanY * scale;
  const offsetX = PAD + (drawableWidth - usedWidth) / 2;
  const offsetY = PAD + (drawableHeight - usedHeight) / 2;

  return systems.map((system, i) => {
    if (hasCoordinates(system)) {
      return {
        ...system,
        sx: offsetX + (system.x - minX) * scale,
        sy: offsetY + (system.y - minY) * scale,
      };
    }

    const angle = (Math.PI * 2 * i) / Math.max(systems.length, 1);
    return {
      ...system,
      sx: WIDTH / 2 + Math.cos(angle) * Math.min(drawableWidth, drawableHeight) * 0.42,
      sy: HEIGHT / 2 + Math.sin(angle) * Math.min(drawableWidth, drawableHeight) * 0.42,
    };
  });
}

function shortLabel(id: string): string {
  return id.replace(/^sys(?:tem)?[_-]/i, "").replace(/_/g, " ");
}

function systemLabel(system: GalaxyMapSystem): string {
  return system.name?.trim() || shortLabel(system.id);
}

function normalizeEmpire(value: string | undefined): keyof typeof EMPIRE_COLORS | null {
  const normalized =
    value
      ?.trim()
      .toLowerCase()
      .replace(/[\s-]+/g, "_") ?? "";
  if (!normalized) return null;
  if (normalized.includes("frontier") || normalized.includes("nebula")) return "frontier";
  if (normalized.includes("solarian") || normalized === "sol") return "solarian";
  if (normalized.includes("voidborn") || normalized.includes("void")) return "voidborn";
  if (normalized.includes("crimson")) return "crimson";
  if (normalized.includes("trade") || normalized.includes("outerrim") || normalized.includes("outer_rim") || normalized.includes("outer")) {
    return "trade";
  }
  return null;
}

function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace("#", "");
  return [parseInt(clean.slice(0, 2), 16), parseInt(clean.slice(2, 4), 16), parseInt(clean.slice(4, 6), 16)];
}

function rgbToHex([r, g, b]: [number, number, number]): string {
  return `#${[r, g, b].map((part) => Math.round(part).toString(16).padStart(2, "0")).join("")}`;
}

function mixColor(base: string, tint: string, amount: number): string {
  const [br, bg, bb] = hexToRgb(base);
  const [tr, tg, tb] = hexToRgb(tint);
  return rgbToHex([br + (tr - br) * amount, bg + (tg - bg) * amount, bb + (tb - bb) * amount]);
}

function useStableSnapshot<T>(value: T, key = JSON.stringify(value) ?? "undefined"): T {
  const stable = useRef<{ key: string; value: T }>({ key, value });
  if (stable.current.key !== key) stable.current = { key, value };
  return stable.current.value;
}

function galaxySessionRenderKey(sessions: SessionState[], squadColors?: Map<string, string>): string {
  return sessions
    .map(
      (session) =>
        `${session.botId}:${session.sessionHandle}:${session.username ?? ""}:${session.location.system ?? ""}:${session.location.activeRouteHops.join(",")}:${squadColors?.get(session.sessionHandle) ?? ""}`,
    )
    .sort()
    .join("|");
}

function unitMarkerKey(session: SessionState): string {
  return `${session.botId || session.sessionHandle}:${session.sessionHandle}`;
}

function stableNoise(key: string, salt: number): number {
  let hash = 2166136261 ^ salt;
  for (let index = 0; index < key.length; index += 1) {
    hash ^= key.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0) / 0xffffffff;
}

function enableInstancedReveal(material: THREE.MeshBasicMaterial, billboard = false) {
  const waveRadius = { value: 0 };
  const revealFeather = { value: HOLOGRAM_WAVE_FEATHER };
  material.onBeforeCompile = (shader) => {
    const tintVertexDeclarations = billboard ? "\nattribute vec3 instanceTint;\nvarying vec3 vInstanceTint;" : "";
    const tintVertexAssignment = billboard ? "\nvInstanceTint = instanceTint;" : "";
    const tintFragmentDeclaration = billboard ? "\nvarying vec3 vInstanceTint;" : "";
    const tintFragmentAssignment = billboard ? "\ndiffuseColor.rgb *= vInstanceTint;" : "";
    shader.uniforms.waveRadius = waveRadius;
    shader.uniforms.revealFeather = revealFeather;
    shader.vertexShader = shader.vertexShader
      .replace(
        "#include <common>",
        `#include <common>\nattribute float baseOpacity;\nattribute float revealDistance;\nuniform float waveRadius;\nuniform float revealFeather;\nvarying float vInstanceOpacity;\nvarying float vRevealProgress;${tintVertexDeclarations}`,
      )
      .replace(
        "#include <begin_vertex>",
        `#include <begin_vertex>\nvRevealProgress = smoothstep( 0.0, revealFeather, waveRadius - revealDistance );\nvInstanceOpacity = baseOpacity * vRevealProgress;${tintVertexAssignment}`,
      );
    if (billboard) {
      shader.vertexShader = shader.vertexShader.replace(
        "#include <project_vertex>",
        `vec4 instanceOrigin = instanceMatrix * vec4( 0.0, 0.0, 0.0, 1.0 );
instanceOrigin.y += sin( vRevealProgress * PI ) * ${HOLOGRAM_WAVE_LIFT.toFixed(1)};
vec4 mvPosition = modelViewMatrix * instanceOrigin;
float instanceScaleX = length( vec3( instanceMatrix[ 0 ] ) );
float instanceScaleY = length( vec3( instanceMatrix[ 1 ] ) );
mvPosition.xy += transformed.xy * vec2( instanceScaleX, instanceScaleY );
gl_Position = projectionMatrix * mvPosition;`,
      );
    }
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>\nvarying float vInstanceOpacity;${tintFragmentDeclaration}`)
      .replace("vec4 diffuseColor = vec4( diffuse, opacity );", `vec4 diffuseColor = vec4( diffuse, opacity * vInstanceOpacity );${tintFragmentAssignment}`);
  };
  material.customProgramCacheKey = () => `galaxy-instanced-reveal-${billboard ? "billboard" : "surface"}`;
  return waveRadius;
}

function enableVertexReveal(material: THREE.LineBasicMaterial | THREE.LineDashedMaterial) {
  const waveRadius = { value: 0 };
  const revealFeather = { value: HOLOGRAM_WAVE_FEATHER };
  material.onBeforeCompile = (shader) => {
    shader.uniforms.waveRadius = waveRadius;
    shader.uniforms.revealFeather = revealFeather;
    shader.vertexShader = shader.vertexShader
      .replace(
        "#include <common>",
        "#include <common>\nattribute float baseOpacity;\nattribute float revealDistance;\nattribute float liftWeight;\nuniform float waveRadius;\nuniform float revealFeather;\nvarying float vVertexOpacity;",
      )
      .replace(
        "#include <begin_vertex>",
        `#include <begin_vertex>
float revealProgress = smoothstep( 0.0, revealFeather, waveRadius - revealDistance );
vVertexOpacity = baseOpacity * revealProgress;
transformed.y += liftWeight * sin( revealProgress * PI ) * ${HOLOGRAM_WAVE_LIFT.toFixed(1)};`,
      );
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying float vVertexOpacity;")
      .replace("vec4 diffuseColor = vec4( diffuse, opacity );", "vec4 diffuseColor = vec4( diffuse, opacity * vVertexOpacity );");
  };
  material.customProgramCacheKey = () => "galaxy-line-vertex-reveal";
  return waveRadius;
}

function GalaxyMapViewportComponent({
  sessions,
  map,
  exploration: explorationSnapshot,
  loading = false,
  error = null,
  variant = "panel",
  squads = [],
  selectedSystemId,
  highlightedSystemIds = [],
  dimmedSystemIds = [],
  selectablePoiIds,
  onSystemClick,
  onSelectSystem,
}: GalaxyMapViewportProps) {
  const embedded = variant === "embedded";
  const threeCanvasRef = useRef<HTMLCanvasElement>(null);
  const systemCanvasRef = useRef<HTMLCanvasElement>(null);
  const systemPoiLabelsRef = useRef<HTMLDivElement>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | THREE.OrthographicCamera | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);
  const cameraInteractionRef = useRef(false);
  const hoveredSystemIdRef = useRef<string | null>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const latestOccupancyKeyRef = useRef("");
  const updateDynamicSceneRef = useRef<(() => void) | null>(null);
  const mapRevealRef = useRef<{ key: string; startedAt: number } | null>(null);
  const selectPointerRef = useRef<{ x: number; y: number } | null>(null);
  const onSelectSystemRef = useRef(onSelectSystem);
  onSelectSystemRef.current = onSelectSystem;
  const onSystemClickRef = useRef(onSystemClick);
  onSystemClickRef.current = onSystemClick;
  const sessionsRef = useRef(sessions);
  sessionsRef.current = sessions;
  const cameraStateRef = useRef<Record<"ortho" | "perspective", { position: THREE.Vector3; target: THREE.Vector3; zoom: number } | null>>({
    ortho: null,
    perspective: null,
  });
  const systemCameraStateRef = useRef<{ position: THREE.Vector3; target: THREE.Vector3 } | null>(null);
  const [projectionMode, setProjectionMode] = useState<"ortho" | "perspective">("perspective");
  const [systemHover, setSystemHover] = useState<SystemHoverInfo | null>(null);
  const [localSelectedSystemId, setLocalSelectedSystemId] = useState<string>();
  const [mapLevel, setMapLevel] = useState<"galaxy" | "system">("galaxy");
  const [readoutCollapsed, setReadoutCollapsed] = useState(false);
  const routeSelectionMode = onSelectSystem !== undefined;
  const directSystemClickMode = onSystemClick !== undefined;
  const effectiveSelectedSystemId = routeSelectionMode ? (localSelectedSystemId ?? selectedSystemId) : (selectedSystemId ?? localSelectedSystemId);
  const incomingMapData = useStableSnapshot(map);
  const [mapData, setMapData] = useState(incomingMapData);
  const pendingMapDataRef = useRef<typeof incomingMapData>();
  const exploration = useStableSnapshot(explorationSnapshot);
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedRouteHops, setSelectedRouteHops] = useState<Map<string, string[]>>(new Map());
  const highlightedSystemsKey = [...highlightedSystemIds].sort().join("|");
  const highlightedSystems = useMemo(() => new Set(highlightedSystemsKey ? highlightedSystemsKey.split("|") : []), [highlightedSystemsKey]);
  const dimmedSystemsKey = [...dimmedSystemIds].sort().join("|");
  const dimmedSystems = useMemo(() => new Set(dimmedSystemsKey ? dimmedSystemsKey.split("|") : []), [dimmedSystemsKey]);
  const selectablePoisKey = selectablePoiIds ? [...selectablePoiIds].sort().join("|") : null;
  const selectablePois = useMemo(
    () => (selectablePoisKey === null ? null : new Set(selectablePoisKey ? selectablePoisKey.split("|") : [])),
    [selectablePoisKey],
  );

  useEffect(() => {
    if (cameraInteractionRef.current) {
      pendingMapDataRef.current = incomingMapData;
      return;
    }
    pendingMapDataRef.current = undefined;
    setMapData(incomingMapData);
  }, [incomingMapData]);

  useEffect(() => {
    const origins = [...new Set(sessions.map((session) => session.location.system).filter((system): system is string => Boolean(system)))];
    if (!routeSelectionMode || !effectiveSelectedSystemId || !origins.length) {
      setSelectedRouteHops(new Map());
      return;
    }
    const controller = new AbortController();
    void fetchRoutes(
      origins.map((from) => ({ from, to: effectiveSelectedSystemId })),
      true,
      controller.signal,
    )
      .then((routes) => setSelectedRouteHops(new Map(routes.flatMap((route) => (route ? [[route.fromSystem, route.hops] as const] : [])))))
      .catch((routeError) => {
        if (!controller.signal.aborted) console.warn("Failed to load selected-system routes", routeError);
      });
    return () => controller.abort();
  }, [effectiveSelectedSystemId, routeSelectionMode, sessions]);

  const positioned = useMemo(() => fitSystems(mapData?.systems ?? []), [mapData]);
  const systemsById = useMemo(() => new Map(positioned.map((system) => [system.id, system])), [positioned]);
  const exploredSystems = useMemo(() => new Set(exploration?.exploredSystems ?? []), [exploration]);
  const visitedPois = useMemo(() => new Set(exploration?.visitedPois ?? []), [exploration]);
  const stationSystems = useMemo(
    () => new Set((mapData?.knownPois ?? []).filter((poi) => poi.hasBase || /station/i.test(`${poi.type} ${poi.id} ${poi.name}`)).map((poi) => poi.systemId)),
    [mapData],
  );
  const normalizedSearch = searchQuery.trim().toLowerCase();
  const sessionsBySystem = useMemo(() => {
    const bySystem = new Map<string, SessionState[]>();
    for (const session of sessions) {
      const system = session.location.system;
      if (!system) continue;
      bySystem.set(system, [...(bySystem.get(system) ?? []), session]);
    }
    for (const occupants of bySystem.values()) {
      occupants.sort((left, right) => unitMarkerKey(left).localeCompare(unitMarkerKey(right)));
    }
    return bySystem;
  }, [sessions]);
  const observedSquadColorBySession = useMemo(() => {
    const colors = new Map<string, string>();
    const byIdentity = new Map(
      sessions.flatMap(
        (session) =>
          [
            [session.botId, session],
            [session.sessionHandle, session],
          ] as const,
      ),
    );
    for (const squad of squads) {
      for (const botId of squad.botIds) {
        const session = byIdentity.get(botId);
        if (session) colors.set(session.sessionHandle, squad.color);
      }
    }
    return colors;
  }, [sessions, squads]);
  const stableSquadColorEntries = useStableSnapshot([...observedSquadColorBySession.entries()].sort(([left], [right]) => left.localeCompare(right)));
  const squadColorBySession = useMemo(() => new Map(stableSquadColorEntries), [stableSquadColorEntries]);
  const sessionsBySystemRef = useRef(sessionsBySystem);
  sessionsBySystemRef.current = sessionsBySystem;
  const squadColorBySessionRef = useRef(squadColorBySession);
  squadColorBySessionRef.current = squadColorBySession;
  const selectedRouteHopsRef = useRef(selectedRouteHops);
  selectedRouteHopsRef.current = selectedRouteHops;
  const selectedSystemIdRef = useRef(effectiveSelectedSystemId);
  selectedSystemIdRef.current = effectiveSelectedSystemId;
  const occupancyKey = galaxySessionRenderKey(sessions, squadColorBySession);
  latestOccupancyKeyRef.current = occupancyKey;
  const [renderedOccupancyKey, setRenderedOccupancyKey] = useState(occupancyKey);
  useEffect(() => {
    if (!cameraInteractionRef.current) setRenderedOccupancyKey(occupancyKey);
  }, [occupancyKey]);
  const selectedSquadHandles = useMemo<Set<string> | null>(() => null, []);
  const selectedSquadHandlesKey = selectedSquadHandles === null ? "__all__" : [...selectedSquadHandles].sort().join("|");
  const selectedSystem = effectiveSelectedSystemId ? systemsById.get(effectiveSelectedSystemId) : undefined;
  const selectedSystemPois = useMemo(
    () => (mapData?.knownPois ?? []).filter((poi) => poi.systemId === effectiveSelectedSystemId),
    [effectiveSelectedSystemId, mapData],
  );
  const selectedSystemStar = useMemo(
    () =>
      selectedSystemPois.find((poi) => {
        const type = poi.type
          .trim()
          .toLowerCase()
          .replace(/[\s-]+/g, "_");
        const id = poi.id.trim().toLowerCase();
        return /(^|_)(star|sun)(_|$)/.test(type) || /(^|[_-])(star|sun)([_-]|$)/.test(id);
      }),
    [selectedSystemPois],
  );
  useEffect(() => {
    if (selectablePois === null) return;
    console.info(
      "[GalaxyMap] mining render diagnostic",
      JSON.stringify({
        selectablePoiIds: [...selectablePois],
        visibleSystemIds: positioned.filter((system) => !dimmedSystems.has(system.id)).map((system) => system.id),
        selectedSystemId: effectiveSelectedSystemId ?? null,
        selectedSystemDimmed: effectiveSelectedSystemId ? dimmedSystems.has(effectiveSelectedSystemId) : null,
        selectedSystemPois: selectedSystemPois.map((poi) => ({ id: poi.id, selectable: selectablePois.has(poi.id), type: poi.type })),
      }),
    );
  }, [dimmedSystems, effectiveSelectedSystemId, positioned, selectablePois, selectedSystemPois]);
  const orbitingSystemPois = useMemo(() => selectedSystemPois.filter((poi) => poi.id !== selectedSystemStar?.id), [selectedSystemPois, selectedSystemStar]);
  const systemMapPois = useMemo(() => {
    const starX = typeof selectedSystemStar?.x === "number" && Number.isFinite(selectedSystemStar.x) ? selectedSystemStar.x : 0;
    const starY = typeof selectedSystemStar?.y === "number" && Number.isFinite(selectedSystemStar.y) ? selectedSystemStar.y : 0;
    const located = orbitingSystemPois.filter(
      (poi): poi is (typeof orbitingSystemPois)[number] & { x: number; y: number } =>
        typeof poi.x === "number" && Number.isFinite(poi.x) && typeof poi.y === "number" && Number.isFinite(poi.y),
    );
    const maxDistance = Math.max(...located.map((poi) => Math.hypot(poi.x - starX, poi.y - starY)), 1);
    const positionedPois = orbitingSystemPois.map((poi, index) => {
      const fallbackAngle = (Math.PI * 2 * index) / Math.max(orbitingSystemPois.length, 1);
      const hasPosition = typeof poi.x === "number" && Number.isFinite(poi.x) && typeof poi.y === "number" && Number.isFinite(poi.y);
      const relativeX = hasPosition ? poi.x! - starX : 0;
      const relativeY = hasPosition ? poi.y! - starY : 0;
      const angle = hasPosition ? Math.atan2(relativeY, relativeX) : fallbackAngle;
      const distance = hasPosition ? Math.hypot(relativeX, relativeY) : maxDistance * (0.35 + (index % 4) * 0.16);
      const radius = 72 + (distance / maxDistance) * 230;
      return { ...poi, angle, radius, sx: WIDTH / 2 + Math.cos(angle) * radius, sy: HEIGHT / 2 + Math.sin(angle) * radius };
    });
    const planets = positionedPois.filter((poi) => /planet/i.test(poi.type));
    return positionedPois.map((poi) => {
      const nearestPlanetOrbitGap = Math.min(...planets.filter((planet) => planet.id !== poi.id).map((planet) => Math.abs(planet.radius - poi.radius)), 24);
      const bodyScale = Math.min(1, Math.max(0.45, Math.sqrt(nearestPlanetOrbitGap / 24)));
      return { ...poi, bodyScale };
    });
  }, [orbitingSystemPois, selectedSystemStar]);
  const observedSystemUnits = useMemo(
    () =>
      sessions
        .filter((session) => !session.location.inTransit && session.location.system === effectiveSelectedSystemId)
        .sort((left, right) => unitMarkerKey(left).localeCompare(unitMarkerKey(right))),
    [effectiveSelectedSystemId, sessions],
  );
  const systemUnits = useStableSnapshot(
    observedSystemUnits,
    observedSystemUnits.map((session) => `${unitMarkerKey(session)}:${session.location.poi ?? ""}`).join("|"),
  );
  const observedExternalUnits = useMemo(() => {
    const ownedIds = new Set(sessions.map((session) => session.botId));
    const units = new Map<string, { key: string; poiId: string | null }>();
    for (const session of sessions) {
      if (session.location.inTransit || session.location.system !== effectiveSelectedSystemId) continue;
      for (const player of Object.values(session.observedPlayers)) {
        if (player.offline || ownedIds.has(player.playerId)) continue;
        units.set(player.playerId, { key: player.playerId, poiId: session.location.poi });
      }
    }
    return [...units.values()].sort((left, right) => left.key.localeCompare(right.key));
  }, [effectiveSelectedSystemId, sessions]);
  const externalUnits = useStableSnapshot(observedExternalUnits, observedExternalUnits.map((unit) => `${unit.key}:${unit.poiId ?? ""}`).join("|"));
  useEffect(() => {
    const canvas = systemCanvasRef.current;
    if (!canvas || mapLevel !== "system" || !selectedSystem) return;
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: false });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(GALAXY_BACKGROUND_COLOR);
    scene.fog = new THREE.FogExp2(GALAXY_BACKGROUND_COLOR, 0.0012);
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.toneMapping = THREE.NoToneMapping;
    const camera = new THREE.PerspectiveCamera(46, 1, 1, 2200);
    camera.position.copy(systemCameraStateRef.current?.position ?? new THREE.Vector3(0, 390, 470));
    const controls = new OrbitControls(camera, canvas);
    controls.target.copy(systemCameraStateRef.current?.target ?? new THREE.Vector3(0, 0, 0));
    controls.enablePan = false;
    controls.enableDamping = true;
    controls.dampingFactor = 0.075;
    controls.minDistance = 180;
    controls.maxDistance = 1050;
    controls.maxPolarAngle = Math.PI * 0.49;
    controls.update();

    const gridMaterial = new THREE.ShaderMaterial({
      transparent: true,
      depthWrite: false,
      side: THREE.DoubleSide,
      uniforms: {
        fogDensity: { value: GRID_FOG_DENSITY },
        minorColor: { value: new THREE.Color(0x164e59) },
        majorColor: { value: new THREE.Color(0x62dff4) },
      },
      vertexShader: `
        varying vec3 vWorldPosition;
        varying float vViewDepth;
        void main() {
          vec4 worldPosition = modelMatrix * vec4(position, 1.0);
          vec4 viewPosition = viewMatrix * worldPosition;
          vWorldPosition = worldPosition.xyz;
          vViewDepth = -viewPosition.z;
          gl_Position = projectionMatrix * viewPosition;
        }
      `,
      fragmentShader: `
        uniform float fogDensity;
        uniform vec3 minorColor;
        uniform vec3 majorColor;
        varying vec3 vWorldPosition;
        varying float vViewDepth;

        float gridLine(vec2 worldCoordinate, float spacing, float pixelWidth) {
          vec2 coordinate = worldCoordinate / spacing;
          vec2 coordinateWidth = max(fwidth(coordinate), vec2(0.00001));
          vec2 distanceToLine = abs(fract(coordinate - 0.5) - 0.5) / coordinateWidth;
          return 1.0 - smoothstep(pixelWidth - 0.5, pixelWidth + 0.5, min(distanceToLine.x, distanceToLine.y));
        }

        void main() {
          vec2 worldCoordinate = vWorldPosition.xz;
          float minorDerivative = max(fwidth(worldCoordinate.x / 40.0), fwidth(worldCoordinate.y / 40.0));
          float minorLine = gridLine(worldCoordinate, 40.0, 1.15) * (1.0 - smoothstep(0.24, 0.58, minorDerivative));
          float majorLine = gridLine(worldCoordinate, 200.0, 1.15);
          float lineAlpha = max(minorLine * 0.78, majorLine * 0.62);
          vec3 lineColor = mix(minorColor, majorColor, clamp(majorLine, 0.0, 1.0));
          float radialFade = 1.0 - smoothstep(500.0, 780.0, length(worldCoordinate));
          float fogVisibility = exp(-fogDensity * fogDensity * vViewDepth * vViewDepth);
          float alpha = lineAlpha * radialFade * fogVisibility;
          if (alpha < 0.002) discard;
          gl_FragColor = vec4(lineColor, alpha);
        }
      `,
    });
    const gridGeometry = new THREE.PlaneGeometry(1600, 1600);
    const grid = new THREE.Mesh(gridGeometry, gridMaterial);
    grid.rotation.x = -Math.PI / 2;
    grid.position.y = -2;
    grid.renderOrder = -10;
    scene.add(grid);

    const orbitVertices: number[] = [];
    const orbitRadii = [...new Set(systemMapPois.map((poi) => Math.round(poi.radius)))];
    const segments = 96;
    for (const radius of orbitRadii) {
      for (let segment = 0; segment < segments; segment++) {
        const a = (segment / segments) * Math.PI * 2;
        const b = ((segment + 1) / segments) * Math.PI * 2;
        orbitVertices.push(Math.cos(a) * radius, 0, Math.sin(a) * radius, Math.cos(b) * radius, 0, Math.sin(b) * radius);
      }
    }
    const orbitGeometry = new THREE.BufferGeometry();
    orbitGeometry.setAttribute("position", new THREE.Float32BufferAttribute(orbitVertices, 3));
    const orbitMaterial = new THREE.LineDashedMaterial({ color: 0x7de8f5, transparent: true, opacity: 0.22, dashSize: 4, gapSize: 7 });
    const orbits = new THREE.LineSegments(orbitGeometry, orbitMaterial);
    orbits.computeLineDistances();
    scene.add(orbits);

    const poiGeometry = new THREE.SphereGeometry(6, 12, 8);
    const poiMaterial = new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 1 });
    const poiMesh = new THREE.InstancedMesh(poiGeometry, poiMaterial, systemMapPois.length);
    const poiPositions = systemMapPois.map((poi) => new THREE.Vector3(Math.cos(poi.angle) * poi.radius, 4, Math.sin(poi.angle) * poi.radius));
    const poiMatrix = new THREE.Matrix4();
    const poiQuaternion = new THREE.Quaternion();
    const poiScale = new THREE.Vector3();
    systemMapPois.forEach((poi, index) => {
      poiScale.setScalar(poi.bodyScale);
      poiMatrix.compose(poiPositions[index]!, poiQuaternion, poiScale);
      poiMesh.setMatrixAt(index, poiMatrix);
      const selectable = selectablePois === null || selectablePois.has(poi.id);
      poiMesh.setColorAt(index, new THREE.Color(selectable ? (poi.hasBase ? 0xffd65e : 0x75ddec) : 0x17282b));
    });
    poiMesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    poiMesh.instanceMatrix.needsUpdate = true;
    if (poiMesh.instanceColor) poiMesh.instanceColor.needsUpdate = true;
    scene.add(poiMesh);

    const unitPipCanvas = document.createElement("canvas");
    unitPipCanvas.width = 64;
    unitPipCanvas.height = 64;
    const unitPipContext = unitPipCanvas.getContext("2d");
    if (unitPipContext) {
      unitPipContext.fillStyle = "#ffffff";
      unitPipContext.beginPath();
      unitPipContext.moveTo(32, 5);
      unitPipContext.lineTo(58, 55);
      unitPipContext.lineTo(6, 55);
      unitPipContext.closePath();
      unitPipContext.fill();
    }
    const unitPipTexture = new THREE.CanvasTexture(unitPipCanvas);
    unitPipTexture.colorSpace = THREE.SRGBColorSpace;
    const unitsPerAnchor = new Map<string, number>();
    for (const session of systemUnits) {
      const anchorKey = session.location.poi ?? "__star__";
      unitsPerAnchor.set(anchorKey, (unitsPerAnchor.get(anchorKey) ?? 0) + 1);
    }
    const nextUnitIndex = new Map<string, number>();
    const unitPips = systemUnits.map((session) => {
      const anchorKey = session.location.poi ?? "__star__";
      const index = nextUnitIndex.get(anchorKey) ?? 0;
      nextUnitIndex.set(anchorKey, index + 1);
      const sprite = new THREE.Sprite(
        new THREE.SpriteMaterial({
          map: unitPipTexture,
          color: squadColorBySession.get(session.sessionHandle) ?? 0xffffff,
          transparent: true,
          depthTest: false,
          depthWrite: false,
        }),
      );
      sprite.scale.set(5.5, 5.5, 1);
      sprite.renderOrder = 20;
      scene.add(sprite);
      return { session, sprite, index, count: unitsPerAnchor.get(anchorKey) ?? 1 };
    });
    const externalGeometry = new THREE.PlaneGeometry(2.8, 2.8);
    const externalMaterial = new THREE.MeshBasicMaterial({
      map: unitPipTexture,
      color: 0xb8dce3,
      transparent: true,
      depthTest: false,
      depthWrite: false,
      side: THREE.DoubleSide,
    });
    const externalMesh = new THREE.InstancedMesh(externalGeometry, externalMaterial, externalUnits.length);
    externalMesh.renderOrder = 10;
    externalMesh.frustumCulled = false;
    scene.add(externalMesh);
    const externalLayout = externalUnits.map((unit) => ({
      ...unit,
      angle: stableNoise(unit.key, 17) * Math.PI * 2,
      radiusNoise: Math.sqrt(stableNoise(unit.key, 43)),
      radialJitter: stableNoise(unit.key, 89) - 0.5,
    }));
    const externalMatrix = new THREE.Matrix4();
    const externalPosition = new THREE.Vector3();
    const externalScale = new THREE.Vector3(1, 1, 1);

    const starGeometry = new THREE.SphereGeometry(20, 24, 16);
    const starMaterial = new THREE.MeshBasicMaterial({ color: 0xffef8f });
    scene.add(new THREE.Mesh(starGeometry, starMaterial));

    const resize = () => {
      const { width, height } = canvas.getBoundingClientRect();
      renderer.setSize(width, height, false);
      camera.aspect = width / Math.max(height, 1);
      camera.updateProjectionMatrix();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    resize();
    let systemSelectStart: { x: number; y: number } | null = null;
    const handleSystemPointerDown = (event: globalThis.PointerEvent) => {
      systemSelectStart = { x: event.clientX, y: event.clientY };
    };
    const handleSystemPointerUp = (event: globalThis.PointerEvent) => {
      const start = systemSelectStart;
      systemSelectStart = null;
      if (!routeSelectionMode || !start || Math.hypot(event.clientX - start.x, event.clientY - start.y) > 5) return;
      const bounds = canvas.getBoundingClientRect();
      const pointerX = event.clientX - bounds.left;
      const pointerY = event.clientY - bounds.top;
      const projected = new THREE.Vector3();
      let selectedPoiId: string | undefined;
      let nearestDistanceSquared = 18 * 18;
      const candidates = [
        ...systemMapPois
          .filter((poi) => selectablePois === null || selectablePois.has(poi.id))
          .map((poi) => ({ id: poi.id, position: poiPositions[systemMapPois.indexOf(poi)]! })),
        ...(selectedSystemStar && (selectablePois === null || selectablePois.has(selectedSystemStar.id))
          ? [{ id: selectedSystemStar.id, position: new THREE.Vector3(0, 0, 0) }]
          : selectedSystemPois.length === 0 && (selectablePois === null || selectablePois.has(selectedSystem.id))
            ? [{ id: selectedSystem.id, position: new THREE.Vector3(0, 0, 0) }]
            : []),
      ];
      for (const candidate of candidates) {
        projected.copy(candidate.position).project(camera);
        if (projected.z < -1 || projected.z > 1) continue;
        const screenX = (projected.x * 0.5 + 0.5) * bounds.width;
        const screenY = (-projected.y * 0.5 + 0.5) * bounds.height;
        const distanceSquared = (screenX - pointerX) ** 2 + (screenY - pointerY) ** 2;
        if (distanceSquared >= nearestDistanceSquared) continue;
        nearestDistanceSquared = distanceSquared;
        selectedPoiId = candidate.id;
      }
      if (selectedPoiId) {
        if (selectablePois !== null) console.info("[GalaxyMap] selected mining POI", JSON.stringify({ systemId: selectedSystem.id, poiId: selectedPoiId }));
        onSelectSystemRef.current?.(selectedPoiId);
        setMapLevel("galaxy");
      }
    };
    canvas.addEventListener("pointerdown", handleSystemPointerDown);
    canvas.addEventListener("pointerup", handleSystemPointerUp);
    let frame = 0;
    const labelPosition = new THREE.Vector3();
    const render = () => {
      const timeNow = Date.now() / 1000;
      systemMapPois.forEach((poi, index) => {
        const angularVelocity = 0.16 * Math.pow(72 / Math.max(poi.radius, 72), 1.5);
        const durablePhase = (timeNow * angularVelocity) % (Math.PI * 2);
        const angle = poi.angle + durablePhase;
        const position = poiPositions[index]!;
        position.set(Math.cos(angle) * poi.radius, 4, Math.sin(angle) * poi.radius);
        poiScale.setScalar(poi.bodyScale);
        poiMatrix.compose(position, poiQuaternion, poiScale);
        poiMesh.setMatrixAt(index, poiMatrix);
      });
      poiMesh.instanceMatrix.needsUpdate = true;
      for (const { session, sprite, index, count } of unitPips) {
        const poiId = session.location.poi;
        const poiIndex = poiId ? systemMapPois.findIndex((poi) => poi.id === poiId) : -1;
        const anchor = poiIndex >= 0 ? poiPositions[poiIndex]! : new THREE.Vector3();
        const markerOrbitRadius = poiIndex >= 0 ? 6 * systemMapPois[poiIndex]!.bodyScale + 6 : 26;
        const angle = (Math.PI * 2 * index) / Math.max(count, 1);
        sprite.position.set(anchor.x + Math.cos(angle) * markerOrbitRadius, anchor.y + 1.5, anchor.z + Math.sin(angle) * markerOrbitRadius);
      }
      externalLayout.forEach((unit, instanceIndex) => {
        const poiIndex = unit.poiId ? systemMapPois.findIndex((poi) => poi.id === unit.poiId) : -1;
        const anchor = poiIndex >= 0 ? poiPositions[poiIndex]! : new THREE.Vector3();
        const bodyClearance = poiIndex >= 0 ? 6 * systemMapPois[poiIndex]!.bodyScale + 7 : 18;
        const cloudDepth = poiIndex >= 0 ? 24 : 42;
        const radius = bodyClearance + unit.radiusNoise * cloudDepth + unit.radialJitter * 3;
        externalPosition.set(anchor.x + Math.cos(unit.angle) * radius, anchor.y - 2.5, anchor.z + Math.sin(unit.angle) * radius);
        externalMatrix.compose(externalPosition, camera.quaternion, externalScale);
        externalMesh.setMatrixAt(instanceIndex, externalMatrix);
      });
      externalMesh.instanceMatrix.needsUpdate = true;
      controls.update();
      renderer.render(scene, camera);
      const labels = systemPoiLabelsRef.current?.children;
      if (labels) {
        const { width, height } = canvas.getBoundingClientRect();
        poiPositions.forEach((position, index) => {
          const label = labels[index] as HTMLElement | undefined;
          if (!label) return;
          const projected = labelPosition.copy(position).project(camera);
          const visible = projected.z >= -1 && projected.z <= 1;
          label.style.display = visible ? "block" : "none";
          if (visible) label.style.transform = `translate(${(projected.x * 0.5 + 0.5) * width + 9}px, ${(-projected.y * 0.5 + 0.5) * height - 6}px)`;
        });
      }
      frame = requestAnimationFrame(render);
    };
    render();
    return () => {
      systemCameraStateRef.current = {
        position: camera.position.clone(),
        target: controls.target.clone(),
      };
      cancelAnimationFrame(frame);
      observer.disconnect();
      canvas.removeEventListener("pointerdown", handleSystemPointerDown);
      canvas.removeEventListener("pointerup", handleSystemPointerUp);
      controls.dispose();
      gridGeometry.dispose();
      gridMaterial.dispose();
      orbitGeometry.dispose();
      orbitMaterial.dispose();
      poiGeometry.dispose();
      poiMaterial.dispose();
      for (const { sprite } of unitPips) {
        (sprite.material as THREE.SpriteMaterial).dispose();
      }
      unitPipTexture.dispose();
      externalGeometry.dispose();
      externalMaterial.dispose();
      starGeometry.dispose();
      starMaterial.dispose();
      renderer.dispose();
    };
  }, [
    externalUnits,
    mapLevel,
    routeSelectionMode,
    selectablePois,
    selectedSystem,
    selectedSystemPois,
    selectedSystemStar,
    squadColorBySession,
    systemMapPois,
    systemUnits,
  ]);

  function selectSystem(systemId: string) {
    if (selectablePois !== null) console.info("[GalaxyMap] opened mining system", JSON.stringify({ systemId, dimmed: dimmedSystems.has(systemId) }));
    if (systemId !== effectiveSelectedSystemId) systemCameraStateRef.current = null;
    setLocalSelectedSystemId(systemId);
    setReadoutCollapsed(false);
    if (directSystemClickMode) {
      onSystemClickRef.current?.(systemId);
      return;
    }
    setMapLevel("system");
  }
  const edges = useMemo(() => {
    const seen = new Set<string>();
    const result: Array<{ from: PositionedSystem; to: PositionedSystem; key: string }> = [];
    for (const from of positioned) {
      for (const id of from.connections ?? []) {
        const to = systemsById.get(id);
        if (!to) continue;
        const key = [from.id, to.id].sort().join("::");
        if (seen.has(key)) continue;
        seen.add(key);
        result.push({ from, to, key });
      }
    }
    return result;
  }, [positioned, systemsById]);

  function visitLevel(system: GalaxyMapSystem, occupiedSystems = sessionsBySystem): SystemVisitLevel {
    const pois = system.pois ?? [];
    if (pois.length > 0 && pois.every((poi) => visitedPois.has(poi.id))) {
      return "complete";
    }
    if (exploredSystems.has(system.id) || occupiedSystems.has(system.id) || pois.some((poi) => visitedPois.has(poi.id))) {
      return "visited";
    }
    return "unknown";
  }

  function nodeColor(system: GalaxyMapSystem, occupiedSystems = sessionsBySystem): string {
    if (highlightedSystems.has(system.id)) return "#ff0000";
    const level = visitLevel(system, occupiedSystems);
    if (system.isStronghold) {
      return mixColor(VISIT_LEVEL_COLORS[level], STRONGHOLD_COLOR, Math.max(0.82, VISIT_LEVEL_MIX[level]));
    }
    const empire = normalizeEmpire(system.empire);
    if (!empire) return VISIT_LEVEL_COLORS[level];
    return mixColor(VISIT_LEVEL_COLORS[level], EMPIRE_COLORS[empire], VISIT_LEVEL_MIX[level]);
  }

  useEffect(() => {
    const canvas = threeCanvasRef.current;
    if (!canvas || positioned.length === 0) return;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(GALAXY_BACKGROUND_COLOR);
    scene.fog = new THREE.FogExp2(GALAXY_BACKGROUND_COLOR, 0.00072);
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: false });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.toneMapping = THREE.NoToneMapping;

    const camera = projectionMode === "ortho" ? new THREE.OrthographicCamera(-440, 440, 300, -300, 1, 4000) : new THREE.PerspectiveCamera(46, 1, 1, 4000);
    camera.up.set(0, projectionMode === "perspective" ? 1 : 0, projectionMode === "perspective" ? 0 : -1);
    const defaultPosition = projectionMode === "ortho" ? new THREE.Vector3(0, 780, 0) : DEFAULT_PERSPECTIVE_POSITION;
    camera.position.copy(cameraStateRef.current[projectionMode]?.position ?? defaultPosition);
    camera.zoom = cameraStateRef.current[projectionMode]?.zoom ?? (projectionMode === "ortho" ? DEFAULT_ORTHO_ZOOM : 1);
    const initialTarget = cameraStateRef.current[projectionMode]?.target ?? new THREE.Vector3(0, 0, 0);
    camera.lookAt(initialTarget);
    camera.updateProjectionMatrix();
    camera.updateMatrixWorld(true);
    cameraRef.current = camera;
    const controls = new OrbitControls(camera, canvas);
    controls.target.copy(initialTarget);
    controls.enableRotate = projectionMode === "perspective";
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.minDistance = projectionMode === "perspective" ? MIN_PERSPECTIVE_DISTANCE / (1 + MAP_ZOOM_RANGE) : 180;
    controls.maxDistance = MIN_PERSPECTIVE_DISTANCE / (1 - MAP_ZOOM_RANGE);
    controls.minZoom = DEFAULT_ORTHO_ZOOM * (1 - MAP_ZOOM_RANGE);
    controls.maxZoom = DEFAULT_ORTHO_ZOOM * (1 + MAP_ZOOM_RANGE);
    controls.maxPolarAngle = projectionMode === "perspective" ? Math.PI / 2 - 0.03 : Math.PI;
    controls.update();
    controls.saveState();
    controls.screenSpacePanning = true;
    controlsRef.current = controls;
    const handleInteractionStart = () => {
      cameraInteractionRef.current = true;
      if (hoveredSystemIdRef.current !== null) {
        hoveredSystemIdRef.current = null;
        setSystemHover(null);
      }
    };
    const handleInteractionEnd = () => {
      cameraInteractionRef.current = false;
      if (pendingMapDataRef.current !== undefined) {
        setMapData(pendingMapDataRef.current);
        pendingMapDataRef.current = undefined;
      }
      setRenderedOccupancyKey(latestOccupancyKeyRef.current);
    };
    controls.addEventListener("start", handleInteractionStart);
    controls.addEventListener("end", handleInteractionEnd);

    const gridMaterial = new THREE.ShaderMaterial({
      transparent: true,
      depthWrite: false,
      side: THREE.DoubleSide,
      uniforms: {
        fogDensity: { value: GRID_FOG_DENSITY },
        minorColor: { value: new THREE.Color(0x164e59) },
        majorColor: { value: new THREE.Color(0x62dff4) },
      },
      vertexShader: `
        varying vec3 vWorldPosition;
        varying float vViewDepth;
        void main() {
          vec4 worldPosition = modelMatrix * vec4(position, 1.0);
          vec4 viewPosition = viewMatrix * worldPosition;
          vWorldPosition = worldPosition.xyz;
          vViewDepth = -viewPosition.z;
          gl_Position = projectionMatrix * viewPosition;
        }
      `,
      fragmentShader: `
        uniform float fogDensity;
        uniform vec3 minorColor;
        uniform vec3 majorColor;
        varying vec3 vWorldPosition;
        varying float vViewDepth;

        float gridLine(vec2 worldCoordinate, float spacing, float pixelWidth) {
          vec2 coordinate = worldCoordinate / spacing;
          vec2 coordinateWidth = max(fwidth(coordinate), vec2(0.00001));
          vec2 distanceToLine = abs(fract(coordinate - 0.5) - 0.5) / coordinateWidth;
          float nearestLine = min(distanceToLine.x, distanceToLine.y);
          return 1.0 - smoothstep(pixelWidth - 0.5, pixelWidth + 0.5, nearestLine);
        }

        void main() {
          vec2 worldCoordinate = vWorldPosition.xz;
          float minorDerivative = max(fwidth(worldCoordinate.x / 40.0), fwidth(worldCoordinate.y / 40.0));
          float minorLod = 1.0 - smoothstep(0.24, 0.58, minorDerivative);
          float minorLine = gridLine(worldCoordinate, 40.0, 1.15) * minorLod;
          float majorLine = gridLine(worldCoordinate, 200.0, 1.15);
          float lineAlpha = max(minorLine * 0.78, majorLine * 0.62);
          vec3 lineColor = mix(minorColor, majorColor, clamp(majorLine, 0.0, 1.0));

          float radialDistance = length(worldCoordinate);
          float radialFade = 1.0 - smoothstep(1350.0, 1950.0, radialDistance);
          float fogVisibility = exp(-fogDensity * fogDensity * vViewDepth * vViewDepth);
          float alpha = lineAlpha * radialFade * fogVisibility;
          if (alpha < 0.002) discard;
          gl_FragColor = vec4(lineColor, alpha);
        }
      `,
    });
    const grid = new THREE.Mesh(new THREE.PlaneGeometry(4000, 4000), gridMaterial);
    grid.rotation.x = -Math.PI / 2;
    grid.renderOrder = -10;
    scene.add(grid);
    const orbitReticle = new THREE.Group();
    const orbitRing = new THREE.Mesh(
      new THREE.RingGeometry(4.5, 5.1, 32),
      new THREE.MeshBasicMaterial({ color: 0x7de8f5, transparent: true, opacity: 0.8, side: THREE.DoubleSide }),
    );
    orbitRing.rotation.x = -Math.PI / 2;
    orbitReticle.add(orbitRing);
    const orbitCross = new THREE.LineSegments(
      new THREE.BufferGeometry().setFromPoints([
        new THREE.Vector3(-8, 0, 0),
        new THREE.Vector3(-5.8, 0, 0),
        new THREE.Vector3(5.8, 0, 0),
        new THREE.Vector3(8, 0, 0),
        new THREE.Vector3(0, 0, -8),
        new THREE.Vector3(0, 0, -5.8),
        new THREE.Vector3(0, 0, 5.8),
        new THREE.Vector3(0, 0, 8),
      ]),
      new THREE.LineBasicMaterial({ color: 0x7de8f5, transparent: true, opacity: 0.7 }),
    );
    orbitReticle.add(orbitCross);
    orbitReticle.position.y = 0.15;
    orbitReticle.visible = projectionMode === "perspective";
    scene.add(orbitReticle);

    const worldPosition = (system: PositionedSystem) =>
      new THREE.Vector3((system.sx - WIDTH / 2) * 0.82, systemElevation(system.id) * 0.11, (system.sy - HEIGHT / 2) * 0.82);
    const pipCanvas = document.createElement("canvas");
    pipCanvas.width = 64;
    pipCanvas.height = 64;
    const pipContext = pipCanvas.getContext("2d");
    if (pipContext) {
      pipContext.fillStyle = "#ffffff";
      pipContext.beginPath();
      pipContext.moveTo(32, 5);
      pipContext.lineTo(58, 55);
      pipContext.lineTo(6, 55);
      pipContext.closePath();
      pipContext.fill();
    }
    const pipTexture = new THREE.CanvasTexture(pipCanvas);
    pipTexture.colorSpace = THREE.SRGBColorSpace;
    const starCanvas = document.createElement("canvas");
    starCanvas.width = 256;
    starCanvas.height = 256;
    const starContext = starCanvas.getContext("2d");
    if (starContext) {
      starContext.fillStyle = "#ffffff";
      starContext.beginPath();
      starContext.arc(128, 128, 118, 0, Math.PI * 2);
      starContext.fill();
    }
    const starTexture = new THREE.CanvasTexture(starCanvas);
    starTexture.colorSpace = THREE.SRGBColorSpace;
    starTexture.minFilter = THREE.LinearMipmapLinearFilter;
    starTexture.magFilter = THREE.LinearFilter;
    starTexture.anisotropy = renderer.capabilities.getMaxAnisotropy();
    const stationOrbiters: Array<{ mesh: THREE.Sprite; center: THREE.Vector3; phase: number }> = [];
    const revealStars: Array<{
      materials: Array<{ material: THREE.Material; standardOpacity: number }>;
      distanceFromCenter: number;
    }> = [];
    const revealLines: Array<{
      material: THREE.Material;
      standardOpacity: number;
      endpointDistances: number[];
    }> = [];

    const connectionPositions: number[] = [];
    const connectionColors: number[] = [];
    const connectionBaseOpacities = new Float32Array(edges.length * 2);
    const connectionDistances: number[] = [];
    const connectionLiftWeights = new Float32Array(edges.length * 2).fill(1);
    const unknownConnectionColor = new THREE.Color(0x2c3739);
    const connectionOpacity = selectablePois === null ? 1 : 0.06;
    for (const [edgeIndex, { from, to }] of edges.entries()) {
      const fromPosition = worldPosition(from);
      const toPosition = worldPosition(to);
      connectionPositions.push(fromPosition.x, fromPosition.y, fromPosition.z, toPosition.x, toPosition.y, toPosition.z);
      const fromColor = new THREE.Color(nodeColor(from));
      const toColor = new THREE.Color(nodeColor(to));
      if (visitLevel(from) === "unknown") fromColor.copy(unknownConnectionColor);
      if (visitLevel(to) === "unknown") toColor.copy(unknownConnectionColor);
      connectionColors.push(fromColor.r, fromColor.g, fromColor.b, toColor.r, toColor.g, toColor.b);
      connectionBaseOpacities[edgeIndex * 2] = connectionOpacity;
      connectionBaseOpacities[edgeIndex * 2 + 1] = connectionOpacity;
      connectionDistances.push(Math.hypot(fromPosition.x, fromPosition.z), Math.hypot(toPosition.x, toPosition.z));
    }
    const connectionGeometry = new THREE.BufferGeometry();
    connectionGeometry.setAttribute("position", new THREE.Float32BufferAttribute(connectionPositions, 3));
    connectionGeometry.setAttribute("color", new THREE.Float32BufferAttribute(connectionColors, 3));
    connectionGeometry.setAttribute("baseOpacity", new THREE.Float32BufferAttribute(connectionBaseOpacities, 1));
    connectionGeometry.setAttribute("revealDistance", new THREE.Float32BufferAttribute(connectionDistances, 1));
    connectionGeometry.setAttribute("liftWeight", new THREE.Float32BufferAttribute(connectionLiftWeights, 1));
    const connectionMaterial = new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, depthWrite: false });
    const connectionWaveRadius = enableVertexReveal(connectionMaterial);
    scene.add(new THREE.LineSegments(connectionGeometry, connectionMaterial));

    const stemPositions: number[] = [];
    const stemColors: number[] = [];
    const stemLiftWeights = new Float32Array(positioned.length * 2);
    const stemBaseOpacities = new Float32Array(positioned.length * 2);
    const stemRevealDistances = new Float32Array(positioned.length * 2);
    const planeBaseOpacities = new Float32Array(positioned.length);
    const starBaseOpacities = new Float32Array(positioned.length);
    const systemRevealDistances = new Float32Array(positioned.length);
    const starColors = new Float32Array(positioned.length * 3);
    const systemMatches: boolean[] = [];
    for (const [systemIndex, system] of positioned.entries()) {
      const position = worldPosition(system);
      const base = new THREE.Vector3(position.x, 0, position.z);
      const matches =
        (!normalizedSearch || system.id.toLowerCase().includes(normalizedSearch) || systemLabel(system).toLowerCase().includes(normalizedSearch)) &&
        (selectedSquadHandles === null || (sessionsBySystem.get(system.id) ?? []).some((session) => selectedSquadHandles.has(session.sessionHandle))) &&
        !dimmedSystems.has(system.id);
      systemMatches.push(matches);
      stemPositions.push(base.x, base.y, base.z, position.x, position.y, position.z);
      stemColors.push(0.396, 0.851, 0.922, 0.396, 0.851, 0.922);
      stemLiftWeights[systemIndex * 2] = 0;
      stemLiftWeights[systemIndex * 2 + 1] = 1;
      const stemOpacity = matches ? 0.65 : 0.08;
      stemBaseOpacities[systemIndex * 2] = stemOpacity;
      stemBaseOpacities[systemIndex * 2 + 1] = stemOpacity;
      const revealDistance = Math.hypot(position.x, position.z);
      stemRevealDistances[systemIndex * 2] = revealDistance;
      stemRevealDistances[systemIndex * 2 + 1] = revealDistance;
      planeBaseOpacities[systemIndex] = stemOpacity;
      starBaseOpacities[systemIndex] = matches ? 1 : 0.12;
      systemRevealDistances[systemIndex] = revealDistance;
    }
    const stemGeometry = new THREE.BufferGeometry();
    stemGeometry.setAttribute("position", new THREE.Float32BufferAttribute(stemPositions, 3));
    stemGeometry.setAttribute("color", new THREE.Float32BufferAttribute(stemColors, 3));
    stemGeometry.setAttribute("baseOpacity", new THREE.Float32BufferAttribute(stemBaseOpacities, 1));
    stemGeometry.setAttribute("revealDistance", new THREE.Float32BufferAttribute(stemRevealDistances, 1));
    stemGeometry.setAttribute("liftWeight", new THREE.Float32BufferAttribute(stemLiftWeights, 1));
    const stemMaterial = new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, depthWrite: false });
    const stemWaveRadius = enableVertexReveal(stemMaterial);
    scene.add(new THREE.LineSegments(stemGeometry, stemMaterial));

    const planeGeometry = new THREE.CircleGeometry(0.6, 12);
    planeGeometry.rotateX(-Math.PI / 2);
    planeGeometry.setAttribute("baseOpacity", new THREE.InstancedBufferAttribute(planeBaseOpacities, 1));
    planeGeometry.setAttribute("revealDistance", new THREE.InstancedBufferAttribute(systemRevealDistances, 1));
    const planeMaterial = new THREE.MeshBasicMaterial({ color: 0x65d9eb, transparent: true, depthWrite: false, side: THREE.DoubleSide });
    const planeWaveRadius = enableInstancedReveal(planeMaterial);
    const planePoints = new THREE.InstancedMesh(planeGeometry, planeMaterial, positioned.length);
    scene.add(planePoints);

    const starGeometry = new THREE.PlaneGeometry(1, 1);
    const starColorAttribute = new THREE.InstancedBufferAttribute(starColors, 3);
    starGeometry.setAttribute("baseOpacity", new THREE.InstancedBufferAttribute(starBaseOpacities, 1));
    starGeometry.setAttribute("revealDistance", new THREE.InstancedBufferAttribute(systemRevealDistances, 1));
    starGeometry.setAttribute("instanceTint", starColorAttribute);
    const starMaterial = new THREE.MeshBasicMaterial({
      map: starTexture,
      transparent: true,
      depthTest: false,
      depthWrite: false,
    });
    const starWaveRadius = enableInstancedReveal(starMaterial, true);
    const stars = new THREE.InstancedMesh(starGeometry, starMaterial, positioned.length);
    stars.renderOrder = 5;
    scene.add(stars);
    const instanceTransform = new THREE.Matrix4();
    const instanceScale = new THREE.Vector3();
    const instanceQuaternion = new THREE.Quaternion();

    for (const [systemIndex, system] of positioned.entries()) {
      const position = worldPosition(system);
      const matches = systemMatches[systemIndex];
      const systemRevealMaterials: Array<{ material: THREE.Material; standardOpacity: number }> = [];
      const color = new THREE.Color(nodeColor(system));
      instanceTransform.makeTranslation(position.x, 0.12, position.z);
      planePoints.setMatrixAt(systemIndex, instanceTransform);
      const nodeSize = visitLevel(system) === "complete" ? 4.86 : 3.78;
      instanceScale.set(nodeSize, nodeSize, 1);
      instanceTransform.compose(position, instanceQuaternion, instanceScale);
      stars.setMatrixAt(systemIndex, instanceTransform);
      color.toArray(starColors, systemIndex * 3);
      if (routeSelectionMode && system.id === effectiveSelectedSystemId) {
        const selectionRing = new THREE.Mesh(
          new THREE.RingGeometry(7.5, 9, 36),
          new THREE.MeshBasicMaterial({ color: 0xf4d77c, transparent: true, opacity: 0.95, side: THREE.DoubleSide, depthTest: false, depthWrite: false }),
        );
        selectionRing.position.copy(position);
        selectionRing.rotation.x = -Math.PI / 2;
        selectionRing.renderOrder = 6;
        scene.add(selectionRing);
        systemRevealMaterials.push({ material: selectionRing.material, standardOpacity: 0.95 });
      }

      if (stationSystems.has(system.id)) {
        const station = new THREE.Sprite(
          new THREE.SpriteMaterial({
            map: starTexture,
            color: color.clone().lerp(new THREE.Color(0xffffff), 0.3),
            transparent: true,
            depthTest: false,
            depthWrite: false,
          }),
        );
        station.scale.setScalar(nodeSize * 0.4);
        const phase = ((Math.abs(system.id.split("").reduce((hash, char) => hash + char.charCodeAt(0), 0)) % 360) * Math.PI) / 180;
        station.position.set(position.x + Math.cos(phase) * 4, position.y, position.z + Math.sin(phase) * 4);
        station.renderOrder = 6;
        scene.add(station);
        systemRevealMaterials.push({ material: station.material, standardOpacity: matches ? 1 : 0.12 });
        stationOrbiters.push({ mesh: station, center: position.clone(), phase });
      }

      if (system.isStronghold) {
        const ring = new THREE.Mesh(
          new THREE.RingGeometry(6, 7, 28),
          new THREE.MeshBasicMaterial({ color: 0xff8a2b, transparent: true, depthWrite: false, side: THREE.DoubleSide }),
        );
        ring.position.copy(position);
        ring.rotation.x = -Math.PI / 2;
        scene.add(ring);
        systemRevealMaterials.push({ material: ring.material, standardOpacity: matches ? 1 : 0.12 });
      }

      revealStars.push({
        materials: systemRevealMaterials,
        distanceFromCenter: Math.hypot(position.x, position.z),
      });
    }
    planePoints.instanceMatrix.needsUpdate = true;
    stars.instanceMatrix.needsUpdate = true;
    starColorAttribute.needsUpdate = true;

    const dynamicGroup = new THREE.Group();
    scene.add(dynamicGroup);
    const routePool: THREE.Line[] = [];
    const previewRoutePool: THREE.Line[] = [];
    const pipPool: THREE.Sprite[] = [];
    const pooledLine = (pool: THREE.Line[], index: number, opacity: number, renderOrder = 0) => {
      let line = pool[index];
      if (!line) {
        const material = new THREE.LineBasicMaterial({ transparent: true, depthWrite: false });
        line = new THREE.Line(new THREE.BufferGeometry(), material);
        line.userData.waveRadius = enableVertexReveal(material);
        line.renderOrder = renderOrder;
        pool[index] = line;
        dynamicGroup.add(line);
      }
      line.userData.baseOpacity = opacity;
      line.visible = true;
      return line;
    };
    const updateRouteGeometry = (line: THREE.Line, route: PositionedSystem[], heightOffset: number) => {
      const positions: THREE.Vector3[] = [];
      const revealDistances = new Float32Array(route.length);
      const baseOpacities = new Float32Array(route.length);
      const liftWeights = new Float32Array(route.length).fill(1);
      for (const [index, system] of route.entries()) {
        const point = worldPosition(system);
        point.y += heightOffset;
        positions.push(point);
        revealDistances[index] = Math.hypot(point.x, point.z);
        baseOpacities[index] = line.userData.baseOpacity as number;
      }
      line.geometry.setFromPoints(positions);
      line.geometry.setAttribute("revealDistance", new THREE.Float32BufferAttribute(revealDistances, 1));
      line.geometry.setAttribute("baseOpacity", new THREE.Float32BufferAttribute(baseOpacities, 1));
      line.geometry.setAttribute("liftWeight", new THREE.Float32BufferAttribute(liftWeights, 1));
    };
    const pooledPip = (index: number) => {
      let pip = pipPool[index];
      if (!pip) {
        pip = new THREE.Sprite(new THREE.SpriteMaterial({ map: pipTexture, transparent: true, depthTest: false, depthWrite: false }));
        pip.scale.set(5.5, 5.5, 1);
        pipPool[index] = pip;
        dynamicGroup.add(pip);
      }
      pip.visible = true;
      return pip;
    };
    const clearDynamicGroup = () => {
      for (const child of [...dynamicGroup.children]) {
        dynamicGroup.remove(child);
        if ("geometry" in child && child.geometry instanceof THREE.BufferGeometry) child.geometry.dispose();
        if ("material" in child) {
          const materials = Array.isArray(child.material) ? child.material : [child.material];
          materials.forEach((material) => material.dispose());
        }
      }
    };
    const updateDynamicScene = () => {
      const currentSessions = sessionsRef.current;
      const currentSessionsBySystem = sessionsBySystemRef.current;
      const currentSquadColors = squadColorBySessionRef.current;
      const knownSystemIds = new Set(systemsById.keys());

      for (const [systemIndex, system] of positioned.entries()) {
        new THREE.Color(nodeColor(system, currentSessionsBySystem)).toArray(starColors, systemIndex * 3);
      }
      starColorAttribute.needsUpdate = true;

      let routeCount = 0;
      let previewRouteCount = 0;
      for (const session of currentSessions) {
        const routeIds = activeRoutePath(session.location.system, session.location.activeRouteHops, knownSystemIds);
        const route = routeIds.map((id) => systemsById.get(id)).filter((system): system is PositionedSystem => Boolean(system));
        if (route.length >= 2) {
          const line = pooledLine(routePool, routeCount++, 0.92);
          updateRouteGeometry(line, route, 2.2);
          (line.material as THREE.LineBasicMaterial).color.set(currentSquadColors.get(session.sessionHandle) ?? 0xffffff);
        }

        const selectedId = routeSelectionMode ? selectedSystemIdRef.current : undefined;
        const origin = session.location.system;
        const hops = origin && selectedId ? selectedRouteHopsRef.current.get(origin) : undefined;
        const previewRoute = (origin && hops ? [origin, ...hops] : [])
          .map((id) => systemsById.get(id))
          .filter((system): system is PositionedSystem => Boolean(system));
        if (previewRoute.length >= 2) {
          const preview = pooledLine(previewRoutePool, previewRouteCount++, 0.68, 4);
          updateRouteGeometry(preview, previewRoute, 3.4);
          (preview.material as THREE.LineBasicMaterial).color.set(currentSquadColors.get(session.sessionHandle) ?? 0xf4d77c);
        }
      }
      for (let index = routeCount; index < routePool.length; index++) routePool[index]!.visible = false;
      for (let index = previewRouteCount; index < previewRoutePool.length; index++) previewRoutePool[index]!.visible = false;

      let pipCount = 0;
      for (const system of positioned) {
        const occupants = currentSessionsBySystem.get(system.id) ?? [];
        const position = worldPosition(system);
        occupants.forEach((session, index) => {
          const angle = (Math.PI * 2 * index) / Math.max(occupants.length, 1);
          const pip = pooledPip(pipCount++);
          (pip.material as THREE.SpriteMaterial).color.set(currentSquadColors.get(session.sessionHandle) ?? 0xffffff);
          pip.position.set(position.x + Math.cos(angle) * 7, position.y + 1.5, position.z + Math.sin(angle) * 7);
          pip.userData.revealDistance = Math.hypot(position.x, position.z);
          pip.userData.standardY = position.y + 1.5;
        });
      }
      for (let index = pipCount; index < pipPool.length; index++) pipPool[index]!.visible = false;
    };
    updateDynamicSceneRef.current = updateDynamicScene;
    updateDynamicScene();

    const revealKey = positioned
      .map((system) => system.id)
      .sort()
      .join("|");
    if (mapRevealRef.current?.key !== revealKey) {
      mapRevealRef.current = {
        key: revealKey,
        startedAt: performance.now(),
      };
    }
    const revealStartedAt = mapRevealRef.current.startedAt;
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const revealRadius = Math.max(...revealStars.map((star) => star.distanceFromCenter), 1);
    const revealWaveMaterial = new THREE.MeshBasicMaterial({
      color: 0x7de8f5,
      transparent: true,
      opacity: 0,
      side: THREE.DoubleSide,
      depthWrite: false,
    });
    const revealWave = new THREE.Mesh(new THREE.RingGeometry(0.965, 1, 96), revealWaveMaterial);
    revealWave.rotation.x = -Math.PI / 2;
    revealWave.position.y = 0.3;
    revealWave.visible = !reduceMotion;
    scene.add(revealWave);

    const resize = () => {
      const { width, height } = canvas.getBoundingClientRect();
      renderer.setSize(width, height, false);
      const aspect = width / Math.max(height, 1);
      if (camera instanceof THREE.PerspectiveCamera) {
        camera.aspect = aspect;
      } else {
        const halfHeight = 330;
        camera.left = -halfHeight * aspect;
        camera.right = halfHeight * aspect;
        camera.top = halfHeight;
        camera.bottom = -halfHeight;
      }
      camera.updateProjectionMatrix();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    resize();
    const pickPosition = new THREE.Vector3();
    const pickableSystems = positioned
      .filter((system) => selectablePois === null || !dimmedSystems.has(system.id))
      .map((system) => ({ id: system.id, position: worldPosition(system) }));
    const pickSystem = (clientX: number, clientY: number): PositionedSystem | undefined => {
      const bounds = canvas.getBoundingClientRect();
      const pointerX = clientX - bounds.left;
      const pointerY = clientY - bounds.top;
      const pickRadius = 14;
      let nearestSystem: PositionedSystem | undefined;
      let nearestDistanceSquared = pickRadius * pickRadius;
      for (const pickable of pickableSystems) {
        pickPosition.copy(pickable.position).project(camera);
        if (pickPosition.z < -1 || pickPosition.z > 1) continue;
        const screenX = (pickPosition.x * 0.5 + 0.5) * bounds.width;
        const screenY = (-pickPosition.y * 0.5 + 0.5) * bounds.height;
        const dx = screenX - pointerX;
        const dy = screenY - pointerY;
        const distanceSquared = dx * dx + dy * dy;
        if (distanceSquared >= nearestDistanceSquared) continue;
        nearestDistanceSquared = distanceSquared;
        nearestSystem = systemsById.get(pickable.id);
      }
      return nearestSystem;
    };
    let pendingPointer: { clientX: number; clientY: number } | null = null;
    let pickFrame = 0;
    const positionTooltip = (bounds: DOMRect, clientX: number, clientY: number) => {
      const tooltip = tooltipRef.current;
      if (!tooltip) return;
      tooltip.style.left = `${clientX - bounds.left + 14}px`;
      tooltip.style.top = `${clientY - bounds.top + 14}px`;
    };
    const processPointerMove = () => {
      pickFrame = 0;
      const pending = pendingPointer;
      pendingPointer = null;
      if (!pending || cameraInteractionRef.current) return;
      const bounds = canvas.getBoundingClientRect();
      positionTooltip(bounds, pending.clientX, pending.clientY);
      const system = pickSystem(pending.clientX, pending.clientY);
      if (!system) {
        if (hoveredSystemIdRef.current !== null) {
          hoveredSystemIdRef.current = null;
          setSystemHover(null);
        }
        return;
      }
      if (hoveredSystemIdRef.current === system.id) return;
      hoveredSystemIdRef.current = system.id;
      setSystemHover({
        x: pending.clientX - bounds.left,
        y: pending.clientY - bounds.top,
        name: systemLabel(system),
        color: normalizeEmpire(system.empire) ? EMPIRE_COLORS[normalizeEmpire(system.empire)!] : "#b9f5ff",
        bots: (sessionsBySystemRef.current.get(system.id) ?? []).map((session) => ({
          name: session.username ?? session.sessionHandle,
          color: squadColorBySessionRef.current.get(session.sessionHandle) ?? "#ffffff",
        })),
      });
    };
    const handlePointerMove = (event: globalThis.PointerEvent) => {
      if (cameraInteractionRef.current) return;
      pendingPointer = { clientX: event.clientX, clientY: event.clientY };
      if (!pickFrame) pickFrame = requestAnimationFrame(processPointerMove);
    };
    const handlePointerLeave = () => {
      pendingPointer = null;
      if (pickFrame) cancelAnimationFrame(pickFrame);
      pickFrame = 0;
      if (hoveredSystemIdRef.current !== null) {
        hoveredSystemIdRef.current = null;
        setSystemHover(null);
      }
    };
    const handlePointerDownForSelection = (event: globalThis.PointerEvent) => {
      selectPointerRef.current = { x: event.clientX, y: event.clientY };
    };
    const handlePointerUpForSelection = (event: globalThis.PointerEvent) => {
      const start = selectPointerRef.current;
      selectPointerRef.current = null;
      if (!start || Math.hypot(event.clientX - start.x, event.clientY - start.y) > 5) return;
      const system = pickSystem(event.clientX, event.clientY);
      if (system) selectSystem(system.id);
    };
    canvas.addEventListener("pointermove", handlePointerMove);
    canvas.addEventListener("pointerleave", handlePointerLeave);
    canvas.addEventListener("pointerdown", handlePointerDownForSelection);
    canvas.addEventListener("pointerup", handlePointerUpForSelection);
    let frame = 0;
    const render = () => {
      const now = performance.now();
      const elapsed = now / 1000;
      const revealProgress = reduceMotion ? 1 : Math.min(1, Math.max(0, (now - revealStartedAt - HOLOGRAM_REVEAL_DELAY_MS) / HOLOGRAM_REVEAL_DURATION_MS));
      const easedReveal = 1 - Math.pow(1 - revealProgress, 3);
      const waveRadius = -HOLOGRAM_WAVE_FEATHER + easedReveal * (revealRadius + HOLOGRAM_WAVE_FEATHER * 2);
      for (const star of revealStars) {
        const alphaProgress = THREE.MathUtils.smoothstep(waveRadius - star.distanceFromCenter, 0, HOLOGRAM_WAVE_FEATHER);
        for (const { material, standardOpacity } of star.materials) {
          material.opacity = standardOpacity * alphaProgress;
        }
      }
      for (const pip of pipPool) {
        if (!pip.visible) continue;
        const alphaProgress = THREE.MathUtils.smoothstep(waveRadius - (pip.userData.revealDistance as number), 0, HOLOGRAM_WAVE_FEATHER);
        (pip.material as THREE.SpriteMaterial).opacity = alphaProgress;
        pip.position.y = (pip.userData.standardY as number) + Math.sin(alphaProgress * Math.PI) * HOLOGRAM_WAVE_LIFT;
      }
      connectionWaveRadius.value = waveRadius;
      stemWaveRadius.value = waveRadius;
      planeWaveRadius.value = waveRadius;
      starWaveRadius.value = waveRadius;
      for (const line of routePool) {
        if (line.visible) (line.userData.waveRadius as { value: number }).value = waveRadius;
      }
      for (const line of previewRoutePool) {
        if (line.visible) (line.userData.waveRadius as { value: number }).value = waveRadius;
      }
      for (const line of revealLines) {
        const alphaProgress =
          line.endpointDistances.reduce((sum, distance) => sum + THREE.MathUtils.smoothstep(waveRadius - distance, 0, HOLOGRAM_WAVE_FEATHER), 0) /
          line.endpointDistances.length;
        line.material.opacity = line.standardOpacity * alphaProgress;
      }
      revealWave.visible = !reduceMotion && revealProgress > 0 && revealProgress < 1;
      if (revealWave.visible) {
        const visibleWaveRadius = Math.max(1, waveRadius);
        revealWave.scale.set(visibleWaveRadius, visibleWaveRadius, 1);
        revealWaveMaterial.opacity = Math.sin(revealProgress * Math.PI) * 0.62;
      }
      for (const orbiter of stationOrbiters) {
        const angle = orbiter.phase + elapsed * 0.75;
        orbiter.mesh.position.set(orbiter.center.x + Math.cos(angle) * 4, orbiter.center.y, orbiter.center.z + Math.sin(angle) * 4);
      }
      controls.update();
      if (projectionMode === "perspective" && controls.target.y !== 0) {
        camera.position.y -= controls.target.y;
        controls.target.y = 0;
        controls.update();
      }
      orbitReticle.position.x = controls.target.x;
      orbitReticle.position.z = controls.target.z;
      renderer.render(scene, camera);
      frame = requestAnimationFrame(render);
    };
    render();

    return () => {
      cameraStateRef.current[projectionMode] = {
        position: camera.position.clone(),
        target: controls.target.clone(),
        zoom: camera.zoom,
      };
      cancelAnimationFrame(frame);
      if (pickFrame) cancelAnimationFrame(pickFrame);
      observer.disconnect();
      canvas.removeEventListener("pointermove", handlePointerMove);
      canvas.removeEventListener("pointerleave", handlePointerLeave);
      canvas.removeEventListener("pointerdown", handlePointerDownForSelection);
      canvas.removeEventListener("pointerup", handlePointerUpForSelection);
      setSystemHover(null);
      controls.removeEventListener("start", handleInteractionStart);
      controls.removeEventListener("end", handleInteractionEnd);
      controls.dispose();
      if (updateDynamicSceneRef.current === updateDynamicScene) updateDynamicSceneRef.current = null;
      clearDynamicGroup();
      pipTexture.dispose();
      starTexture.dispose();
      scene.traverse((object) => {
        if (object instanceof THREE.Mesh || object instanceof THREE.Line || object instanceof THREE.Sprite) {
          if ("geometry" in object) object.geometry.dispose();
          const materials = Array.isArray(object.material) ? object.material : [object.material];
          materials.forEach((material) => material.dispose());
        }
      });
      renderer.dispose();
      cameraRef.current = null;
      controlsRef.current = null;
    };
  }, [
    edges,
    directSystemClickMode,
    effectiveSelectedSystemId,
    exploredSystems,
    highlightedSystems,
    dimmedSystems,
    mapLevel,
    normalizedSearch,
    positioned,
    projectionMode,
    routeSelectionMode,
    selectablePois,
    selectedSquadHandlesKey,
    visitedPois,
  ]);

  useEffect(() => {
    updateDynamicSceneRef.current?.();
  }, [effectiveSelectedSystemId, renderedOccupancyKey, selectedRouteHops]);

  function zoomBy(multiplier: number) {
    const camera = cameraRef.current;
    const controls = controlsRef.current;
    if (camera && controls) {
      if (camera instanceof THREE.OrthographicCamera) {
        camera.zoom = Math.min(DEFAULT_ORTHO_ZOOM * (1 + MAP_ZOOM_RANGE), Math.max(DEFAULT_ORTHO_ZOOM * (1 - MAP_ZOOM_RANGE), camera.zoom * multiplier));
        camera.updateProjectionMatrix();
      } else {
        camera.position
          .sub(controls.target)
          .multiplyScalar(1 / multiplier)
          .add(controls.target);
      }
      controls.update();
    }
  }

  function resetView() {
    const camera = cameraRef.current;
    const controls = controlsRef.current;
    if (camera && controls) {
      camera.up.set(0, projectionMode === "perspective" ? 1 : 0, projectionMode === "perspective" ? 0 : -1);
      camera.position.copy(projectionMode === "ortho" ? new THREE.Vector3(0, 780, 0) : DEFAULT_PERSPECTIVE_POSITION);
      if (camera instanceof THREE.OrthographicCamera) {
        camera.zoom = DEFAULT_ORTHO_ZOOM;
        camera.updateProjectionMatrix();
      }
      controls.target.set(0, 0, 0);
      controls.update();
    }
  }

  function toggleProjectionMode() {
    const nextMode = projectionMode === "ortho" ? "perspective" : "ortho";
    cameraStateRef.current[nextMode] = null;
    setProjectionMode(nextMode);
  }

  return (
    <section className={`galaxy-map-panel galaxy-map-panel--${variant}`}>
      <div className="galaxy-map-toolbar">
        <div>
          <div className="galaxy-map-title">galaxy map</div>
          <div className="galaxy-map-meta">{mapData ? `${mapData.systems.length} systems / ${edges.length} lanes` : "waiting for map data"}</div>
        </div>
        <div className="galaxy-map-controls" aria-label="Galaxy map controls">
          {!embedded && (
            <input
              className="galaxy-map-search"
              type="search"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search systems..."
              aria-label="Search systems"
            />
          )}
          <button type="button" onClick={() => zoomBy(1.2)} title="Zoom in" aria-label="Zoom in">
            +
          </button>
          <button type="button" onClick={() => zoomBy(1 / 1.2)} title="Zoom out" aria-label="Zoom out">
            -
          </button>
          <button type="button" onClick={resetView} title="Reset view" aria-label="Reset view">
            1:1
          </button>
          <button
            type="button"
            onClick={toggleProjectionMode}
            title={projectionMode === "ortho" ? "Switch to perspective 3D" : "Switch to orthographic 2D"}
            aria-label={projectionMode === "ortho" ? "Switch to perspective 3D" : "Switch to orthographic 2D"}
          >
            {projectionMode === "ortho" ? "3D" : "2D"}
          </button>
        </div>
      </div>

      <div className="galaxy-map-canvas">
        {mapLevel === "system" && (
          <button className="system-map-back" type="button" onClick={() => setMapLevel("galaxy")}>
            <span aria-hidden="true">←</span> Back to galaxy
          </button>
        )}
        {selectedSystem && (mapLevel === "system" || routeSelectionMode) && (
          <aside className={`galaxy-map-readout${readoutCollapsed ? " galaxy-map-readout--collapsed" : ""}`} aria-label="Selected system readout">
            <button
              className="galaxy-map-readout-toggle"
              type="button"
              onClick={() => setReadoutCollapsed((collapsed) => !collapsed)}
              aria-expanded={!readoutCollapsed}
              title={readoutCollapsed ? "Expand system readout" : "Collapse system readout"}
            >
              <span>{systemLabel(selectedSystem)}</span>
              <span aria-hidden="true">{readoutCollapsed ? "+" : "−"}</span>
            </button>
            {!readoutCollapsed && (
              <div className="galaxy-map-readout-body">
                <dl>
                  <div>
                    <dt>system</dt>
                    <dd>{selectedSystem.id}</dd>
                  </div>
                  <div>
                    <dt>empire</dt>
                    <dd>{selectedSystem.empire || "unaffiliated"}</dd>
                  </div>
                  <div>
                    <dt>coordinates</dt>
                    <dd>
                      {selectedSystem.x ?? "?"}, {selectedSystem.y ?? "?"}
                    </dd>
                  </div>
                  <div>
                    <dt>jump lanes</dt>
                    <dd>{selectedSystem.connections.length}</dd>
                  </div>
                  <div>
                    <dt>known POIs</dt>
                    <dd>{selectedSystemPois.length}</dd>
                  </div>
                  <div>
                    <dt>star</dt>
                    <dd>{selectedSystemStar?.name || selectedSystemStar?.id || "unknown"}</dd>
                  </div>
                </dl>
              </div>
            )}
          </aside>
        )}
        {loading && <div className="galaxy-map-empty">Loading galaxy graph…</div>}
        {!loading && error && <div className="galaxy-map-empty galaxy-map-empty--error">{error}</div>}
        {!loading && !error && !mapData && <div className="galaxy-map-empty">no sessions to chart</div>}
        {!loading && !error && mapData && positioned.length === 0 && <div className="galaxy-map-empty">no systems found in map data</div>}
        {!loading && !error && positioned.length > 0 && mapLevel === "galaxy" && (
          <canvas ref={threeCanvasRef} className="galaxy-map-canvas3d" role="img" aria-label="Interactive perspective-projected 3D galaxy trade map" />
        )}
        {systemHover && (
          <div ref={tooltipRef} className="galaxy-map-tooltip" style={{ left: systemHover.x + 14, top: systemHover.y + 14 }}>
            <div className="galaxy-map-tooltip-title" style={{ color: systemHover.color }}>
              {systemHover.name}
            </div>
            {systemHover.bots.map((bot, index) => (
              <div className="galaxy-map-tooltip-bot" style={{ color: bot.color }} key={`${bot.name}-${index}`}>
                {bot.name}
              </div>
            ))}
          </div>
        )}
        {!loading && !error && selectedSystem && mapLevel === "system" && (
          <>
            <canvas
              ref={systemCanvasRef}
              className="system-map-canvas"
              role="img"
              aria-label={`${systemLabel(selectedSystem)} system map centered on ${selectedSystemStar?.name || selectedSystemStar?.id || "an unknown star"}, with ${systemMapPois.length} orbiting points of interest`}
            />
            <div ref={systemPoiLabelsRef} className="system-map-poi-labels" aria-hidden="true">
              {systemMapPois.map((poi) => (
                <span key={poi.id}>{poi.name || poi.id}</span>
              ))}
            </div>
            <div className="system-map-center-label">{selectedSystemStar?.name || selectedSystemStar?.id || systemLabel(selectedSystem)}</div>
            {systemMapPois.length === 0 && <div className="system-map-empty">No known POIs in this system</div>}
          </>
        )}
      </div>
    </section>
  );
}

export const GalaxyMapViewport = memo(
  GalaxyMapViewportComponent,
  (previous, next) =>
    galaxySessionRenderKey(previous.sessions) === galaxySessionRenderKey(next.sessions) &&
    previous.map === next.map &&
    previous.exploration === next.exploration &&
    previous.loading === next.loading &&
    previous.error === next.error &&
    previous.variant === next.variant &&
    previous.squads === next.squads &&
    previous.selectedSystemId === next.selectedSystemId &&
    previous.highlightedSystemIds === next.highlightedSystemIds &&
    previous.dimmedSystemIds === next.dimmedSystemIds &&
    previous.selectablePoiIds === next.selectablePoiIds &&
    previous.onSystemClick === next.onSystemClick &&
    previous.onSelectSystem === next.onSelectSystem,
);

export default function GalaxyMap({ sessions }: GalaxyMapProps) {
  const prayer = usePrayer();
  const squads = useSquads();
  const map = selectGalaxyMap(prayer.galaxyMap);
  const graphLoading = !prayer.error && (prayer.connection === "connecting" || !map || map.systems.length === 0);
  return (
    <GalaxyMapViewport
      sessions={sessions}
      map={map}
      exploration={selectGalaxyExploration(prayer.galaxyExploration)}
      loading={graphLoading}
      error={prayer.error?.message ?? null}
      variant="panel"
      squads={squads}
    />
  );
}
