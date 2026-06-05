<script lang="ts">
  import AppButton from "./AppButton.svelte";
  import DebugMeter from "./DebugMeter.svelte";
  import Icon from "./Icon.svelte";
  import StatusCluster from "./StatusCluster.svelte";
  import { workspaceWheelOffset } from "../lib/workspace_wheel";
  import { sendAction } from "../shell/bridge";
  import { shortDate } from "../lib/labels";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();

  function launch(command: string) {
    sendAction({ type: "dock-launch", command });
  }

  function openMenu(command: string, x: number) {
    sendAction({ type: "dock-menu-open", command, x });
  }

  function workspaceScroll(event: WheelEvent) {
    const offset = workspaceWheelOffset(event);
    if (offset === 0) return;
    sendAction({ type: "workspace-relative", offset });
  }
</script>

<section class="shell-taskbar" class:is-chrome-hidden={snapshot.chromeHidden} onwheel={workspaceScroll}>
  <nav class="taskbar-apps" aria-label="Pinned applications">
    <button type="button" class="taskbar-launcher" aria-label="Open overview" onclick={() => sendAction({ type: "toggle-overview" })}>
      <Icon name="app" />
    </button>
    {#each snapshot.dockApps as app (app.command)}
      <AppButton {app} variant="taskbar" onlaunch={launch} onmenu={openMenu} />
    {/each}
  </nav>

  <StatusCluster {snapshot} className="taskbar-status" />

  <button
    type="button"
    class="taskbar-clock"
    aria-label={`${snapshot.date} ${snapshot.time}`}
    onclick={() => sendAction({ type: "toggle-date-center" })}
  >
    <span>{snapshot.time}</span>
    <strong>{shortDate()}</strong>
  </button>

  {#if snapshot.debugOverlay}
    <DebugMeter surface="PANEL" />
  {/if}
</section>
