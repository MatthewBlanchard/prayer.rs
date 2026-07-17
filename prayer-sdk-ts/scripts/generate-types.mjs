import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const input = resolve(process.argv[2] ?? resolve(root, "prayer-api/openapi/prayer-v1.json"));
const output = resolve(process.argv[3] ?? resolve(root, "prayer-sdk-ts/src/generated/types.ts"));
const spec = JSON.parse(readFileSync(input, "utf8"));
const schemas = spec.components?.schemas;
if (!schemas || typeof schemas !== "object") throw new Error("OpenAPI components.schemas is missing");

const refName = (ref) => ref.slice(ref.lastIndexOf("/") + 1);
function typeOf(schema) {
  if (!schema || Object.keys(schema).length === 0) return "unknown";
  if (schema.$ref) return refName(schema.$ref);
  if (schema.const !== undefined) return JSON.stringify(schema.const);
  if (schema.enum) return schema.enum.map((value) => JSON.stringify(value)).join(" | ") || "never";
  if (schema.oneOf) return schema.oneOf.map(typeOf).join(" | ");
  if (schema.anyOf) return schema.anyOf.map(typeOf).join(" | ");
  if (schema.allOf) return schema.allOf.map(typeOf).join(" & ");
  if (Array.isArray(schema.type)) return schema.type.map((type) => typeOf({ ...schema, type })).join(" | ");
  if (schema.type === "array") return `Array<${typeOf(schema.items)}> ` .trim();
  if (schema.type === "object" || schema.properties || schema.additionalProperties) {
    if (!schema.properties && schema.additionalProperties === false) return "Record<string, never>";
    if (!schema.properties && schema.additionalProperties === undefined) return "unknown";
    if (!schema.properties && schema.additionalProperties) return `Record<string, ${typeOf(schema.additionalProperties)}>`;
    const required = new Set(schema.required ?? []);
    const fields = Object.entries(schema.properties ?? {}).map(([name, value]) => `${JSON.stringify(name)}${required.has(name) ? "" : "?"}: ${typeOf(value)};`);
    if (schema.additionalProperties) fields.push(`[field: string]: ${schema.additionalProperties === true ? "unknown" : typeOf(schema.additionalProperties)};`);
    return `{ ${fields.join(" ")} }`;
  }
  if (schema.type === "string") return "string";
  if (schema.type === "integer" || schema.type === "number") return "number";
  if (schema.type === "boolean") return "boolean";
  if (schema.type === "null") return "null";
  return "unknown";
}

const declarations = Object.entries(schemas).map(([name, schema]) => {
  if (name === "WorldState") {
    const resolved = structuredClone(schema);
    resolved.required = Object.keys(resolved.properties);
    resolved.properties.stationMarkets = { $ref: "#/components/schemas/StationMarkets" };
    return `export interface WorldState ${typeOf(resolved)}`;
  }
  const value = typeOf(schema);
  if (schema.properties && !schema.oneOf && !schema.anyOf && !schema.allOf) return `export interface ${name} ${value}`;
  return `export type ${name} = ${value};`;
});

writeFileSync(output, `// AUTO-GENERATED from prayer-api/openapi/prayer-v1.json. DO NOT EDIT.\n${declarations.join("\n")}\n`);
