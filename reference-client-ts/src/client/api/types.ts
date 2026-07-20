import type {
  CatalogDumpItemsItem,
  CommissionEntry,
  CraftingQueueProjection,
  FacilityDefinition,
  Recipe,
  ShipClass,
  V2GameStateShip,
} from "@prayer/sdk/types";

export type OwnedShipInfo = {
  owner: string;
  ownerHandle: string;
  shipId: string;
  classId: string;
  location: string;
  locationBaseId: string;
  ownerKind: string;
  ownerId: string;
  ownerName: string;
  factionId: string;
  factionTag: string;
  active: boolean;
  isActive: boolean;
  isGaraged: boolean;
  className: string;
  customName: string;
  fuel: string;
  hull: string;
  cargoUsed: number | null;
  modules: number | null;
  listingId: string;
  listingBaseId: string;
  listingPrice: number | null;
};

export type FactionGarageShipInfo = {
  ownerHandle: string;
  baseId: string;
  baseName: string;
  systemName: string;
  factionId: string;
  factionTag: string;
  shipId: string;
  classId: string;
  className: string;
  customName: string;
  depositorId: string;
  depositorName: string;
  depositedTick: number | null;
};

export type FactionGarageInfo = {
  used: number | null;
  capacity: number | null;
  ships: FactionGarageShipInfo[];
};

export type ShipyardShowroomEntry = {
  shipClassId: string;
  shipId: string;
  name: string;
  role: string;
  category: string;
  empire: string;
  tier: number | null;
  scale: number | null;
  hull: number | null;
  shield: number | null;
  cargo: number | null;
  price: number | null;
  canCommission: boolean | null;
};

export type ActiveCommissionInfo = {
  commissionId: string;
  shipClass: string;
  shipClassName: string;
  status: string;
  baseId: string;
  baseName: string;
  progress: number | null;
  ticksRemaining: number | null;
  totalCost: number | null;
  provideMaterials: boolean | null;
  source: CommissionEntry | null;
};

export type ShipyardData = {
  sessionId: string | null;
  stateVersion: number | null;
  factionId: string | null;
  currentBaseId: string | null;
  currentBaseName: string | null;
  docked: boolean | null;
  activeShip: V2GameStateShip | null;
  installedModules: string[];
  installedModuleNames: Record<string, string>;
  ships: OwnedShipInfo[];
  ownedShips: OwnedShipInfo[];
  factionGarage: FactionGarageInfo;
  inProgressCommissions: ActiveCommissionInfo[];
  shipyardShowroom: ShipyardShowroomEntry[];
};

export type ShipyardFleetData = {
  stateVersion: number | null;
  knowledgeVersion: number | null;
  sessionsObserved: number | null;
  sessionsTotal: number | null;
  ships: OwnedShipInfo[];
  ownedShips: OwnedShipInfo[];
  factionGarageShips: FactionGarageShipInfo[];
};

export type StorageSessionState = {
  sessionId: string | null;
  stateVersion: number | null;
  username: string | null;
  factionId: string | null;
  cargo: Record<string, number>;
  storageByPoi: Record<string, Record<string, number>>;
  factionStorage: Record<string, number>;
};

export type PassengerInfo = {
  citizenId: string;
  name: string;
  bio: string;
  className: string;
  citizenship: string;
  destination: string;
  destinationName: string;
  destinationSystem: string;
  baseFare: number | null;
  estimatedFare: number | null;
  speedBonus: number | null;
  ticksRemaining: number | null;
};

export type PassengerBerths = {
  current: number;
  max: number;
};

export type PassengerState = {
  sessionId: string | null;
  stateVersion: number | null;
  username: string | null;
  system: string | null;
  currentPoi: string | null;
  currentPoiName: string | null;
  aboardCount: number | null;
  economyBerths: PassengerBerths;
  businessBerths: PassengerBerths;
  firstBerths: PassengerBerths;
  aboard: PassengerInfo[];
  station: string;
  waitingCount: number | null;
  waiting: PassengerInfo[];
};

export type PassengerSessionResult = {
  handle: string;
  state: PassengerState | null;
  error: string | null;
};

export type CommanderStorageSourceKind = "cargo" | "personal" | "faction" | "financial";

export type CommanderStorageRow = {
  key: string;
  itemId: string;
  quantity: number;
  unitMarketPrice: number | null;
  totalMarketValue: number | null;
  unitMedianBuyPrice: number | null;
  unitMedianSellPrice: number | null;
  totalMedianBuyValue: number | null;
  totalMedianSellValue: number | null;
  marketPriceSource: string | null;
  sourceKind: CommanderStorageSourceKind;
  ownerId: string | null;
  ownerName: string;
  locationId: string;
  locationName: string | null;
  systemId: string | null;
  observedBy: string[];
  stateVersion: number | null;
  details: { kind?: "credits" | "faction_treasury"; label?: string; jumps?: number } | null;
};

