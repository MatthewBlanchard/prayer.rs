# Autonomous Agent Mode — Invokable Skills Plan

## Goal

Let agents self-direct by discovering and invoking **skills** — the same model Claude Code uses.
Skill descriptions are always visible to the agent; the agent reads and invokes the full body of a
skill when it decides it's relevant. No pre-injection, no config files selecting what gets loaded.

---

## Concept

### How Claude Code skills work (the model we're copying)

1. Skill descriptions are always in context — the model sees `name` + `description` for every skill.
2. Full skill content is only loaded when invoked.
3. The model invokes a skill by choosing to when the situation fits.
4. Users can also invoke directly (`/skill-name`).

### How this maps to player agents

1. On startup, the agent's system prompt includes a `## Available Skills` section listing every
   non-dot skill by name and description.
2. The agent has a `read_skill(name)` tool that returns the full body of a named skill.
3. The agent calls `read_skill` when it decides a skill is relevant — the body comes back as a tool
   result and the agent follows it.
4. The agent self-directs from the start: no objective required, no config selecting which skills
   are active.

If the `agent-skills/` directory is empty or absent → **directed mode**: agent starts paused,
waits for a human objective. Existing behavior fully preserved.

---

## File Layout

```
agent-skills/
  mining.md           ← discoverable skill (no leading dot)
  fuel_management.md
  trading.md
  .Krag.md            ← dot-prefixed: NOT listed in discovery, NOT auto-invokable
  .Berserker.md       ← dot-prefixed: same — must be explicitly named in a nudge/objective
```

No JSON config files. The presence of non-dot `.md` files in the directory is sufficient.

The directory is configurable via `--agent-skills-dir` / `PRAYER_MCP_CLIENT_SKILLS_DIR`,
defaulting to `./agent-skills`.

---

## Skill File Format

```markdown
---
name: mining
description: Mine resources continuously, managing cargo and fuel. Use when the primary goal is resource accumulation.
---

Mine resources continuously. When cargo is full, stash to storage immediately.
Check fuel before each mining run; refuel if below 30%.
Prefer iron_ore unless a mission specifies otherwise.
```

Only `name` and `description` are required in frontmatter. The body is returned verbatim on invocation.

---

## System Prompt Composition

```
You are an autonomous agent managing SpaceMolt player "{sessionHandle}".
Your tools are pre-scoped to your session — do not include session_handle in any tool arguments.
At the start of a new turn, call fs_ls on / to get your bearings before choosing actions.
When running a script, pass only raw PrayerLang script text — no markdown fences, no prose.
Use read_skill to load the full instructions for a skill before acting on it.
After completing a skill's goal, re-evaluate which skill to invoke next.

## Available Skills

- **mining**: Mine resources continuously, managing cargo and fuel. Use when the primary goal is resource accumulation.
- **fuel_management**: Monitor and maintain fuel levels across runs.
- **trading**: Sell cargo at market, evaluate prices, manage credits.

## MCP Reference
{dslReference}
```

The `## Available Skills` section is generated from the discovered non-dot skill files.
No skill bodies in the system prompt — just names and descriptions.

---

## Implementation Plan

### 1. Skill discovery (`src/server/agent_skills.ts`)

```ts
type SkillMeta  = { name: string; description: string };
type SkillFull  = SkillMeta & { body: string };

// Returns all non-dot-prefixed skills found in dir. Returns [] if dir missing.
async function discoverSkills(dir: string): Promise<SkillMeta[]>

// Returns full body of a single named skill, or null if not found.
async function readSkill(name: string, dir: string): Promise<SkillFull | null>
```

Discovery logic:
- Read all `.md` files in `dir` (non-recursive, top-level only).
- Skip files whose basename starts with `.` (dot-prefixed = not discoverable).
- Parse frontmatter for `name` and `description`; skip malformed files with a warning.
- Return sorted by filename for deterministic ordering.

### 2. `read_skill` server-side tool

Add a new tool to the server's local tool registry (not MCP — this is a server-internal tool the
agent loop can call). The `ChatToolLoop` already has an extension point for synthetic tools
(`includeSyntheticReadResource`). Follow the same pattern.

```
Tool: read_skill
Input: { name: string }
Output: full skill body as text, or an error message if the skill doesn't exist
```

The tool reads from `agentSkillsDir` at call time — always fresh, no caching needed.
Dot-prefixed files are **not** resolvable by name through this tool (return "skill not found").

### 3. Extend `PlayerAgent`

Constructor gains `skillMetas: SkillMeta[]` (empty array = directed mode):
- Empty → directed mode (starts paused, no `## Available Skills` section).
- Non-empty → autonomous mode, skip initial pause, inject skills listing into system prompt.

```ts
function buildAutonomousSystemPrompt(
  sessionHandle: string,
  dslReference: string | undefined,
  skills: SkillMeta[]
): string
```

