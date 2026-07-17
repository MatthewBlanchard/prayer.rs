import type {
  FleetEntry as ContractFleetEntry,
  AgentSightingData,
  ChatMessageData,
  FactionSnapshotData,
  FacilityResponse,
  GalaxyCatalog as GeneratedGalaxyCatalog,
  GalaxyWildlife as GeneratedGalaxyWildlife,
  SalvageData,
  SpaceLootInfo,
  GalaxyMap,
  GalaxyResources,
  PoiFacilitiesSnapshot as ContractPoiFacilitiesSnapshot,
  PassengerState,
  StationMarkets,
  StorageByOwner,
} from "@prayer/sdk/types";

// Deliberately UI-local projections for evolving game payloads. These are not
// Prayer SDK contract aliases; selectors below normalize them for rendering.
export type AgentSightingWire = AgentSightingData;
export type ChatMessageWire = ChatMessageData;
export type FactionSnapshotWire = FactionSnapshotData;
export interface FacilityWire {
  facility_id: string;
  type?: string | null;
  definition_id?: string | null;
  custom_name?: string | null;
  name?: string | null;
  category?: string | null;
  level?: number | null;
  status?: string | null;
  under_construction?: boolean | null;
  owner_id?: string | null;
  faction_id?: string | null;
  rent_per_cycle?: number | null;
  production?: { public?: boolean | null } | null;
}
export interface FacilityResponseWire {
  station_facilities?: FacilityWire[];
  player_facilities?: FacilityWire[];
  public_facilities?: FacilityWire[];
  faction_facilities?: FacilityWire[];
  facilities?: FacilityWire[];
  total_rent_per_cycle?: number | null;
  arrears_owed?: number | null;
}
export type PassengerBoardWire = PassengerState;
export type SalvageLootableWire = SpaceLootInfo;
export type SalvageStateWire = SalvageData;
export type FleetEntry = ContractFleetEntry;
export type GalaxyCatalog = GeneratedGalaxyCatalog;
export type GalaxyWildlife = GeneratedGalaxyWildlife;
export type PoiFacilitiesSnapshot = ContractPoiFacilitiesSnapshot;
import type {
  ArbitradePackage,
  ArbitrageDeal,
  CatalogState,
  CharacterSkillInfo,
  CharacterSkillsState,
  CommanderStorageRow,
  CommanderStorageView,
  EconomyArbitrageData,
  EconomyMarketData,
  FacilitiesData,
  FacilityInfo,
  FacilityOwnerKind,
  FacilityTypeInfo,
  FactionGarageShipInfo,
  FactionInfo,
  GalaxyExplorationData,
  GalaxyMapData,
  GalaxyResourcesData,
  GameChatMessage,
  PassengerInfo,
  PassengerSessionResult,
  SalvageLootable,
  SalvageSessionState,
  ShipyardData,
  ShipyardFleetData,
  OwnedShipInfo,
  SocialBot,
  StorageSessionState,
  WildlifeState,
} from "../api.js";

export function selectGalaxyMap(map: GalaxyMap | null): GalaxyMapData | null {
  if (!map) return null;
  return {
    systems: map.systems.map((system) => ({
      ...system,
      name: system.name ?? undefined,
      x: system.x ?? null,
      y: system.y ?? null,
      poiCount: system.poiCount ?? null,
      firstEnteredUnix: system.firstEnteredUnix ?? null,
      lastEnteredUnix: system.lastEnteredUnix ?? null,
      lastScannedUnix: system.lastScannedUnix ?? null,
      lastSurveyedUnix: system.lastSurveyedUnix ?? null,
      bloomStatus: system.bloomStatus ?? null,
      bloomIntensity: system.bloomIntensity ?? null,
      pois: system.pois.map((poi) => ({ ...poi, x: poi.x ?? null, y: poi.y ?? null })),
    })),
    knownPois: map.knownPois.map((poi) => ({ ...poi, x: poi.x ?? null, y: poi.y ?? null })),
  };
}

export function selectGalaxyExploration(exploration: GalaxyExplorationData | null): GalaxyExplorationData | null {
  return exploration;
}

export function selectGalaxyResources(resources: GalaxyResources | null): GalaxyResourcesData | null {
  return resources;
}

