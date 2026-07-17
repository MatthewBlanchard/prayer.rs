import { cpSync, mkdtempSync, mkdirSync, writeFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const sdk = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const reference = resolve(sdk, "../reference-client-ts");
const temp = mkdtempSync(join(tmpdir(), "prayer-sdk-package-"));
const cache = join(temp, "npm-cache");
const run = (command, args, cwd) => {
  const result = spawnSync(command, args, { cwd, stdio: "inherit", shell: process.platform === "win32" });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed with ${result.status}`);
};

try {
  run("npm", ["run", "build"], sdk);
  run("npm", ["pack", "--pack-destination", temp, "--cache", cache], sdk);
  const tarball = join(temp, "prayer-sdk-0.1.0-alpha.0.tgz");
  const fixture = join(temp, "consumer");
  mkdirSync(fixture);
  writeFileSync(join(fixture, "package.json"), JSON.stringify({ private: true, type: "module" }));
  run("npm", ["install", "--ignore-scripts", "--package-lock=false", "--cache", cache, tarball], fixture);
  writeFileSync(join(fixture, "node.mjs"), 'import { Prayer } from "@prayer/sdk";\nif (typeof Prayer.connect !== "function") throw new Error("root import failed");\n');
  run("node", ["node.mjs"], fixture);
  writeFileSync(join(fixture, "types.ts"), 'import { Prayer, wait, type ActionRun } from "@prayer/sdk";\nimport type { StateResponse } from "@prayer/sdk/types";\nimport { PrayerApi } from "@prayer/sdk/api";\nconst action = wait(1);\nconst connect = Prayer.connect;\nconst api: typeof PrayerApi = PrayerApi;\nconst consume = (run: ActionRun, state: StateResponse) => [run.id, state, action, connect, api] as const;\nvoid consume;\n');
  run(resolve(sdk, "node_modules/.bin/tsc"), ["--noEmit", "--strict", "--target", "ES2022", "--module", "NodeNext", "--moduleResolution", "NodeNext", "types.ts"], fixture);
  writeFileSync(join(fixture, "browser.ts"), 'import { Prayer, wait } from "@prayer/sdk";\nexport const connect = Prayer.connect;\nexport const action = wait(1);\n');
  writeFileSync(join(fixture, "index.html"), '<script type="module" src="/browser.ts"></script>');
  run(resolve(reference, "node_modules/.bin/vite"), ["build", "--outDir", "browser-dist"], fixture);

  // Prove the repository consumer builds against the same artifact users install.
  const referenceSdk = join(reference, "node_modules/@prayer/sdk");
  rmSync(referenceSdk, { recursive: true, force: true });
  cpSync(join(fixture, "node_modules/@prayer/sdk"), referenceSdk, { recursive: true });
  try { run("npm", ["run", "build"], reference); }
  finally {
    rmSync(referenceSdk, { recursive: true, force: true });
    symlinkSync(sdk, referenceSdk, process.platform === "win32" ? "junction" : "dir");
  }
} finally {
  rmSync(temp, { recursive: true, force: true });
}
