import type { Prayer } from "@prayer/sdk";
import type { Express } from "express";
import type { JobConfig, JobDefinition } from "./shared.js";
import type { JobRun, JobRunEvent } from "../server/job-runner/types.js";

export type JobRunnerContext = {
  prayer: Prayer;
  run: JobRun;
  config: JobConfig;
  signal: AbortSignal;
  recovering: boolean;
  update(patch: Partial<JobRun>, message?: string, level?: JobRunEvent["level"]): Promise<void>;
  setBot(botId: string, patch: Partial<JobRun["botStates"][string]>): Promise<void>;
  delay(ms: number): Promise<void>;
  execute(botId: string, actions: Parameters<Awaited<ReturnType<Prayer["bot"]>>["startActions"]>[0], options?: { idempotencyKey?: string }): ReturnType<Awaited<ReturnType<Prayer["bot"]>>["execute"]>;
};

export type JobPlugin = {
  definition: JobDefinition;
  validate?(config: JobConfig): JobConfig;
  execute(context: JobRunnerContext): Promise<void>;
};

export type ServerPlugin = { jobs?: JobPlugin[]; routes?(app: Express): void | Promise<void> };
