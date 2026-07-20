# AUTO-GENERATED from prayer-api/openapi/prayer-v1.json. DO NOT EDIT.
from __future__ import annotations

from collections.abc import Iterator, Mapping
from datetime import datetime
from types import MappingProxyType
from typing import Any, Generic, Literal, TypeVar
from uuid import UUID

from pydantic import BaseModel, ConfigDict, Field, RootModel

V = TypeVar('V')
class FrozenDict(Mapping[str, V], Generic[V]):
    def __init__(self, value: Mapping[str, V] | None = None): self._data = MappingProxyType(dict(value or {}))
    def __getitem__(self, key: str) -> V: return self._data[key]
    def __iter__(self) -> Iterator[str]: return iter(self._data)
    def __len__(self) -> int: return len(self._data)
    def __repr__(self) -> str: return repr(dict(self._data))
    @classmethod
    def __get_pydantic_core_schema__(cls, source: Any, handler: Any) -> Any:
        from pydantic_core import core_schema
        args = getattr(source, '__args__', (str, Any))
        values = handler.generate_schema(args[1])
        return core_schema.no_info_after_validator_function(cls, core_schema.dict_schema(core_schema.str_schema(), values))

class WireModel(BaseModel):
    model_config = ConfigDict(populate_by_name=True, frozen=True, extra='forbid')

class ActionVariant1(WireModel):
    type: Literal['halt'] = Field(...)

class ActionVariant2Request(WireModel):
    ticks: int = Field(...)

class ActionVariant2(WireModel):
    request: ActionVariant2Request = Field(...)
    type: Literal['wait'] = Field(...)

class ActionVariant3Request(WireModel):
    destination: GoTarget = Field(...)

class ActionVariant3(WireModel):
    request: ActionVariant3Request = Field(...)
    type: Literal['go'] = Field(...)

class ActionVariant4(WireModel):
    type: Literal['dock'] = Field(...)

class ActionVariant5(WireModel):
    type: Literal['undock'] = Field(...)

class ActionVariant6Request(WireModel):
    resource: str | None = Field(None)

class ActionVariant6(WireModel):
    request: ActionVariant6Request = Field(...)
    type: Literal['mine'] = Field(...)

class ActionVariant7(WireModel):
    request: TransferRequest = Field(...)
    type: Literal['transfer'] = Field(...)

class ActionVariant8(WireModel):
    type: Literal['set_home'] = Field(...)

class ActionVariant9(WireModel):
    request: FindRequest = Field(...)
    type: Literal['find'] = Field(...)

class ActionVariant10(WireModel):
    type: Literal['survey'] = Field(...)

class ActionVariant11Request(WireModel):
    target_id: str = Field(...)

class ActionVariant11(WireModel):
    request: ActionVariant11Request = Field(...)
    type: Literal['attack'] = Field(...)

class ActionVariant12Request(WireModel):
    target: str | None = Field(None)

class ActionVariant12(WireModel):
    request: ActionVariant12Request = Field(...)
    type: Literal['scan'] = Field(...)

class ActionVariant13Request(WireModel):
    enabled: bool = Field(...)

class ActionVariant13(WireModel):
    request: ActionVariant13Request = Field(...)
    type: Literal['cloak'] = Field(...)

class ActionVariant14Request(WireModel):
    target: str = Field(...)

class ActionVariant14(WireModel):
    request: ActionVariant14Request = Field(...)
    type: Literal['hunt'] = Field(...)

class ActionVariant15Request(WireModel):
    quantity: int = Field(...)

class ActionVariant15(WireModel):
    request: ActionVariant15Request = Field(...)
    type: Literal['prepay_tax'] = Field(...)

class ActionVariant16Request(WireModel):
    mission_id: str = Field(...)

class ActionVariant16(WireModel):
    request: ActionVariant16Request = Field(...)
    type: Literal['accept_mission'] = Field(...)

class ActionVariant17Request(WireModel):
    mission_id: str = Field(...)

class ActionVariant17(WireModel):
    request: ActionVariant17Request = Field(...)
    type: Literal['abandon_mission'] = Field(...)

class ActionVariant18Request(WireModel):
    template_id: str = Field(...)

class ActionVariant18(WireModel):
    request: ActionVariant18Request = Field(...)
    type: Literal['decline_mission'] = Field(...)

class ActionVariant19Request(WireModel):
    mission_id: str = Field(...)

class ActionVariant19(WireModel):
    request: ActionVariant19Request = Field(...)
    type: Literal['complete_mission'] = Field(...)

class ActionVariant20Request(WireModel):
    destination: str = Field(...)

class ActionVariant20(WireModel):
    request: ActionVariant20Request = Field(...)
    type: Literal['load_passenger'] = Field(...)

class ActionVariant21Request(WireModel):
    name: str = Field(...)
    target: str | None = Field(None)

class ActionVariant21(WireModel):
    request: ActionVariant21Request = Field(...)
    type: Literal['unload_passenger'] = Field(...)

class ActionVariant22(WireModel):
    request: BuyRequest = Field(...)
    type: Literal['buy'] = Field(...)

class ActionVariant23(WireModel):
    request: SellRequest = Field(...)
    type: Literal['sell'] = Field(...)

class ActionVariant24Request(WireModel):
    item: str = Field(...)

class ActionVariant24(WireModel):
    request: ActionVariant24Request = Field(...)
    type: Literal['cancel_buy'] = Field(...)

class ActionVariant25Request(WireModel):
    item: str = Field(...)

class ActionVariant25(WireModel):
    request: ActionVariant25Request = Field(...)
    type: Literal['cancel_sell'] = Field(...)

class ActionVariant26Request(WireModel):
    name: str = Field(...)
    tag: str = Field(...)

class ActionVariant26(WireModel):
    request: ActionVariant26Request = Field(...)
    type: Literal['faction_create'] = Field(...)

class ActionVariant27Request(WireModel):
    player: str = Field(...)

class ActionVariant27(WireModel):
    request: ActionVariant27Request = Field(...)
    type: Literal['faction_invite'] = Field(...)

class ActionVariant28Request(WireModel):
    faction: str = Field(...)

class ActionVariant28(WireModel):
    request: ActionVariant28Request = Field(...)
    type: Literal['faction_accept_invite'] = Field(...)

class ActionVariant29Request(WireModel):
    player: str = Field(...)

class ActionVariant29(WireModel):
    request: ActionVariant29Request = Field(...)
    type: Literal['faction_kick'] = Field(...)

class ActionVariant30Request(WireModel):
    player: str = Field(...)
    role: str = Field(...)

class ActionVariant30(WireModel):
    request: ActionVariant30Request = Field(...)
    type: Literal['faction_set_role'] = Field(...)

class ActionVariant31Request(WireModel):
    name: str = Field(...)
    public_access: bool = Field(...)

class ActionVariant31(WireModel):
    request: ActionVariant31Request = Field(...)
    type: Literal['found_station'] = Field(...)

class ActionVariant32Request(WireModel):
    facility_type: str = Field(...)

class ActionVariant32(WireModel):
    request: ActionVariant32Request = Field(...)
    type: Literal['facility_build'] = Field(...)

class ActionVariant33Request(WireModel):
    facility_type: str = Field(...)

class ActionVariant33(WireModel):
    request: ActionVariant33Request = Field(...)
    type: Literal['faction_facility_build'] = Field(...)

class ActionVariant34(WireModel):
    request: FacilityUpgradeRequest = Field(...)
    type: Literal['facility_upgrade'] = Field(...)

class ActionVariant35(WireModel):
    request: FacilityUpgradeRequest = Field(...)
    type: Literal['faction_facility_upgrade'] = Field(...)

class ActionVariant36Request(WireModel):
    facility_id: str = Field(...)

class ActionVariant36(WireModel):
    request: ActionVariant36Request = Field(...)
    type: Literal['facility_dismantle'] = Field(...)

class ActionVariant37Request(WireModel):
    facility_id: str = Field(...)

class ActionVariant37(WireModel):
    request: ActionVariant37Request = Field(...)
    type: Literal['faction_facility_dismantle'] = Field(...)

class ActionVariant38(WireModel):
    request: FacilityAccessRequest = Field(...)
    type: Literal['facility_set_access'] = Field(...)

class ActionVariant39(WireModel):
    request: FacilityOutputPriceRequest = Field(...)
    type: Literal['facility_set_output_price'] = Field(...)

class ActionVariant40(WireModel):
    request: FacilityNameRequest = Field(...)
    type: Literal['facility_set_name'] = Field(...)

class ActionVariant41Request(WireModel):
    item: str = Field(...)
    quantity: int = Field(...)

class ActionVariant41(WireModel):
    request: ActionVariant41Request = Field(...)
    type: Literal['use_item'] = Field(...)

class ActionVariant42(WireModel):
    request: ServiceTransferRequest = Field(...)
    type: Literal['repair'] = Field(...)

class ActionVariant43Request(WireModel):
    module: str = Field(...)

class ActionVariant43(WireModel):
    request: ActionVariant43Request = Field(...)
    type: Literal['repair_module'] = Field(...)

class ActionVariant44(WireModel):
    request: RecycleRequest = Field(...)
    type: Literal['recycle'] = Field(...)

class ActionVariant45(WireModel):
    request: ServiceTransferRequest = Field(...)
    type: Literal['refuel'] = Field(...)

class ActionVariant46(WireModel):
    type: Literal['self_destruct'] = Field(...)

class ActionVariant47Request(WireModel):
    ship: str = Field(...)

class ActionVariant47(WireModel):
    request: ActionVariant47Request = Field(...)
    type: Literal['switch_ship'] = Field(...)

class ActionVariant48Request(WireModel):
    name: str = Field(...)

class ActionVariant48(WireModel):
    request: ActionVariant48Request = Field(...)
    type: Literal['rename_ship'] = Field(...)

class ActionVariant49Request(WireModel):
    module: str = Field(...)

class ActionVariant49(WireModel):
    request: ActionVariant49Request = Field(...)
    type: Literal['install_mod'] = Field(...)

class ActionVariant50Request(WireModel):
    module: str = Field(...)

class ActionVariant50(WireModel):
    request: ActionVariant50Request = Field(...)
    type: Literal['uninstall_mod'] = Field(...)

class ActionVariant51Request(WireModel):
    listing: str = Field(...)

class ActionVariant51(WireModel):
    request: ActionVariant51Request = Field(...)
    type: Literal['buy_ship'] = Field(...)

class ActionVariant52Request(WireModel):
    listing: str = Field(...)

class ActionVariant52(WireModel):
    request: ActionVariant52Request = Field(...)
    type: Literal['buy_listed_ship'] = Field(...)

class ActionVariant53(WireModel):
    request: CommissionShipRequest = Field(...)
    type: Literal['commission_ship'] = Field(...)

class ActionVariant54Request(WireModel):
    ship: str = Field(...)

class ActionVariant54(WireModel):
    request: ActionVariant54Request = Field(...)
    type: Literal['sell_ship'] = Field(...)

class ActionVariant55Request(WireModel):
    ship: str = Field(...)

class ActionVariant55(WireModel):
    request: ActionVariant55Request = Field(...)
    type: Literal['scrap_ship'] = Field(...)

class ActionVariant56Request(WireModel):
    price: int = Field(...)
    ship: str = Field(...)

class ActionVariant56(WireModel):
    request: ActionVariant56Request = Field(...)
    type: Literal['list_ship_for_sale'] = Field(...)

class ActionVariant57(WireModel):
    type: Literal['refit_ship'] = Field(...)

class ActionVariant58Request(WireModel):
    commission_id: str = Field(...)

class ActionVariant58(WireModel):
    request: ActionVariant58Request = Field(...)
    type: Literal['cancel_commission'] = Field(...)

class ActionVariant59Request(WireModel):
    commission_id: str = Field(...)
    item: str = Field(...)
    quantity: int = Field(...)

class ActionVariant59(WireModel):
    request: ActionVariant59Request = Field(...)
    type: Literal['supply_commission'] = Field(...)

class ActionVariant60Request(WireModel):
    listing_id: str = Field(...)

class ActionVariant60(WireModel):
    request: ActionVariant60Request = Field(...)
    type: Literal['cancel_ship_listing'] = Field(...)

class ActionVariant61Request(WireModel):
    price: int = Field(...)
    ship_class: str = Field(...)

class ActionVariant61(WireModel):
    request: ActionVariant61Request = Field(...)
    type: Literal['place_ship_buy_order'] = Field(...)

class ActionVariant62Request(WireModel):
    order_id: str = Field(...)

class ActionVariant62(WireModel):
    request: ActionVariant62Request = Field(...)
    type: Literal['cancel_ship_buy_order'] = Field(...)

class ActionVariant63Request(WireModel):
    order_id: str = Field(...)
    ship_id: str = Field(...)

class ActionVariant63(WireModel):
    request: ActionVariant63Request = Field(...)
    type: Literal['sell_ship_to_order'] = Field(...)

class ActionVariant64Request(WireModel):
    order_id: str = Field(...)

class ActionVariant64(WireModel):
    request: ActionVariant64Request = Field(...)
    type: Literal['cancel_order'] = Field(...)

class ActionVariant65Request(WireModel):
    order_id: str = Field(...)
    price_each: int = Field(...)

class ActionVariant65(WireModel):
    request: ActionVariant65Request = Field(...)
    type: Literal['modify_order'] = Field(...)

class ActionVariant66(WireModel):
    request: CraftRequest = Field(...)
    type: Literal['craft'] = Field(...)

class ActionVariant67Request(WireModel):
    job_id: str = Field(...)

class ActionVariant67(WireModel):
    request: ActionVariant67Request = Field(...)
    type: Literal['cancel_craft_job'] = Field(...)

class ActionVariant68Request(WireModel):
    wreck_id: str = Field(...)

class ActionVariant68(WireModel):
    request: ActionVariant68Request = Field(...)
    type: Literal['salvage_wreck'] = Field(...)

class ActionVariant69Request(WireModel):
    wreck_id: str = Field(...)

class ActionVariant69(WireModel):
    request: ActionVariant69Request = Field(...)
    type: Literal['tow_wreck'] = Field(...)

class ActionVariant70(WireModel):
    type: Literal['scrap_wreck'] = Field(...)

class ActionVariant71(WireModel):
    type: Literal['sell_wreck'] = Field(...)

class ActionVariant72(WireModel):
    type: Literal['release_wreck'] = Field(...)

class ActionVariant73Request(WireModel):
    ticks: int = Field(...)

class ActionVariant73(WireModel):
    request: ActionVariant73Request = Field(...)
    type: Literal['insure_ship'] = Field(...)

class ActionVariant74Request(WireModel):
    empire_id: str = Field(...)

class ActionVariant74(WireModel):
    request: ActionVariant74Request = Field(...)
    type: Literal['citizenship_apply'] = Field(...)

class ActionVariant75Request(WireModel):
    empire_id: str = Field(...)

class ActionVariant75(WireModel):
    request: ActionVariant75Request = Field(...)
    type: Literal['citizenship_withdraw'] = Field(...)

class ActionVariant76Request(WireModel):
    empire_id: str = Field(...)

class ActionVariant76(WireModel):
    request: ActionVariant76Request = Field(...)
    type: Literal['citizenship_renounce'] = Field(...)

class ActionVariant77(WireModel):
    request: TradeOfferRequest = Field(...)
    type: Literal['trade_offer'] = Field(...)

class ActionVariant78Request(WireModel):
    trade_id: str = Field(...)

class ActionVariant78(WireModel):
    request: ActionVariant78Request = Field(...)
    type: Literal['trade_accept'] = Field(...)

class ActionVariant79(WireModel):
    type: Literal['faction_leave'] = Field(...)

class ActionVariant80Request(WireModel):
    player: str = Field(...)

class ActionVariant80(WireModel):
    request: ActionVariant80Request = Field(...)
    type: Literal['faction_withdraw_invite'] = Field(...)

class ActionVariant81Request(WireModel):
    faction: str = Field(...)

class ActionVariant81(WireModel):
    request: ActionVariant81Request = Field(...)
    type: Literal['faction_propose_ally'] = Field(...)

