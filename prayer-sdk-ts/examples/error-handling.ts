import {
  PrayerAbortError,
  PrayerApiError,
  PrayerCompatibilityError,
  PrayerNotFoundError,
  PrayerTimeoutError,
  isRetryableError,
} from "@prayer/sdk";

export function classify(error: unknown): "abort" | "retry" | "select" | "upgrade" | "api" {
  if (error instanceof PrayerAbortError) return "abort";
  if (error instanceof PrayerTimeoutError || isRetryableError(error)) return "retry";
  if (error instanceof PrayerNotFoundError) return "select";
  if (error instanceof PrayerCompatibilityError) return "upgrade";
  if (error instanceof PrayerApiError) return "api";
  throw error;
}
