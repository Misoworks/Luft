<script lang="ts">
  import AppButton from "./AppButton.svelte";
  import DebugMeter from "./DebugMeter.svelte";
  import { workspaceWheelOffset } from "../lib/workspace_wheel";
  import { sendAction } from "../shell/bridge";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();

  function launch(command: string) {
    sendAction({ type: "dock-menu-close" });
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

<section class="dock-shell" class:is-chrome-hidden={snapshot.chromeHidden}>
  <nav class="shell-dock" aria-label="Pinned applications" onwheel={workspaceScroll}>
    {#each snapshot.dockApps as app (app.command)}
      <AppButton {app} variant="dock" onlaunch={launch} onmenu={openMenu} />
    {/each}
  </nav>
  {#if snapshot.debugOverlay}
    <DebugMeter surface="DOCK" />
  {/if}
</section>