class ActionVariant82Request(WireModel):
    faction: str = Field(...)

class ActionVariant82(WireModel):
    request: ActionVariant82Request = Field(...)
    type: Literal['faction_accept_ally'] = Field(...)

class ActionVariant83Request(WireModel):
    faction: str = Field(...)

class ActionVariant83(WireModel):
    request: ActionVariant83Request = Field(...)
    type: Literal['faction_remove_ally'] = Field(...)

class ActionVariant84Request(WireModel):
    faction: str = Field(...)
    reason: str | None = Field(None)

class ActionVariant84(WireModel):
    request: ActionVariant84Request = Field(...)
    type: Literal['faction_declare_war'] = Field(...)

class ActionVariant85Request(WireModel):
    faction: str = Field(...)
    message: str | None = Field(None)

class ActionVariant85(WireModel):
    request: ActionVariant85Request = Field(...)
    type: Literal['faction_propose_peace'] = Field(...)

class ActionVariant86Request(WireModel):
    faction: str = Field(...)

class ActionVariant86(WireModel):
    request: ActionVariant86Request = Field(...)
    type: Literal['faction_accept_peace'] = Field(...)

class ActionVariant87Request(WireModel):
    faction: str = Field(...)

class ActionVariant87(WireModel):
    request: ActionVariant87Request = Field(...)
    type: Literal['faction_set_enemy'] = Field(...)

class ActionVariant88Request(WireModel):
    faction: str = Field(...)

class ActionVariant88(WireModel):
    request: ActionVariant88Request = Field(...)
    type: Literal['faction_remove_enemy'] = Field(...)

class ActionVariant89Request(WireModel):
    quantity: int = Field(...)

class ActionVariant89(WireModel):
    request: ActionVariant89Request = Field(...)
    type: Literal['faction_prepay_tax'] = Field(...)

class ActionVariant90Request(WireModel):
    mission_id: str = Field(...)

class ActionVariant90(WireModel):
    request: ActionVariant90Request = Field(...)
    type: Literal['faction_cancel_mission'] = Field(...)

class ActionVariant91(WireModel):
    type: Literal['espionage'] = Field(...)

class ActionVariant92Request(WireModel):
    poi_id: str = Field(...)

class ActionVariant92(WireModel):
    request: ActionVariant92Request = Field(...)
    type: Literal['scan_poi'] = Field(...)

class ActionVariant93Request(WireModel):
    distress_type: str | None = Field(None)

class ActionVariant93(WireModel):
    request: ActionVariant93Request = Field(...)
    type: Literal['distress_signal'] = Field(...)

class ActionVariant94(WireModel):
    request: SayRequest = Field(...)
    type: Literal['say'] = Field(...)

class Action(RootModel['ActionVariant1 | ActionVariant2 | ActionVariant3 | ActionVariant4 | ActionVariant5 | ActionVariant6 | ActionVariant7 | ActionVariant8 | ActionVariant9 | ActionVariant10 | ActionVariant11 | ActionVariant12 | ActionVariant13 | ActionVariant14 | ActionVariant15 | ActionVariant16 | ActionVariant17 | ActionVariant18 | ActionVariant19 | ActionVariant20 | ActionVariant21 | ActionVariant22 | ActionVariant23 | ActionVariant24 | ActionVariant25 | ActionVariant26 | ActionVariant27 | ActionVariant28 | ActionVariant29 | ActionVariant30 | ActionVariant31 | ActionVariant32 | ActionVariant33 | ActionVariant34 | ActionVariant35 | ActionVariant36 | ActionVariant37 | ActionVariant38 | ActionVariant39 | ActionVariant40 | ActionVariant41 | ActionVariant42 | ActionVariant43 | ActionVariant44 | ActionVariant45 | ActionVariant46 | ActionVariant47 | ActionVariant48 | ActionVariant49 | ActionVariant50 | ActionVariant51 | ActionVariant52 | ActionVariant53 | ActionVariant54 | ActionVariant55 | ActionVariant56 | ActionVariant57 | ActionVariant58 | ActionVariant59 | ActionVariant60 | ActionVariant61 | ActionVariant62 | ActionVariant63 | ActionVariant64 | ActionVariant65 | ActionVariant66 | ActionVariant67 | ActionVariant68 | ActionVariant69 | ActionVariant70 | ActionVariant71 | ActionVariant72 | ActionVariant73 | ActionVariant74 | ActionVariant75 | ActionVariant76 | ActionVariant77 | ActionVariant78 | ActionVariant79 | ActionVariant80 | ActionVariant81 | ActionVariant82 | ActionVariant83 | ActionVariant84 | ActionVariant85 | ActionVariant86 | ActionVariant87 | ActionVariant88 | ActionVariant89 | ActionVariant90 | ActionVariant91 | ActionVariant92 | ActionVariant93 | ActionVariant94']):
    model_config = ConfigDict(frozen=True)

class ActionOverrideRequest(WireModel):
    actions: tuple[Action, ...] = Field(...)
    return_to_origin: bool | None = Field(None, alias='returnToOrigin')

class ActionRunOutcomeVariant1(WireModel):
    status: Literal['succeeded'] = Field(...)

class ActionRunOutcomeVariant2(WireModel):
    action_index: int = Field(...)
    message: str = Field(...)
    status: Literal['failed'] = Field(...)

class ActionRunOutcomeVariant3(WireModel):
    reason: str = Field(...)
    status: Literal['cancelled'] = Field(...)

class ActionRunOutcomeVariant4(WireModel):
    reason: str = Field(...)
    status: Literal['halted'] = Field(...)

class ActionRunOutcome(RootModel['ActionRunOutcomeVariant1 | ActionRunOutcomeVariant2 | ActionRunOutcomeVariant3 | ActionRunOutcomeVariant4']):
    model_config = ConfigDict(frozen=True)

class ActionRunRequest(WireModel):
    actions: tuple[Action, ...] = Field(...)
    idempotency_key: str | None = Field(None, alias='idempotencyKey')

class ActionRunResponseVariant1(WireModel):
    bot_id: str = Field(..., alias='botId')
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['running'] = Field(...)

class ActionRunResponseVariant2(WireModel):
    bot_id: str = Field(..., alias='botId')
    outcome: ActionRunOutcome = Field(...)
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['succeeded'] = Field(...)

class ActionRunResponseVariant3(WireModel):
    bot_id: str = Field(..., alias='botId')
    outcome: ActionRunOutcome = Field(...)
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['failed'] = Field(...)

class ActionRunResponseVariant4(WireModel):
    bot_id: str = Field(..., alias='botId')
    outcome: ActionRunOutcome = Field(...)
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['cancelled'] = Field(...)

class ActionRunResponseVariant5(WireModel):
    bot_id: str = Field(..., alias='botId')
    outcome: ActionRunOutcome = Field(...)
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['halted'] = Field(...)

class ActionRunResponse(RootModel['ActionRunResponseVariant1 | ActionRunResponseVariant2 | ActionRunResponseVariant3 | ActionRunResponseVariant4 | ActionRunResponseVariant5']):
    model_config = ConfigDict(frozen=True)

class ActiveRoute(WireModel):
    estimated_fuel_use: int = Field(..., alias='estimatedFuelUse')
    hops: tuple[str, ...] = Field(...)
    target: str = Field(...)
    target_poi: str | None = Field(None, alias='targetPoi')
    target_system: str = Field(..., alias='targetSystem')
    total_jumps: int = Field(..., alias='totalJumps')

class ActorPassengerState(WireModel):
    aboard: tuple[PassengerView, ...] = Field(...)
    aboard_count: int | None = Field(None)
    business_berths: PassengerBerthView = Field(...)
    business_berths_raw: str = Field(...)
    economy_berths: PassengerBerthView = Field(...)
    economy_berths_raw: str = Field(...)
    first_berths: PassengerBerthView = Field(...)
    first_berths_raw: str = Field(...)

class AgentSightingData(WireModel):
    contact: NearbyPlayer = Field(...)
    first_seen_unix: int = Field(...)
    last_seen_system: str = Field(...)
    last_seen_unix: int = Field(...)
    times_seen: int = Field(...)

class AmmoStats(WireModel):
    anti_drone_mod: float | None = Field(None)
    anti_large_mod: float | None = Field(None)
    anti_small_mod: float | None = Field(None)
    armor_bypass: float | None = Field(None)
    armor_melt_pct: float | None = Field(None)
    armor_melt_ticks: int | None = Field(None)
    damage_mod: float | None = Field(None)
    disrupt_bonus_speed: float | None = Field(None)
    disrupt_bonus_ticks: int | None = Field(None)
    disrupt_damage: float | None = Field(None)
    disrupt_speed: float | None = Field(None)
    disrupt_ticks: int | None = Field(None)
    dot_pct: float | None = Field(None)
    dot_ticks: int | None = Field(None)
    hit_chance_mod: int | None = Field(None)
    hull_damage_mod: float | None = Field(None)
    shield_bypass: float | None = Field(None)
    shield_damage_mod: float | None = Field(None)
    splash_pct: float | None = Field(None)
    untraceable: bool | None = Field(None)
    wear_per_shot: float | None = Field(None)

class BotConnectionState(RootModel["Literal['Connected', 'Disconnected']"]):
    model_config = ConfigDict(frozen=True)

class BotList(RootModel['tuple[V1BotSummary, ...]']):
    model_config = ConfigDict(frozen=True)

class BotState(WireModel):
    active_commissions: tuple[CommissionEntry, ...] = Field(...)
    cargo: FrozenDict[str, int] = Field(...)
    cargo_capacity: int = Field(...)
    cargo_items: tuple[V2GameStateCargoItem, ...] = Field(...)
    cargo_pct: int = Field(...)
    cargo_used: int = Field(...)
    combat_stance: str | None = Field(None)
    combat_target: str | None = Field(None)
    crafting_queue: tuple[CraftingQueueProjection, ...] = Field(...)
    fuel: int = Field(...)
    fuel_pct: int = Field(...)
    in_battle: bool = Field(...)
    installed_modules: tuple[str, ...] = Field(...)
    last_mined: FrozenDict[str, int] = Field(...)
    last_stored: FrozenDict[str, int] = Field(...)
    location: V2GameStateLocation = Field(...)
    max_fuel: int = Field(...)
    mission_complete: FrozenDict[str, bool] = Field(...)
    missions: MissionData = Field(...)
    modules: tuple[V2GameStateModulesItem, ...] = Field(...)
    observation_nearby: FrozenDict[str, ObservedPlayer] = Field(...)
    own_buy_orders: tuple[ExchangeOrder, ...] = Field(...)
    own_sell_orders: tuple[ExchangeOrder, ...] = Field(...)
    owned_ship_details: tuple[OwnedShipDetail, ...] = Field(...)
    passengers: ActorPassengerState = Field(...)
    player: V2GameStatePlayer = Field(...)
    script_mined_by_item: FrozenDict[str, int] = Field(...)
    script_stored_by_item: FrozenDict[str, int] = Field(...)
    ship: V2GameStateShip = Field(...)
    skills: FrozenDict[str, V2GameStateSkillsValue] = Field(...)

class BotSummary(WireModel):
    bot_id: str = Field(..., alias='botId')
    connection: V1BotConnectionState = Field(...)
    name: str | None = Field(None)
    observed_at: datetime | None = Field(None, alias='observedAt')
    state_version: int = Field(..., alias='stateVersion')

class BulkJobCancelResponse(WireModel):
    action: BulkJobCancelResponseAction = Field(...)
    kind: BulkJobCancelResponseKind = Field(...)
    message: str = Field(...)
    mode: BulkJobCancelResponseMode = Field(...)
    results: tuple[JobCancelResult, ...] = Field(...)
    summary: BulkSummary = Field(...)

class BulkJobCancelResponseAction(RootModel["Literal['job_cancel']"]):
    model_config = ConfigDict(frozen=True)

class BulkJobCancelResponseKind(RootModel["Literal['bulk_cancel']"]):
    model_config = ConfigDict(frozen=True)

class BulkJobCancelResponseMode(RootModel["Literal['bulk']"]):
    model_config = ConfigDict(frozen=True)

class BulkSummary(WireModel):
    failed: int = Field(...)
    succeeded: int = Field(...)
    total: int = Field(...)

class BuyRequest(WireModel):
    deliver_to: str | None = Field(None)
    item: str = Field(...)
    max_price: int | None = Field(None)
    place_order: bool = Field(...)
    quantity: int = Field(...)

class CancelRequest(WireModel):
    reason: str | None = Field(None)

class CatalogDumpItemsItem(RootModel['Item | Module']):
    model_config = ConfigDict(frozen=True)

class ChatMessageData(WireModel):
    channel: str = Field(...)
    content: str = Field(...)
    empire_official: bool | None = Field(None)
    faction_id: str | None = Field(None)
    id: str = Field(...)
    poi_id: str | None = Field(None)
    sender: str = Field(...)
    sender_id: str = Field(...)
    system_id: str | None = Field(None)
    target_id: str | None = Field(None)
    target_name: str | None = Field(None)
    timestamp_utc: str = Field(...)

class CommissionEntry(WireModel):
    base_id: str | None = Field(None)
    base_name: str | None = Field(None)
    build_complete_tick: int | None = Field(None)
    build_start_tick: int | None = Field(None)
    built_ship_id: str | None = Field(None)
    commission_id: str = Field(...)
    created_at: datetime | None = Field(None)
    credits_paid: int | None = Field(None)
    earmarked_credits: int | None = Field(None)
    material_cost_estimate: int | None = Field(None)
    materials_gathered: FrozenDict[str, int] | None = Field(None)
    materials_provided: bool = Field(...)
    required_materials: FrozenDict[str, int] | None = Field(None)
    ship_class_id: str = Field(...)
    ship_name: str | None = Field(None)
    status: str = Field(...)
    ticks_remaining: int | None = Field(None)

class CommissionShipRequest(WireModel):
    provide_materials: bool | None = Field(None)
    ship_class: str = Field(...)

class CraftJobResponse(WireModel):
    action: CraftJobResponseAction = Field(...)
    effective_time_per_run: float = Field(...)
    escrowed: EscrowSummary = Field(...)
    est_completion_tick: int = Field(...)
    external: bool | None = Field(None)
    facility_id: str = Field(...)
    job_id: str = Field(...)
    kind: CraftJobResponseKind = Field(...)
    message: str = Field(...)
    mode: CraftJobResponseMode = Field(...)
    produces: tuple[ItemQuantity, ...] | None = Field(None)
    recipe: str = Field(...)
    runs: int = Field(...)
    venue: str = Field(...)
    venue_type: str = Field(...)

class CraftJobResponseAction(RootModel["Literal['craft', 'recycle', 'job_add']"]):
    model_config = ConfigDict(frozen=True)

class CraftJobResponseKind(RootModel["Literal['job']"]):
    model_config = ConfigDict(frozen=True)

class CraftJobResponseMode(RootModel["Literal['craft', 'recycle']"]):
    model_config = ConfigDict(frozen=True)

class CraftJobStatus(RootModel["Literal['optimistic', 'active', 'completed', 'cancelled', 'failed', 'lost']"]):
    model_config = ConfigDict(frozen=True)

class CraftRequest(WireModel):
    destination: str | None = Field(None)
    facility_id: str | None = Field(None)
    preset: str | None = Field(None)
    quantity: int = Field(...)
    recipe_id: str = Field(...)
    source: str | None = Field(None)

class CraftReservationResponse(WireModel):
    orders: tuple[VirtualCraftOrder, ...] = Field(...)
    reservation_results: tuple[RuntimeVirtualOrderReservationResultDto, ...] = Field(..., alias='reservationResults')

class CraftingQueueProjection(WireModel):
    crafts: int | None = Field(None)
    facility_id: str | None = Field(None)
    item_id: str | None = Field(None)
    job_id: str | None = Field(None)
    order_id: str | None = Field(None)
    preset: str | None = Field(None)
    quantity: int | None = Field(None)
    raw_text: str | None = Field(None)
    recipe_id: str | None = Field(None)
    reservation_id: str | None = Field(None)
    source: str | None = Field(None)
    station_id: str | None = Field(None)
    status: CraftJobStatus | None = Field(None)