export function selectCatalog(catalog: GalaxyCatalog | null): CatalogState | null {
  if (!catalog) return null;
  const ingredients = (rows: Array<{ item_id: string; quantity: number }>) =>
    rows.map((row) => ({
      itemId: row.item_id,
      item: row.item_id,
      id: row.item_id,
      name: catalog.itemsById[row.item_id]?.name ?? row.item_id,
      quantity: row.quantity,
      amount: null,
      count: null,
    }));
  return {
    sessionId: null,
    stateVersion: null,
    username: null,
    items: Object.values(catalog.itemsById).map((item) => ({
      id: item.id,
      name: item.name,
      classId: "",
      className: "",
      category: "category" in item ? item.category : "module",
      typeName: "type" in item ? item.type : "item",
      tier: null,
      scale: null,
      size: item.size,
      hull: null,
      baseHull: null,
      shield: null,
      baseShield: null,
      cargo: null,
      cargoCapacity: "cargo_bonus" in item ? (item.cargo_bonus ?? null) : null,
      speed: "speed_bonus" in item ? (item.speed_bonus ?? null) : null,
      baseSpeed: null,
      price: item.base_value,
      materials: {},
      ingredients: [],
      inputs: [],
      outputs: [],
      requiredSkills: "required_skills" in item ? (item.required_skills ?? {}) : {},
      recipeIds: [],
      source: item,
    })),
    ships: Object.values(catalog.shipsById).map((ship) => ({
      id: ship.id,
      name: ship.name,
      classId: ship.id,
      className: ship.class,
      category: ship.category ?? "",
      typeName: ship.class,
      tier: ship.tier ?? null,
      scale: ship.scale ?? null,
      size: null,
      hull: ship.base_hull ?? null,
      baseHull: ship.base_hull ?? null,
      shield: ship.base_shield ?? null,
      baseShield: ship.base_shield ?? null,
      cargo: ship.cargo_capacity ?? null,
      cargoCapacity: ship.cargo_capacity ?? null,
      speed: ship.base_speed ?? null,
      baseSpeed: ship.base_speed ?? null,
      price: null,
      materials: ship.build_materials ?? {},
      ingredients: [],
      inputs: [],
      outputs: [],
      requiredSkills: {},
      recipeIds: [],
      cpuCapacity: ship.cpu_capacity ?? null,
      powerCapacity: ship.power_capacity ?? null,
      weaponSlots: ship.weapon_slots ?? null,
      defenseSlots: ship.defense_slots ?? null,
      utilitySlots: ship.utility_slots ?? null,
      defaultModules: ship.default_modules ?? [],
      inherentCapabilities: ship.inherent_capabilities ?? [],
      source: ship,
    })),
    recipes: Object.values(catalog.recipesById).map((recipe) => ({
      id: recipe.id,
      name: recipe.name,
      inputs: ingredients(recipe.inputs),
      outputs: ingredients(recipe.outputs),
      requiredSkills: {},
      requiredFacilityTypes: recipe.facility_only ? ["facility"] : [],
      source: recipe,
    })),
    facilities: Object.values(catalog.facilitiesById).map((facility) => ({
      id: facility.id,
      name: facility.name,
      classId: "",
      className: "",
      category: facility.category,
      typeName: facility.service_type ?? "",
      tier: facility.level,
      scale: null,
      size: null,
      hull: null,
      baseHull: null,
      shield: null,
      baseShield: null,
      cargo: null,
      cargoCapacity: null,
      speed: null,
      baseSpeed: null,
      price: facility.build_cost,
      materials: Object.fromEntries((facility.build_materials ?? []).map((row) => [row.item_id, row.quantity])),
      ingredients: ingredients(facility.build_materials ?? []),
      inputs: [],
      outputs: [],
      requiredSkills: {},
      recipeIds: facility.recipe_id ? [facility.recipe_id] : [],
      source: facility,
    })),
  };
}

export function selectCharacterSkills(bot: FleetEntry | null, catalog: GalaxyCatalog | null): CharacterSkillsState | null {
  if (!bot) return null;
  const state = bot.state;
  const catalogView = catalog;
  const skills: CharacterSkillInfo[] = Object.entries(state.skills ?? {}).map(([id, skillValue]) => {
    const skill = skillValue;
    const definition = catalogView?.skillsById[id];
    const name =
      typeof definition?.name === "string" && definition.name.trim()
        ? definition.name
        : id.replace(/[_-]+/g, " ").replace(/\b\w/g, (character) => character.toUpperCase());
    const category = typeof definition?.category === "string" ? definition.category : null;
    return {
      id,
      name,
      category,
      level: skill.level ?? null,
      maxLevel: skill.max_level ?? null,
      xp: skill.xp ?? null,
      nextLevelXp: skill.next_level_xp ?? null,
    };
  });
  skills.sort(
    (left, right) =>
      (left.category ?? "").localeCompare(right.category ?? "") || (right.level ?? -1) - (left.level ?? -1) || left.name.localeCompare(right.name),
  );
  return { sessionId: bot.id, stateVersion: bot.version, username: bot.username ?? null, skills };
}

function facilityRows(response: FacilityResponse | null | undefined): FacilityWire[] {
  if (!response) return [];
  return [
    "station_facilities" in response ? response.station_facilities : [],
    "player_facilities" in response ? response.player_facilities : [],
    "public_facilities" in response ? (response.public_facilities ?? []) : [],
    "faction_facilities" in response ? response.faction_facilities : [],
    "facilities" in response ? response.facilities : [],
  ].flat();
}

function selectFacilityRow(
  row: FacilityWire,
  ownerKind: FacilityOwnerKind,
  ownerName: string,
  locationId: string,
  locationName: string,
  systemId: string,
): FacilityInfo {
  return {
    facilityId: row.facility_id,
    facilityType: row.type ?? row.definition_id ?? "",
    name: row.custom_name ?? row.name ?? "",
    category: row.category ?? "",
    level: row.level ?? null,
    status: row.status ?? (row.under_construction ? "under construction" : ""),
    ownerKind,
    ownerName: row.owner_id ?? row.faction_id ?? ownerName,
    locationId,
    locationName,
    systemId,
    buildTime: null,
    rentPerCycle: row.rent_per_cycle ?? null,
    public: row.production?.public ?? null,
  };
}

