import type { ClientPlugin } from "../plugin-api/client.js";

// Replaced by the Vite plugin during client builds. Keeping a real module makes
// non-browser unit tests and hosts without plugins work without a custom loader.
export const clientPlugins: Array<ClientPlugin & { id: string }> = [];
