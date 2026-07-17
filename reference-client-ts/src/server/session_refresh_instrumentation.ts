import { AsyncLocalStorage } from "node:async_hooks";

export type SessionRefreshContext = {
  workflow?: string;
  cycle?: number;
  sessionHandle?: string;
  operation?: string;
};

export type SessionRefreshMetrics = {
  count: number;
  totalMs: number;
  botsMs: number;
  stateMs: number;
};

type SessionRefreshStore = SessionRefreshContext & {
  metrics?: SessionRefreshMetrics;
};

const storage = new AsyncLocalStorage<SessionRefreshStore>();

export function sessionRefreshContext(): SessionRefreshStore | undefined {
  return storage.getStore();
}

export async function withSessionRefreshMetrics<T>(
  context: SessionRefreshContext,
  fn: () => Promise<T>,
): Promise<{ result: T; metrics: SessionRefreshMetrics }> {
  const metrics: SessionRefreshMetrics = { count: 0, totalMs: 0, botsMs: 0, stateMs: 0 };
  const parent = storage.getStore();
  const result = await storage.run({ ...parent, ...context, metrics }, fn);
  return { result, metrics };
}

export function recordSessionRefresh(metrics: Omit<SessionRefreshMetrics, "count">): void {
  const active = storage.getStore()?.metrics;
  if (!active) return;
  active.count += 1;
  active.totalMs += metrics.totalMs;
  active.botsMs += metrics.botsMs;
  active.stateMs += metrics.stateMs;
}
