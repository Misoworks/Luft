<script lang="ts">
  import DateCenter from "./components/DateCenter.svelte";
  import Dock from "./components/Dock.svelte";
  import DockMenu from "./components/DockMenu.svelte";
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

  onMount(() => {
    applySnapshot(snapshot);
    return subscribe(applySnapshot);
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
    rootElement.style.setProperty("--panel", next.palette.panel);
    rootElement.style.setProperty("--panel-control", next.palette.panelControl);
    rootElement.style.setProperty("--panel-text", next.palette.panelText);
    rootElement.style.setProperty("--dock", next.palette.dock);
    rootElement.style.setProperty("--accent", next.palette.accent);
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
    if (event.key === "Escape") {
      sendAction({ type: "dock-menu-close" });
    }
  }

  function pointerdown(event: PointerEvent) {
    if ((event.target as Element).closest(".dock-menu, .dock-item")) return;
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
