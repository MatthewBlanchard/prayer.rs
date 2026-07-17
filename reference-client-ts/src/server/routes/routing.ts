import type { Prayer } from "@prayer/sdk";
import type { Express } from "express";

export function registerRoutingRoutes(app: Express, prayer: Prayer): void {
  app.post("/api/routes", async (req, res, next) => {
    try {
      const routes = Array.isArray(req.body?.routes) ? req.body.routes : [];
      const safe = req.body?.safe !== false;
      res.json({ routes: await prayer.routes(routes, { safe }) });
    } catch (error) {
      next(error);
    }
  });
}
