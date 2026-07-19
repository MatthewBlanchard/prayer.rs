import { lazy, Suspense, useState } from "react";
import {
  BookOpen,
  Bot,
  ChartLine,
  Factory,
  Flag,
  Library,
  Map as MapIcon,
  MessagesSquare,
  Package,
  Rocket,
  Users,
  UserRoundCheck,
  type LucideIcon,
} from "lucide-react";
import JobsPanel from "./JobsPanel.js";
import SessionsPanel from "./SessionsPanel.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { useFleetSessions } from "./prayer/useFleetSessions.js";
import { clientPlugins } from "./plugins.js";
import type { SidebarPanel } from "../plugin-api/client.js";

const CatalogPanel = lazy(() => import("./CatalogPanel.js"));
const CraftingPanel = lazy(() => import("./CraftingPanel.js"));
const EconomyPanel = lazy(() => import("./EconomyPanel.js"));
const FacilitiesPanel = lazy(() => import("./FacilitiesPanel.js"));
const FactionPanel = lazy(() => import("./FactionPanel.js"));
const GalaxyMap = lazy(() => import("./GalaxyMap.js"));
const GameChatPanel = lazy(() => import("./GameChatPanel.js"));
const PassengersPanel = lazy(() => import("./PassengersPanel.js"));
const QuartermasterPanel = lazy(() => import("./QuartermasterPanel.js"));
const ShipyardPanel = lazy(() => import("./ShipyardPanel.js"));
const SkillsPanel = lazy(() => import("./SkillsPanel.js"));
const SocialPanel = lazy(() => import("./SocialPanel.js"));
const StoragePanel = lazy(() => import("./StoragePanel.js"));

type ActiveView =
  | "game-chat"
  | "jobs"
  | "skills"
  | "galaxy"
  | "economy"
  | "quartermaster"
  | "crafting"
  | "storage"
  | "passengers"
  | "facilities"
  | "shipyard"
  | "social"
  | "faction"
  | "catalog"
  | `plugin:${string}`;

type SidebarItem = { type: "item"; view: ActiveView; label: string; icon: LucideIcon } | { type: "separator"; id: string };

const sidebarItems: SidebarItem[] = [
  { type: "item", view: "galaxy", label: "Galaxy map", icon: MapIcon },
  { type: "item", view: "jobs", label: "Squads", icon: Bot },
  { type: "separator", id: "operations" },
  { type: "item", view: "economy", label: "Economy", icon: ChartLine },
  { type: "item", view: "storage", label: "Storage", icon: Package },
  { type: "item", view: "passengers", label: "Passengers", icon: UserRoundCheck },
  { type: "item", view: "facilities", label: "Facilities", icon: Factory },
  { type: "item", view: "shipyard", label: "Shipyard", icon: Rocket },
  { type: "item", view: "skills", label: "Skills", icon: BookOpen },
  { type: "separator", id: "people" },
  { type: "item", view: "social", label: "Social", icon: Users },
  { type: "item", view: "game-chat", label: "Universe chat", icon: MessagesSquare },
  { type: "item", view: "faction", label: "Faction", icon: Flag },
  { type: "separator", id: "reference" },
  { type: "item", view: "catalog", label: "Catalog", icon: Library },
];

const pluginPanels = clientPlugins
  .flatMap((plugin) =>
    (plugin.panels ?? []).map((panel: SidebarPanel) => ({
      ...panel,
      view: `plugin:${plugin.id}:${panel.id}` as const,
    })),
  )
  .sort((a, b) => (a.order ?? 0) - (b.order ?? 0) || a.label.localeCompare(b.label));

