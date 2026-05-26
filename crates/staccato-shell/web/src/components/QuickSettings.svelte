<script lang="ts">
  import DebugMeter from "./DebugMeter.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { audioLabel, batteryLabel, networkLabel, nextProfileLabel } from "../lib/labels";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
</script>

<section class="popover quick-settings">
  <header class="quick-header">
    <span>Quick Settings</span>
    <small>{snapshot.activeProfile} / Workspace {snapshot.activeWorkspace}</small>
  </header>

  <div class="quick-status-grid">
    <button type="button" class="setting-tile is-action" onclick={() => sendAction({ type: "quick-open-settings", page: "network" })}>
      <Icon name="network" />
      <span>{snapshot.status.network?.wireless ? "Wi-Fi" : "Wired"}<small>{networkLabel(snapshot)}</small></span>
    </button>
    <button type="button" class="setting-tile is-action" onclick={() => sendAction({ type: "quick-open-settings", page: "audio" })}>
      <Icon name="volume" />
      <span>Audio<small>{audioLabel(snapshot)}</small></span>
    </button>
    <button type="button" class="setting-tile is-action" onclick={() => sendAction({ type: "quick-open-settings", page: "power" })}>
      <Icon name="power" />
      <span>Power<small>{batteryLabel(snapshot)}</small></span>
    </button>
    <button type="button" class="setting-tile is-action" onclick={() => sendAction({ type: "quick-next-profile" })}>
      <Icon name="settings" />
      <span>Workspace profile<small>{nextProfileLabel(snapshot)}</small></span>
    </button>
  </div>

  <div class="quick-control-grid">
    <button
      type="button"
      class="toggle-row"
      class:is-on={snapshot.blurEnabled}
      onclick={() => sendAction({ type: "quick-toggle-blur" })}
    >
      <span>Blur</span>
      <span class="switch"><span></span></span>
    </button>
    <button
      type="button"
      class="toggle-row"
      class:is-on={snapshot.debugOverlay}
      onclick={() => sendAction({ type: "quick-toggle-debug-overlay" })}
    >
      <span>Debug overlay</span>
      <span class="switch"><span></span></span>
    </button>
  </div>

  <footer class="quick-footer">
    <button type="button" class="round-action" aria-label="Open launcher" onclick={() => sendAction({ type: "open-launcher" })}>
      <Icon name="search" />
    </button>
    <button type="button" class="round-action" aria-label="Open overview" onclick={() => sendAction({ type: "toggle-overview" })}>
      <Icon name="app" />
    </button>
    <button type="button" class="round-action" aria-label="Reload config" onclick={() => sendAction({ type: "quick-reload-config" })}>
      <Icon name="reload" />
    </button>
  </footer>

  {#if snapshot.debugOverlay}
    <DebugMeter surface="QS" />
  {/if}
</section>
