import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(import.meta.dirname, "../..");
const outputs = [
  resolve(root, "prayer-sdk-ts/src/generated/types.ts"),
  resolve(root, "prayer-sdk-ts/src/generated/api.ts"),
];
const before = outputs.map((path) => readFileSync(path, "utf8"));
for (const script of ["generate-types.mjs", "generate-api.mjs"]) {
  const result = spawnSync(process.execPath, [resolve(root, "prayer-sdk-ts/scripts", script)], { stdio: "inherit" });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
outputs.forEach((path, index) => {
  if (readFileSync(path, "utf8") !== before[index]) {
    throw new Error(`Generated SDK artifact is stale: ${path}`);
  }
});
