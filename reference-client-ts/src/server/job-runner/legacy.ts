import fs from "fs/promises";

export async function archiveLegacyJobs(filePath: string, now = new Date()): Promise<string | undefined> {
  const archivedPath = `${filePath}.archived-${now.toISOString().replace(/[:.]/g, "-")}`;
  try {
    await fs.rename(filePath, archivedPath);
    return archivedPath;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw error;
  }
}