export function selectFacilities(
  bot: FleetEntry | null,
  map: GalaxyMap | null,
  catalog: GalaxyCatalog | null,
  facilitiesByPoi: Record<string, PoiFacilitiesSnapshot>,
  ownedByPlayer: Record<string, FacilityResponse>,
  ownedByFaction: Record<string, FacilityResponse>,
): FacilitiesData | null {
  if (!bot) return null;
  const poiId = bot.state.location.poi_id ?? "";
  const poi = map?.knownPois.find((candidate) => candidate.id === poiId);
  const playerId = typeof bot.state.player.id === "string" ? bot.state.player.id : bot.id;
  const factionId = typeof bot.state.player.faction_id === "string" ? bot.state.player.faction_id : null;
  const currentSnapshot = facilitiesByPoi[poiId];
  const currentRows = facilityRows(currentSnapshot?.current).filter((row) => !row.faction_id);
  const current = currentRows.map((row) =>
    selectFacilityRow(
      row,
      row.owner_id ? "personal" : "other",
      bot.username ?? bot.id,
      poiId,
      poi?.name ?? poiId,
      poi?.systemId ?? bot.state.location.system_id ?? "",
    ),
  );
  const factionCurrent = facilityRows(currentSnapshot?.faction_current).map((row) =>
    selectFacilityRow(row, "faction", factionId ?? "", poiId, poi?.name ?? poiId, poi?.systemId ?? bot.state.location.system_id ?? ""),
  );
  const ownedResponse = ownedByPlayer[playerId] ?? ownedByPlayer[bot.id] ?? (bot.username ? ownedByPlayer[bot.username] : undefined);
  const factionResponse = factionId ? ownedByFaction[factionId] : undefined;
  const owned = facilityRows(ownedResponse).map((row) => selectFacilityRow(row, "personal", bot.username ?? playerId, "", "", ""));
  const factionOwned = facilityRows(factionResponse).map((row) => selectFacilityRow(row, "faction", factionId ?? "", "", "", ""));
  const types: FacilityTypeInfo[] = Object.entries(catalog?.facilitiesById ?? {}).map(([id, value]) => {
    const definition = value;
    return {
      facilityType: typeof definition.id === "string" ? definition.id : id,
      name: typeof definition.name === "string" ? definition.name : "",
      category: typeof definition.category === "string" ? definition.category : "",
      level: typeof definition.level === "number" ? definition.level : null,
      upgradesFrom: typeof definition.upgrades_from === "string" ? definition.upgrades_from : "",
      price: typeof definition.build_cost === "number" ? definition.build_cost : null,
      buildTime: typeof definition.build_time === "number" ? definition.build_time : null,
      rentPerCycle: null,
      requiredSkills: {},
      requiredItems: Array.isArray(definition.build_materials)
        ? Object.fromEntries(
            definition.build_materials.flatMap((entry) =>
              entry && typeof entry === "object" && typeof entry.item_id === "string" && typeof entry.quantity === "number"
                ? [[entry.item_id, entry.quantity]]
                : [],
            ),
          )
        : {},
      recipeId: definition.recipe_id ?? null,
    };
  });
  return {
    sessionId: bot.id,
    username: bot.username ?? null,
    latestSystem: bot.state.location.system_id ?? null,
    latestPoi: poiId || null,
    docked: typeof bot.state.location.docked_at === "string" ? true : null,
    current,
    publicFacilities: (currentSnapshot?.current && "public_facilities" in currentSnapshot.current ? (currentSnapshot.current.public_facilities ?? []) : []).map(
      (row) => selectFacilityRow(row, "other", "", poiId, poi?.name ?? poiId, poi?.systemId ?? ""),
    ),
    owned,
    factionCurrent,
    factionOwned,
    factionId,
    factionRentPerCycle: factionResponse && "total_rent_per_cycle" in factionResponse ? factionResponse.total_rent_per_cycle : null,
    factionArrearsOwed: factionResponse && "arrears_owed" in factionResponse ? (factionResponse.arrears_owed ?? null) : null,
    types,
    errors: [],
  };
}

export function selectShipyard(bot: FleetEntry | null): ShipyardData | null {
  if (!bot) return null;
  const { state } = bot;
  const owner = bot.username ?? state.player.id ?? bot.id;
  const ownedShips: OwnedShipInfo[] = state.owned_ship_details.map((ship) => ({
    owner,
    ownerHandle: bot.username ?? bot.id,
    shipId: ship.ship_id,
    classId: ship.class_id,
    location: ship.location ?? "",
    locationBaseId: ship.location_base_id ?? "",
    ownerKind: "personal",
    ownerId: state.player.id ?? bot.id,
    ownerName: bot.username ?? "",
    factionId: state.player.faction_id ?? "",
    factionTag: "",
    active: ship.is_active,
    isActive: ship.is_active,
    isGaraged: !ship.is_active,
    className: ship.class_name ?? "",
    customName: ship.custom_name ?? "",
    fuel: ship.fuel ?? "",
    hull: ship.hull ?? "",
    cargoUsed: ship.cargo_used ?? null,
    modules: ship.modules ?? null,
    listingId: ship.listing_id ?? "",
    listingBaseId: ship.listing_base_id ?? "",
    listingPrice: ship.listing_price ?? null,
  }));
  return {
    sessionId: bot.id,
    stateVersion: bot.version,
    factionId: state.player.faction_id ?? null,
    currentBaseId: state.location.docked_at ?? null,
    currentBaseName: state.location.docked_at ?? null,
    docked: state.location.docked_at != null,
    activeShip: state.ship,
    installedModules:
      state.modules.length > 0
        ? state.modules.flatMap((module) => (module.type_id ? [module.type_id] : []))
        : state.installed_modules,
    installedModuleNames: Object.fromEntries(state.modules.flatMap((module) => (module.type_id && module.name ? [[module.type_id, module.name]] : []))),
    ships: ownedShips,
    ownedShips,
    factionGarage: { used: null, capacity: null, ships: [] },
    inProgressCommissions: state.active_commissions.map((commission) => ({
      commissionId: commission.commission_id,
      shipClass: commission.ship_class_id,
      shipClassName: commission.ship_name ?? "",
      status: commission.status,
      baseId: commission.base_id ?? "",
      baseName: commission.base_name ?? "",
      progress:
        commission.build_start_tick != null && commission.build_complete_tick != null && commission.build_complete_tick > commission.build_start_tick
          ? Math.max(
              0,
              Math.min(
                100,
                ((commission.build_complete_tick - (commission.ticks_remaining ?? 0) - commission.build_start_tick) /
                  (commission.build_complete_tick - commission.build_start_tick)) *
                  100,
              ),
            )
          : null,
      ticksRemaining: commission.ticks_remaining ?? null,
      totalCost: commission.credits_paid ?? null,
      provideMaterials: commission.materials_provided,
      source: commission,
    })),
    shipyardShowroom: [],
  };
}

