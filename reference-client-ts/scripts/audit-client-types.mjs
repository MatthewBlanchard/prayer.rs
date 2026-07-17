import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import console from "node:console";
import { URL } from "node:url";

const root = new URL("..", import.meta.url).pathname;
const clientRoot = join(root, "src/client");
const files = [];
function walk(directory) {
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (/\.[cm]?[tj]sx?$/.test(name)) files.push(path);
  }
}
walk(clientRoot);

const rules = [
  { name: "double assertion in canonical selectors", scope: /src\/client\/prayer\//, pattern: /as\s+unknown\s+as/g },
  { name: "SDK state assertion", scope: /src\/client\//, pattern: /(?:bot\.state|snapshot\.[\w.]+)\s+as\s+/g },
  { name: "legacy body parser import", scope: /src\/client\/prayer\//, pattern: /(?:import|require)[^\n]*(?:parseCatalogBody|parseShipyardBody)/g },
  { name: "raw canonical view field", scope: /src\/client\/api\/types\.ts$/, pattern: /\braw\s*:\s*Record<string,\s*unknown>/g },
  { name: "unchecked JSON response assertion", scope: /src\/client\/api\//, pattern: /(?:await\s+[^\n]*\.json\(\)|\.json\(\))\s+as\s+/g },
];

const failures = [];
for (const file of files) {
  const name = relative(root, file);
  const text = readFileSync(file, "utf8");
  for (const rule of rules) {
    if (!rule.scope.test(name)) continue;
    for (const match of text.matchAll(rule.pattern)) {
      const line = text.slice(0, match.index).split("\n").length;
      failures.push(`${name}:${line}: ${rule.name}`);
    }
  }
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exitCode = 1;
} else {
  console.log("Canonical client type-flow audit passed.");
}
