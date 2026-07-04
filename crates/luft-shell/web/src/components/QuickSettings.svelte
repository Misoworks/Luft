<script lang="ts">
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
  const tileCount = $derived(Number(showNetwork) + Number(showPower) + 1);
  const volume = $derived(snapshot.status.audio?.percent ?? 0);
  const brightness = $derived(snapshot.status.brightness?.percent ?? 0);
  const notificationLabel = $derived(snapshot.notifications.length === 1 ? "1 notification" : `${snapshot.notifications.length} notifications`);

  function setVolume(percent: number) {
    sendAction({ type: "quick-set-volume", percent });
  }

  function toggleMute() {
    sendAction({ type: "quick-toggle-mute" });
  }

  function setBrightness(percent: number) {
    sendAction({ type: "quick-set-brightness", percent });
  }

  function toggleDoNotDisturb() {
    sendAction({ type: "notification-do-not-disturb", enabled: !snapshot.doNotDisturb });
  }

  function togglePowerMenu() {
    sendAction({ type: "toggle-session-menu" });
  }
</script>

<section class="popover quick-settings">
  <header class="quick-header">
    <span class="quick-profile" aria-label="User profile">
      {#if snapshot.userProfileIconUri}
        <img src={snapshot.userProfileIconUri} alt="" />
      {:else}
        <Icon name="user" />
      {/if}
    </span>
    <div class="quick-header-actions">
      <button
        type="button"
        class="round-action is-compact"
        aria-label="Open system settings"
        onclick={() => sendAction({ type: "launch-default-app", app: "settings" })}
      >
        <Icon name="settings" />
      </button>
      <button
        type="button"
        class="round-action is-compact"
        aria-label="Open power menu"
        onclick={togglePowerMenu}
      >
        <Icon name="power" />
      </button>
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
    <button
      type="button"
      class="setting-tile is-action"
      class:is-primary={snapshot.doNotDisturb}
      class:is-wide={tileCount === 1}
      style="--index: 2"
      aria-pressed={snapshot.doNotDisturb}
      onclick={toggleDoNotDisturb}
    >
      <span class="setting-icon"><Icon name={snapshot.doNotDisturb ? "bell-off" : "bell"} /></span>
      <span class="setting-copy">Do Not Disturb<small>{snapshot.doNotDisturb ? "On" : notificationLabel}</small></span>
    </button>
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
</section>
