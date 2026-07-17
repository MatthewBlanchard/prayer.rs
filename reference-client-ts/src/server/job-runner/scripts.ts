import type { JobConfig } from "./types.js";

const token = (name: string, value: unknown): string => {
  const text = String(value ?? "").trim();
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]*$/.test(text)) throw new Error(`${name} must be an explicit literal PrayerLang identifier`);
  return text;
};

export function scriptFor(config: JobConfig): string {
  switch (config.kind) {
    case "navigate":
      return `go ${token("destination", config["destination"])};`;
    case "mine": {
      const ore = token("resourceId", config["resourceId"]);
      const miningStep = config["miningPoi"] ? `go ${token("miningPoi", config["miningPoi"])};\n` : "";
      if (config["disposition"] === "space") return `${miningStep}mine ${ore};\ntransfer from cargo to space;\n`;
      const destination = token("destinationPoi", config["destinationPoi"]);
      const transfer = config["storageTarget"] === "faction" ? "transfer from cargo to faction;" : "transfer from cargo to storage;";
      return `${miningStep}mine ${ore};\ngo ${destination};\n${transfer}\nrefuel;\n`;
    }
    case "script":
      return String(config["script"]);
    default:
      throw new Error(`${config.kind} uses the economy runner`);
  }
}
