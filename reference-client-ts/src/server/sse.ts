import { Response } from "express";
import type { JobRun } from "./job-runner/types.js";

export type ServerEventEnvelope =
  | {
      type: "state_sync";
      jobRuns?: JobRun[];
    }
  | {
      type: "job_run_updated";
      run: JobRun;
    };

export class SseHub {
  private readonly clients = new Set<Response>();

  addClient(client: Response): void {
    this.clients.add(client);
  }

  removeClient(client: Response): void {
    this.clients.delete(client);
  }

  write(client: Response, event: ServerEventEnvelope): void {
    client.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);
  }

  broadcast(event: ServerEventEnvelope): void {
    for (const client of this.clients) {
      try {
        this.write(client, event);
      } catch {
        this.clients.delete(client);
      }
    }
  }
}
