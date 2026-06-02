<script lang="ts">
  import DebugMeter from "./DebugMeter.svelte";
  import ControlSlider from "./ControlSlider.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { batteryLabel, networkLabel } from "../lib/labels";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  const showNetwork = $derived(Boolean(snapshot.status.network));
  const showPower = $derived(Boolean(snapshot.status.battery));
  const showVolume = $derived(Boolean(snapshot.status.audio));
  const showBrightness = $derived(Boolean(snapshot.status.brightness));
  const tileCount = $derived(Number(showNetwork) + Number(showPower));
  const volume = $derived(snapshot.status.audio?.percent ?? 0);
  const brightness = $derived(snapshot.status.brightness?.percent ?? 0);

  function setVolume(percent: number) {
    sendAction({ type: "quick-set-volume", percent });
  }

  function toggleMute() {
    sendAction({ type: "quick-toggle-mute" });
  }

  function setBrightness(percent: number) {
    sendAction({ type: "quick-set-brightness", percent });
  }
</script>

<section class="popover quick-settings">
  <header class="quick-header">
    <div>
      <span>Quick Settings</span>
      <small>{snapshot.activeProfile} / Workspace {snapshot.activeWorkspace}</small>
    </div>
  </header>

  <div class="quick-status-grid">
    {#if showNetwork}
      <button
        type="button"
        class="setting-tile is-action is-primary"
        class:is-wide={tileCount === 1}
        style="--index: 0"
        onclick={() => sendAction({ type: "quick-open-settings", page: "network" })}
      >
        <span class="setting-icon"><Icon name="network" /></span>
        <span class="setting-copy">{snapshot.status.network?.wireless ? "Wi-Fi" : "Wired"}<small>{networkLabel(snapshot)}</small></span>
      </button>
    {/if}
    {#if showPower}
      <button
        type="button"
        class="setting-tile is-action"
        class:is-wide={tileCount === 1}
        style="--index: 1"
        onclick={() => sendAction({ type: "quick-open-settings", page: "power" })}
      >
        <span class="setting-icon"><Icon name="power" /></span>
        <span class="setting-copy">Power<small>{batteryLabel(snapshot)}</small></span>
      </button>
    {/if}
  </div>

  <div class="quick-slider-stack">
    {#if showVolume}
      <ControlSlider
        label="Volume"
        icon="volume"
        value={volume}
        muted={snapshot.status.audio?.muted ?? false}
        index={2}
        onChange={setVolume}
        onToggle={toggleMute}
      />
    {/if}
    {#if showBrightness}
      <ControlSlider label="Brightness" icon="sun" value={brightness} index={3} onChange={setBrightness} />
    {/if}
  </div>

  <footer class="quick-footer">
    <button type="button" class="round-action" aria-label="Open launcher" onclick={() => sendAction({ type: "open-launcher" })}>
      <Icon name="search" />
    </button>
    <button type="button" class="round-action" aria-label="Open overview" onclick={() => sendAction({ type: "toggle-overview" })}>
      <Icon name="app" />
    </button>
    <button type="button" class="round-action" aria-label="Open settings" onclick={() => sendAction({ type: "quick-open-settings", page: "power" })}>
      <Icon name="settings" />
    </button>
  </footer>

  {#if snapshot.debugOverlay}
    <DebugMeter surface="QS" />
  {/if}
</section>