export function selectShipyardFleet(fleet: FleetEntry[]): ShipyardFleetData {
  const yards = fleet.map(selectShipyard).filter((yard): yard is ShipyardData => yard !== null);
  const ownedShips = yards.flatMap((yard) => yard.ownedShips);
  const factionGarageShips = yards.flatMap((yard) => yard.factionGarage.ships);
  const garageById = new Map<string, FactionGarageShipInfo>();
  for (const ship of factionGarageShips) if (ship.shipId) garageById.set(ship.shipId, ship);
  const ownedById = new Map(ownedShips.filter((ship) => ship.shipId).map((ship) => [ship.shipId, ship]));
  const garageRows = [...garageById.values()];
  const ships = [...ownedById.values(), ...yards.flatMap((yard) => yard.ships).filter((ship) => ship.ownerKind === "faction_garage")];
  return {
    stateVersion: fleet.length ? Math.max(...fleet.map((bot) => bot.version)) : null,
    knowledgeVersion: null,
    sessionsObserved: fleet.length,
    sessionsTotal: fleet.length,
    ships,
    ownedShips: [...ownedById.values()],
    factionGarageShips: garageRows,
  };
}

export function selectStorageState(
  bot: FleetEntry | null,
  storageByPlayer: StorageByOwner,
  factionStorageByFactionPoi: StorageByOwner,
): StorageSessionState | null {
  if (!bot) return null;
  const playerId = typeof bot.state.player.id === "string" ? bot.state.player.id : bot.id;
  const factionId = typeof bot.state.player.faction_id === "string" ? bot.state.player.faction_id : null;
  const factionByPoi = factionId ? (factionStorageByFactionPoi[factionId] ?? {}) : {};
  return {
    sessionId: bot.id,
    stateVersion: bot.version,
    username: bot.username ?? null,
    factionId,
    cargo: bot.state.cargo,
    storageByPoi: storageByPlayer[playerId] ?? storageByPlayer[bot.id] ?? {},
    factionStorage: Object.values(factionByPoi).reduce<Record<string, number>>((all, items) => {
      for (const [id, quantity] of Object.entries(items)) all[id] = (all[id] ?? 0) + quantity;
      return all;
    }, {}),
  };
}

export function selectWildlifeStates(fleet: FleetEntry[], wildlife: GalaxyWildlife | null): Array<{ handle: string; state: WildlifeState }> {
  if (!wildlife) return [];
  return fleet.map((bot) => {
    const system = bot.state.location.system_id ?? null;
    const poi = bot.state.location.poi_id ?? null;
    const nearby = poi ? (wildlife.pois.find((item) => item.poiId === poi)?.creatures ?? []) : [];
    return {
      handle: bot.username ?? bot.id,
      state: {
        sessionId: bot.id,
        stateVersion: bot.version,
        username: bot.username ?? null,
        currentSystem: system,
        currentPoi: poi,
        nearbyCreatureCount: nearby.length,
        nearbyCreatures: nearby,
        systems: wildlife.systems,
        pois: wildlife.pois,
      },
    };
  });
}

type PassengerWire = NonNullable<FleetEntry["state"]["passengers"]["aboard"]>[number] | NonNullable<PassengerState["waiting"]>[number];
function selectPassenger(passenger: PassengerWire): PassengerInfo {
  return {
    citizenId: passenger.citizen_id ?? "",
    name: passenger.name ?? "",
    bio: passenger.bio ?? "",
    className: passenger.class ?? "",
    citizenship: "citizenship" in passenger ? (passenger.citizenship ?? "") : "",
    destination: passenger.destination ?? "",
    destinationName: passenger.destination_name ?? "",
    destinationSystem: passenger.destination_system ?? "",
    baseFare: "base_fare" in passenger ? (passenger.base_fare ?? null) : null,
    estimatedFare: "estimated_fare" in passenger ? (passenger.estimated_fare ?? null) : null,
    speedBonus: "speed_bonus" in passenger ? (passenger.speed_bonus ?? null) : null,
    ticksRemaining: "ticks_remaining" in passenger ? (passenger.ticks_remaining ?? null) : null,
  };
}

