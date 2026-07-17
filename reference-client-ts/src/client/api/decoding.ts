export type JsonRecord = Record<string, unknown>;

export function isRecord(value: unknown): value is JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === "string");
}

export function decodeArray<T>(value: unknown, decode: (entry: unknown) => T | null): T[] | null {
  if (!Array.isArray(value)) return null;
  const decoded: T[] = [];
  for (const entry of value) {
    const item = decode(entry);
    if (item === null) return null;
    decoded.push(item);
  }
  return decoded;
}

export function errorMessage(value: unknown): string | null {
  if (!isRecord(value)) return null;
  if (typeof value.error === "string") return value.error;
  if (isRecord(value.error) && typeof value.error.message === "string") return value.error.message;
  return null;
}

export async function decodeResponse<T>(response: Response, decode: (value: unknown) => T | null): Promise<T> {
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) throw new Error(errorMessage(body) ?? `HTTP ${response.status}`);
  const decoded = decode(body);
  if (decoded === null) throw new Error(`Invalid JSON response from ${response.url || "server"}`);
  return decoded;
}
