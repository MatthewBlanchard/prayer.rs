// actions.d.ts
import type { Action } from "./generated/types.js";
import type { GoTarget } from "./conveniences.js";
export declare const undock: () => Action;
export declare const dock: () => Action;
export declare const wait: (ticks: number) => Action;
export declare const go: (target: {
    poi: string;
} | {
    system: string;
} | GoTarget) => Action;
type ActionType = Action["type"];
type RequestFor<T extends ActionType> = Action extends infer A ? A extends {
    type: infer K;
    request: infer R;
} ? T extends K ? R : never : never : never;
type RequestActionType = {
    [T in ActionType]: [RequestFor<T>] extends [never] ? never : T;
}[ActionType];
type UnitActionType = Exclude<ActionType, RequestActionType>;
export declare const mine: (resource?: string) => Action;
/** Build any canonical prayer-actions operation with its exact request type. */
export declare function action<T extends RequestActionType>(type: T, request: RequestFor<T>): Action;
export declare function action<T extends UnitActionType>(type: T): Action;
/** Complete helper catalog for the canonical prayer-actions wire shape. */
export declare const actions: {
    readonly undock: () => Action;
    readonly dock: () => Action;
    readonly wait: (ticks: number) => Action;
    readonly mine: (resource?: string) => Action;
    readonly go: (target: {
        poi: string;
    } | {
        system: string;
    } | GoTarget) => Action;
    readonly halt: () => Action;
    readonly transfer: (request: import("./generated/types.js").TransferRequest) => Action;
    readonly setHome: () => Action;
    readonly find: (request: import("./generated/types.js").FindRequest) => Action;
    readonly survey: () => Action;
    readonly attack: (request: {
        target_id: string;
    }) => Action;
    readonly scan: (request?: Partial<{
        target?: string | null;
    }>) => Action;
    readonly cloak: (request: {
        enabled: boolean;
    }) => Action;
    readonly hunt: (request: {
        target: string;
    }) => Action;
    readonly prepayTax: (request: {
        quantity: number;
    }) => Action;
    readonly acceptMission: (request: {
        mission_id: string;
    }) => Action;
    readonly abandonMission: (request: {
        mission_id: string;
    }) => Action;
    readonly declineMission: (request: {
        template_id: string;
    }) => Action;
    readonly completeMission: (request: {
        mission_id: string;
    }) => Action;
    readonly loadPassenger: (request: {
        destination: string;
    }) => Action;
    readonly unloadPassenger: (request: {
        name: string;
        target?: string | null;
    }) => Action;
    readonly buy: (request: import("./generated/types.js").BuyRequest) => Action;
    readonly sell: (request: import("./generated/types.js").SellRequest) => Action;
    readonly cancelBuy: (request: {
        item: string;
    }) => Action;
    readonly cancelSell: (request: {
        item: string;
    }) => Action;
    readonly factionCreate: (request: {
        name: string;
        tag: string;
    }) => Action;
    readonly factionInvite: (request: {
        player: string;
    }) => Action;
    readonly factionAcceptInvite: (request: {
        faction: string;
    }) => Action;
    readonly factionKick: (request: {
        player: string;
    }) => Action;
    readonly factionSetRole: (request: {
        player: string;
        role: string;
    }) => Action;
    readonly facilityBuild: (request: {
        facility_type: string;
    }) => Action;
    readonly factionFacilityBuild: (request: {
        facility_type: string;
    }) => Action;
    readonly facilityUpgrade: (request: import("./generated/types.js").FacilityUpgradeRequest) => Action;
    readonly factionFacilityUpgrade: (request: import("./generated/types.js").FacilityUpgradeRequest) => Action;
    readonly facilityDismantle: (request: {
        facility_id: string;
    }) => Action;
    readonly factionFacilityDismantle: (request: {
        facility_id: string;
    }) => Action;
    readonly facilitySetAccess: (request: import("./generated/types.js").FacilityAccessRequest) => Action;
    readonly facilitySetOutputPrice: (request: import("./generated/types.js").FacilityOutputPriceRequest) => Action;
    readonly facilitySetName: (request: import("./generated/types.js").FacilityNameRequest) => Action;
    readonly useItem: (request: {
        item: string;
        quantity: number;
    }) => Action;
    readonly repair: (request?: Partial<import("./generated/types.js").ServiceTransferRequest>) => Action;
    readonly repairModule: (request: {
        module: string;
    }) => Action;
    readonly recycle: (request: import("./generated/types.js").RecycleRequest) => Action;
    readonly refuel: (request?: Partial<import("./generated/types.js").ServiceTransferRequest>) => Action;
    readonly selfDestruct: () => Action;
    readonly switchShip: (request: {
        ship: string;
    }) => Action;
    readonly renameShip: (request: {
        name: string;
    }) => Action;
    readonly installMod: (request: {
        module: string;
    }) => Action;
    readonly uninstallMod: (request: {
        module: string;
    }) => Action;
    readonly buyShip: (request: {
        listing: string;
    }) => Action;
    readonly buyListedShip: (request: {
        listing: string;
    }) => Action;
    readonly commissionShip: (request: import("./generated/types.js").CommissionShipRequest) => Action;
    readonly sellShip: (request: {
        ship: string;
    }) => Action;
    readonly scrapShip: (request: {
        ship: string;
    }) => Action;
    readonly listShipForSale: (request: {
        price: number;
        ship: string;
    }) => Action;
    readonly refitShip: () => Action;
    readonly cancelCommission: (request: {
        commission_id: string;
    }) => Action;
    readonly supplyCommission: (request: {
        commission_id: string;
        item: string;
        quantity: number;
    }) => Action;
    readonly cancelShipListing: (request: {
        listing_id: string;
    }) => Action;
    readonly placeShipBuyOrder: (request: {
        price: number;
        ship_class: string;
    }) => Action;
    readonly cancelShipBuyOrder: (request: {
        order_id: string;
    }) => Action;
    readonly sellShipToOrder: (request: {
        order_id: string;
        ship_id: string;
    }) => Action;
    readonly cancelOrder: (request: {
        order_id: string;
    }) => Action;
    readonly modifyOrder: (request: {
        order_id: string;
        price_each: number;
    }) => Action;
    readonly craft: (request: import("./generated/types.js").CraftRequest) => Action;
    readonly cancelCraftJob: (request: {
        job_id: string;
    }) => Action;
    readonly salvageWreck: (request: {
        wreck_id: string;
    }) => Action;
    readonly towWreck: (request: {
        wreck_id: string;
    }) => Action;
    readonly scrapWreck: () => Action;
    readonly sellWreck: () => Action;
    readonly releaseWreck: () => Action;
    readonly insureShip: (request: {
        ticks: number;
    }) => Action;
    readonly citizenshipApply: (request: {
        empire_id: string;
    }) => Action;
    readonly citizenshipWithdraw: (request: {
        empire_id: string;
    }) => Action;
    readonly citizenshipRenounce: (request: {
        empire_id: string;
    }) => Action;
    readonly tradeOffer: (request: import("./generated/types.js").TradeOfferRequest) => Action;
    readonly tradeAccept: (request: {
        trade_id: string;
    }) => Action;
    readonly factionLeave: () => Action;
    readonly factionWithdrawInvite: (request: {
        player: string;
    }) => Action;
    readonly factionProposeAlly: (request: {
        faction: string;
    }) => Action;
    readonly factionAcceptAlly: (request: {
        faction: string;
    }) => Action;
    readonly factionRemoveAlly: (request: {
        faction: string;
    }) => Action;
    readonly factionDeclareWar: (request: {
        faction: string;
        reason?: string | null;
    }) => Action;
    readonly factionProposePeace: (request: {
        faction: string;
        message?: string | null;
    }) => Action;
    readonly factionAcceptPeace: (request: {
        faction: string;
    }) => Action;
    readonly factionSetEnemy: (request: {
        faction: string;
    }) => Action;
    readonly factionRemoveEnemy: (request: {
        faction: string;
    }) => Action;
    readonly factionPrepayTax: (request: {
        quantity: number;
    }) => Action;
    readonly factionCancelMission: (request: {
        mission_id: string;
    }) => Action;
    readonly espionage: () => Action;
    readonly scanPoi: (request: {
        poi_id: string;
    }) => Action;
    readonly distressSignal: (request?: Partial<{
        distress_type?: string | null;
    }>) => Action;
    readonly say: (request: import("./generated/types.js").SayRequest) => Action;
};
export {};

// client.d.ts
import type { Action, ActionRunResponse, BotSummary, FleetEntry, QueueLane, QueueResponse, RouteQuery, RouteSelection as CachedRouteSelection, ScriptRunResponse } from "./generated/types.js";
import type { StateSnapshot } from "./conveniences.js";
import { type RequestOptions, type TransportOptions } from "./transport.js";
import { PrayerApi } from "./generated/api.js";
export interface SubmitOptions extends RequestOptions {
    idempotencyKey?: string;
}
export interface ExecuteOptions extends SubmitOptions {
    pollMs?: number;
}
export interface OverrideOptions extends RequestOptions {
    returnToOrigin?: boolean;
}
export interface WaitOptions<T> extends RequestOptions {
    pollMs?: number;
    onStatus?: (status: T) => void | Promise<void>;
}
export type ActionInput = Action | readonly Action[];
export interface PrayerAdvanced {
    readonly api: PrayerApi;
}
export interface RouteOptions {
    safe?: boolean;
}
export type RouteRequest = RouteQuery;
export type AuthoritativeRoute = CachedRouteSelection;
export declare class Prayer {
    private readonly transport;
    readonly advanced: PrayerAdvanced;
    private get api();
    private stateCache;
    private constructor();
    static connect(options: TransportOptions): Promise<Prayer>;
    bots(options?: RequestOptions): Promise<BotSummary[]>;
    route(from: string, to: string, options?: RouteOptions, requestOptions?: RequestOptions): Promise<AuthoritativeRoute | null>;
    routes(routes: readonly RouteRequest[], options?: RouteOptions, requestOptions?: RequestOptions): Promise<Array<AuthoritativeRoute | null>>;
    state(options?: RequestOptions): Promise<StateSnapshot>;
    bot(selector: string, options?: RequestOptions): Promise<Bot>;
}
export declare class Bot {
    private readonly api;
    readonly summary: BotSummary;
    private readonly readState;
    constructor(api: PrayerApi, summary: BotSummary, readState: (options?: RequestOptions) => Promise<StateSnapshot>);
    get id(): string;
    state(options?: RequestOptions): Promise<FleetEntry>;
    queue(options?: RequestOptions): Promise<QueueResponse>;
    normalQueue(options?: RequestOptions): Promise<QueueLane>;
    overrideQueue(options?: RequestOptions): Promise<QueueLane>;
    halt(reason?: string, options?: RequestOptions): Promise<void>;
    startActions(actions: ActionInput, options?: SubmitOptions): Promise<ActionRun>;
    /** @deprecated Prefer the explicit `startActions` name. */
    start(actions: ActionInput, options?: SubmitOptions): Promise<ActionRun>;
    actionRun(runId: string, options?: RequestOptions): Promise<ActionRun>;
    execute(actions: ActionInput, options?: ExecuteOptions): Promise<ActionRunResponse>;
    executeActionOverride(actions: ActionInput, options?: OverrideOptions): Promise<void>;
    executeScriptOverride(script: string, options?: OverrideOptions): Promise<void>;
    startScript(script: string, options?: SubmitOptions): Promise<ScriptRun>;
    scriptRun(runId: string, options?: RequestOptions): Promise<ScriptRun>;
}
declare abstract class Run<T extends ActionRunResponse | ScriptRunResponse> {
    protected readonly api: PrayerApi;
    protected current: T;
    private readonly kind;
    readonly idempotencyKey?: string | undefined;
    constructor(api: PrayerApi, current: T, kind: "action-runs" | "script-runs", idempotencyKey?: string | undefined);
    get id(): string;
    get prayerlang(): string;
    get snapshot(): T;
    get isTerminal(): boolean;
    get succeeded(): boolean;
    get cancellationKind(): "cancelled" | "halted" | undefined;
    abstract get errorMessage(): string | undefined;
    abstract status(options?: RequestOptions): Promise<T>;
    wait(options?: WaitOptions<T>): Promise<T>;
    abstract cancel(reason?: string, options?: RequestOptions): Promise<T>;
}
export declare class ActionRun extends Run<ActionRunResponse> {
    constructor(api: PrayerApi, snapshot: ActionRunResponse, idempotencyKey?: string);
    get errorMessage(): string | undefined;
    status(options?: RequestOptions): Promise<ActionRunResponse>;
    cancel(reason?: string, options?: RequestOptions): Promise<ActionRunResponse>;
}
export declare class ScriptRun extends Run<ScriptRunResponse> {
    constructor(api: PrayerApi, snapshot: ScriptRunResponse, idempotencyKey?: string);
    get errorMessage(): string | undefined;
    status(options?: RequestOptions): Promise<ScriptRunResponse>;
    cancel(reason?: string, options?: RequestOptions): Promise<ScriptRunResponse>;
}
export {};

