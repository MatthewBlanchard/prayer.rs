import { readFileSync, readdirSync } from "node:fs";
import { extname, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const generated = readFileSync(resolve(root, "prayer-sdk-ts/src/generated/types.ts"), "utf8");

// These are upstream or workflow-owned JSON bags whose keys are not a stable
// Prayer contract. Every other generated public declaration must be typed.
const opaqueContracts = new Map([
  ["MarketMovement", "Prayer-owned restart context is intentionally caller-defined JSON."],
  ["MarketMovementReserveRequest", "Prayer-owned restart context is intentionally caller-defined JSON."],
  ["RuntimeGalaxySystemInfoDto", "SpaceMolt survey signatures and wildlife summaries have no published stable shape."],
  ["ShipClass", "SpaceMolt's generated required_items catalog field is intentionally open JSON."],
  ["V1ErrorDetail", "Error details vary by error code and are intentionally open JSON."],
  ["V2GameStateModulesItem", "SpaceMolt module stats are a server-defined stat bag."],
  ["V2GameStatePlayer", "SpaceMolt player stats are a server-defined stat bag."],
]);

for (const declaration of generated.matchAll(/^export (?:interface|type) (\w+).*\bunknown\b.*$/gm)) {
  const [text, name] = declaration;
  if (!opaqueContracts.has(name)) {
    throw new Error(`Unexpected unknown in generated public contract ${name}`);
  }
}

const clientRoot = resolve(root, "reference-client-ts/src");
const sourceFiles = [];
const visit = (directory) => {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) visit(path);
    else if ([".ts", ".tsx"].includes(extname(path))) sourceFiles.push(path);
  }
};
visit(clientRoot);
for (const path of sourceFiles) {
  const source = readFileSync(path, "utf8");
  if (/(?:\bas\s+any\b|:\s*any\b|<any>|\[field:\s*string\]:\s*any\b)/.test(source)) {
    throw new Error(`Client source replaces a typed contract with any: ${path}`);
  }
}
