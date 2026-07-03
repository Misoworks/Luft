<script lang="ts">
  import ControlSlider from "./ControlSlider.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { batteryLabel, networkLabel } from "../lib/labels";
  import type { ShellSnapshot } from "../shell/model";
  import { onMount } from "svelte";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  let powerMenuOpen = $state(false);
  let powerMenuNative = $state(false);
  const showNetwork = $derived(Boolean(snapshot.status.network));
  const showPower = $derived(Boolean(snapshot.status.battery));
  const showVolume = $derived(Boolean(snapshot.status.audio));
  const showBrightness = $derived(Boolean(snapshot.status.brightness));
  const tileCount = $derived(Number(showNetwork) + Number(showPower) + 1);
  const volume = $derived(snapshot.status.audio?.percent ?? 0);
  const brightness = $derived(snapshot.status.brightness?.percent ?? 0);
  const notificationLabel = $derived(snapshot.notifications.length === 1 ? "1 notification" : `${snapshot.notifications.length} notifications`);

  onMount(() => {
    const closePowerMenu = () => {
      void closeNativePopup();
      powerMenuOpen = false;
      powerMenuNative = false;
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
      powerMenuNative = false;
      return;
    }
    if (canUseNativePopup()) {
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
          html: sessionMenuDocument(),
        });
        powerMenuOpen = true;
        powerMenuNative = true;
      } catch (error) {
        powerMenuOpen = true;
        powerMenuNative = false;
        console.error("failed to open native power popup", error);
      }
      return;
    }
    powerMenuOpen = true;
    powerMenuNative = false;
  }

  function runSessionCommand(command: "lock" | "suspend" | "reboot" | "power-off") {
    sendAction({ type: "session-command", command });
    powerMenuOpen = false;
    powerMenuNative = false;
    void closeNativePopup();
  }

  function canUseNativePopup() {
    const bridge = window.fenestra?.bridge;
    return Boolean(
      window.fenestra?.popup?.open ||
        (bridge?.commands.includes("fenestra.popup.open") && bridge.commands.includes("fenestra.popup.close")),
    );
  }

  function openNativePopup(options: { x: number; y: number; width: number; height: number; html: string }) {
    const popup = window.fenestra?.popup;
    if (popup?.open) return popup.open(options);
    return window.fenestra?.bridge?.invoke("fenestra.popup.open", options);
  }

  function closeNativePopup() {
    const popup = window.fenestra?.popup;
    if (popup?.close) return popup.close();
    return window.fenestra?.bridge?.invoke("fenestra.popup.close", {});
  }

  function sessionMenuDocument() {
    const actions = [
      { command: "lock", icon: "lock", label: "Lock" },
      { command: "suspend", icon: "moon", label: "Suspend" },
      { command: "reboot", icon: "reboot", label: "Restart" },
      { command: "power-off", icon: "power", label: "Power Off", danger: true },
    ];
    const buttons = actions
      .map(
        (action, index) => `
          <button class="session-action ${action.danger ? "is-danger" : ""}" style="--index:${index}" data-command="${escapeHtml(action.command)}">
            <span class="session-action-icon">${iconSvg(action.icon)}</span>
            <span>${escapeHtml(action.label)}</span>
          </button>`,
      )
      .join("");
    return `<!doctype html>
      <html>
        <head>
          <meta charset="utf-8" />
          <style>${sessionMenuCss()}</style>
        </head>
        <body>
          <main class="session-actions is-open">${buttons}</main>
          <script>
            document.addEventListener("click", async (event) => {
              const button = event.target.closest("[data-command]");
              if (!button || !window.fenestra?.bridge) return;
              await window.fenestra.bridge.invoke("luft.action", {
                type: "session-command",
                command: button.dataset.command
              });
              window.fenestra.popup?.close?.();
            });
          <\/script>
        </body>
      </html>`;
  }

  function sessionMenuCss() {
    return `
      :root {
        color: rgba(248,248,246,.96);
        font-family: Geist, "Adwaita Sans", Cantarell, ui-sans-serif, system-ui, sans-serif;
        font-size: 14px;
        -webkit-font-smoothing: antialiased;
      }
      * { box-sizing: border-box; user-select: none; }
      html, body {
        width: 100%;
        height: 100%;
        margin: 0;
        overflow: hidden;
        background: transparent;
      }
      .session-actions {
        display: grid;
        gap: 4px;
        width: 100%;
        height: 100%;
        padding: 6px;
        border-radius: 18px;
        background: linear-gradient(rgba(20,20,18,.64), rgba(20,20,18,.64)), rgba(24,23,20,.34);
        box-shadow: inset 0 1px 0 rgba(255,255,255,.11), inset 0 0 0 1px rgba(255,255,255,.055);
      }
      .session-action {
        display: grid;
        grid-template-columns: 34px minmax(0, 1fr);
        align-items: center;
        gap: 10px;
        min-height: 36px;
        padding: 0 10px 0 6px;
        border: 0;
        border-radius: 12px;
        color: inherit;
        background: rgba(255,255,255,.075);
        font: inherit;
        text-align: left;
      }
      .session-action:hover { background: rgba(255,255,255,.12); }
      .session-action:active { transform: scale(.97); }
      .session-action-icon {
        display: grid;
        place-items: center;
        width: 30px;
        height: 30px;
        border-radius: 11px;
        background: rgba(255,255,255,.08);
      }
      .session-action svg {
        width: 16px;
        height: 16px;
        stroke: currentColor;
        stroke-width: 2;
        fill: none;
        stroke-linecap: round;
        stroke-linejoin: round;
      }
    `;
  }

  function iconSvg(name: string) {
    switch (name) {
      case "lock":
        return '<svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/></svg>';
      case "moon":
        return '<svg viewBox="0 0 24 24"><path d="M20 14.6A8 8 0 0 1 9.4 4 7 7 0 1 0 20 14.6Z"/></svg>';
      case "reboot":
        return '<svg viewBox="0 0 24 24"><path d="M3 12a9 9 0 1 0 3-6.7"/><path d="M3 4v5h5"/></svg>';
      default:
        return '<svg viewBox="0 0 24 24"><path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.8 0"/></svg>';
    }
  }

  function escapeHtml(value: string) {
    return value.replace(/[&<>"']/g, (char) => {
      switch (char) {
        case "&":
          return "&amp;";
        case "<":
          return "&lt;";
        case ">":
          return "&gt;";
        case '"':
          return "&quot;";
        default:
          return "&#39;";
      }
    });
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

  {#if powerMenuOpen && !powerMenuNative}
    <menu class="session-actions-fallback">
      <button type="button" onclick={() => runSessionCommand("lock")}>
        <Icon name="lock" />
        <span>Lock</span>
      </button>
      <button type="button" onclick={() => runSessionCommand("suspend")}>
        <Icon name="moon" />
        <span>Suspend</span>
      </button>
      <button type="button" onclick={() => runSessionCommand("reboot")}>
        <Icon name="reboot" />
        <span>Restart</span>
      </button>
      <button type="button" class="is-danger" onclick={() => runSessionCommand("power-off")}>
        <Icon name="power" />
        <span>Power Off</span>
      </button>
    </menu>
  {/if}

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
