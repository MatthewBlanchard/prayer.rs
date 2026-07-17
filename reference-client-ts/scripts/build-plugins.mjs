import { cp, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";

const source = "plugins";
const output = "dist/plugin-runtime/plugins";
await mkdir(output, { recursive: true });
for (const entry of await readdir(source, { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const from = path.join(source, entry.name);
  const to = path.join(output, entry.name);
  await mkdir(to, { recursive: true });
  const manifest = JSON.parse(await readFile(path.join(from, "plugin.json"), "utf8"));
  if (typeof manifest.server === "string" && manifest.server.endsWith(".ts")) manifest.server = `${manifest.server.slice(0, -3)}.js`;
  await writeFile(path.join(to, "plugin.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  await cp(path.join(from, "README.md"), path.join(to, "README.md")).catch(() => undefined);
}
