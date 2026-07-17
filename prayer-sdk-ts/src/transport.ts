import { PrayerAbortError, PrayerApiError, PrayerConnectionError, PrayerTimeoutError } from "./errors.js";
import type { ErrorEnvelope } from "./generated/types.js";

export interface RequestOptions { signal?: AbortSignal; timeoutMs?: number }
export interface TransportOptions { baseUrl: string; token?: string; fetch?: typeof globalThis.fetch; timeoutMs?: number; signal?: AbortSignal; headers?: Record<string, string> }

export class Transport {
  private readonly fetcher: typeof globalThis.fetch;
  constructor(private readonly options: TransportOptions) {
    this.fetcher = options.fetch ?? globalThis.fetch.bind(globalThis);
  }
  async request<T>(path: string, init: RequestInit = {}, options: RequestOptions = {}): Promise<T> {
    const timeout = AbortSignal.timeout(options.timeoutMs ?? this.options.timeoutMs ?? 30_000);
    const callerSignal = options.signal ?? this.options.signal;
    const signal = callerSignal ? AbortSignal.any([callerSignal, timeout]) : timeout;
    const headers = new Headers(this.options.headers);
    headers.set("accept", "application/json");
    if (init.body) headers.set("content-type", "application/json");
    if (this.options.token) headers.set("authorization", `Bearer ${this.options.token}`);
    if (init.headers) new Headers(init.headers).forEach((value, key) => {
      if (key.toLowerCase() === "idempotency-key" && !value.trim()) throw new TypeError("Idempotency-Key must not be blank");
      headers.set(key, value);
    });
    let response: Response;
    try { response = await this.fetcher(new URL(path, normalizedBase(this.options.baseUrl)), { ...init, headers, signal }); }
    catch (error) {
      if (callerSignal?.aborted) throw new PrayerAbortError(callerSignal.reason ?? error);
      if (timeout.aborted) throw new PrayerTimeoutError(options.timeoutMs ?? this.options.timeoutMs ?? 30_000, error);
      const detail = error instanceof Error && error.message ? `: ${error.message}` : "";
      throw new PrayerConnectionError(`Prayer API ${init.method ?? "GET"} ${path} failed${detail}`, error);
    }
    if (response.status === 204) return undefined as T;
    const body = await response.json().catch(() => undefined) as T | ErrorEnvelope | undefined;
    if (!response.ok) {
      if (body && typeof body === "object" && "error" in body) throw PrayerApiError.from(response.status, body as ErrorEnvelope);
      throw new PrayerApiError(response.status, "http_error", `Prayer API returned ${response.status}`, response.status >= 500);
    }
    return body as T;
  }
}
function normalizedBase(value: string): string { return value.endsWith("/") ? value : `${value}/`; }