export default function App() {
  const prayer = usePrayer();
  const { sessions, setSessions, systemEmpires } = useFleetSessions(prayer.bots, prayer.galaxyMap);
  const [activeView, setActiveView] = useState<ActiveView>("galaxy");
  const [requestedJobRunId, setRequestedJobRunId] = useState<string | null>(null);
  const [requestedJobSquadId, setRequestedJobSquadId] = useState<string | null>(null);
  const [jobNavigationRequest, setJobNavigationRequest] = useState(0);

  async function handleHaltSessionScript(handle: string) {
    setSessions((previous) => previous.map((session) => (session.sessionHandle === handle ? { ...session, runningScript: null } : session)));
    await (await prayer.bot(handle)).halt("halted from client");
    await prayer.refresh();
  }

  const status = prayer.connection === "connected" ? "Ready" : (prayer.error?.message ?? "Connecting...");

  return (
    <div className="app">
      <header className="app-header">
        <span className="app-title">NavCom</span>
        <span className="app-status">{status}</span>
      </header>
      <div className="app-body">
        <nav className="app-sidebar" aria-label="Primary views">
          {sidebarItems.map((item) => {
            if (item.type === "separator") return <div key={item.id} className="app-sidebar-separator" role="separator" />;
            const Icon = item.icon;
            return (
              <button
                key={item.view}
                className="app-sidebar-btn"
                data-active={activeView === item.view}
                onClick={() => setActiveView(item.view)}
                title={item.label}
                aria-label={item.label}
              >
                <Icon aria-hidden="true" strokeWidth={1.8} />
              </button>
            );
          })}
          {pluginPanels.length ? <div className="app-sidebar-separator" role="separator" /> : null}
          {pluginPanels.map((panel) => {
            const Icon = panel.icon;
            return (
              <button
                key={panel.view}
                className="app-sidebar-btn"
                data-active={activeView === panel.view}
                onClick={() => setActiveView(panel.view)}
                title={panel.label}
                aria-label={panel.label}
              >
                <Icon aria-hidden="true" strokeWidth={1.8} />
              </button>
            );
          })}
        </nav>
        <div className="app-main">
          <Suspense fallback={<div className="app-view-loading">Loading view…</div>}>
            {activeView === "game-chat" ? (
              <GameChatPanel sessions={sessions} />
            ) : activeView === "jobs" ? (
              <JobsPanel
                sessions={sessions}
                requestedRunId={requestedJobRunId}
                requestedSquadId={requestedJobSquadId}
                navigationRequest={jobNavigationRequest}
              />
            ) : activeView === "skills" ? (
              <SkillsPanel sessions={sessions} />
            ) : activeView === "galaxy" ? (
              <GalaxyMap sessions={sessions} />
            ) : activeView === "economy" ? (
              <EconomyPanel sessions={sessions} />
            ) : activeView === "quartermaster" ? (
              <QuartermasterPanel sessions={sessions} />
            ) : activeView === "crafting" ? (
              <CraftingPanel sessions={sessions} />
            ) : activeView === "storage" ? (
              <StoragePanel sessions={sessions} />
            ) : activeView === "passengers" ? (
              <PassengersPanel sessions={sessions} />
            ) : activeView === "facilities" ? (
              <FacilitiesPanel sessions={sessions} />
            ) : activeView === "shipyard" ? (
              <ShipyardPanel sessions={sessions} onChanged={prayer.refresh} />
            ) : activeView === "social" ? (
              <SocialPanel sessions={sessions} />
            ) : activeView === "faction" ? (
              <FactionPanel sessions={sessions} />
            ) : activeView.startsWith("plugin:") ? (
              (() => {
                const panel = pluginPanels.find((candidate) => candidate.view === activeView);
                return panel ? <panel.component sessions={sessions} /> : <div>Plugin panel unavailable.</div>;
              })()
            ) : (
              <CatalogPanel sessions={sessions} />
            )}
          </Suspense>
        </div>
        <SessionsPanel
          sessions={sessions}
          systemEmpires={systemEmpires}
          onHaltScript={handleHaltSessionScript}
          onRegistered={prayer.refresh}
          onOpenJob={(runId, squadId) => {
            setRequestedJobRunId(runId);
            setRequestedJobSquadId(squadId);
            setJobNavigationRequest((current) => current + 1);
            setActiveView("jobs");
          }}
        />
      </div>
    </div>
  );
}
