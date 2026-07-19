import assert from "node:assert/strict";
import test from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import SessionsPanel, { SessionCard, type SessionState } from "../src/client/SessionsPanel.js";

test("sessions panel renders typed route and running-script state", () => {
  const session: SessionState = {
    sessionHandle: "ada",
    botId: "bot-1",
    username: "Ada",
    connected: true,
    credits: 100,
    fuel: 8,
    maxFuel: 10,
    hull: 9,
    maxHull: 10,
    cargoUsed: 1,
    cargoCapacity: 5,
    passengerBerths: 0,
    cargo: {},
    passengersAboard: [],
    inBattle: false,
    combatStance: null,
    combatTarget: null,
    battleStartedAt: null,
    runningScript: { script: "go beta;", currentLine: 1, isRunning: true, frameKind: "main", frameName: null },
    location: {
      system: "alpha",
      poi: "alpha-base",
      inTransit: false,
      transitDestSystem: null,
      transitDestPoi: null,
      activeRouteDestSystem: "gamma",
      activeRouteDestPoi: null,
      activeRouteHops: ["beta", "gamma"],
    },
  };
  const markup = renderToStaticMarkup(
    React.createElement(SessionCard, {
      session,
      systemEmpires: {},
      onHaltScript: () => undefined,
    }),
  );
  assert.match(markup, /Ada/);
  assert.match(markup, /alpha-base/);
  assert.match(markup, /gamma/);
  assert.match(markup, /Halt running script/);
});

test("sessions panel exposes new bot registration", () => {
  const markup = renderToStaticMarkup(
    React.createElement(SessionsPanel, {
      sessions: [],
      systemEmpires: {},
      onHaltScript: () => undefined,
      onRegistered: async () => undefined,
      onOpenJob: () => undefined,
    }),
  );
  assert.match(markup, /Register a new bot/);
});
