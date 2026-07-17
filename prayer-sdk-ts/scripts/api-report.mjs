import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const declarations = [
  "actions.d.ts", "client.d.ts", "conveniences.d.ts", "errors.d.ts",
  "transport.d.ts", "index.d.ts", "api.d.ts",
  "types.d.ts", "generated/api.d.ts", "generated/types.d.ts",
];
const report = declarations.map((file) =>
  `// ${file}\n${readFileSync(resolve(root, "dist/src", file), "utf8").trim()}\n`,
).join("\n");
const destination = resolve(root, "api-report.d.ts");
if (process.argv.includes("--write")) writeFileSync(destination, report);
else if (readFileSync(destination, "utf8") !== report) {
  throw new Error("TypeScript public API changed; review it and run `npm run api:report`");
}