// conveniences.d.ts
import type { FleetSnapshot, GalaxyCatalog, StationMarketData, StateVersions, StationMarketDelta, WorldState } from "./generated/types.js";
type StationMarkets = Record<string, StationMarketData>;
/** A fully resolved state response after conditional updates have been merged. */
export interface StateSnapshot {
    versions: StateVersions;
    fleet: FleetSnapshot;
    world: WorldState;
    catalog: GalaxyCatalog;
}
/** The conditional world payload accepted by the SDK cache merger. */
export type WorldStateUpdate = Partial<WorldState> & {
    stationMarkets?: StationMarkets | null;
    stationMarketDelta?: StationMarketDelta | null;
};
/** Higher-level navigation target accepted by the action helper. */
export type GoTarget = {
    kind: "identifier" | "system" | "poi";
    value: string;
} | {
    kind: "coordinate";
    value: {
        x: number;
        y: number;
    };
};
export {};

// errors.d.ts
import type { ErrorEnvelope } from "./generated/types.js";
export declare class PrayerApiError extends Error {
    readonly status: number;
    readonly code: string;
    readonly retryable: boolean;
    readonly details?: unknown | undefined;
    readonly requestId?: string | undefined;
    readonly retryAfterMs?: number;
    constructor(status: number, code: string, message: string, retryable: boolean, details?: unknown | undefined, requestId?: string | undefined, options?: ErrorOptions);
    static from(status: number, body: ErrorEnvelope): PrayerApiError;
}
export declare class LaneBusyError extends PrayerApiError {
    constructor(status: number, body: ErrorEnvelope);
}
export declare class PrayerValidationError extends PrayerApiError {
    constructor(status: number, body: ErrorEnvelope);
}
export declare class PrayerAuthenticationError extends PrayerApiError {
    constructor(status: number, body: ErrorEnvelope);
}
export declare class PrayerNotFoundError extends PrayerApiError {
    constructor(status: number, body: ErrorEnvelope);
}
export declare class PrayerConnectionError extends Error {
    readonly cause?: unknown | undefined;
    constructor(message: string, cause?: unknown | undefined);
}
export declare class PrayerTimeoutError extends PrayerConnectionError {
    readonly timeoutMs: number;
    constructor(timeoutMs: number, cause?: unknown);
}
export declare class PrayerAbortError extends Error {
    readonly cause?: unknown | undefined;
    constructor(cause?: unknown | undefined);
}
export declare class PrayerCompatibilityError extends Error {
    constructor(message: string);
}
export declare const isLaneBusyError: (error: unknown) => error is LaneBusyError;
export declare const isNotFoundError: (error: unknown) => error is PrayerNotFoundError;
export declare const isValidationError: (error: unknown) => error is PrayerValidationError;
export declare const isAuthenticationError: (error: unknown) => error is PrayerAuthenticationError;
export declare const isRetryableError: (error: unknown) => error is PrayerApiError | PrayerConnectionError;

// transport.d.ts
export interface RequestOptions {
    signal?: AbortSignal;
    timeoutMs?: number;
}
export interface TransportOptions {
    baseUrl: string;
    token?: string;
    fetch?: typeof globalThis.fetch;
    timeoutMs?: number;
    signal?: AbortSignal;
    headers?: Record<string, string>;
}
export declare class Transport {
    private readonly options;
    private readonly fetcher;
    constructor(options: TransportOptions);
    request<T>(path: string, init?: RequestInit, options?: RequestOptions): Promise<T>;
}

// index.d.ts
export * from "./actions.js";
export * from "./client.js";
export type * from "./conveniences.js";
export * from "./errors.js";
export type * from "./transport.js";

// api.d.ts
/** Advanced, generated HTTP client. Prefer the high-level root package API. */
export { PrayerApi } from "./generated/api.js";
export { Transport } from "./transport.js";

// types.d.ts
/** Generated HTTP wire contracts for advanced integrations. */
export type * from "./generated/types.js";

// generated/api.d.ts
import type { RequestOptions } from "../transport.js";
import { Transport } from "../transport.js";
import type { ActionOverrideRequest, ActionRunRequest, ActionRunResponse, BotList, BotSummary, CancelRequest, CraftReservationResponse, MarketMovement, MarketMovementHealth, MarketMovementList, MarketMovementReserveRequest, MarketMovementReserveResponse, MarketMovementTransitionRequest, Meta, OverrideResponse, QueueLane, QueueResponse, RegisterBotRequest, RegisterBotResponse, ReservationRequest, ReservationResponse, RouteBatchRequest, RouteBatchResponse, ScriptOverrideRequest, ScriptRunRequest, ScriptRunResponse, StateResponse, VirtualCraftOrderList, VirtualCraftOrderWrite, VirtualOrderList, VirtualOrderWrite } from "./types.js";
export declare class PrayerApi {
    private readonly transport;
    constructor(transport: Transport);
    listMarketMovements(options?: RequestOptions): Promise<MarketMovementList>;
    reserveMarketMovement(body: MarketMovementReserveRequest, idempotencyKey: string, options?: RequestOptions): Promise<MarketMovementReserveResponse>;
    completeMarketMovement(movementId: string, idempotencyKey: string, options?: RequestOptions): Promise<MarketMovement>;
    failMarketMovement(movementId: string, idempotencyKey: string, options?: RequestOptions): Promise<MarketMovement>;
    getMarketMovementHealth(movementId: string, options?: RequestOptions): Promise<MarketMovementHealth>;
    reconcileMarketMovement(movementId: string, body: MarketMovementTransitionRequest, idempotencyKey: string, options?: RequestOptions): Promise<MarketMovement>;
    releaseMarketMovement(movementId: string, idempotencyKey: string, options?: RequestOptions): Promise<MarketMovement>;
    startMarketMovement(movementId: string, idempotencyKey: string, options?: RequestOptions): Promise<MarketMovement>;
    listVirtualCraftOrders(options?: RequestOptions): Promise<VirtualCraftOrderList>;
    createVirtualCraftOrders(body: VirtualCraftOrderWrite, idempotencyKey: string, options?: RequestOptions): Promise<VirtualCraftOrderList>;
    reserveVirtualCraftOrders(body: ReservationRequest, idempotencyKey: string, options?: RequestOptions): Promise<CraftReservationResponse>;
    fillVirtualCraftOrder(orderId: string, idempotencyKey: string, options?: RequestOptions): Promise<VirtualCraftOrderList>;
    releaseVirtualCraftOrder(orderId: string, idempotencyKey: string, options?: RequestOptions): Promise<VirtualCraftOrderList>;
    listVirtualOrders(options?: RequestOptions): Promise<VirtualOrderList>;
    createVirtualOrders(body: VirtualOrderWrite, idempotencyKey: string, options?: RequestOptions): Promise<VirtualOrderList>;
    reserveVirtualOrders(body: ReservationRequest, idempotencyKey: string, options?: RequestOptions): Promise<ReservationResponse>;
    fillVirtualOrder(orderId: string, idempotencyKey: string, options?: RequestOptions): Promise<VirtualOrderList>;
    releaseVirtualOrder(orderId: string, idempotencyKey: string, options?: RequestOptions): Promise<VirtualOrderList>;
    listBots(options?: RequestOptions): Promise<BotList>;
    registerBot(body: RegisterBotRequest, options?: RequestOptions): Promise<RegisterBotResponse>;
    getBot(botId: string, options?: RequestOptions): Promise<BotSummary>;
    executeActionOverride(botId: string, body: ActionOverrideRequest, options?: RequestOptions): Promise<OverrideResponse>;
    startActionRun(botId: string, body: ActionRunRequest, idempotencyKey?: string, options?: RequestOptions): Promise<ActionRunResponse>;
    getActionRun(botId: string, runId: string, options?: RequestOptions): Promise<ActionRunResponse>;
    cancelActionRun(botId: string, runId: string, body?: CancelRequest, options?: RequestOptions): Promise<ActionRunResponse>;
    haltBot(botId: string, body?: CancelRequest, options?: RequestOptions): Promise<void>;
    getBotQueue(botId: string, options?: RequestOptions): Promise<QueueResponse>;
    getBotNormalQueue(botId: string, options?: RequestOptions): Promise<QueueLane>;
    getBotOverrideQueue(botId: string, options?: RequestOptions): Promise<QueueLane>;
    executeScriptOverride(botId: string, body: ScriptOverrideRequest, options?: RequestOptions): Promise<OverrideResponse>;
    startScriptRun(botId: string, body: ScriptRunRequest, idempotencyKey?: string, options?: RequestOptions): Promise<ScriptRunResponse>;
    getScriptRun(botId: string, runId: string, options?: RequestOptions): Promise<ScriptRunResponse>;
    cancelScriptRun(botId: string, runId: string, body?: CancelRequest, options?: RequestOptions): Promise<ScriptRunResponse>;
    getMeta(options?: RequestOptions): Promise<Meta>;
    selectRoutes(body: RouteBatchRequest, options?: RequestOptions): Promise<RouteBatchResponse>;
    getState(query?: {
        fleetVersion?: number;
        worldVersion?: number;
        mapVersion?: number;
        resourcesVersion?: number;
        wildlifeVersion?: number;
        marketsVersion?: number;
        storageVersion?: number;
        facilitiesVersion?: number;
        observationsVersion?: number;
        communicationsVersion?: number;
        factionsVersion?: number;
        catalogVersion?: string;
    }, options?: RequestOptions): Promise<StateResponse>;
}

