import type { ShellAction, ShellSnapshot, ShellSurface } from "./model";
import { emptySnapshot } from "./model";

declare global {
  interface Window {
    __STACCATO_INITIAL_STATE__?: ShellSnapshot;
    __STACCATO_SURFACE__?: ShellSurface;
    ipc?: { postMessage: (message: string) => void };
    staccatoShell?: {
      setSnapshot: (snapshot: ShellSnapshot) => void;
    };
  }
}

type Listener = (snapshot: ShellSnapshot) => void;

const listeners = new Set<Listener>();
let currentSnapshot = normalizeSnapshot(window.__STACCATO_INITIAL_STATE__);

export const getSnapshot = () => currentSnapshot;

export const subscribe = (listener: Listener) => {
  listeners.add(listener);
  listener(currentSnapshot);
  return () => listeners.delete(listener);
};

export const sendAction = (action: ShellAction) => {
  window.ipc?.postMessage(JSON.stringify(action));
};

window.staccatoShell = {
  setSnapshot(snapshot: ShellSnapshot) {
    currentSnapshot = normalizeSnapshot(snapshot);
    for (const listener of listeners) {
      listener(currentSnapshot);
    }
  },
};

function normalizeSnapshot(snapshot?: ShellSnapshot): ShellSnapshot {
  return {
    ...emptySnapshot(),
    ...snapshot,
    surface: surfaceFromRuntime(snapshot),
    palette: {
      ...emptySnapshot().palette,
      ...snapshot?.palette,
    },
    status: {
      ...snapshot?.status,
    },
  };
}

function surfaceFromRuntime(snapshot?: ShellSnapshot): ShellSurface {
  if (window.__STACCATO_SURFACE__) {
    return window.__STACCATO_SURFACE__;
  }
  if (snapshot?.surface) {
    return snapshot.surface;
  }
  const query = new URLSearchParams(window.location.search);
  const surface = query.get("surface");
  if (isSurface(surface)) {
    return surface;
  }
  return "panel";
}

function isSurface(value: string | null): value is ShellSurface {
  return (
    value === "panel" ||
    value === "dock" ||
    value === "dock-menu" ||
    value === "sidebar" ||
    value === "quick-settings" ||
    value === "date-center" ||
    value === "overview"
  );
}
