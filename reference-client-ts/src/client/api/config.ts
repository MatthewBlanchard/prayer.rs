import { fetchWithTimeout } from "./http.js";
import { errorMessage, isRecord } from "./decoding.js";

export type ProviderConfig = {
  prayerApiUrl?: string;
};

export async function fetchConfig(): Promise<ProviderConfig | null> {
  try {
    const res = await fetchWithTimeout("/api/config", 5_000, "GET /api/config");
    if (!res.ok) return null;
    const body: unknown = await res.json();
    if (!isRecord(body) || (body.prayerApiUrl !== undefined && typeof body.prayerApiUrl !== "string")) return null;
    return typeof body.prayerApiUrl === "string" ? { prayerApiUrl: body.prayerApiUrl } : {};
  } catch {
    return null;
  }
}

export type RegisterBotResult = {
  bot: { botId: string; name: string | null };
  playerId: string;
  password: string;
};

export async function registerBot(input: { username: string; empire: string; registrationCode?: string }): Promise<RegisterBotResult> {
  const config = await fetchConfig();
  const baseUrl = config?.prayerApiUrl?.trim() || window.location.origin;
  const response = await fetch(`${baseUrl.replace(/\/$/, "")}/api/v1/bots/register`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    throw new Error(errorMessage(body) ?? `Registration failed (${response.status})`);
  }
  if (
    !isRecord(body) ||
    !isRecord(body.bot) ||
    typeof body.bot.botId !== "string" ||
    (body.bot.name !== null && typeof body.bot.name !== "string") ||
    typeof body.playerId !== "string" ||
    typeof body.password !== "string"
  ) {
    throw new Error("Registration returned an invalid response");
  }
  return { bot: { botId: body.bot.botId, name: body.bot.name }, playerId: body.playerId, password: body.password };
}
