import fs from "fs/promises";
import path from "path";
import type { JobRun } from "./types.js";

type JobRunsFile = { schemaVersion: 1; runs: JobRun[] };

export class JobRunStore {
  private runs = new Map<string, JobRun>();
  private writeChain = Promise.resolve();
  constructor(private readonly filePath: string) {}
  async load(): Promise<void> {
    try {
      const parsed = JSON.parse(await fs.readFile(this.filePath, "utf8")) as JobRunsFile;
      if (parsed.schemaVersion !== 1) throw new Error(`unsupported job run schema ${String(parsed.schemaVersion)}`);
      this.runs = new Map((parsed.runs ?? []).map((run) => [run.id, run]));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
  }
  list(): JobRun[] {
    return [...this.runs.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  }
  get(id: string): JobRun | undefined {
    return this.runs.get(id);
  }
  async put(run: JobRun): Promise<void> {
    this.runs.set(run.id, run);
    await this.persist();
  }
  async delete(id: string): Promise<boolean> {
    const deleted = this.runs.delete(id);
    if (deleted) await this.persist();
    return deleted;
  }
  private persist(): Promise<void> {
    const snapshot = JSON.stringify({ schemaVersion: 1, runs: this.list() } satisfies JobRunsFile, null, 2) + "\n";
    this.writeChain = this.writeChain.then(async () => {
      await fs.mkdir(path.dirname(this.filePath), { recursive: true });
      const temporary = `${this.filePath}.tmp`;
      await fs.writeFile(temporary, snapshot, "utf8");
      await fs.rename(temporary, this.filePath);
    });
    return this.writeChain;
  }
}
