import type { Express } from "express";
import type { SquadStore } from "../squads.js";

export function registerSquadRoutes(app: Express, store: SquadStore): void {
  app.get("/api/squads", (_req, res) => res.json(store.list()));
  app.post("/api/squads", async (req, res) => res.status(201).json(await store.create(req.body)));
  app.patch("/api/squads/:id", async (req, res) => {
    try {
      res.json(await store.update(req.params["id"] ?? "", req.body));
    } catch (error) {
      res.status(404).json({ error: error instanceof Error ? error.message : String(error) });
    }
  });
  app.delete("/api/squads/:id", async (req, res) => {
    try {
      await store.delete(req.params["id"] ?? "");
      res.status(204).end();
    } catch (error) {
      res.status(404).json({ error: error instanceof Error ? error.message : String(error) });
    }
  });
}
