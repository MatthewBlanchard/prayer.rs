import type { ComponentType } from "react";
import type { LucideIcon } from "lucide-react";
import type { JobRun } from "../shared/types.js";

export type SidebarPanelProps = { sessions: Array<{ sessionHandle: string; botId?: string | null; username?: string | null }> };
export type SidebarPanel = {
  id: string;
  label: string;
  icon: LucideIcon;
  order?: number;
  component: ComponentType<SidebarPanelProps>;
};
export type JobRunViewProps = { run: JobRun };
export type JobRunView = { kind: string; component: ComponentType<JobRunViewProps> };
export type ClientPlugin = { panels?: SidebarPanel[]; runViews?: JobRunView[] };
