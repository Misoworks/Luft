<script lang="ts">
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot, className }: { snapshot: ShellSnapshot; className: string } = $props();

  function toggleSettings(event: MouseEvent) {
    if ((event.target as Element).closest(".tray-item")) return;
    sendAction({
      type: snapshot.quickSettingsOpen ? "close-quick-settings" : "toggle-quick-settings",
    });
  }

  function activateTray(event: MouseEvent, index: number) {
    event.stopPropagation();
    sendAction({ type: "tray-activate", index });
  }

  function openTrayMenu(event: MouseEvent, index: number) {
    event.preventDefault();
    event.stopPropagation();
    sendAction({ type: "tray-menu", index });
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    sendAction({
      type: snapshot.quickSettingsOpen ? "close-quick-settings" : "toggle-quick-settings",
    });
  }
</script>

<div class={className} role="button" tabindex="0" aria-label="Quick settings" onclick={toggleSettings} onkeydown={keydown}>
  {#each snapshot.tray as item, index (item.title + index)}
    <button
      type="button"
      class="tray-item"
      aria-label={item.title}
      onclick={(event) => activateTray(event, index)}
      oncontextmenu={(event) => openTrayMenu(event, index)}
    >
      {#if item.iconUri}
        <img src={item.iconUri} alt="" />
      {/if}
    </button>
  {/each}

  <Icon name={snapshot.status.network?.wireless ? "wifi" : "network"} />
  {#if snapshot.status.audio}
    <Icon name="volume" />
  {/if}
  {#if snapshot.status.battery}
    <Icon name="battery" />
  {/if}
</div>