// generated/types.d.ts
export type Action = {
    "type": "halt";
} | {
    "request": {
        "ticks": number;
    };
    "type": "wait";
} | {
    "request": {
        "destination": GoTarget;
    };
    "type": "go";
} | {
    "type": "dock";
} | {
    "type": "undock";
} | {
    "request": {
        "resource"?: string | null;
    };
    "type": "mine";
} | {
    "request": TransferRequest;
    "type": "transfer";
} | {
    "type": "set_home";
} | {
    "request": FindRequest;
    "type": "find";
} | {
    "type": "survey";
} | {
    "request": {
        "target_id": string;
    };
    "type": "attack";
} | {
    "request": {
        "target"?: string | null;
    };
    "type": "scan";
} | {
    "request": {
        "enabled": boolean;
    };
    "type": "cloak";
} | {
    "request": {
        "target": string;
    };
    "type": "hunt";
} | {
    "request": {
        "quantity": number;
    };
    "type": "prepay_tax";
} | {
    "request": {
        "mission_id": string;
    };
    "type": "accept_mission";
} | {
    "request": {
        "mission_id": string;
    };
    "type": "abandon_mission";
} | {
    "request": {
        "template_id": string;
    };
    "type": "decline_mission";
} | {
    "request": {
        "mission_id": string;
    };
    "type": "complete_mission";
} | {
    "request": {
        "destination": string;
    };
    "type": "load_passenger";
} | {
    "request": {
        "name": string;
        "target"?: string | null;
    };
    "type": "unload_passenger";
} | {
    "request": BuyRequest;
    "type": "buy";
} | {
    "request": SellRequest;
    "type": "sell";
} | {
    "request": {
        "item": string;
    };
    "type": "cancel_buy";
} | {
    "request": {
        "item": string;
    };
    "type": "cancel_sell";
} | {
    "request": {
        "name": string;
        "tag": string;
    };
    "type": "faction_create";
} | {
    "request": {
        "player": string;
    };
    "type": "faction_invite";
} | {
    "request": {
        "faction": string;
    };
    "type": "faction_accept_invite";
} | {
    "request": {
        "player": string;
    };
    "type": "faction_kick";
} | {
    "request": {
        "player": string;
        "role": string;
    };
    "type": "faction_set_role";
} | {
    "request": {
        "facility_type": string;
    };
    "type": "facility_build";
} | {
    "request": {
        "facility_type": string;
    };
    "type": "faction_facility_build";
} | {
    "request": FacilityUpgradeRequest;
    "type": "facility_upgrade";
} | {
    "request": FacilityUpgradeRequest;
    "type": "faction_facility_upgrade";
} | {
    "request": {
        "facility_id": string;
    };
    "type": "facility_dismantle";
} | {
    "request": {
        "facility_id": string;
    };
    "type": "faction_facility_dismantle";
} | {
    "request": FacilityAccessRequest;
    "type": "facility_set_access";
} | {
    "request": FacilityOutputPriceRequest;
    "type": "facility_set_output_price";
} | {
    "request": FacilityNameRequest;
    "type": "facility_set_name";
} | {
    "request": {
        "item": string;
        "quantity": number;
    };
    "type": "use_item";
} | {
    "request": ServiceTransferRequest;
    "type": "repair";
} | {
    "request": {
        "module": string;
    };
    "type": "repair_module";
} | {
    "request": RecycleRequest;
    "type": "recycle";
} | {
    "request": ServiceTransferRequest;
    "type": "refuel";
} | {
    "type": "self_destruct";
} | {
    "request": {
        "ship": string;
    };
    "type": "switch_ship";
} | {
    "request": {
        "name": string;
    };
    "type": "rename_ship";
} | {
    "request": {
        "module": string;
    };
    "type": "install_mod";
} | {
    "request": {
        "module": string;
    };
    "type": "uninstall_mod";
} | {
    "request": {
        "listing": string;
    };
    "type": "buy_ship";
} | {
    "request": {
        "listing": string;
    };
    "type": "buy_listed_ship";
} | {
    "request": CommissionShipRequest;
    "type": "commission_ship";
} | {
    "request": {
        "ship": string;
    };
    "type": "sell_ship";
} | {
    "request": {
        "ship": string;
    };
    "type": "scrap_ship";
} | {
    "request": {
        "price": number;
        "ship": string;
    };
    "type": "list_ship_for_sale";
} | {
    "type": "refit_ship";
} | {
    "request": {
        "commission_id": string;
    };
    "type": "cancel_commission";
} | {
    "request": {
        "commission_id": string;
        "item": string;
        "quantity": number;
    };
    "type": "supply_commission";
} | {
    "request": {
        "listing_id": string;
    };
    "type": "cancel_ship_listing";
} | {
    "request": {
        "price": number;
        "ship_class": string;
    };
    "type": "place_ship_buy_order";
} | {
    "request": {
        "order_id": string;
    };
    "type": "cancel_ship_buy_order";
} | {
    "request": {
        "order_id": string;
        "ship_id": string;
    };
    "type": "sell_ship_to_order";
} | {
    "request": {
        "order_id": string;
    };
    "type": "cancel_order";
} | {
    "request": {
        "order_id": string;
        "price_each": number;
    };
    "type": "modify_order";
} | {
    "request": CraftRequest;
    "type": "craft";
} | {
    "request": {
        "job_id": string;
    };
    "type": "cancel_craft_job";
} | {
    "request": {
        "wreck_id": string;
    };
    "type": "salvage_wreck";
} | {
    "request": {
        "wreck_id": string;
    };
    "type": "tow_wreck";
} | {
    "type": "scrap_wreck";
} | {
    "type": "sell_wreck";
} | {
    "type": "release_wreck";
} | {
    "request": {
        "ticks": number;
    };
    "type": "insure_ship";
} | {
    "request": {
        "empire_id": string;
    };
    "type": "citizenship_apply";
} | {
    "request": {
        "empire_id": string;
    };
    "type": "citizenship_withdraw";
} | {
    "request": {
        "empire_id": string;
    };
    "type": "citizenship_renounce";
} | {
    "request": TradeOfferRequest;
    "type": "trade_offer";
} | {
    "request": {
        "trade_id": string;
    };
    "type": "trade_accept";
} | {
    "type": "faction_leave";
} | {
    "request": {
        "player": string;
    };
    "type": "faction_withdraw_invite";
} | {
    "request": {
        "faction": string;
    };
    "type": "faction_propose_ally";
} | {
    "request": {
        "faction": string;
    };
    "type": "faction_accept_ally";
} | {
    "request": {
        "faction": string;
    };
    "type": "faction_remove_ally";
} | {
    "request": {
        "faction": string;
        "reason"?: string | null;
    };
    "type": "faction_declare_war";
} | {
    "request": {
        "faction": string;
        "message"?: string | null;
    };
    "type": "faction_propose_peace";
} | {
    "request": {
        "faction": string;
    };
    "type": "faction_accept_peace";
} | {
    "request": {
        "faction": string;
    };
    "type": "faction_set_enemy";
} | {
    "request": {
        "faction": string;
    };
    "type": "faction_remove_enemy";
} | {
    "request": {
        "quantity": number;
    };
    "type": "faction_prepay_tax";
} | {
    "request": {
        "mission_id": string;
    };
    "type": "faction_cancel_mission";
} | {
    "type": "espionage";
} | {
    "request": {
        "poi_id": string;
    };
    "type": "scan_poi";
} | {
    "request": {
        "distress_type"?: string | null;
    };
    "type": "distress_signal";
} | {
    "request": SayRequest;
    "type": "say";
};
export interface ActionOverrideRequest {
    "actions": Array<Action>;
    "returnToOrigin"?: boolean;
}
export type ActionRunOutcome = {
    "status": "succeeded";
} | {
    "action_index": number;
    "message": string;
    "status": "failed";
} | {
    "reason": string;
    "status": "cancelled";
} | {
    "reason": string;
    "status": "halted";
};
export interface ActionRunRequest {
    "actions": Array<Action>;
    "idempotencyKey"?: string | null;
}
export type ActionRunResponse = {
    "botId": string;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "running";
} | {
    "botId": string;
    "outcome": ActionRunOutcome;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "succeeded";
} | {
    "botId": string;
    "outcome": ActionRunOutcome;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "failed";
} | {
    "botId": string;
    "outcome": ActionRunOutcome;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "cancelled";
} | {
    "botId": string;
    "outcome": ActionRunOutcome;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "halted";
};
export interface ActiveRoute {
    "estimatedFuelUse": number;
    "hops": Array<string>;
    "target": string;
    "targetPoi"?: string | null;
    "targetSystem": string;
    "totalJumps": number;
}
export interface ActorPassengerState {
    "aboard": Array<PassengerView>;
    "aboard_count"?: number | null;
    "business_berths": PassengerBerthView;
    "business_berths_raw": string;
    "economy_berths": PassengerBerthView;
    "economy_berths_raw": string;
    "first_berths": PassengerBerthView;
    "first_berths_raw": string;
}
export interface AgentSightingData {
    "contact": NearbyPlayer;
    "first_seen_unix": number;
    "last_seen_system": string;
    "last_seen_unix": number;
    "times_seen": number;
}
export interface AmmoStats {
    "anti_drone_mod"?: number | null;
    "anti_large_mod"?: number | null;
    "anti_small_mod"?: number | null;
    "armor_bypass"?: number | null;
    "armor_melt_pct"?: number | null;
    "armor_melt_ticks"?: number | null;
    "damage_mod"?: number | null;
    "disrupt_bonus_speed"?: number | null;
    "disrupt_bonus_ticks"?: number | null;
    "disrupt_damage"?: number | null;
    "disrupt_speed"?: number | null;
    "disrupt_ticks"?: number | null;
    "dot_pct"?: number | null;
    "dot_ticks"?: number | null;
    "hit_chance_mod"?: number | null;
    "hull_damage_mod"?: number | null;
    "shield_bypass"?: number | null;
    "shield_damage_mod"?: number | null;
    "splash_pct"?: number | null;
    "untraceable"?: boolean | null;
    "wear_per_shot"?: number | null;
}
export type BotConnectionState = "Connected" | "Disconnected";
export type BotList = Array<V1BotSummary>;
export interface BotState {
    "active_commissions": Array<CommissionEntry>;
    "cargo": Record<string, number>;
    "cargo_capacity": number;
    "cargo_items": Array<V2GameStateCargoItem>;
    "cargo_pct": number;
    "cargo_used": number;
    "combat_stance"?: string | null;
    "combat_target"?: string | null;
    "crafting_queue": Array<CraftingQueueProjection>;
    "fuel": number;
    "fuel_pct": number;
    "in_battle": boolean;
    "installed_modules": Array<string>;
    "last_mined": Record<string, number>;
    "last_stored": Record<string, number>;
    "location": V2GameStateLocation;
    "max_fuel": number;
    "mission_complete": Record<string, boolean>;
    "missions": MissionData;
    "modules": Array<V2GameStateModulesItem>;
    "observation_nearby": Record<string, ObservedPlayer>;
    "own_buy_orders": Array<ExchangeOrder>;
    "own_sell_orders": Array<ExchangeOrder>;
    "owned_ship_details": Array<OwnedShipDetail>;
    "passengers": ActorPassengerState;
    "player": V2GameStatePlayer;
    "script_mined_by_item": Record<string, number>;
    "script_stored_by_item": Record<string, number>;
    "ship": V2GameStateShip;
    "skills": Record<string, V2GameStateSkillsValue>;
}
export interface BotSummary {
    "botId": string;
    "connection": V1BotConnectionState;
    "name"?: string | null;
    "observedAt"?: string | null;
    "stateVersion": number;
}
export interface BulkJobCancelResponse {
    "action": BulkJobCancelResponseAction;
    "kind": BulkJobCancelResponseKind;
    "message": string;
    "mode": BulkJobCancelResponseMode;
    "results": Array<JobCancelResult>;
    "summary": BulkSummary;
}
export type BulkJobCancelResponseAction = "job_cancel";
export type BulkJobCancelResponseKind = "bulk_cancel";
export type BulkJobCancelResponseMode = "bulk";
export interface BulkSummary {
    "failed": number;
    "succeeded": number;
    "total": number;
}
export interface BuyRequest {
    "deliver_to"?: string | null;
    "item": string;
    "max_price"?: number | null;
    "place_order": boolean;
    "quantity": number;
}
export interface CancelRequest {
    "reason"?: string | null;
}
export type CatalogDumpItemsItem = Item | Module;
export interface ChatMessageData {
    "channel": string;
    "content": string;
    "empire_official"?: boolean | null;
    "faction_id"?: string | null;
    "id": string;
    "poi_id"?: string | null;
    "sender": string;
    "sender_id": string;
    "system_id"?: string | null;
    "target_id"?: string | null;
    "target_name"?: string | null;
    "timestamp_utc": string;
}
export interface CommissionEntry {
    "base_id"?: string | null;
    "base_name"?: string | null;
    "build_complete_tick"?: number | null;
    "build_start_tick"?: number | null;
    "built_ship_id"?: string | null;
    "commission_id": string;
    "created_at"?: string | null;
    "credits_paid"?: number | null;
    "earmarked_credits"?: number | null;
    "material_cost_estimate"?: number | null;
    "materials_gathered"?: Record<string, number>;
    "materials_provided": boolean;
    "required_materials"?: Record<string, number>;
    "ship_class_id": string;
    "ship_name"?: string | null;
    "status": string;
    "ticks_remaining"?: number | null;
}
export interface CommissionShipRequest {
    "provide_materials"?: boolean;
    "ship_class": string;
}
export interface CraftJobResponse {
    "action": CraftJobResponseAction;
    "effective_time_per_run": number;
    "escrowed": EscrowSummary;
    "est_completion_tick": number;
    "external"?: boolean | null;
    "facility_id": string;
    "job_id": string;
    "kind": CraftJobResponseKind;
    "message": string;
    "mode": CraftJobResponseMode;
    "produces"?: Array<ItemQuantity>;
    "recipe": string;
    "runs": number;
    "venue": string;
    "venue_type": string;
}
export type CraftJobResponseAction = "craft" | "recycle" | "job_add";
export type CraftJobResponseKind = "job";
export type CraftJobResponseMode = "craft" | "recycle";
export type CraftJobStatus = "optimistic" | "active" | "completed" | "cancelled" | "failed" | "lost";
export interface CraftRequest {
    "destination"?: string | null;
    "facility_id"?: string | null;
    "preset"?: string | null;
    "quantity": number;
    "recipe_id": string;
    "source"?: string | null;
}
export interface CraftReservationResponse {
    "orders": Array<VirtualCraftOrder>;
    "reservationResults": Array<RuntimeVirtualOrderReservationResultDto>;
}
export interface CraftingQueueProjection {
    "crafts"?: number | null;
    "facility_id"?: string | null;
    "item_id"?: string | null;
    "job_id"?: string | null;
    "order_id"?: string | null;
    "preset"?: string | null;
    "quantity"?: number | null;
    "raw_text"?: string | null;
    "recipe_id"?: string | null;
    "reservation_id"?: string | null;
    "source"?: string | null;
    "station_id"?: string | null;
    "status"?: CraftJobStatus | null;
}
export type EmptyRequest = Record<string, never>;
export interface ErrorEnvelope {
    "error": V1ErrorDetail;
    "requestId": string;
}
export interface EscrowSummary {
    "fee"?: number | null;
    "inputs"?: Array<ItemQuantity>;
    "labor"?: number | null;
}
export interface ExchangeOrder {
    "created_at": string;
    "created_by"?: string | null;
    "faction_order"?: boolean | null;
    "filled_quantity": number;
    "item_id": string;
    "item_name"?: string | null;
    "listing_fee": number;
    "order_id": string;
    "order_type": string;
    "price_each": number;
    "quantity": number;
    "remaining": number;
    "side": string;
}
export interface FacilityAccessRequest {
    "access": string;
    "facility_id": string;
}
export interface FacilityBrowseForSaleResponse {
    "action": FacilityBrowseForSaleResponseAction;
    "base_id": string;
    "base_name": string;
    "count": number;
    "listings": Array<FacilityListingEntry>;
}
export type FacilityBrowseForSaleResponseAction = "browse_for_sale";
export interface FacilityBuildResponse {
    "action": FacilityBuildResponseAction;
    "base_id": string;
    "facility_id": string;
    "facility_name": string;
    "facility_type": string;
    "hint": string;
    "recipe_id"?: string | null;
    "rent_per_cycle": number;
    "skill_xp"?: Record<string, number>;
}
export type FacilityBuildResponseAction = "build";
export interface FacilityBuyListingResponse {
    "action": FacilityBuyListingResponseAction;
    "credits_left": number;
    "definition_id": string;
    "facility_id": string;
    "message": string;
    "price": number;
    "sales_tax"?: number | null;
}
export type FacilityBuyListingResponseAction = "buy_listing";
export interface FacilityCancelListingResponse {
    "action": FacilityCancelListingResponseAction;
    "facility_id": string;
    "message": string;
}
export type FacilityCancelListingResponseAction = "cancel_listing";
export interface FacilityCategoryInfo {
    "buildable"?: number | null;
    "count": number;
    "description": string;
}
export interface FacilityDefSummary {
    "build_cost": number;
    "build_materials"?: Array<ItemQuantity>;
    "build_time": number;
    "category": string;
    "description": string;
    "faction_cap"?: number | null;
    "faction_service"?: string | null;
    "labor_cost": number;
    "level": number;
    "maintenance_per_cycle"?: Array<ItemQuantity>;
    "name": string;
    "recipe_id"?: string | null;
    "rent_per_cycle": number;
    "type_id": string;
}
export interface FacilityDefinition {
    "allows_contraband"?: boolean | null;
    "always_on": boolean;
    "ammo_item"?: string | null;
    "battery_capacity"?: number | null;
    "build_cost": number;
    "build_materials"?: Array<RecipeInput>;
    "build_time": number;
    "category": string;
    "degraded_description"?: string | null;
    "deposit_to_empire_reserves"?: boolean | null;
    "description": string;
    "dining_points"?: number | null;
    "disguised"?: boolean | null;
    "empire"?: string | null;
    "expansion_of"?: string | null;
    "expansion_scale"?: number | null;
    "faction_cap"?: number | null;
    "faction_service_type"?: string | null;
    "fleet_upkeep"?: boolean | null;
    "fuel_capacity"?: number | null;
    "fuel_output"?: boolean | null;
    "id": string;
    "is_recycler"?: boolean | null;
    "labor_cost": number;
    "leisure_points"?: number | null;
    "level": number;
    "life_support_draw"?: number | null;
    "life_support_supply"?: number | null;
    "logistics"?: boolean | null;
    "lore"?: string | null;
    "maintenance_inputs"?: Array<RecipeInput>;
    "name": string;
    "personal_bonus_type"?: string | null;
    "personal_bonus_value"?: number | null;
    "personal_service_type"?: string | null;
    "pirate_base_only"?: boolean | null;
    "player_station_buildable"?: boolean | null;
    "power_draw"?: number | null;
    "power_supply"?: number | null;
    "recipe_id"?: string | null;
    "repair_hull_per_item"?: number | null;
    "repair_item"?: string | null;
    "requires_service_type"?: string | null;
    "satisfied_description"?: string | null;
    "scan_falloff"?: number | null;
    "scan_power"?: number | null;
    "self_repair_rate"?: number | null;
    "service_type"?: string | null;
    "station_armor"?: number | null;
    "station_hull_hp"?: number | null;
    "station_or_faction_only"?: boolean | null;
    "station_shield_hp"?: number | null;
    "tourism_upkeep"?: boolean | null;
    "transit_deadline_bonus"?: number | null;
    "unique"?: boolean | null;
    "upgrades_from"?: string | null;
    "weapon_cooldown"?: number | null;
    "weapon_damage"?: number | null;
    "weapon_damage_type"?: string | null;
    "weapon_reach"?: number | null;
}
export interface FacilityDismantleMaterial {
    "item_id": string;
    "quantity": number;
}
export interface FacilityDismantleResponse {
    "action": FacilityDismantleResponseAction;
    "base_id": string;
    "complete_tick": number;
    "facility_id": string;
    "facility_name": string;
    "facility_type": string;
    "hint": string;
    "materials_to_package": Array<FacilityDismantleMaterial>;
    "package_count": number;
    "ticks_to_complete": number;
}
export type FacilityDismantleResponseAction = "dismantle" | "faction_dismantle";
export interface FacilityEntry {
    "bonus_type"?: string | null;
    "bonus_value"?: number | null;
    "capacity"?: number | null;
    "category": string;
    "custom_name"?: string | null;
    "damaged"?: boolean | null;
    "description": string;
    "dining_points"?: number | null;
    "facility_id": string;
    "faction_id"?: string | null;
    "faction_service"?: string | null;
    "is_recycler"?: boolean | null;
    "labor_per_cycle"?: number | null;
    "leisure_points"?: number | null;
    "level": number;
    "maintenance_level"?: number | null;
    "maintenance_per_cycle"?: Array<ItemQuantity>;
    "maintenance_satisfied"?: boolean | null;
    "missed_rent_cycles"?: number | null;
    "name": string;
    "owner_id"?: string | null;
    "personal_service"?: string | null;
    "power_throttled"?: boolean | null;
    "production"?: FacilityProduction | null;
    "recipe_id"?: string | null;
    "rent_paid_until_tick"?: number | null;
    "rent_per_cycle"?: number | null;
    "repair_complete_tick"?: number | null;
    "service"?: string | null;
    "tourism_upkeep"?: boolean | null;
    "type": string;
    "under_construction"?: boolean | null;
}
export interface FacilityFactionBuildResponse {
    "action": FacilityFactionBuildResponseAction;
    "base_id": string;
    "capacity"?: number | null;
    "facility_id": string;
    "facility_name": string;
    "facility_type": string;
    "faction_service": string;
    "hint": string;
    "members_awarded_xp"?: number | null;
    "recipe_id"?: string | null;
    "rent_per_cycle": number;
    "skill_xp"?: Record<string, number>;
    "under_construction"?: boolean | null;
}
export type FacilityFactionBuildResponseAction = "faction_build";
export interface FacilityFactionEntry {
    "capacity"?: number | null;
    "custom_name"?: string | null;
    "facility_id": string;
    "faction_service": string;
    "level": number;
    "missed_rent_cycles"?: number | null;
    "name": string;
    "rent_per_cycle": number;
    "rental_fee_per_run"?: number | null;
    "status": string;
    "ticks_until_complete"?: number | null;
    "type": string;
}
export interface FacilityFactionListResponse {
    "action": FacilityFactionListResponseAction;
    "base_id": string;
    "faction_facilities": Array<FacilityFactionEntry>;
    "faction_id": string;
    "faction_storage"?: FacilityFactionStorage | null;
    "hint": string;
}
export type FacilityFactionListResponseAction = "faction_list";
export interface FacilityFactionOwnedResponse {
    "action": FacilityFactionOwnedResponseAction;
    "arrears_owed"?: number | null;
    "facilities": Array<FactionOwnedFacilityEntry>;
    "faction_id": string;
    "grace_cycles"?: number | null;
    "hint"?: string | null;
    "note"?: string | null;
    "total_rent_per_cycle": number;
}
export type FacilityFactionOwnedResponseAction = "faction_owned";
export interface FacilityFactionStorage {
    "credits": number;
    "item_types": number;
    "rooms": number;
}
export interface FacilityFactionUpgradeResponse {
    "action": FacilityFactionUpgradeResponseAction;
    "base_id": string;
    "capacity"?: number | null;
    "facility_id": string;
    "facility_name": string;
    "facility_type": string;
    "faction_service": string;
    "hint": string;
    "level": number;
    "members_awarded_xp"?: number | null;
    "skill_xp"?: Record<string, number>;
}
export type FacilityFactionUpgradeResponseAction = "faction_upgrade";
export interface FacilityHelpResponse {
    "action": FacilityHelpResponseAction;
    "help": string;
}
export type FacilityHelpResponseAction = "help";
export interface FacilityJobListResponse {
    "action": FacilityJobListResponseAction;
    "facility_id": string;
    "jobs": Array<JobView>;
    "message"?: string | null;
    "total_jobs": number;
    "venue": string;
}
export type FacilityJobListResponseAction = "job_list";
export interface FacilityListForSaleResponse {
    "action": FacilityListForSaleResponseAction;
    "credits_left"?: number | null;
    "definition_id": string;
    "facility_id": string;
    "fee": number;
    "listing_id": string;
    "message": string;
    "price": number;
}
export type FacilityListForSaleResponseAction = "list_for_sale";
export interface FacilityListResponse {
    "action": FacilityListResponseAction;
    "base_id": string;
    "construction"?: StationConstructionResponse | null;
    "faction_facilities": Array<FacilityEntry>;
    "faction_rent"?: FacilityRentSummary | null;
    "life_support"?: StationLifeSupportStatus | null;
    "player_facilities": Array<FacilityEntry>;
    "player_rent"?: FacilityRentSummary | null;
    "power"?: StationPowerStatus | null;
    "public_facilities"?: Array<FacilityEntry>;
    "station_facilities": Array<FacilityEntry>;
}
export type FacilityListResponseAction = "list";
export interface FacilityListingEntry {
    "build_cost"?: number | null;
    "build_time"?: number | null;
    "category"?: string | null;
    "compatibility_note"?: string | null;
    "definition_id": string;
    "facility_id": string;
    "facility_name"?: string | null;
    "fuel_capacity_bonus"?: number | null;
    "fuel_output"?: boolean | null;
    "level"?: number | null;
    "listed_at": string;
    "listing_id": string;
    "price": number;
    "recipe_id"?: string | null;
    "required_skill_level"?: number | null;
    "seller_name"?: string | null;
    "seller_type": string;
    "skill_met"?: boolean | null;
    "station_or_faction_only"?: boolean | null;
    "under_construction"?: boolean | null;
}
export interface FacilityNameRequest {
    "custom_name": string;
    "facility_id": string;
}
export interface FacilityOutputPriceRequest {
    "facility_id": string;
    "item": string;
    "price": number;
}
export interface FacilityOwnedResponse {
    "action": FacilityOwnedResponseAction;
    "facilities": Array<OwnedFacilityEntry>;
    "hint"?: string | null;
    "rent": FacilityRentSummary;
}
export type FacilityOwnedResponseAction = "owned";
export interface FacilityPersonalBuildResponse {
    "action": FacilityPersonalBuildResponseAction;
    "base_id": string;
    "bonus_type"?: string | null;
    "bonus_value"?: number | null;
    "facility_id": string;
    "facility_name": string;
    "facility_type": string;
    "hint": string;
    "home_base_set"?: boolean | null;
    "personal_service": string;
    "rent_per_cycle": number;
    "skill_xp"?: Record<string, number>;
    "under_construction"?: boolean | null;
}
export type FacilityPersonalBuildResponseAction = "personal_build";
export interface FacilityPersonalDecorateResponse {
    "access": string;
    "action": FacilityPersonalDecorateResponseAction;
    "facility_id": string;
    "facility_name": string;
    "hint"?: string | null;
    "message"?: string | null;
}
export type FacilityPersonalDecorateResponseAction = "personal_decorate";
export interface FacilityPersonalVisitResponse {
    "access"?: string | null;
    "action": FacilityPersonalVisitResponseAction;
    "base_id": string;
    "description": string;
    "facility_name": string;
    "hint"?: string | null;
    "owner": string;
}
export type FacilityPersonalVisitResponseAction = "personal_visit";
export interface FacilityProduction {
    "backlog_ticks": number;
    "items_per_hour"?: number | null;
    "output_per_run"?: number | null;
    "output_price_per_operation"?: number | null;
    "output_price_per_unit"?: number | null;
    "pack_operations_per_hour"?: number | null;
    "public"?: boolean | null;
    "queued_items": number;
    "queued_runs": number;
    "recipe"?: string | null;
    "rental_fee_per_run"?: number | null;
    "ticks_per_run"?: number | null;
    "unpack_operations_per_hour"?: number | null;
}
export interface FacilityRecipeInfo {
    "crafting_time": number;
    "id": string;
    "inputs": Array<ItemQuantity>;
    "name": string;
    "outputs": Array<ItemQuantity>;
}
export interface FacilityRentSummary {
    "arrears_owed"?: number | null;
    "est_rent_per_day": number;
    "facilities": number;
    "grace_cycles"?: number | null;
    "note"?: string | null;
    "total_rent_per_cycle": number;
}
export interface FacilityRepairMaterial {
    "item_id": string;
    "quantity": number;
}
export interface FacilityRepairResponse {
    "action": FacilityRepairResponseAction;
    "complete_tick": number;
    "facility_id": string;
    "facility_name": string;
    "hint": string;
    "materials_used": Array<FacilityRepairMaterial>;
    "ticks_to_complete": number;
}
export type FacilityRepairResponseAction = "repair";
export type FacilityResponse = FacilityListResponse | FacilityOwnedResponse | FacilityFactionOwnedResponse | FacilityHelpResponse | FacilityBuildResponse | FacilityUpgradesResponse | FacilityUpgradeResponse | FacilityDismantleResponse | FacilityRepairResponse | FacilityFactionBuildResponse | FacilityFactionUpgradeResponse | FacilityFactionListResponse | FacilityTransferResponse | FacilityPersonalBuildResponse | FacilityPersonalDecorateResponse | FacilityPersonalVisitResponse | FacilityTypeDiscoveryResponse | FacilityTypeListResponse | FacilityTypeDetailResponse | FacilityListForSaleResponse | FacilityBrowseForSaleResponse | FacilityBuyListingResponse | FacilityCancelListingResponse | CraftJobResponse | PackageJobResponse | FacilityJobListResponse | JobCancelResponse | BulkJobCancelResponse | JobReorderResponse | SetOutputPriceResponse | SetAccessResponse | SetFacilityNameResponse | SetFacilityDescriptionResponse;
export interface FacilityTransferResponse {
    "action": FacilityTransferResponseAction;
    "direction": string;
    "facility_id": string;
    "hint": string;
    "new_owner"?: string | null;
}
export type FacilityTransferResponseAction = "transfer";
export interface FacilityTypeDetailResponse {
    "action": FacilityTypeDetailResponseAction;
    "bonus_type"?: string | null;
    "bonus_value"?: number | null;
    "build_cost": number;
    "build_materials"?: Array<ItemQuantity>;
    "build_time": number;
    "buildable": boolean;
    "category": string;
    "degraded_description"?: string | null;
    "description": string;
    "faction_cap"?: number | null;
    "faction_service"?: string | null;
    "hint"?: string | null;
    "kind": FacilityTypeDetailResponseKind;
    "labor_cost": number;
    "level": number;
    "lore"?: string | null;
    "maintenance_per_cycle"?: Array<ItemQuantity>;
    "name": string;
    "personal_service"?: string | null;
    "recipe"?: FacilityRecipeInfo | null;
    "recipe_id"?: string | null;
    "rent_per_cycle": number;
    "requires_service_name"?: string | null;
    "requires_service_type"?: string | null;
    "satisfied_description"?: string | null;
    "type_id": string;
    "upgrades_from"?: string | null;
    "upgrades_from_name"?: string | null;
    "upgrades_to"?: string | null;
    "upgrades_to_name"?: string | null;
}
export type FacilityTypeDetailResponseAction = "types";
export type FacilityTypeDetailResponseKind = "detail";
export interface FacilityTypeDiscoveryResponse {
    "action": FacilityTypeDiscoveryResponseAction;
    "categories": Record<string, FacilityCategoryInfo>;
    "filters": FacilityTypeFilterInfo;
    "hint": string;
    "kind": FacilityTypeDiscoveryResponseKind;
    "pagination": FacilityTypePaginationInfo;
    "total": number;
}
export type FacilityTypeDiscoveryResponseAction = "types";
export type FacilityTypeDiscoveryResponseKind = "discovery";
export interface FacilityTypeFilterInfo {
    "category": string;
    "level": string;
    "name": string;
}
export interface FacilityTypeListResponse {
    "action": FacilityTypeListResponseAction;
    "hint": string;
    "kind": FacilityTypeListResponseKind;
    "page": number;
    "per_page": number;
    "total": number;
    "total_pages": number;
    "types": Array<FacilityTypeSummary>;
}
export type FacilityTypeListResponseAction = "types";
export type FacilityTypeListResponseKind = "list";
export interface FacilityTypePaginationInfo {
    "page": string;
    "per_page": string;
}
export interface FacilityTypeSummary {
    "bonus_type"?: string | null;
    "bonus_value"?: number | null;
    "build_cost": number;
    "buildable"?: boolean | null;
    "category": string;
    "id": string;
    "level": number;
    "name": string;
    "personal_service"?: string | null;
    "recipe_id"?: string | null;
    "service"?: string | null;
}
export interface FacilityUpgradeEntry {
    "current_level": number;
    "requires"?: string | null;
    "upgrade_to": FacilityDefSummary;
    "your_facility_id": string;
    "your_facility_name": string;
    "your_facility_type": string;
}
export interface FacilityUpgradeRequest {
    "facility_id": string;
    "facility_type": string;
}
export interface FacilityUpgradeResponse {
    "action": FacilityUpgradeResponseAction;
    "base_id": string;
    "bonus_type"?: string | null;
    "bonus_value"?: number | null;
    "facility_id": string;
    "facility_name": string;
    "facility_type": string;
    "hint": string;
    "level": number;
    "personal_service"?: string | null;
    "recipe_id"?: string | null;
    "rent_per_cycle": number;
}
export type FacilityUpgradeResponseAction = "upgrade";
export interface FacilityUpgradesResponse {
    "action": FacilityUpgradesResponseAction;
    "base_id": string;
    "faction_locked_upgrades"?: Array<FacilityUpgradeEntry>;
    "faction_upgrade_hint"?: string | null;
    "faction_upgrades"?: Array<FacilityUpgradeEntry>;
    "hint": string;
    "locked_upgrades"?: Array<FacilityUpgradeEntry>;
    "upgrades": Array<FacilityUpgradeEntry>;
}
export type FacilityUpgradesResponseAction = "upgrades";
export interface FactionMemberData {
    "online": boolean;
    "player_id": string;
    "role": string;
    "username": string;
}
export interface FactionOwnedFacilityEntry {
    "arrears_owed"?: number | null;
    "base_id": string;
    "base_name": string;
    "custom_name"?: string | null;
    "damaged"?: boolean | null;
    "facility_id": string;
    "labor_per_run": number;
    "missed_rent_cycles"?: number | null;
    "name": string;
    "power_throttled"?: boolean | null;
    "rent_per_cycle": number;
    "rental_fee_per_run"?: number | null;
    "repair_complete_tick"?: number | null;
    "system_id"?: string | null;
    "type": string;
    "under_construction"?: boolean | null;
}
export interface FactionRoleData {
    "name": string;
    "priority": number;
}
export interface FactionSnapshotData {
    "description": string;
    "id": string;
    "is_member": boolean;
    "leader_id": string;
    "leader_username": string;
    "member_count": number;
    "members": Array<FactionMemberData>;
    "name": string;
    "primary_color": string;
    "roles": Array<FactionRoleData>;
    "secondary_color": string;
    "tag": string;
    "treasury"?: number | null;
}
export interface FindRequest {
    "targets": Array<string>;
}
export interface FleetEntry {
    "active_route"?: ActiveRoute | null;
    "connection": BotConnectionState;
    "id": string;
    "in_transit"?: boolean;
    "observed_at"?: string | null;
    "script_execution"?: ScriptExecution | null;
    "state": BotState;
    "transit_dest_poi"?: string | null;
    "transit_dest_system"?: string | null;
    "username"?: string | null;
    "version": number;
}
export interface FleetSnapshot {
    "bots": Record<string, FleetEntry>;
}
export interface GalaxyCatalog {
    "facilitiesById": Record<string, FacilityDefinition>;
    "itemsById": Record<string, CatalogDumpItemsItem>;
    "recipesById": Record<string, Recipe>;
    "shipsById": Record<string, ShipClass>;
    "skillsById": Record<string, SkillDefinition>;
}
export interface GalaxyMap {
    "knownPois": Array<RuntimeGalaxyKnownPoiInfoDto>;
    "systems": Array<RuntimeGalaxySystemInfoDto>;
}
export interface GalaxyResources {
    "poisByResource": Record<string, Array<string>>;
    "systemsByResource": Record<string, Array<string>>;
}
export interface GalaxyWildlife {
    "pois": Array<RuntimeWildlifePoiDto>;
    "systems": Array<RuntimeWildlifeSystemDto>;
}
export type GoTarget = {
    "kind": "identifier";
    "value": string;
} | {
    "kind": "system";
    "value": string;
} | {
    "kind": "poi";
    "value": string;
} | {
    "kind": "coordinate";
    "value": {
        "x": number;
        "y": number;
    };
};
export interface InventoryClaim {
    "itemId": string;
    "locationId": string;
    "lotId"?: string | null;
    "ownerId": string;
    "quantity": number;
    "sourceKind": string;
}
export interface Item {
    "base_value": number;
    "category": string;
    "description": string;
    "effect"?: ItemEffect | null;
    "extracted_by"?: string | null;
    "food_type"?: string | null;
    "hazardous"?: boolean | null;
    "hidden"?: boolean | null;
    "id": string;
    "name": string;
    "quest_item"?: boolean | null;
    "rarity"?: string | null;
    "region_lock"?: Array<string>;
    "size": number;
    "stackable": boolean;
    "tradeable": boolean;
}
export interface ItemEffect {
    "ammo"?: AmmoStats | null;
    "amount"?: number | null;
    "duration"?: number | null;
    "stat"?: string | null;
    "subtype"?: string | null;
    "type": string;
}
export interface ItemQuantity {
    "item_id": string;
    "name": string;
    "quantity": number;
}
export interface JobCancelResponse {
    "action": JobCancelResponseAction;
    "job_id": string;
    "kind": JobCancelResponseKind;
    "message": string;
    "refunded": EscrowSummary;
}
export type JobCancelResponseAction = "job_cancel";
export type JobCancelResponseKind = "cancel";
export interface JobCancelResult {
    "error"?: string | null;
    "error_code"?: string | null;
    "job_id": string;
    "refunded"?: EscrowSummary | null;
    "success": boolean;
}
export interface JobReorderResponse {
    "action": JobReorderResponseAction;
    "facility_id": string;
    "job_id": string;
    "message": string;
    "position": number;
}
export type JobReorderResponseAction = "job_reorder";
export interface JobView {
    "base_id"?: string | null;
    "base_name"?: string | null;
    "eta_ticks": number;
    "external"?: boolean | null;
    "facility_id": string;
    "job_id": string;
    "label"?: string | null;
    "mode": string;
    "orderer": string;
    "package_id"?: string | null;
    "position": number;
    "produces"?: Array<ItemQuantity>;
    "progress": number;
    "recipe": string;
    "runs_done": number;
    "runs_remaining": number;
    "runs_total": number;
    "status": string;
    "venue"?: string | null;
}
export type LaneOwner = {
    "kind": "prayer_lang";
} | {
    "controller_kind": string;
    "kind": "controller";
} | {
    "kind": "manual";
};
export interface LootedModule {
    "id": string;
    "name": string;
    "type": string;
    "type_id": string;
    "wear": number;
}
export interface MarketMovement {
    "claims": Array<InventoryClaim>;
    "context": unknown;
    "createdAtUnix": number;
    "kind": string;
    "movementId": string;
    "sessionId": string;
    "status": MarketMovementStatus;
    "updatedAtUnix": number;
    "virtualOrderUses": Array<ReservationUse>;
}
export interface MarketMovementHealth {
    "active": boolean;
    "backedQuantity": number;
    "claims": Array<RuntimeInventoryClaimHealthDto>;
    "fullyBacked": boolean;
    "movementId": string;
    "requestedQuantity": number;
    "shortfallQuantity": number;
    "status": MarketMovementStatus;
}
export interface MarketMovementList {
    "movements": Array<MarketMovement>;
}
export interface MarketMovementReserveRequest {
    "claims"?: Array<InventoryClaim>;
    "context"?: unknown;
    "kind": string;
    "sessionId": string;
    "virtualOrderUses"?: Array<ReservationUse>;
}
export interface MarketMovementReserveResponse {
    "accepted": boolean;
    "movement"?: MarketMovement | null;
    "unavailableClaims"?: Array<InventoryClaim>;
    "unavailableVirtualOrderUses"?: Array<ReservationUse>;
}
export type MarketMovementStatus = "reserved" | "running" | "completed" | "failed" | "released" | "needs_reconciliation";
export interface MarketMovementTransitionRequest {
    "reason": string;
}
export interface Meta {
    "actionSchemaVersion": number;
    "apiVersion": string;
    "capabilities": Array<string>;
    "serverVersion": string;
}
export interface MissionData {
    "active": Array<string>;
    "active_details": Array<V2GameStateMissionsActiveItem>;
    "available": Array<string>;
    "available_details": Array<MissionInfo>;
}
export interface MissionDialogInfo {
    "accept"?: string | null;
    "complete"?: string | null;
    "decline"?: string | null;
    "offer"?: string | null;
}
export interface MissionGiverInfo {
    "name": string;
    "title": string;
}
export interface MissionInfo {
    "chain_next"?: string | null;
    "community"?: boolean | null;
    "community_percent"?: number | null;
    "community_progress"?: Record<string, string>;
    "description": string;
    "dialog"?: MissionDialogInfo | null;
    "difficulty": number;
    "expires_in_ticks": number;
    "faction_id"?: string | null;
    "faction_name"?: string | null;
    "giver"?: MissionGiverInfo | null;
    "issuing_base"?: string | null;
    "issuing_base_id"?: string | null;
    "issuing_system_id"?: string | null;
    "issuing_system_name"?: string | null;
    "mission_id": string;
    "objectives"?: Array<ObjectiveInfo>;
    "provided_items"?: Record<string, number>;
    "repeatable"?: boolean | null;
    "required_modules"?: Array<string>;
    "rewards": MissionRewardsInfo;
    "template_id"?: string | null;
    "title": string;
    "type": string;
    "warnings"?: Array<string>;
}
export interface MissionRewardsInfo {
    "credits": number;
    "items"?: Record<string, number>;
    "pirate_rep"?: number | null;
    "reputation"?: number | null;
    "skill_xp"?: Record<string, number>;
}
export interface Module {
    "accuracy_bonus"?: number | null;
    "ammo_type"?: string | null;
    "armor_bonus"?: number | null;
    "armor_bypass_bonus"?: number | null;
    "armor_repair_rate"?: number | null;
    "base_value": number;
    "cargo_bonus"?: number | null;
    "cloak_strength"?: number | null;
    "cooldown"?: number | null;
    "cpu_bonus"?: number | null;
    "cpu_usage": number;
    "current_cool"?: number | null;
    "damage"?: number | null;
    "damage_reduction"?: number | null;
    "damage_type"?: string | null;
    "description": string;
    "dining_points"?: number | null;
    "disruptor_power"?: number | null;
    "drone_bandwidth"?: number | null;
    "drone_capacity"?: number | null;
    "fuel_efficiency"?: number | null;
    "hidden"?: boolean | null;
    "hull_bonus"?: number | null;
    "hull_penalty"?: number | null;
    "id": string;
    "leisure_points"?: number | null;
    "magazine_size"?: number | null;
    "max_fuel_bonus"?: number | null;
    "mining_power"?: number | null;
    "name": string;
    "passenger_business_berths"?: number | null;
    "passenger_comfort"?: number | null;
    "passenger_economy_berths"?: number | null;
    "passenger_first_berths"?: number | null;
    "passive_recipe"?: string | null;
    "power_bonus"?: number | null;
    "power_usage": number;
    "precision_factor"?: number | null;
    "quest_item"?: boolean | null;
    "reach"?: number | null;
    "remote_repair_power"?: number | null;
    "required_skills"?: Record<string, number>;
    "resistance_bonus"?: Record<string, number>;
    "salvage_bonus"?: number | null;
    "scanner_power"?: number | null;
    "scramble_power"?: number | null;
    "shield_bonus"?: number | null;
    "shield_bypass_bonus"?: number | null;
    "shield_recharge_bonus"?: number | null;
    "signature_bonus"?: number | null;
    "size": number;
    "slot": string;
    "special"?: string | null;
    "speed_bonus"?: number | null;
    "speed_penalty"?: number | null;
    "survey_power"?: number | null;
    "tow_speed_penalty"?: number | null;
    "tracking_bonus"?: number | null;
    "type": string;
    "type_id": string;
    "warp_stabilization"?: number | null;
    "webify_strength"?: number | null;
}
export interface NearbyPlayer {
    "clan_tag"?: string | null;
    "docked"?: boolean | null;
    "faction_id"?: string | null;
    "faction_tag"?: string | null;
    "in_combat"?: boolean | null;
    "offline"?: boolean | null;
    "player_id"?: string | null;
    "primary_color"?: string | null;
    "secondary_color"?: string | null;
    "ship_class"?: string | null;
    "ship_name"?: string | null;
    "status_message"?: string | null;
    "username"?: string | null;
}
export interface ObjectiveInfo {
    "description": string;
    "eligible_players"?: Array<string>;
    "item_id"?: string | null;
    "participants"?: Array<string>;
    "quantity"?: number | null;
    "system_id"?: string | null;
    "system_name"?: string | null;
    "target_base_id"?: string | null;
    "target_base_name"?: string | null;
    "type": string;
}
export type ObservedPlayer = NearbyPlayer | NearbyPlayer | NearbyPlayer | NearbyPlayer;
export interface OrderLevel {
    "my_quantity"?: number | null;
    "price_each": number;
    "quantity": number;
    "source"?: string | null;
}
export interface OverrideResponse {
    "accepted": boolean;
}
export interface OwnedFacilityEntry {
    "arrears_owed"?: number | null;
    "base_id": string;
    "base_name": string;
    "custom_name"?: string | null;
    "damaged"?: boolean | null;
    "facility_id": string;
    "labor_per_run"?: number | null;
    "missed_rent_cycles"?: number | null;
    "name": string;
    "power_throttled"?: boolean | null;
    "rent_per_cycle": number;
    "rental_fee_per_run"?: number | null;
    "repair_complete_tick"?: number | null;
    "system_id"?: string | null;
    "type": string;
    "under_construction"?: boolean | null;
}
export interface OwnedShipDetail {
    "cargo_used"?: number | null;
    "class_id": string;
    "class_name"?: string | null;
    "custom_name"?: string | null;
    "fuel"?: string | null;
    "hull"?: string | null;
    "is_active": boolean;
    "listing_base_id"?: string | null;
    "listing_id"?: string | null;
    "listing_price"?: number | null;
    "location"?: string | null;
    "location_base_id"?: string | null;
    "modules"?: number | null;
    "ship_id": string;
}
export interface PackageJobResponse {
    "action": PackageJobResponseAction;
    "escrowed": EscrowSummary;
    "eta_ticks": number;
    "external"?: boolean | null;
    "job_id": string;
    "kind": PackageJobResponseKind;
    "label": string;
    "message": string;
    "package_id": string;
    "venue": string;
}
export type PackageJobResponseAction = "pack" | "unpack";
export type PackageJobResponseKind = "package";
export interface PassengerBerthView {
    "current": number;
    "max": number;
}
export interface PassengerState {
    "aboard"?: Array<PassengerView>;
    "aboard_count"?: number | null;
    "business_berths": PassengerBerthView;
    "business_berths_raw": string;
    "economy_berths": PassengerBerthView;
    "economy_berths_raw": string;
    "first_berths": PassengerBerthView;
    "first_berths_raw": string;
    "station": string;
    "waiting"?: Array<WaitingPassengerView>;
    "waiting_count"?: number | null;
}
export interface PassengerView {
    "base_fare": number;
    "berth_class"?: string | null;
    "bio": string;
    "citizen_id": string;
    "class": string;
    "connecting"?: boolean | null;
    "destination": string;
    "destination_name": string;
    "destination_system"?: string | null;
    "name": string;
    "speed_bonus"?: number | null;
    "ticks_remaining": number;
}
export interface PoiFacilitiesSnapshot {
    "current"?: FacilityResponse | null;
    "faction_current"?: FacilityResponse | null;
    "observed_at_unix"?: number;
}
export interface QueueLane {
    "active": boolean;
    "pendingActions": number;
    "prayerlang": string;
}
export interface QueueResponse {
    "prayerlang": string;
    "scheduler": QueueSnapshot;
    "scriptExecution"?: ScriptExecutionDto | null;
}
export interface QueueSnapshot {
    "generation": number;
    "haltReason"?: string | null;
    "halted": boolean;
    "interruptActive": boolean;
    "owner"?: LaneOwner | null;
    "pendingActions": number;
    "runningAction": boolean;
}
export interface Recipe {
    "category": string;
    "crafting_time": number;
    "description": string;
    "facility_only"?: boolean | null;
    "fuel_output"?: number | null;
    "hidden"?: boolean | null;
    "id": string;
    "inputs": Array<RecipeInput>;
    "name": string;
    "no_recycle"?: boolean | null;
    "outputs": Array<RecipeOutput>;
    "package_operation"?: string | null;
}
export interface RecipeInput {
    "item_id": string;
    "quantity": number;
}
export interface RecipeOutput {
    "item_id": string;
    "quantity": number;
}
export interface RecycleRequest {
    "destination"?: string | null;
    "facility_id"?: string | null;
    "quantity": number;
    "recipe_id": string;
    "source"?: string | null;
}
export interface RegisterBotRequest {
    "empire": string;
    "registrationCode": string;
    "username": string;
}
export interface RegisterBotResponse {
    "bot": V1BotSummary;
    "password": string;
    "playerId": string;
}
export interface ReservationRequest {
    "uses"?: Array<ReservationUse>;
}
export interface ReservationResponse {
    "orders": Array<VirtualMarketOrder>;
    "reservationResults": Array<RuntimeVirtualOrderReservationResultDto>;
}
export interface ReservationResult {
    "accepted": number;
    "orderId": string;
    "requested": number;
    "reservationId"?: string | null;
    "reservedAfter": number;
    "reservedBefore": number;
}
export interface ReservationUse {
    "orderId": string;
    "quantity": number;
}
export interface RouteBatchRequest {
    "routes": Array<RouteQuery>;
    "safe"?: boolean;
}
export interface RouteBatchResponse {
    "routes": Array<RouteSelection | null>;
}
export interface RouteQuery {
    "from": string;
    "to": string;
}
export interface RouteSelection {
    "cost": number;
    "from": string;
    "fromSystem": string;
    "hops": Array<string>;
    "safe": boolean;
    "to": string;
    "toSystem": string;
    "totalJumps": number;
}
export interface RuntimeGalaxyKnownPoiInfoDto {
    "baseId"?: string | null;
    "baseName"?: string | null;
    "firstDiscoveredUnix"?: number | null;
    "firstVisitedUnix"?: number | null;
    "hasBase": boolean;
    "id": string;
    "lastObservedUnix"?: number | null;
    "lastVisitedUnix"?: number | null;
    "name": string;
    "resources": Array<RuntimePoiResourceInfoDto>;
    "systemId": string;
    "type": string;
    "x"?: number | null;
    "y"?: number | null;
}
export interface RuntimeGalaxyPoiInfoDto {
    "id": string;
    "x"?: number | null;
    "y"?: number | null;
}
export interface RuntimeGalaxySystemInfoDto {
    "bloomIntensity"?: number | null;
    "bloomStatus"?: string | null;
    "connections": Array<string>;
    "empire": string;
    "faintSignatures": Array<unknown>;
    "firstEnteredUnix"?: number | null;
    "id": string;
    "isStronghold": boolean;
    "lastEnteredUnix"?: number | null;
    "lastScannedUnix"?: number | null;
    "lastSurveyedUnix"?: number | null;
    "name"?: string | null;
    "poiCount"?: number | null;
    "pois": Array<RuntimeGalaxyPoiInfoDto>;
    "poisComplete": boolean;
    "wildlife": Array<unknown>;
    "x"?: number | null;
    "y"?: number | null;
}
export interface RuntimeInventoryClaimHealthDto {
    "backedQuantity": number;
    "itemId": string;
    "locationId": string;
    "requestedQuantity": number;
    "shortfallQuantity": number;
    "sourceKind": string;
}
export interface RuntimePoiResourceInfoDto {
    "name": string;
    "remaining"?: number | null;
    "remainingDisplay": string;
    "resourceId": string;
    "richness"?: number | null;
    "richnessText": string;
}
export interface RuntimeVirtualOrderReservationResultDto {
    "accepted": number;
    "orderId": string;
    "requested": number;
    "reservationId"?: string | null;
    "reservedAfter": number;
    "reservedBefore": number;
}
export interface RuntimeWildlifeCreatureDto {
    "creatureId": string;
    "hull": number;
    "inCombat": boolean;
    "maxHull": number;
    "name": string;
    "observedAtUnix": number;
    "poiId": string;
    "role": string;
    "species": string;
    "systemId": string;
}
export interface RuntimeWildlifePoiDto {
    "creatureCount": number;
    "creatures": Array<RuntimeWildlifeCreatureDto>;
    "observedAtUnix": number;
    "poiId": string;
    "systemId": string;
}
export interface RuntimeWildlifeSpeciesDto {
    "count": number;
    "name": string;
    "role": string;
    "species": string;
}
export interface RuntimeWildlifeSystemDto {
    "creatureCount": number;
    "observedAtUnix": number;
    "pois": Array<string>;
    "species": Array<RuntimeWildlifeSpeciesDto>;
    "systemId": string;
}
export interface SalvageData {
    "last_seen_poi"?: string | null;
    "last_seen_system"?: string | null;
    "lootables_by_poi"?: Record<string, Array<SpaceLootInfo>>;
    "observed_at_unix"?: number | null;
    "visible_lootables": Array<SpaceLootInfo>;
}
export interface SayRequest {
    "channel": string;
    "content": string;
    "target"?: string | null;
}
export type ScriptErrorKind = "runtime" | "user_halt" | "cancelled" | "replaced" | "shutdown" | "runner_exited" | "internal";
export type ScriptErrorKindDto = "runtime" | "user_halt" | "cancelled" | "replaced" | "shutdown" | "runner_exited" | "internal";
export interface ScriptExecution {
    "currentLine"?: number | null;
    "frameKind"?: string | null;
    "frameName"?: string | null;
    "id": string;
    "lastLine"?: number | null;
    "outcome"?: ScriptExecutionOutcome | null;
    "runId"?: string | null;
    "script"?: string | null;
    "state": string;
}
export type ScriptExecutionDto = {
    "current_line"?: number | null;
    "last_line"?: number | null;
    "outcome"?: ScriptOutcomeDto | null;
    "state": "running";
} | {
    "current_line"?: number | null;
    "last_line"?: number | null;
    "outcome": ScriptOutcomeDto;
    "state": "stopped";
};
export type ScriptExecutionOutcome = {
    "message"?: string | null;
    "status": "success";
} | {
    "kind": string;
    "message": string;
    "status": "error";
};
export type ScriptOutcomeDto = {
    "message"?: string | null;
    "status": "success";
} | {
    "kind": ScriptErrorKindDto;
    "message": string;
    "status": "error";
};
export interface ScriptOverrideRequest {
    "returnToOrigin"?: boolean;
    "script": string;
}
export type ScriptRunOutcome = {
    "message"?: string | null;
    "status": "success";
} | {
    "kind": ScriptErrorKind;
    "message": string;
    "status": "error";
};
export interface ScriptRunRequest {
    "idempotencyKey"?: string | null;
    "script": string;
}
export type ScriptRunResponse = {
    "botId": string;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "running";
} | {
    "botId": string;
    "outcome": ScriptRunOutcome;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "succeeded";
} | {
    "botId": string;
    "outcome": ScriptRunOutcome;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "failed";
} | {
    "botId": string;
    "outcome": ScriptRunOutcome;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "cancelled";
} | {
    "botId": string;
    "outcome": ScriptRunOutcome;
    "prayerlang": string;
    "runId": string;
    "runVersion": number;
    "status": "halted";
};
export interface SellRequest {
    "item"?: string | null;
    "min_price"?: number | null;
    "place_order": boolean;
    "quantity"?: number | null;
}
export interface ServiceTransferRequest {
    "item"?: string | null;
    "quantity"?: number | null;
    "target"?: string | null;
}
export interface SetAccessResponse {
    "access": string;
    "action": SetAccessResponseAction;
    "facility_id": string;
    "message": string;
}
export type SetAccessResponseAction = "set_access";
export interface SetFacilityDescriptionResponse {
    "action": SetFacilityDescriptionResponseAction;
    "description"?: string | null;
    "facility_id": string;
    "message": string;
}
export type SetFacilityDescriptionResponseAction = "set_description";
export interface SetFacilityNameResponse {
    "action": SetFacilityNameResponseAction;
    "custom_name"?: string | null;
    "facility_id": string;
    "message": string;
}
export type SetFacilityNameResponseAction = "set_name";
export interface SetOutputPriceResponse {
    "action": SetOutputPriceResponseAction;
    "facility_id": string;
    "message": string;
    "price": number;
}
export type SetOutputPriceResponseAction = "set_output_price";
export interface ShipCargoItem {
    "item_id": string;
    "name"?: string | null;
    "quantity": number;
    "size"?: number | null;
}
export interface ShipClass {
    "base_armor"?: number | null;
    "base_fuel"?: number | null;
    "base_hull"?: number | null;
    "base_shield"?: number | null;
    "base_shield_recharge"?: number | null;
    "base_speed"?: number | null;
    "based_on"?: string | null;
    "build_materials"?: Record<string, number>;
    "build_time"?: number | null;
    "cargo_capacity"?: number | null;
    "category"?: string | null;
    "class": string;
    "cpu_capacity"?: number | null;
    "default_loadout_version"?: number | null;
    "default_modules"?: Array<string>;
    "defense_slots"?: number | null;
    "description"?: string | null;
    "faction"?: string | null;
    "flavor_tags"?: Array<string>;
    "hidden"?: boolean | null;
    "id": string;
    "inherent_capabilities"?: Array<ShipClassInherentCapabilitiesItem>;
    "legacy"?: boolean | null;
    "lore"?: string | null;
    "name": string;
    "passive_recipes"?: Array<string>;
    "piloting_required"?: number | null;
    "power_capacity"?: number | null;
    "prestige_lock"?: string | null;
    "required_achievement"?: string | null;
    "required_faction_achievement"?: string | null;
    "required_faction_leader"?: boolean | null;
    "required_items"?: Array<Record<string, unknown>>;
    "required_reputation"?: number | null;
    "scale"?: number | null;
    "shipyard_tier"?: number | null;
    "special"?: string | null;
    "starter_ship"?: boolean | null;
    "tier"?: number | null;
    "tow_speed_bonus"?: number | null;
    "utility_slots"?: number | null;
    "weapon_slots"?: number | null;
}
export interface ShipClassInherentCapabilitiesItem {
    "flag"?: string | null;
    "type"?: string | null;
    "value"?: number | null;
}
export interface SkillDefinition {
    "bonus_per_level"?: Record<string, number>;
    "category": string;
    "description": string;
    "empire_restriction"?: string | null;
    "id": string;
    "max_level": number;
    "name": string;
    "training_source"?: string | null;
    "xp_per_level": Array<number>;
}
export interface SpaceLootInfo {
    "cargo": Array<ShipCargoItem>;
    "created_at"?: string | null;
    "expire_tick"?: number | null;
    "expires_at"?: string | null;
    "id": string;
    "killer_name"?: string | null;
    "kind": string;
    "modules": Array<LootedModule>;
    "poi_id": string;
    "salvage_value"?: number | null;
    "ship_class"?: string | null;
    "ship_name"?: string | null;
    "system_id": string;
    "victim_name"?: string | null;
}
export interface StateResponse {
    "catalog"?: GalaxyCatalog | null;
    "fleet"?: FleetSnapshot | null;
    "versions": StateVersions;
    "world"?: WorldState | null;
}
export interface StateVersions {
    "catalog": string;
    "communications": number;
    "facilities": number;
    "factions": number;
    "fleet": number;
    "map": number;
    "markets": number;
    "observations": number;
    "resources": number;
    "storage": number;
    "wildlife": number;
    "world": number;
}
export interface StationConstructionEntry {
    "build_cost"?: number | null;
    "category": string;
    "definition_id": string;
    "materials"?: Array<StationConstructionMaterial>;
    "name": string;
    "reason"?: string | null;
    "status": string;
    "ticks_until_complete"?: number | null;
}
export interface StationConstructionMaterial {
    "item_id": string;
    "name"?: string | null;
    "quantity_in_storage": number;
    "quantity_missing"?: number | null;
    "quantity_required": number;
}
export interface StationConstructionResponse {
    "pending"?: Array<StationConstructionEntry>;
    "under_construction"?: Array<StationConstructionEntry>;
}
export interface StationLifeSupportInput {
    "item_id": string;
    "name"?: string | null;
    "quantity_per_cycle": number;
}
export interface StationLifeSupportStatus {
    "demand": number;
    "maintenance"?: Array<StationLifeSupportInput>;
    "maintenance_cycle_ticks": number;
    "plants": number;
    "remediation"?: string | null;
    "starved"?: Array<StationLifeSupportInput>;
    "supply": number;
}
export interface StationMarketData {
    "buy_orders": Record<string, Array<OrderLevel>>;
    "current_tick"?: number | null;
    "observed_at_unix"?: number | null;
    "sell_orders": Record<string, Array<OrderLevel>>;
}
export interface StationMarketDelta {
    "baseVersion": number;
    "remove": Array<string>;
    "upsert": Record<string, StationMarketData>;
}
export type StationMarkets = Record<string, StationMarketData>;
export interface StationPowerInput {
    "item_id": string;
    "name"?: string | null;
    "quantity_per_cycle": number;
}
export interface StationPowerStatus {
    "battery_capacity": number;
    "battery_stored": number;
    "current_draw": number;
    "efficiency": number;
    "fuel_inputs"?: Array<StationPowerInput>;
    "remediation"?: string | null;
    "supply": number;
}
export type StorageByOwner = Record<string, Record<string, Record<string, number>>>;
export interface TradeItem {
    "item": string;
    "quantity": number;
}
export interface TradeOfferRequest {
    "offer_credits"?: number | null;
    "offer_items": Array<TradeItem>;
    "request_credits"?: number | null;
    "request_items": Array<TradeItem>;
    "target": string;
}
export type TransferEndpoint = {
    "kind": "cargo";
} | {
    "kind": "storage";
} | {
    "id": string;
    "kind": "ship";
} | {
    "kind": "faction";
} | {
    "id": string;
    "kind": "faction_tag";
} | {
    "id": string;
    "kind": "player";
} | {
    "id": string | null;
    "kind": "space";
} | {
    "id": string;
    "kind": "commission";
};
export interface TransferItem {
    "id": string;
    "quantity": number;
}
export interface TransferRequest {
    "from": TransferEndpoint;
    "subject": TransferSubject;
    "to": TransferEndpoint;
}
export type TransferSubject = {
    "kind": "all_cargo";
} | {
    "kind": "credits";
    "quantity": number;
} | {
    "id": string;
    "kind": "item";
    "quantity"?: number | null;
} | {
    "id": string;
    "kind": "ship";
} | {
    "id": string;
    "kind": "module";
} | {
    "items": Array<TransferItem>;
    "kind": "items";
};
export type V1BotConnectionState = "connected" | "disconnected";
export interface V1BotSummary {
    "botId": string;
    "connection": V1BotConnectionState;
    "name"?: string | null;
    "observedAt"?: string | null;
    "stateVersion": number;
}
export interface V1ErrorDetail {
    "code": string;
    "details"?: unknown;
    "message": string;
    "retryable": boolean;
}
export interface V2GameStateCargoItem {
    "item_id"?: string | null;
    "item_name"?: string | null;
    "quantity"?: number | null;
    "size"?: number | null;
}
export interface V2GameStateLocation {
    "connections"?: Array<string>;
    "docked_at"?: string | null;
    "empire"?: string | null;
    "in_transit"?: boolean | null;
    "nearby_empire_npc_count"?: number | null;
    "nearby_empire_npcs"?: Array<V2GameStateLocationNearbyEmpireNpcsItem>;
    "nearby_pirate_count"?: number | null;
    "nearby_pirates"?: Array<V2GameStateLocationNearbyPiratesItem>;
    "nearby_player_count"?: number | null;
    "nearby_players"?: Array<V2GameStateLocationNearbyPlayersItem>;
    "offline_collapsed"?: number | null;
    "poi_id"?: string | null;
    "poi_name"?: string | null;
    "poi_type"?: string | null;
    "resources"?: Array<V2GameStateLocationResourcesItem>;
    "security_status"?: string | null;
    "system_id"?: string | null;
    "system_name"?: string | null;
    "transit_arrival_tick"?: number | null;
    "transit_bearing"?: number | null;
    "transit_dest_poi_id"?: string | null;
    "transit_dest_poi_name"?: string | null;
    "transit_dest_system_id"?: string | null;
    "transit_dest_system_name"?: string | null;
    "transit_ticks_elapsed"?: number | null;
    "transit_type"?: string | null;
    "transit_x"?: number | null;
    "transit_y"?: number | null;
    "unknown_signature"?: boolean | null;
    "void_message"?: string | null;
}
export interface V2GameStateLocationNearbyEmpireNpcsItem {
    "empire"?: string | null;
    "fleet_name"?: string | null;
    "in_combat"?: boolean | null;
    "name"?: string | null;
    "npc_id"?: string | null;
    "role"?: string | null;
    "ship_class"?: string | null;
    "ship_name"?: string | null;
}
export interface V2GameStateLocationNearbyPiratesItem {
    "hull"?: number | null;
    "is_boss"?: boolean | null;
    "max_hull"?: number | null;
    "max_shield"?: number | null;
    "name"?: string | null;
    "pirate_id"?: string | null;
    "shield"?: number | null;
    "status"?: string | null;
    "tier"?: string | null;
}
export interface V2GameStateLocationNearbyPlayersItem {
    "clan_tag"?: string | null;
    "faction_tag"?: string | null;
    "in_combat"?: boolean | null;
    "offline"?: boolean | null;
    "player_id"?: string | null;
    "ship_class"?: string | null;
    "ship_name"?: string | null;
    "username"?: string | null;
}
export interface V2GameStateLocationResourcesItem {
    "item_id"?: string | null;
    "item_name"?: string | null;
    "remaining"?: number | null;
    "richness"?: number | null;
    "supported_power"?: number | null;
}
export interface V2GameStateMissionsActiveItem {
    "accepted_at"?: string | null;
    "community"?: boolean | null;
    "community_percent"?: number | null;
    "community_progress"?: Record<string, string>;
    "description"?: string | null;
    "difficulty"?: number | null;
    "expires_in_ticks"?: number | null;
    "giver"?: V2GameStateMissionsActiveItemGiver | null;
    "issuing_base"?: string | null;
    "issuing_base_id"?: string | null;
    "issuing_system_id"?: string | null;
    "issuing_system_name"?: string | null;
    "mission_id"?: string | null;
    "objectives"?: Array<V2GameStateMissionsActiveItemObjectivesItem>;
    "percent_complete"?: number | null;
    "rewards"?: V2GameStateMissionsActiveItemRewards | null;
    "template_id"?: string | null;
    "title"?: string | null;
    "type"?: string | null;
}
export interface V2GameStateMissionsActiveItemGiver {
    "name"?: string | null;
    "title"?: string | null;
}
export interface V2GameStateMissionsActiveItemObjectivesItem {
    "completed"?: boolean | null;
    "current"?: number | null;
    "description"?: string | null;
    "eligible_players"?: Array<string>;
    "in_cargo"?: number | null;
    "in_storage"?: number | null;
    "item_id"?: string | null;
    "item_name"?: string | null;
    "participants"?: Array<string>;
    "required"?: number | null;
    "system_id"?: string | null;
    "system_name"?: string | null;
    "target_base"?: string | null;
    "target_base_name"?: string | null;
    "type"?: string | null;
}
export interface V2GameStateMissionsActiveItemRewards {
    "credits"?: number | null;
    "items"?: Record<string, number>;
    "pirate_rep"?: number | null;
    "reputation"?: number | null;
    "skill_xp"?: Record<string, number>;
}
export interface V2GameStateModulesItem {
    "ammo_type"?: string | null;
    "cpu_usage"?: number | null;
    "current_ammo"?: number | null;
    "loaded_ammo_id"?: string | null;
    "loaded_ammo_name"?: string | null;
    "magazine_size"?: number | null;
    "module_id"?: string | null;
    "name"?: string | null;
    "power_usage"?: number | null;
    "size"?: number | null;
    "slot"?: string | null;
    "stats"?: Record<string, unknown>;
    "type"?: string | null;
    "type_id"?: string | null;
    "wear"?: number | null;
    "wear_status"?: string | null;
}
export interface V2GameStatePlayer {
    "citizenships"?: Array<string>;
    "clan_tag"?: string | null;
    "credits"?: number | null;
    "empire"?: string | null;
    "faction_id"?: string | null;
    "faction_rank"?: string | null;
    "home_base"?: string | null;
    "home_poi"?: string | null;
    "home_system"?: string | null;
    "id"?: string | null;
    "is_cloaked"?: boolean | null;
    "primary_color"?: string | null;
    "secondary_color"?: string | null;
    "standings"?: Record<string, V2GameStatePlayerStandingsValue>;
    "stats"?: Record<string, unknown>;
    "status_message"?: string | null;
    "towing_wreck_id"?: string | null;
    "trading_restricted_until"?: string | null;
    "username"?: string | null;
}
export interface V2GameStatePlayerStandingsValue {
    "baseline"?: number | null;
    "jailed_until"?: string | null;
    "outstanding_bounty"?: number | null;
    "reputation"?: number | null;
}
export interface V2GameStateShip {
    "active_buffs"?: Array<V2GameStateShipActiveBuffsItem>;
    "armor"?: number | null;
    "armor_melt_pct"?: number | null;
    "armor_melt_ticks_remaining"?: number | null;
    "berths"?: V2GameStateShipBerths | null;
    "burn_damage_per_tick"?: number | null;
    "burn_source_id"?: string | null;
    "burn_ticks_remaining"?: number | null;
    "cargo_capacity"?: number | null;
    "cargo_used"?: number | null;
    "class_id"?: string | null;
    "class_name"?: string | null;
    "cpu_capacity"?: number | null;
    "cpu_used"?: number | null;
    "custom_name"?: string | null;
    "damage_penalty"?: number | null;
    "defense_slots"?: number | null;
    "disruption_ticks_remaining"?: number | null;
    "fuel"?: number | null;
    "gas_cargo_efficiency"?: number | null;
    "hull"?: number | null;
    "ice_cargo_efficiency"?: number | null;
    "id"?: string | null;
    "max_fuel"?: number | null;
    "max_hull"?: number | null;
    "max_shield"?: number | null;
    "name"?: string | null;
    "ore_cargo_efficiency"?: number | null;
    "power_capacity"?: number | null;
    "power_used"?: number | null;
    "shield"?: number | null;
    "shield_recharge"?: number | null;
    "speed"?: number | null;
    "speed_penalty"?: number | null;
    "utility_slots"?: number | null;
    "weapon_slots"?: number | null;
}
export interface V2GameStateShipActiveBuffsItem {
    "amount"?: number | null;
    "expires_at"?: number | null;
    "item_id"?: string | null;
    "stat"?: string | null;
}
export interface V2GameStateShipBerths {
    "business": V2GameStateShipBerthsBusiness;
    "economy": V2GameStateShipBerthsEconomy;
    "first": V2GameStateShipBerthsFirst;
}
export interface V2GameStateShipBerthsBusiness {
    "free": number;
    "total": number;
}
export interface V2GameStateShipBerthsEconomy {
    "free": number;
    "total": number;
}
export interface V2GameStateShipBerthsFirst {
    "free": number;
    "total": number;
}
export interface V2GameStateSkillsValue {
    "category"?: string | null;
    "level"?: number | null;
    "max_level"?: number | null;
    "name"?: string | null;
    "next_level_xp"?: number | null;
    "xp"?: number | null;
}
export interface VirtualCraftOrder {
    "action": string;
    "creditFloor"?: number | null;
    "doForever"?: boolean;
    "enabled"?: boolean;
    "facilityId"?: string | null;
    "filled"?: number;
    "id": string;
    "itemId"?: string;
    "preset"?: string | null;
    "priority"?: number;
    "quantity": number;
    "recipeId": string;
    "reservationId"?: string | null;
    "reserved"?: number;
    "sessionHandles"?: Array<string>;
    "squadId"?: string | null;
    "stationId"?: string;
    "status"?: string;
}
export interface VirtualCraftOrderList {
    "orders": Array<VirtualCraftOrder>;
}
export interface VirtualCraftOrderWrite {
    "orders"?: Array<VirtualCraftOrder>;
}
export interface VirtualMarketOrder {
    "doForever"?: boolean;
    "dumping"?: boolean;
    "enabled"?: boolean;
    "filled"?: number;
    "id": string;
    "internalOnly"?: boolean;
    "itemId": string;
    "priceEach": number;
    "priority"?: number;
    "quantity": number;
    "reservationId"?: string | null;
    "reserved"?: number;
    "side": string;
    "stationId": string;
    "status"?: string;
    "tippingPoint"?: number | null;
}
export interface VirtualOrderList {
    "orders": Array<VirtualMarketOrder>;
}
export interface VirtualOrderWrite {
    "orders"?: Array<VirtualMarketOrder>;
}
export interface WaitingPassengerView {
    "bio": string;
    "citizen_id": string;
    "citizenship": string;
    "class": string;
    "destination": string;
    "destination_name": string;
    "destination_system"?: string | null;
    "estimated_fare"?: number | null;
    "name": string;
}
export interface WorldState {
    "agentSightings": Record<string, AgentSightingData> | Record<string, AgentSightingData>;
    "chatMessagesBySession": Record<string, Array<ChatMessageData>> | Record<string, Array<ChatMessageData>>;
    "facilitiesByPoi": Record<string, PoiFacilitiesSnapshot> | Record<string, PoiFacilitiesSnapshot>;
    "factionBySession": Record<string, FactionSnapshotData> | Record<string, FactionSnapshotData>;
    "factionStorageByFactionPoi": Record<string, Record<string, Record<string, number>>> | Record<string, Record<string, Record<string, number>>>;
    "map": GalaxyMap | null;
    "ownedFacilitiesByFaction": Record<string, FacilityResponse> | Record<string, FacilityResponse>;
    "ownedFacilitiesByPlayer": Record<string, FacilityResponse> | Record<string, FacilityResponse>;
    "resources": GalaxyResources | null;
    "salvageByPoi": Record<string, SalvageData> | Record<string, SalvageData>;
    "stationMarketDelta": StationMarketDelta | null;
    "stationMarkets": StationMarkets;
    "stationPassengers": Record<string, PassengerState> | Record<string, PassengerState>;
    "storageByPlayer": Record<string, Record<string, Record<string, number>>> | Record<string, Record<string, Record<string, number>>>;
    "updatedAtUtc": string;
    "wildlife": GalaxyWildlife | null;
}
