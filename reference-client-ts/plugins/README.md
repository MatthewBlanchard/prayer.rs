# Application plugins

Each child directory is a trusted, rebuild-time plugin. A plugin contains a `plugin.json` manifest and may expose jobs from its server entry and left-sidebar panels from its client entry.

The public contracts live in `src/plugin-api`. Plugin code may import those contracts and `@prayer/sdk`; it must not import host implementation modules. IDs and job kinds must be unique. The current host API version is `1`.

Installing, updating, or removing a plugin requires rebuilding and restarting the application. Persisted run history remains readable after removal; active runs belonging to a removed plugin are marked interrupted during recovery.

See [ExploreExample](./ExploreExample/README.md) for the minimal jobs-only example. TypeScript plugin server entries are typechecked and emitted as part of the production build.
