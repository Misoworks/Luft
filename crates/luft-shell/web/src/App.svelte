<script lang="ts">
  import DateCenter from "./components/DateCenter.svelte";
  import PanelMenu from "./components/PanelMenu.svelte";
  import NotificationToast from "./components/NotificationToast.svelte";
  import StartMenu from "./components/StartMenu.svelte";
  import QuickSettings from "./components/QuickSettings.svelte";
  import Panel from "./components/Panel.svelte";
  import { getSnapshot, sendAction, subscribe } from "./shell/bridge";
  import type { ShellSnapshot } from "./shell/model";
  import { onMount } from "svelte";

  let snapshot = $state.raw<ShellSnapshot>(getSnapshot());
  let startMenuQuery = $state("");
  let startMenuSelection = $state(-1);

  const surface = $derived(snapshot.surface ?? "panel");
  const rootElement = document.querySelector<HTMLElement>("#app");
  let surfaceAnimationTimer: number | undefined;
  let surfaceAnimationFrame: number | undefined;
  let surfaceAnimationPhase: "opening" | "closing" | undefined;
  let lastSurfaceAnimationAt = 0;

  onMount(() => {
    applySnapshot(snapshot);
    if (!isNativeSurfaceRuntime()) {
      scheduleSurfaceAnimation("opening");
    }
    const unsubscribe = subscribe(applySnapshot);
    const surfaceOpen = () => scheduleSurfaceAnimation("opening");
    const surfaceClose = () => runSurfaceAnimation("closing");
    window.addEventListener("fenestra:luft.surface-open", surfaceOpen);
    window.addEventListener("fenestra:luft.surface-close", surfaceClose);
    return () => {
      unsubscribe();
      window.removeEventListener("fenestra:luft.surface-open", surfaceOpen);
      window.removeEventListener("fenestra:luft.surface-close", surfaceClose);
      if (surfaceAnimationTimer) {
        window.clearTimeout(surfaceAnimationTimer);
      }
      if (surfaceAnimationFrame) {
        window.cancelAnimationFrame(surfaceAnimationFrame);
      }
    };
  });

  function applySnapshot(next: ShellSnapshot) {
    const previousSurface = snapshot.surface ?? "panel";
    const nextSurface = next.surface ?? "panel";
    snapshot = next;
    if (nextSurface === "start-menu" && previousSurface !== "start-menu" && !startMenuQuery.trim()) {
      startMenuSelection = -1;
    }
    if (!rootElement) return;
    rootElement.dataset.surface = next.surface ?? "panel";
    rootElement.dataset.material = "glass";
    rootElement.style.setProperty("--panel", next.palette.panel);
    rootElement.style.setProperty("--panel-control", next.palette.panelControl);
    rootElement.style.setProperty("--panel-text", next.palette.panelText);
    rootElement.style.setProperty("--panel-bar", next.palette.panelBar);
    rootElement.style.setProperty("--accent", next.palette.accent);
    rootElement.style.setProperty("--text-soft", next.palette.textSoft);
    rootElement.style.setProperty("--text-muted", next.palette.textMuted);
  }

  function runSurfaceAnimation(phase: "opening" | "closing") {
    if (!rootElement) return;
    const now = performance.now();
    const reopening =
      phase === "opening" && surfaceAnimationPhase === "closing";
    if (
      !reopening &&
      surfaceAnimationPhase === phase &&
      now - lastSurfaceAnimationAt < 80
    ) {
      return;
    }
    surfaceAnimationPhase = phase;
    lastSurfaceAnimationAt = now;
    const activeClass = phase === "opening" ? "is-surface-opening" : "is-surface-closing";
    const inactiveClass = phase === "opening" ? "is-surface-closing" : "is-surface-opening";
    rootElement.classList.remove(activeClass, inactiveClass);
    void rootElement.offsetWidth;
    rootElement.classList.add(activeClass);
    if (surfaceAnimationTimer) {
      window.clearTimeout(surfaceAnimationTimer);
    }
    surfaceAnimationTimer = window.setTimeout(
      () => {
        rootElement.classList.remove(activeClass);
      if (surfaceAnimationPhase === phase) {
        surfaceAnimationPhase = undefined;
      }
    },
      phase === "opening" ? 280 : 210,
    );
  }

  function scheduleSurfaceAnimation(phase: "opening" | "closing") {
    if (phase === "closing") {
      if (surfaceAnimationFrame) {
        window.cancelAnimationFrame(surfaceAnimationFrame);
        surfaceAnimationFrame = undefined;
      }
      runSurfaceAnimation(phase);
      return;
    }
    if (surfaceAnimationFrame) {
      window.cancelAnimationFrame(surfaceAnimationFrame);
    }
    surfaceAnimationFrame = undefined;
    runSurfaceAnimation("opening");
  }

  function isNativeSurfaceRuntime() {
    return Boolean(window.fenestra?.bridge) || new URLSearchParams(window.location.search).has("fenestra");
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "Escape" && surface === "start-menu") {
      sendAction({ type: "close-start-menu" });
      return;
    }
    if (event.key === "Escape" && surface === "quick-settings") {
      sendAction({ type: "close-quick-settings" });
      return;
    }
    if (event.key === "Escape" && surface === "date-center") {
      sendAction({ type: "close-date-center" });
      return;
    }
    if (event.key === "Escape") {
      sendAction({ type: "panel-menu-close" });
    }
  }

  function pointerdown(event: PointerEvent) {
    if (!(event.target instanceof Element)) return;
    const target = event.target;
    if (surface === "start-menu" && !target.closest(".shell-start-menu")) {
      sendAction({ type: "close-start-menu" });
      return;
    }
    if ((surface === "quick-settings" || surface === "date-center") && !target.closest(".popover")) {
      if (surface === "quick-settings") {
        sendAction({ type: "close-quick-settings" });
      } else {
        sendAction({ type: "close-date-center" });
      }
      return;
    }
    if (target.closest(".panel-menu, .panel-item, .app-button")) return;
    sendAction({ type: "panel-menu-close" });
  }
</script>

<svelte:document onkeydown={keydown} onpointerdown={pointerdown} />

{#if surface === "panel-menu"}
  <PanelMenu {snapshot} />
{:else if surface === "quick-settings"}
  <QuickSettings {snapshot} />
{:else if surface === "date-center"}
  <DateCenter {snapshot} />
{:else if surface === "notification-toast"}
  <NotificationToast {snapshot} />
{:else if surface === "start-menu"}
  <StartMenu
    {snapshot}
    query={startMenuQuery}
    selection={startMenuSelection}
    setQuery={(query) => (startMenuQuery = query)}
    setSelection={(selection) => (startMenuSelection = selection)}
  />
{:else}
  <Panel {snapshot} />
{/if}
