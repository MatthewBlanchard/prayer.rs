import { isRecord, type JsonRecord } from "./api/decoding.js";

export function readStored<T>(key: string, decode: (value: unknown) => T | null, fallback: T): T {
  try {
    const text = window.localStorage.getItem(key);
    if (!text) return fallback;
    const value: unknown = JSON.parse(text);
    return decode(value) ?? fallback;
  } catch {
    return fallback;
  }
}

export function writeStored(key: string, value: unknown): boolean {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
    return true;
  } catch {
    return false;
  }
}

export function readVersionedStored<T>(key: string, version: number, decode: (value: unknown) => T | null, fallback: T): T {
  return readStored(
    key,
    (value) => {
      if (isRecord(value) && "version" in value) {
        if (value.version !== version || !("data" in value)) return null;
        return decode(value.data);
      }
      // Explicit version-0 migration: values predating envelopes stored data directly.
      return decode(value);
    },
    fallback,
  );
}

export function writeVersionedStored(key: string, version: number, value: unknown): boolean {
  return writeStored(key, { version, data: value });
}

export function readStoredRecord(key: string): JsonRecord | null {
  return readStored(key, (value) => (isRecord(value) ? value : null), null);
}

export function readVersionedStoredRecord(key: string, version = 1): JsonRecord | null {
  return readVersionedStored(key, version, (value) => (isRecord(value) ? value : null), null);
}

export function readStoredStringSet(key: string): Set<string> {
  return readStored(key, (value) => (Array.isArray(value) && value.every((item) => typeof item === "string") ? new Set(value) : null), new Set<string>());
}

export function readVersionedStoredStringSet(key: string, version = 1): Set<string> {
  return readVersionedStored(
    key,
    version,
    (value) => (Array.isArray(value) && value.every((item) => typeof item === "string") ? new Set(value) : null),
    new Set<string>(),
  );
}
