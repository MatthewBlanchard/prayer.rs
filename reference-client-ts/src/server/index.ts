import express, { Request, Response } from "express";
import { fileURLToPath } from "url";
import path from "path";
import { JOBS_PATH, SQUADS_PATH, parseArgs } from "./config.js";
import { registerConfigRoutes } from "./routes/config.js";
import { registerSessionRoutes } from "./routes/sessions.js";
import { SseHub } from "./sse.js";
import { registerVirtualOrderRoutes } from "./routes/virtual_orders.js";
import { Prayer } from "@prayer/sdk";
import { JOB_RUNS_PATH } from "./config.js";
import { JobRunStore } from "./job-runner/store.js";
import { JobSupervisor } from "./job-runner/supervisor.js";
import { registerJobRunRoutes } from "./routes/job_runs.js";
import { archiveLegacyJobs } from "./job-runner/legacy.js";
import { SquadStore } from "./squads.js";
import { registerSquadRoutes } from "./routes/squads.js";
import { registerRoutingRoutes } from "./routes/routing.js";
import { discoverPlugins } from "./plugins/registry.js";
import { FuelWatcher } from "./fuel-watcher.js";

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  const { prayerApiUrl, port } = parseArgs();
  const archivedLegacyJobs = await archiveLegacyJobs(JOBS_PATH);
  if (archivedLegacyJobs) {
    console.warn(`Archived legacy squad conversations at ${archivedLegacyJobs}; they were not converted into executable jobs.`);
  }
  const prayer = await Prayer.connect({ baseUrl: prayerApiUrl });
  const fuelWatcher = new FuelWatcher(prayer);
  fuelWatcher.start();
  const sse = new SseHub();
  const productionPlugins = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../plugin-runtime/plugins");
  const pluginRegistry = await discoverPlugins(import.meta.url.includes("/dist/server/") ? productionPlugins : path.resolve("plugins"));
  console.log(`Loaded ${pluginRegistry.plugins.length} plugin(s): ${pluginRegistry.plugins.map((plugin) => plugin.id).join(", ") || "none"}`);
  const jobRunStore = new JobRunStore(JOB_RUNS_PATH);
  await jobRunStore.load();
  const squadStore = new SquadStore(SQUADS_PATH, JOBS_PATH);
  await squadStore.load();
  const fleetIdentities = new Map((await prayer.bots()).flatMap((bot) => [[bot.botId, bot.botId], ...(bot.name ? [[bot.name, bot.botId] as const] : [])]));
  for (const squad of squadStore.list()) {
    const stableBotIds = squad.botIds.map((id) => fleetIdentities.get(id) ?? id);
    if (stableBotIds.some((id, index) => id !== squad.botIds[index])) await squadStore.update(squad.id, { botIds: stableBotIds });
  }
  const jobSupervisor = new JobSupervisor(
    prayer,
    jobRunStore,
    (run) => sse.broadcast({ type: "job_run_updated", run }),
    pluginRegistry,
  );
  // ---------------------------------------------------------------------------
  // Express app
  // ---------------------------------------------------------------------------

  const app = express();
  app.use(express.json());

  // Serve built frontend in production
  const __dirname = path.dirname(fileURLToPath(import.meta.url));
  const publicDir = path.join(__dirname, "../public");
  app.use(express.static(publicDir));

  // SSE endpoint
  app.get("/events", (req: Request, res: Response) => {
    res.setHeader("Content-Type", "text/event-stream");
    res.setHeader("Cache-Control", "no-cache");
    res.setHeader("Connection", "keep-alive");
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.flushHeaders();

    // Send current state immediately on connect
    const stateEvent = {
      type: "state_sync" as const,
      jobRuns: jobSupervisor.listRuns({ limit: 50 }),
    };
    sse.write(res, stateEvent);

    sse.addClient(res);

    req.on("close", () => {
      sse.removeClient(res);
    });
  });

  await pluginRegistry.registerRoutes(app);
  registerVirtualOrderRoutes(app, prayer);
  registerRoutingRoutes(app, prayer);

  registerJobRunRoutes(app, jobSupervisor, squadStore);
  registerSquadRoutes(app, squadStore);

  registerSessionRoutes(app);

  registerConfigRoutes(app, { prayerApiUrl });

  await jobSupervisor.recover();
  app.listen(port, () => {
    console.log(`reference-client-ts running at http://localhost:${port}`);
  });

  // Graceful shutdown
  process.on("SIGINT", async () => {
    console.log("\nShutting down...");
    fuelWatcher.stop();
    process.exit(0);
  });
}

main().catch((err) => {
  console.error("Fatal:", err);
  process.exit(1);
});
