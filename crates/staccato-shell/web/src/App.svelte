<script lang="ts">
  import DateCenter from "./components/DateCenter.svelte";
  import Dock from "./components/Dock.svelte";
  import DockMenu from "./components/DockMenu.svelte";
  import NotificationToast from "./components/NotificationToast.svelte";
  import Overview from "./components/Overview.svelte";
  import QuickSettings from "./components/QuickSettings.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Taskbar from "./components/Taskbar.svelte";
  import TopPanel from "./components/TopPanel.svelte";
  import { getSnapshot, sendAction, subscribe } from "./shell/bridge";
  import type { ShellSnapshot } from "./shell/model";
  import { onMount } from "svelte";

  let snapshot = $state.raw<ShellSnapshot>(getSnapshot());
  let overviewQuery = $state("");
  let overviewSelection = $state(-1);

  const surface = $derived(snapshot.surface ?? "panel");
  const rootElement = document.querySelector<HTMLElement>("#app");
  let surfaceAnimationTimer: number | undefined;
  let surfaceAnimationFrame: number | undefined;

  onMount(() => {
    applySnapshot(snapshot);
    scheduleSurfaceAnimation("opening");
    const unsubscribe = subscribe(applySnapshot);
    const resume = () => scheduleSurfaceAnimation("opening");
    const surfaceOpen = () => scheduleSurfaceAnimation("opening");
    const surfaceClose = () => runSurfaceAnimation("closing");
    const suspend = () => runSurfaceAnimation("closing");
    window.addEventListener("fenestra:resume", resume);
    window.addEventListener("fenestra:staccato.surface-open", surfaceOpen);
    window.addEventListener("fenestra:staccato.surface-close", surfaceClose);
    window.addEventListener("fenestra:suspend", suspend);
    return () => {
      unsubscribe();
      window.removeEventListener("fenestra:resume", resume);
      window.removeEventListener("fenestra:staccato.surface-open", surfaceOpen);
      window.removeEventListener("fenestra:staccato.surface-close", surfaceClose);
      window.removeEventListener("fenestra:suspend", suspend);
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
    if (nextSurface === "overview" && previousSurface !== "overview" && !overviewQuery.trim()) {
      overviewSelection = -1;
    }
    if (!rootElement) return;
    rootElement.dataset.surface = next.surface ?? "panel";
    rootElement.dataset.mode = next.activeMode;
    rootElement.dataset.taskbar = next.panelTaskbar ? "true" : "false";
    rootElement.style.setProperty("--panel", next.palette.panel);
    rootElement.style.setProperty("--panel-control", next.palette.panelControl);
    rootElement.style.setProperty("--panel-text", next.palette.panelText);
    rootElement.style.setProperty("--dock", next.palette.dock);
    rootElement.style.setProperty("--accent", next.palette.accent);
    rootElement.style.setProperty(
      "--wallpaper-image",
      next.wallpaperUri ? `url("${next.wallpaperUri}")` : "none",
    );
  }

  function runSurfaceAnimation(phase: "opening" | "closing") {
    if (!rootElement) return;
    const activeClass = phase === "opening" ? "is-surface-opening" : "is-surface-closing";
    const inactiveClass = phase === "opening" ? "is-surface-closing" : "is-surface-opening";
    rootElement.classList.remove(activeClass, inactiveClass);
    void rootElement.offsetWidth;
    rootElement.classList.add(activeClass);
    if (surfaceAnimationTimer) {
      window.clearTimeout(surfaceAnimationTimer);
    }
    surfaceAnimationTimer = window.setTimeout(
      () => rootElement.classList.remove(activeClass),
      phase === "opening" ? 560 : 240,
    );
  }

  function scheduleSurfaceAnimation(phase: "opening" | "closing") {
    if (phase === "closing") {
      runSurfaceAnimation(phase);
      return;
    }
    if (surfaceAnimationFrame) {
      window.cancelAnimationFrame(surfaceAnimationFrame);
    }
    surfaceAnimationFrame = window.requestAnimationFrame(() => {
      surfaceAnimationFrame = window.requestAnimationFrame(() => {
        surfaceAnimationFrame = undefined;
        runSurfaceAnimation("opening");
      });
    });
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "F3") {
      event.preventDefault();
      sendAction({ type: "quick-toggle-debug-overlay" });
      return;
    }
    if (event.key === "F4") {
      event.preventDefault();
      sendAction({ type: "toggle-shell-style" });
      return;
    }
    if (event.key === "Escape" && surface === "overview") {
      sendAction({ type: "toggle-overview" });
      return;
    }
    if (event.key === "Escape" && surface === "quick-settings") {
      sendAction({ type: "toggle-quick-settings" });
      return;
    }
    if (event.key === "Escape" && surface === "date-center") {
      sendAction({ type: "toggle-date-center" });
      return;
    }
    if (event.key === "Escape") {
      sendAction({ type: "dock-menu-close" });
    }
  }

  function pointerdown(event: PointerEvent) {
    if (!(event.target instanceof Element)) return;
    const target = event.target;
    if (
      (surface === "quick-settings" || surface === "date-center") &&
      !target.closest(".popover")
    ) {
      if (surface === "quick-settings") {
        sendAction({ type: "toggle-quick-settings" });
      } else {
        sendAction({ type: "toggle-date-center" });
      }
      return;
    }
    if (target.closest(".dock-menu, .dock-item, .app-button")) return;
    sendAction({ type: "dock-menu-close" });
  }
</script>

<svelte:document onkeydown={keydown} onpointerdown={pointerdown} />

{#if surface === "dock"}
  <Dock {snapshot} />
{:else if surface === "dock-menu"}
  <DockMenu {snapshot} />
{:else if surface === "sidebar"}
  <Sidebar {snapshot} />
{:else if surface === "quick-settings"}
  <QuickSettings {snapshot} />
{:else if surface === "date-center"}
  <DateCenter {snapshot} />
{:else if surface === "notification-toast"}
  <NotificationToast {snapshot} />
{:else if surface === "overview"}
  <Overview
    {snapshot}
    query={overviewQuery}
    selection={overviewSelection}
    setQuery={(query) => (overviewQuery = query)}
    setSelection={(selection) => (overviewSelection = selection)}
  />
{:else if snapshot.activeMode === "panel"}
  <Taskbar {snapshot} />
{:else}
  <TopPanel {snapshot} />
{/if}