export function selectPassengerStates(
  fleet: FleetEntry[],
  map: GalaxyMap | null,
  stationPassengers: Record<string, PassengerBoardWire> = {},
): PassengerSessionResult[] {
  const poiNames = new Map(map?.knownPois.map((poi) => [poi.id, poi.name]) ?? []);
  return fleet.map((bot) => {
    const passengers = bot.state.passengers;
    const currentPoi = bot.state.location.poi_id ?? null;
    const board = currentPoi ? stationPassengers[currentPoi] : undefined;
    return {
      handle: bot.username?.trim() || bot.id,
      error: null,
      state: {
        sessionId: bot.id,
        stateVersion: bot.version,
        username: bot.username ?? null,
        system: bot.state.location.system_id ?? null,
        currentPoi,
        currentPoiName: currentPoi ? (poiNames.get(currentPoi) ?? null) : null,
        aboardCount: passengers.aboard_count ?? null,
        economyBerths: passengers.economy_berths,
        businessBerths: passengers.business_berths,
        firstBerths: passengers.first_berths,
        aboard: (passengers.aboard ?? []).map(selectPassenger),
        station: board?.station ?? "",
        waitingCount: board?.waiting_count ?? null,
        waiting: (board?.waiting ?? []).map(selectPassenger),
      },
    };
  });
}

function selectSalvageLootable(value: SalvageLootableWire): SalvageLootable {
  return {
    id: value.id,
    kind: value.kind,
    poiId: value.poi_id,
    systemId: value.system_id,
    cargo: (value.cargo ?? [])
      .map((item) => ({ itemId: item.item_id ?? "", name: item.name ?? item.item_id ?? "", quantity: item.quantity ?? 0, size: item.size ?? null }))
      .filter((item) => item.itemId && item.quantity > 0),
    modules: (value.modules ?? []).map((module) => ({
      id: module.id ?? "",
      typeId: module.type_id ?? "",
      name: module.name ?? module.type_id ?? module.id ?? "",
      moduleType: module.type ?? "",
      wear: module.wear ?? null,
    })),
    salvageValue: value.salvage_value ?? null,
    createdAt: value.created_at ?? null,
    expiresAt: value.expires_at ?? null,
    expireTick: value.expire_tick ?? null,
    shipClass: value.ship_class ?? null,
    shipName: value.ship_name ?? null,
    victimName: value.victim_name ?? null,
    killerName: value.killer_name ?? null,
  };
}

export function selectSalvageStates(
  fleet: FleetEntry[],
  salvageByPoi: Record<string, SalvageStateWire>,
): Array<{ handle: string; state: SalvageSessionState }> {
  return fleet.map((bot) => {
    const poi = bot.state.location.poi_id ?? "";
    const salvage = salvageByPoi[poi] ??
      Object.values(salvageByPoi).find((value) => value.last_seen_poi === poi) ?? { visible_lootables: [], lootables_by_poi: {} };
    return {
      handle: bot.username ?? bot.id,
      state: {
        sessionId: bot.id,
        stateVersion: bot.version,
        username: bot.username ?? null,
        visibleLootables: (salvage.visible_lootables ?? []).map(selectSalvageLootable),
        lootablesByPoi: Object.fromEntries(Object.entries(salvage.lootables_by_poi ?? {}).map(([id, rows]) => [id, rows.map(selectSalvageLootable)])),
        lastSeenPoi: salvage.last_seen_poi ?? null,
        lastSeenSystem: salvage.last_seen_system ?? null,
        observedAtUnix: salvage.observed_at_unix ?? null,
      },
    };
  });
}

export function selectSocialBots(sightings: Record<string, AgentSightingWire>, fleet: FleetEntry[]): SocialBot[] {
  const rows = Object.values(sightings).map((sighting): SocialBot => ({
    actorKind: "player",
    synthetic: false,
    playerId: sighting.contact.player_id ?? null,
    username: sighting.contact.username ?? "",
    factionId: sighting.contact.faction_id ?? null,
    factionTag: sighting.contact.faction_tag ?? null,
    clanTag: sighting.contact.clan_tag ?? null,
    shipClass: sighting.contact.ship_class ?? null,
    shipName: sighting.contact.ship_name ?? null,
    statusMessage: sighting.contact.status_message ?? null,
    primaryColor: sighting.contact.primary_color ?? null,
    secondaryColor: sighting.contact.secondary_color ?? null,
    inCombat: sighting.contact.in_combat ?? false,
    offline: sighting.contact.offline ?? false,
    lastSeenSystem: sighting.last_seen_system,
    firstSeenUtc: new Date(sighting.first_seen_unix * 1000).toISOString(),
    lastSeenUtc: new Date(sighting.last_seen_unix * 1000).toISOString(),
    timesSeen: sighting.times_seen,
  }));
  const seen = new Set(rows.map((row) => row.playerId ?? row.username));
  for (const bot of fleet) {
    const player = bot.state.player;
    const playerId = typeof player?.id === "string" ? player.id : bot.id;
    const username = bot.username ?? (typeof player?.username === "string" ? player.username : bot.id);
    if (seen.has(playerId) || seen.has(username)) continue;
    rows.push({
      actorKind: "player",
      synthetic: false,
      playerId,
      username,
      factionId: typeof player?.faction_id === "string" ? player.faction_id : null,
      factionTag: null,
      clanTag: typeof player?.clan_tag === "string" ? player.clan_tag : null,
      shipClass: null,
      shipName: null,
      statusMessage: null,
      primaryColor: null,
      secondaryColor: null,
      inCombat: bot.state.in_battle,
      offline: bot.connection !== "Connected",
      lastSeenSystem: bot.state.location.system_id ?? "",
      firstSeenUtc: bot.observed_at ?? new Date(0).toISOString(),
      lastSeenUtc: bot.observed_at ?? new Date(0).toISOString(),
      timesSeen: 1,
    });
  }
  return rows;
}

