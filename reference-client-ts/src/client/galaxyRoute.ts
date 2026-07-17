export function activeRoutePath(start: string | null, hops: string[], knownSystemIds: ReadonlySet<string>): string[] {
  if (!start || hops.length === 0) return [];
  const path = [start, ...hops.filter((hop) => hop !== start)].filter((systemId) => knownSystemIds.has(systemId));
  return path.length >= 2 ? path : [];
}
