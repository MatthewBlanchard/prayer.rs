import fs from "node:fs/promises";
import path from "node:path";

export type Squad = { id: string; name: string; color: string; priority: number; botIds: string[]; createdAt: string; updatedAt: string };

export class SquadStore {
  private squads: Squad[] = [];
  constructor(
    private readonly filePath: string,
    private readonly legacyJobsPath: string,
  ) {}

  async load(): Promise<void> {
    try {
      this.squads = normalizeSquads(JSON.parse(await fs.readFile(this.filePath, "utf8")));
      return;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    this.squads = await this.loadLegacySquads();
    if (this.squads.length) await this.save();
  }
  list(): Squad[] {
    return [...this.squads].sort((a, b) => b.priority - a.priority || a.name.localeCompare(b.name));
  }
  get(id: string): Squad | undefined {
    return this.squads.find((squad) => squad.id === id);
  }
  async create(input: Partial<Squad>): Promise<Squad> {
    const now = new Date().toISOString();
    const squad: Squad = {
      id: `squad_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`,
      name: cleanName(input.name) ?? `Squad ${this.squads.length + 1}`,
      color: cleanColor(input.color) ?? "#5f9fed",
      priority: Number.isFinite(input.priority) ? Number(input.priority) : this.squads.length,
      botIds: cleanBotIds(input.botIds),
      createdAt: now,
      updatedAt: now,
    };
    this.squads.push(squad);
    await this.save();
    return squad;
  }
  async update(id: string, input: Partial<Squad>): Promise<Squad> {
    const squad = this.get(id);
    if (!squad) throw new Error("squad not found");
    if (input.name !== undefined) squad.name = cleanName(input.name) ?? squad.name;
    if (input.color !== undefined) squad.color = cleanColor(input.color) ?? squad.color;
    if (input.priority !== undefined && Number.isFinite(input.priority)) squad.priority = Number(input.priority);
    if (input.botIds !== undefined) squad.botIds = cleanBotIds(input.botIds);
    squad.updatedAt = new Date().toISOString();
    await this.save();
    return squad;
  }
  async delete(id: string): Promise<void> {
    const next = this.squads.filter((squad) => squad.id !== id);
    if (next.length === this.squads.length) throw new Error("squad not found");
    this.squads = next;
    await this.save();
  }
  private async save(): Promise<void> {
    await fs.mkdir(path.dirname(this.filePath), { recursive: true });
    const temporary = `${this.filePath}.${process.pid}.tmp`;
    await fs.writeFile(temporary, JSON.stringify({ schemaVersion: 1, squads: this.squads }, null, 2) + "\n");
    await fs.rename(temporary, this.filePath);
  }
  private async loadLegacySquads(): Promise<Squad[]> {
    const directory = path.dirname(this.legacyJobsPath);
    const prefix = `${path.basename(this.legacyJobsPath)}.archived-`;
    try {
      const candidates = (await fs.readdir(directory))
        .filter((name) => name.startsWith(prefix))
        .sort()
        .reverse();
      for (const candidate of candidates) {
        const parsed = JSON.parse(await fs.readFile(path.join(directory, candidate), "utf8")) as Record<string, unknown>;
        const jobs = Array.isArray(parsed["jobs"]) ? parsed["jobs"] : Array.isArray(parsed) ? parsed : [];
        const squads = normalizeSquads({ squads: jobs });
        if (squads.length) return squads;
      }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") console.warn("[squads] legacy migration failed", error);
    }
    return [];
  }
}

function normalizeSquads(value: unknown): Squad[] {
  const root = value && typeof value === "object" ? (value as Record<string, unknown>) : {};
  const rows = Array.isArray(root["squads"]) ? root["squads"] : [];
  return rows.flatMap((row, index) => {
    if (!row || typeof row !== "object") return [];
    const data = row as Record<string, unknown>;
    const now = new Date().toISOString();
    return [
      {
        id: typeof data["id"] === "string" ? data["id"] : `squad_migrated_${index}`,
        name: cleanName(data["name"]) ?? `Squad ${index + 1}`,
        color: cleanColor(data["color"]) ?? "#5f9fed",
        priority: Number.isFinite(data["priority"]) ? Number(data["priority"]) : 0,
        botIds: cleanBotIds(data["botIds"] ?? data["sessionHandles"]),
        createdAt: typeof data["createdAt"] === "string" ? data["createdAt"] : now,
        updatedAt: typeof data["updatedAt"] === "string" ? data["updatedAt"] : now,
      },
    ];
  });
}
const cleanName = (value: unknown) => (typeof value === "string" && value.trim() ? value.trim().slice(0, 80) : undefined);
const cleanColor = (value: unknown) => (typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value) ? value : undefined);
const cleanBotIds = (value: unknown) =>
  Array.isArray(value)
    ? [
        ...new Set(
          value
            .map(String)
            .map((id) => id.trim())
            .filter(Boolean),
        ),
      ]
    : [];
