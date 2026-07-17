import type { ErrorEnvelope } from "./generated/types.js";

export class PrayerApiError extends Error {
  readonly retryAfterMs?: number;
  constructor(public readonly status: number, public readonly code: string, message: string, public readonly retryable: boolean, public readonly details?: unknown, public readonly requestId?: string, options?: ErrorOptions) {
    super(message, options); this.name = "PrayerApiError";
    this.retryAfterMs = retryAfter(details);
  }
  static from(status: number, body: ErrorEnvelope): PrayerApiError {
    if (body.error.code === "lane_busy") return new LaneBusyError(status, body);
    if (status === 400 || body.error.code === "validation" || body.error.code === "bad_request") return new PrayerValidationError(status, body);
    if (status === 401 || status === 403) return new PrayerAuthenticationError(status, body);
    if (status === 404 || body.error.code === "not_found") return new PrayerNotFoundError(status, body);
    return new PrayerApiError(status, body.error.code, body.error.message, body.error.retryable, body.error.details, body.requestId);
  }
}
export class LaneBusyError extends PrayerApiError {
  constructor(status: number, body: ErrorEnvelope) { super(status, body.error.code, body.error.message, body.error.retryable, body.error.details, body.requestId); this.name = "LaneBusyError"; }
}
export class PrayerValidationError extends PrayerApiError { constructor(status: number, body: ErrorEnvelope) { super(status, body.error.code, body.error.message, body.error.retryable, body.error.details, body.requestId); this.name = "PrayerValidationError"; } }
export class PrayerAuthenticationError extends PrayerApiError { constructor(status: number, body: ErrorEnvelope) { super(status, body.error.code, body.error.message, body.error.retryable, body.error.details, body.requestId); this.name = "PrayerAuthenticationError"; } }
export class PrayerNotFoundError extends PrayerApiError { constructor(status: number, body: ErrorEnvelope) { super(status, body.error.code, body.error.message, body.error.retryable, body.error.details, body.requestId); this.name = "PrayerNotFoundError"; } }
export class PrayerConnectionError extends Error { constructor(message: string, public readonly cause?: unknown) { super(message, { cause }); this.name = "PrayerConnectionError"; } }
export class PrayerTimeoutError extends PrayerConnectionError {
  constructor(public readonly timeoutMs: number, cause?: unknown) { super(`Prayer API request timed out after ${timeoutMs}ms`, cause); this.name = "PrayerTimeoutError"; }
}
export class PrayerAbortError extends Error { constructor(public readonly cause?: unknown) { super("Prayer API request was aborted by the caller", { cause }); this.name = "PrayerAbortError"; } }
export class PrayerCompatibilityError extends Error { constructor(message: string) { super(message); this.name = "PrayerCompatibilityError"; } }

export const isLaneBusyError = (error: unknown): error is LaneBusyError => error instanceof LaneBusyError;
export const isNotFoundError = (error: unknown): error is PrayerNotFoundError => error instanceof PrayerNotFoundError;
export const isValidationError = (error: unknown): error is PrayerValidationError => error instanceof PrayerValidationError;
export const isAuthenticationError = (error: unknown): error is PrayerAuthenticationError => error instanceof PrayerAuthenticationError;
export const isRetryableError = (error: unknown): error is PrayerApiError | PrayerConnectionError => error instanceof PrayerConnectionError || (error instanceof PrayerApiError && error.retryable);

function retryAfter(details: unknown): number | undefined {
  if (!details || typeof details !== "object") return undefined;
  const value = (details as Record<string, unknown>).retryAfterMs ?? (details as Record<string, unknown>).retry_after_ms;
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : undefined;
}
