import { errorMessage, isRecord } from "./decoding.js";

export type RegisterBotResult = {
  bot: { botId: string; name: string | null };
  playerId: string;
  password: string;
};

export async function registerBot(input: { username: string; empire: string; registrationCode?: string }): Promise<RegisterBotResult> {
  const response = await fetch("/api/v1/bots/register", {
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