export function selectGameChatMessages(messages: ChatMessageWire[], sessionHandle: string): GameChatMessage[] {
  return messages.map((message) => ({
    id: message.id,
    channel: message.channel,
    senderId: message.sender_id,
    sender: message.sender,
    content: message.content,
    timestampUtc: message.timestamp_utc,
    systemId: message.system_id ?? null,
    poiId: message.poi_id ?? null,
    factionId: message.faction_id ?? null,
    targetId: message.target_id ?? null,
    targetName: message.target_name ?? null,
    empireOfficial: message.empire_official ?? false,
    sessionHandle,
  }));
}

export function selectFaction(value: FactionSnapshotWire | undefined): FactionInfo | null {
  if (!value?.id) return null;
  return {
    id: value.id,
    name: value.name,
    tag: value.tag,
    leaderId: value.leader_id,
    leaderUsername: value.leader_username,
    memberCount: value.member_count,
    treasury: value.treasury ?? null,
    isMember: value.is_member,
    description: value.description || null,
    primaryColor: value.primary_color || null,
    secondaryColor: value.secondary_color || null,
    members: value.members.map((member) => ({
      playerId: member.player_id,
      username: member.username,
      role: member.role,
      online: member.online,
    })),
    roles: value.roles.map((role) => ({ name: role.name, priority: role.priority, permissions: [] })),
  };
}

function optionalString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

export function selectCommanderStorage(
  fleet: FleetEntry[],
  map: GalaxyMap | null,
  storageByPlayer: StorageByOwner,
  factionStorageByFactionPoi: StorageByOwner,
  factionBySession: Record<string, FactionSnapshotWire> = {},
): CommanderStorageView {
  const poiById = new Map(map?.knownPois.map((poi) => [poi.id, poi]) ?? []);
  const rows: CommanderStorageRow[] = [];
  const row = (
    value: Omit<
      CommanderStorageRow,
      | "unitMarketPrice"
      | "totalMarketValue"
      | "unitMedianBuyPrice"
      | "unitMedianSellPrice"
      | "totalMedianBuyValue"
      | "totalMedianSellValue"
      | "marketPriceSource"
    >,
  ): CommanderStorageRow => ({
    ...value,
    unitMarketPrice: null,
    totalMarketValue: null,
    unitMedianBuyPrice: null,
    unitMedianSellPrice: null,
    totalMedianBuyValue: null,
    totalMedianSellValue: null,
    marketPriceSource: null,
  });

  for (const bot of fleet) {
    const ownerName = bot.username?.trim() || bot.id;
    const ownerId = optionalString(bot.state.player.id) ?? bot.id;
    const locationId = bot.state.location.poi_id ?? bot.state.location.system_id ?? "ship";
    const location = poiById.get(locationId);
    for (const [itemId, quantity] of Object.entries(bot.state.cargo)) {
      rows.push(
        row({
          key: `cargo:${bot.id}:${locationId}:${itemId}`,
          itemId,
          quantity: quantity as number,
          sourceKind: "cargo",
          ownerId,
          ownerName,
          locationId,
          locationName: location?.name ?? "ship",
          systemId: bot.state.location.system_id ?? location?.systemId ?? null,
          observedBy: [ownerName],
          stateVersion: bot.version,
          details: null,
        }),
      );
    }
    const credits = bot.state.player.credits ?? 0;
    rows.push(
      row({
        key: `financial:${bot.id}:wallet:credits`,
        itemId: "credits",
        quantity: credits,
        sourceKind: "financial",
        ownerId,
        ownerName,
        locationId: "wallet",
        locationName: "wallet",
        systemId: null,
        observedBy: [ownerName],
        stateVersion: bot.version,
        details: { kind: "credits", label: "Wallet credits" },
      }),
    );
  }

  const factions = new Map<string, { snapshot: FactionSnapshotWire; sessionKeys: string[] }>();
  for (const [sessionKey, snapshot] of Object.entries(factionBySession)) {
    if (!snapshot.id) continue;
    const existing = factions.get(snapshot.id);
    if (existing) {
      existing.sessionKeys.push(sessionKey);
    } else {
      factions.set(snapshot.id, { snapshot, sessionKeys: [sessionKey] });
    }
  }
  for (const [factionId, { snapshot, sessionKeys }] of factions) {
    const observers = fleet.filter(
      (bot) =>
        optionalString(bot.state.player.faction_id) === factionId ||
        sessionKeys.includes(bot.id) ||
        (bot.username ? sessionKeys.includes(bot.username) : false),
    );
    rows.push(
      row({
        key: `financial:faction:${factionId}:treasury:credits`,
        itemId: "credits",
        quantity: snapshot.treasury ?? 0,
        sourceKind: "financial",
        ownerId: factionId,
        ownerName: snapshot.name?.trim() || snapshot.tag?.trim() || factionId,
        locationId: "faction-treasury",
        locationName: "faction treasury",
        systemId: null,
        observedBy: observers.map((bot) => bot.username?.trim() || bot.id),
        stateVersion: observers.length ? Math.max(...observers.map((bot) => bot.version)) : null,
        details: { kind: "faction_treasury", label: "Faction wallet" },
      }),
    );
  }

  for (const [ownerId, byPoi] of Object.entries(storageByPlayer)) {
    const observers = fleet.filter((bot) => bot.id === ownerId || bot.username === ownerId || bot.state.player.id === ownerId);
    const ownerName = observers[0]?.username?.trim() || ownerId;
    for (const [poiId, items] of Object.entries(byPoi)) {
      const poi = poiById.get(poiId);
      for (const [itemId, quantity] of Object.entries(items)) {
        rows.push(
          row({
            key: `personal:${ownerId}:${poiId}:${itemId}`,
            itemId,
            quantity,
            sourceKind: "personal",
            ownerId,
            ownerName,
            locationId: poiId,
            locationName: poi?.name ?? null,
            systemId: poi?.systemId ?? null,
            observedBy: observers.map((bot) => bot.username?.trim() || bot.id),
            stateVersion: observers.length ? Math.max(...observers.map((bot) => bot.version)) : null,
            details: null,
          }),
        );
      }
    }
  }

  for (const [factionId, byPoi] of Object.entries(factionStorageByFactionPoi)) {
    const observers = fleet.filter((bot) => optionalString(bot.state.player.faction_id) === factionId);
    for (const [poiId, items] of Object.entries(byPoi)) {
      const poi = poiById.get(poiId);
      for (const [itemId, quantity] of Object.entries(items)) {
        rows.push(
          row({
            key: `faction:${factionId}:${poiId}:${itemId}`,
            itemId,
            quantity,
            sourceKind: "faction",
            ownerId: factionId,
            ownerName: factionId,
            locationId: poiId,
            locationName: poi?.name ?? null,
            systemId: poi?.systemId ?? null,
            observedBy: observers.map((bot) => bot.username?.trim() || bot.id),
            stateVersion: observers.length ? Math.max(...observers.map((bot) => bot.version)) : null,
            details: null,
          }),
        );
      }
    }
  }

  return {
    stateVersion: fleet.length ? Math.max(...fleet.map((bot) => bot.version)) : null,
    knowledgeVersion: null,
    sessionsObserved: fleet.length,
    sessionsTotal: fleet.length,
    rows: rows.filter((item) => item.quantity > 0 || item.sourceKind === "financial"),
  };
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle]! : (sorted[middle - 1]! + sorted[middle]!) / 2;
}

