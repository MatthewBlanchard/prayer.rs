import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const input = resolve(process.argv[2] ?? resolve(root, "prayer-api/openapi/prayer-v1.json"));
const destination = resolve(process.argv[3] ?? resolve(root, "prayer-sdk-ts/src/generated/api.ts"));
const spec = JSON.parse(readFileSync(input, "utf8"));
const typeImports = new Set();
const camel = (value) => {
  const converted = value.replace(/[-_]([A-Za-z])/g, (_, letter) => letter.toUpperCase());
  return converted[0].toLowerCase() + converted.slice(1);
};
const refType = (ref) => {
  const name = ref.slice(ref.lastIndexOf("/") + 1);
  typeImports.add(name);
  return name;
};
const schemaType = (schema, context) => {
  if (!schema || Object.keys(schema).length === 0) throw new Error(`${context} has a missing or vague schema`);
  if (schema.$ref) return refType(schema.$ref);
  if (schema.enum) return schema.enum.map(JSON.stringify).join(" | ");
  if (Array.isArray(schema.type)) return schema.type.map((type) => schemaType({ ...schema, type }, context)).join(" | ");
  if (schema.type === "array") return `Array<${schemaType(schema.items, `${context} items`)}>`;
  if (schema.type === "string") return "string";
  if (schema.type === "integer" || schema.type === "number") return "number";
  if (schema.type === "boolean") return "boolean";
  if (schema.type === "null") return "null";
  throw new Error(`${context} uses an unsupported or vague inline schema`);
};
const jsonSchema = (content, context) => schemaType(content?.["application/json"]?.schema, context);
const operations = [];

for (const [path, pathItem] of Object.entries(spec.paths ?? {})) {
  for (const [method, operation] of Object.entries(pathItem)) {
    if (!["get", "post", "put", "patch", "delete"].includes(method)) continue;
    if (!operation.operationId) throw new Error(`${method.toUpperCase()} ${path} is missing operationId`);
    const parameters = [...(pathItem.parameters ?? []), ...(operation.parameters ?? [])];
    const pathParameters = parameters.filter((parameter) => parameter.in === "path");
    const queryParameters = parameters.filter((parameter) => parameter.in === "query");
    const headerParameters = parameters.filter((parameter) => parameter.in === "header");
    const args = pathParameters.map((parameter) => `${camel(parameter.name)}: ${schemaType(parameter.schema, `${operation.operationId} path parameter ${parameter.name}`)}`);
    if (queryParameters.length) {
      const fields = queryParameters.map((parameter) => `${camel(parameter.name)}${parameter.required ? "" : "?"}: ${schemaType(parameter.schema, `${operation.operationId} query parameter ${parameter.name}`)}`);
      args.push(`query: { ${fields.join("; ")} } = {}`);
    }
    let bodyName;
    if (operation.requestBody && Object.keys(operation.requestBody).length) {
      bodyName = "body";
      args.push(`body${operation.requestBody.required ? "" : "?"}: ${jsonSchema(operation.requestBody.content, `${operation.operationId} request body`)}`);
    }
    for (const parameter of headerParameters) {
      args.push(`${camel(parameter.name)}${parameter.required ? "" : "?"}: ${schemaType(parameter.schema, `${operation.operationId} header ${parameter.name}`)}`);
    }
    args.push("options?: RequestOptions");

    const success = Object.entries(operation.responses ?? {}).find(([status]) => /^2\d\d$/.test(status));
    if (!success) throw new Error(`${operation.operationId} has no successful response`);
    const [status, response] = success;
    const returnType = status === "204" ? "void" : jsonSchema(response.content, `${operation.operationId} response`);
    let pathExpression = path.replace(/^\//, "").replaceAll(/\{([^}]+)\}/g, (_, name) => `\${encodeURIComponent(${camel(name)})}`);
    const queryLines = queryParameters.map((parameter) => {
      const name = camel(parameter.name);
      return `    if (query.${name} !== undefined) search.set(${JSON.stringify(parameter.name)}, String(query.${name}));`;
    });
    const suffix = queryParameters.length ? " + (search.size ? `?${search}` : \"\")" : "";
    const headers = headerParameters.length
      ? `, headers: { ${headerParameters.map((parameter) => parameter.required
        ? `${JSON.stringify(parameter.name)}: ${camel(parameter.name)}`
        : `...(${camel(parameter.name)} !== undefined ? { ${JSON.stringify(parameter.name)}: ${camel(parameter.name)} } : {})`).join(", ")} }`
      : "";
    const body = bodyName ? `, body: JSON.stringify(${bodyName})` : "";
    const init = method === "get" && !headers && !body ? "{}" : `{ method: ${JSON.stringify(method.toUpperCase())}${headers}${body} }`;
    operations.push(`  ${operation.operationId}(${args.join(", ")}): Promise<${returnType}> {\n${queryParameters.length ? "    const search = new URLSearchParams();\n" + queryLines.join("\n") + "\n" : ""}    return this.transport.request(\`${pathExpression}\`${suffix}, ${init}, options);\n  }`);
  }
}

const imports = [...typeImports].sort().join(", ");
const output = `// AUTO-GENERATED by scripts/generate-api.mjs from prayer-api/openapi/prayer-v1.json. DO NOT EDIT.
import type { RequestOptions } from "../transport.js";
import { Transport } from "../transport.js";
${imports ? `import type { ${imports} } from "./types.js";\n` : ""}
export class PrayerApi {
  constructor(private readonly transport: Transport) {}
${operations.join("\n")}
}
`;
writeFileSync(destination, output);
