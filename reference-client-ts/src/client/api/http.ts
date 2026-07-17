const CLIENT_SLOW_MS = 750;

function logClientTiming(label: string, started: number, detail: string, force = false): void {
  const elapsed = Math.round(performance.now() - started);
  const message = `[client-api] ${label} ${detail} ms=${elapsed}`;
  if (force || elapsed >= CLIENT_SLOW_MS) console.warn(message);
  else if (label.includes("sessions")) console.info(message);
}

export async function fetchWithTimeout(url: string, timeoutMs: number, initOrLabel?: RequestInit | string, label = url): Promise<Response> {
  const init = typeof initOrLabel === "string" ? undefined : initOrLabel;
  const logLabel = typeof initOrLabel === "string" ? initOrLabel : label;
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), timeoutMs);
  const started = performance.now();
  try {
    const response = await fetch(url, {
      ...init,
      signal: controller.signal,
      cache: init?.cache ?? "no-store",
    });
    logClientTiming(logLabel, started, `status=${response.status}`);
    return response;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    logClientTiming(logLabel, started, `failed=${message}`, true);
    throw error;
  } finally {
    window.clearTimeout(timer);
  }
}
