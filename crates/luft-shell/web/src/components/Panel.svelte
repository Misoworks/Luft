<script lang="ts">
  import AppButton from "./AppButton.svelte";
  import StatusCluster from "./StatusCluster.svelte";
  import { appFly, runningAppEnter, runningAppExit } from "../lib/app_motion";
  import { movePanelCommand, sameOrder } from "../lib/panel_reorder";
  import { workspaceWheelOffset } from "../lib/workspace_wheel";
  import { sendAction } from "../shell/bridge";
  import { shortDate } from "../lib/labels";
  import type { PanelApp, ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  let order = $state<string[]>([]);
  let orderSignature = $state("");
  let draggedCommand = $state<string | null>(null);

  const pinnedApps = $derived(snapshot.panelApps.filter((app) => app.pinned));
  const runningOnlyApps = $derived(snapshot.panelApps.filter((app) => !app.pinned));
  const appByCommand = $derived(new Map(pinnedApps.map((app) => [app.command, app])));
  const orderedPinnedApps = $derived(
    (order.length > 0 ? order : pinnedApps.map((app) => app.command))
      .map((command) => appByCommand.get(command))
      .filter((app): app is PanelApp => Boolean(app)),
  );

  const runningExit = runningAppExit;

  $effect(() => {
    const commands = pinnedApps.map((app) => app.command);
    const signature = commands.join("\0");
    if (!draggedCommand && signature !== orderSignature) {
      order = commands;
      orderSignature = signature;
    }
  });

  function launch(app: PanelApp) {
    const windowId = nextWindowId(app);
    if (windowId !== undefined) {
      sendAction({ type: "window-activate", window: windowId });
      return;
    }
    sendAction({ type: "panel-launch", command: app.command });
  }

  function openMenu(command: string, x: number) {
    sendAction({ type: "panel-menu-open", command, x });
  }

  function workspaceScroll(event: WheelEvent) {
    const offset = workspaceWheelOffset(event);
    if (offset === 0) return;
    sendAction({ type: "workspace-relative", offset });
  }

  function startReorder(command: string) {
    draggedCommand = command;
    order = pinnedApps.map((app) => app.command);
    sendAction({ type: "panel-menu-close" });
  }

  function previewReorder(target: string, after: boolean) {
    if (!draggedCommand) return;
    order = movePanelCommand(order, draggedCommand, target, after);
  }

  function commitReorder() {
    if (!draggedCommand) return;
    const current = pinnedApps.map((app) => app.command);
    if (!sameOrder(order, current)) {
      sendAction({ type: "panel-reorder", commands: order });
    }
    draggedCommand = null;
  }

  function endReorder() {
    if (!draggedCommand) return;
    draggedCommand = null;
    order = pinnedApps.map((app) => app.command);
  }

  function windowIdFromCommand(command: string) {
    if (!command.startsWith("window:")) return undefined;
    const id = Number(command.slice("window:".length));
    return Number.isFinite(id) ? id : undefined;
  }

  function nextWindowId(app: PanelApp) {
    const ids = app.windowIds.length > 0 ? app.windowIds : app.windowId !== undefined ? [app.windowId] : [];
    if (ids.length === 0) return windowIdFromCommand(app.command);
    const visible = ids.find((id) => snapshot.windows.some((window) => window.id === id && window.visible));
    const active = app.activeWindowId ?? ids.find((id) => snapshot.windows.some((window) => window.id === id && window.active));
    if (app.active && active !== undefined && ids.length > 1) {
      const index = ids.indexOf(active);
      return ids[(index + 1) % ids.length];
    }
    return active ?? visible ?? ids[0];
  }
</script>

<section class="shell-panel-bar" onwheel={workspaceScroll}>
  <nav class="panel-apps" aria-label="Pinned applications">
    <button
      type="button"
      class="panel-launcher"
      aria-label="Open Start menu"
      onclick={() =>
        sendAction({
          type: snapshot.startMenuOpen ? "close-start-menu" : "toggle-start-menu",
        })}
    >
      <svg class="luft-mark" width="31" height="32" viewBox="0 0 31 32" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path
          class="luft-mark-secondary"
          d="M19.4995 21.3357C26.0585 18.4154 26.3224 7.15562 29.2427 13.7146C32.1625 20.2734 29.2125 27.9574 22.6538 30.8777C16.0949 33.7979 8.41007 30.8486 5.48975 24.2898C2.56954 17.7309 12.9405 24.2556 19.4995 21.3357Z"
          fill="currentColor"
        />
        <path
          class="luft-mark-primary"
          d="M12.772 10.3591C19.331 7.43888 21.4211 -4.63442 24.3413 1.92456C27.2614 8.48338 24.3121 16.1673 17.7534 19.0876C11.1945 22.0079 3.50967 19.0586 0.589355 12.4998C-2.33088 5.94079 6.213 13.2793 12.772 10.3591Z"
          fill="currentColor"
        />
      </svg>
    </button>
    {#each orderedPinnedApps as app (app.command)}
      <div class="panel-app-slot">
        <AppButton
          {app}
          onlaunch={launch}
          onmenu={openMenu}
          onreorderstart={startReorder}
          onreorderover={previewReorder}
          onreorderdrop={commitReorder}
          onreorderend={endReorder}
        />
      </div>
    {/each}
    {#each runningOnlyApps as app (app.command)}
      <div
        class="panel-app-slot panel-app-slot--running"
        in:appFly={runningAppEnter}
        out:appFly={runningExit}
      >
        <AppButton
          {app}
          onlaunch={launch}
          onmenu={openMenu}
          onreorderstart={startReorder}
          onreorderover={previewReorder}
          onreorderdrop={commitReorder}
          onreorderend={endReorder}
          reorderable={false}
        />
      </div>
    {/each}
  </nav>

  <StatusCluster {snapshot} className="panel-status" />

  <button
    type="button"
    class="panel-clock"
    aria-label={`${snapshot.date} ${snapshot.time}`}
    onclick={() =>
      sendAction({
        type: snapshot.dateCenterOpen ? "close-date-center" : "toggle-date-center",
      })}
  >
    <span>{snapshot.time}</span>
    <strong>{shortDate()}</strong>
  </button>

</section>