Add `setSkillMetas(skills: SkillMeta[])` for hot-reload — same pause/rebuild/resume pattern.

`isAutonomous(): boolean` → `this.skillMetas.length > 0`.

### 4. Filesystem watcher in `PlayerAgentManager`

Watch `agentSkillsDir` with `fs.watch`. On any change, re-run discovery and push updated
`SkillMeta[]` to all running agents via `setSkillMetas`. Debounce ~300ms.

The `read_skill` tool always reads from disk at call time so hot-reloaded bodies are picked up
automatically without needing to notify the agent.

### 5. `spawnAgent` update

```ts
private async spawnAgent(handle: string): Promise<void> {
  const skills = await discoverSkills(this.agentSkillsDir);
  const agent = new PlayerAgent(handle, this.provider, this.sharedMcp,
                                this.model, this.dslReference, this.onEvent, skills);
  // ...
}
```

### 6. "Open skills folder" endpoint

```
GET /api/agents/skills/open
Response: { ok: true, path: "/absolute/path/to/agent-skills" }
```

Runs `xdg-open` / `open` / `explorer` on the resolved `agentSkillsDir`.

### 7. `listAgents` — add `mode`

```ts
{ sessionHandle: string; paused: boolean; mode: "autonomous" | "directed" }
```

### 8. UI changes

- **"Open skills folder"** button (📂) in agents panel header. Tooltip shows resolved path.
- Mode badge on each card: `auto` (green) / `directed` (gray).
- Keep the "set objective" (💬) button in autonomous mode as an optional nudge — appends a user
  message that the agent can act on before continuing its loop.
- No inline textarea anywhere.

---

## Dot-Prefix Convention

Dot-prefixed files (`.Krag.md`, `.Berserker.md`) are **character/role overrides**:
- Not returned by `discoverSkills` — never appear in `## Available Skills`.
- Not resolvable via `read_skill` by default.
- Intended to be loaded by giving the agent an explicit objective/nudge that names the skill,
  or via a future "assign character" UI action that pushes the content directly.
- Safe to have in the folder without affecting default behavior.

---

## Edge Cases

- **`agent-skills/` missing or empty**: `discoverSkills` returns `[]` → all agents directed mode.
- **Skill file has no description**: skip from discovery with a warning (agent can't know when to use it).
- **Agent invokes `read_skill` for unknown name**: tool returns an error message; agent recovers and tries another approach.
- **Skill body edited while agent is mid-run**: next `read_skill` call for that skill gets the fresh body; no coordination needed.
- **All skill files deleted**: watcher fires, `discoverSkills` returns `[]`, agents switch to directed mode and pause.
- **`setObjective()` on autonomous agent**: appends a user message, agent acts on it and continues its loop.
- **Compaction**: system message (`messages[0]`) is outside the compaction window — skill listing is preserved across context compression.
- **`fs.watch` unavailable**: log warning, skip watcher, rely on server restart for changes.

---

## What Does NOT Change

- `ChatToolLoop` / `tool_loop.ts` — minimal addition of the `read_skill` synthetic tool.
- `SessionScopedMcpProxy` — untouched.
- Pause/resume/stop controls — same behavior.
- Directed-mode agents — fully preserved.
- The commander chat (`/api/chat`) — unrelated.

---

## File Changelist

| File | Change |
|---|---|
| `src/server/agent_skills.ts` | New — `discoverSkills()`, `readSkill()`, frontmatter parser |
| `src/server/player_agent.ts` | `skillMetas` param, `buildAutonomousSystemPrompt`, `setSkillMetas()`, `isAutonomous()`, skip pause when skills present, wire `read_skill` synthetic tool |
| `src/server/index.ts` | Pass `agentSkillsDir` to manager; `GET /api/agents/skills/open`; `mode` in `listAgents` |
| `src/client/AgentsPanel.tsx` | "Open skills folder" button; `auto`/`directed` badge |
| `src/client/api.ts` | `openSkillsFolder()` |
| `src/shared/types.ts` | Add `mode` to `AgentInfo` |

---

## Smallest Shippable Slice

1. `agent_skills.ts` — `discoverSkills` + `readSkill`.
2. `player_agent.ts` — `skillMetas` param, `buildAutonomousSystemPrompt`, `read_skill` synthetic tool.
3. `index.ts` — pass `agentSkillsDir`, `open` endpoint, `mode` in list.

Drop a `.md` file in `agent-skills/` and agents start running immediately with that skill available.

---

## Acceptance Criteria

- Add a skill file → all agents pick it up on next watcher fire; it appears in their `## Available Skills`.
- Agent calls `read_skill("mining")` → gets full skill body back as tool result.
- Agent calls `read_skill(".Krag")` → gets "skill not found" (dot-prefixed not resolvable).
- Edit a skill body → next agent invocation of that skill gets the updated content.
- Empty `agent-skills/` → all agents in directed mode, unchanged from today.
- No JSON config files anywhere.
