<script lang="ts">
  import ControlSlider from "./ControlSlider.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { batteryLabel, networkLabel } from "../lib/labels";
  import type { ShellSnapshot } from "../shell/model";
  import { onMount } from "svelte";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  let powerMenuOpen = $state(false);
  const showNetwork = $derived(Boolean(snapshot.status.network));
  const showPower = $derived(Boolean(snapshot.status.battery));
  const showVolume = $derived(Boolean(snapshot.status.audio));
  const showBrightness = $derived(Boolean(snapshot.status.brightness));
  const tileCount = $derived(Number(showNetwork) + Number(showPower) + 1);
  const volume = $derived(snapshot.status.audio?.percent ?? 0);
  const brightness = $derived(snapshot.status.brightness?.percent ?? 0);
  const notificationLabel = $derived(snapshot.notifications.length === 1 ? "1 notification" : `${snapshot.notifications.length} notifications`);
  const sessionMenuUrl = $derived(new URL("session-menu.html", window.location.href).toString());

  onMount(() => {
    const closePowerMenu = () => {
      void closeNativePopup();
      powerMenuOpen = false;
    };
    window.addEventListener("fenestra:luft.surface-open", closePowerMenu);
    window.addEventListener("fenestra:luft.surface-close", closePowerMenu);
    window.addEventListener("fenestra:popup.close", closePowerMenu);
    return () => {
      window.removeEventListener("fenestra:luft.surface-open", closePowerMenu);
      window.removeEventListener("fenestra:luft.surface-close", closePowerMenu);
      window.removeEventListener("fenestra:popup.close", closePowerMenu);
    };
  });

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

  async function togglePowerMenu(event: MouseEvent) {
    if (powerMenuOpen) {
      await closeNativePopup();
      powerMenuOpen = false;
      return;
    }
    if (!canUseNativePopup()) {
      console.error("native popup bridge unavailable");
      return;
    }
    const target = event.currentTarget;
    if (!(target instanceof HTMLElement)) {
      return;
    }
    const rect = target.getBoundingClientRect();
    const width = 188;
    const height = 172;
    try {
      await openNativePopup({
        x: Math.round(rect.right - width),
        y: Math.round(rect.bottom + 8),
        width,
        height,
        url: sessionMenuUrl,
      });
      powerMenuOpen = true;
    } catch (error) {
      powerMenuOpen = false;
      console.error("failed to open native power popup", error);
    }
  }

  function canUseNativePopup() {
    const bridge = window.fenestra?.bridge;
    return Boolean(
      window.fenestra?.popup?.open ||
        (bridge?.commands.includes("fenestra.popup.open") && bridge.commands.includes("fenestra.popup.close")),
    );
  }

  function openNativePopup(options: {
    x: number;
    y: number;
    width: number;
    height: number;
    url: string;
  }) {
    const popup = window.fenestra?.popup;
    if (popup?.open) return popup.open(options);
    return window.fenestra?.bridge?.invoke("fenestra.popup.open", options);
  }

  function closeNativePopup() {
    const popup = window.fenestra?.popup;
    if (popup?.close) return popup.close();
    return window.fenestra?.bridge?.invoke("fenestra.popup.close", {});
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
        class:is-active={powerMenuOpen}
        aria-label={powerMenuOpen ? "Close power menu" : "Open power menu"}
        aria-expanded={powerMenuOpen}
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