class EmptyRequest(RootModel['FrozenDict[str, Any]']):
    model_config = ConfigDict(frozen=True)

class ErrorEnvelope(WireModel):
    error: V1ErrorDetail = Field(...)
    request_id: str = Field(..., alias='requestId')

class EscrowSummary(WireModel):
    fee: int | None = Field(None)
    inputs: tuple[ItemQuantity, ...] | None = Field(None)
    labor: int | None = Field(None)

class ExchangeOrder(WireModel):
    created_at: str = Field(...)
    created_by: str | None = Field(None)
    faction_order: bool | None = Field(None)
    filled_quantity: int = Field(...)
    item_id: str = Field(...)
    item_name: str | None = Field(None)
    listing_fee: int = Field(...)
    order_id: str = Field(...)
    order_type: str = Field(...)
    price_each: int = Field(...)
    quantity: int = Field(...)
    remaining: int = Field(...)
    side: str = Field(...)

class FacilityAccessRequest(WireModel):
    access: str = Field(...)
    facility_id: str = Field(...)

class FacilityBrowseForSaleResponse(WireModel):
    action: FacilityBrowseForSaleResponseAction = Field(...)
    base_id: str = Field(...)
    base_name: str = Field(...)
    count: int = Field(...)
    listings: tuple[FacilityListingEntry, ...] = Field(...)

class FacilityBrowseForSaleResponseAction(RootModel["Literal['browse_for_sale']"]):
    model_config = ConfigDict(frozen=True)

class FacilityBuildResponse(WireModel):
    action: FacilityBuildResponseAction = Field(...)
    base_id: str = Field(...)
    facility_id: str = Field(...)
    facility_name: str = Field(...)
    facility_type: str = Field(...)
    hint: str = Field(...)
    recipe_id: str | None = Field(None)
    rent_per_cycle: int = Field(...)
    skill_xp: FrozenDict[str, int] | None = Field(None)

class FacilityBuildResponseAction(RootModel["Literal['build']"]):
    model_config = ConfigDict(frozen=True)

class FacilityBuyListingResponse(WireModel):
    action: FacilityBuyListingResponseAction = Field(...)
    credits_left: int = Field(...)
    definition_id: str = Field(...)
    facility_id: str = Field(...)
    message: str = Field(...)
    price: int = Field(...)
    sales_tax: int | None = Field(None)

class FacilityBuyListingResponseAction(RootModel["Literal['buy_listing']"]):
    model_config = ConfigDict(frozen=True)

class FacilityCancelListingResponse(WireModel):
    action: FacilityCancelListingResponseAction = Field(...)
    facility_id: str = Field(...)
    message: str = Field(...)

class FacilityCancelListingResponseAction(RootModel["Literal['cancel_listing']"]):
    model_config = ConfigDict(frozen=True)

class FacilityCategoryInfo(WireModel):
    buildable: int | None = Field(None)
    count: int = Field(...)
    description: str = Field(...)

class FacilityDefSummary(WireModel):
    build_cost: int = Field(...)
    build_materials: tuple[ItemQuantity, ...] | None = Field(None)
    build_time: int = Field(...)
    category: str = Field(...)
    description: str = Field(...)
    faction_cap: int | None = Field(None)
    faction_service: str | None = Field(None)
    labor_cost: int = Field(...)
    level: int = Field(...)
    maintenance_per_cycle: tuple[ItemQuantity, ...] | None = Field(None)
    name: str = Field(...)
    recipe_id: str | None = Field(None)
    rent_per_cycle: int = Field(...)
    type_id: str = Field(...)

class FacilityDefinition(WireModel):
    allows_contraband: bool | None = Field(None)
    always_on: bool = Field(...)
    ammo_item: str | None = Field(None)
    battery_capacity: int | None = Field(None)
    build_cost: int = Field(...)
    build_materials: tuple[RecipeInput, ...] | None = Field(None)
    build_time: int = Field(...)
    category: str = Field(...)
    degraded_description: str | None = Field(None)
    deposit_to_empire_reserves: bool | None = Field(None)
    description: str = Field(...)
    dining_points: int | None = Field(None)
    disguised: bool | None = Field(None)
    empire: str | None = Field(None)
    expansion_of: str | None = Field(None)
    expansion_scale: float | None = Field(None)
    faction_cap: int | None = Field(None)
    faction_service_type: str | None = Field(None)
    fleet_upkeep: bool | None = Field(None)
    fuel_capacity: int | None = Field(None)
    fuel_output: bool | None = Field(None)
    id: str = Field(...)
    is_recycler: bool | None = Field(None)
    labor_cost: int = Field(...)
    leisure_points: int | None = Field(None)
    level: int = Field(...)
    life_support_draw: int | None = Field(None)
    life_support_supply: int | None = Field(None)
    logistics: bool | None = Field(None)
    lore: str | None = Field(None)
    maintenance_inputs: tuple[RecipeInput, ...] | None = Field(None)
    name: str = Field(...)
    personal_bonus_type: str | None = Field(None)
    personal_bonus_value: float | None = Field(None)
    personal_service_type: str | None = Field(None)
    pirate_base_only: bool | None = Field(None)
    player_station_buildable: bool | None = Field(None)
    power_draw: int | None = Field(None)
    power_supply: int | None = Field(None)
    recipe_id: str | None = Field(None)
    repair_hull_per_item: int | None = Field(None)
    repair_item: str | None = Field(None)
    requires_service_type: str | None = Field(None)
    satisfied_description: str | None = Field(None)
    scan_falloff: int | None = Field(None)
    scan_power: int | None = Field(None)
    self_repair_rate: int | None = Field(None)
    service_type: str | None = Field(None)
    station_armor: int | None = Field(None)
    station_hull_hp: int | None = Field(None)
    station_or_faction_only: bool | None = Field(None)
    station_shield_hp: int | None = Field(None)
    tourism_upkeep: bool | None = Field(None)
    transit_deadline_bonus: int | None = Field(None)
    unique: bool | None = Field(None)
    upgrades_from: str | None = Field(None)
    weapon_cooldown: int | None = Field(None)
    weapon_damage: int | None = Field(None)
    weapon_damage_type: str | None = Field(None)
    weapon_reach: int | None = Field(None)

class FacilityDismantleMaterial(WireModel):
    item_id: str = Field(...)
    quantity: int = Field(...)

class FacilityDismantleResponse(WireModel):
    action: FacilityDismantleResponseAction = Field(...)
    base_id: str = Field(...)
    complete_tick: int = Field(...)
    facility_id: str = Field(...)
    facility_name: str = Field(...)
    facility_type: str = Field(...)
    hint: str = Field(...)
    materials_to_package: tuple[FacilityDismantleMaterial, ...] = Field(...)
    package_count: int = Field(...)
    ticks_to_complete: int = Field(...)

class FacilityDismantleResponseAction(RootModel["Literal['dismantle', 'faction_dismantle']"]):
    model_config = ConfigDict(frozen=True)

class FacilityEntry(WireModel):
    bonus_type: str | None = Field(None)
    bonus_value: float | None = Field(None)
    capacity: int | None = Field(None)
    category: str = Field(...)
    custom_name: str | None = Field(None)
    damaged: bool | None = Field(None)
    description: str = Field(...)
    dining_points: int | None = Field(None)
    facility_id: str = Field(...)
    faction_id: str | None = Field(None)
    faction_service: str | None = Field(None)
    is_recycler: bool | None = Field(None)
    labor_per_cycle: int | None = Field(None)
    leisure_points: int | None = Field(None)
    level: int = Field(...)
    maintenance_level: float | None = Field(None)
    maintenance_per_cycle: tuple[ItemQuantity, ...] | None = Field(None)
    maintenance_satisfied: bool | None = Field(None)
    missed_rent_cycles: int | None = Field(None)
    name: str = Field(...)
    owner_id: str | None = Field(None)
    personal_service: str | None = Field(None)
    power_throttled: bool | None = Field(None)
    production: FacilityProduction | None = Field(None)
    recipe_id: str | None = Field(None)
    rent_paid_until_tick: int | None = Field(None)
    rent_per_cycle: int | None = Field(None)
    repair_complete_tick: int | None = Field(None)
    service: str | None = Field(None)
    tourism_upkeep: bool | None = Field(None)
    type: str = Field(...)
    under_construction: bool | None = Field(None)

class FacilityFactionBuildResponse(WireModel):
    action: FacilityFactionBuildResponseAction = Field(...)
    base_id: str = Field(...)
    capacity: int | None = Field(None)
    facility_id: str = Field(...)
    facility_name: str = Field(...)
    facility_type: str = Field(...)
    faction_service: str = Field(...)
    hint: str = Field(...)
    members_awarded_xp: int | None = Field(None)
    recipe_id: str | None = Field(None)
    rent_per_cycle: int = Field(...)
    skill_xp: FrozenDict[str, int] | None = Field(None)
    under_construction: bool | None = Field(None)

class FacilityFactionBuildResponseAction(RootModel["Literal['faction_build']"]):
    model_config = ConfigDict(frozen=True)

class FacilityFactionEntry(WireModel):
    capacity: int | None = Field(None)
    custom_name: str | None = Field(None)
    facility_id: str = Field(...)
    faction_service: str = Field(...)
    level: int = Field(...)
    missed_rent_cycles: int | None = Field(None)
    name: str = Field(...)
    rent_per_cycle: int = Field(...)
    rental_fee_per_run: int | None = Field(None)
    status: str = Field(...)
    ticks_until_complete: int | None = Field(None)
    type: str = Field(...)

class FacilityFactionListResponse(WireModel):
    action: FacilityFactionListResponseAction = Field(...)
    base_id: str = Field(...)
    faction_facilities: tuple[FacilityFactionEntry, ...] = Field(...)
    faction_id: str = Field(...)
    faction_storage: FacilityFactionStorage | None = Field(None)
    hint: str = Field(...)

class FacilityFactionListResponseAction(RootModel["Literal['faction_list']"]):
    model_config = ConfigDict(frozen=True)

class FacilityFactionOwnedResponse(WireModel):
    action: FacilityFactionOwnedResponseAction = Field(...)
    arrears_owed: int | None = Field(None)
    facilities: tuple[FactionOwnedFacilityEntry, ...] = Field(...)
    faction_id: str = Field(...)
    grace_cycles: int | None = Field(None)
    hint: str | None = Field(None)
    note: str | None = Field(None)
    total_rent_per_cycle: int = Field(...)

class FacilityFactionOwnedResponseAction(RootModel["Literal['faction_owned']"]):
    model_config = ConfigDict(frozen=True)

class FacilityFactionStorage(WireModel):
    credits: int = Field(...)
    item_types: int = Field(...)
    rooms: int = Field(...)

class FacilityFactionUpgradeResponse(WireModel):
    action: FacilityFactionUpgradeResponseAction = Field(...)
    base_id: str = Field(...)
    capacity: int | None = Field(None)
    facility_id: str = Field(...)
    facility_name: str = Field(...)
    facility_type: str = Field(...)
    faction_service: str = Field(...)
    hint: str = Field(...)
    level: int = Field(...)
    members_awarded_xp: int | None = Field(None)
    skill_xp: FrozenDict[str, int] | None = Field(None)

class FacilityFactionUpgradeResponseAction(RootModel["Literal['faction_upgrade']"]):
    model_config = ConfigDict(frozen=True)

class FacilityHelpResponse(WireModel):
    action: FacilityHelpResponseAction = Field(...)
    help: str = Field(...)

class FacilityHelpResponseAction(RootModel["Literal['help']"]):
    model_config = ConfigDict(frozen=True)

class FacilityJobListResponse(WireModel):
    action: FacilityJobListResponseAction = Field(...)
    facility_id: str = Field(...)
    jobs: tuple[JobView, ...] = Field(...)
    message: str | None = Field(None)
    total_jobs: int = Field(...)
    venue: str = Field(...)

class FacilityJobListResponseAction(RootModel["Literal['job_list']"]):
    model_config = ConfigDict(frozen=True)

class FacilityListForSaleResponse(WireModel):
    action: FacilityListForSaleResponseAction = Field(...)
    credits_left: int | None = Field(None)
    definition_id: str = Field(...)
    facility_id: str = Field(...)
    fee: int = Field(...)
    listing_id: str = Field(...)
    message: str = Field(...)
    price: int = Field(...)

class FacilityListForSaleResponseAction(RootModel["Literal['list_for_sale']"]):
    model_config = ConfigDict(frozen=True)

class FacilityListResponse(WireModel):
    action: FacilityListResponseAction = Field(...)
    base_id: str = Field(...)
    construction: StationConstructionResponse | None = Field(None)
    faction_facilities: tuple[FacilityEntry, ...] = Field(...)
    faction_rent: FacilityRentSummary | None = Field(None)
    life_support: StationLifeSupportStatus | None = Field(None)
    player_facilities: tuple[FacilityEntry, ...] = Field(...)
    player_rent: FacilityRentSummary | None = Field(None)
    power: StationPowerStatus | None = Field(None)
    public_facilities: tuple[FacilityEntry, ...] | None = Field(None)
    station_facilities: tuple[FacilityEntry, ...] = Field(...)

class FacilityListResponseAction(RootModel["Literal['list']"]):
    model_config = ConfigDict(frozen=True)

class FacilityListingEntry(WireModel):
    build_cost: int | None = Field(None)
    build_time: int | None = Field(None)
    category: str | None = Field(None)
    compatibility_note: str | None = Field(None)
    definition_id: str = Field(...)
    facility_id: str = Field(...)
    facility_name: str | None = Field(None)
    fuel_capacity_bonus: int | None = Field(None)
    fuel_output: bool | None = Field(None)
    level: int | None = Field(None)
    listed_at: str = Field(...)
    listing_id: str = Field(...)
    price: int = Field(...)
    recipe_id: str | None = Field(None)
    required_skill_level: int | None = Field(None)
    seller_name: str | None = Field(None)
    seller_type: str = Field(...)
    skill_met: bool | None = Field(None)
    station_or_faction_only: bool | None = Field(None)
    under_construction: bool | None = Field(None)

class FacilityNameRequest(WireModel):
    custom_name: str = Field(...)
    facility_id: str = Field(...)

class FacilityOutputPriceRequest(WireModel):
    facility_id: str = Field(...)
    item: str = Field(...)
    price: int = Field(...)

class FacilityOwnedResponse(WireModel):
    action: FacilityOwnedResponseAction = Field(...)
    facilities: tuple[OwnedFacilityEntry, ...] = Field(...)
    hint: str | None = Field(None)
    rent: FacilityRentSummary = Field(...)

class FacilityOwnedResponseAction(RootModel["Literal['owned']"]):
    model_config = ConfigDict(frozen=True)

class FacilityPersonalBuildResponse(WireModel):
    action: FacilityPersonalBuildResponseAction = Field(...)
    base_id: str = Field(...)
    bonus_type: str | None = Field(None)
    bonus_value: float | None = Field(None)
    facility_id: str = Field(...)
    facility_name: str = Field(...)
    facility_type: str = Field(...)
    hint: str = Field(...)
    home_base_set: bool | None = Field(None)
    personal_service: str = Field(...)
    rent_per_cycle: int = Field(...)
    skill_xp: FrozenDict[str, int] | None = Field(None)
    under_construction: bool | None = Field(None)

class FacilityPersonalBuildResponseAction(RootModel["Literal['personal_build']"]):
    model_config = ConfigDict(frozen=True)

class FacilityPersonalDecorateResponse(WireModel):
    access: str = Field(...)
    action: FacilityPersonalDecorateResponseAction = Field(...)
    facility_id: str = Field(...)
    facility_name: str = Field(...)
    hint: str | None = Field(None)
    message: str | None = Field(None)

class FacilityPersonalDecorateResponseAction(RootModel["Literal['personal_decorate']"]):
    model_config = ConfigDict(frozen=True)

