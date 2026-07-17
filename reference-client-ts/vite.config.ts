import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import fs from "node:fs";
import path from "node:path";
import { validateManifest } from "./src/server/plugins/registry.js";

function validatePrayerPlugins() {
  let generated = "";
  return {
    name: "validate-prayer-plugins",
    buildStart() {
      const root = path.resolve("plugins");
      if (!fs.existsSync(root)) { generated = "export const clientPlugins = [];"; return; }
      const ids = new Set<string>();
      const imports: string[] = [];
      const clients: string[] = [];
      for (const name of fs.readdirSync(root).sort()) {
        const directory = path.join(root, name);
        if (!fs.statSync(directory).isDirectory()) continue;
        const manifest = validateManifest(JSON.parse(fs.readFileSync(path.join(directory, "plugin.json"), "utf8")), name);
        if (ids.has(manifest.id)) throw new Error(`duplicate plugin ID '${manifest.id}'`);
        ids.add(manifest.id);
        for (const key of ["shared", "client", "server"] as const) if (manifest[key] && !fs.existsSync(path.resolve(directory, manifest[key]))) throw new Error(`${name}: missing ${key} entry ${manifest[key]}`);
        if (manifest.client) {
          const variable = `plugin${clients.length}`;
          imports.push(`import ${variable} from ${JSON.stringify(path.resolve(directory, manifest.client))};`);
          clients.push(`{ id: ${JSON.stringify(manifest.id)}, ...${variable} }`);
        }
      }
      generated = `${imports.join("\n")}\nexport const clientPlugins = [${clients.join(",")}];`;
    },
    transform(_code: string, id: string) { if (id.endsWith("/src/client/plugins.ts")) return { code: generated || "export const clientPlugins = [];", map: null }; },
  };
}

export default defineConfig({
  plugins: [react(), validatePrayerPlugins()],
  root: ".",
  server: {
    host: "127.0.0.1",
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:3001",
      "/events": "http://127.0.0.1:3001",
    },
  },
  build: {
    outDir: "dist/public",
    emptyOutDir: true,
    // The Three.js galaxy view is lazy-loaded and intentionally forms a large,
    // isolated chunk; keep the warning threshold focused on unexpected growth.
    chunkSizeWarningLimit: 600,
  },
});
