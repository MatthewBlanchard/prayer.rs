import { Prayer, wait } from "@prayer/sdk";

export async function quickStart(baseUrl: string, token?: string): Promise<void> {
  const prayer = await Prayer.connect({ baseUrl, token });
  const [summary] = await prayer.bots();
  if (!summary) throw new Error("No bots are connected");
  const bot = await prayer.bot(summary.botId);
  await bot.state();
  const run = await bot.startActions(wait(1));
  const terminal = await run.wait();
  switch (terminal.status) {
    case "succeeded": return;
    case "failed":
    case "cancelled":
    case "halted": {
      const outcome = terminal.outcome;
      throw new Error("message" in outcome ? outcome.message : "reason" in outcome ? outcome.reason : terminal.status);
    }
  }
}
