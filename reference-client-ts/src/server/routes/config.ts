import type { Express } from "express";

export function registerConfigRoutes(app: Express): void {
  app.get("/api/health", (_req, res) => {
    res.json({ ok: true });
  });
}
