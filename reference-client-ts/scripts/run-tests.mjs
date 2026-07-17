import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const tests = [];
function discover(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) discover(path);
    else if (/\.test\.(?:ts|tsx|mjs)$/.test(entry.name)) tests.push(path);
  }
}
for (const directory of ["src", "test"]) discover(directory);
for (const entry of readdirSync("plugins", { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const manifest = JSON.parse(readFileSync(join("plugins", entry.name, "plugin.json"), "utf8"));
  if (typeof manifest.server === "string" && manifest.server.endsWith(".ts")) discover(join("plugins", entry.name));
}
tests.sort();
if (tests.length === 0) throw new Error("no test files found under src or test");

const coverage = process.argv.includes("--coverage");
const cli = join("node_modules", "tsx", "dist", "cli.mjs");
const args = [cli, "--tsconfig", "tsconfig.client.json", "--test"];
if (coverage) {
  args.push("--experimental-test-coverage", "--test-coverage-lines=60");
}
args.push(...tests);
const result = spawnSync(process.execPath, args, { stdio: "inherit" });
if (result.error) throw result.error;
process.exit(result.status ?? 1);