class FacilityPersonalVisitResponse(WireModel):
    access: str | None = Field(None)
    action: FacilityPersonalVisitResponseAction = Field(...)
    base_id: str = Field(...)
    description: str = Field(...)
    facility_name: str = Field(...)
    hint: str | None = Field(None)
    owner: str = Field(...)

class FacilityPersonalVisitResponseAction(RootModel["Literal['personal_visit']"]):
    model_config = ConfigDict(frozen=True)

class FacilityProduction(WireModel):
    backlog_ticks: int = Field(...)
    items_per_hour: int | None = Field(None)
    output_per_run: int | None = Field(None)
    output_price_per_operation: float | None = Field(None)
    output_price_per_unit: float | None = Field(None)
    pack_operations_per_hour: int | None = Field(None)
    public: bool | None = Field(None)
    queued_items: int = Field(...)
    queued_runs: int = Field(...)
    recipe: str | None = Field(None)
    rental_fee_per_run: int | None = Field(None)
    ticks_per_run: float | None = Field(None)
    unpack_operations_per_hour: int | None = Field(None)

class FacilityRecipeInfo(WireModel):
    crafting_time: float = Field(...)
    id: str = Field(...)
    inputs: tuple[ItemQuantity, ...] = Field(...)
    name: str = Field(...)
    outputs: tuple[ItemQuantity, ...] = Field(...)

class FacilityRentSummary(WireModel):
    arrears_owed: int | None = Field(None)
    est_rent_per_day: int = Field(...)
    facilities: int = Field(...)
    grace_cycles: int | None = Field(None)
    note: str | None = Field(None)
    total_rent_per_cycle: int = Field(...)

class FacilityRepairMaterial(WireModel):
    item_id: str = Field(...)
    quantity: int = Field(...)

class FacilityRepairResponse(WireModel):
    action: FacilityRepairResponseAction = Field(...)
    complete_tick: int = Field(...)
    facility_id: str = Field(...)
    facility_name: str = Field(...)
    hint: str = Field(...)
    materials_used: tuple[FacilityRepairMaterial, ...] = Field(...)
    ticks_to_complete: int = Field(...)

class FacilityRepairResponseAction(RootModel["Literal['repair']"]):
    model_config = ConfigDict(frozen=True)

class FacilityResponse(RootModel['FacilityListResponse | FacilityOwnedResponse | FacilityFactionOwnedResponse | FacilityHelpResponse | FacilityBuildResponse | FacilityUpgradesResponse | FacilityUpgradeResponse | FacilityDismantleResponse | FacilityRepairResponse | FacilityFactionBuildResponse | FacilityFactionUpgradeResponse | FacilityFactionListResponse | FacilityTransferResponse | FacilityPersonalBuildResponse | FacilityPersonalDecorateResponse | FacilityPersonalVisitResponse | FacilityTypeDiscoveryResponse | FacilityTypeListResponse | FacilityTypeDetailResponse | FacilityListForSaleResponse | FacilityBrowseForSaleResponse | FacilityBuyListingResponse | FacilityCancelListingResponse | CraftJobResponse | PackageJobResponse | FacilityJobListResponse | JobCancelResponse | BulkJobCancelResponse | JobReorderResponse | SetOutputPriceResponse | SetAccessResponse | SetFacilityNameResponse | SetFacilityDescriptionResponse']):
    model_config = ConfigDict(frozen=True)

class FacilityTransferResponse(WireModel):
    action: FacilityTransferResponseAction = Field(...)
    direction: str = Field(...)
    facility_id: str = Field(...)
    hint: str = Field(...)
    new_owner: str | None = Field(None)

class FacilityTransferResponseAction(RootModel["Literal['transfer']"]):
    model_config = ConfigDict(frozen=True)

class FacilityTypeDetailResponse(WireModel):
    action: FacilityTypeDetailResponseAction = Field(...)
    bonus_type: str | None = Field(None)
    bonus_value: float | None = Field(None)
    build_cost: int = Field(...)
    build_materials: tuple[ItemQuantity, ...] | None = Field(None)
    build_time: int = Field(...)
    buildable: bool = Field(...)
    category: str = Field(...)
    degraded_description: str | None = Field(None)
    description: str = Field(...)
    faction_cap: int | None = Field(None)
    faction_service: str | None = Field(None)
    hint: str | None = Field(None)
    kind: FacilityTypeDetailResponseKind = Field(...)
    labor_cost: int = Field(...)
    level: int = Field(...)
    lore: str | None = Field(None)
    maintenance_per_cycle: tuple[ItemQuantity, ...] | None = Field(None)
    name: str = Field(...)
    personal_service: str | None = Field(None)
    recipe: FacilityRecipeInfo | None = Field(None)
    recipe_id: str | None = Field(None)
    rent_per_cycle: int = Field(...)
    requires_service_name: str | None = Field(None)
    requires_service_type: str | None = Field(None)
    satisfied_description: str | None = Field(None)
    type_id: str = Field(...)
    upgrades_from: str | None = Field(None)
    upgrades_from_name: str | None = Field(None)
    upgrades_to: str | None = Field(None)
    upgrades_to_name: str | None = Field(None)

class FacilityTypeDetailResponseAction(RootModel["Literal['types']"]):
    model_config = ConfigDict(frozen=True)

class FacilityTypeDetailResponseKind(RootModel["Literal['detail']"]):
    model_config = ConfigDict(frozen=True)

class FacilityTypeDiscoveryResponse(WireModel):
    action: FacilityTypeDiscoveryResponseAction = Field(...)
    categories: FrozenDict[str, FacilityCategoryInfo] = Field(...)
    filters: FacilityTypeFilterInfo = Field(...)
    hint: str = Field(...)
    kind: FacilityTypeDiscoveryResponseKind = Field(...)
    pagination: FacilityTypePaginationInfo = Field(...)
    total: int = Field(...)

class FacilityTypeDiscoveryResponseAction(RootModel["Literal['types']"]):
    model_config = ConfigDict(frozen=True)

class FacilityTypeDiscoveryResponseKind(RootModel["Literal['discovery']"]):
    model_config = ConfigDict(frozen=True)

class FacilityTypeFilterInfo(WireModel):
    category: str = Field(...)
    level: str = Field(...)
    name: str = Field(...)

class FacilityTypeListResponse(WireModel):
    action: FacilityTypeListResponseAction = Field(...)
    hint: str = Field(...)
    kind: FacilityTypeListResponseKind = Field(...)
    page: int = Field(...)
    per_page: int = Field(...)
    total: int = Field(...)
    total_pages: int = Field(...)
    types: tuple[FacilityTypeSummary, ...] = Field(...)

class FacilityTypeListResponseAction(RootModel["Literal['types']"]):
    model_config = ConfigDict(frozen=True)

class FacilityTypeListResponseKind(RootModel["Literal['list']"]):
    model_config = ConfigDict(frozen=True)

class FacilityTypePaginationInfo(WireModel):
    page: str = Field(...)
    per_page: str = Field(...)

class FacilityTypeSummary(WireModel):
    bonus_type: str | None = Field(None)
    bonus_value: float | None = Field(None)
    build_cost: int = Field(...)
    buildable: bool | None = Field(None)
    category: str = Field(...)
    id: str = Field(...)
    level: int = Field(...)
    name: str = Field(...)
    personal_service: str | None = Field(None)
    recipe_id: str | None = Field(None)
    service: str | None = Field(None)

class FacilityUpgradeEntry(WireModel):
    current_level: int = Field(...)
    requires: str | None = Field(None)
    upgrade_to: FacilityDefSummary = Field(...)
    your_facility_id: str = Field(...)
    your_facility_name: str = Field(...)
    your_facility_type: str = Field(...)

class FacilityUpgradeRequest(WireModel):
    facility_id: str = Field(...)
    facility_type: str = Field(...)

class FacilityUpgradeResponse(WireModel):
    action: FacilityUpgradeResponseAction = Field(...)
    base_id: str = Field(...)
    bonus_type: str | None = Field(None)
    bonus_value: float | None = Field(None)
    facility_id: str = Field(...)
    facility_name: str = Field(...)
    facility_type: str = Field(...)
    hint: str = Field(...)
    level: int = Field(...)
    personal_service: str | None = Field(None)
    recipe_id: str | None = Field(None)
    rent_per_cycle: int = Field(...)

class FacilityUpgradeResponseAction(RootModel["Literal['upgrade']"]):
    model_config = ConfigDict(frozen=True)

class FacilityUpgradesResponse(WireModel):
    action: FacilityUpgradesResponseAction = Field(...)
    base_id: str = Field(...)
    faction_locked_upgrades: tuple[FacilityUpgradeEntry, ...] | None = Field(None)
    faction_upgrade_hint: str | None = Field(None)
    faction_upgrades: tuple[FacilityUpgradeEntry, ...] | None = Field(None)
    hint: str = Field(...)
    locked_upgrades: tuple[FacilityUpgradeEntry, ...] | None = Field(None)
    upgrades: tuple[FacilityUpgradeEntry, ...] = Field(...)

class FacilityUpgradesResponseAction(RootModel["Literal['upgrades']"]):
    model_config = ConfigDict(frozen=True)

class FactionMemberData(WireModel):
    online: bool = Field(...)
    player_id: str = Field(...)
    role: str = Field(...)
    username: str = Field(...)

class FactionOwnedFacilityEntry(WireModel):
    arrears_owed: int | None = Field(None)
    base_id: str = Field(...)
    base_name: str = Field(...)
    custom_name: str | None = Field(None)
    damaged: bool | None = Field(None)
    facility_id: str = Field(...)
    labor_per_run: int = Field(...)
    missed_rent_cycles: int | None = Field(None)
    name: str = Field(...)
    power_throttled: bool | None = Field(None)
    rent_per_cycle: int = Field(...)
    rental_fee_per_run: int | None = Field(None)
    repair_complete_tick: int | None = Field(None)
    system_id: str | None = Field(None)
    type: str = Field(...)
    under_construction: bool | None = Field(None)

class FactionRoleData(WireModel):
    name: str = Field(...)
    priority: int = Field(...)

class FactionSnapshotData(WireModel):
    description: str = Field(...)
    id: str = Field(...)
    is_member: bool = Field(...)
    leader_id: str = Field(...)
    leader_username: str = Field(...)
    member_count: int = Field(...)
    members: tuple[FactionMemberData, ...] = Field(...)
    name: str = Field(...)
    primary_color: str = Field(...)
    roles: tuple[FactionRoleData, ...] = Field(...)
    secondary_color: str = Field(...)
    tag: str = Field(...)
    treasury: int | None = Field(None)

class FindRequest(WireModel):
    targets: tuple[str, ...] = Field(...)

class FleetEntry(WireModel):
    active_route: ActiveRoute | None = Field(None)
    connection: BotConnectionState = Field(...)
    id: str = Field(...)
    in_transit: bool | None = Field(None)
    observed_at: datetime | None = Field(None)
    script_execution: ScriptExecution | None = Field(None)
    state: BotState = Field(...)
    transit_dest_poi: str | None = Field(None)
    transit_dest_system: str | None = Field(None)
    username: str | None = Field(None)
    version: int = Field(...)

class FleetSnapshot(WireModel):
    bots: FrozenDict[str, FleetEntry] = Field(...)

class GalaxyCatalog(WireModel):
    facilities_by_id: FrozenDict[str, FacilityDefinition] = Field(..., alias='facilitiesById')
    items_by_id: FrozenDict[str, CatalogDumpItemsItem] = Field(..., alias='itemsById')
    recipes_by_id: FrozenDict[str, Recipe] = Field(..., alias='recipesById')
    ships_by_id: FrozenDict[str, ShipClass] = Field(..., alias='shipsById')
    skills_by_id: FrozenDict[str, SkillDefinition] = Field(..., alias='skillsById')

class GalaxyMap(WireModel):
    known_pois: tuple[RuntimeGalaxyKnownPoiInfoDto, ...] = Field(..., alias='knownPois')
    systems: tuple[RuntimeGalaxySystemInfoDto, ...] = Field(...)

class GalaxyResources(WireModel):
    pois_by_resource: FrozenDict[str, tuple[str, ...]] = Field(..., alias='poisByResource')
    systems_by_resource: FrozenDict[str, tuple[str, ...]] = Field(..., alias='systemsByResource')

class GalaxyWildlife(WireModel):
    pois: tuple[RuntimeWildlifePoiDto, ...] = Field(...)
    systems: tuple[RuntimeWildlifeSystemDto, ...] = Field(...)

class GoTargetVariant1(WireModel):
    kind: Literal['identifier'] = Field(...)
    value: str = Field(...)

class GoTargetVariant2(WireModel):
    kind: Literal['system'] = Field(...)
    value: str = Field(...)

class GoTargetVariant3(WireModel):
    kind: Literal['poi'] = Field(...)
    value: str = Field(...)

class GoTargetVariant4Value(WireModel):
    x: int = Field(...)
    y: int = Field(...)

class GoTargetVariant4(WireModel):
    kind: Literal['coordinate'] = Field(...)
    value: GoTargetVariant4Value = Field(...)

class GoTarget(RootModel['GoTargetVariant1 | GoTargetVariant2 | GoTargetVariant3 | GoTargetVariant4']):
    model_config = ConfigDict(frozen=True)

class InventoryClaim(WireModel):
    item_id: str = Field(..., alias='itemId')
    location_id: str = Field(..., alias='locationId')
    lot_id: str | None = Field(None, alias='lotId')
    owner_id: str = Field(..., alias='ownerId')
    quantity: int = Field(...)
    source_kind: str = Field(..., alias='sourceKind')

class Item(WireModel):
    base_value: int = Field(...)
    category: str = Field(...)
    description: str = Field(...)
    effect: ItemEffect | None = Field(None)
    extracted_by: str | None = Field(None)
    food_type: str | None = Field(None)
    hazardous: bool | None = Field(None)
    hidden: bool | None = Field(None)
    id: str = Field(...)
    name: str = Field(...)
    quest_item: bool | None = Field(None)
    rarity: str | None = Field(None)
    region_lock: tuple[str, ...] | None = Field(None)
    size: int = Field(...)
    stackable: bool = Field(...)
    tradeable: bool = Field(...)

class ItemEffect(WireModel):
    ammo: AmmoStats | None = Field(None)
    amount: int | None = Field(None)
    duration: int | None = Field(None)
    stat: str | None = Field(None)
    subtype: str | None = Field(None)
    type: str = Field(...)

class ItemQuantity(WireModel):
    item_id: str = Field(...)
    name: str = Field(...)
    quantity: int = Field(...)

class JobCancelResponse(WireModel):
    action: JobCancelResponseAction = Field(...)
    job_id: str = Field(...)
    kind: JobCancelResponseKind = Field(...)
    message: str = Field(...)
    refunded: EscrowSummary = Field(...)

class JobCancelResponseAction(RootModel["Literal['job_cancel']"]):
    model_config = ConfigDict(frozen=True)

class JobCancelResponseKind(RootModel["Literal['cancel']"]):
    model_config = ConfigDict(frozen=True)

class JobCancelResult(WireModel):
    error: str | None = Field(None)
    error_code: str | None = Field(None)
    job_id: str = Field(...)
    refunded: EscrowSummary | None = Field(None)
    success: bool = Field(...)

class JobReorderResponse(WireModel):
    action: JobReorderResponseAction = Field(...)
    facility_id: str = Field(...)
    job_id: str = Field(...)
    message: str = Field(...)
    position: int = Field(...)

class JobReorderResponseAction(RootModel["Literal['job_reorder']"]):
    model_config = ConfigDict(frozen=True)

class JobView(WireModel):
    base_id: str | None = Field(None)
    base_name: str | None = Field(None)
    eta_ticks: int = Field(...)
    external: bool | None = Field(None)
    facility_id: str = Field(...)
    job_id: str = Field(...)
    label: str | None = Field(None)
    mode: str = Field(...)
    orderer: str = Field(...)
    package_id: str | None = Field(None)
    position: int = Field(...)
    produces: tuple[ItemQuantity, ...] | None = Field(None)
    progress: float = Field(...)
    recipe: str = Field(...)
    runs_done: int = Field(...)
    runs_remaining: int = Field(...)
    runs_total: int = Field(...)
    status: str = Field(...)
    venue: str | None = Field(None)

