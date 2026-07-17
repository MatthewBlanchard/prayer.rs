import path from "path";

export const JOBS_PATH = process.env["PRAYER_CLIENT_JOBS_PATH"] ?? path.resolve(process.cwd(), ".prayer-client-jobs.json");
export const JOB_RUNS_PATH = process.env["PRAYER_CLIENT_JOB_RUNS_PATH"] ?? path.resolve(process.cwd(), ".prayer-client-job-runs.json");
export const SQUADS_PATH = process.env["PRAYER_CLIENT_SQUADS_PATH"] ?? path.resolve(process.cwd(), ".prayer-client-squads.json");

export type ServerArgs = {
  prayerApiUrl: string;
  port: number;
};

export function parseArgs(): ServerArgs {
  const args = process.argv.slice(2);
  const get = (flag: string, env?: string, fallback = ""): string => {
    const idx = args.indexOf(flag);
    const value = idx === -1 ? undefined : args[idx + 1];
    if (value) return value;
    if (env && process.env[env]) return process.env[env]!;
    return fallback;
  };

  const portStr = get("--port", "PRAYER_CLIENT_PORT", "3001");
  const port = parseInt(portStr, 10) || 3001;

  const prayerApiUrl = get("--prayer-api-url", "PRAYER_CLIENT_API_URL", "http://127.0.0.1:7777");

  return { prayerApiUrl, port };
}