export type CommanderStorageView = {
  stateVersion: number | null;
  knowledgeVersion: number | null;
  sessionsObserved: number;
  sessionsTotal: number;
  rows: CommanderStorageRow[];
};

export type FacilityOwnerKind = "personal" | "faction" | "other";

export type FacilityInfo = {
  facilityId: string;
  facilityType: string;
  name: string;
  category: string;
  level: number | null;
  status: string;
  ownerKind: FacilityOwnerKind;
  ownerName: string;
  locationId: string;
  locationName: string;
  systemId: string;
  buildTime: number | null;
  rentPerCycle: number | null;
  public: boolean | null;
};

export type FacilityTypeInfo = {
  facilityType: string;
  name: string;
  category: string;
  level: number | null;
  upgradesFrom: string;
  price: number | null;
  buildTime: number | null;
  rentPerCycle: number | null;
  requiredSkills: Record<string, number>;
  requiredItems: Record<string, number>;
  recipeId: string | null;
};

export type FacilitiesData = {
  sessionId: string | null;
  username: string | null;
  latestSystem: string | null;
  latestPoi: string | null;
  docked: boolean | null;
  current: FacilityInfo[];
  publicFacilities: FacilityInfo[];
  owned: FacilityInfo[];
  factionCurrent: FacilityInfo[];
  factionOwned: FacilityInfo[];
  factionId: string | null;
  factionRentPerCycle: number | null;
  factionArrearsOwed: number | null;
  types: FacilityTypeInfo[];
  errors: string[];
};

export type CraftingQueueJob = CraftingQueueProjection;

export type CraftingSessionState = {
  craftingQueue: CraftingQueueJob[];
};

export type CraftActionResult = {
  succeeded: boolean;
  message: string | null;
  runId: string | null;
  command: string | null;
};

export type CraftStorageSource = "cargo" | "faction" | "storage";

export type SalvageLootItem = {
  itemId: string;
  name: string;
  quantity: number;
  size: number | null;
};

export type SalvageLootModule = {
  id: string;
  typeId: string;
  name: string;
  moduleType: string;
  wear: number | null;
};

export type SalvageLootable = {
  id: string;
  kind: string;
  poiId: string;
  systemId: string;
  cargo: SalvageLootItem[];
  modules: SalvageLootModule[];
  salvageValue: number | null;
  createdAt: string | null;
  expiresAt: string | null;
  expireTick: number | null;
  shipClass: string | null;
  shipName: string | null;
  victimName: string | null;
  killerName: string | null;
};

export type SalvageSessionState = {
  sessionId: string | null;
  stateVersion: number | null;
  username: string | null;
  visibleLootables: SalvageLootable[];
  lootablesByPoi: Record<string, SalvageLootable[]>;
  lastSeenPoi: string | null;
  lastSeenSystem: string | null;
  observedAtUnix: number | null;
};

export type WildlifeCreature = {
  creatureId: string;
  species: string;
  name: string;
  role: string;
  hull: number;
  maxHull: number;
  inCombat: boolean;
  systemId: string;
  poiId: string;
  observedAtUnix: number;
};

export type WildlifeSpecies = {
  species: string;
  name: string;
  role: string;
  count: number;
};

export type WildlifeSystem = {
  systemId: string;
  creatureCount: number;
  species: WildlifeSpecies[];
  pois: string[];
  observedAtUnix: number;
};

export type WildlifePoi = {
  systemId: string;
  poiId: string;
  creatureCount: number;
  observedAtUnix: number;
  creatures: WildlifeCreature[];
};

export type WildlifeState = {
  sessionId: string | null;
  stateVersion: number | null;
  username: string | null;
  currentSystem: string | null;
  currentPoi: string | null;
  nearbyCreatureCount: number | null;
  nearbyCreatures: WildlifeCreature[];
  systems: WildlifeSystem[];
  pois: WildlifePoi[];
};

export type CatalogIngredient = {
  itemId: string;
  item: string;
  id: string;
  name: string;
  quantity: number | null;
  amount: number | null;
  count: number | null;
};

export type CatalogEntry = {
  id: string;
  name: string;
  classId: string;
  className: string;
  category: string;
  typeName: string;
  tier: number | null;
  scale: number | null;
  size: number | null;
  hull: number | null;
  baseHull: number | null;
  shield: number | null;
  baseShield: number | null;
  cargo: number | null;
  cargoCapacity: number | null;
  speed: number | null;
  baseSpeed: number | null;
  price: number | null;
  materials: Record<string, number>;
  ingredients: CatalogIngredient[];
  inputs: CatalogIngredient[];
  outputs: CatalogIngredient[];
  requiredSkills: Record<string, number>;
  recipeIds: string[];
  source: CatalogDumpItemsItem | FacilityDefinition | ShipClass;
};