class LaneOwnerVariant1(WireModel):
    kind: Literal['prayer_lang'] = Field(...)

class LaneOwnerVariant2(WireModel):
    controller_kind: str = Field(...)
    kind: Literal['controller'] = Field(...)

class LaneOwnerVariant3(WireModel):
    kind: Literal['manual'] = Field(...)

class LaneOwner(RootModel['LaneOwnerVariant1 | LaneOwnerVariant2 | LaneOwnerVariant3']):
    model_config = ConfigDict(frozen=True)

class LootedModule(WireModel):
    id: str = Field(...)
    name: str = Field(...)
    type: str = Field(...)
    type_id: str = Field(...)
    wear: float = Field(...)

class MarketMovement(WireModel):
    claims: tuple[InventoryClaim, ...] = Field(...)
    context: Any = Field(...)
    created_at_unix: int = Field(..., alias='createdAtUnix')
    kind: str = Field(...)
    movement_id: str = Field(..., alias='movementId')
    session_id: str = Field(..., alias='sessionId')
    status: MarketMovementStatus = Field(...)
    updated_at_unix: int = Field(..., alias='updatedAtUnix')
    virtual_order_uses: tuple[ReservationUse, ...] = Field(..., alias='virtualOrderUses')

class MarketMovementHealth(WireModel):
    active: bool = Field(...)
    backed_quantity: int = Field(..., alias='backedQuantity')
    claims: tuple[RuntimeInventoryClaimHealthDto, ...] = Field(...)
    fully_backed: bool = Field(..., alias='fullyBacked')
    movement_id: str = Field(..., alias='movementId')
    requested_quantity: int = Field(..., alias='requestedQuantity')
    shortfall_quantity: int = Field(..., alias='shortfallQuantity')
    status: MarketMovementStatus = Field(...)

class MarketMovementList(WireModel):
    movements: tuple[MarketMovement, ...] = Field(...)

class MarketMovementReserveRequest(WireModel):
    claims: tuple[InventoryClaim, ...] | None = Field(None)
    context: Any | None = Field(None)
    kind: str = Field(...)
    session_id: str = Field(..., alias='sessionId')
    virtual_order_uses: tuple[ReservationUse, ...] | None = Field(None, alias='virtualOrderUses')

class MarketMovementReserveResponse(WireModel):
    accepted: bool = Field(...)
    movement: MarketMovement | None = Field(None)
    unavailable_claims: tuple[InventoryClaim, ...] | None = Field(None, alias='unavailableClaims')
    unavailable_virtual_order_uses: tuple[ReservationUse, ...] | None = Field(None, alias='unavailableVirtualOrderUses')

class MarketMovementStatus(RootModel["Literal['reserved', 'running', 'completed', 'failed', 'released', 'needs_reconciliation']"]):
    model_config = ConfigDict(frozen=True)

class MarketMovementTransitionRequest(WireModel):
    reason: str = Field(...)

class Meta(WireModel):
    action_schema_version: int = Field(..., alias='actionSchemaVersion')
    api_version: str = Field(..., alias='apiVersion')
    capabilities: tuple[str, ...] = Field(...)
    server_version: str = Field(..., alias='serverVersion')

class MissionData(WireModel):
    active: tuple[str, ...] = Field(...)
    active_details: tuple[V2GameStateMissionsActiveItem, ...] = Field(...)
    available: tuple[str, ...] = Field(...)
    available_details: tuple[MissionInfo, ...] = Field(...)

class MissionDialogInfo(WireModel):
    accept: str | None = Field(None)
    complete: str | None = Field(None)
    decline: str | None = Field(None)
    offer: str | None = Field(None)

class MissionGiverInfo(WireModel):
    name: str = Field(...)
    title: str = Field(...)

class MissionInfo(WireModel):
    chain_next: str | None = Field(None)
    community: bool | None = Field(None)
    community_percent: float | None = Field(None)
    community_progress: FrozenDict[str, str] | None = Field(None)
    description: str = Field(...)
    dialog: MissionDialogInfo | None = Field(None)
    difficulty: int = Field(...)
    expires_in_ticks: int = Field(...)
    faction_id: str | None = Field(None)
    faction_name: str | None = Field(None)
    giver: MissionGiverInfo | None = Field(None)
    issuing_base: str | None = Field(None)
    issuing_base_id: str | None = Field(None)
    issuing_system_id: str | None = Field(None)
    issuing_system_name: str | None = Field(None)
    mission_id: str = Field(...)
    objectives: tuple[ObjectiveInfo, ...] | None = Field(None)
    provided_items: FrozenDict[str, int] | None = Field(None)
    repeatable: bool | None = Field(None)
    required_modules: tuple[str, ...] | None = Field(None)
    rewards: MissionRewardsInfo = Field(...)
    template_id: str | None = Field(None)
    title: str = Field(...)
    type: str = Field(...)
    warnings: tuple[str, ...] | None = Field(None)

class MissionRewardsInfo(WireModel):
    credits: int = Field(...)
    items: FrozenDict[str, int] | None = Field(None)
    pirate_rep: int | None = Field(None)
    reputation: int | None = Field(None)
    skill_xp: FrozenDict[str, int] | None = Field(None)

class Module(WireModel):
    accuracy_bonus: int | None = Field(None)
    ammo_type: str | None = Field(None)
    armor_bonus: int | None = Field(None)
    armor_bypass_bonus: float | None = Field(None)
    armor_repair_rate: int | None = Field(None)
    base_value: int = Field(...)
    cargo_bonus: int | None = Field(None)
    cloak_strength: int | None = Field(None)
    cooldown: int | None = Field(None)
    cpu_bonus: int | None = Field(None)
    cpu_usage: int = Field(...)
    current_cool: int | None = Field(None)
    damage: int | None = Field(None)
    damage_reduction: int | None = Field(None)
    damage_type: str | None = Field(None)
    description: str = Field(...)
    dining_points: int | None = Field(None)
    disruptor_power: int | None = Field(None)
    drone_bandwidth: int | None = Field(None)
    drone_capacity: int | None = Field(None)
    fuel_efficiency: int | None = Field(None)
    hidden: bool | None = Field(None)
    hull_bonus: int | None = Field(None)
    hull_penalty: int | None = Field(None)
    id: str = Field(...)
    leisure_points: int | None = Field(None)
    magazine_size: int | None = Field(None)
    max_fuel_bonus: int | None = Field(None)
    mining_power: int | None = Field(None)
    name: str = Field(...)
    passenger_business_berths: int | None = Field(None)
    passenger_comfort: int | None = Field(None)
    passenger_economy_berths: int | None = Field(None)
    passenger_first_berths: int | None = Field(None)
    passive_recipe: str | None = Field(None)
    power_bonus: int | None = Field(None)
    power_usage: int = Field(...)
    precision_factor: float | None = Field(None)
    quest_item: bool | None = Field(None)
    reach: int | None = Field(None)
    remote_repair_power: int | None = Field(None)
    required_skills: FrozenDict[str, int] | None = Field(None)
    resistance_bonus: FrozenDict[str, int] | None = Field(None)
    salvage_bonus: int | None = Field(None)
    scanner_power: int | None = Field(None)
    scramble_power: int | None = Field(None)
    shield_bonus: int | None = Field(None)
    shield_bypass_bonus: float | None = Field(None)
    shield_recharge_bonus: int | None = Field(None)
    signature_bonus: int | None = Field(None)
    size: int = Field(...)
    slot: str = Field(...)
    special: str | None = Field(None)
    speed_bonus: int | None = Field(None)
    speed_penalty: int | None = Field(None)
    survey_power: int | None = Field(None)
    tow_speed_penalty: int | None = Field(None)
    tracking_bonus: int | None = Field(None)
    type: str = Field(...)
    type_id: str = Field(...)
    warp_stabilization: int | None = Field(None)
    webify_strength: int | None = Field(None)

class NearbyPlayer(WireModel):
    clan_tag: str | None = Field(None)
    docked: bool | None = Field(None)
    faction_id: str | None = Field(None)
    faction_tag: str | None = Field(None)
    in_combat: bool | None = Field(None)
    offline: bool | None = Field(None)
    player_id: str | None = Field(None)
    primary_color: str | None = Field(None)
    secondary_color: str | None = Field(None)
    ship_class: str | None = Field(None)
    ship_name: str | None = Field(None)
    status_message: str | None = Field(None)
    username: str | None = Field(None)

class ObjectiveInfo(WireModel):
    description: str = Field(...)
    eligible_players: tuple[str, ...] | None = Field(None)
    item_id: str | None = Field(None)
    participants: tuple[str, ...] | None = Field(None)
    quantity: int | None = Field(None)
    system_id: str | None = Field(None)
    system_name: str | None = Field(None)
    target_base_id: str | None = Field(None)
    target_base_name: str | None = Field(None)
    type: str = Field(...)

class ObservedPlayer(RootModel['NearbyPlayer | NearbyPlayer | NearbyPlayer | NearbyPlayer']):
    model_config = ConfigDict(frozen=True)

class OrderLevel(WireModel):
    my_quantity: int | None = Field(None)
    price_each: int = Field(...)
    quantity: int = Field(...)
    source: str | None = Field(None)

class OverrideResponse(WireModel):
    accepted: bool = Field(...)

class OwnedFacilityEntry(WireModel):
    arrears_owed: int | None = Field(None)
    base_id: str = Field(...)
    base_name: str = Field(...)
    custom_name: str | None = Field(None)
    damaged: bool | None = Field(None)
    facility_id: str = Field(...)
    labor_per_run: int | None = Field(None)
    missed_rent_cycles: int | None = Field(None)
    name: str = Field(...)
    power_throttled: bool | None = Field(None)
    rent_per_cycle: int = Field(...)
    rental_fee_per_run: int | None = Field(None)
    repair_complete_tick: int | None = Field(None)
    system_id: str | None = Field(None)
    type: str = Field(...)
    under_construction: bool | None = Field(None)

class OwnedShipDetail(WireModel):
    cargo_used: int | None = Field(None)
    class_id: str = Field(...)
    class_name: str | None = Field(None)
    custom_name: str | None = Field(None)
    fuel: str | None = Field(None)
    hull: str | None = Field(None)
    is_active: bool = Field(...)
    listing_base_id: str | None = Field(None)
    listing_id: str | None = Field(None)
    listing_price: int | None = Field(None)
    location: str | None = Field(None)
    location_base_id: str | None = Field(None)
    modules: int | None = Field(None)
    ship_id: str = Field(...)

class PackageJobResponse(WireModel):
    action: PackageJobResponseAction = Field(...)
    escrowed: EscrowSummary = Field(...)
    eta_ticks: int = Field(...)
    external: bool | None = Field(None)
    job_id: str = Field(...)
    kind: PackageJobResponseKind = Field(...)
    label: str = Field(...)
    message: str = Field(...)
    package_id: str = Field(...)
    venue: str = Field(...)

class PackageJobResponseAction(RootModel["Literal['pack', 'unpack']"]):
    model_config = ConfigDict(frozen=True)

class PackageJobResponseKind(RootModel["Literal['package']"]):
    model_config = ConfigDict(frozen=True)

class PassengerBerthView(WireModel):
    current: int = Field(...)
    max: int = Field(...)

class PassengerState(WireModel):
    aboard: tuple[PassengerView, ...] | None = Field(None)
    aboard_count: int | None = Field(None)
    business_berths: PassengerBerthView = Field(...)
    business_berths_raw: str = Field(...)
    economy_berths: PassengerBerthView = Field(...)
    economy_berths_raw: str = Field(...)
    first_berths: PassengerBerthView = Field(...)
    first_berths_raw: str = Field(...)
    station: str = Field(...)
    waiting: tuple[WaitingPassengerView, ...] | None = Field(None)
    waiting_count: int | None = Field(None)

class PassengerView(WireModel):
    base_fare: int = Field(...)
    berth_class: str | None = Field(None)
    bio: str = Field(...)
    citizen_id: str = Field(...)
    class_: str = Field(..., alias='class')
    connecting: bool | None = Field(None)
    destination: str = Field(...)
    destination_name: str = Field(...)
    destination_system: str | None = Field(None)
    name: str = Field(...)
    speed_bonus: int | None = Field(None)
    ticks_remaining: int = Field(...)

class PoiFacilitiesSnapshot(WireModel):
    current: FacilityResponse | None = Field(None)
    faction_current: FacilityResponse | None = Field(None)
    observed_at_unix: int | None = Field(None)

class QueueLane(WireModel):
    active: bool = Field(...)
    pending_actions: int = Field(..., alias='pendingActions')
    prayerlang: str = Field(...)

class QueueResponse(WireModel):
    prayerlang: str = Field(...)
    scheduler: QueueSnapshot = Field(...)
    script_execution: ScriptExecutionDto | None = Field(None, alias='scriptExecution')

class QueueSnapshot(WireModel):
    generation: int = Field(...)
    halt_reason: str | None = Field(None, alias='haltReason')
    halted: bool = Field(...)
    interrupt_active: bool = Field(..., alias='interruptActive')
    owner: LaneOwner | None = Field(None)
    pending_actions: int = Field(..., alias='pendingActions')
    running_action: bool = Field(..., alias='runningAction')

class Recipe(WireModel):
    category: str = Field(...)
    crafting_time: float = Field(...)
    description: str = Field(...)
    facility_only: bool | None = Field(None)
    fuel_output: int | None = Field(None)
    hidden: bool | None = Field(None)
    id: str = Field(...)
    inputs: tuple[RecipeInput, ...] = Field(...)
    name: str = Field(...)
    no_recycle: bool | None = Field(None)
    outputs: tuple[RecipeOutput, ...] = Field(...)
    package_operation: str | None = Field(None)

class RecipeInput(WireModel):
    item_id: str = Field(...)
    quantity: int = Field(...)

class RecipeOutput(WireModel):
    item_id: str = Field(...)
    quantity: int = Field(...)

class RecycleRequest(WireModel):
    destination: str | None = Field(None)
    facility_id: str | None = Field(None)
    quantity: int = Field(...)
    recipe_id: str = Field(...)
    source: str | None = Field(None)

class RegisterBotRequest(WireModel):
    empire: str = Field(...)
    registration_code: str = Field(..., alias='registrationCode')
    username: str = Field(...)

class RegisterBotResponse(WireModel):
    bot: V1BotSummary = Field(...)
    password: str = Field(...)
    player_id: str = Field(..., alias='playerId')

class ReservationRequest(WireModel):
    uses: tuple[ReservationUse, ...] | None = Field(None)

class ReservationResponse(WireModel):
    orders: tuple[VirtualMarketOrder, ...] = Field(...)
    reservation_results: tuple[RuntimeVirtualOrderReservationResultDto, ...] = Field(..., alias='reservationResults')

class ReservationResult(WireModel):
    accepted: int = Field(...)
    order_id: str = Field(..., alias='orderId')
    requested: int = Field(...)
    reservation_id: str | None = Field(None, alias='reservationId')
    reserved_after: int = Field(..., alias='reservedAfter')
    reserved_before: int = Field(..., alias='reservedBefore')

class ReservationUse(WireModel):
    order_id: str = Field(..., alias='orderId')
    quantity: int = Field(...)

class RouteBatchRequest(WireModel):
    routes: tuple[RouteQuery, ...] = Field(...)
    safe: bool | None = Field(None)

class RouteBatchResponse(WireModel):
    routes: tuple[RouteSelection | None, ...] = Field(...)

class RouteQuery(WireModel):
    from_: str = Field(..., alias='from')
    to: str = Field(...)

class RouteSelection(WireModel):
    cost: int = Field(...)
    from_: str = Field(..., alias='from')
    from_system: str = Field(..., alias='fromSystem')
    hops: tuple[str, ...] = Field(...)
    safe: bool = Field(...)
    to: str = Field(...)
    to_system: str = Field(..., alias='toSystem')
    total_jumps: int = Field(..., alias='totalJumps')

