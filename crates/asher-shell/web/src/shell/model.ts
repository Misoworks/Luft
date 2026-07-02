export type ShellSurface =
  | "panel"
  | "panel-menu"
  | "quick-settings"
  | "date-center"
  | "notification-toast"
  | "start-menu";

export type ShellSnapshot = {
  surface?: ShellSurface;
  time: string;
  date: string;
  activeWorkspace: string;
  userProfileIconUri?: string;
  palette: ShellPalette;
  workspaces: WorkspaceItem[];
  windows: WindowItem[];
  panelApps: PanelApp[];
  panelMenuCommand?: string;
  panelMenuX?: number;
  applications: ApplicationItem[];
  status: SystemStatus;
  tray: TrayItem[];
  doNotDisturb: boolean;
  notifications: NotificationItem[];
  toastNotifications: NotificationItem[];
  startMenuOpen: boolean;
  quickSettingsOpen: boolean;
  dateCenterOpen: boolean;
};

export type ShellPalette = {
  panel: string;
  panelControl: string;
  panelText: string;
  panelBar: string;
  accent: string;
  textSoft: string;
  textMuted: string;
};

export type WorkspaceItem = {
  id: string;
  name: string;
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

export type PanelApp = {
  label: string;
  command: string;
  iconUri?: string;
  running: boolean;
  active: boolean;
  pinned: boolean;
  windowId?: number;
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
  | { type: "toggle-start-menu" }
  | { type: "close-start-menu" }
  | { type: "toggle-quick-settings" }
  | { type: "close-quick-settings" }
  | { type: "toggle-date-center" }
  | { type: "close-date-center" }
  | { type: "workspace-switch"; workspace: string }
  | { type: "workspace-relative"; offset: number }
  | { type: "workspace-new" }
  | { type: "window-activate"; window: number }
  | { type: "window-close"; window: number }
  | { type: "window-minimize"; window: number }
  | { type: "window-move"; window: number; workspace: string }
  | { type: "panel-launch"; command: string }
  | { type: "panel-menu-open"; command: string; x?: number }
  | { type: "panel-menu-close" }
  | { type: "panel-pin"; label: string; command: string; icon?: string }
  | { type: "panel-unpin"; command: string }
  | { type: "panel-force-quit"; command: string }
  | { type: "panel-reorder"; commands: string[] }
  | { type: "app-launch"; command: string }
  | { type: "tray-activate"; index: number }
  | { type: "tray-menu"; index: number }
  | { type: "quick-open-settings"; page: "appearance" | "network" | "audio" | "power" }
  | { type: "quick-set-volume"; percent: number }
  | { type: "quick-toggle-mute" }
  | { type: "quick-set-brightness"; percent: number }
  | { type: "session-command"; command: "lock" | "suspend" | "reboot" | "power-off" }
  | { type: "reload-config" }
  | { type: "open-logs-folder" }
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
    palette: {
      panel: "rgba(22, 22, 20, 0.62)",
      panelControl: "rgba(255, 255, 255, 0.08)",
      panelText: "rgba(248, 248, 246, 0.96)",
      panelBar: "rgba(24, 23, 20, 0.34)",
      accent: "rgba(210, 192, 130, 1)",
      textSoft: "rgba(218, 216, 205, 0.91)",
      textMuted: "rgba(164, 162, 154, 0.87)",
    },
    workspaces: [],
    windows: [],
    panelApps: [],
    panelMenuCommand: undefined,
    panelMenuX: undefined,
    applications: [],
    status: {},
    tray: [],
    doNotDisturb: false,
    notifications: [],
    toastNotifications: [],
    startMenuOpen: false,
    quickSettingsOpen: false,
    dateCenterOpen: false,
  };
};
