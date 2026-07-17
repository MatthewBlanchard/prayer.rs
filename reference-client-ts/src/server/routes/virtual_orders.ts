import { Express, Request } from "express";
import type { Prayer } from "@prayer/sdk";
import type { VirtualCraftOrder, VirtualMarketOrder } from "@prayer/sdk/types";

export function registerVirtualOrderRoutes(app: Express, prayer: Prayer): void {
  app.get("/api/virtual-orders", async (_req, res) => {
    try {
      res.json(await prayer.advanced.api.listVirtualOrders());
    } catch {
      res.status(502).json({ error: "failed to fetch virtual orders from prayer-api" });
    }
  });

  app.put("/api/virtual-orders", async (req: Request, res) => {
    const body = req.body as Record<string, unknown>;
    const orders = Array.isArray(body["orders"]) ? body["orders"] : [];
    try {
      res.json(await prayer.advanced.api.createVirtualOrders({ orders: orders as VirtualMarketOrder[] }, crypto.randomUUID()));
    } catch {
      res.status(502).json({ error: "failed to save virtual orders to prayer-api" });
    }
  });

  app.post("/api/virtual-orders/reserve", async (req: Request, res) => {
    const uses = requestUses(req);
    try {
      res.json(await prayer.advanced.api.reserveVirtualOrders({ uses }, crypto.randomUUID()));
    } catch {
      res.status(502).json({ error: "failed to reserve virtual orders in prayer-api" });
    }
  });

  app.post("/api/virtual-orders/:id/fill", async (req: Request, res) => {
    try {
      res.json(await prayer.advanced.api.fillVirtualOrder(req.params["id"] ?? "", crypto.randomUUID()));
    } catch {
      res.status(502).json({ error: "failed to fill virtual order in prayer-api" });
    }
  });

  app.post("/api/virtual-orders/:id/release", async (req: Request, res) => {
    try {
      res.json(await prayer.advanced.api.releaseVirtualOrder(req.params["id"] ?? "", crypto.randomUUID()));
    } catch {
      res.status(502).json({ error: "failed to release virtual order in prayer-api" });
    }
  });

  app.get("/api/virtual-craft-orders", async (_req, res) => {
    try {
      res.json(await prayer.advanced.api.listVirtualCraftOrders());
    } catch {
      res.status(502).json({ error: "failed to fetch virtual craft orders from prayer-api" });
    }
  });

  app.put("/api/virtual-craft-orders", async (req: Request, res) => {
    const body = req.body as Record<string, unknown>;
    const orders = Array.isArray(body["orders"]) ? body["orders"] : [];
    try {
      res.json(await prayer.advanced.api.createVirtualCraftOrders({ orders: orders as VirtualCraftOrder[] }, crypto.randomUUID()));
    } catch {
      res.status(502).json({ error: "failed to save virtual craft orders to prayer-api" });
    }
  });

  app.post("/api/virtual-craft-orders/reserve", async (req: Request, res) => {
    try {
      res.json(await prayer.advanced.api.reserveVirtualCraftOrders({ uses: requestUses(req) }, crypto.randomUUID()));
    } catch {
      res.status(502).json({ error: "failed to reserve virtual craft orders in prayer-api" });
    }
  });

  app.post("/api/virtual-craft-orders/:id/fill", async (req: Request, res) => {
    try {
      res.json(await prayer.advanced.api.fillVirtualCraftOrder(req.params["id"] ?? "", crypto.randomUUID()));
    } catch {
      res.status(502).json({ error: "failed to fill virtual craft order in prayer-api" });
    }
  });

  app.post("/api/virtual-craft-orders/:id/release", async (req: Request, res) => {
    try {
      res.json(await prayer.advanced.api.releaseVirtualCraftOrder(req.params["id"] ?? "", crypto.randomUUID()));
    } catch {
      res.status(502).json({ error: "failed to release virtual craft order in prayer-api" });
    }
  });
}

function requestUses(req: Request): Array<{ orderId: string; quantity: number }> {
  const body = req.body as Record<string, unknown>;
  const raw = Array.isArray(body["uses"]) ? body["uses"] : [];
  return raw.flatMap((value) => {
    if (!value || typeof value !== "object" || Array.isArray(value)) return [];
    const obj = value as Record<string, unknown>;
    const orderId = typeof obj["orderId"] === "string" ? obj["orderId"] : "";
    const quantity = typeof obj["quantity"] === "number" ? obj["quantity"] : Number(obj["quantity"]);
    return orderId && Number.isFinite(quantity) && quantity > 0 ? [{ orderId, quantity }] : [];
  });
}