class RuntimeGalaxyKnownPoiInfoDto(WireModel):
    base_id: str | None = Field(None, alias='baseId')
    base_name: str | None = Field(None, alias='baseName')
    first_discovered_unix: int | None = Field(None, alias='firstDiscoveredUnix')
    first_visited_unix: int | None = Field(None, alias='firstVisitedUnix')
    has_base: bool = Field(..., alias='hasBase')
    id: str = Field(...)
    last_observed_unix: int | None = Field(None, alias='lastObservedUnix')
    last_visited_unix: int | None = Field(None, alias='lastVisitedUnix')
    name: str = Field(...)
    resources: tuple[RuntimePoiResourceInfoDto, ...] = Field(...)
    system_id: str = Field(..., alias='systemId')
    type: str = Field(...)
    x: float | None = Field(None)
    y: float | None = Field(None)

class RuntimeGalaxyPoiInfoDto(WireModel):
    id: str = Field(...)
    x: float | None = Field(None)
    y: float | None = Field(None)

class RuntimeGalaxySystemInfoDto(WireModel):
    bloom_intensity: float | None = Field(None, alias='bloomIntensity')
    bloom_status: str | None = Field(None, alias='bloomStatus')
    connections: tuple[str, ...] = Field(...)
    empire: str = Field(...)
    faint_signatures: tuple[Any, ...] = Field(..., alias='faintSignatures')
    first_entered_unix: int | None = Field(None, alias='firstEnteredUnix')
    id: str = Field(...)
    is_stronghold: bool = Field(..., alias='isStronghold')
    last_entered_unix: int | None = Field(None, alias='lastEnteredUnix')
    last_scanned_unix: int | None = Field(None, alias='lastScannedUnix')
    last_surveyed_unix: int | None = Field(None, alias='lastSurveyedUnix')
    name: str | None = Field(None)
    poi_count: int | None = Field(None, alias='poiCount')
    pois: tuple[RuntimeGalaxyPoiInfoDto, ...] = Field(...)
    pois_complete: bool = Field(..., alias='poisComplete')
    wildlife: tuple[Any, ...] = Field(...)
    x: float | None = Field(None)
    y: float | None = Field(None)

class RuntimeInventoryClaimHealthDto(WireModel):
    backed_quantity: int = Field(..., alias='backedQuantity')
    item_id: str = Field(..., alias='itemId')
    location_id: str = Field(..., alias='locationId')
    requested_quantity: int = Field(..., alias='requestedQuantity')
    shortfall_quantity: int = Field(..., alias='shortfallQuantity')
    source_kind: str = Field(..., alias='sourceKind')

class RuntimePoiResourceInfoDto(WireModel):
    name: str = Field(...)
    remaining: int | None = Field(None)
    remaining_display: str = Field(..., alias='remainingDisplay')
    resource_id: str = Field(..., alias='resourceId')
    richness: int | None = Field(None)
    richness_text: str = Field(..., alias='richnessText')

class RuntimeVirtualOrderReservationResultDto(WireModel):
    accepted: int = Field(...)
    order_id: str = Field(..., alias='orderId')
    requested: int = Field(...)
    reservation_id: str | None = Field(None, alias='reservationId')
    reserved_after: int = Field(..., alias='reservedAfter')
    reserved_before: int = Field(..., alias='reservedBefore')

class RuntimeWildlifeCreatureDto(WireModel):
    creature_id: str = Field(..., alias='creatureId')
    hull: int = Field(...)
    in_combat: bool = Field(..., alias='inCombat')
    max_hull: int = Field(..., alias='maxHull')
    name: str = Field(...)
    observed_at_unix: int = Field(..., alias='observedAtUnix')
    poi_id: str = Field(..., alias='poiId')
    role: str = Field(...)
    species: str = Field(...)
    system_id: str = Field(..., alias='systemId')

class RuntimeWildlifePoiDto(WireModel):
    creature_count: int = Field(..., alias='creatureCount')
    creatures: tuple[RuntimeWildlifeCreatureDto, ...] = Field(...)
    observed_at_unix: int = Field(..., alias='observedAtUnix')
    poi_id: str = Field(..., alias='poiId')
    system_id: str = Field(..., alias='systemId')

class RuntimeWildlifeSpeciesDto(WireModel):
    count: int = Field(...)
    name: str = Field(...)
    role: str = Field(...)
    species: str = Field(...)

class RuntimeWildlifeSystemDto(WireModel):
    creature_count: int = Field(..., alias='creatureCount')
    observed_at_unix: int = Field(..., alias='observedAtUnix')
    pois: tuple[str, ...] = Field(...)
    species: tuple[RuntimeWildlifeSpeciesDto, ...] = Field(...)
    system_id: str = Field(..., alias='systemId')

class SalvageData(WireModel):
    last_seen_poi: str | None = Field(None)
    last_seen_system: str | None = Field(None)
    lootables_by_poi: FrozenDict[str, tuple[SpaceLootInfo, ...]] | None = Field(None)
    observed_at_unix: int | None = Field(None)
    visible_lootables: tuple[SpaceLootInfo, ...] = Field(...)

class SayRequest(WireModel):
    channel: str = Field(...)
    content: str = Field(...)
    target: str | None = Field(None)

class ScriptErrorKind(RootModel["Literal['runtime', 'user_halt', 'cancelled', 'replaced', 'shutdown', 'runner_exited', 'internal']"]):
    model_config = ConfigDict(frozen=True)

class ScriptErrorKindDto(RootModel["Literal['runtime', 'user_halt', 'cancelled', 'replaced', 'shutdown', 'runner_exited', 'internal']"]):
    model_config = ConfigDict(frozen=True)

class ScriptExecution(WireModel):
    current_line: int | None = Field(None, alias='currentLine')
    frame_kind: str | None = Field(None, alias='frameKind')
    frame_name: str | None = Field(None, alias='frameName')
    id: str = Field(...)
    last_line: int | None = Field(None, alias='lastLine')
    outcome: ScriptExecutionOutcome | None = Field(None)
    run_id: str | None = Field(None, alias='runId')
    script: str | None = Field(None)
    state: str = Field(...)

class ScriptExecutionDtoVariant1(WireModel):
    current_line: int | None = Field(None)
    last_line: int | None = Field(None)
    outcome: ScriptOutcomeDto | None = Field(None)
    state: Literal['running'] = Field(...)

class ScriptExecutionDtoVariant2(WireModel):
    current_line: int | None = Field(None)
    last_line: int | None = Field(None)
    outcome: ScriptOutcomeDto = Field(...)
    state: Literal['stopped'] = Field(...)

class ScriptExecutionDto(RootModel['ScriptExecutionDtoVariant1 | ScriptExecutionDtoVariant2']):
    model_config = ConfigDict(frozen=True)

class ScriptExecutionOutcomeVariant1(WireModel):
    message: str | None = Field(None)
    status: Literal['success'] = Field(...)

class ScriptExecutionOutcomeVariant2(WireModel):
    kind: str = Field(...)
    message: str = Field(...)
    status: Literal['error'] = Field(...)

class ScriptExecutionOutcome(RootModel['ScriptExecutionOutcomeVariant1 | ScriptExecutionOutcomeVariant2']):
    model_config = ConfigDict(frozen=True)

class ScriptOutcomeDtoVariant1(WireModel):
    message: str | None = Field(None)
    status: Literal['success'] = Field(...)

class ScriptOutcomeDtoVariant2(WireModel):
    kind: ScriptErrorKindDto = Field(...)
    message: str = Field(...)
    status: Literal['error'] = Field(...)

class ScriptOutcomeDto(RootModel['ScriptOutcomeDtoVariant1 | ScriptOutcomeDtoVariant2']):
    model_config = ConfigDict(frozen=True)

class ScriptOverrideRequest(WireModel):
    return_to_origin: bool | None = Field(None, alias='returnToOrigin')
    script: str = Field(...)

class ScriptRunOutcomeVariant1(WireModel):
    message: str | None = Field(None)
    status: Literal['success'] = Field(...)

class ScriptRunOutcomeVariant2(WireModel):
    kind: ScriptErrorKind = Field(...)
    message: str = Field(...)
    status: Literal['error'] = Field(...)

class ScriptRunOutcome(RootModel['ScriptRunOutcomeVariant1 | ScriptRunOutcomeVariant2']):
    model_config = ConfigDict(frozen=True)

class ScriptRunRequest(WireModel):
    idempotency_key: str | None = Field(None, alias='idempotencyKey')
    script: str = Field(...)

class ScriptRunResponseVariant1(WireModel):
    bot_id: str = Field(..., alias='botId')
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['running'] = Field(...)

class ScriptRunResponseVariant2(WireModel):
    bot_id: str = Field(..., alias='botId')
    outcome: ScriptRunOutcome = Field(...)
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['succeeded'] = Field(...)

class ScriptRunResponseVariant3(WireModel):
    bot_id: str = Field(..., alias='botId')
    outcome: ScriptRunOutcome = Field(...)
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['failed'] = Field(...)

class ScriptRunResponseVariant4(WireModel):
    bot_id: str = Field(..., alias='botId')
    outcome: ScriptRunOutcome = Field(...)
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['cancelled'] = Field(...)

class ScriptRunResponseVariant5(WireModel):
    bot_id: str = Field(..., alias='botId')
    outcome: ScriptRunOutcome = Field(...)
    prayerlang: str = Field(...)
    run_id: str = Field(..., alias='runId')
    run_version: int = Field(..., alias='runVersion')
    status: Literal['halted'] = Field(...)

class ScriptRunResponse(RootModel['ScriptRunResponseVariant1 | ScriptRunResponseVariant2 | ScriptRunResponseVariant3 | ScriptRunResponseVariant4 | ScriptRunResponseVariant5']):
    model_config = ConfigDict(frozen=True)

class SellRequest(WireModel):
    item: str | None = Field(None)
    min_price: int | None = Field(None)
    place_order: bool = Field(...)
    quantity: int | None = Field(None)

class ServiceTransferRequest(WireModel):
    item: str | None = Field(None)
    quantity: int | None = Field(None)
    target: str | None = Field(None)

class SetAccessResponse(WireModel):
    access: str = Field(...)
    action: SetAccessResponseAction = Field(...)
    facility_id: str = Field(...)
    message: str = Field(...)

class SetAccessResponseAction(RootModel["Literal['set_access']"]):
    model_config = ConfigDict(frozen=True)

class SetFacilityDescriptionResponse(WireModel):
    action: SetFacilityDescriptionResponseAction = Field(...)
    description: str | None = Field(None)
    facility_id: str = Field(...)
    message: str = Field(...)

class SetFacilityDescriptionResponseAction(RootModel["Literal['set_description']"]):
    model_config = ConfigDict(frozen=True)

class SetFacilityNameResponse(WireModel):
    action: SetFacilityNameResponseAction = Field(...)
    custom_name: str | None = Field(None)
    facility_id: str = Field(...)
    message: str = Field(...)

class SetFacilityNameResponseAction(RootModel["Literal['set_name']"]):
    model_config = ConfigDict(frozen=True)

class SetOutputPriceResponse(WireModel):
    action: SetOutputPriceResponseAction = Field(...)
    facility_id: str = Field(...)
    message: str = Field(...)
    price: float = Field(...)

class SetOutputPriceResponseAction(RootModel["Literal['set_output_price']"]):
    model_config = ConfigDict(frozen=True)

class ShipCargoItem(WireModel):
    item_id: str = Field(...)
    name: str | None = Field(None)
    quantity: int = Field(...)
    size: int | None = Field(None)

class ShipClass(WireModel):
    base_armor: int | None = Field(None)
    base_fuel: int | None = Field(None)
    base_hull: int | None = Field(None)
    base_shield: int | None = Field(None)
    base_shield_recharge: int | None = Field(None)
    base_speed: int | None = Field(None)
    based_on: str | None = Field(None)
    build_materials: FrozenDict[str, int] | None = Field(None)
    build_time: int | None = Field(None)
    cargo_capacity: int | None = Field(None)
    category: str | None = Field(None)
    class_: str = Field(..., alias='class')
    cpu_capacity: int | None = Field(None)
    default_loadout_version: int | None = Field(None)
    default_modules: tuple[str, ...] | None = Field(None)
    defense_slots: int | None = Field(None)
    description: str | None = Field(None)
    faction: str | None = Field(None)
    flavor_tags: tuple[str, ...] | None = Field(None)
    hidden: bool | None = Field(None)
    id: str = Field(...)
    inherent_capabilities: tuple[ShipClassInherentCapabilitiesItem, ...] | None = Field(None)
    legacy: bool | None = Field(None)
    lore: str | None = Field(None)
    name: str = Field(...)
    passive_recipes: tuple[str, ...] | None = Field(None)
    piloting_required: int | None = Field(None)
    power_capacity: int | None = Field(None)
    prestige_lock: str | None = Field(None)
    required_achievement: str | None = Field(None)
    required_faction_achievement: str | None = Field(None)
    required_faction_leader: bool | None = Field(None)
    required_items: tuple[FrozenDict[str, Any], ...] | None = Field(None)
    required_reputation: int | None = Field(None)
    scale: int | None = Field(None)
    shipyard_tier: int | None = Field(None)
    special: str | None = Field(None)
    starter_ship: bool | None = Field(None)
    tier: int | None = Field(None)
    tow_speed_bonus: int | None = Field(None)
    utility_slots: int | None = Field(None)
    weapon_slots: int | None = Field(None)

class ShipClassInherentCapabilitiesItem(WireModel):
    flag: str | None = Field(None)
    type: str | None = Field(None)
    value: int | None = Field(None)

class SkillDefinition(WireModel):
    bonus_per_level: FrozenDict[str, int] | None = Field(None)
    category: str = Field(...)
    description: str = Field(...)
    empire_restriction: str | None = Field(None)
    id: str = Field(...)
    max_level: int = Field(...)
    name: str = Field(...)
    training_source: str | None = Field(None)
    xp_per_level: tuple[int, ...] = Field(...)

class SpaceLootInfo(WireModel):
    cargo: tuple[ShipCargoItem, ...] = Field(...)
    created_at: str | None = Field(None)
    expire_tick: int | None = Field(None)
    expires_at: str | None = Field(None)
    id: str = Field(...)
    killer_name: str | None = Field(None)
    kind: str = Field(...)
    modules: tuple[LootedModule, ...] = Field(...)
    poi_id: str = Field(...)
    salvage_value: int | None = Field(None)
    ship_class: str | None = Field(None)
    ship_name: str | None = Field(None)
    system_id: str = Field(...)
    victim_name: str | None = Field(None)

class StateResponse(WireModel):
    catalog: GalaxyCatalog | None = Field(None)
    fleet: FleetSnapshot | None = Field(None)
    versions: StateVersions = Field(...)
    world: WorldState | None = Field(None)

class StateVersions(WireModel):
    catalog: str = Field(...)
    communications: int = Field(...)
    facilities: int = Field(...)
    factions: int = Field(...)
    fleet: int = Field(...)
    map: int = Field(...)
    markets: int = Field(...)
    observations: int = Field(...)
    resources: int = Field(...)
    storage: int = Field(...)
    wildlife: int = Field(...)
    world: int = Field(...)

class StationConstructionEntry(WireModel):
    build_cost: int | None = Field(None)
    category: str = Field(...)
    definition_id: str = Field(...)
    materials: tuple[StationConstructionMaterial, ...] | None = Field(None)
    name: str = Field(...)
    reason: str | None = Field(None)
    status: str = Field(...)
    ticks_until_complete: int | None = Field(None)

class StationConstructionMaterial(WireModel):
    item_id: str = Field(...)
    name: str | None = Field(None)
    quantity_in_storage: int = Field(...)
    quantity_missing: int | None = Field(None)
    quantity_required: int = Field(...)

class StationConstructionResponse(WireModel):
    pending: tuple[StationConstructionEntry, ...] | None = Field(None)
    under_construction: tuple[StationConstructionEntry, ...] | None = Field(None)

class StationLifeSupportInput(WireModel):
    item_id: str = Field(...)
    name: str | None = Field(None)
    quantity_per_cycle: int = Field(...)

