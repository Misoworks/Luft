<script lang="ts">
  import AppButton from "./AppButton.svelte";
  import DebugMeter from "./DebugMeter.svelte";
  import Icon from "./Icon.svelte";
  import StatusCluster from "./StatusCluster.svelte";
  import { sendAction } from "../shell/bridge";
  import { shortDate } from "../lib/labels";
  import { isLaunching, markLaunching } from "../state/launch_state";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  let launchRevision = $state(0);
  let hoverEpoch = $state(0);

  function launch(command: string) {
    markLaunching(command, () => {
      launchRevision += 1;
    });
    launchRevision += 1;
    sendAction({ type: "dock-launch", command });
  }

  function openMenu(command: string) {
    sendAction({ type: "dock-menu-open", command });
  }

  function launching(command: string, revision: number) {
    revision;
    return isLaunching(command);
  }

  function workspaceScroll(event: WheelEvent) {
    sendAction({ type: "workspace-relative", offset: event.deltaY > 0 ? 1 : -1 });
  }

  function clearHover() {
    hoverEpoch += 1;
  }
</script>

<section class="shell-taskbar" class:is-chrome-hidden={snapshot.chromeHidden}>
  <button type="button" class="taskbar-launcher" aria-label="Open overview" onclick={() => sendAction({ type: "toggle-overview" })}>
    <Icon name="app" />
  </button>

  <nav class="taskbar-apps" aria-label="Pinned applications" onwheel={workspaceScroll} onpointerleave={clearHover} onmouseleave={clearHover}>
    {#each snapshot.dockApps as app (app.command)}
      <AppButton {app} variant="taskbar" launching={launching(app.command, launchRevision)} {hoverEpoch} onlaunch={launch} onmenu={openMenu} />
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
