import { Prayer, wait } from "../dist/src/index.js";

const baseUrl = process.env.PRAYER_BASE_URL;
const token = process.env.PRAYER_TOKEN;
const selector = process.env.PRAYER_BOT;
if (!baseUrl || !selector) throw new Error("PRAYER_BASE_URL and PRAYER_BOT are required");

const prayer = await Prayer.connect({ baseUrl, token, timeoutMs: 15_000 });
const bot = await prayer.bot(selector);
await bot.state();
const run = await bot.startActions(wait(1), { idempotencyKey: crypto.randomUUID() });
const terminal = await run.wait({ pollMs: 250 });
if (terminal.status !== "succeeded") throw new Error(run.errorMessage ?? `smoke run ${terminal.status}`);
