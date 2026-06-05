export type ShellSurface =
  | "panel"
  | "dock"
  | "dock-menu"
  | "sidebar"
  | "quick-settings"
  | "date-center"
  | "notification-toast"
  | "overview";

export type ShellSnapshot = {
  surface?: ShellSurface;
  time: string;
  date: string;
  activeWorkspace: string;
  activeProfile: string;
  activeMode: string;
  panelTaskbar: boolean;
  blurEnabled: boolean;
  debugOverlay: boolean;
  safeMode: boolean;
  chromeHidden: boolean;
  wallpaperUri?: string;
  palette: ShellPalette;
  profiles: ProfileItem[];
  workspaces: WorkspaceItem[];
  windows: WindowItem[];
  dockApps: DockApp[];
  dockMenuCommand?: string;
  dockMenuX?: number;
  applications: ApplicationItem[];
  status: SystemStatus;
  tray: TrayItem[];
  doNotDisturb: boolean;
  notifications: NotificationItem[];
  toastNotifications: NotificationItem[];
};

export type ShellPalette = {
  panel: string;
  panelControl: string;
  panelText: string;
  dock: string;
  accent: string;
};

export type WorkspaceItem = {
  id: string;
  name: string;
  profile: string;
  mode: string;
  active: boolean;
};

export type ProfileItem = {
  id: string;
  name: string;
  mode: string;
  active: boolean;
};

export type WindowItem = {
  id: number;
  title: string;
  appId?: string;
  iconUri?: string;
  workspace: string;
  geometry: Geometry;
  active: boolean;
  visible: boolean;
};

export type DockApp = {
  label: string;
  command: string;
  iconUri?: string;
  running: boolean;
  active: boolean;
};

export type ApplicationItem = {
  name: string;
  command: string;
  comment?: string;
  icon?: string;
  iconUri?: string;
  pinned: boolean;
};

export type Geometry = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type SystemStatus = {
  network?: { name: string; wireless: boolean };
  audio?: { percent: number; muted: boolean };
  brightness?: { percent: number };
  battery?: { percent: number; state: string };
};

export type TrayItem = {
  title: string;
  iconUri?: string;
  status: "passive" | "active" | "needs-attention";
};

export type NotificationItem = {
  id: number;
  appName: string;
  iconUri?: string;
  receivedAt: number;
  summary: string;
  body: string;
  urgency: "low" | "normal" | "critical";
  actions: { key: string; label: string }[];
};

export type ShellAction =
  | { type: "open-launcher" }
  | { type: "launch-default-app"; app: "terminal" | "file-manager" | "browser" | "settings" }
  | { type: "toggle-overview" }
  | { type: "toggle-quick-settings" }
  | { type: "toggle-date-center" }
  | { type: "toggle-shell-style" }
  | { type: "workspace-switch"; workspace: string }
  | { type: "workspace-relative"; offset: number }
  | { type: "workspace-new" }
  | { type: "workspace-set-profile"; profile: string }
  | { type: "window-activate"; window: number }
  | { type: "window-move"; window: number; workspace: string }
  | { type: "dock-launch"; command: string }
  | { type: "dock-menu-open"; command: string; x?: number }
  | { type: "dock-menu-close" }
  | { type: "dock-pin"; label: string; command: string; icon?: string }
  | { type: "dock-unpin"; command: string }
  | { type: "app-launch"; command: string }
  | { type: "tray-activate"; index: number }
  | { type: "tray-menu"; index: number }
  | { type: "quick-open-settings"; page: "network" | "audio" | "power" }
  | { type: "quick-set-volume"; percent: number }
  | { type: "quick-toggle-mute" }
  | { type: "quick-set-brightness"; percent: number }
  | { type: "quick-toggle-debug-overlay" }
  | { type: "session-command"; command: "lock" | "suspend" | "reboot" | "power-off" }
  | { type: "reload-config" }
  | { type: "open-logs-folder" }
  | { type: "toggle-safe-mode" }
  | { type: "notification-close"; notification: number }
  | { type: "notification-clear-all" }
  | { type: "notification-do-not-disturb"; enabled: boolean }
  | { type: "notification-action"; notification: number; action: string };

export const emptySnapshot = (): ShellSnapshot => {
  const now = new Date();
  return {
    time: now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
    date: now.toLocaleDateString([], {
      weekday: "long",
      month: "long",
      day: "numeric",
    }),
    activeWorkspace: "1",
    activeProfile: "panel-default",
    activeMode: "panel",
    panelTaskbar: true,
    blurEnabled: true,
    debugOverlay: false,
    safeMode: false,
    chromeHidden: false,
    wallpaperUri: undefined,
    palette: {
      panel: "rgba(22, 22, 20, 0.62)",
      panelControl: "rgba(255, 255, 255, 0.08)",
      panelText: "rgba(248, 248, 246, 0.96)",
      dock: "rgba(24, 23, 20, 0.34)",
      accent: "rgb(216, 162, 24)",
    },
    profiles: [],
    workspaces: [],
    windows: [],
    dockApps: [],
    dockMenuCommand: undefined,
    dockMenuX: undefined,
    applications: [],
    status: {},
    tray: [],
    doNotDisturb: false,
    notifications: [],
    toastNotifications: [],
  };
};
