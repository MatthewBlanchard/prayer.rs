import type { Express } from "express";
import http, { type IncomingHttpHeaders } from "node:http";
import https from "node:https";
import { pipeline } from "node:stream";

const HOP_HEADERS = ["connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "te", "trailer", "transfer-encoding", "upgrade"];

function endToEndHeaders(input: IncomingHttpHeaders): IncomingHttpHeaders {
  const headers = { ...input };
  for (const name of [...HOP_HEADERS, ...(input.connection ?? "").split(",").map((name) => name.trim().toLowerCase())]) delete headers[name];
  return headers;
}

/** Register before body parsers so requests and responses can stream unchanged. */
export function registerPrayerProxy(app: Express, options: { baseUrl: string; token?: string }): void {
  const base = new URL(options.baseUrl.endsWith("/") ? options.baseUrl : `${options.baseUrl}/`);
  if (base.protocol !== "http:" && base.protocol !== "https:") throw new Error("Prayer API URL must use HTTP or HTTPS");
  app.use("/api/v1", (req, res) => {
    const target = new URL(req.originalUrl.slice(1), base);
    const headers = endToEndHeaders(req.headers);
    delete headers.host;
    // Browser credentials belong to the control room, not its upstream service.
    delete headers.cookie;
    delete headers.authorization;
    if (options.token) headers.authorization = `Bearer ${options.token}`;
    const fail = () => {
      if (res.destroyed) return;
      if (res.headersSent) {
        res.destroy();
        return;
      }
      res.status(502).json({ error: { code: "upstream_unavailable", message: "Prayer API connection failed", retryable: true } });
    };
    const upstream = (target.protocol === "https:" ? https : http).request(target, { method: req.method, headers }, (response) => {
      // Prayer is a JSON API. Never let an upstream redirect send the browser
      // directly to a private upstream or replay a mutation at another origin.
      if (response.headers.location && (response.statusCode ?? 0) >= 300 && (response.statusCode ?? 0) < 400) {
        fail();
        response.destroy();
        return;
      }
      const responseHeaders = endToEndHeaders(response.headers);
      delete responseHeaders["set-cookie"];
      res.writeHead(response.statusCode ?? 502, responseHeaders);
      pipeline(response, res, () => {});
    });
    upstream.on("error", fail);
    upstream.setTimeout(120_000, () => upstream.destroy(new Error("Prayer API timed out")));
    req.on("aborted", () => upstream.destroy());
    res.on("close", () => upstream.destroy());
    // Do not retry mutations: the upstream may already have accepted them.
    req.pipe(upstream);
  });
}
