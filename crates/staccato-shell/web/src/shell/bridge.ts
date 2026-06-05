import type { ShellAction, ShellSnapshot, ShellSurface } from "./model";
import { emptySnapshot } from "./model";

declare global {
  interface Window {
    __STACCATO_INITIAL_STATE__?: ShellSnapshot;
    __STACCATO_SURFACE__?: ShellSurface;
    fenestra?: {
      bridge?: NativeBridge;
    };
    ipc?: { postMessage: (message: string) => void };
    staccatoShell?: {
      setSnapshot: (snapshot: ShellSnapshot) => void;
    };
  }
}

type NativeBridge = {
  commands: string[];
  invoke<T>(name: string, params?: Record<string, unknown>): Promise<T>;
  listen?<T>(name: string, callback: (payload: T) => void): () => void;
};

type NativeReady = {
  surface?: ShellSurface;
  snapshot?: ShellSnapshot;
};

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
  const bridge = window.fenestra?.bridge;
  if (bridge?.commands.includes("staccato.action")) {
    void bridge.invoke(
      "staccato.action",
      action as unknown as Record<string, unknown>,
    );
    return;
  }
  window.ipc?.postMessage(JSON.stringify(action));
};

window.staccatoShell = {
  setSnapshot(snapshot: ShellSnapshot) {
    applySnapshot(snapshot);
  },
};

void initializeNativeBridge();

async function initializeNativeBridge() {
  const bridge = await waitForNativeBridge();
  if (!bridge) return;

  bridge.listen?.<ShellSnapshot>("staccato.snapshot", applySnapshot);

  if (!bridge.commands.includes("staccato.ready")) return;
  try {
    const ready = await bridge.invoke<NativeReady>("staccato.ready");
    if (ready.surface) {
      window.__STACCATO_SURFACE__ = ready.surface;
    }
    if (ready.snapshot) {
      applySnapshot(ready.snapshot);
    }
  } catch (error) {
    console.error("failed to initialize staccato shell bridge", error);
  }
}

function applySnapshot(snapshot: ShellSnapshot) {
  currentSnapshot = normalizeSnapshot(snapshot);
  for (const listener of listeners) {
    listener(currentSnapshot);
  }
}

async function waitForNativeBridge(): Promise<NativeBridge | undefined> {
  if (!isFenestraRuntime()) return undefined;

  const deadline = performance.now() + 2000;
  while (performance.now() < deadline) {
    if (window.fenestra?.bridge) {
      return window.fenestra.bridge;
    }
    await new Promise((resolve) => window.setTimeout(resolve, 16));
  }
  return undefined;
}

function isFenestraRuntime() {
  return (
    Boolean(window.fenestra?.bridge) ||
    new URLSearchParams(window.location.search).has("fenestra")
  );
}

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
    value === "notification-toast" ||
    value === "overview"
  );
}
