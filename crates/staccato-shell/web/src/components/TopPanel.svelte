<script lang="ts">
  import DebugMeter from "./DebugMeter.svelte";
  import StatusCluster from "./StatusCluster.svelte";
  import { workspaceWheelOffset } from "../lib/workspace_wheel";
  import { sendAction } from "../shell/bridge";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();

  function workspaceScroll(event: WheelEvent) {
    const offset = workspaceWheelOffset(event);
    if (offset === 0) return;
    sendAction({ type: "workspace-relative", offset });
  }
</script>

<section class="shell-panel" class:is-chrome-hidden={snapshot.chromeHidden} onwheel={workspaceScroll}>
  <div class="panel-spacer"></div>
  <button
    type="button"
    class="clock-button"
    aria-label={`${snapshot.date} ${snapshot.time}`}
    onclick={() => sendAction({ type: "toggle-date-center" })}
  >
    {snapshot.time}
  </button>
  <StatusCluster {snapshot} className="status-cluster" />
  {#if snapshot.debugOverlay}
    <DebugMeter surface="PANEL" />
  {/if}
</section>
