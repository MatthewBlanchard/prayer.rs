from __future__ import annotations

import asyncio
import json

import httpx
import pytest

from prayer_sdk import (
    LaneBusyError,
    Prayer,
    PrayerCompatibilityError,
    PrayerConnectionError,
    PrayerTimeoutError,
)
from prayer_sdk.actions import ACTION_TYPES, dock, go, mine, refuel, scan, undock, wait

META = {"apiVersion": "1.0", "serverVersion": "test", "actionSchemaVersion": 5,
        "capabilities": []}


def response(body: object = None, status: int = 200) -> httpx.Response:
    return httpx.Response(status, json=body, headers={"content-type": "application/json"})


@pytest.mark.asyncio
async def test_connect_negotiates_and_actions_have_exact_wire_shape() -> None:
    client = httpx.AsyncClient(transport=httpx.MockTransport(lambda request: response(META)))
    prayer = await Prayer.connect("http://test", client=client)
    assert prayer.meta and prayer.meta.api_version == "1.0"
    assert [dict(undock()), _plain(go(poi="sol")), dict(dock())] == [
        {"type": "undock"},
        {"type": "go", "request": {"destination": {"kind": "poi", "value": "sol"}}},
        {"type": "dock"},
    ]
    assert _plain(wait(3)) == {"type": "wait", "request": {"ticks": 3}}
    assert _plain(mine()) == {"type": "mine", "request": {"resource": None}}
    assert _plain(scan()) == {"type": "scan", "request": {"target": None}}
    assert _plain(refuel(target="ship"))["request"] == {
        "target": "ship", "quantity": None, "item": None}
    assert len(ACTION_TYPES) == len(set(ACTION_TYPES))
    await prayer.aclose()
    assert not client.is_closed
    await client.aclose()


@pytest.mark.asyncio
async def test_routes_use_bulk_endpoint_and_safe_default() -> None:
    requests: list[httpx.Request] = []
    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path.endswith("/meta"): return response(META)
        requests.append(request)
        body = json.loads(request.content)
        return response({"routes": [{"cost": 2} for _ in body["routes"]]})
    prayer = await Prayer.connect("http://test", client=httpx.AsyncClient(
        transport=httpx.MockTransport(handler)))
    assert (await prayer.route("a", "b"))["cost"] == 2
    assert len(await prayer.routes(({"from": "a", "to": "b"}, {"from": "b", "to": "a"}), safe=False)) == 2
    assert json.loads(requests[0].content) == {"routes": [{"from": "a", "to": "b"}], "safe": True}


@pytest.mark.asyncio
async def test_structured_error_and_headers() -> None:
    observed: httpx.Request | None = None
    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal observed
        observed = request
        if request.url.path.endswith("/meta"): return response(META)
        return response({"error": {"code": "lane_busy", "message": "busy",
                         "retryable": False, "details": {"retryAfterMs": 9}},
                         "requestId": "req"}, 409)
    prayer = await Prayer.connect("http://test", token="secret", headers={"x-client": "test"},
                                  client=httpx.AsyncClient(transport=httpx.MockTransport(handler)))
    with pytest.raises(LaneBusyError) as caught:
        await prayer.bot("miner")
    assert caught.value.request_id == "req" and caught.value.retry_after_ms == 9
    assert observed and observed.headers["authorization"] == "Bearer secret"
    assert observed.headers["x-client"] == "test"


@pytest.mark.asyncio
async def test_incompatible_major_closes_owned_client() -> None:
    client = httpx.AsyncClient(transport=httpx.MockTransport(
        lambda request: response({**META, "apiVersion": "2.0"})))
    with pytest.raises(PrayerCompatibilityError):
        await Prayer.connect("http://test", client=client, close_client=True)
    assert client.is_closed


