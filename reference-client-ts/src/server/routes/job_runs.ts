import type { Express, Request, Response } from "express";
import type { JobSupervisor } from "../job-runner/supervisor.js";
import type { SquadStore } from "../squads.js";

export function registerJobRunRoutes(app: Express, supervisor: JobSupervisor, squads: SquadStore): void {
  app.get("/api/job-definitions", (_req, res) => res.json(supervisor.listDefinitions()));
  app.get("/api/job-runs", (req, res) =>
    res.json(supervisor.listRuns({ status: stringQuery(req, "status"), kind: stringQuery(req, "kind"), limit: numberQuery(req, "limit") })),
  );
  app.post("/api/job-runs", async (req: Request, res: Response) => {
    try {
      const squadId = typeof req.body?.squadId === "string" ? req.body.squadId : "";
      const squad = squads.get(squadId);
      if (!squad) return res.status(400).json({ error: "a valid squadId is required" });
      if (!squad.botIds.length) return res.status(400).json({ error: "the selected squad has no bots" });
      res.status(201).json(await supervisor.start(req.body?.config ?? req.body, squad));
    } catch (error) {
      res.status(error instanceof Error && error.message.includes("locked") ? 409 : 400).json({ error: message(error) });
    }
  });
  app.get("/api/job-runs/:id", (req, res) => {
    const run = supervisor.getRun(req.params["id"] ?? "");
    if (!run) return res.status(404).json({ error: "job run not found" });
    res.json(run);
  });
  app.post("/api/job-runs/:id/stop", async (req, res) => {
    try {
      res.json(await supervisor.stop(req.params["id"] ?? "", req.body?.mode === "halt_now" ? "halt_now" : "after_current"));
    } catch (error) {
      res.status(message(error).includes("not found") ? 404 : 409).json({ error: message(error) });
    }
  });
  app.delete("/api/job-runs/:id", async (req, res) => {
    try {
      await supervisor.delete(req.params["id"] ?? "");
      res.status(204).end();
    } catch (error) {
      res.status(message(error).includes("not found") ? 404 : 409).json({ error: message(error) });
    }
  });
}
const message = (error: unknown) => (error instanceof Error ? error.message : String(error));
const stringQuery = (req: Request, key: string) => (typeof req.query[key] === "string" ? (req.query[key] as string) : undefined);
const numberQuery = (req: Request, key: string) => {
  const raw = stringQuery(req, key);
  const value = raw ? Number(raw) : undefined;
  return value && Number.isFinite(value) ? value : undefined;
};