class StationLifeSupportStatus(WireModel):
    demand: int = Field(...)
    maintenance: tuple[StationLifeSupportInput, ...] | None = Field(None)
    maintenance_cycle_ticks: int = Field(...)
    plants: int = Field(...)
    remediation: str | None = Field(None)
    starved: tuple[StationLifeSupportInput, ...] | None = Field(None)
    supply: int = Field(...)

class StationMarketData(WireModel):
    buy_orders: FrozenDict[str, tuple[OrderLevel, ...]] = Field(...)
    current_tick: int | None = Field(None)
    observed_at_unix: int | None = Field(None)
    sell_orders: FrozenDict[str, tuple[OrderLevel, ...]] = Field(...)

class StationMarketDelta(WireModel):
    base_version: int = Field(..., alias='baseVersion')
    remove: tuple[str, ...] = Field(...)
    upsert: FrozenDict[str, StationMarketData] = Field(...)

class StationMarkets(RootModel['FrozenDict[str, StationMarketData]']):
    model_config = ConfigDict(frozen=True)

class StationPowerInput(WireModel):
    item_id: str = Field(...)
    name: str | None = Field(None)
    quantity_per_cycle: int = Field(...)

class StationPowerStatus(WireModel):
    battery_capacity: int = Field(...)
    battery_stored: int = Field(...)
    current_draw: int = Field(...)
    efficiency: float = Field(...)
    fuel_inputs: tuple[StationPowerInput, ...] | None = Field(None)
    remediation: str | None = Field(None)
    supply: int = Field(...)

class StorageByOwner(RootModel['FrozenDict[str, FrozenDict[str, FrozenDict[str, int]]]']):
    model_config = ConfigDict(frozen=True)

class TradeItem(WireModel):
    item: str = Field(...)
    quantity: int = Field(...)

class TradeOfferRequest(WireModel):
    offer_credits: int | None = Field(None)
    offer_items: tuple[TradeItem, ...] = Field(...)
    request_credits: int | None = Field(None)
    request_items: tuple[TradeItem, ...] = Field(...)
    target: str = Field(...)

class TransferEndpointVariant1(WireModel):
    kind: Literal['cargo'] = Field(...)

class TransferEndpointVariant2(WireModel):
    kind: Literal['storage'] = Field(...)

class TransferEndpointVariant3(WireModel):
    id: str = Field(...)
    kind: Literal['ship'] = Field(...)

class TransferEndpointVariant4(WireModel):
    kind: Literal['faction'] = Field(...)

class TransferEndpointVariant5(WireModel):
    id: str = Field(...)
    kind: Literal['faction_tag'] = Field(...)

class TransferEndpointVariant6(WireModel):
    id: str = Field(...)
    kind: Literal['player'] = Field(...)

class TransferEndpointVariant7(WireModel):
    id: str | None = Field(...)
    kind: Literal['space'] = Field(...)

class TransferEndpointVariant8(WireModel):
    id: str = Field(...)
    kind: Literal['commission'] = Field(...)

class TransferEndpoint(RootModel['TransferEndpointVariant1 | TransferEndpointVariant2 | TransferEndpointVariant3 | TransferEndpointVariant4 | TransferEndpointVariant5 | TransferEndpointVariant6 | TransferEndpointVariant7 | TransferEndpointVariant8']):
    model_config = ConfigDict(frozen=True)

class TransferItem(WireModel):
    id: str = Field(...)
    quantity: int = Field(...)

class TransferRequest(WireModel):
    from_: TransferEndpoint = Field(..., alias='from')
    subject: TransferSubject = Field(...)
    to: TransferEndpoint = Field(...)

class TransferSubjectVariant1(WireModel):
    kind: Literal['all_cargo'] = Field(...)

class TransferSubjectVariant2(WireModel):
    kind: Literal['credits'] = Field(...)
    quantity: int = Field(...)

class TransferSubjectVariant3(WireModel):
    id: str = Field(...)
    kind: Literal['item'] = Field(...)
    quantity: int | None = Field(None)

class TransferSubjectVariant4(WireModel):
    id: str = Field(...)
    kind: Literal['ship'] = Field(...)

class TransferSubjectVariant5(WireModel):
    id: str = Field(...)
    kind: Literal['module'] = Field(...)

class TransferSubjectVariant6(WireModel):
    items: tuple[TransferItem, ...] = Field(...)
    kind: Literal['items'] = Field(...)

class TransferSubject(RootModel['TransferSubjectVariant1 | TransferSubjectVariant2 | TransferSubjectVariant3 | TransferSubjectVariant4 | TransferSubjectVariant5 | TransferSubjectVariant6']):
    model_config = ConfigDict(frozen=True)

class V1BotConnectionState(RootModel["Literal['connected', 'disconnected']"]):
    model_config = ConfigDict(frozen=True)

class V1BotSummary(WireModel):
    bot_id: str = Field(..., alias='botId')
    connection: V1BotConnectionState = Field(...)
    name: str | None = Field(None)
    observed_at: datetime | None = Field(None, alias='observedAt')
    state_version: int = Field(..., alias='stateVersion')

class V1ErrorDetail(WireModel):
    code: str = Field(...)
    details: Any | None = Field(None)
    message: str = Field(...)
    retryable: bool = Field(...)

class V2GameStateCargoItem(WireModel):
    item_id: str | None = Field(None)
    item_name: str | None = Field(None)
    quantity: int | None = Field(None)
    size: int | None = Field(None)

class V2GameStateLocation(WireModel):
    connections: tuple[str, ...] | None = Field(None)
    docked_at: str | None = Field(None)
    empire: str | None = Field(None)
    in_transit: bool | None = Field(None)
    nearby_empire_npc_count: int | None = Field(None)
    nearby_empire_npcs: tuple[V2GameStateLocationNearbyEmpireNpcsItem, ...] | None = Field(None)
    nearby_pirate_count: int | None = Field(None)
    nearby_pirates: tuple[V2GameStateLocationNearbyPiratesItem, ...] | None = Field(None)
    nearby_player_count: int | None = Field(None)
    nearby_players: tuple[V2GameStateLocationNearbyPlayersItem, ...] | None = Field(None)
    offline_collapsed: int | None = Field(None)
    poi_id: str | None = Field(None)
    poi_name: str | None = Field(None)
    poi_type: str | None = Field(None)
    resources: tuple[V2GameStateLocationResourcesItem, ...] | None = Field(None)
    security_status: str | None = Field(None)
    system_id: str | None = Field(None)
    system_name: str | None = Field(None)
    transit_arrival_tick: int | None = Field(None)
    transit_bearing: float | None = Field(None)
    transit_dest_poi_id: str | None = Field(None)
    transit_dest_poi_name: str | None = Field(None)
    transit_dest_system_id: str | None = Field(None)
    transit_dest_system_name: str | None = Field(None)
    transit_ticks_elapsed: int | None = Field(None)
    transit_type: str | None = Field(None)
    transit_x: float | None = Field(None)
    transit_y: float | None = Field(None)
    unknown_signature: bool | None = Field(None)
    void_message: str | None = Field(None)

class V2GameStateLocationNearbyEmpireNpcsItem(WireModel):
    empire: str | None = Field(None)
    fleet_name: str | None = Field(None)
    in_combat: bool | None = Field(None)
    name: str | None = Field(None)
    npc_id: str | None = Field(None)
    role: str | None = Field(None)
    ship_class: str | None = Field(None)
    ship_name: str | None = Field(None)

class V2GameStateLocationNearbyPiratesItem(WireModel):
    hull: int | None = Field(None)
    is_boss: bool | None = Field(None)
    max_hull: int | None = Field(None)
    max_shield: int | None = Field(None)
    name: str | None = Field(None)
    pirate_id: str | None = Field(None)
    shield: int | None = Field(None)
    status: str | None = Field(None)
    tier: str | None = Field(None)

class V2GameStateLocationNearbyPlayersItem(WireModel):
    clan_tag: str | None = Field(None)
    faction_tag: str | None = Field(None)
    in_combat: bool | None = Field(None)
    offline: bool | None = Field(None)
    player_id: str | None = Field(None)
    ship_class: str | None = Field(None)
    ship_name: str | None = Field(None)
    username: str | None = Field(None)

class V2GameStateLocationResourcesItem(WireModel):
    item_id: str | None = Field(None)
    item_name: str | None = Field(None)
    remaining: int | None = Field(None)
    richness: int | None = Field(None)
    supported_power: int | None = Field(None)

class V2GameStateMissionsActiveItem(WireModel):
    accepted_at: datetime | None = Field(None)
    community: bool | None = Field(None)
    community_percent: float | None = Field(None)
    community_progress: FrozenDict[str, str] | None = Field(None)
    description: str | None = Field(None)
    difficulty: int | None = Field(None)
    expires_in_ticks: int | None = Field(None)
    giver: V2GameStateMissionsActiveItemGiver | None = Field(None)
    issuing_base: str | None = Field(None)
    issuing_base_id: str | None = Field(None)
    issuing_system_id: str | None = Field(None)
    issuing_system_name: str | None = Field(None)
    mission_id: str | None = Field(None)
    objectives: tuple[V2GameStateMissionsActiveItemObjectivesItem, ...] | None = Field(None)
    percent_complete: float | None = Field(None)
    rewards: V2GameStateMissionsActiveItemRewards | None = Field(None)
    template_id: str | None = Field(None)
    title: str | None = Field(None)
    type: str | None = Field(None)

class V2GameStateMissionsActiveItemGiver(WireModel):
    name: str | None = Field(None)
    title: str | None = Field(None)

class V2GameStateMissionsActiveItemObjectivesItem(WireModel):
    completed: bool | None = Field(None)
    current: int | None = Field(None)
    description: str | None = Field(None)
    eligible_players: tuple[str, ...] | None = Field(None)
    in_cargo: int | None = Field(None)
    in_storage: int | None = Field(None)
    item_id: str | None = Field(None)
    item_name: str | None = Field(None)
    participants: tuple[str, ...] | None = Field(None)
    required: int | None = Field(None)
    system_id: str | None = Field(None)
    system_name: str | None = Field(None)
    target_base: str | None = Field(None)
    target_base_name: str | None = Field(None)
    type: str | None = Field(None)

class V2GameStateMissionsActiveItemRewards(WireModel):
    credits: int | None = Field(None)
    items: FrozenDict[str, int] | None = Field(None)
    pirate_rep: int | None = Field(None)
    reputation: int | None = Field(None)
    skill_xp: FrozenDict[str, int] | None = Field(None)

class V2GameStateModulesItem(WireModel):
    ammo_type: str | None = Field(None)
    cpu_usage: int | None = Field(None)
    current_ammo: int | None = Field(None)
    loaded_ammo_id: str | None = Field(None)
    loaded_ammo_name: str | None = Field(None)
    magazine_size: int | None = Field(None)
    module_id: str | None = Field(None)
    name: str | None = Field(None)
    power_usage: int | None = Field(None)
    size: int | None = Field(None)
    slot: str | None = Field(None)
    stats: FrozenDict[str, Any] | None = Field(None)
    type: str | None = Field(None)
    type_id: str | None = Field(None)
    wear: float | None = Field(None)
    wear_status: str | None = Field(None)

class V2GameStatePlayer(WireModel):
    citizenships: tuple[str, ...] | None = Field(None)
    clan_tag: str | None = Field(None)
    credits: int | None = Field(None)
    empire: str | None = Field(None)
    faction_id: str | None = Field(None)
    faction_rank: str | None = Field(None)
    home_base: str | None = Field(None)
    home_poi: str | None = Field(None)
    home_system: str | None = Field(None)
    id: str | None = Field(None)
    is_cloaked: bool | None = Field(None)
    primary_color: str | None = Field(None)
    secondary_color: str | None = Field(None)
    standings: FrozenDict[str, V2GameStatePlayerStandingsValue] | None = Field(None)
    stats: FrozenDict[str, Any] | None = Field(None)
    status_message: str | None = Field(None)
    towing_wreck_id: str | None = Field(None)
    trading_restricted_until: datetime | None = Field(None)
    username: str | None = Field(None)

class V2GameStatePlayerStandingsValue(WireModel):
    baseline: int | None = Field(None)
    jailed_until: datetime | None = Field(None)
    outstanding_bounty: int | None = Field(None)
    reputation: int | None = Field(None)

class V2GameStateShip(WireModel):
    active_buffs: tuple[V2GameStateShipActiveBuffsItem, ...] | None = Field(None)
    armor: int | None = Field(None)
    armor_melt_pct: float | None = Field(None)
    armor_melt_ticks_remaining: int | None = Field(None)
    berths: V2GameStateShipBerths | None = Field(None)
    burn_damage_per_tick: int | None = Field(None)
    burn_source_id: str | None = Field(None)
    burn_ticks_remaining: int | None = Field(None)
    cargo_capacity: int | None = Field(None)
    cargo_used: int | None = Field(None)
    class_id: str | None = Field(None)
    class_name: str | None = Field(None)
    cpu_capacity: int | None = Field(None)
    cpu_used: int | None = Field(None)
    custom_name: str | None = Field(None)
    damage_penalty: float | None = Field(None)
    defense_slots: int | None = Field(None)
    disruption_ticks_remaining: int | None = Field(None)
    fuel: int | None = Field(None)
    gas_cargo_efficiency: int | None = Field(None)
    hull: int | None = Field(None)
    ice_cargo_efficiency: int | None = Field(None)
    id: str | None = Field(None)
    max_fuel: int | None = Field(None)
    max_hull: int | None = Field(None)
    max_shield: int | None = Field(None)
    name: str | None = Field(None)
    ore_cargo_efficiency: int | None = Field(None)
    power_capacity: int | None = Field(None)
    power_used: int | None = Field(None)
    shield: int | None = Field(None)
    shield_recharge: int | None = Field(None)
    speed: int | None = Field(None)
    speed_penalty: float | None = Field(None)
    utility_slots: int | None = Field(None)
    weapon_slots: int | None = Field(None)

class V2GameStateShipActiveBuffsItem(WireModel):
    amount: int | None = Field(None)
    expires_at: int | None = Field(None)
    item_id: str | None = Field(None)
    stat: str | None = Field(None)

class V2GameStateShipBerths(WireModel):
    business: V2GameStateShipBerthsBusiness = Field(...)
    economy: V2GameStateShipBerthsEconomy = Field(...)
    first: V2GameStateShipBerthsFirst = Field(...)

class V2GameStateShipBerthsBusiness(WireModel):
    free: int = Field(...)
    total: int = Field(...)

class V2GameStateShipBerthsEconomy(WireModel):
    free: int = Field(...)
    total: int = Field(...)

class V2GameStateShipBerthsFirst(WireModel):
    free: int = Field(...)
    total: int = Field(...)

class V2GameStateSkillsValue(WireModel):
    category: str | None = Field(None)
    level: int | None = Field(None)
    max_level: int | None = Field(None)
    name: str | None = Field(None)
    next_level_xp: int | None = Field(None)
    xp: int | None = Field(None)

class VirtualCraftOrder(WireModel):
    action: str = Field(...)
    credit_floor: int | None = Field(None, alias='creditFloor')
    do_forever: bool | None = Field(None, alias='doForever')
    enabled: bool | None = Field(None)
    facility_id: str | None = Field(None, alias='facilityId')
    filled: int | None = Field(None)
    id: str = Field(...)
    item_id: str | None = Field(None, alias='itemId')
    preset: str | None = Field(None)
    priority: float | None = Field(None)
    quantity: int = Field(...)
    recipe_id: str = Field(..., alias='recipeId')
    reservation_id: str | None = Field(None, alias='reservationId')
    reserved: int | None = Field(None)
    session_handles: tuple[str, ...] | None = Field(None, alias='sessionHandles')
    squad_id: str | None = Field(None, alias='squadId')
    station_id: str | None = Field(None, alias='stationId')
    status: str | None = Field(None)

class VirtualCraftOrderList(WireModel):
    orders: tuple[VirtualCraftOrder, ...] = Field(...)

class VirtualCraftOrderWrite(WireModel):
    orders: tuple[VirtualCraftOrder, ...] | None = Field(None)