export type ShipCatalogEntry = Omit<CatalogEntry, "source"> & {
  cpuCapacity: number | null;
  powerCapacity: number | null;
  weaponSlots: number | null;
  defenseSlots: number | null;
  utilitySlots: number | null;
  defaultModules: string[];
  inherentCapabilities: unknown[];
  source: ShipClass;
};

export type RecipeCatalogEntry = {
  id: string;
  name: string;
  inputs: CatalogIngredient[];
  outputs: CatalogIngredient[];
  requiredSkills: Record<string, number>;
  requiredFacilityTypes: string[];
  source: Recipe;
};

export type CatalogState = {
  sessionId: string | null;
  stateVersion: number | null;
  username: string | null;
  items: CatalogEntry[];
  ships: ShipCatalogEntry[];
  recipes: RecipeCatalogEntry[];
  facilities: CatalogEntry[];
};

export type ShipyardAction = "switch" | "garage" | "sell" | "scrap";

export type ShipyardModuleLoadoutResult = {
  succeeded: boolean;
  message: string;
};

export type ShipyardScriptResult = {
  succeeded: boolean;
  message: string;
};

export type RouteDistanceResult = {
  totalJumps: number | null;
};

export type GalaxyMapSystem = {
  id: string;
  name?: string;
  empire?: string;
  isStronghold?: boolean;
  isCapital?: boolean;
  x: number | null;
  y: number | null;
  connections: string[];
  poiCount?: number | null;
  poisComplete?: boolean;
  firstEnteredUnix?: number | null;
  lastEnteredUnix?: number | null;
  lastScannedUnix?: number | null;
  lastSurveyedUnix?: number | null;
  bloomStatus?: string | null;
  bloomIntensity?: number | null;
  pois?: Array<{ id: string; x: number | null; y: number | null }>;
};

export type GalaxyMapData = {
  systems: GalaxyMapSystem[];
  knownPois: Array<{
    id: string;
    systemId: string;
    name: string;
    type: string;
    x: number | null;
    y: number | null;
    hasBase?: boolean;
    baseId?: string | null;
    baseName?: string | null;
    firstDiscoveredUnix?: number | null;
    lastObservedUnix?: number | null;
    firstVisitedUnix?: number | null;
    lastVisitedUnix?: number | null;
  }>;
};

export type GalaxyExplorationData = {
  exploredSystems: string[];
  visitedPois: string[];
  surveyedSystems: string[];
};

export type GalaxyResourcesData = {
  systemsByResource: Record<string, string[]>;
  poisByResource: Record<string, string[]>;
};

export type SocialBot = {
  actorKind?: "player" | "pirate" | "empire" | string;
  synthetic?: boolean;
  playerId: string | null;
  username: string;
  factionId: string | null;
  factionTag: string | null;
  clanTag: string | null;
  shipClass: string | null;
  shipName: string | null;
  statusMessage: string | null;
  primaryColor: string | null;
  secondaryColor: string | null;
  inCombat: boolean;
  offline: boolean;
  lastSeenSystem: string;
  firstSeenUtc: string;
  lastSeenUtc: string;
  timesSeen: number;
};

export type GameChatChannel = "system" | "local" | "faction" | "private" | "emergency";

export type GameChatMessage = {
  id: string;
  channel: GameChatChannel | string;
  senderId: string;
  sender: string;
  content: string;
  timestampUtc: string;
  systemId: string | null;
  poiId: string | null;
  factionId: string | null;
  targetId: string | null;
  targetName: string | null;
  empireOfficial: boolean;
  sessionHandle: string;
};

export type FactionMember = {
  playerId: string | null;
  username: string | null;
  role: string | null;
  online: boolean | null;
};

export type FactionRole = {
  name: string | null;
  priority: number | null;
  permissions: string[];
};

export type FactionInfo = {
  id: string | null;
  name: string | null;
  tag: string | null;
  leaderId: string | null;
  leaderUsername: string | null;
  memberCount: number | null;
  treasury: number | null;
  isMember: boolean | null;
  description: string | null;
  primaryColor: string | null;
  secondaryColor: string | null;
  members: FactionMember[];
  roles: FactionRole[];
};

export type MarketOrder = {
  price_each: number;
  quantity: number;
  source?: string | null;
  my_quantity?: number | null;
};

export type StationMarket = {
  stationId: string;
  poiId?: string | null;
  stationName?: string | null;
  sellOrders: Record<string, MarketOrder[]>;
  buyOrders: Record<string, MarketOrder[]>;
  observedAtUnix: number | null;
  currentTick: number | null;
};

