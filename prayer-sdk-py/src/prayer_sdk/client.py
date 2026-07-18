"""Long-lived high-level Prayer client."""

from __future__ import annotations

import asyncio
import inspect
import uuid
from collections.abc import Awaitable, Callable, Mapping
from types import MappingProxyType
from typing import Any, Generic, TypeVar

import httpx

from .actions import Action
from .errors import PrayerCompatibilityError
from .generated.api import PrayerApi
from .generated.models import Meta
from .transport import RequestOptions, Transport

T = TypeVar("T", bound=Mapping[str, Any])
StatusCallback = Callable[[T], None | Awaitable[None]]


class Prayer:
    def __init__(self, transport: Transport) -> None:
        self._transport = transport
        self.advanced = Advanced(PrayerApi(transport))
        self.meta: Meta | None = None
        self._state_cache: Mapping[str, Any] | None = None
        self._state_lock = asyncio.Lock()

    @classmethod
    async def connect(cls, base_url: str, *, token: str | None = None,
                      headers: Mapping[str, str] | None = None,
                      timeout: float | httpx.Timeout = 30.0,
                      client: httpx.AsyncClient | None = None,
                      close_client: bool | None = None) -> Prayer:
        prayer = cls(Transport(base_url, token=token, headers=headers, timeout=timeout,
                               client=client, close_client=close_client))
        try:
            raw = await prayer.advanced.api.get_meta()
            prayer.meta = Meta.model_validate(raw)
            if prayer.meta.api_version.split(".", 1)[0] != "1":
                raise PrayerCompatibilityError(f"Unsupported Prayer API {prayer.meta.api_version}")
            return prayer
        except BaseException:
            await prayer.aclose()
            raise

    async def __aenter__(self) -> Prayer: return self
    async def __aexit__(self, *_: object) -> None: await self.aclose()
    async def aclose(self) -> None: await self._transport.aclose()

    async def bots(self, *, options: RequestOptions | None = None) -> tuple[Mapping[str, Any], ...]:
        raw = await self.advanced.api.list_bots(options=options)
        values = raw.get("bots", raw) if isinstance(raw, dict) else raw
        return tuple(_freeze(item) for item in values)

    async def bot(self, selector: str, *, options: RequestOptions | None = None) -> Bot:
        return Bot(self, _freeze(await self.advanced.api.get_bot(selector, options=options)))

    async def routes(self, routes: list[Mapping[str, Any]] | tuple[Mapping[str, Any], ...],
                     *, safe: bool = True, options: RequestOptions | None = None) -> tuple[Any, ...]:
        raw = await self.advanced.api.select_routes({"routes": list(routes), "safe": safe}, options=options)
        return tuple(_freeze(item) if item is not None else None for item in raw["routes"])

    async def route(self, from_: str, to: str, *, safe: bool = True,
                    options: RequestOptions | None = None) -> Any:
        values = await self.routes(({"from": from_, "to": to},), safe=safe, options=options)
        return values[0] if values else None

    async def state(self, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        async with self._state_lock:
            versions = self._state_cache.get("versions") if self._state_cache else None
            query = _version_query(versions) if isinstance(versions, Mapping) else {}
            response = await self.advanced.api.get_state(**query, options=options)
            try:
                snapshot = _merge_state(self._state_cache, response)
            except PrayerCompatibilityError:
                if not response.get("world", {}).get("stationMarketDelta"):
                    raise
                recovered = await self.advanced.api.get_state(options=options)
                snapshot = _merge_state(None, recovered)
            self._state_cache = _freeze(snapshot)
            return self._state_cache


class Advanced:
    __slots__ = ("api",)
    def __init__(self, api: PrayerApi) -> None: self.api = api


class Bot:
    def __init__(self, prayer: Prayer, summary: Mapping[str, Any]) -> None:
        self._prayer = prayer
        self.summary = summary

    @property
    def id(self) -> str: return str(self.summary.get("botId", self.summary.get("bot_id")))

    async def state(self, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        fleet = (await self._prayer.state(options=options))["fleet"]
        bots = fleet["bots"]
        entry = bots.get(self.id)
        if entry is None:
            entry = next((item for item in bots.values() if item.get("id") == self.id), None)
        if entry is None:
            raise PrayerCompatibilityError(f"Bot {self.id} is missing from the aggregate state snapshot")
        return entry

    async def queue(self, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        return _freeze(await self._prayer.advanced.api.get_bot_queue(self.id, options=options))
    async def normal_queue(self, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        return _freeze(await self._prayer.advanced.api.get_bot_normal_queue(self.id, options=options))
    async def override_queue(self, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        return _freeze(await self._prayer.advanced.api.get_bot_override_queue(self.id, options=options))
    async def halt(self, reason: str | None = None, *, options: RequestOptions | None = None) -> None:
        await self._prayer.advanced.api.halt_bot(self.id, {"reason": reason} if reason else None, options=options)

    async def start_actions(self, actions: Action | list[Action] | tuple[Action, ...], *,
                            idempotency_key: str | None = None,
                            options: RequestOptions | None = None) -> ActionRun:
        key = _idempotency_key(idempotency_key)
        items = list(actions) if isinstance(actions, (list, tuple)) else [actions]
        raw = await self._prayer.advanced.api.start_action_run(
            self.id, {"actions": items}, idempotency_key=key, options=options)
        return ActionRun(self._prayer.advanced.api, _freeze(raw), key)

    start = start_actions

    async def action_run(self, run_id: str, *, options: RequestOptions | None = None) -> ActionRun:
        raw = await self._prayer.advanced.api.get_action_run(self.id, run_id, options=options)
        return ActionRun(self._prayer.advanced.api, _freeze(raw))

    async def execute(self, actions: Action | list[Action] | tuple[Action, ...], *,
                      idempotency_key: str | None = None, poll_interval: float = 0.25,
                      options: RequestOptions | None = None) -> Mapping[str, Any]:
        run = await self.start_actions(actions, idempotency_key=idempotency_key, options=options)
        return await run.wait(poll_interval=poll_interval, options=options)

    async def execute_action_override(self, actions: Action | list[Action] | tuple[Action, ...], *,
                                      return_to_origin: bool = False,
                                      options: RequestOptions | None = None) -> None:
        items = list(actions) if isinstance(actions, (list, tuple)) else [actions]
        await self._prayer.advanced.api.execute_action_override(
            self.id, {"actions": items, "returnToOrigin": return_to_origin}, options=options)

    async def execute_script_override(self, script: str, *, return_to_origin: bool = False,
                                      options: RequestOptions | None = None) -> None:
        await self._prayer.advanced.api.execute_script_override(
            self.id, {"script": script, "returnToOrigin": return_to_origin}, options=options)

    async def start_script(self, script: str, *, idempotency_key: str | None = None,
                           options: RequestOptions | None = None) -> ScriptRun:
        key = _idempotency_key(idempotency_key)
        raw = await self._prayer.advanced.api.start_script_run(
            self.id, {"script": script}, idempotency_key=key, options=options)
        return ScriptRun(self._prayer.advanced.api, _freeze(raw), key)

    async def script_run(self, run_id: str, *, options: RequestOptions | None = None) -> ScriptRun:
        raw = await self._prayer.advanced.api.get_script_run(self.id, run_id, options=options)
        return ScriptRun(self._prayer.advanced.api, _freeze(raw))


class Run(Generic[T]):
    def __init__(self, api: PrayerApi, snapshot: T, idempotency_key: str | None = None) -> None:
        self._api = api
        self._current = snapshot
        self.idempotency_key = idempotency_key
    @property
    def id(self) -> str: return str(self._current.get("runId", self._current.get("run_id")))
    @property
    def prayerlang(self) -> str: return str(self._current["prayerlang"])
    @property
    def snapshot(self) -> T: return self._current
    @property
    def is_terminal(self) -> bool: return self._current["status"] != "running"
    @property
    def succeeded(self) -> bool: return self._current["status"] == "succeeded"
    @property
    def cancellation_kind(self) -> str | None:
        status = self._current["status"]
        return status if status in {"cancelled", "halted"} else None
    @property
    def error_message(self) -> str | None:
        if self._current["status"] in {"running", "succeeded"}: return None
        outcome = self._current.get("outcome", {})
        return outcome.get("message", outcome.get("reason"))
    async def wait(self, *, poll_interval: float = 0.25, on_status: StatusCallback[T] | None = None,
                   options: RequestOptions | None = None) -> T:
        while not self.is_terminal:
            await asyncio.sleep(poll_interval)
            await self.status(options=options)
            if on_status:
                result = on_status(self._current)
                if inspect.isawaitable(result): await result
        return self._current


class ActionRun(Run[Mapping[str, Any]]):
    async def status(self, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        self._current = _freeze(await self._api.get_action_run(
            self._current.get("botId", self._current.get("bot_id")), self.id, options=options))
        return self._current
    async def cancel(self, reason: str | None = None, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        self._current = _freeze(await self._api.cancel_action_run(
            self._current.get("botId", self._current.get("bot_id")), self.id,
            {"reason": reason} if reason else None, options=options))
        return self._current


class ScriptRun(Run[Mapping[str, Any]]):
    async def status(self, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        self._current = _freeze(await self._api.get_script_run(
            self._current.get("botId", self._current.get("bot_id")), self.id, options=options))
        return self._current
    async def cancel(self, reason: str | None = None, *, options: RequestOptions | None = None) -> Mapping[str, Any]:
        self._current = _freeze(await self._api.cancel_script_run(
            self._current.get("botId", self._current.get("bot_id")), self.id,
            {"reason": reason} if reason else None, options=options))
        return self._current


def _idempotency_key(value: str | None) -> str:
    if value is None: return str(uuid.uuid4())
    key = value.strip()
    if not key: raise ValueError("idempotency_key must not be blank")
    if len(key) > 255: raise ValueError("idempotency_key must be at most 255 characters")
    return key


def _freeze(value: Any) -> Any:
    if isinstance(value, Mapping): return MappingProxyType({k: _freeze(v) for k, v in value.items()})
    if isinstance(value, list): return tuple(_freeze(v) for v in value)
    return value


def _version_query(versions: Mapping[str, Any]) -> dict[str, Any]:
    return {f"{name}_version": versions[name] for name in (
        "fleet", "world", "map", "resources", "wildlife", "markets", "storage", "facilities",
        "observations", "communications", "factions", "catalog") if name in versions}


def _merge_state(previous: Mapping[str, Any] | None, response: Mapping[str, Any]) -> dict[str, Any]:
    fleet = response.get("fleet") or (previous and previous.get("fleet"))
    catalog = response.get("catalog") or (previous and previous.get("catalog"))
    update = response.get("world")
    prior_world = previous.get("world") if previous else None
    world = dict(prior_world or {})
    if update:
        world.update({k: v for k, v in update.items() if k != "stationMarketDelta" and v is not None})
        delta = update.get("stationMarketDelta")
        if delta and update.get("stationMarkets") is None:
            if not prior_world:
                raise PrayerCompatibilityError("Prayer API returned a market delta without a cached base snapshot")
            prior_version = previous["versions"].get("markets")
            if delta["baseVersion"] != prior_version:
                raise PrayerCompatibilityError("Prayer API market delta base does not match cached markets")
            markets = dict(prior_world["stationMarkets"])
            markets.update(delta.get("upsert", {}))
            for station in delta.get("remove", ()): markets.pop(station, None)
            world["stationMarkets"] = markets
    if fleet is None or not world or catalog is None:
        raise PrayerCompatibilityError("Prayer API returned an incomplete initial state snapshot")
    return {"versions": response["versions"], "fleet": fleet, "world": world, "catalog": catalog}