function weightedMedian(values: Array<{ price: number; quantity: number }>): number | null {
  const sorted = values.filter((value) => value.quantity > 0).sort((a, b) => a.price - b.price);
  const total = sorted.reduce((sum, value) => sum + value.quantity, 0);
  if (total <= 0) return null;
  const atRank = (rank: number) => {
    let cumulative = 0;
    for (const value of sorted) {
      cumulative += value.quantity;
      if (cumulative >= rank) return value.price;
    }
    return sorted.at(-1)!.price;
  };
  return (atRank(Math.floor((total + 1) / 2)) + atRank(Math.floor((total + 2) / 2))) / 2;
}

export function selectEconomyMarket(markets: StationMarkets, map: GalaxyMap | null): EconomyMarketData {
  const poiById = new Map(map?.knownPois.map((poi) => [poi.id, poi]) ?? []);
  const bestBuys = new Map<string, number[]>();
  const bestSells = new Map<string, number[]>();
  const sellDepth = new Map<string, Array<{ price: number; quantity: number }>>();
  const marketsByStation = Object.fromEntries(
    Object.entries(markets).map(([stationId, market]) => {
      for (const [itemId, orders] of Object.entries(market.buy_orders)) {
        const prices = orders.filter((order) => order.quantity > 0).map((order) => order.price_each);
        if (prices.length) bestBuys.set(itemId, [...(bestBuys.get(itemId) ?? []), Math.max(...prices)]);
      }
      for (const [itemId, orders] of Object.entries(market.sell_orders)) {
        const live = orders.filter((order) => order.quantity > 0);
        if (live.length) bestSells.set(itemId, [...(bestSells.get(itemId) ?? []), Math.min(...live.map((order) => order.price_each))]);
        sellDepth.set(itemId, [
          ...(sellDepth.get(itemId) ?? []),
          ...live.filter((order) => order.price_each > 0).map((order) => ({ price: order.price_each, quantity: order.quantity })),
        ]);
      }
      const poi = poiById.get(stationId);
      return [
        stationId,
        {
          stationId,
          poiId: poi?.id ?? stationId,
          stationName: poi?.name ?? null,
          sellOrders: market.sell_orders,
          buyOrders: market.buy_orders,
          observedAtUnix: market.observed_at_unix ?? null,
          currentTick: market.current_tick ?? null,
        },
      ];
    }),
  );
  return {
    marketsByStation,
    globalMedianBuyPrices: Object.fromEntries([...bestBuys].map(([item, values]) => [item, median(values)])),
    globalMedianSellPrices: Object.fromEntries([...bestSells].map(([item, values]) => [item, median(values)])),
    globalWeightedMidPrices: Object.fromEntries(
      [...sellDepth].flatMap(([item, values]) => {
        const value = weightedMedian(values);
        return value === null ? [] : [[item, value]];
      }),
    ),
  };
}