class VirtualMarketOrder(WireModel):
    do_forever: bool | None = Field(None, alias='doForever')
    dumping: bool | None = Field(None)
    enabled: bool | None = Field(None)
    filled: int | None = Field(None)
    id: str = Field(...)
    internal_only: bool | None = Field(None, alias='internalOnly')
    item_id: str = Field(..., alias='itemId')
    price_each: int = Field(..., alias='priceEach')
    priority: float | None = Field(None)
    quantity: int = Field(...)
    reservation_id: str | None = Field(None, alias='reservationId')
    reserved: int | None = Field(None)
    side: str = Field(...)
    station_id: str = Field(..., alias='stationId')
    status: str | None = Field(None)
    tipping_point: int | None = Field(None, alias='tippingPoint')

class VirtualOrderList(WireModel):
    orders: tuple[VirtualMarketOrder, ...] = Field(...)

class VirtualOrderWrite(WireModel):
    orders: tuple[VirtualMarketOrder, ...] | None = Field(None)

class WaitingPassengerView(WireModel):
    bio: str = Field(...)
    citizen_id: str = Field(...)
    citizenship: str = Field(...)
    class_: str = Field(..., alias='class')
    destination: str = Field(...)
    destination_name: str = Field(...)
    destination_system: str | None = Field(None)
    estimated_fare: int | None = Field(None)
    name: str = Field(...)

class WorldState(WireModel):
    agent_sightings: FrozenDict[str, AgentSightingData] | FrozenDict[str, AgentSightingData] | None = Field(None, alias='agentSightings')
    chat_messages_by_session: FrozenDict[str, tuple[ChatMessageData, ...]] | FrozenDict[str, tuple[ChatMessageData, ...]] | None = Field(None, alias='chatMessagesBySession')
    facilities_by_poi: FrozenDict[str, PoiFacilitiesSnapshot] | FrozenDict[str, PoiFacilitiesSnapshot] | None = Field(None, alias='facilitiesByPoi')
    faction_by_session: FrozenDict[str, FactionSnapshotData] | FrozenDict[str, FactionSnapshotData] | None = Field(None, alias='factionBySession')
    faction_storage_by_faction_poi: FrozenDict[str, FrozenDict[str, FrozenDict[str, int]]] | FrozenDict[str, FrozenDict[str, FrozenDict[str, int]]] | None = Field(None, alias='factionStorageByFactionPoi')
    map: GalaxyMap | None = Field(None)
    owned_facilities_by_faction: FrozenDict[str, FacilityResponse] | FrozenDict[str, FacilityResponse] | None = Field(None, alias='ownedFacilitiesByFaction')
    owned_facilities_by_player: FrozenDict[str, FacilityResponse] | FrozenDict[str, FacilityResponse] | None = Field(None, alias='ownedFacilitiesByPlayer')
    resources: GalaxyResources | None = Field(None)
    salvage_by_poi: FrozenDict[str, SalvageData] | FrozenDict[str, SalvageData] | None = Field(None, alias='salvageByPoi')
    station_market_delta: StationMarketDelta | None = Field(None, alias='stationMarketDelta')
    station_markets: FrozenDict[str, StationMarketData] | FrozenDict[str, StationMarketData] | None = Field(None, alias='stationMarkets')
    station_passengers: FrozenDict[str, PassengerState] | FrozenDict[str, PassengerState] | None = Field(None, alias='stationPassengers')
    storage_by_player: FrozenDict[str, FrozenDict[str, FrozenDict[str, int]]] | FrozenDict[str, FrozenDict[str, FrozenDict[str, int]]] | None = Field(None, alias='storageByPlayer')
    updated_at_utc: datetime = Field(..., alias='updatedAtUtc')
    wildlife: GalaxyWildlife | None = Field(None)

Action.model_rebuild()
ActionOverrideRequest.model_rebuild()
ActionRunOutcome.model_rebuild()
ActionRunRequest.model_rebuild()
ActionRunResponse.model_rebuild()
ActiveRoute.model_rebuild()
ActorPassengerState.model_rebuild()
AgentSightingData.model_rebuild()
AmmoStats.model_rebuild()
BotConnectionState.model_rebuild()
BotList.model_rebuild()
BotState.model_rebuild()
BotSummary.model_rebuild()
BulkJobCancelResponse.model_rebuild()
BulkJobCancelResponseAction.model_rebuild()
BulkJobCancelResponseKind.model_rebuild()
BulkJobCancelResponseMode.model_rebuild()
BulkSummary.model_rebuild()
BuyRequest.model_rebuild()
CancelRequest.model_rebuild()
CatalogDumpItemsItem.model_rebuild()
ChatMessageData.model_rebuild()
CommissionEntry.model_rebuild()
CommissionShipRequest.model_rebuild()
CraftJobResponse.model_rebuild()
CraftJobResponseAction.model_rebuild()
CraftJobResponseKind.model_rebuild()
CraftJobResponseMode.model_rebuild()
CraftJobStatus.model_rebuild()
CraftRequest.model_rebuild()
CraftReservationResponse.model_rebuild()
CraftingQueueProjection.model_rebuild()
EmptyRequest.model_rebuild()
ErrorEnvelope.model_rebuild()
EscrowSummary.model_rebuild()
ExchangeOrder.model_rebuild()
FacilityAccessRequest.model_rebuild()
FacilityBrowseForSaleResponse.model_rebuild()
FacilityBrowseForSaleResponseAction.model_rebuild()
FacilityBuildResponse.model_rebuild()
FacilityBuildResponseAction.model_rebuild()
FacilityBuyListingResponse.model_rebuild()
FacilityBuyListingResponseAction.model_rebuild()
FacilityCancelListingResponse.model_rebuild()
FacilityCancelListingResponseAction.model_rebuild()
FacilityCategoryInfo.model_rebuild()
FacilityDefSummary.model_rebuild()
FacilityDefinition.model_rebuild()
FacilityDismantleMaterial.model_rebuild()
FacilityDismantleResponse.model_rebuild()
FacilityDismantleResponseAction.model_rebuild()
FacilityEntry.model_rebuild()
FacilityFactionBuildResponse.model_rebuild()
FacilityFactionBuildResponseAction.model_rebuild()
FacilityFactionEntry.model_rebuild()
FacilityFactionListResponse.model_rebuild()
FacilityFactionListResponseAction.model_rebuild()
FacilityFactionOwnedResponse.model_rebuild()
FacilityFactionOwnedResponseAction.model_rebuild()
FacilityFactionStorage.model_rebuild()
FacilityFactionUpgradeResponse.model_rebuild()
FacilityFactionUpgradeResponseAction.model_rebuild()
FacilityHelpResponse.model_rebuild()
FacilityHelpResponseAction.model_rebuild()
FacilityJobListResponse.model_rebuild()
FacilityJobListResponseAction.model_rebuild()
FacilityListForSaleResponse.model_rebuild()
FacilityListForSaleResponseAction.model_rebuild()
FacilityListResponse.model_rebuild()
FacilityListResponseAction.model_rebuild()
FacilityListingEntry.model_rebuild()
FacilityNameRequest.model_rebuild()
FacilityOutputPriceRequest.model_rebuild()
FacilityOwnedResponse.model_rebuild()
FacilityOwnedResponseAction.model_rebuild()
FacilityPersonalBuildResponse.model_rebuild()
FacilityPersonalBuildResponseAction.model_rebuild()
FacilityPersonalDecorateResponse.model_rebuild()
FacilityPersonalDecorateResponseAction.model_rebuild()
FacilityPersonalVisitResponse.model_rebuild()
FacilityPersonalVisitResponseAction.model_rebuild()
FacilityProduction.model_rebuild()
FacilityRecipeInfo.model_rebuild()
FacilityRentSummary.model_rebuild()
FacilityRepairMaterial.model_rebuild()
FacilityRepairResponse.model_rebuild()
FacilityRepairResponseAction.model_rebuild()
FacilityResponse.model_rebuild()
FacilityTransferResponse.model_rebuild()
FacilityTransferResponseAction.model_rebuild()
FacilityTypeDetailResponse.model_rebuild()
FacilityTypeDetailResponseAction.model_rebuild()
FacilityTypeDetailResponseKind.model_rebuild()
FacilityTypeDiscoveryResponse.model_rebuild()
FacilityTypeDiscoveryResponseAction.model_rebuild()
FacilityTypeDiscoveryResponseKind.model_rebuild()
FacilityTypeFilterInfo.model_rebuild()
FacilityTypeListResponse.model_rebuild()
FacilityTypeListResponseAction.model_rebuild()
FacilityTypeListResponseKind.model_rebuild()
FacilityTypePaginationInfo.model_rebuild()
FacilityTypeSummary.model_rebuild()
FacilityUpgradeEntry.model_rebuild()
FacilityUpgradeRequest.model_rebuild()
FacilityUpgradeResponse.model_rebuild()
FacilityUpgradeResponseAction.model_rebuild()
FacilityUpgradesResponse.model_rebuild()
FacilityUpgradesResponseAction.model_rebuild()
FactionMemberData.model_rebuild()
FactionOwnedFacilityEntry.model_rebuild()
FactionRoleData.model_rebuild()
FactionSnapshotData.model_rebuild()
FindRequest.model_rebuild()
FleetEntry.model_rebuild()
FleetSnapshot.model_rebuild()
GalaxyCatalog.model_rebuild()
GalaxyMap.model_rebuild()
GalaxyResources.model_rebuild()
GalaxyWildlife.model_rebuild()
GoTarget.model_rebuild()
InventoryClaim.model_rebuild()
Item.model_rebuild()
ItemEffect.model_rebuild()
ItemQuantity.model_rebuild()
JobCancelResponse.model_rebuild()
JobCancelResponseAction.model_rebuild()
JobCancelResponseKind.model_rebuild()
JobCancelResult.model_rebuild()
JobReorderResponse.model_rebuild()
JobReorderResponseAction.model_rebuild()
JobView.model_rebuild()
LaneOwner.model_rebuild()
LootedModule.model_rebuild()
MarketMovement.model_rebuild()
MarketMovementHealth.model_rebuild()
MarketMovementList.model_rebuild()
MarketMovementReserveRequest.model_rebuild()
MarketMovementReserveResponse.model_rebuild()
MarketMovementStatus.model_rebuild()
MarketMovementTransitionRequest.model_rebuild()
Meta.model_rebuild()
MissionData.model_rebuild()
MissionDialogInfo.model_rebuild()
MissionGiverInfo.model_rebuild()
MissionInfo.model_rebuild()
MissionRewardsInfo.model_rebuild()
Module.model_rebuild()
NearbyPlayer.model_rebuild()
ObjectiveInfo.model_rebuild()
ObservedPlayer.model_rebuild()
OrderLevel.model_rebuild()
OverrideResponse.model_rebuild()
OwnedFacilityEntry.model_rebuild()
OwnedShipDetail.model_rebuild()
PackageJobResponse.model_rebuild()
PackageJobResponseAction.model_rebuild()
PackageJobResponseKind.model_rebuild()
PassengerBerthView.model_rebuild()
PassengerState.model_rebuild()
PassengerView.model_rebuild()
PoiFacilitiesSnapshot.model_rebuild()
QueueLane.model_rebuild()
QueueResponse.model_rebuild()
QueueSnapshot.model_rebuild()
Recipe.model_rebuild()
RecipeInput.model_rebuild()
RecipeOutput.model_rebuild()
RecycleRequest.model_rebuild()
RegisterBotRequest.model_rebuild()
RegisterBotResponse.model_rebuild()
ReservationRequest.model_rebuild()
ReservationResponse.model_rebuild()
ReservationResult.model_rebuild()
ReservationUse.model_rebuild()
RouteBatchRequest.model_rebuild()
RouteBatchResponse.model_rebuild()
RouteQuery.model_rebuild()
RouteSelection.model_rebuild()
RuntimeGalaxyKnownPoiInfoDto.model_rebuild()
RuntimeGalaxyPoiInfoDto.model_rebuild()
RuntimeGalaxySystemInfoDto.model_rebuild()
RuntimeInventoryClaimHealthDto.model_rebuild()
RuntimePoiResourceInfoDto.model_rebuild()
RuntimeVirtualOrderReservationResultDto.model_rebuild()
RuntimeWildlifeCreatureDto.model_rebuild()
RuntimeWildlifePoiDto.model_rebuild()
RuntimeWildlifeSpeciesDto.model_rebuild()
RuntimeWildlifeSystemDto.model_rebuild()
SalvageData.model_rebuild()
SayRequest.model_rebuild()
ScriptErrorKind.model_rebuild()
ScriptErrorKindDto.model_rebuild()
ScriptExecution.model_rebuild()
ScriptExecutionDto.model_rebuild()
ScriptExecutionOutcome.model_rebuild()
ScriptOutcomeDto.model_rebuild()
ScriptOverrideRequest.model_rebuild()
ScriptRunOutcome.model_rebuild()
ScriptRunRequest.model_rebuild()
ScriptRunResponse.model_rebuild()
SellRequest.model_rebuild()
ServiceTransferRequest.model_rebuild()
SetAccessResponse.model_rebuild()
SetAccessResponseAction.model_rebuild()
SetFacilityDescriptionResponse.model_rebuild()
SetFacilityDescriptionResponseAction.model_rebuild()
SetFacilityNameResponse.model_rebuild()
SetFacilityNameResponseAction.model_rebuild()
SetOutputPriceResponse.model_rebuild()
SetOutputPriceResponseAction.model_rebuild()
ShipCargoItem.model_rebuild()
ShipClass.model_rebuild()
ShipClassInherentCapabilitiesItem.model_rebuild()
SkillDefinition.model_rebuild()
SpaceLootInfo.model_rebuild()
StateResponse.model_rebuild()
StateVersions.model_rebuild()
StationConstructionEntry.model_rebuild()
StationConstructionMaterial.model_rebuild()
StationConstructionResponse.model_rebuild()
StationLifeSupportInput.model_rebuild()
StationLifeSupportStatus.model_rebuild()
StationMarketData.model_rebuild()
StationMarketDelta.model_rebuild()
StationMarkets.model_rebuild()
StationPowerInput.model_rebuild()
StationPowerStatus.model_rebuild()
StorageByOwner.model_rebuild()
TradeItem.model_rebuild()
TradeOfferRequest.model_rebuild()
TransferEndpoint.model_rebuild()
TransferItem.model_rebuild()
TransferRequest.model_rebuild()
TransferSubject.model_rebuild()
V1BotConnectionState.model_rebuild()
V1BotSummary.model_rebuild()
V1ErrorDetail.model_rebuild()
V2GameStateCargoItem.model_rebuild()
V2GameStateLocation.model_rebuild()
V2GameStateLocationNearbyEmpireNpcsItem.model_rebuild()
V2GameStateLocationNearbyPiratesItem.model_rebuild()
V2GameStateLocationNearbyPlayersItem.model_rebuild()
V2GameStateLocationResourcesItem.model_rebuild()
V2GameStateMissionsActiveItem.model_rebuild()
V2GameStateMissionsActiveItemGiver.model_rebuild()
V2GameStateMissionsActiveItemObjectivesItem.model_rebuild()
V2GameStateMissionsActiveItemRewards.model_rebuild()
V2GameStateModulesItem.model_rebuild()
V2GameStatePlayer.model_rebuild()
V2GameStatePlayerStandingsValue.model_rebuild()
V2GameStateShip.model_rebuild()
V2GameStateShipActiveBuffsItem.model_rebuild()
V2GameStateShipBerths.model_rebuild()
V2GameStateShipBerthsBusiness.model_rebuild()
V2GameStateShipBerthsEconomy.model_rebuild()
V2GameStateShipBerthsFirst.model_rebuild()
V2GameStateSkillsValue.model_rebuild()
VirtualCraftOrder.model_rebuild()
VirtualCraftOrderList.model_rebuild()
VirtualCraftOrderWrite.model_rebuild()
VirtualMarketOrder.model_rebuild()
VirtualOrderList.model_rebuild()
VirtualOrderWrite.model_rebuild()
WaitingPassengerView.model_rebuild()
WorldState.model_rebuild()