@pytest.mark.asyncio
async def test_timeout_connection_and_cancellation_are_distinct() -> None:
    class Timeout(httpx.AsyncBaseTransport):
        async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
            raise httpx.ReadTimeout("late", request=request)
    with pytest.raises(PrayerTimeoutError):
        await Prayer.connect("http://test", client=httpx.AsyncClient(transport=Timeout()))

    class Offline(httpx.AsyncBaseTransport):
        async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
            raise httpx.ConnectError("offline", request=request)
    with pytest.raises(PrayerConnectionError):
        await Prayer.connect("http://test", client=httpx.AsyncClient(transport=Offline()))

    async def cancelled(request: httpx.Request) -> httpx.Response:
        raise asyncio.CancelledError
    with pytest.raises(asyncio.CancelledError):
        await Prayer.connect("http://test", client=httpx.AsyncClient(
            transport=httpx.MockTransport(cancelled)))


@pytest.mark.asyncio
async def test_runs_idempotency_polling_and_explicit_cancel() -> None:
    requests: list[httpx.Request] = []
    polls = 0
    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal polls
        if request.url.path.endswith("/meta"): return response(META)
        if request.url.path.endswith("/bots/miner"):
            return response({"botId": "bot", "name": "miner"})
        requests.append(request)
        if request.method == "POST" and request.url.path.endswith("action-runs"):
            return response({"runId": "run", "botId": "bot", "status": "running",
                             "runVersion": 1, "prayerlang": "wait 1;"}, 202)
        if request.method == "GET":
            polls += 1
            return response({"runId": "run", "botId": "bot", "status": "succeeded",
                             "runVersion": 2, "prayerlang": "wait 1;", "outcome": {"status": "ok"}})
        return response({"runId": "run", "botId": "bot", "status": "cancelled",
                         "runVersion": 2, "prayerlang": "wait 1;", "outcome": {"reason": "stop"}})
    prayer = await Prayer.connect("http://test", client=httpx.AsyncClient(
        transport=httpx.MockTransport(handler)))
    bot = await prayer.bot("miner")
    with pytest.raises(ValueError): await bot.start_actions(wait(1), idempotency_key=" ")
    run = await bot.start_actions(wait(1), idempotency_key=" durable ")
    assert run.idempotency_key == "durable"
    assert requests[-1].headers["idempotency-key"] == "durable"
    assert json.loads(requests[-1].content) == {"actions": [{"type": "wait", "request": {"ticks": 1}}]}
    assert (await run.wait(poll_interval=0))["status"] == "succeeded" and polls == 1


@pytest.mark.asyncio
async def test_state_cache() -> None:
    calls = 0
    queries: list[dict[str, str]] = []
    versions = {name: 1 for name in ("fleet", "world", "map", "resources", "wildlife",
                "markets", "storage", "facilities", "observations", "communications", "factions")}
    versions["catalog"] = "v1"
    world = {"map": {}, "resources": {}, "wildlife": {}, "stationMarkets": {
        "old": {"observed": 1}, "remove": {"observed": 1}}, "storageByPlayer": {},
        "facilitiesByPoi": {}, "updatedAtUtc": "2026-01-01T00:00:00Z"}
    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        if request.url.path.endswith("/meta"): return response(META)
        calls += 1
        queries.append(dict(request.url.params))
        if calls == 1:
            return response({"versions": versions, "fleet": {"bots": {}}, "world": world,
                             "catalog": {"itemsById": {}}})
        return response({"versions": {**versions, "world": 2, "markets": 2}, "fleet": None,
                         "world": {"stationMarkets": None, "stationMarketDelta": {
                             "baseVersion": 1, "upsert": {"new": {"observed": 2}},
                             "remove": ["remove"]}}, "catalog": None})
    prayer = await Prayer.connect("http://test", client=httpx.AsyncClient(
        transport=httpx.MockTransport(handler)))
    first = await prayer.state()
    second = await prayer.state()
    assert queries[1]["fleet_version"] == "1" and queries[1]["catalog_version"] == "v1"
    assert "new" in second["world"]["stationMarkets"]
    assert "remove" not in second["world"]["stationMarkets"]
    with pytest.raises(TypeError): second["versions"]["world"] = 9
    assert "remove" in first["world"]["stationMarkets"]


def _plain(value: object) -> object:
    if isinstance(value, dict) or hasattr(value, "items"):
        return {key: _plain(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)): return [_plain(item) for item in value]
    return value
