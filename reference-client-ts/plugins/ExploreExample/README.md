# Explore example plugin

This trusted, jobs-only example continuously spreads a squad across reachable systems. Rebuild and restart the reference client after installing or changing it.

Target selection is automatic: the job chooses the nearest reachable incomplete or never-visited system outside the stronghold exclusion area. When no such target remains, it chooses the system with the oldest effective system/POI visit. `strongholdExclusionHops` defaults to 3 and acts as the exploration blacklist around known strongholds. `idleDelayMs` defaults to 1000.

After entering a target, the job surveys exactly once when the active ship's equipped `state.modules` contains a survey scanner, then visits known POIs with unvisited POIs first. It uses `survey`, never the combat `scan` action, and consumes refreshed canonical map fields including newly revealed POIs, faint signatures, and wildlife.

Plugins execute as trusted server code. Review them before installation.
