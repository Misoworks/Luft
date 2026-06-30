import type { ApplicationItem, ProfileItem, ShellAction, ShellSnapshot, WindowItem, WorkspaceItem } from "../shell/model";

type StartMenuCommand = {
  title: string;
  detail: string;
  icon: string;
  label: string;
  keywords: string[];
  action: ShellAction;
};

export type StartMenuSearchResult =
  | {
      kind: "app";
      key: string;
      title: string;
      detail: string;
      iconUri?: string;
      app: ApplicationItem;
    }
  | {
      kind: "window";
      key: string;
      title: string;
      detail: string;
      iconUri?: string;
      window: WindowItem;
    }
  | {
      kind: "workspace";
      key: string;
      title: string;
      detail: string;
      workspace: WorkspaceItem;
    }
  | {
      kind: "profile";
      key: string;
      title: string;
      detail: string;
      profile: ProfileItem;
    }
  | {
      kind: "action";
      key: string;
      title: string;
      detail: string;
      icon: string;
      label: string;
      action: ShellAction;
    };

export function filteredApplications(snapshot: ShellSnapshot, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return snapshot.applications;
  return snapshot.applications.filter((app) =>
    [app.name, app.comment ?? "", app.command].some((value) => value.toLowerCase().includes(needle)),
  );
}

export function startMenuSearchResults(snapshot: ShellSnapshot, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return [];

  const actions = startMenuCommands(snapshot)
    .filter((command) => searchable([command.title, command.detail, ...command.keywords], needle))
    .slice(0, 6)
    .map<StartMenuSearchResult>((command) => ({
      kind: "action",
      key: `action:${command.title}`,
      title: command.title,
      detail: command.detail,
      icon: command.icon,
      label: command.label,
      action: command.action,
    }));

  const apps = snapshot.applications
    .filter((app) => searchable([app.name, app.comment ?? "", app.command], needle))
    .slice(0, 8)
    .map<StartMenuSearchResult>((app) => ({
      kind: "app",
      key: `app:${app.command}`,
      title: app.name,
      detail: app.comment || app.command,
      iconUri: app.iconUri,
      app,
    }));

  const windows = snapshot.windows
    .filter((window) => window.visible && searchable([window.title, window.appId ?? "", window.workspace], needle))
    .slice(0, 6)
    .map<StartMenuSearchResult>((window) => ({
      kind: "window",
      key: `window:${window.id}`,
      title: window.title,
      detail: `${window.appId ?? "Window"} / Workspace ${window.workspace}`,
      iconUri: window.iconUri,
      window,
    }));

  const workspaces = snapshot.workspaces
    .filter((workspace) => searchable([workspace.name, workspace.profile, workspace.mode, workspace.id], needle))
    .slice(0, 4)
    .map<StartMenuSearchResult>((workspace) => ({
      kind: "workspace",
      key: `workspace:${workspace.id}`,
      title: workspace.name,
      detail: `${workspace.profile} / ${workspace.mode}`,
      workspace,
    }));

  const profiles = snapshot.profiles
    .filter((profile) => searchable([profile.name, profile.id, profile.mode], needle))
    .slice(0, 5)
    .map<StartMenuSearchResult>((profile) => ({
      kind: "profile",
      key: `profile:${profile.id}`,
      title: profile.name,
      detail: profile.active ? `${profile.id} / active on this workspace` : `${profile.id} / ${profile.mode}`,
      profile,
    }));

  return [...actions, ...apps, ...windows, ...workspaces, ...profiles];
}

export function selectedStartMenuResult(results: StartMenuSearchResult[], selection: number) {
  return results[Math.max(0, Math.min(selection, results.length - 1))];
}

function searchable(values: string[], needle: string) {
  return values.some((value) => value.toLowerCase().includes(needle));
}