export type EconomyMarketData = {
  marketsByStation: Record<string, StationMarket>;
  globalMedianBuyPrices: Record<string, number>;
  globalMedianSellPrices: Record<string, number>;
  globalWeightedMidPrices: Record<string, number>;
};

export type ArbitrageDeal = {
  itemId: string;
  buyStationId: string;
  buySystemId: string;
  acquireFrom: {
    kind: "market" | "virtual_faction" | "personal_storage" | string;
    virtualOrderId?: string | null;
  };
  buyPrice: number;
  sellStationId: string;
  sellSystemId: string;
  disposeTo: {
    kind: "market" | "virtual_faction" | "personal_storage" | string;
    virtualOrderId?: string | null;
  };
  sellPrice: number;
  profitPerUnit: number;
  itemSize: number;
  quantity: number;
  totalProfit: number;
  capitalRequired: number;
  roi: number;
  grossMargin: number;
  breakEvenCover: number;
  riskBand: "low" | "medium" | "high" | "thin" | string;
  jumpsToBuy: number;
  jumpsBuyToSell: number;
  dataAgeSeconds: number | null;
  rawScore: number;
  score: number;
};

export type ArbitradePackage = {
  buyStationId: string;
  buySystemId: string;
  sellStationId: string;
  sellSystemId: string;
  deals: ArbitrageDeal[];
  passengerFares?: PassengerFareDeal[];
  passengerRevenue?: number;
  berthUsed?: PassengerBerthUsage;
  berthCapacity?: PassengerBerthUsage;
  anchorKind?: "item_deal" | "passenger_fare" | string;
  cargoUsed: number;
  cargoCapacity: number;
  capitalRequired: number;
  totalProfit: number;
  roi: number;
  grossMargin: number;
  breakEvenCover: number;
  riskBand: "low" | "medium" | "high" | "thin" | string;
  jumpsToBuy: number;
  jumpsBuyToSell: number;
  dataAgeSeconds: number | null;
  rawScore: number;
  score: number;
};

export type PassengerBerthUsage = {
  economy: number;
  business: number;
  first: number;
};

export type PassengerFareDeal = {
  citizenId: string;
  name: string;
  className: string;
  originStationId: string;
  destinationStationId: string;
  destinationSystemId?: string | null;
  estimatedFare: number;
  baseFare?: number | null;
  speedBonus?: number | null;
  berthUnits: number;
  totalJumps: number;
  farePerJump: number;
  score: number;
  riskBand: "passenger" | string;
};

export type VirtualFactionOrderInput = {
  id: string;
  side: "buy" | "sell" | "buy_until" | "sell_until";
  itemId: string;
  stationId: string;
  priceEach: number;
  quantity: number;
  tippingPoint?: number | null;
  dumping?: boolean;
  enabled: boolean;
  internalOnly?: boolean;
  reserved?: number;
  filled?: number;
  priority?: number;
  doForever?: boolean;
};

export type VirtualCraftOrderInput = {
  id: string;
  action: "craft" | "craft_until" | "commission_until" | "credit_floor";
  recipeId: string;
  itemId?: string;
  stationId: string;
  quantity: number;
  enabled: boolean;
  reserved?: number;
  filled?: number;
  priority?: number;
  facilityId?: string | null;
  preset?: string | null;
  squadId?: string | null;
  sessionHandles?: string[];
  creditFloor?: number | null;
  doForever?: boolean;
};

export type EconomyArbitrageData = {
  packages: ArbitradePackage[];
};

export type LogisticsEndpoint = {
  kind: "market" | "virtual_faction" | "personal_storage" | string;
  virtualOrderId?: string | null;
};

export type LogisticsItem = {
  itemId: string;
  quantity: number;
  itemSize: number;
  sourcePrice: number;
  destinationPrice: number;
  source: LogisticsEndpoint;
  destination: LogisticsEndpoint;
  priority: number;
  valuePerUnit: number;
  routeValue: number;
  score: number;
};

export type LogisticsPackage = {
  sourceStationId: string;
  sourceSystemId: string;
  destinationStationId: string;
  destinationSystemId: string;
  items: LogisticsItem[];
  cargoUsed: number;
  cargoCapacity: number;
  jumpsToSource: number;
  jumpsSourceToDestination: number;
  totalJumps: number;
  score: number;
};

export type EconomyLogisticsData = {
  packages: LogisticsPackage[];
};

export type ArbitrageScope = "current" | "global";

export type ArbitrageFilterOptions = {
  minMargin?: number | null;
  minDepthCoverage?: number | null;
  maxUnits?: number | null;
  firstBerths?: number | null;
};

export type CreditWireResult = {
  succeeded: boolean;
  message: string;
};

export type GoScriptResult = {
  succeeded: boolean;
  message: string;
  runId?: string;
};
