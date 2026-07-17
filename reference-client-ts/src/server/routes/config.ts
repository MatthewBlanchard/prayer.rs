import { Express, Request, Response } from "express";

type ConfigRouteContext = {
  prayerApiUrl: string;
};

export function registerConfigRoutes(app: Express, ctx: ConfigRouteContext): void {
  app.get("/api/health", (_req: Request, res: Response) => {
    res.json({ ok: true });
  });

  app.get("/api/config", (_req: Request, res: Response) => {
    res.json({
      prayerApiUrl: ctx.prayerApiUrl,
    });
  });
}
