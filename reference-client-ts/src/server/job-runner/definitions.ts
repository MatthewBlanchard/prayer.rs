import type { JobDefinition } from "./types.js";

const bots = { name: "botIds", label: "Bots", type: "text", required: true, description: "Stable bot IDs, comma separated" } as const;
export const JOB_DEFINITIONS: JobDefinition[] = [
  {
    kind: "mine",
    title: "Mine",
    description: "Mine a bounded quantity of a resource.",
    mode: "one_shot",
    fields: [
      bots,
      { name: "resourceId", label: "Resource ID", type: "text", required: true },
      { name: "quantity", label: "Quantity", type: "number", required: true },
      { name: "miningPoi", label: "Mining POI", type: "text", required: true, description: "Known POIs containing the selected resource." },
      { name: "destinationPoi", label: "Destination POI", type: "text" },
      { name: "storageTarget", label: "Store mined resources in", type: "text", required: true },
    ],
    defaults: { quantity: 100, disposition: "storage", storageTarget: "personal" },
    capabilities: ["halt_now"],
  },
  {
    kind: "navigate",
    title: "Go",
    description: "Send the squad to a destination.",
    mode: "one_shot",
    fields: [bots, { name: "destination", label: "Destination", type: "text", required: true }],
    defaults: {},
    capabilities: ["halt_now"],
  },
  {
    kind: "script",
    title: "Run Script",
    description: "Run PrayerLang directly on selected bots.",
    mode: "one_shot",
    fields: [bots, { name: "script", label: "PrayerLang", type: "textarea", required: true }, { name: "failFast", label: "Fail fast", type: "boolean" }],
    defaults: { failFast: true },
    capabilities: ["halt_now"],
  },
];