/** Pure UI projection using authoritative route costs fetched by the caller. */
export function selectMarketArbitrage(
  markets: StationMarkets,
  map: GalaxyMap | null,
  catalog: GalaxyCatalog | null,
  originSystem: string | null,
  includeOriginJump: boolean,
  options: { minMargin: number; minDepthCoverage: number; maxUnits: number; limit: number; routeCosts: ReadonlyMap<string, number | null> },
): EconomyArbitrageData {
  const systemByPoi = new Map(map?.knownPois.map((poi) => [poi.id, poi.systemId]) ?? []);
  const groups = new Map<string, ArbitradePackage>();
  const now = Math.floor(Date.now() / 1000);
  for (const [buyStationId, buyMarket] of Object.entries(markets)) {
    const buySystemId = systemByPoi.get(buyStationId);
    if (!buySystemId) continue;
    for (const [itemId, asks] of Object.entries(buyMarket.sell_orders)) {
      const item = catalog?.itemsById[itemId];
      const itemSize = typeof item?.["size"] === "number" && item["size"] > 0 ? item["size"] : 1;
      for (const [sellStationId, sellMarket] of Object.entries(markets)) {
        if (sellStationId === buyStationId) continue;
        const sellSystemId = systemByPoi.get(sellStationId);
        if (!sellSystemId) continue;
        const jumpsBuyToSell = options.routeCosts.get(`${buySystemId}\0${sellSystemId}`) ?? null;
        if (jumpsBuyToSell === null) continue;
        const jumpsToBuy = includeOriginJump && originSystem ? (options.routeCosts.get(`${originSystem}\0${buySystemId}`) ?? null) : 0;
        if (jumpsToBuy === null) continue;
        for (const ask of asks)
          for (const bid of sellMarket.buy_orders[itemId] ?? []) {
            if (ask.quantity <= 0 || bid.quantity <= 0 || ask.price_each <= 0 || bid.price_each <= ask.price_each) continue;
            const grossMargin = (bid.price_each - ask.price_each) / ask.price_each;
            if (grossMargin < options.minMargin) continue;
            const available = Math.min(ask.quantity, bid.quantity);
            const quantity = Math.min(available, Math.floor(options.maxUnits / itemSize));
            if (quantity <= 0) continue;
            const breakEvenCover = available / quantity;
            if (breakEvenCover < options.minDepthCoverage) continue;
            const profitPerUnit = bid.price_each - ask.price_each;
            const totalProfit = profitPerUnit * quantity;
            const capitalRequired = ask.price_each * quantity;
            const roi = capitalRequired > 0 ? totalProfit / capitalRequired : 0;
            const dataAgeSeconds = Math.max(
              buyMarket.observed_at_unix ? now - buyMarket.observed_at_unix : 0,
              sellMarket.observed_at_unix ? now - sellMarket.observed_at_unix : 0,
            );
            const rawScore = totalProfit / Math.max(1, jumpsToBuy + jumpsBuyToSell);
            const riskBand = grossMargin >= 0.5 ? "low" : grossMargin >= 0.25 ? "medium" : "high";
            const deal: ArbitrageDeal = {
              itemId,
              buyStationId,
              buySystemId,
              acquireFrom: { kind: "market" },
              buyPrice: ask.price_each,
              sellStationId,
              sellSystemId,
              disposeTo: { kind: "market" },
              sellPrice: bid.price_each,
              profitPerUnit,
              itemSize,
              quantity,
              totalProfit,
              capitalRequired,
              roi,
              grossMargin,
              breakEvenCover,
              riskBand,
              jumpsToBuy,
              jumpsBuyToSell,
              dataAgeSeconds,
              rawScore,
              score: rawScore,
            };
            const key = `${buyStationId}\0${sellStationId}`;
            const pkg = groups.get(key) ?? {
              buyStationId,
              buySystemId,
              sellStationId,
              sellSystemId,
              deals: [],
              cargoUsed: 0,
              cargoCapacity: options.maxUnits,
              capitalRequired: 0,
              totalProfit: 0,
              roi: 0,
              grossMargin: 0,
              breakEvenCover,
              riskBand,
              jumpsToBuy,
              jumpsBuyToSell,
              dataAgeSeconds,
              rawScore: 0,
              score: 0,
            };
            const remaining = Math.max(0, options.maxUnits - pkg.cargoUsed);
            const fitted = Math.min(deal.quantity, Math.floor(remaining / itemSize));
            if (fitted <= 0) continue;
            if (fitted !== deal.quantity) {
              deal.quantity = fitted;
              deal.totalProfit = fitted * profitPerUnit;
              deal.capitalRequired = fitted * ask.price_each;
            }
            pkg.deals.push(deal);
            pkg.cargoUsed += fitted * itemSize;
            pkg.totalProfit += deal.totalProfit;
            pkg.capitalRequired += deal.capitalRequired;
            pkg.roi = pkg.capitalRequired ? pkg.totalProfit / pkg.capitalRequired : 0;
            pkg.grossMargin = pkg.roi;
            pkg.rawScore = pkg.totalProfit / Math.max(1, jumpsToBuy + jumpsBuyToSell);
            pkg.score = pkg.rawScore;
            groups.set(key, pkg);
          }
      }
    }
  }
  return { packages: [...groups.values()].sort((a, b) => b.score - a.score).slice(0, options.limit) };
}
