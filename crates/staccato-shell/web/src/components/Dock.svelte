<script lang="ts">
  import AppButton from "./AppButton.svelte";
  import DebugMeter from "./DebugMeter.svelte";
  import { sendAction } from "../shell/bridge";
  import { isLaunching, markLaunching } from "../state/launch_state";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  let launchRevision = $state(0);
  let hoverEpoch = $state(0);

  function launch(command: string) {
    sendAction({ type: "dock-menu-close" });
    markLaunching(command, () => {
      launchRevision += 1;
    });
    launchRevision += 1;
    sendAction({ type: "dock-launch", command });
  }

  function openMenu(command: string) {
    sendAction({ type: "dock-menu-open", command });
  }

  function workspaceScroll(event: WheelEvent) {
    sendAction({ type: "workspace-relative", offset: event.deltaY > 0 ? 1 : -1 });
  }

  function launching(command: string, revision: number) {
    revision;
    return isLaunching(command);
  }

  function clearHover() {
    hoverEpoch += 1;
  }
</script>

<section class="dock-shell" class:is-chrome-hidden={snapshot.chromeHidden}>
  <nav class="shell-dock" aria-label="Pinned applications" onwheel={workspaceScroll} onpointerleave={clearHover} onmouseleave={clearHover}>
    {#each snapshot.dockApps as app (app.command)}
      <AppButton {app} variant="dock" launching={launching(app.command, launchRevision)} {hoverEpoch} onlaunch={launch} onmenu={openMenu} />
    {/each}
  </nav>
  {#if snapshot.debugOverlay}
    <DebugMeter surface="DOCK" />
  {/if}
</section>
