import assert from "node:assert/strict";
import test from "node:test";
import http, { type Server } from "node:http";
import { once } from "node:events";
import express from "express";
import { registerPrayerProxy } from "./prayer-proxy.js";

async function listen(server: Server): Promise<string> {
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  return `http://127.0.0.1:${address.port}`;
}
async function close(server: Server): Promise<void> {
  server.closeAllConnections();
  await new Promise<void>((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
}

test("gateway preserves mutations, query strings, idempotency and error envelopes with server credentials", async (t) => {
  let requests = 0;
  const upstream = http.createServer(async (req, res) => {
    requests++;
    assert.equal(req.url, "/prayer/api/v1/bots/b/scripts?mode=normal");
    assert.equal(req.method, "POST");
    assert.equal(req.headers.authorization, "Bearer server-secret");
    assert.equal(req.headers.cookie, undefined);
    assert.equal(req.headers["idempotency-key"], "stable-key");
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    assert.equal(Buffer.concat(chunks).toString(), '{ "script": "go sol;" }');
    res.writeHead(409, { "content-type": "application/json", "retry-after": "2" });
    res.end(JSON.stringify({ error: { code: "lane_busy", message: "Busy", retryable: true } }));
  });
  const upstreamUrl = await listen(upstream);
  t.after(() => close(upstream));
  const app = express();
  registerPrayerProxy(app, { baseUrl: `${upstreamUrl}/prayer`, token: "server-secret" });
  app.use(express.json());
  app.post("/api/job-runs", (_req, res) => res.json({ local: true }));
  const server = http.createServer(app);
  const origin = await listen(server);
  t.after(() => close(server));
  const response = await fetch(`${origin}/api/v1/bots/b/scripts?mode=normal`, {
    method: "POST",
    headers: { "content-type": "application/json", "idempotency-key": "stable-key", authorization: "Bearer browser-secret", cookie: "session=private" },
    body: '{ "script": "go sol;" }',
  });
  assert.equal(response.status, 409);
  assert.equal(response.headers.get("retry-after"), "2");
  assert.deepEqual(await response.json(), { error: { code: "lane_busy", message: "Busy", retryable: true } });
  assert.deepEqual(await (await fetch(`${origin}/api/job-runs`, { method: "POST" })).json(), { local: true });
  assert.equal(requests, 1);
});

test("gateway streams before upstream completion and disconnects when the browser aborts", async (t) => {
  let disconnected!: () => void;
  const ended = new Promise<void>((resolve) => {
    disconnected = resolve;
  });
  const upstream = http.createServer((_req, res) => {
    res.writeHead(200, { "content-type": "text/event-stream" });
    res.write("data: ready\n\n");
    res.on("close", disconnected);
  });
  const upstreamUrl = await listen(upstream);
  t.after(() => close(upstream));
  const app = express();
  registerPrayerProxy(app, { baseUrl: upstreamUrl });
  const server = http.createServer(app);
  const origin = await listen(server);
  t.after(() => close(server));
  const controller = new AbortController();
  const response = await fetch(`${origin}/api/v1/stream`, { signal: controller.signal });
  const chunk = await response.body!.getReader().read();
  assert.equal(new TextDecoder().decode(chunk.value), "data: ready\n\n");
  controller.abort();
  await ended;
});

test("unavailable upstream produces a sanitized 502 without retrying", async (t) => {
  let attempts = 0;
  const upstream = http.createServer((req) => {
    attempts++;
    req.socket.destroy();
  });
  const upstreamUrl = await listen(upstream);
  t.after(() => close(upstream));
  const app = express();
  registerPrayerProxy(app, { baseUrl: upstreamUrl });
  const server = http.createServer(app);
  const origin = await listen(server);
  t.after(() => close(server));
  const response = await fetch(`${origin}/api/v1/bots/register`, { method: "POST", body: "{}" });
  assert.equal(response.status, 502);
  assert.deepEqual(await response.json(), { error: { code: "upstream_unavailable", message: "Prayer API connection failed", retryable: true } });
  assert.equal(attempts, 1);
});

test("upstream redirects cannot bypass the reference client", async (t) => {
  const upstream = http.createServer((_req, res) => {
    res.writeHead(307, { location: "https://private-prayer.invalid/api/v1/state" });
    res.end();
  });
  const upstreamUrl = await listen(upstream);
  t.after(() => close(upstream));
  const app = express();
  registerPrayerProxy(app, { baseUrl: upstreamUrl });
  const server = http.createServer(app);
  const origin = await listen(server);
  t.after(() => close(server));
  const response = await fetch(`${origin}/api/v1/state`, { redirect: "manual" });
  assert.equal(response.status, 502);
  assert.equal(response.headers.get("location"), null);
});
