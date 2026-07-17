import assert from "node:assert/strict";
import test from "node:test";
import { allocateDistinct, effectiveVisit, orderedPois, rankCandidates, strongholdDistances } from "./planner.js";

const system = (id: string, patch: Record<string, unknown> = {}) =>
  ({ id, connections: [], isStronghold: false, firstEnteredUnix: 1, lastEnteredUnix: 10, poisComplete: true, pois: [], ...patch }) as never;
const poi = (id: string, systemId: string, firstVisitedUnix: number | null, lastVisitedUnix: number | null) =>
  ({ id, systemId, firstVisitedUnix, lastVisitedUnix }) as never;
const route = (id: string, totalJumps: number) => ({ toSystem: id, totalJumps }) as never;

test("computes nearest multi-source stronghold distances across cycles and disconnected components", () => {
  const systems = [
    system("a", { isStronghold: true, connections: ["b"] }),
    system("b", { connections: ["a", "c"] }),
    system("c", { connections: ["b", "d"] }),
    system("d", { isStronghold: true, connections: ["c"] }),
    system("x"),
  ];
  assert.deepEqual(Object.fromEntries(strongholdDistances(systems)), { a: 0, d: 0, b: 1, c: 1 });
});

test("prefers fewest-hop unexplored and excludes the exact stronghold boundary", () => {
  const systems = [
    system("s", { isStronghold: true, connections: ["a"] }),
    system("a", { connections: ["s", "b"] }),
    system("b", { connections: ["a", "c"], firstEnteredUnix: null }),
    system("c", { connections: ["b"], firstEnteredUnix: null }),
  ];
  const routes = new Map([
    ["a", route("a", 1)],
    ["b", route("b", 1)],
    ["c", route("c", 2)],
  ]);
  assert.deepEqual(
    rankCandidates(systems, [], routes, 1).map((x) => x.targetId),
    ["b", "c"],
  );
  assert.deepEqual(
    rankCandidates(systems, [], routes, 2).map((x) => x.targetId),
    ["c"],
  );
  assert.deepEqual(
    rankCandidates(systems, [], routes, 0).map((x) => x.targetId),
    ["b", "c"],
  );
});

test("applies manual blacklist and unblacklist overrides", () => {
  const systems = [
    system("s", { isStronghold: true, connections: ["a"] }),
    system("a", { connections: ["s", "b"], firstEnteredUnix: null }),
    system("b", { connections: ["a"], firstEnteredUnix: null }),
  ];
  const routes = new Map([
    ["s", route("s", 1)],
    ["a", route("a", 1)],
    ["b", route("b", 2)],
  ]);
  assert.deepEqual(
    rankCandidates(systems, [], routes, 1, new Set(["b"]), new Set(["a"])).map((x) => x.targetId),
    ["a"],
  );
});

test("falls back to oldest effective system or POI visit and ignores unreachable systems", () => {
  const systems = [system("a", { lastEnteredUnix: 20, pois: [{ id: "pa" }] }), system("b", { lastEnteredUnix: 5 }), system("x", { lastEnteredUnix: null })];
  const pois = [poi("pa", "a", 1, 2)];
  const ranked = rankCandidates(
    systems,
    pois,
    new Map([
      ["a", route("a", 1)],
      ["b", route("b", 4)],
    ]),
    3,
  );
  assert.deepEqual(
    ranked.map((x) => x.targetId),
    ["pa"],
  );
  assert.equal(effectiveVisit(system("z", { lastEnteredUnix: null }), []), Number.NEGATIVE_INFINITY);
});

test("allocates distinct targets when possible and degrades deterministically", () => {
  const a = { system: system("a"), route: route("a", 1), targetKind: "system" as const, targetId: "a", priority: 1, timestamp: 0 };
  const b = { system: system("b"), route: route("b", 2), targetKind: "system" as const, targetId: "b", priority: 1, timestamp: 0 };
  assert.deepEqual(
    Object.fromEntries(
      [
        ...allocateDistinct([
          { botId: "2", candidates: [a, b] },
          { botId: "1", candidates: [a, b] },
        ]),
      ].map(([id, value]) => [id, value.system.id]),
    ),
    { "1": "a", "2": "b" },
  );
  assert.equal(
    allocateDistinct([
      { botId: "1", candidates: [a] },
      { botId: "2", candidates: [a] },
    ]).has("2"),
    false,
  );
});

test("ties unvisited POIs with unvisited systems by distance, then prioritizes surveys and stale POIs", () => {
  const systems = [
    system("visited", { pois: [{ id: "new-poi" }, { id: "old-poi" }], lastSurveyedUnix: null }),
    system("new-system", { firstEnteredUnix: null }),
  ];
  const routes = new Map([
    ["visited", route("visited", 2)],
    ["new-system", route("new-system", 1)],
  ]);
  const unvisited = poi("new-poi", "visited", null, null);
  const old = { ...poi("old-poi", "visited", 1, 5), lastObservedUnix: null } as never;
  assert.deepEqual(
    rankCandidates(systems, [unvisited, old], routes, 0, new Set(), new Set(), true).map((x) => x.targetId),
    ["new-system", "new-poi"],
  );
  assert.deepEqual(
    rankCandidates(systems, [old], routes, 0, new Set(), new Set(), false).map((x) => x.targetId),
    ["new-system"],
  );
  const entered = systems.map((item) => (item.id === "new-system" ? system("new-system", { lastSurveyedUnix: 1 }) : item));
  assert.deepEqual(
    rankCandidates(entered, [old], routes, 0, new Set(), new Set(), true).map((x) => x.targetId),
    ["visited"],
  );
  const observed = { ...old, lastObservedUnix: 10 } as never;
  assert.deepEqual(
    rankCandidates(entered, [observed], routes, 0).map((x) => x.targetId),
    ["old-poi"],
  );
});

test("orders unvisited then stale POIs and never returns seen POIs", () => {
  const target = system("s", { pois: [{ id: "old" }, { id: "new" }, { id: "fresh" }] });
  assert.deepEqual(
    orderedPois(target, [poi("fresh", "s", 1, 20), poi("old", "s", 1, 2), poi("new", "s", null, null)], new Set(["old"])).map((x) => x.id),
    ["new", "fresh"],
  );
});
