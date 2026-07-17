import { SessionInfo } from "../shared/types.js";

/** The jobs runtime historically addresses bots by their unique player username. */
export function jobHandleForSession(session: SessionInfo): string {
  return session.sessionHandle;
}

export function sessionMatchesJobHandle(session: SessionInfo, handle: string): boolean {
  return session.sessionHandle === handle || jobHandleForSession(session) === handle;
}

export function sessionByJobHandle<T extends SessionInfo>(sessions: T[]): Map<string, T> {
  const result = new Map<string, T>();
  for (const session of sessions) {
    result.set(session.sessionHandle, session);
    result.set(jobHandleForSession(session), session);
  }
  return result;
}