function startMenuCommands(snapshot: ShellSnapshot): StartMenuCommand[] {
  return [
    {
      title: "Open Launcher",
      detail: "Vicinae app launcher",
      icon: "search",
      label: "Command",
      keywords: ["apps", "launcher", "vicinae", "search"],
      action: { type: "open-launcher" },
    },
    {
      title: "Open Terminal",
      detail: "Launch the configured terminal",
      icon: "terminal",
      label: "App",
      keywords: ["terminal", "shell", "console", "default app"],
      action: { type: "launch-default-app", app: "terminal" },
    },
    {
      title: "Open Files",
      detail: "Launch the configured file manager",
      icon: "files",
      label: "App",
      keywords: ["files", "folder", "nautilus", "file manager", "default app"],
      action: { type: "launch-default-app", app: "file-manager" },
    },
    {
      title: "Open Browser",
      detail: "Launch the configured browser",
      icon: "browser",
      label: "App",
      keywords: ["browser", "web", "chrome", "default app"],
      action: { type: "launch-default-app", app: "browser" },
    },
    {
      title: "Open Settings",
      detail: "Launch the configured system settings app",
      icon: "settings",
      label: "App",
      keywords: ["settings", "system", "control center", "default app"],
      action: { type: "launch-default-app", app: "settings" },
    },
    {
      title: "Quick Settings",
      detail: "Network, audio, power",
      icon: "settings",
      label: "Command",
      keywords: ["control", "settings", "wifi", "sound", "volume", "power"],
      action: { type: "toggle-quick-settings" },
    },
    {
      title: "Notifications",
      detail: "Calendar and notification center",
      icon: "bell",
      label: "Command",
      keywords: ["date", "calendar", "notification", "center"],
      action: { type: "toggle-date-center" },
    },
    {
      title: "New Workspace",
      detail: "Create or move to the next empty workspace",
      icon: "plus",
      label: "Workspace",
      keywords: ["workspace", "desktop", "new"],
      action: { type: "workspace-new" },
    },
    {
      title: "Appearance Settings",
      detail: "Panel, panel, blur, wallpaper, and shell presentation",
      icon: "palette",
      label: "Settings",
      keywords: ["panel", "mode", "style", "appearance", "wallpaper"],
      action: { type: "quick-open-settings", page: "appearance" },
    },
    {
      title: "Network Settings",
      detail: "Open system network settings",
      icon: "network",
      label: "Settings",
      keywords: ["wifi", "wired", "ethernet", "settings"],
      action: { type: "quick-open-settings", page: "network" },
    },
    {
      title: "Audio Settings",
      detail: "Open system sound settings",
      icon: "volume",
      label: "Settings",
      keywords: ["sound", "speaker", "microphone", "volume", "settings"],
      action: { type: "quick-open-settings", page: "audio" },
    },
    {
      title: "Power Settings",
      detail: "Open system power settings",
      icon: "power",
      label: "Settings",
      keywords: ["battery", "performance", "suspend", "settings"],
      action: { type: "quick-open-settings", page: "power" },
    },
    {
      title: "Lock Screen",
      detail: "Lock the current session",
      icon: "lock",
      label: "Session",
      keywords: ["lock", "screen", "secure", "session"],
      action: { type: "session-command", command: "lock" },
    },
    {
      title: "Suspend",
      detail: "Put the computer to sleep",
      icon: "moon",
      label: "Session",
      keywords: ["sleep", "suspend", "power", "session"],
      action: { type: "session-command", command: "suspend" },
    },
    {
      title: "Restart Computer",
      detail: "Restart the system",
      icon: "reboot",
      label: "Session",
      keywords: ["restart", "reboot", "power", "session"],
      action: { type: "session-command", command: "reboot" },
    },
    {
      title: "Power Off",
      detail: "Shut down the system",
      icon: "power",
      label: "Session",
      keywords: ["shutdown", "power off", "turn off", "session"],
      action: { type: "session-command", command: "power-off" },
    },
    {
      title: snapshot.doNotDisturb ? "Turn Off Do Not Disturb" : "Turn On Do Not Disturb",
      detail: snapshot.doNotDisturb ? "Resume notification alerts" : "Silence notification alerts",
      icon: "bell",
      label: "Notifications",
      keywords: ["notifications", "quiet", "focus", "dnd", "do not disturb"],
      action: { type: "notification-do-not-disturb", enabled: !snapshot.doNotDisturb },
    },
    {
      title: snapshot.debugOverlay ? "Hide Debug Overlay" : "Show Debug Overlay",
      detail: "Toggle compositor diagnostics",
      icon: "gauge",
      label: "Debug",
      keywords: ["debug", "fps", "performance", "diagnostics"],
      action: { type: "quick-toggle-debug-overlay" },
    },
    {
      title: "Reload Config",
      detail: "Reload Asher configuration",
      icon: "refresh",
      label: "System",
      keywords: ["reload", "config", "configuration", "refresh"],
      action: { type: "reload-config" },
    },
    {
      title: "Open Logs Folder",
      detail: "Open Asher log files",
      icon: "files",
      label: "Logs",
      keywords: ["logs", "journal", "debug", "diagnostics", "folder"],
      action: { type: "open-logs-folder" },
    },
  ];
}
